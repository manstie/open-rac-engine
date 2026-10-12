//! The hero kit's standalone demo (PLAN.md Phase 1): the greybox test level (P1.4) with
//! the hero (P1.5) and his camera, or a spectator camera to look round it with.
//!
//! The app decides when the demo exists: it runs [`spawn_demo`] when gameplay starts and
//! [`despawn_demo`] when it ends. [`DemoPlugin`] adds the demo's simulation systems, and
//! the hero's if the app hasn't.

mod level;
mod spectator;

use bevy::prelude::*;
use rac_hero::{Checkpoint, Collider, FollowCamera, HeroPlugin, HeroTuning, hero_bundle};
use rac_sim::{SimSet, SimTick, SimTransform};

pub use level::{GAP_WIDTHS, Mover, Piece, Shape, Surface, greybox_level};
pub use spectator::SpectatorCamera;

/// Marks everything [`spawn_demo`] creates.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct DemoEntity;

/// Moves the level's platforms and flies the spectator camera, both in the simulation.
pub struct DemoPlugin;

/// How the demo is set up when it spawns.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DemoSettings {
    /// Fly a free camera instead of playing the hero.
    pub spectator: bool,
}

/// Where the hero starts, on the floor facing down the level.
const HERO_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 0.0);
/// Falling this far below the floor puts the hero back at the start.
const RESPAWN_BELOW: f32 = -20.0;

impl Plugin for DemoPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<HeroPlugin>() {
            app.add_plugins(HeroPlugin);
        }
        app.init_resource::<DemoSettings>()
            .add_systems(FixedUpdate, move_platforms.in_set(SimSet::World))
            .add_systems(FixedUpdate, spectator::fly.in_set(SimSet::Camera));
    }
}

/// Spawns the greybox level, its lights and the spectator camera.
pub fn spawn_demo(
    settings: Res<DemoSettings>,
    tuning: Res<HeroTuning>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut material_for = |surface: Surface, index: usize| {
        // Neighbouring pieces alternate shades so edges show without textures.
        let shade = if index.is_multiple_of(2) { 1.0 } else { 0.85 };
        let (r, g, b) = match surface {
            Surface::Ground => (0.55, 0.57, 0.6),
            Surface::Ledge => (0.85, 0.55, 0.25),
            Surface::WallJump => (0.3, 0.45, 0.8),
            Surface::Moving => (0.9, 0.8, 0.2),
            Surface::GrindRail => (0.75, 0.75, 0.8),
            Surface::SwingTarget => (0.2, 0.85, 0.35),
            Surface::Hazard => (0.85, 0.15, 0.1),
            Surface::Checkpoint => (0.3, 0.9, 0.9),
        };
        materials.add(StandardMaterial {
            base_color: Color::srgb(r * shade, g * shade, b * shade),
            perceptual_roughness: 0.9,
            ..default()
        })
    };

    for (index, piece) in greybox_level().into_iter().enumerate() {
        let material = material_for(piece.surface, index);
        let (mesh, rotation) = match piece.shape {
            // A checkpoint shows as a pad on the ground at its spawn point.
            Shape::Box { half_extents } if piece.surface == Surface::Checkpoint => (
                meshes.add(Cuboid::new(
                    half_extents.x * 1.2,
                    0.04,
                    half_extents.z * 1.2,
                )),
                piece.rotation,
            ),
            Shape::Box { half_extents } => (
                meshes.add(Cuboid::from_size(half_extents * 2.0)),
                piece.rotation,
            ),
            Shape::Rail { start, end, radius } => (
                meshes.add(Cylinder::new(radius, start.distance(end))),
                // A cylinder's axis is +y; turn it to run from start to end.
                Quat::from_rotation_arc(Vec3::Y, (end - start).normalize()),
            ),
            Shape::Sphere { radius } => (meshes.add(Sphere::new(radius)), piece.rotation),
        };
        let mut entity = commands.spawn((
            Name::new(piece.name),
            DemoEntity,
            piece.surface,
            Mesh3d(mesh),
            MeshMaterial3d(material),
        ));
        if let Shape::Box { half_extents } = piece.shape {
            match piece.surface {
                Surface::Checkpoint => {
                    entity.insert(Checkpoint {
                        half_extents,
                        spawn_yaw: 0.0,
                    });
                }
                surface if surface.is_solid() => {
                    entity.insert((Collider::Cuboid { half_extents }, surface.material()));
                }
                _ => {}
            }
        }
        match piece.mover {
            Some(mover) => {
                entity.insert((
                    mover,
                    SimTransform::from_translation(mover.position_at(0)).with_rotation(rotation),
                ));
            }
            None => {
                entity
                    .insert(Transform::from_translation(piece.translation).with_rotation(rotation));
            }
        }
    }

    commands.spawn((
        Name::new("sun"),
        DemoEntity,
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(30.0, 60.0, 20.0).looking_at(Vec3::new(0.0, 0.0, -40.0), Vec3::Y),
    ));
    if settings.spectator {
        commands.spawn((
            Name::new("spectator camera"),
            DemoEntity,
            Camera3d::default(),
            SpectatorCamera::default(),
            SimTransform::from_xyz(0.0, 6.0, 12.0),
        ));
        return;
    }

    // A stand-in for the hero's model until the placeholder rig (P1.17): a body 1.5 u
    // tall, the height of the collision capsule, and a nose to show which way he faces.
    let body = meshes.add(Capsule3d::new(0.4, 0.7));
    let nose = meshes.add(Cuboid::new(0.24, 0.14, 0.3));
    let fur = materials.add(Color::srgb(0.85, 0.65, 0.3));
    let trim = materials.add(Color::srgb(0.3, 0.3, 0.35));
    commands.spawn((
        Name::new("hero"),
        DemoEntity,
        hero_bundle(0, HERO_SPAWN, 0.0, RESPAWN_BELOW, &tuning.0),
        Visibility::default(),
        children![
            (
                Mesh3d(body),
                MeshMaterial3d(fur),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ),
            (
                Mesh3d(nose),
                MeshMaterial3d(trim),
                Transform::from_xyz(0.0, 1.15, -0.4),
            ),
        ],
    ));
    let camera = FollowCamera::for_player(0, 0.0);
    let (translation, rotation) = camera.placement(HERO_SPAWN, &tuning.0.camera);
    commands.spawn((
        Name::new("hero camera"),
        DemoEntity,
        Camera3d::default(),
        camera,
        SimTransform::from_translation(translation).with_rotation(rotation),
    ));
}

/// Removes everything [`spawn_demo`] created.
pub fn despawn_demo(mut commands: Commands, entities: Query<Entity, With<DemoEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn move_platforms(tick: Res<SimTick>, mut platforms: Query<(&Mover, &mut SimTransform)>) {
    for (mover, mut transform) in &mut platforms {
        transform.translation = mover.position_at(tick.0);
    }
}
