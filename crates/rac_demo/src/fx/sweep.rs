//! Flat ribbons through where a shot has been, as the Disc Blade's from V2 (0x371128,
//! drawn by 0x4068C8): a point of its way kept every so often, each with its two sides
//! along one level line, fading from the shot back.

use std::collections::{BTreeMap, VecDeque};

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use serde::Deserialize;

use super::{Blend, FxImages, FxRecipes, Tex, fx_material, opaque};
use crate::DemoEntity;

/// A ribbon a shot leaves: a point of its way every `every` ticks, the last `points` with
/// the shot's own, `width` across along the level line `turn` degrees from where it first
/// headed (the game keeps the disc's first turn for every point). Its colour goes from
/// `colour` to `end_colour` back along it, the oldest point reaching the end as it is
/// dropped; the game gives the shot's own point the second's share again
/// ([`fades`]). Its texture's middle column lies across it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Sweep {
    pub texture: Tex,
    pub every: u32,
    pub points: usize,
    pub width: f32,
    #[serde(default)]
    pub turn: f32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    pub end_colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// The ribbons drawn now, by player, shot and the ribbon's place in its recipe.
#[derive(Resource, Debug, Default)]
pub(crate) struct Sweeps(BTreeMap<(usize, u32, usize), Swept>);

#[derive(Debug)]
pub(crate) struct Swept {
    entity: Entity,
    mesh: Handle<Mesh>,
    side: Vec3,
    /// The shot's places kept, the newest first.
    kept: VecDeque<Vec3>,
    /// The shot's age when it last kept one.
    kept_at: u32,
}

/// Lays each shot's ribbons through the points it has kept, making those that start and
/// dropping those whose shots are gone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_sweeps(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut sweeps: ResMut<Sweeps>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let mut drawn = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let kind = Some(shot.kind.name());
            for (index, sweep) in recipes.sweep(key, shot.level, kind).iter().enumerate() {
                let id = (hero.player, shot.serial, index);
                let swept = sweeps.0.entry(id).or_insert_with(|| {
                    let heading = Vec3::new(shot.velocity.x, 0.0, shot.velocity.z);
                    let side = Quat::from_rotation_y(sweep.turn.to_radians())
                        * heading.normalize_or(Vec3::NEG_Z);
                    let mesh = meshes.add(sheet(&[], side, sweep.width));
                    let material = fx_material(
                        &mut materials,
                        &images,
                        sweep.texture,
                        [1.0; 4],
                        sweep.blend,
                    );
                    // Its bounds change each frame.
                    let entity = commands
                        .spawn((
                            Name::new("sweep"),
                            DemoEntity,
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            NotShadowCaster,
                            NoFrustumCulling,
                            Transform::from_translation(shot.position),
                        ))
                        .id();
                    Swept {
                        entity,
                        mesh,
                        side,
                        kept: VecDeque::new(),
                        kept_at: shot.ticks,
                    }
                });
                let every = sweep.every.max(1);
                if shot.ticks >= swept.kept_at + every {
                    swept.kept.push_front(shot.position);
                    swept.kept.truncate(sweep.points.saturating_sub(1));
                    swept.kept_at = shot.ticks - shot.ticks % every;
                }
                let along = (shot.ticks - swept.kept_at) as f32 / every as f32;
                let way: Vec<Vec3> = std::iter::once(shot.position)
                    .chain(swept.kept.iter().copied())
                    .collect();
                let points: Vec<(Vec3, [f32; 4])> = way
                    .iter()
                    .zip(fades(way.len(), along))
                    .map(|(&place, t)| {
                        let colour = lerp(sweep.colour, sweep.end_colour, t);
                        (place - shot.position, colour)
                    })
                    .collect();
                if let Some(mut mesh) = meshes.get_mut(&swept.mesh) {
                    *mesh = sheet(&points, swept.side, sweep.width);
                }
                if let Ok(mut transform) = places.get_mut(swept.entity) {
                    transform.translation = shot.position;
                }
                drawn.push(id);
            }
        }
    }
    sweeps.0.retain(|id, swept| {
        let keep = drawn.contains(id);
        if !keep {
            commands.entity(swept.entity).despawn();
        }
        keep
    });
}

/// How far from its start colour to its end each of `count` points is, the shot's own
/// first, `along` (0 to 1) of the way to the next point kept: the kept ones (k − 1 +
/// `along`) / (count − 1) for the kth after the shot, the oldest reaching the end as it
/// goes; the shot's own one step, as 0x4068C8 gives it.
fn fades(count: usize, along: f32) -> Vec<f32> {
    if count < 2 {
        return vec![0.0; count];
    }
    let step = 1.0 / (count - 1) as f32;
    (0..count)
        .map(|point| match point {
            0 => step,
            _ => ((point - 1) as f32 + along) * step,
        })
        .map(|t| t.min(1.0))
        .collect()
}

/// A colour of the recipes' (the game's bytes over 0x80) `t` of the way from `from` to
/// `to`, as the mesh's linear vertex colour.
fn lerp(from: [f32; 4], to: [f32; 4], t: f32) -> [f32; 4] {
    let at = |channel: usize| from[channel] + (to[channel] - from[channel]) * t;
    Color::srgba(at(0), at(1), at(2), at(3))
        .to_linear()
        .to_f32_array()
}

/// A flat strip through `points` (places and colours), `width` across along `side` at
/// each; the texture's middle column across it, as the game lays it (0x1D63E0 [data]).
fn sheet(points: &[(Vec3, [f32; 4])], side: Vec3, width: f32) -> Mesh {
    let half = side * width / 2.0;
    let mut positions = Vec::with_capacity(points.len() * 2);
    let mut colours = Vec::with_capacity(points.len() * 2);
    let mut uvs = Vec::with_capacity(points.len() * 2);
    for &(place, colour) in points {
        positions.extend([place + half, place - half]);
        colours.extend([colour, colour]);
        uvs.extend([[0.5, 0.0], [0.5, 1.0]]);
    }
    let pieces = points.len().saturating_sub(1) as u32;
    let indices = (0..pieces)
        .flat_map(|piece| {
            let first = piece * 2;
            [first, first + 1, first + 2, first + 2, first + 1, first + 3]
        })
        .collect();
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn a_sweeps_points_fade_back_to_the_oldest_as_it_goes() {
        assert_eq!(fades(1, 0.5), [0.0]);
        // Six points, a third of the way to the next kept: the shot's own one step, then
        // the kept ones from a third of a step.
        let six = fades(6, 1.0 / 3.0);
        let want = [
            0.2,
            1.0 / 15.0,
            4.0 / 15.0,
            7.0 / 15.0,
            10.0 / 15.0,
            13.0 / 15.0,
        ];
        for (got, want) in six.iter().zip(want) {
            assert!((got - want).abs() < 1e-5, "{six:?}");
        }
        // The oldest reaches the end just as it would be dropped.
        assert!((fades(6, 1.0)[5] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_sweep_lies_flat_along_its_side() {
        let points = [
            (Vec3::ZERO, [1.0; 4]),
            (Vec3::Z * 2.0, [0.5; 4]),
            (Vec3::Z * 4.0, [0.0; 4]),
        ];
        let mesh = sheet(&points, Vec3::X, 1.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        assert_eq!(positions.len(), 6);
        assert_eq!(positions[0], [0.5, 0.0, 0.0]);
        assert_eq!(positions[5], [-0.5, 0.0, 4.0]);
        assert!(positions.iter().all(|corner| corner[1] == 0.0));
        assert_eq!(mesh.indices().map(Indices::len), Some(12));
    }
}
