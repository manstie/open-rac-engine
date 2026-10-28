//! Ribbons through a gun's shots in flight, the newest first, rebuilt each frame to face
//! the camera: the Lava Gun's stream (0x305CA0).

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::weapons::Shot;
use rac_combat::{Armory, Arsenal, WeaponId};
use rac_hero::Hero;
use serde::Deserialize;

use super::{Blend, FxImages, FxRecipes, FxRng, Tex, eye, fx_material, opaque, whole};
use crate::DemoEntity;

/// A ribbon drawn each frame through the points of a gun's shots of one kind, the newest
/// first: `width` across at the newest (between the two, picked each frame, the same
/// share for each of the gun's ribbons) and `narrow` times as wide at each point after.
/// Its texture lies along each piece between two points, as the game lays it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Ribbon {
    pub texture: Tex,
    pub width: (f32, f32),
    #[serde(default = "whole")]
    pub narrow: f32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// The ribbons drawn now: by player, gun, kind of shot and the ribbon's place in its
/// recipe.
#[derive(Resource, Debug, Default)]
pub(crate) struct Ribbons(BTreeMap<(usize, usize, &'static str, usize), (Entity, Handle<Mesh>)>);

/// Lays each gun's ribbons through its shots, making the ribbons that start and dropping
/// those whose shots are gone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_ribbons(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut ribbons: ResMut<Ribbons>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let mut drawn = Vec::new();
    for (hero, arsenal) in &heroes {
        let mut runs: BTreeMap<(WeaponId, &'static str), Vec<&Shot>> = BTreeMap::new();
        for shot in &arsenal.shots {
            runs.entry((shot.weapon, shot.kind.name()))
                .or_default()
                .push(shot);
        }
        for ((weapon, name), mut shots) in runs {
            if shots.len() < 2 {
                continue;
            }
            shots.sort_by_key(|shot| std::cmp::Reverse(shot.serial));
            let key = armory.get(weapon).map_or("", |def| def.effects.as_str());
            let level = shots[0].level;
            let mut points: Vec<Vec3> = shots.iter().map(|shot| shot.position).collect();
            // While they still come, from the nozzle, where the newest left a tick ago, as
            // the game starts its stream there.
            if shots[0].ticks <= 1 {
                let left = shots[0].position - shots[0].velocity * shots[0].ticks as f32 / 60.0;
                points.insert(0, left);
            }
            let share = rng.unit();
            for (index, ribbon) in recipes.ribbon(key, level, Some(name)).iter().enumerate() {
                let id = (hero.player, weapon.0, name, index);
                let width = ribbon.width.0 + (ribbon.width.1 - ribbon.width.0) * share;
                let mesh = strip(&points, width, ribbon.narrow, eye);
                // Placed at its newest point, so it is sorted among the other blended
                // sprites by where it starts.
                let place = Transform::from_translation(points[0]);
                if let Some(&(entity, ref handle)) = ribbons.0.get(&id)
                    && let Some(mut old) = meshes.get_mut(handle)
                {
                    *old = mesh;
                    if let Ok(mut transform) = places.get_mut(entity) {
                        *transform = place;
                    }
                } else {
                    let handle = meshes.add(mesh);
                    let material = fx_material(
                        &mut materials,
                        &images,
                        ribbon.texture,
                        ribbon.colour,
                        ribbon.blend,
                    );
                    // Its bounds change each frame.
                    let entity = commands
                        .spawn((
                            Name::new("ribbon"),
                            DemoEntity,
                            Mesh3d(handle.clone()),
                            MeshMaterial3d(material),
                            NotShadowCaster,
                            NoFrustumCulling,
                            place,
                        ))
                        .id();
                    ribbons.0.insert(id, (entity, handle));
                }
                drawn.push(id);
            }
        }
    }
    ribbons.0.retain(|id, (entity, _)| {
        let keep = drawn.contains(id);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
}

/// A strip through `points`, from the first, `width` across there and `narrow` times as
/// wide at each after, turned at each point to face `eye`; a quad between each two
/// points, its texture laid along it.
pub(crate) fn strip(points: &[Vec3], width: f32, narrow: f32, eye: Vec3) -> Mesh {
    let mut sides = Vec::with_capacity(points.len());
    let mut half = width / 2.0;
    for (index, &point) in points.iter().enumerate() {
        let before = points[index.saturating_sub(1)];
        let after = points[(index + 1).min(points.len() - 1)];
        let side = (after - before)
            .cross(eye - point)
            .try_normalize()
            .unwrap_or(Vec3::Y);
        sides.push(side * half);
        half *= narrow;
    }
    let pieces = points.len().saturating_sub(1);
    let mut positions = Vec::with_capacity(pieces * 4);
    let mut uvs = Vec::with_capacity(pieces * 4);
    let mut indices = Vec::with_capacity(pieces * 6);
    for piece in 0..pieces {
        let (a, b) = (points[piece], points[piece + 1]);
        let (side_a, side_b) = (sides[piece], sides[piece + 1]);
        let first = positions.len() as u32;
        let (a, b) = (a - points[0], b - points[0]);
        positions.extend([a + side_a, a - side_a, b + side_b, b - side_b]);
        uvs.extend([[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]]);
        indices.extend([first, first + 1, first + 2, first + 2, first + 1, first + 3]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn a_ribbon_narrows_along_its_points_and_faces_the_eye() {
        let points = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
        let mesh = strip(&points, 0.1, 0.5, Vec3::new(1.0, 0.0, 10.0));
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        // Two pieces of four corners, across the way and square to the eye (along y).
        assert_eq!(positions.len(), 8);
        let across = |corner: usize| Vec3::from(positions[corner]).y.abs();
        assert!((across(0) - 0.05).abs() < 1e-5);
        assert!((across(2) - 0.025).abs() < 1e-5);
        assert!((across(6) - 0.0125).abs() < 1e-5);
        assert!(positions.iter().all(|corner| corner[2].abs() < 1e-5));
        assert_eq!(mesh.indices().map(Indices::len), Some(12));
    }
}
