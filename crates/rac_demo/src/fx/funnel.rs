//! The Qwack-O-Ray's beam (`effects.md`, "Qwack-O-Ray"; 0x37E830, drawn by 0x37F738): rings
//! round a rope that lags behind his aim, with quads of a scrolling texture between them on
//! the far side from the camera, so what it hits stays in sight, and strips spiralling round
//! it. It goes with the gun's rays.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::{Armory, Arsenal, GunEvent, GunEventKind};
use rac_hero::Hero;
use serde::Deserialize;

use super::{
    Blend, Emit, FxImages, FxRecipes, FxRng, Making, Tex, eye, fx_material, level, opaque,
};
use crate::DemoEntity;

/// A beam drawn as a funnel, its way `pieces` steps of a twelfth of the way to the target's:
/// its first point at the gun, and each after it a step from the one before, along a way
/// between where that point was a tick ago and the aim, the aim's share `follow.0` less
/// `follow.1` of how far along it is.
///
/// At each point a ring of `round` points, radius going from `radius.0` to `radius.1`
/// along the way. From ring `from_ring` on, quads between them show `texture` twice, its
/// `along` coordinate going `along_step` a ring and scrolling `along_scroll` a tick, and
/// the other once round each ring, the two layers turning `round_scroll` a tick apart; the
/// last ring fades to nothing.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Funnel {
    pub pieces: usize,
    pub follow: (f32, f32),
    pub round: usize,
    pub radius: (f32, f32),
    pub from_ring: usize,
    pub texture: Tex,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    pub along_step: f32,
    pub along_scroll: f32,
    pub round_scroll: f32,
    #[serde(default)]
    pub spirals: Option<Spirals>,
    /// Shown each tick this far along the way from the gun: its glow and light.
    #[serde(default)]
    pub at_gun: Vec<Emit>,
    #[serde(default)]
    pub at_gun_along: f32,
    /// Shown one tick in two from the gun, up the beam: the sparks flowing along it.
    #[serde(default)]
    pub flowing: Vec<Emit>,
    /// Shown every `pulse_every` ticks from the gun, up the beam: its pulses.
    #[serde(default)]
    pub pulse: Vec<Emit>,
    #[serde(default)]
    pub pulse_every: u32,
}

/// Strips round a funnel's way, one for each of `strips`, the second going round the other
/// way: at a radius from `radius.0` to `radius.1` along the way, at an angle growing `twist`
/// radians a point and `turn` a tick, their textures scrolling `scroll` a tick.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Spirals {
    pub radius: (f32, f32),
    pub twist: f32,
    pub turn: f32,
    pub scroll: f32,
    pub strips: Vec<SpiralStrip>,
}

/// The two layers of one spiral: a core of even `core_width` in `core`, and a glow in
/// `glow` growing to `glow_width` across along the way. Both fade out at the last point.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpiralStrip {
    pub core: Tex,
    pub core_width: f32,
    #[serde(default = "opaque")]
    pub core_colour: [f32; 4],
    pub glow: Tex,
    pub glow_width: f32,
    #[serde(default = "opaque")]
    pub glow_colour: [f32; 4],
}

impl Funnel {
    /// Every texture it draws with.
    pub(super) fn textures(&self) -> impl Iterator<Item = Tex> + '_ {
        let strips = self.spirals.iter().flat_map(|spirals| &spirals.strips);
        std::iter::once(self.texture).chain(strips.flat_map(|strip| [strip.core, strip.glow]))
    }

    /// The emits it shows, for their textures.
    pub(super) fn emits(&self) -> impl Iterator<Item = &Emit> + '_ {
        self.at_gun.iter().chain(&self.flowing).chain(&self.pulse)
    }
}

/// A funnel drawn now.
#[derive(Debug)]
struct Shown {
    rope: Vec<Vec3>,
    age: u32,
    parts: Vec<(Entity, Handle<Mesh>)>,
}

/// The funnels drawn now, by player and gun.
#[derive(Resource, Debug, Default)]
pub(crate) struct Funnels(BTreeMap<(usize, usize), Shown>);

/// A straight rope of `pieces` points from `from` towards `to`.
fn straight(pieces: usize, from: Vec3, to: Vec3) -> Vec<Vec3> {
    (0..pieces)
        .map(|point| from.lerp(to, point as f32 / pieces as f32))
        .collect()
}

/// Moves the rope on a tick (0x37E830): from the gun, each point a step from the one before.
fn lay_rope(rope: &mut [Vec3], from: Vec3, to: Vec3, follow: (f32, f32)) {
    let pieces = rope.len();
    let step = (to - from) / pieces as f32;
    let length = step.length();
    rope[0] = from;
    for point in 1..pieces {
        let share = follow.0 - follow.1 * point as f32 / pieces as f32;
        let way = (rope[point] - rope[point - 1]).lerp(step, share);
        rope[point] = rope[point - 1] + way.normalize_or(step.normalize_or_zero()) * length;
    }
}

/// The way a point of the rope goes on, and square to it on the camera's side, its two axes
/// round the way.
fn frame(rope: &[Vec3], point: usize, eye: Vec3) -> (Vec3, Vec3, Vec3) {
    let way = (rope[point] - rope[point.saturating_sub(1)]).normalize_or(Vec3::NEG_Z);
    let side = way
        .cross(eye - rope[point])
        .try_normalize()
        .unwrap_or_else(|| way.any_orthonormal_vector());
    (way, side, way.cross(side))
}

/// What goes into a mesh of vertices each with a colour.
#[derive(Default)]
struct Quads {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Quads {
    /// A quad of corners `a`, `b` at one end and `c`, `d` at the other, each its position,
    /// coordinates and alpha.
    fn push(&mut self, corners: [(Vec3, [f32; 2], f32); 4]) {
        let first = self.positions.len() as u32;
        for (position, uv, alpha) in corners {
            self.positions.push(position.to_array());
            self.uvs.push(uv);
            self.colours.push([1.0, 1.0, 1.0, alpha]);
        }
        self.indices
            .extend([first, first + 1, first + 2, first + 2, first + 1, first + 3]);
    }

    fn mesh(self) -> Mesh {
        let normals = vec![[0.0, 1.0, 0.0]; self.positions.len()];
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// The quads between the rings, on the side turned from `eye`, from `rope[0]`.
fn funnel_mesh(funnel: &Funnel, rope: &[Vec3], eye: Vec3, age: u32) -> Mesh {
    let pieces = rope.len();
    let rings: Vec<Vec<Vec3>> = (0..pieces)
        .map(|point| {
            let (way, side, _) = frame(rope, point, eye);
            let share = point as f32 / pieces as f32;
            let radius = funnel.radius.0 + (funnel.radius.1 - funnel.radius.0) * share;
            (0..funnel.round)
                .map(|at| {
                    let angle = TAU * at as f32 / funnel.round as f32;
                    rope[point] + Quat::from_axis_angle(way, angle) * side * radius
                })
                .collect()
        })
        .collect();
    let scrolled = age as f32;
    let around = 1.0 / funnel.round as f32;
    let mut quads = Quads::default();
    for ring in funnel.from_ring + 1..pieces {
        let far =
            -funnel.along_scroll * scrolled + (ring - funnel.from_ring) as f32 * funnel.along_step;
        let near = far - funnel.along_step;
        // The last ring fades to nothing.
        let alpha = if ring + 1 == pieces { 0.0 } else { 1.0 };
        for at in 0..funnel.round {
            let next = (at + 1) % funnel.round;
            let (a, b) = (rings[ring][at], rings[ring][next]);
            let (c, d) = (rings[ring - 1][at], rings[ring - 1][next]);
            if (c - b).cross(a - b).dot(b - eye) >= 0.0 {
                continue;
            }
            for sign in [-1.0, 1.0] {
                let start = sign * funnel.round_scroll * scrolled + at as f32 * around;
                let end = start + around;
                quads.push([
                    (a - rope[0], [far, start], alpha),
                    (b - rope[0], [far, end], alpha),
                    (c - rope[0], [near, start], 1.0),
                    (d - rope[0], [near, end], 1.0),
                ]);
            }
        }
    }
    quads.mesh()
}

/// The points of a spiral, from `rope[0]`: the gun's first, then round the way.
fn spiral_points(
    spirals: &Spirals,
    rope: &[Vec3],
    eye: Vec3,
    age: u32,
    other_way: bool,
) -> Vec<Vec3> {
    let pieces = rope.len();
    let phase = spirals.turn * age as f32;
    (0..pieces)
        .map(|point| {
            if point == 0 {
                return rope[0];
            }
            let (_, side, up) = frame(rope, point, eye);
            let share = point as f32 / pieces as f32;
            let radius = spirals.radius.0 + (spirals.radius.1 - spirals.radius.0) * share;
            let angle = phase + spirals.twist * point as f32;
            let across = ops::sin(angle) * if other_way { -1.0 } else { 1.0 };
            rope[point] + (side * ops::cos(angle) + up * across) * radius
        })
        .collect()
}

/// A strip through `points`, from `points[0]`, `half(share)` to either side at each as far
/// along as `share`, turned to face `eye`; each piece shows its texture once, scrolled.
fn strip_mesh(points: &[Vec3], half: impl Fn(f32) -> f32, eye: Vec3, scroll: f32) -> Mesh {
    let mut quads = Quads::default();
    let last = points.len().saturating_sub(1);
    let side = |index: usize| {
        let way = points[(index + 1).min(last)] - points[index.saturating_sub(1)];
        let share = index as f32 / points.len() as f32;
        way.cross(eye - points[index])
            .try_normalize()
            .unwrap_or(Vec3::Y)
            * half(share)
    };
    for piece in 0..last {
        let (a, b) = (points[piece] - points[0], points[piece + 1] - points[0]);
        let (side_a, side_b) = (side(piece), side(piece + 1));
        let alpha = if piece + 1 == last { 0.0 } else { 1.0 };
        quads.push([
            (a + side_a, [scroll, 0.0], 1.0),
            (a - side_a, [scroll, 1.0], 1.0),
            (b + side_b, [scroll + 1.0, 0.0], alpha),
            (b - side_b, [scroll + 1.0, 1.0], alpha),
        ]);
    }
    quads.mesh()
}

/// Each mesh of a funnel this tick, the funnel's first and each spiral's core and glow.
fn meshes_of(funnel: &Funnel, rope: &[Vec3], eye: Vec3, age: u32) -> Vec<Mesh> {
    let mut meshes = vec![funnel_mesh(funnel, rope, eye, age)];
    if let Some(spirals) = &funnel.spirals {
        let scroll = -spirals.scroll * age as f32;
        for (index, strip) in spirals.strips.iter().enumerate() {
            let points = spiral_points(spirals, rope, eye, age, index % 2 == 1);
            meshes.push(strip_mesh(&points, |_| strip.core_width / 2.0, eye, scroll));
            meshes.push(strip_mesh(
                &points,
                |share| strip.glow_width / 2.0 * share,
                eye,
                scroll,
            ));
        }
    }
    meshes
}

/// Lays each funnel its gun's rays ask for this tick, and drops those whose rays stopped.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_funnels(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    images: Res<FxImages>,
    mut events: MessageReader<GunEvent>,
    mut funnels: ResMut<Funnels>,
    mut rng: ResMut<FxRng>,
    mut places: Query<&mut Transform>,
    mut making: Making,
) {
    let eye = eye(&cameras);
    let mut drawn = Vec::new();
    for event in events.read() {
        let GunEventKind::Ray { from, to } = event.kind else {
            continue;
        };
        let key = armory
            .get(event.weapon)
            .map_or("", |def| def.effects.as_str());
        let level = level(&heroes, event.player, event.weapon);
        let Some(funnel) = recipes.funnel(key, level) else {
            continue;
        };
        let id = (event.player, event.weapon.0);
        drawn.push(id);
        let shown = funnels.0.entry(id).or_insert_with(|| Shown {
            rope: straight(funnel.pieces, from, to),
            age: 0,
            parts: Vec::new(),
        });
        shown.age += 1;
        lay_rope(&mut shown.rope, from, to, funnel.follow);
        let meshes = meshes_of(funnel, &shown.rope, eye, shown.age);
        if shown.parts.is_empty() {
            let looks = funnel_looks(funnel);
            for (mesh, (texture, colour)) in meshes.into_iter().zip(looks) {
                let handle = making.meshes.add(mesh);
                let material = fx_material(
                    &mut making.materials,
                    &images,
                    texture,
                    colour,
                    Blend::default(),
                );
                // Its bounds change each tick.
                let entity = making
                    .commands
                    .spawn((
                        Name::new("funnel"),
                        DemoEntity,
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(material),
                        NotShadowCaster,
                        NoFrustumCulling,
                        Transform::from_translation(from),
                    ))
                    .id();
                shown.parts.push((entity, handle));
            }
        } else {
            for (mesh, (entity, handle)) in meshes.into_iter().zip(&shown.parts) {
                if let Some(mut old) = making.meshes.get_mut(handle) {
                    *old = mesh;
                }
                if let Ok(mut transform) = places.get_mut(*entity) {
                    transform.translation = from;
                }
            }
        }
        let way = (to - from).normalize_or_zero();
        let age = shown.age;
        let mut spawner = making.spawner();
        for emit in recipes.expand(&funnel.at_gun) {
            spawner.emit(
                emit,
                from + way * funnel.at_gun_along,
                Some(way),
                eye,
                &mut rng,
            );
        }
        if rng.unit() < 0.5 {
            for emit in recipes.expand(&funnel.flowing) {
                spawner.emit(emit, from, Some(way), eye, &mut rng);
            }
        }
        if funnel.pulse_every > 0 && age % funnel.pulse_every == 1 {
            for emit in recipes.expand(&funnel.pulse) {
                spawner.emit(emit, from, Some(way), eye, &mut rng);
            }
        }
    }
    funnels.0.retain(|id, shown| {
        let keep = drawn.contains(id);
        if !keep {
            for (entity, _) in &shown.parts {
                making.commands.entity(*entity).despawn();
            }
        }
        keep
    });
}

/// The texture and colour of each mesh [`meshes_of`] makes.
fn funnel_looks(funnel: &Funnel) -> Vec<(Tex, [f32; 4])> {
    let mut looks = vec![(funnel.texture, funnel.colour)];
    for strip in funnel.spirals.iter().flat_map(|spirals| &spirals.strips) {
        looks.push((strip.core, strip.core_colour));
        looks.push((strip.glow, strip.glow_colour));
    }
    looks
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn funnel() -> Funnel {
        Funnel {
            pieces: 12,
            follow: (0.9, 0.5),
            round: 20,
            radius: (0.05, 1.234),
            from_ring: 1,
            texture: Tex::Effect(13),
            colour: opaque(),
            along_step: 0.0667,
            along_scroll: 0.04,
            round_scroll: 0.01,
            spirals: None,
            at_gun: Vec::new(),
            at_gun_along: 0.0,
            flowing: Vec::new(),
            pulse: Vec::new(),
            pulse_every: 0,
        }
    }

    #[test]
    fn its_rope_lags_behind_the_aim_most_at_its_far_end_and_catches_up() {
        let (from, to) = (Vec3::ZERO, Vec3::new(0.0, 0.0, -12.0));
        let mut rope = straight(12, from, to);
        // He turns a quarter round.
        let turned = Vec3::new(12.0, 0.0, 0.0);
        lay_rope(&mut rope, from, turned, (0.9, 0.5));
        let off = |rope: &[Vec3], point: usize| rope[point].angle_between(turned);
        assert!(off(&rope, 1) < off(&rope, 11));
        assert!(off(&rope, 11) > 0.3);
        for _ in 0..60 {
            lay_rope(&mut rope, from, turned, (0.9, 0.5));
        }
        assert!(off(&rope, 11) < 0.01);
        assert!((rope[11].length() - 11.0).abs() < 0.01);
    }

    #[test]
    fn the_funnel_shows_the_far_side_of_each_ring_twice() {
        let rope = straight(12, Vec3::ZERO, Vec3::new(0.0, 0.0, -12.0));
        let eye = Vec3::new(20.0, 3.0, -6.0);
        let mesh = funnel_mesh(&funnel(), &rope, eye, 1);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        // About half of 20 quads round each of the 10 rings' gaps, in two layers of 4 corners.
        let quads = positions.len() / 4;
        assert!((150..=250).contains(&quads), "{quads}");
        // All on the side away from the eye, seen near the far end where it is widest.
        let far: Vec<_> = positions.iter().filter(|p| p[2] < -10.0).collect();
        assert!(!far.is_empty());
        assert!(far.iter().all(|p| p[0] < 0.2));
    }

    #[test]
    fn a_strip_widens_as_its_glow_does() {
        let points = [
            Vec3::ZERO,
            Vec3::NEG_Z,
            Vec3::NEG_Z * 2.0,
            Vec3::NEG_Z * 3.0,
        ];
        let mesh = strip_mesh(&points, |share| share, Vec3::X * 10.0, 0.0);
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        assert_eq!(positions.len(), 12);
        assert!(positions[0][1].abs() < 1e-5);
        assert!((positions[10][1].abs() - 0.75).abs() < 1e-5);
    }
}
