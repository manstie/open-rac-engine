//! A tail each shot drags behind it, as the RY3NO's missiles' from V2 (0x3DB4D0 keeps the
//! points, 0x3DC8D0 draws them): each of its points goes a share of the way to the one
//! before it a tick, the first being where the shot was, and a strip facing the camera
//! runs through them, narrowing; or, following wholly, through where the shot was each tick,
//! its colour going along it, as the N90 Hurricane's bullets' (0x384CF0, drawn by
//! 0x384F58).

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use serde::Deserialize;

use super::{Blend, FxImages, FxRecipes, Tex, eye, fx_material, opaque};
use crate::DemoEntity;

/// A shot's tail: `points` points following at `follow` of the way a tick; `width` across at
/// the shot, `narrow` times as wide at each point after; the texture `step` further along
/// at each; shown once the shot is `after` ticks old. With `end_colour`, its colour goes
/// from `colour` at the shot to that at its last point, and `head` is its nearest edge's.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Tail {
    pub texture: Tex,
    pub points: usize,
    pub follow: f32,
    pub width: f32,
    pub narrow: f32,
    pub step: f32,
    #[serde(default)]
    pub after: u32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
    #[serde(default)]
    pub end_colour: Option<[f32; 4]>,
    #[serde(default)]
    pub head: Option<[f32; 4]>,
}

/// The tails drawn now, by player, shot and the tail's place in its recipe.
#[derive(Resource, Debug, Default)]
pub(crate) struct Tails(BTreeMap<(usize, u32, usize), Dragged>);

#[derive(Debug)]
pub(crate) struct Dragged {
    entity: Entity,
    mesh: Handle<Mesh>,
    /// Its points, the nearest the shot first.
    points: Vec<Vec3>,
    /// The shot's age they were last stepped at.
    stepped: u32,
}

/// One tick of a tail's points after a shot now at `at`: the far ones first, so each goes
/// towards where the one before it was, and the first to where the shot was.
fn step_points(points: &mut [Vec3], at: Vec3, follow: f32) {
    for index in (1..points.len()).rev() {
        points[index] = points[index].lerp(points[index - 1], follow);
    }
    if let Some(first) = points.first_mut() {
        *first = at;
    }
}

/// The strip through `points`, from `at`, in units from `at`.
fn strip(at: Vec3, points: &[Vec3], tail: &Tail, eye: Vec3) -> Mesh {
    let mut way = Vec::with_capacity(points.len() + 1);
    way.push(at);
    way.extend_from_slice(points);
    let mut positions = Vec::with_capacity(way.len() * 2);
    let mut uvs = Vec::with_capacity(way.len() * 2);
    let mut colours = Vec::with_capacity(way.len() * 2);
    let last = way.len().saturating_sub(1).max(1) as f32;
    let mut indices = Vec::with_capacity(way.len() * 6);
    let mut half = tail.width / 2.0;
    for (index, &point) in way.iter().enumerate() {
        let before = way[index.saturating_sub(1)];
        let after = way[(index + 1).min(way.len() - 1)];
        let side = (after - before)
            .cross(eye - point)
            .try_normalize()
            .unwrap_or(Vec3::Y)
            * half;
        positions.push((point - at + side).to_array());
        positions.push((point - at - side).to_array());
        let u = index as f32 * tail.step;
        uvs.extend([[u, 0.0], [u, 1.0]]);
        if let Some(end) = tail.end_colour {
            let colour = match tail.head {
                Some(head) if index == 0 => head,
                _ => lerp(tail.colour, end, index as f32 / last),
            };
            colours.extend([linear(colour); 2]);
        }
        half *= tail.narrow;
        if index > 0 {
            let first = (index as u32 - 1) * 2;
            indices.extend([first, first + 1, first + 2, first + 2, first + 1, first + 3]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    if colours.is_empty() {
        mesh
    } else {
        mesh.with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    }
}

fn lerp(from: [f32; 4], to: [f32; 4], share: f32) -> [f32; 4] {
    std::array::from_fn(|channel| from[channel] + (to[channel] - from[channel]) * share)
}

/// A colour as the game's bytes over 0x80 (the effects' sRGB), in linear light.
fn linear([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    let colour = Color::srgba(r, g, b, a).to_linear();
    [colour.red, colour.green, colour.blue, colour.alpha]
}

/// Steps and lays each shot's tails, making those that start and dropping those whose
/// shots are gone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tails(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut tails: ResMut<Tails>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut places: Query<(&mut Transform, &mut Visibility)>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let mut drawn = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let kind = Some(shot.kind.name());
            for (index, tail) in recipes.tail(key, shot.level, kind).iter().enumerate() {
                let id = (hero.player, shot.serial, index);
                let dragged = tails.0.entry(id).or_insert_with(|| {
                    let mesh = meshes.add(strip(shot.position, &[], tail, eye));
                    // Coloured along it, its points carry the colour.
                    let colour = if tail.end_colour.is_some() {
                        opaque()
                    } else {
                        tail.colour
                    };
                    let material =
                        fx_material(&mut materials, &images, tail.texture, colour, tail.blend);
                    // Its bounds change each frame.
                    let entity = commands
                        .spawn((
                            Name::new("tail"),
                            DemoEntity,
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            NotShadowCaster,
                            NoFrustumCulling,
                            Transform::from_translation(shot.position),
                            Visibility::Hidden,
                        ))
                        .id();
                    Dragged {
                        entity,
                        mesh,
                        points: vec![shot.position; tail.points],
                        stepped: shot.ticks,
                    }
                });
                while dragged.stepped < shot.ticks {
                    dragged.stepped += 1;
                    step_points(&mut dragged.points, shot.position, tail.follow);
                }
                if let Some(mut mesh) = meshes.get_mut(&dragged.mesh) {
                    *mesh = strip(shot.position, &dragged.points, tail, eye);
                }
                if let Ok((mut transform, mut visibility)) = places.get_mut(dragged.entity) {
                    transform.translation = shot.position;
                    *visibility = if shot.ticks >= tail.after {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                drawn.push(id);
            }
        }
    }
    tails.0.retain(|id, dragged| {
        let kept = drawn.contains(id);
        if !kept {
            commands.entity(dragged.entity).despawn();
        }
        kept
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tail_coloured_along_it_goes_from_its_head_to_its_end_colour() {
        let tail = Tail {
            texture: Tex::Effect(14),
            points: 2,
            follow: 1.0,
            width: 0.4,
            narrow: 1.0,
            step: 1.0,
            after: 0,
            colour: [0.25, 0.25, 0.25, 0.5],
            blend: Blend::Add,
            end_colour: Some([1.0, 0.25, 0.25, 0.0]),
            head: Some([0.875, 0.875, 0.875, 0.5]),
        };
        let points = [Vec3::Z, Vec3::Z * 2.0];
        let mesh = strip(Vec3::ZERO, &points, &tail, Vec3::X * 10.0);
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colours)) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("no colours");
        };
        assert_eq!(colours.len(), 6);
        assert_eq!(colours[0], linear([0.875, 0.875, 0.875, 0.5]));
        assert_eq!(colours[2], linear([0.625, 0.25, 0.25, 0.25]));
        assert_eq!(colours[5], linear([1.0, 0.25, 0.25, 0.0]));
        // A tail of one colour leaves it to its material.
        let plain = Tail {
            end_colour: None,
            ..tail
        };
        let mesh = strip(Vec3::ZERO, &points, &plain, Vec3::X * 10.0);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none());
    }

    #[test]
    fn a_tails_points_each_go_a_share_of_the_way_to_the_one_before() {
        let mut points = vec![Vec3::ZERO; 3];
        step_points(&mut points, Vec3::X, 0.2);
        assert_eq!(points, [Vec3::X, Vec3::ZERO, Vec3::ZERO]);
        step_points(&mut points, Vec3::X * 2.0, 0.2);
        assert_eq!(points[0], Vec3::X * 2.0);
        assert!((points[1].x - 0.2).abs() < 1e-6);
        assert_eq!(points[2], Vec3::ZERO);
    }
}
