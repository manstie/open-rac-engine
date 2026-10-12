//! The hero kit's player character (PLAN.md P1.5): a kinematic capsule that moves like
//! Ratchet in R&C3/UYA, from numbers read in the games' code
//! (`docs/research/hero-controller.md`).
//!
//! - [`controller::step`] is the whole controller as one pure function; the plugin runs it
//!   for every [`Hero`] each tick in `rac_sim::SimSet::Hero`.
//! - Solid geometry comes from [`Collider`] components, gathered into a [`CollisionScene`]
//!   each tick. The controller itself only sees the narrow [`CollisionWorld`] interface.
//! - [`HeroTuning`] holds the movement profile, UYA NTSC-U's unless the app sets another.
//! - [`FollowCamera`] is a basic third-person camera (P1.7).

pub mod camera;
pub mod collision;
pub mod controller;
pub mod motor;
pub mod profile;
pub mod stick;

use bevy::prelude::*;
use rac_input::PlayerInputs;
use rac_sim::{PreviousSimTransform, SimSet, SimTransform};

pub use camera::FollowCamera;
pub use collision::{BodyPlacement, Collider, CollisionWorld, ParryWorld};
pub use controller::{HeroMotion, HeroState, JumpKind};
pub use profile::HeroProfile;

/// A player-controlled hero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(SimTransform)]
pub struct Hero {
    /// The `PlayerInputs` slot that steers him.
    pub player: usize,
    /// Where he starts, and returns to after falling out of the level.
    pub spawn: Vec3,
    pub spawn_yaw: f32,
    /// Falling below this height sends him back to `spawn`.
    pub respawn_below: f32,
}

/// The controller's state for a [`Hero`].
#[derive(Component, Debug, Clone, PartialEq)]
pub struct HeroBody(pub HeroMotion);

/// The movement profile every hero uses.
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
pub fn hero_bundle(player: usize, spawn: Vec3, spawn_yaw: f32, respawn_below: f32) -> impl Bundle {
    (
        Hero {
            player,
            spawn,
            spawn_yaw,
            respawn_below,
        },
        HeroBody(HeroMotion::new(spawn, spawn_yaw)),
        SimTransform::from_translation(spawn).with_rotation(Quat::from_rotation_y(spawn_yaw)),
    )
}

pub struct HeroPlugin;

impl Plugin for HeroPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeroTuning>()
            .init_resource::<CollisionScene>()
            .add_systems(
                FixedUpdate,
                (gather_colliders, move_heroes).chain().in_set(SimSet::Hero),
            )
            .add_systems(FixedUpdate, camera::follow.in_set(SimSet::Camera));
    }
}

/// Rebuilds the [`CollisionScene`] from every [`Collider`]. Colliders with a
/// `SimTransform` may move, and the hero rides them; others are placed by their
/// `Transform`. They are ordered by entity so every peer builds the same list.
fn gather_colliders(
    colliders: Query<(
        Entity,
        &Collider,
        &Transform,
        Option<(&SimTransform, &PreviousSimTransform)>,
    )>,
    mut scene: ResMut<CollisionScene>,
) {
    let mut bodies: Vec<(Entity, BodyPlacement)> = colliders
        .iter()
        .map(|(entity, collider, transform, simulated)| {
            let placement = match simulated {
                Some((current, previous)) => BodyPlacement {
                    collider: *collider,
                    translation: current.translation,
                    rotation: current.rotation,
                    motion: current.translation - previous.0.translation,
                },
                None => BodyPlacement {
                    collider: *collider,
                    translation: transform.translation,
                    rotation: transform.rotation,
                    motion: Vec3::ZERO,
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
    mut heroes: Query<(
        &Hero,
        &mut HeroBody,
        &mut SimTransform,
        &mut PreviousSimTransform,
    )>,
) {
    let mut heroes: Vec<_> = heroes.iter_mut().collect();
    heroes.sort_by_key(|(hero, ..)| hero.player);
    for (hero, mut body, mut transform, mut previous) in heroes {
        let camera_yaw = cameras
            .iter()
            .find(|camera| camera.player == hero.player)
            .map_or(0.0, |camera| camera.yaw);
        controller::step(
            &mut body.0,
            &inputs.get(hero.player),
            camera_yaw,
            &tuning.0,
            &scene.0,
        );
        let teleport = body.0.position.y < hero.respawn_below;
        if teleport {
            body.0 = HeroMotion::new(hero.spawn, hero.spawn_yaw);
        }
        transform.translation = body.0.position;
        transform.rotation = Quat::from_rotation_y(body.0.yaw);
        if teleport {
            previous.0 = *transform;
        }
    }
}
