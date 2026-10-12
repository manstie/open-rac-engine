//! The hero kit's standalone demo (PLAN.md Phase 1): for now, the greybox test level
//! (P1.4) and a spectator camera to look round it with.
//!
//! The app decides when the demo exists: it runs [`spawn_demo`] when gameplay starts and
//! [`despawn_demo`] when it ends. [`DemoPlugin`] adds the demo's simulation systems.

mod level;
mod spectator;

use bevy::prelude::*;
use rac_sim::{SimSet, SimTick, SimTransform};

pub use level::{GAP_WIDTHS, Mover, Piece, Shape, Surface, greybox_level};
pub use spectator::SpectatorCamera;

/// Marks everything [`spawn_demo`] creates.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct DemoEntity;

/// Moves the level's platforms and flies the spectator camera, both in the simulation.
pub struct DemoPlugin;

impl Plugin for DemoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, move_platforms.in_set(SimSet::World))
            .add_systems(FixedUpdate, spectator::fly.in_set(SimSet::Camera));
    }
}

/// Spawns the greybox level, its lights and the spectator camera.
pub fn spawn_demo(
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
    commands.spawn((
        Name::new("spectator camera"),
        DemoEntity,
        Camera3d::default(),
        SpectatorCamera::default(),
        SimTransform::from_xyz(0.0, 6.0, 12.0),
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
