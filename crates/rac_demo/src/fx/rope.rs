//! The Plasma Whip's rope as the game draws it (0x359110, `effects.md`, "Plasma Whip"):
//! strands through the points of the rig's rope each frame, and particles laid along its
//! pieces each tick (0x357F10).

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::weapons::Hand;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use rac_rig::{HeroRig, RopeShape};
use serde::Deserialize;

use super::{Blend, Emit, FxImages, FxRecipes, FxRng, Making, Tex, eye, fx_material, opaque};
use crate::DemoEntity;

/// A strand drawn each frame through the whip's rope while its links show: through its
/// first `points` points (all if not given), `width` across, picked each frame between
/// the two (`attacking`'s while he swings or the whip's end is thrown), or `times` the
/// width of the strand before it in its list (a glow round a core). Each point is moved
/// up to `jitter` along each axis each frame (its second while he attacks), unless the
/// strand keeps the points of the one before it (`same_points`). Its texture lies along
/// each piece, moving `scroll` of its length a tick; with `fade` it fades to nothing over
/// its last piece.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Strand {
    pub texture: Tex,
    #[serde(default)]
    pub width: (f32, f32),
    #[serde(default)]
    pub attacking: Option<(f32, f32)>,
    #[serde(default)]
    pub times: Option<(f32, f32)>,
    #[serde(default)]
    pub points: Option<usize>,
    #[serde(default)]
    pub same_points: bool,
    #[serde(default)]
    pub jitter: (f32, f32),
    #[serde(default)]
    pub scroll: f32,
    #[serde(default)]
    pub fade: bool,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// Particles laid each tick at random points of each of the rope's pieces past the
/// handle's, as the game lays them on pieces 1–8: `per_piece` a piece (a fraction being a
/// chance), only while he attacks or only while he doesn't if `attacking` says so.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AlongRope {
    pub per_piece: f32,
    #[serde(default)]
    pub attacking: Option<bool>,
    pub emit: Emit,
}

/// The effects key of the gun whose rope a hero's rig draws, and its level.
fn whip<'a>(armory: &'a Armory, arsenal: &Arsenal) -> Option<(&'a str, u8)> {
    let weapon = match arsenal.hand {
        Hand::Gun { weapon, .. } | Hand::Drawing { weapon, .. } => weapon,
        Hand::Wrench => return None,
    };
    let key = armory.get(weapon)?.effects.as_str();
    Some((key, arsenal.gun(weapon).map_or(0, |gun| gun.level)))
}

/// The strands drawn now: by player, the gun's level and the strand's place in its
/// recipe.
#[derive(Resource, Debug, Default)]
pub(crate) struct Strands(Slots<(usize, u8, usize)>);

/// Meshes rebuilt each frame, by an id, each with its entity.
pub(crate) type Slots<K> = BTreeMap<K, (Entity, Handle<Mesh>)>;

/// Lays each hero's strands through his rope as the rig drew it this frame, making those
/// that start and dropping those whose rope is gone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_strands(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    time: Res<Time>,
    rigs: Query<&HeroRig>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut strands: ResMut<Strands>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let ticks = time.elapsed_secs_wrapped() * 60.0;
    let mut drawn = Vec::new();
    for rig in &rigs {
        let (Some(rope), Ok((hero, arsenal))) = (rig.rope(), heroes.get(rig.hero)) else {
            continue;
        };
        let Some((key, level)) = whip(&armory, arsenal) else {
            continue;
        };
        let recipe = recipes.rope(key, level);
        let laid = lay_strands(recipe, &rope.points, rope.attacking, eye, ticks, &mut rng);
        for (index, mesh, place) in laid {
            let id = (hero.player, level, index);
            let strand = &recipe[index];
            put_mesh(
                &mut strands.0,
                id,
                mesh,
                place,
                &mut meshes,
                &mut places,
                |shown| {
                    let material = fx_material(
                        &mut materials,
                        &images,
                        strand.texture,
                        strand.colour,
                        strand.blend,
                    );
                    commands
                        .spawn((Name::new("whip strand"), shown, MeshMaterial3d(material)))
                        .id()
                },
            );
            drawn.push(id);
        }
    }
    drop_undrawn(&mut strands.0, &drawn, &mut commands);
}

/// Each of `strands` this frame through `base`'s points, by its place in the list: its
/// mesh and where it is placed. `ticks` is the time in 60 Hz ticks, which scrolls them.
pub(crate) fn lay_strands(
    strands: &[Strand],
    base: &[Vec3],
    attacking: bool,
    eye: Vec3,
    ticks: f32,
    rng: &mut FxRng,
) -> Vec<(usize, Mesh, Vec3)> {
    let mut laid = Vec::new();
    let mut points: Vec<Vec3> = Vec::new();
    let mut width = 0.0;
    for (index, strand) in strands.iter().enumerate() {
        if !strand.same_points {
            let jitter = if attacking {
                strand.jitter.1
            } else {
                strand.jitter.0
            };
            points = base
                .iter()
                .map(|&point| {
                    point + Vec3::from_array([0; 3].map(|_| rng.between((-jitter, jitter))))
                })
                .collect();
        }
        let count = strand.points.unwrap_or(points.len()).min(points.len());
        if count < 2 {
            continue;
        }
        width = match strand.times {
            Some(times) => width * rng.between(times),
            None => rng.between(match strand.attacking.filter(|_| attacking) {
                Some(attacking) => attacking,
                None => strand.width,
            }),
        };
        let shift = (ticks * strand.scroll).fract();
        let mesh = strand_mesh(&points[..count], width, eye, shift, strand.fade);
        laid.push((index, mesh, points[0]));
    }
    laid
}

/// What a mesh rebuilt each frame is shown with: its bounds change each frame.
pub(crate) type Shown = (
    DemoEntity,
    Mesh3d,
    NotShadowCaster,
    NoFrustumCulling,
    Transform,
);

/// Puts `mesh` in slot `id` at `place`; with no entity yet, `spawn` makes one from what
/// it is shown with (its material and name are the caller's).
pub(crate) fn put_mesh<K: Ord>(
    slots: &mut Slots<K>,
    id: K,
    mesh: Mesh,
    place: Vec3,
    meshes: &mut Assets<Mesh>,
    places: &mut Query<&mut Transform>,
    spawn: impl FnOnce(Shown) -> Entity,
) {
    // Placed at its first point, so it is sorted among the other blended sprites by
    // where it starts.
    if let Some(&(entity, ref handle)) = slots.get(&id)
        && let Some(mut old) = meshes.get_mut(handle)
    {
        *old = mesh;
        if let Ok(mut transform) = places.get_mut(entity) {
            transform.translation = place;
        }
        return;
    }
    let handle = meshes.add(mesh);
    let entity = spawn((
        DemoEntity,
        Mesh3d(handle.clone()),
        NotShadowCaster,
        NoFrustumCulling,
        Transform::from_translation(place),
    ));
    slots.insert(id, (entity, handle));
}

/// Despawns the slots not drawn this frame.
pub(crate) fn drop_undrawn<K: Ord>(slots: &mut Slots<K>, drawn: &[K], commands: &mut Commands) {
    slots.retain(|id, (entity, _)| {
        let keep = drawn.contains(id);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
}

/// Lays each hero's particles along his rope, where the rig last drew it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_along_ropes(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    rigs: Query<&HeroRig>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut rng: ResMut<FxRng>,
    mut making: Making,
) {
    let mut spawner = making.spawner();
    let eye = eye(&cameras);
    for rig in &rigs {
        let (Some(rope), Ok((_, arsenal))) = (rig.rope(), heroes.get(rig.hero)) else {
            continue;
        };
        let Some((key, level)) = whip(&armory, arsenal) else {
            continue;
        };
        for along in recipes.along_rope(key, level) {
            if along
                .attacking
                .is_some_and(|attacking| attacking != rope.attacking)
            {
                continue;
            }
            for (from, to) in pieces(rope) {
                let count =
                    along.per_piece as u32 + u32::from(rng.unit() < along.per_piece.fract());
                for _ in 0..count {
                    let at = from.lerp(to, rng.unit());
                    for emit in recipes.expand(std::slice::from_ref(&along.emit)) {
                        spawner.emit(emit, at, None, eye, &mut rng);
                    }
                }
            }
        }
    }
}

/// The rope's pieces past the handle's, from the hand out.
fn pieces(rope: &RopeShape) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    rope.points[1..].windows(2).map(|pair| (pair[0], pair[1]))
}

/// A strand through `points`, from the first, `width` across and turned at each point
/// to face `eye`; its texture's u runs 0 to 1 along each piece, moved on by `shift`
/// (each piece split where it wraps), and with `fade` the last piece fades to nothing.
pub(crate) fn strand_mesh(points: &[Vec3], width: f32, eye: Vec3, shift: f32, fade: bool) -> Mesh {
    let half = width / 2.0;
    let sides: Vec<Vec3> = (0..points.len())
        .map(|index| {
            let before = points[index.saturating_sub(1)];
            let after = points[(index + 1).min(points.len() - 1)];
            (after - before)
                .cross(eye - points[index])
                .try_normalize()
                .unwrap_or(Vec3::Y)
                * half
        })
        .collect();
    let pieces = points.len().saturating_sub(1);
    let mut positions = Vec::with_capacity(pieces * 8);
    let mut uvs = Vec::with_capacity(pieces * 8);
    let mut colours = Vec::with_capacity(pieces * 8);
    let mut indices = Vec::with_capacity(pieces * 12);
    // Where along a piece its u wraps from 1 back to 0.
    let start = (1.0 - shift.rem_euclid(1.0)).rem_euclid(1.0);
    let wrap = 1.0 - start;
    for piece in 0..pieces {
        let at = |t: f32| {
            let middle = points[piece].lerp(points[piece + 1], t) - points[0];
            let side = sides[piece].lerp(sides[piece + 1], t);
            let shade = if fade && piece + 1 == pieces {
                1.0 - t
            } else {
                1.0
            };
            (middle, side, [shade, shade, shade, shade])
        };
        let mut quad = |(t0, u0): (f32, f32), (t1, u1): (f32, f32)| {
            let first = positions.len() as u32;
            let ((a, side_a, shade_a), (b, side_b, shade_b)) = (at(t0), at(t1));
            positions.extend([a + side_a, a - side_a, b + side_b, b - side_b]);
            uvs.extend([[u0, 0.0], [u0, 1.0], [u1, 0.0], [u1, 1.0]]);
            colours.extend([shade_a, shade_a, shade_b, shade_b]);
            indices.extend([first, first + 1, first + 2, first + 2, first + 1, first + 3]);
        };
        if wrap > 0.0 && wrap < 1.0 {
            quad((0.0, start), (wrap, 1.0));
            quad((wrap, 0.0), (1.0, start));
        } else {
            quad((0.0, 0.0), (1.0, 1.0));
        }
    }
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

    fn floats(mesh: &Mesh, attribute: bevy::mesh::MeshVertexAttribute) -> Vec<Vec<f32>> {
        match mesh.attribute(attribute) {
            Some(VertexAttributeValues::Float32x2(values)) => {
                values.iter().map(|value| value.to_vec()).collect()
            }
            Some(VertexAttributeValues::Float32x3(values)) => {
                values.iter().map(|value| value.to_vec()).collect()
            }
            Some(VertexAttributeValues::Float32x4(values)) => {
                values.iter().map(|value| value.to_vec()).collect()
            }
            _ => panic!("no {attribute:?}"),
        }
    }

    #[test]
    fn a_strand_faces_the_eye_and_fades_over_its_last_piece() {
        let points = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
        let mesh = strand_mesh(&points, 0.1, Vec3::new(1.0, 0.0, 10.0), 0.0, true);
        let positions = floats(&mesh, Mesh::ATTRIBUTE_POSITION);
        // Two pieces of four corners, 0.05 either side along y, square to the eye.
        assert_eq!(positions.len(), 8);
        assert!(
            positions
                .iter()
                .all(|corner| (corner[1].abs() - 0.05).abs() < 1e-5)
        );
        assert!(positions.iter().all(|corner| corner[2].abs() < 1e-5));
        let colours = floats(&mesh, Mesh::ATTRIBUTE_COLOR);
        assert!(colours[..6].iter().all(|colour| colour[3] == 1.0));
        assert!(colours[6..].iter().all(|colour| colour[3] == 0.0));
    }

    #[test]
    fn a_scrolled_strand_splits_each_piece_where_its_texture_wraps() {
        let points = [Vec3::ZERO, Vec3::X];
        let mesh = strand_mesh(&points, 0.1, Vec3::new(0.0, 0.0, 10.0), 0.25, false);
        let positions = floats(&mesh, Mesh::ATTRIBUTE_POSITION);
        let uvs = floats(&mesh, Mesh::ATTRIBUTE_UV_0);
        // Moved on a quarter: u runs 0.75 to 1 over the first quarter, then 0 to 0.75.
        assert_eq!(positions.len(), 8);
        assert!((positions[2][0] - 0.25).abs() < 1e-5);
        assert!((uvs[0][0] - 0.75).abs() < 1e-5 && (uvs[2][0] - 1.0).abs() < 1e-5);
        assert!(uvs[4][0].abs() < 1e-5 && (uvs[6][0] - 0.75).abs() < 1e-5);
    }
}
