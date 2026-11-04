//! Saving and restoring the simulation, for rollback co-op (PLAN.md P1.18, section 4.2).
//!
//! A rollback session keeps a [`Snapshot`] of every recent tick. When a remote player's
//! input turns out different from what was predicted, it loads the snapshot of the tick
//! the input belongs to and simulates forward again. For that to work:
//!
//! - Every kit plugin registers the components and resources its ticks change
//!   ([`RollbackApp`]). Anything a tick changes that isn't registered is not rolled back,
//!   and peers drift apart.
//! - Simulated entities carry a [`SimId`], numbered in the order the simulation spawns
//!   them, so it is the same on every peer. Ordering by `Entity` is not: its values depend
//!   on everything else the app has spawned, menus and effects included. Simulation code
//!   orders entities by `SimId`.
//! - The simulation removes entities with [`RetireExt::retire`] instead of despawning
//!   them. Outside a session that despawns; in one ([`RollbackWindow`]) it only disables
//!   the entity, so loading an older snapshot can bring it back under the same `Entity`,
//!   and every component that refers to it still does. Retired entities are buried once no
//!   snapshot from before their retirement can be loaded.

use bevy::{
    ecs::{
        component::Mutable, entity::EntityHashSet, entity_disabling::Disabled,
        lifecycle::HookContext, query::Allow, world::DeferredWorld,
    },
    prelude::*,
};
use std::{
    any::{Any, TypeId},
    sync::Arc,
};

use crate::{SimRng, SimTick, SimTransform};

/// A simulated entity's number, the same on every peer: entities are numbered in the
/// order they are spawned, from 1, while [`SimIds`] exists. Simulation code that needs a
/// stable order sorts by it.
///
/// It is given by a hook when the component is added. The components that make something
/// part of the simulation require it (`rac_hero::Hero`, `rac_hero::Collider` and so on),
/// while `SimTransform` doesn't, so effects that move with the tick stay out of it. An
/// entity spawned without [`SimIds`] keeps 0.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[component(on_add = number_entity)]
pub struct SimId(u32);

impl SimId {
    pub fn get(self) -> u32 {
        self.0
    }
}

/// The next [`SimId`] to give. Rolled back with the rest of the simulation, so entities
/// spawned again after a rollback get the numbers they had.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimIds {
    next: u32,
}

impl Default for SimIds {
    fn default() -> Self {
        Self { next: 1 }
    }
}

fn number_entity(mut world: DeferredWorld, context: HookContext) {
    let Some(mut ids) = world.get_resource_mut::<SimIds>() else {
        return;
    };
    let id = ids.next;
    ids.next += 1;
    if let Some(mut sim_id) = world.get_mut::<SimId>(context.entity) {
        sim_id.0 = id;
    }
}

/// While this exists, a rollback session is running, and [`RetireExt::retire`] keeps what
/// it removes until no snapshot from before can be loaded.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RollbackWindow;

/// A simulated entity the simulation removed during tick `tick`, kept disabled in case a
/// rollback brings it back.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retired {
    pub tick: u64,
}

/// How the simulation removes entities.
pub trait RetireExt {
    /// Despawns the entity, or, during a rollback session, disables it and its children
    /// (see the module docs). Retiring an entity twice does nothing more.
    fn retire(&mut self);
}

impl RetireExt for EntityCommands<'_> {
    fn retire(&mut self) {
        self.queue_silenced(retire);
    }
}

/// [`RetireExt::retire`] as an entity command.
pub fn retire(mut entity: EntityWorldMut) {
    let world = entity.world();
    if world.get_resource::<RollbackWindow>().is_none() || !entity.contains::<SimId>() {
        entity.despawn();
        return;
    }
    if entity.contains::<Retired>() {
        return;
    }
    let tick = world.get_resource::<SimTick>().map_or(0, |tick| tick.0);
    entity.insert(Retired { tick });
    entity.insert_recursive::<Children>(Disabled);
}

/// Despawns the entities retired before `tick`, which no rollback can bring back any more.
pub fn bury_retired(world: &mut World, tick: u64) {
    let buried: Vec<Entity> = world
        .query_filtered::<(Entity, &Retired), Allow<Disabled>>()
        .iter(world)
        .filter(|(_, retired)| retired.tick < tick)
        .map(|(entity, _)| entity)
        .collect();
    for entity in buried {
        world.despawn(entity);
    }
}

/// Builds the checksum peers compare to find out that they have drifted apart: FNV-1a,
/// 128 bits, which gives the same value on every platform.
///
/// Only values that are the same on every peer may go in, so never an `Entity`.
#[derive(Debug, Clone, Copy)]
pub struct StateHasher(u128);

impl Default for StateHasher {
    fn default() -> Self {
        Self(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d)
    }
}

impl StateHasher {
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    pub fn finish128(&self) -> u128 {
        self.0
    }

    pub fn f32(&mut self, value: f32) {
        std::hash::Hasher::write(self, &value.to_bits().to_le_bytes());
    }

    pub fn vec3(&mut self, value: Vec3) {
        for axis in value.to_array() {
            self.f32(axis);
        }
    }

    pub fn quat(&mut self, value: Quat) {
        for part in value.to_array() {
            self.f32(part);
        }
    }
}

impl std::hash::Hasher for StateHasher {
    fn finish(&self) -> u64 {
        self.0 as u64
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u128::from(byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }
}

/// A registered type's hash, `fn(&T, &mut StateHasher)`, kept type-erased.
type AnyHash = Arc<dyn Any + Send + Sync>;
/// How one registered type saves itself, given its hash if it has one.
type SaveFn = fn(&mut World, Option<&AnyHash>) -> Box<dyn Saved>;

struct Part {
    type_id: TypeId,
    save: SaveFn,
    hash: Option<AnyHash>,
}

/// The components and resources that make up the simulation, in registration order.
#[derive(Resource, Default)]
pub struct RollbackRegistry {
    parts: Vec<Part>,
}

impl RollbackRegistry {
    fn add<T: 'static>(&mut self, save: SaveFn, hash: Option<fn(&T, &mut StateHasher)>) {
        let hash = hash.map(|hash| Arc::new(hash) as AnyHash);
        match self
            .parts
            .iter_mut()
            .find(|part| part.type_id == TypeId::of::<T>())
        {
            Some(part) => part.hash = hash.or(part.hash.take()),
            None => self.parts.push(Part {
                type_id: TypeId::of::<T>(),
                save,
                hash,
            }),
        }
    }
}

/// The typed hash a registry entry holds for `T`.
fn typed_hash<T: 'static>(hash: Option<&AnyHash>) -> Option<fn(&T, &mut StateHasher)> {
    hash?.downcast_ref::<fn(&T, &mut StateHasher)>().copied()
}

/// Registers what the simulation is made of (see the module docs).
pub trait RollbackApp {
    /// Rolls back `C` on every entity with a [`SimId`].
    fn roll_back_component<C: Component<Mutability = Mutable> + Clone>(&mut self) -> &mut Self;

    /// Rolls back `C` and puts it in the checksum with `hash`, which must leave out
    /// anything that differs between peers, such as `Entity` values.
    fn roll_back_component_hashed<C: Component<Mutability = Mutable> + Clone>(
        &mut self,
        hash: fn(&C, &mut StateHasher),
    ) -> &mut Self;

    /// Rolls back the resource `R`, including whether it exists.
    fn roll_back_resource<R: Resource<Mutability = Mutable> + Clone>(&mut self) -> &mut Self;

    /// Rolls back `R` and puts it in the checksum with `hash`.
    fn roll_back_resource_hashed<R: Resource<Mutability = Mutable> + Clone>(
        &mut self,
        hash: fn(&R, &mut StateHasher),
    ) -> &mut Self;
}

impl RollbackApp for App {
    fn roll_back_component<C: Component<Mutability = Mutable> + Clone>(&mut self) -> &mut Self {
        self.init_resource::<RollbackRegistry>();
        self.world_mut()
            .resource_mut::<RollbackRegistry>()
            .add::<C>(save_component::<C>, None);
        self
    }

    fn roll_back_component_hashed<C: Component<Mutability = Mutable> + Clone>(
        &mut self,
        hash: fn(&C, &mut StateHasher),
    ) -> &mut Self {
        self.init_resource::<RollbackRegistry>();
        self.world_mut()
            .resource_mut::<RollbackRegistry>()
            .add::<C>(save_component::<C>, Some(hash));
        self
    }

    fn roll_back_resource<R: Resource<Mutability = Mutable> + Clone>(&mut self) -> &mut Self {
        self.init_resource::<RollbackRegistry>();
        self.world_mut()
            .resource_mut::<RollbackRegistry>()
            .add::<R>(save_resource::<R>, None);
        self
    }

    fn roll_back_resource_hashed<R: Resource<Mutability = Mutable> + Clone>(
        &mut self,
        hash: fn(&R, &mut StateHasher),
    ) -> &mut Self {
        self.init_resource::<RollbackRegistry>();
        self.world_mut()
            .resource_mut::<RollbackRegistry>()
            .add::<R>(save_resource::<R>, Some(hash));
        self
    }
}

/// One registered type's saved state.
trait Saved: Send + Sync {
    fn restore(&self, world: &mut World);
    fn hash(&self, hasher: &mut StateHasher);
}

struct SavedComponent<C> {
    /// In `SimId` order, so the checksum is the same on every peer.
    items: Vec<(SimId, Entity, C)>,
    hash: Option<fn(&C, &mut StateHasher)>,
}

fn save_component<C: Component<Mutability = Mutable> + Clone>(
    world: &mut World,
    hash: Option<&AnyHash>,
) -> Box<dyn Saved> {
    let mut items: Vec<(SimId, Entity, C)> = world
        .query::<(Entity, &SimId, &C)>()
        .iter(world)
        .map(|(entity, id, component)| (*id, entity, component.clone()))
        .collect();
    items.sort_by_key(|(id, ..)| *id);
    Box::new(SavedComponent {
        items,
        hash: typed_hash::<C>(hash),
    })
}

impl<C: Component<Mutability = Mutable> + Clone> Saved for SavedComponent<C> {
    fn restore(&self, world: &mut World) {
        let kept: EntityHashSet = self.items.iter().map(|(_, entity, _)| *entity).collect();
        let gained: Vec<Entity> = world
            .query_filtered::<Entity, (With<C>, With<SimId>)>()
            .iter(world)
            .filter(|entity| !kept.contains(entity))
            .collect();
        for entity in gained {
            world.entity_mut(entity).remove::<C>();
        }
        for (_, entity, saved) in &self.items {
            let Ok(mut entity) = world.get_entity_mut(*entity) else {
                continue;
            };
            match entity.get_mut::<C>() {
                Some(mut current) => *current = saved.clone(),
                None => {
                    entity.insert(saved.clone());
                }
            }
        }
    }

    fn hash(&self, hasher: &mut StateHasher) {
        let Some(hash) = self.hash else {
            return;
        };
        for (id, _, component) in &self.items {
            std::hash::Hash::hash(id, hasher);
            hash(component, hasher);
        }
    }
}

struct SavedResource<R> {
    value: Option<R>,
    hash: Option<fn(&R, &mut StateHasher)>,
}

fn save_resource<R: Resource<Mutability = Mutable> + Clone>(
    world: &mut World,
    hash: Option<&AnyHash>,
) -> Box<dyn Saved> {
    Box::new(SavedResource {
        value: world.get_resource::<R>().cloned(),
        hash: typed_hash::<R>(hash),
    })
}

impl<R: Resource<Mutability = Mutable> + Clone> Saved for SavedResource<R> {
    fn restore(&self, world: &mut World) {
        match &self.value {
            Some(saved) => match world.get_resource_mut::<R>() {
                Some(mut current) => *current = saved.clone(),
                None => world.insert_resource(saved.clone()),
            },
            None => {
                world.remove_resource::<R>();
            }
        }
    }

    fn hash(&self, hasher: &mut StateHasher) {
        if let (Some(hash), Some(value)) = (self.hash, &self.value) {
            hash(value, hasher);
        }
    }
}

/// The simulation as it was at the start of a tick.
#[derive(Clone)]
pub struct Snapshot(Arc<SnapshotData>);

struct SnapshotData {
    tick: u64,
    next_id: u32,
    parts: Vec<Box<dyn Saved>>,
}

impl Snapshot {
    /// Saves every registered component and resource.
    pub fn save(world: &mut World) -> Self {
        let tick = world.get_resource::<SimTick>().map_or(0, |tick| tick.0);
        let next_id = world
            .get_resource::<SimIds>()
            .map_or(SimIds::default().next, |ids| ids.next);
        let parts: Vec<(SaveFn, Option<AnyHash>)> = world
            .get_resource::<RollbackRegistry>()
            .map(|registry| {
                registry
                    .parts
                    .iter()
                    .map(|part| (part.save, part.hash.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let parts = parts
            .into_iter()
            .map(|(save, hash)| save(world, hash.as_ref()))
            .collect();
        Self(Arc::new(SnapshotData {
            tick,
            next_id,
            parts,
        }))
    }

    /// Puts the simulation back as it was: entities spawned since are despawned, those
    /// retired since come back, and every registered component and resource takes its
    /// saved value.
    pub fn load(&self, world: &mut World) {
        let data = &self.0;
        let spawned_since: Vec<Entity> = world
            .query_filtered::<(Entity, &SimId), Allow<Disabled>>()
            .iter(world)
            .filter(|(_, id)| id.0 >= data.next_id)
            .map(|(entity, _)| entity)
            .collect();
        for entity in spawned_since {
            // A child may have gone with its parent already.
            if let Ok(entity) = world.get_entity_mut(entity) {
                entity.despawn();
            }
        }
        let retired_since: Vec<Entity> = world
            .query_filtered::<(Entity, &Retired), Allow<Disabled>>()
            .iter(world)
            .filter(|(_, retired)| retired.tick >= data.tick)
            .map(|(entity, _)| entity)
            .collect();
        for entity in retired_since {
            world
                .entity_mut(entity)
                .remove::<Retired>()
                .remove_recursive::<Children, Disabled>();
        }
        for part in &data.parts {
            part.restore(world);
        }
    }

    /// The tick this snapshot was taken at the start of.
    pub fn tick(&self) -> u64 {
        self.0.tick
    }

    /// A checksum of every part registered with a hash, which peers compare.
    pub fn checksum(&self) -> u128 {
        let mut hasher = StateHasher::default();
        std::hash::Hash::hash(&self.0.tick, &mut hasher);
        std::hash::Hash::hash(&self.0.next_id, &mut hasher);
        for part in &self.0.parts {
            part.hash(&mut hasher);
        }
        hasher.finish128()
    }
}

/// Registers the simulation core's own state.
pub(crate) fn register(app: &mut App) {
    app.init_resource::<SimIds>()
        .roll_back_resource_hashed::<SimTick>(std::hash::Hash::hash)
        .roll_back_resource_hashed::<SimRng>(std::hash::Hash::hash)
        .roll_back_resource::<SimIds>()
        .roll_back_component_hashed::<SimTransform>(|transform, hasher| {
            hasher.vec3(transform.translation);
            hasher.quat(transform.rotation);
        })
        .roll_back_component::<crate::PreviousSimTransform>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimMessageApp, SimMessageReader, SimPlugin, SimSet};
    use bevy::app::FixedMain;

    #[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
    #[require(SimId)]
    struct Counter(u32);

    /// Spawned by a tick, and removed again by `retire`.
    #[derive(Component, Debug, Clone, Copy, PartialEq)]
    #[require(SimId)]
    struct Spark;

    #[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
    struct Ping(u64);

    /// What the reader has taken, in the order it took it.
    #[derive(Resource, Debug, Default, Clone, PartialEq)]
    struct Heard(Vec<u64>);

    fn hear(mut pings: SimMessageReader<Ping>, mut heard: ResMut<Heard>) {
        heard.0.extend(pings.read().map(|ping| ping.0));
    }

    fn ping(tick: Res<SimTick>, mut pings: MessageWriter<Ping>) {
        pings.write(Ping(tick.0));
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin))
            .roll_back_component_hashed::<Counter>(|counter, hasher| {
                std::hash::Hash::hash(&counter.0, hasher);
            })
            .roll_back_component::<Spark>();
        app
    }

    /// Runs one tick, as the fixed loop or a session would.
    fn tick(app: &mut App) {
        app.world_mut().run_schedule(FixedMain);
    }

    #[test]
    fn loading_undoes_changes_spawns_and_retirements() {
        let mut app = app();
        app.insert_resource(RollbackWindow);
        let world = app.world_mut();
        let kept = world
            .spawn((Counter(1), SimTransform::from_xyz(1.0, 0.0, 0.0)))
            .id();
        let doomed = world.spawn(Counter(2)).id();
        let child = world.spawn(ChildOf(doomed)).id();
        let snapshot = Snapshot::save(world);

        world.get_mut::<Counter>(kept).unwrap().0 = 10;
        world.get_mut::<SimTransform>(kept).unwrap().translation.x = 5.0;
        world.entity_mut(kept).insert(Spark);
        let spawned = world.spawn(Counter(3)).id();
        world.commands().entity(doomed).retire();
        world.flush();
        assert!(world.get::<Disabled>(doomed).is_some());
        assert!(world.get::<Disabled>(child).is_some());
        assert_ne!(Snapshot::save(world).checksum(), snapshot.checksum());

        snapshot.load(world);
        assert_eq!(world.get::<Counter>(kept), Some(&Counter(1)));
        assert_eq!(world.get::<SimTransform>(kept).unwrap().translation.x, 1.0);
        assert!(world.get::<Spark>(kept).is_none());
        assert!(world.get_entity(spawned).is_err());
        // Back under the same `Entity`, enabled, children too.
        assert_eq!(world.get::<Counter>(doomed), Some(&Counter(2)));
        assert!(world.get::<Disabled>(doomed).is_none());
        assert!(world.get::<Disabled>(child).is_none());
        assert_eq!(Snapshot::save(world).checksum(), snapshot.checksum());
    }

    #[test]
    fn entities_spawned_again_get_the_same_numbers() {
        let mut app = app();
        let world = app.world_mut();
        let first = world.spawn(Counter(0)).id();
        let snapshot = Snapshot::save(world);
        let second = world.spawn(Counter(0)).id();
        let number = *world.get::<SimId>(second).unwrap();
        assert!(number > *world.get::<SimId>(first).unwrap());
        snapshot.load(world);
        let again = world.spawn(Counter(0)).id();
        assert_eq!(*world.get::<SimId>(again).unwrap(), number);
    }

    #[test]
    fn outside_a_session_retiring_despawns() {
        let mut app = app();
        let world = app.world_mut();
        let entity = world.spawn(Counter(0)).id();
        world.commands().entity(entity).retire();
        world.flush();
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn the_retired_are_buried_once_out_of_reach() {
        let mut app = app();
        app.insert_resource(RollbackWindow);
        let world = app.world_mut();
        world.resource_mut::<SimTick>().0 = 7;
        let entity = world.spawn(Counter(0)).id();
        world.commands().entity(entity).retire();
        world.flush();
        bury_retired(world, 7);
        assert!(world.get_entity(entity).is_ok());
        bury_retired(world, 8);
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn a_message_written_after_its_reader_survives_a_rollback_once() {
        let mut app = app();
        app.add_sim_message::<Ping>()
            .init_resource::<Heard>()
            .roll_back_resource::<Heard>()
            .add_systems(
                FixedUpdate,
                (
                    hear.in_set(SimSet::World),
                    // Written after the reader, so it is read on the next tick.
                    ping.in_set(SimSet::Particles),
                ),
            );
        tick(&mut app); // Tick 0 writes 0.
        let snapshot = Snapshot::save(app.world_mut());
        tick(&mut app); // Tick 1 hears 0, writes 1.
        tick(&mut app); // Tick 2 hears 1, writes 2.
        assert_eq!(app.world().resource::<Heard>().0, [0, 1]);

        snapshot.load(app.world_mut());
        assert_eq!(app.world().resource::<Heard>().0, [] as [u64; 0]);
        tick(&mut app);
        tick(&mut app);
        tick(&mut app);
        // 0 again from the snapshot, never 2 from the abandoned future.
        assert_eq!(app.world().resource::<Heard>().0, [0, 1, 2]);
    }
}
