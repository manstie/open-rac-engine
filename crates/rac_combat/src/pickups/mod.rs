//! Pickups and the bolt economy (PLAN.md P1.10): the bolts, nanotech and ammo that crates
//! ([`crates`]) and dead enemies leave, and a vendor stub that sells rounds ([`vendor`]).
//!
//! - Pickups pop out, bounce, and fly to a hero who comes near, as UYA's do. [`step`] is a
//!   pickup's whole motion as one pure function over the heroes as it sees them
//!   ([`Collector`]); [`move_pickups`] runs it for every [`Pickup`] each tick, in entity
//!   order, and gives what arrives to its hero: bolts to his [`Bolts`], nanotech to his
//!   `Health`, and ammo to his [`Arsenal`] (`Arsenal::add_ammo`, UYA's `ammo_add`).
//! - Ammo is for one gun, as UYA's ammo mobys are (`weapons.md`, "Owning, ammo, levels and
//!   XP": the pickup finds its gun by its class and adds the rounds its own data holds,
//!   here the gun's `pickup_rounds`). It flies only to a hero who owns the gun with room
//!   for a round, the nearest such, so in co-op none is wasted on a hero who can't use it.
//!   With no such hero near it lies where it is until it expires, and what doesn't fit when
//!   it arrives falls there too [placeholder: how UYA treats the ammo of a full or unowned
//!   gun isn't read].
//! - The numbers are in `assets/pickups.ron`.

pub mod crates;
pub mod vendor;

use bevy::prelude::*;
use rac_hero::{CollisionScene, CollisionWorld, Health, Hero, HeroBody, HeroState, HeroTuning};
use rac_sim::{SimRng, SimTransform};
use serde::{Deserialize, Serialize};

use crate::TICK;
use crate::weapons::{Armory, Arsenal, Hand, WeaponId};

pub use crates::{Crate, CrateKind, CrateTable};
pub use vendor::{Vendor, VendorTuning};

/// The bundled tuning.
pub const PICKUPS_RON: &str = include_str!("../../assets/pickups.ron");

/// How pickups move, and what crates and the vendor give.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PickupTuning {
    /// u/s².
    pub gravity: f32,
    pub radius: f32,
    /// The share of its speed into the ground it keeps as it bounces; slower than
    /// `rest_speed`, it stops.
    pub bounce: f32,
    pub rest_speed: f32,
    /// u/s², slowing it along the ground.
    pub ground_brake: f32,
    /// How fast it pops out: a range across the ground and a range up, u/s.
    pub pop_speed: (f32, f32),
    pub pop_lift: (f32, f32),
    /// Ticks after it pops out before it flies to a hero within `magnet_radius` of it.
    pub magnet_delay_ticks: u32,
    pub magnet_radius: f32,
    /// It flies at `fly_speed`, gaining `fly_accel` u/s², and is his within
    /// `collect_radius` of his middle.
    pub fly_speed: f32,
    pub fly_accel: f32,
    pub collect_radius: f32,
    /// Ticks before one no one collects vanishes, blinking for the last `blink_ticks`.
    pub life_ticks: u32,
    pub blink_ticks: u32,
    /// What each kind of crate holds.
    pub crates: CrateTable,
    pub vendor: VendorTuning,
}

#[derive(Debug, thiserror::Error)]
#[error("pickup tuning: {0}")]
pub struct PickupTuningError(#[from] ron::error::SpannedError);

impl PickupTuning {
    pub fn from_ron(text: &str) -> Result<Self, PickupTuningError> {
        Ok(ron::from_str(text)?)
    }
}

impl Default for PickupTuning {
    fn default() -> Self {
        // A test parses the bundled file, so this cannot fail at run time.
        Self::from_ron(PICKUPS_RON).expect("the bundled pickup tuning parses")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    /// This many bolts.
    Bolts(u32),
    /// Heals this much.
    Nanotech(u32),
    /// This many rounds for one gun.
    Ammo { weapon: WeaponId, rounds: u32 },
}

impl PickupKind {
    /// An ammo pickup for `weapon`, holding its `pickup_rounds`; `None` for a gun with
    /// none.
    pub fn ammo(weapon: WeaponId, armory: &Armory) -> Option<Self> {
        let rounds = armory.get(weapon)?.pickup_rounds;
        (rounds > 0).then_some(PickupKind::Ammo { weapon, rounds })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupMotion {
    pub kind: PickupKind,
    /// Its middle.
    pub position: Vec3,
    /// u/s.
    pub velocity: Vec3,
    /// Ticks since it popped out.
    pub age: u32,
    /// The player of the hero it flies to.
    pub flying_to: Option<usize>,
}

/// A bolt, nanotech orb or ammo lying about or flying to a hero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(SimTransform)]
pub struct Pickup(pub PickupMotion);

/// The bolts a hero has collected.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bolts(pub u32);

/// How a pickup ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupEnd {
    /// The hero of this player has it.
    Collected(usize),
    Expired,
}

/// A hero as a pickup sees him.
#[derive(Debug, Clone, PartialEq)]
pub struct Collector {
    pub player: usize,
    /// His middle, which pickups fly to.
    pub centre: Vec3,
    pub alive: bool,
    /// He keeps bolts (he has [`Bolts`]).
    pub keeps_bolts: bool,
    /// The rounds each of his guns has room for ([`ammo_room`]); a gun not listed takes
    /// none.
    pub ammo_room: Vec<(WeaponId, u32)>,
}

impl Collector {
    /// Whether he can use a pickup of `kind`, so it may fly to him.
    pub fn takes(&self, kind: PickupKind) -> bool {
        self.alive
            && match kind {
                PickupKind::Bolts(_) => self.keeps_bolts,
                PickupKind::Nanotech(_) => true,
                PickupKind::Ammo { weapon, .. } => self.room(weapon) > 0,
            }
    }

    pub fn room(&self, weapon: WeaponId) -> u32 {
        self.ammo_room
            .iter()
            .find(|(gun, _)| *gun == weapon)
            .map_or(0, |(_, room)| *room)
    }
}

/// The rounds each gun he owns has room for, in the order given; full guns, and guns that
/// use no ammo or have no ammo pickups, aren't listed.
pub fn ammo_room(arsenal: &Arsenal, armory: &Armory) -> Vec<(WeaponId, u32)> {
    arsenal
        .guns
        .iter()
        .filter_map(|gun| {
            let def = armory.get(gun.weapon)?;
            let room = def.level(gun.level).ammo.saturating_sub(gun.ammo);
            (room > 0 && def.pickup_rounds > 0).then_some((gun.weapon, room))
        })
        .collect()
}

/// The gun a hero is using: the one in his hand or coming out, else, with the wrench out,
/// the one his fire button brings out.
pub fn current_gun(arsenal: &Arsenal) -> Option<WeaponId> {
    match arsenal.hand {
        Hand::Gun { weapon, .. } | Hand::Drawing { weapon, .. } => Some(weapon),
        Hand::Wrench => arsenal.equipped,
    }
}

/// The gun that ammo from a crate or a kill is for, given the guns of the hero who broke or
/// killed it: the gun he is using if it has room, else one of his others with room, at
/// random. With every gun full, the gun he is using, else one of his at random, so the ammo
/// lies there until someone needs it; `None` if he has no gun with ammo pickups
/// [placeholder: the rule is the kit's own; UYA's ammo crates aren't read].
pub fn ammo_for(arsenal: &Arsenal, armory: &Armory, rng: &mut SimRng) -> Option<WeaponId> {
    let current = current_gun(arsenal);
    let room = ammo_room(arsenal, armory);
    if let Some(current) = current
        && room.iter().any(|(gun, _)| *gun == current)
    {
        return Some(current);
    }
    if !room.is_empty() {
        return Some(room[rng.below(room.len() as u32) as usize].0);
    }
    let has_pickups = |weapon: WeaponId| {
        let (Some(def), Some(gun)) = (armory.get(weapon), arsenal.gun(weapon)) else {
            return false;
        };
        def.level(gun.level).ammo > 0 && def.pickup_rounds > 0
    };
    if let Some(current) = current.filter(|&current| has_pickups(current)) {
        return Some(current);
    }
    let owned: Vec<WeaponId> = arsenal
        .guns
        .iter()
        .map(|gun| gun.weapon)
        .filter(|&weapon| has_pickups(weapon))
        .collect();
    if owned.is_empty() {
        return None;
    }
    Some(owned[rng.below(owned.len() as u32) as usize])
}

/// A uniform integer in the range, both ends included.
pub(crate) fn between(rng: &mut SimRng, (low, high): (u32, u32)) -> u32 {
    low + rng.below(high.saturating_sub(low).saturating_add(1))
}

/// Pops `kinds` out of `at`, each a random way.
pub fn scatter(
    at: Vec3,
    kinds: Vec<PickupKind>,
    tuning: &PickupTuning,
    rng: &mut SimRng,
) -> Vec<PickupMotion> {
    kinds
        .into_iter()
        .map(|kind| {
            let angle = rng.range_f32(0.0, std::f32::consts::TAU);
            let speed = rng.range_f32(tuning.pop_speed.0, tuning.pop_speed.1);
            let lift = rng.range_f32(tuning.pop_lift.0, tuning.pop_lift.1);
            let across = Vec3::new(
                bevy::math::ops::cos(angle),
                0.0,
                bevy::math::ops::sin(angle),
            );
            PickupMotion {
                kind,
                position: at,
                velocity: across * speed + Vec3::Y * lift,
                age: 0,
                flying_to: None,
            }
        })
        .collect()
}

/// One tick of a pickup: `None` while it stays about.
pub fn step(
    pickup: &mut PickupMotion,
    heroes: &[Collector],
    tuning: &PickupTuning,
    world: &dyn CollisionWorld,
) -> Option<PickupEnd> {
    pickup.age += 1;
    let kind = pickup.kind;
    let centre_of = |player: usize| {
        heroes
            .iter()
            .find(|hero| hero.player == player && hero.takes(kind))
            .map(|hero| hero.centre)
    };
    if pickup
        .flying_to
        .is_some_and(|player| centre_of(player).is_none())
    {
        // Its hero died, or filled the gun from another pickup: it drops where it is,
        // rather than flying on at its homing speed.
        pickup.flying_to = None;
        pickup.velocity = Vec3::ZERO;
    }
    if pickup.flying_to.is_none() {
        if pickup.age >= tuning.life_ticks {
            return Some(PickupEnd::Expired);
        }
        if pickup.age >= tuning.magnet_delay_ticks {
            let mut nearest: Option<(usize, f32)> = None;
            for hero in heroes.iter().filter(|hero| hero.takes(kind)) {
                let distance = hero.centre.distance(pickup.position);
                if distance <= tuning.magnet_radius && nearest.is_none_or(|(_, d)| distance < d) {
                    nearest = Some((hero.player, distance));
                }
            }
            if let Some((player, _)) = nearest {
                pickup.flying_to = Some(player);
                pickup.velocity = Vec3::ZERO;
            }
        }
    }

    if let Some(player) = pickup.flying_to
        && let Some(centre) = centre_of(player)
    {
        // Straight to him, through anything in the way, as the games' bolts go.
        let speed = (pickup.velocity.length().max(tuning.fly_speed)) + tuning.fly_accel * TICK;
        let to = centre - pickup.position;
        let distance = to.length();
        if distance <= tuning.collect_radius + speed * TICK {
            return Some(PickupEnd::Collected(player));
        }
        pickup.velocity = to / distance * speed;
        pickup.position += pickup.velocity * TICK;
        return None;
    }

    pickup.velocity.y -= tuning.gravity * TICK;
    let motion = pickup.velocity * TICK;
    let Some(hit) = world.cast_sphere(tuning.radius, pickup.position, motion) else {
        pickup.position += motion;
        return None;
    };
    pickup.position += motion * hit.fraction;
    let into = pickup.velocity.dot(hit.normal);
    if into < 0.0 {
        pickup.velocity -= hit.normal * into * (1.0 + tuning.bounce);
    }
    if hit.normal.y > 0.5 {
        if pickup.velocity.y.abs() < tuning.rest_speed {
            pickup.velocity.y = 0.0;
        }
        let across = Vec3::new(pickup.velocity.x, 0.0, pickup.velocity.z);
        let slower = (across.length() - tuning.ground_brake * TICK).max(0.0);
        let across = across.normalize_or_zero() * slower;
        pickup.velocity = Vec3::new(across.x, pickup.velocity.y, across.z);
    }
    None
}

/// Whether a pickup this old shows this tick: it blinks before it expires.
pub fn shows(pickup: &PickupMotion, tuning: &PickupTuning) -> bool {
    let left = tuning.life_ticks.saturating_sub(pickup.age);
    pickup.flying_to.is_some() || left > tuning.blink_ticks || (left / 6).is_multiple_of(2)
}

/// Puts `pickups` in the world.
pub fn spawn(commands: &mut Commands, pickups: Vec<PickupMotion>) {
    for pickup in pickups {
        let name = match pickup.kind {
            PickupKind::Bolts(_) => "bolt",
            PickupKind::Nanotech(_) => "nanotech",
            PickupKind::Ammo { .. } => "ammo",
        };
        commands.spawn((
            Name::new(name),
            Pickup(pickup),
            SimTransform::from_translation(pickup.position),
        ));
    }
}

type HeroItem<'a> = (
    &'a Hero,
    &'a HeroBody,
    &'a mut Health,
    Option<&'a mut Bolts>,
    Option<&'a mut Arsenal>,
);

/// Moves every pickup, and gives those collected to their heroes.
pub fn move_pickups(
    tuning: Res<PickupTuning>,
    armory: Res<Armory>,
    hero_tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    mut heroes: Query<HeroItem>,
    mut pickups: Query<(Entity, &mut Pickup, &mut SimTransform)>,
    mut commands: Commands,
) {
    let body = &hero_tuning.0.body;
    let middle = Vec3::Y * ((body.bottom + body.top) / 2.0);
    let mut collectors: Vec<Collector> = heroes
        .iter()
        .map(|(hero, hero_body, _, bolts, arsenal)| Collector {
            player: hero.player,
            centre: hero_body.0.position + middle,
            alive: hero_body.0.state != HeroState::Dead,
            keeps_bolts: bolts.is_some(),
            ammo_room: arsenal.map_or_else(Vec::new, |arsenal| ammo_room(arsenal, &armory)),
        })
        .collect();
    collectors.sort_by_key(|collector| collector.player);
    let mut pickups: Vec<_> = pickups.iter_mut().collect();
    pickups.sort_by_key(|(entity, ..)| *entity);
    for (entity, pickup, transform) in &mut pickups {
        let end = step(&mut pickup.0, &collectors, &tuning, &scene.0);
        transform.translation = pickup.0.position;
        let Some(end) = end else {
            continue;
        };
        let hero = match end {
            PickupEnd::Collected(player) => {
                heroes.iter_mut().find(|(hero, ..)| hero.player == player)
            }
            PickupEnd::Expired => None,
        };
        let Some((hero, _, mut health, bolts, arsenal)) = hero else {
            commands.entity(*entity).despawn();
            continue;
        };
        let left = match pickup.0.kind {
            PickupKind::Bolts(value) => {
                if let Some(mut bolts) = bolts {
                    bolts.0 = bolts.0.saturating_add(value);
                }
                0
            }
            PickupKind::Nanotech(amount) => {
                health.current = health.current.saturating_add(amount).min(health.max);
                0
            }
            PickupKind::Ammo { weapon, rounds } => {
                let Some(mut arsenal) = arsenal else {
                    commands.entity(*entity).despawn();
                    continue;
                };
                let left = arsenal.add_ammo(weapon, rounds, &armory);
                // Later pickups this tick see the room he has left.
                if let Some(collector) = collectors
                    .iter_mut()
                    .find(|collector| collector.player == hero.player)
                {
                    collector.ammo_room = ammo_room(&arsenal, &armory);
                }
                left
            }
        };
        if left == 0 {
            commands.entity(*entity).despawn();
        } else if let PickupKind::Ammo { weapon, .. } = pickup.0.kind {
            // What didn't fit falls where it is [placeholder], for a partner to take.
            pickup.0.kind = PickupKind::Ammo {
                weapon,
                rounds: left,
            };
            pickup.0.flying_to = None;
            pickup.0.velocity = Vec3::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::{BodyPlacement, Collider, ParryWorld};

    const N60: WeaponId = WeaponId(1);
    const NITRO: WeaponId = WeaponId(2);
    const LAVA: WeaponId = WeaponId(3);

    fn floor() -> ParryWorld {
        ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(50.0, 0.5, 50.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        )])
    }

    /// A hero standing at (x, 0, z), with room for `room`.
    fn hero(player: usize, x: f32, z: f32, room: &[(WeaponId, u32)]) -> Collector {
        Collector {
            player,
            centre: Vec3::new(x, 0.775, z),
            alive: true,
            keeps_bolts: true,
            ammo_room: room.to_vec(),
        }
    }

    /// Lying still on the floor, old enough to fly.
    fn lying(kind: PickupKind, tuning: &PickupTuning) -> PickupMotion {
        PickupMotion {
            kind,
            position: Vec3::new(0.0, tuning.radius, 0.0),
            velocity: Vec3::ZERO,
            age: tuning.magnet_delay_ticks,
            flying_to: None,
        }
    }

    /// Steps until it ends, within `limit` ticks.
    fn run(
        pickup: &mut PickupMotion,
        heroes: &[Collector],
        tuning: &PickupTuning,
        world: &ParryWorld,
        limit: u32,
    ) -> Option<PickupEnd> {
        (0..limit).find_map(|_| step(pickup, heroes, tuning, world))
    }

    #[test]
    fn the_bundled_tuning_parses_and_a_broken_file_is_an_error() {
        let tuning = PickupTuning::default();
        assert!(tuning.magnet_radius > tuning.collect_radius);
        assert!(PickupTuning::from_ron("(gravity: 30.0)").is_err());
    }

    #[test]
    fn scatter_pops_each_kind_out_upwards() {
        let tuning = PickupTuning::default();
        let mut rng = SimRng::new(7, 0);
        let kinds = vec![PickupKind::Bolts(5); 40];
        let drops = scatter(Vec3::Y, kinds, &tuning, &mut rng);
        assert_eq!(drops.len(), 40);
        for drop in &drops {
            assert_eq!(drop.position, Vec3::Y);
            assert!(drop.velocity.y >= tuning.pop_lift.0);
            let across = Vec3::new(drop.velocity.x, 0.0, drop.velocity.z).length();
            assert!(across >= tuning.pop_speed.0 - 1e-4 && across < tuning.pop_speed.1);
        }
    }

    #[test]
    fn a_drop_lands_and_settles_on_the_ground() {
        let tuning = PickupTuning::default();
        let world = floor();
        // The draws the demo's drop table made before this drop's, as when this test was
        // written there: its count and its nanotech roll.
        let mut rng = SimRng::new(1, 0);
        rng.below(4);
        rng.unit_f32();
        let mut drop = scatter(Vec3::Y, vec![PickupKind::Bolts(5)], &tuning, &mut rng)[0];
        for _ in 0..240 {
            assert_eq!(step(&mut drop, &[], &tuning, &world), None);
        }
        assert!(
            (drop.position.y - tuning.radius).abs() < 0.01,
            "{}",
            drop.position
        );
        assert_eq!(drop.velocity, Vec3::ZERO);
    }

    #[test]
    fn a_drop_waits_then_flies_to_the_nearest_hero_near_enough() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = PickupMotion {
            age: 0,
            ..lying(PickupKind::Nanotech(2), &tuning)
        };
        let heroes = [
            hero(0, 0.0, 5.0, &[]),
            hero(1, 3.0, 0.0, &[]),
            hero(2, -20.0, 0.0, &[]),
        ];
        let mut ticks = 0;
        let end = loop {
            ticks += 1;
            if ticks < tuning.magnet_delay_ticks {
                assert_eq!(drop.flying_to, None);
            }
            if let Some(end) = step(&mut drop, &heroes, &tuning, &world) {
                break end;
            }
            assert!(ticks < 120);
        };
        assert_eq!(end, PickupEnd::Collected(1));
        assert!(ticks > tuning.magnet_delay_ticks);
    }

    #[test]
    fn a_drop_whose_hero_dies_on_its_way_falls_where_it_is() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = lying(PickupKind::Bolts(5), &tuning);
        let mut heroes = [hero(0, 5.0, 0.0, &[])];
        for _ in 0..8 {
            assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, Some(0));
        let caught = drop.position;
        heroes[0].alive = false;
        for _ in 0..120 {
            assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, None);
        let moved = drop.position - caught;
        assert!(Vec3::new(moved.x, 0.0, moved.z).length() < 0.01, "{moved}");
        assert!(
            (drop.position.y - tuning.radius).abs() < 0.01,
            "{}",
            drop.position
        );
    }

    #[test]
    fn a_drop_no_one_collects_blinks_and_expires() {
        let tuning = PickupTuning::default();
        let world = floor();
        let mut drop = PickupMotion {
            age: 0,
            ..lying(PickupKind::Bolts(5), &tuning)
        };
        let mut hidden = 0;
        for _ in 1..tuning.life_ticks {
            assert_eq!(step(&mut drop, &[], &tuning, &world), None);
            hidden += u32::from(!shows(&drop, &tuning));
        }
        assert_eq!(
            step(&mut drop, &[], &tuning, &world),
            Some(PickupEnd::Expired)
        );
        assert!(hidden > 0 && hidden < tuning.blink_ticks, "{hidden}");
    }

    #[test]
    fn ammo_flies_past_a_nearer_hero_whose_gun_is_full_to_one_with_room() {
        let tuning = PickupTuning::default();
        let world = floor();
        let ammo = PickupKind::Ammo {
            weapon: N60,
            rounds: 30,
        };
        // Player 0 is nearest but his N60 is full; player 1 lacks it; player 2 has room.
        let heroes = [
            hero(0, 1.0, 0.0, &[(LAVA, 20)]),
            hero(1, 0.0, 2.0, &[]),
            hero(2, -4.0, 0.0, &[(N60, 5)]),
        ];
        let mut drop = lying(ammo, &tuning);
        assert_eq!(
            run(&mut drop, &heroes, &tuning, &world, 120),
            Some(PickupEnd::Collected(2))
        );
        // Bolts go to the nearest whatever his guns.
        let mut bolts = lying(PickupKind::Bolts(5), &tuning);
        assert_eq!(
            run(&mut bolts, &heroes, &tuning, &world, 120),
            Some(PickupEnd::Collected(0))
        );
    }

    #[test]
    fn ammo_no_one_near_can_use_lies_where_it_is() {
        let tuning = PickupTuning::default();
        let world = floor();
        let ammo = PickupKind::Ammo {
            weapon: NITRO,
            rounds: 2,
        };
        let heroes = [hero(0, 1.0, 0.0, &[(N60, 5)])];
        let mut drop = lying(ammo, &tuning);
        for _ in 0..240 {
            assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, None);
        // It settles a little into the floor, as resting drops do.
        let moved = drop.position - Vec3::new(0.0, tuning.radius, 0.0);
        assert!(Vec3::new(moved.x, 0.0, moved.z).length() < 0.01, "{moved}");
        assert!(moved.y.abs() < 0.02, "{moved}");
    }

    #[test]
    fn ammo_whose_hero_fills_the_gun_on_its_way_drops_where_it_is() {
        let tuning = PickupTuning::default();
        let world = floor();
        let ammo = PickupKind::Ammo {
            weapon: N60,
            rounds: 30,
        };
        let mut heroes = [hero(0, 5.0, 0.0, &[(N60, 5)])];
        let mut drop = lying(ammo, &tuning);
        for _ in 0..8 {
            assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        }
        assert_eq!(drop.flying_to, Some(0));
        heroes[0].ammo_room.clear();
        assert_eq!(step(&mut drop, &heroes, &tuning, &world), None);
        assert_eq!(drop.flying_to, None);
        assert_eq!(drop.velocity.y, -tuning.gravity * TICK);
    }

    #[test]
    fn a_hero_takes_ammo_only_for_a_gun_he_owns_with_room() {
        let armory = Armory::uya();
        let mut arsenal = Arsenal::default();
        arsenal.give(N60, &armory);
        arsenal.give(LAVA, &armory);
        assert!(ammo_room(&arsenal, &armory).is_empty());
        arsenal.gun_mut(N60).unwrap().ammo = 140;
        assert_eq!(ammo_room(&arsenal, &armory), [(N60, 10)]);
        let collector = Collector {
            ammo_room: ammo_room(&arsenal, &armory),
            ..hero(0, 0.0, 0.0, &[])
        };
        let n60 = PickupKind::ammo(N60, &armory).unwrap();
        let lava = PickupKind::ammo(LAVA, &armory).unwrap();
        let nitro = PickupKind::ammo(NITRO, &armory).unwrap();
        assert!(collector.takes(n60));
        assert!(!collector.takes(lava));
        assert!(!collector.takes(nitro));
        assert!(
            !Collector {
                alive: false,
                ..collector
            }
            .takes(n60)
        );
    }

    #[test]
    fn ammo_is_for_the_gun_he_uses_if_it_has_room_else_another_with_room() {
        let armory = Armory::uya();
        let mut rng = SimRng::new(5, 0);
        let mut arsenal = Arsenal::default();
        for weapon in [N60, NITRO, LAVA] {
            arsenal.give(weapon, &armory);
        }
        // The N60 is brought out by the fire button: with the wrench out it is his.
        assert_eq!(current_gun(&arsenal), Some(N60));
        arsenal.gun_mut(N60).unwrap().ammo = 100;
        arsenal.gun_mut(LAVA).unwrap().ammo = 100;
        assert_eq!(ammo_for(&arsenal, &armory, &mut rng), Some(N60));
        arsenal.hand = Hand::Gun {
            weapon: NITRO,
            ticks: 30,
        };
        // The Nitro Launcher in his hand is full: the N60 or the Lava Gun, at random.
        let picks: Vec<_> = (0..40)
            .map(|_| ammo_for(&arsenal, &armory, &mut rng).unwrap())
            .collect();
        assert!(picks.contains(&N60) && picks.contains(&LAVA), "{picks:?}");
        assert!(!picks.contains(&NITRO));
        // All full: for the gun in his hand, to lie there till he needs it.
        arsenal.gun_mut(N60).unwrap().ammo = 150;
        arsenal.gun_mut(LAVA).unwrap().ammo = 150;
        assert_eq!(ammo_for(&arsenal, &armory, &mut rng), Some(NITRO));
        assert_eq!(ammo_for(&Arsenal::default(), &armory, &mut rng), None);
    }
}
