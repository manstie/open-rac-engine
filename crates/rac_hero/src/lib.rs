//! The hero kit's player character (PLAN.md P1.5): a kinematic capsule that moves like
//! Ratchet in R&C3/UYA, from numbers read in the games' code
//! (`docs/research/hero-controller.md`).
//!
//! - [`controller::step`] is the whole controller as one pure function; the plugin runs it
//!   for every [`Hero`] each tick in `rac_sim::SimSet::Hero`.
//! - Solid geometry comes from [`Collider`] components, with an optional [`Material`],
//!   gathered into a [`CollisionScene`] each tick, together with [`GrindRail`]s and
//!   [`Water`]. The controller itself only sees the narrow [`CollisionWorld`] interface.
//! - [`HeroTuning`] holds the movement profile, UYA NTSC-U's unless the app sets another,
//!   such as Deadlocked's ([`HeroProfile::dl`]); the games differ only in its numbers.
//! - Health, hits ([`HurtHero`]), death and [`Checkpoint`]s are handled here, around the
//!   controller.
//! - The wrench's attacks turn to [`AimTarget`]s, and what they can hit each tick is in
//!   [`HeroMotion::strikes`] for the game's combat code (`rac_combat`) to test.
//! - [`FollowCamera`] is a basic third-person camera (P1.7).
//! - Strafing faces the camera; with a gamepad, [`LockOn`] holds a target that the camera
//!   turns to.

pub mod camera;
pub mod collision;
pub mod controller;
pub mod lock_on;
pub mod motor;
pub mod profile;
pub mod rail;
pub mod stick;
pub mod water;

use bevy::prelude::*;
use rac_input::PlayerInputs;
use rac_sim::{PreviousSimTransform, SimSet, SimTransform};

pub use camera::FollowCamera;
pub use collision::{BodyPlacement, Collider, CollisionWorld, Material, ParryWorld};
pub use controller::{
    Abilities, Clank, HeroMotion, HeroState, JumpKind, Ledge, Side, StrafeSide, Strike,
    StrikeShape, ThrownWrench,
};
pub use lock_on::LockOn;
pub use profile::HeroProfile;
pub use rail::{GrindRail, RailKind};
pub use water::Water;

/// A player-controlled hero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(SimTransform, LockOn)]
pub struct Hero {
    /// The `PlayerInputs` slot that steers him.
    pub player: usize,
    /// Where he comes back after dying or falling out of the level. Reaching a
    /// [`Checkpoint`] moves it.
    pub spawn: Vec3,
    pub spawn_yaw: f32,
    /// Falling below this height sends him back to `spawn`.
    pub respawn_below: f32,
}

/// Something the hero's attacks turn to, `height` above the entity's position (its
/// `SimTransform` if it moves in the simulation, otherwise its `Transform`).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AimTarget {
    pub height: f32,
}

/// The controller's state for a [`Hero`].
#[derive(Component, Debug, Clone, PartialEq)]
pub struct HeroBody(pub HeroMotion);

/// A hero's health: UYA's nanotech.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Health {
    pub fn full(max: u32) -> Self {
        Self { current: max, max }
    }
}

/// Reaching this box makes its position the hero's spawn point. The box is centred on the
/// entity's `Transform`, and the hero comes back standing at that point.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Checkpoint {
    pub half_extents: Vec3,
    pub spawn_yaw: f32,
}

/// Hurts the hero of `player`: takes `damage` and knocks him back with `knockback` (u/s).
/// Ignored while he is still safe from the last hit.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct HurtHero {
    pub player: usize,
    pub damage: u32,
    pub knockback: Vec3,
}

/// The movement profile every hero uses. Replacing it changes how the heroes move from
/// the next tick.
#[derive(Resource, Debug, Clone)]
pub struct HeroTuning(pub HeroProfile);

impl Default for HeroTuning {
    fn default() -> Self {
        Self(HeroProfile::uya())
    }
}

/// Every [`Collider`] in the world, as the controller sees it this tick.
#[derive(Resource, Debug, Default)]
pub struct CollisionScene(pub ParryWorld);

/// What a new hero needs: spawn it, then add a model.
pub fn hero_bundle(
    player: usize,
    spawn: Vec3,
    spawn_yaw: f32,
    respawn_below: f32,
    profile: &HeroProfile,
) -> impl Bundle {
    (
        Hero {
            player,
            spawn,
            spawn_yaw,
            respawn_below,
        },
        HeroBody(HeroMotion::new(spawn, spawn_yaw)),
        Health::full(profile.hit.max_health),
        SimTransform::from_translation(spawn).with_rotation(Quat::from_rotation_y(spawn_yaw)),
    )
}

pub struct HeroPlugin;

impl Plugin for HeroPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeroTuning>()
            .init_resource::<CollisionScene>()
            .add_message::<HurtHero>()
            .add_systems(
                FixedUpdate,
                (
                    gather_colliders,
                    move_heroes,
                    lock_on_targets,
                    reach_checkpoints,
                )
                    .chain()
                    .in_set(SimSet::Hero),
            )
            .add_systems(FixedUpdate, camera::follow.in_set(SimSet::Camera));
    }
}

/// Rebuilds the [`CollisionScene`] from every [`Collider`], [`GrindRail`] (rails in world
/// space), [`Water`] (placed by its `Transform`) and [`AimTarget`]. Colliders with a
/// `SimTransform` may move, and the hero rides them; others are placed by their
/// `Transform`. They are ordered by entity so every peer builds the same list.
fn gather_colliders(
    colliders: Query<(
        Entity,
        &Collider,
        &Transform,
        Option<&Material>,
        Option<(&SimTransform, &PreviousSimTransform)>,
    )>,
    rails: Query<(Entity, &GrindRail)>,
    water: Query<(Entity, &Water, &Transform)>,
    targets: Query<(
        Entity,
        &AimTarget,
        Option<&Transform>,
        Option<&SimTransform>,
    )>,
    mut scene: ResMut<CollisionScene>,
) {
    let mut target_list: Vec<(Entity, Vec3)> = targets
        .iter()
        .filter_map(|(entity, target, transform, simulated)| {
            let at = match simulated {
                Some(simulated) => simulated.translation,
                None => transform?.translation,
            };
            Some((entity, at + Vec3::Y * target.height))
        })
        .collect();
    target_list.sort_by_key(|(entity, _)| *entity);
    scene
        .0
        .set_targets(target_list.into_iter().map(|(_, at)| at).collect());
    let mut water_list: Vec<(Entity, water::WaterVolume)> = water
        .iter()
        .map(|(entity, water, transform)| {
            (
                entity,
                water::WaterVolume::new(transform.translation, water.half_extents),
            )
        })
        .collect();
    water_list.sort_by_key(|(entity, _)| *entity);
    scene
        .0
        .set_water(water_list.into_iter().map(|(_, volume)| volume).collect());
    let mut rail_list: Vec<(Entity, GrindRail)> = rails
        .iter()
        .map(|(entity, rail)| (entity, rail.clone()))
        .collect();
    rail_list.sort_by_key(|(entity, _)| *entity);
    scene
        .0
        .set_rails(rail_list.into_iter().map(|(_, rail)| rail).collect());
    let mut bodies: Vec<(Entity, BodyPlacement)> = colliders
        .iter()
        .map(|(entity, collider, transform, material, simulated)| {
            let material = material.copied().unwrap_or_default();
            let placement = match simulated {
                Some((current, previous)) => BodyPlacement {
                    collider: *collider,
                    translation: current.translation,
                    rotation: current.rotation,
                    motion: current.translation - previous.0.translation,
                    material,
                },
                None => BodyPlacement {
                    material,
                    ..BodyPlacement::fixed(*collider, transform.translation, transform.rotation)
                },
            };
            (entity, placement)
        })
        .collect();
    bodies.sort_by_key(|(entity, _)| *entity);
    let placements: Vec<BodyPlacement> = bodies.into_iter().map(|(_, body)| body).collect();
    scene.0.set_bodies(&placements);
}

fn move_heroes(
    inputs: Res<PlayerInputs>,
    tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    cameras: Query<&FollowCamera>,
    mut hurts: MessageReader<HurtHero>,
    mut heroes: Query<(
        &Hero,
        &mut HeroBody,
        &mut Health,
        &mut SimTransform,
        &mut PreviousSimTransform,
    )>,
) {
    let profile = &tuning.0;
    let mut heroes: Vec<_> = heroes.iter_mut().collect();
    heroes.sort_by_key(|(hero, ..)| hero.player);

    for hurt in hurts.read() {
        if let Some((_, body, health, ..)) = heroes
            .iter_mut()
            .find(|(hero, ..)| hero.player == hurt.player)
        {
            hurt_hero(&mut body.0, health, hurt.damage, hurt.knockback, profile);
        }
    }

    for (hero, mut body, mut health, mut transform, mut previous) in heroes {
        let camera_yaw = cameras
            .iter()
            .find(|camera| camera.player == hero.player)
            .map_or(0.0, |camera| camera.yaw);
        let events = controller::step(
            &mut body.0,
            &inputs.get(hero.player),
            camera_yaw,
            profile,
            &scene.0,
        );
        if events.hazard {
            let knockback = controller::hazard_knockback(&body.0, profile);
            hurt_hero(
                &mut body.0,
                &mut health,
                profile.hit.hazard_damage,
                knockback,
                profile,
            );
            body.0.stay_safe_for(profile.hit.hazard_safe_ticks);
        }
        let teleport = body.0.position.y < hero.respawn_below || body.0.respawn_due(profile);
        if teleport {
            if body.0.state == HeroState::Dead {
                *health = Health::full(health.max);
            }
            body.0 = HeroMotion::new(hero.spawn, hero.spawn_yaw);
        }
        transform.translation = body.0.position;
        // Nose down is a turn about his own x axis.
        transform.rotation =
            Quat::from_rotation_y(body.0.yaw) * Quat::from_rotation_x(-body.0.pitch);
        if teleport {
            previous.0 = *transform;
        }
    }
}

/// Picks, keeps and drops each hero's [`LockOn`] target among the [`AimTarget`]s.
fn lock_on_targets(
    inputs: Res<PlayerInputs>,
    tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    cameras: Query<&FollowCamera>,
    targets: Query<(
        Entity,
        &AimTarget,
        Option<&Transform>,
        Option<&SimTransform>,
    )>,
    mut heroes: Query<(&Hero, &HeroBody, &mut LockOn)>,
) {
    let mut target_list: Vec<(Entity, Vec3)> = targets
        .iter()
        .filter_map(|(entity, target, transform, simulated)| {
            let at = match simulated {
                Some(simulated) => simulated.translation,
                None => transform?.translation,
            };
            Some((entity, at + Vec3::Y * target.height))
        })
        .collect();
    target_list.sort_by_key(|(entity, _)| *entity);
    for (hero, body, mut lock) in &mut heroes {
        let camera_yaw = cameras
            .iter()
            .find(|camera| camera.player == hero.player)
            .map_or(body.0.yaw, |camera| camera.yaw);
        lock_on::update(
            &mut lock,
            &body.0,
            &inputs.get(hero.player),
            camera_yaw,
            &target_list,
            &tuning.0.lock_on,
            &scene.0,
        );
    }
}

fn hurt_hero(
    body: &mut HeroMotion,
    health: &mut Health,
    damage: u32,
    knockback: Vec3,
    profile: &HeroProfile,
) {
    if !body.can_be_hurt() {
        return;
    }
    health.current = health.current.saturating_sub(damage);
    controller::hurt(body, knockback, health.current == 0, profile);
}

/// Moves a hero's spawn point to any [`Checkpoint`] he stands in.
fn reach_checkpoints(
    checkpoints: Query<(Entity, &Checkpoint, &Transform)>,
    mut heroes: Query<(&mut Hero, &HeroBody)>,
) {
    let mut checkpoints: Vec<_> = checkpoints.iter().collect();
    checkpoints.sort_by_key(|(entity, ..)| *entity);
    for (mut hero, body) in &mut heroes {
        if body.0.state == HeroState::Dead {
            continue;
        }
        let feet = body.0.position;
        let reached = checkpoints.iter().find(|(_, checkpoint, transform)| {
            let local = feet - transform.translation;
            local.abs().cmple(checkpoint.half_extents).all()
        });
        if let Some((_, checkpoint, transform)) = reached {
            hero.spawn = transform.translation;
            hero.spawn_yaw = checkpoint.spawn_yaw;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use rac_sim::{SimPlugin, TICK_HZ};
    use std::time::Duration;

    /// One tick per update, with a floor, a hero at the origin and a checkpoint at x = 10.
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin, HeroPlugin))
            .init_resource::<PlayerInputs>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / TICK_HZ,
            )));
        app.world_mut().spawn((
            Collider::Cuboid {
                half_extents: Vec3::new(50.0, 0.5, 50.0),
            },
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        app.world_mut().spawn((
            Checkpoint {
                half_extents: Vec3::new(1.0, 2.0, 1.0),
                spawn_yaw: 1.0,
            },
            Transform::from_xyz(10.0, 0.0, 0.0),
        ));
        let profile = HeroProfile::uya();
        let hero = app
            .world_mut()
            .spawn(hero_bundle(0, Vec3::ZERO, 0.0, -20.0, &profile))
            .id();
        app.update(); // Starts the clock.
        (app, hero)
    }

    fn ticks(app: &mut App, count: usize) {
        for _ in 0..count {
            app.update();
        }
    }

    fn body(app: &App, hero: Entity) -> &HeroMotion {
        &app.world().get::<HeroBody>(hero).unwrap().0
    }

    #[test]
    fn a_fatal_hurt_kills_and_the_hero_comes_back_healed() {
        let (mut app, hero) = app();
        ticks(&mut app, 2);
        app.world_mut().write_message(HurtHero {
            player: 0,
            damage: 4,
            knockback: Vec3::new(0.0, 0.0, 5.0),
        });
        ticks(&mut app, 1);
        assert_eq!(body(&app, hero).state, HeroState::Hit);
        assert_eq!(app.world().get::<Health>(hero).unwrap().current, 6);

        // Still safe from the last hit: this one is ignored.
        app.world_mut().write_message(HurtHero {
            player: 0,
            damage: 99,
            knockback: Vec3::ZERO,
        });
        ticks(&mut app, 1);
        assert_eq!(app.world().get::<Health>(hero).unwrap().current, 6);

        ticks(&mut app, 120);
        app.world_mut().write_message(HurtHero {
            player: 0,
            damage: 99,
            knockback: Vec3::new(0.0, 0.0, 5.0),
        });
        ticks(&mut app, 60);
        assert_eq!(body(&app, hero).state, HeroState::Dead);
        let profile = HeroProfile::uya();
        ticks(&mut app, profile.death.ticks as usize);
        assert_eq!(body(&app, hero).state, HeroState::Idle);
        assert_eq!(body(&app, hero).position, Vec3::ZERO);
        assert_eq!(app.world().get::<Health>(hero).unwrap().current, 10);
    }

    #[test]
    fn reaching_a_checkpoint_moves_the_spawn_point() {
        let (mut app, hero) = app();
        app.world_mut()
            .get_mut::<HeroBody>(hero)
            .unwrap()
            .0
            .position = Vec3::new(10.5, 0.0, 0.5);
        ticks(&mut app, 2);
        let reached = app.world().get::<Hero>(hero).unwrap();
        assert_eq!(reached.spawn, Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(reached.spawn_yaw, 1.0);
    }
}
