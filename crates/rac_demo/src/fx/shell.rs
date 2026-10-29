//! The Shield Charger's shell while a hero's shield is up (`effects.md`, "Shield Charger",
//! "The shell"; 0x342098): a dome turned to the camera, its texture seen whole straight on
//! and blended in the shield's colour, and a ring round it, added, from the middle of a
//! glow at the dome's edge to the glow's edge outside. The colour flashes to the hit
//! colour whenever the shield's points change and fades back (`weapons.md`, "Its
//! colour"). Round it, arcs crackling out from its middle and back (0x341DA0) and trails
//! orbiting it (0x342478), strips rebuilt each frame facing the camera ("Arcs and
//! trails").

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::weapons::FireMode;
use rac_combat::{Armory, Arsenal};
use rac_hero::{Hero, HeroBody};
use serde::Deserialize;

use super::{Billboard, Blend, FxImages, FxRecipes, FxRng, Tex, eye, fx_material};
use crate::DemoEntity;

/// The shell round a hero whose shield is up.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Shell {
    /// Its middle, this far above his feet.
    pub above: f32,
    pub dome: Dome,
    pub rim: Rim,
    /// Its colour at rest and just after its points change (0x80 being 1.0), which it
    /// fades back from over `flash` ticks.
    pub rest: [f32; 3],
    pub hit: [f32; 3],
    pub flash: u32,
    #[serde(default)]
    pub arcs: Option<Arcs>,
    #[serde(default)]
    pub trails: Option<Trails>,
}

/// Arcs crackling out from the shell's middle and back (0x3410E8, 0x341DA0): `count`
/// loops, each of `steps` pieces `step` long out from the middle, each piece turned from
/// the last by `turn` degrees about the way from the camera, one way and then the other,
/// and a last piece straight back. Each lives `life` ticks, fading from `alpha` to
/// nothing, and is then laid anew; it keeps its shape as the middle moves. Each piece is a
/// core `width` across of `core` in grey, in a glow `times` as wide of `glow` in `edge`,
/// both added.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Arcs {
    pub count: usize,
    pub steps: usize,
    pub step: f32,
    pub turn: (f32, f32),
    pub life: (u32, u32),
    pub alpha: f32,
    pub width: f32,
    pub core: Tex,
    pub times: f32,
    pub glow: Tex,
    pub edge: [f32; 3],
}

/// Trails orbiting the shell's middle (0x340DD0, 0x342478): four heads, each turning
/// `turn` degrees a tick about its own axis, nudged up to `jitter` along each axis and
/// kept within `radius` of the middle, trailing their last `points` places (which move
/// with the middle). Each is drawn from its tail to the place before its head as a core
/// `width` across, in grey fading in to `alpha`, in a glow of one quad a piece `glow`
/// across and stretched `stretch` of the piece's length on past both its ends, in the
/// shell's colour fading in the same way; both of `texture`, added.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Trails {
    pub points: usize,
    pub turn: f32,
    pub radius: (f32, f32),
    pub jitter: f32,
    pub texture: Tex,
    pub width: f32,
    pub glow: f32,
    pub stretch: f32,
    pub alpha: f32,
}

/// The trails' heads and the axes they turn about as the shield goes up, the game's (x, y,
/// z) taken as the kit's (x, −z, y): along x about z, along y about x, along z about y, and
/// halfway between x and z about that × y (0x340DD0, the moby's turn then nothing).
fn orbits() -> [(Vec3, Vec3); 4] {
    let half = std::f32::consts::FRAC_1_SQRT_2;
    [
        (Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::X),
        (Vec3::Y, Vec3::NEG_Z),
        (Vec3::new(half, half, 0.0), Vec3::new(-half, half, 0.0)),
    ]
}

/// A dome `radius` round bulging toward the camera, of `texture` seen whole across it, at
/// `alpha`, blended.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Dome {
    pub texture: Tex,
    pub radius: f32,
    pub alpha: f32,
}

/// A ring facing the camera from the dome's edge to `outer`, `texture`'s middle along its
/// inner edge and the texture's edge along its outer, at `alpha`, added.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Rim {
    pub texture: Tex,
    pub outer: f32,
    pub alpha: f32,
}

/// How finely the dome and ring are cut: the game's 22.5° of longitude and 11.25° of
/// latitude, and its ring's 32 quads.
const AROUND: u32 = 32;
const BANDS: u32 = 8;

/// A dome of radius 1 bulging along +z, its texture projected straight along z so it shows
/// whole from in front, as the game's (0x2A5AE0's coordinates).
pub(crate) fn dome_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for band in 0..=BANDS {
        // From the rim (z 0) to the top (z 1).
        let up = band as f32 / BANDS as f32 * std::f32::consts::FRAC_PI_2;
        let (across, z) = (ops::cos(up), ops::sin(up));
        for step in 0..=AROUND {
            let turn = step as f32 / AROUND as f32 * std::f32::consts::TAU;
            let (x, y) = (ops::cos(turn) * across, ops::sin(turn) * across);
            positions.push([x, y, z]);
            uvs.push([0.5 + x / 2.0, 0.5 - y / 2.0]);
        }
    }
    let row = AROUND + 1;
    for band in 0..BANDS {
        for step in 0..AROUND {
            let at = band * row + step;
            indices.extend([at, at + 1, at + row, at + 1, at + row + 1, at + row]);
        }
    }
    mesh(positions, uvs, indices)
}

/// A flat ring facing +z from radius 1 to `outer`, its inner edge at the texture's middle
/// and its outer at the texture's edge (0x2A7620, 0x342098).
pub(crate) fn rim_mesh(outer: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for step in 0..=AROUND {
        let turn = step as f32 / AROUND as f32 * std::f32::consts::TAU;
        let out = Vec2::new(ops::cos(turn), ops::sin(turn));
        positions.push(out.extend(0.0).to_array());
        positions.push((out * outer).extend(0.0).to_array());
        uvs.extend([[0.5, 0.5], [0.5, 1.0]]);
    }
    for step in 0..AROUND {
        let at = step * 2;
        indices.extend([at, at + 2, at + 1, at + 1, at + 2, at + 3]);
    }
    mesh(positions, uvs, indices)
}

fn mesh(positions: Vec<[f32; 3]>, uvs: Vec<[f32; 2]>, indices: Vec<u32>) -> Mesh {
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// The shell's colour `flash` of the way back from a hit (1 just after it):
/// `colour_lerp`'s rest × (1 − t) + hit × t.
pub(crate) fn colour(shell: &Shell, flash: f32) -> [f32; 3] {
    std::array::from_fn(|channel| shell.rest[channel] * (1.0 - flash) + shell.hit[channel] * flash)
}

/// One arc as laid: its points from the middle, and the ticks it has left of its life.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Arc {
    points: Vec<Vec3>,
    left: u32,
    life: u32,
}

impl Arc {
    /// A tick on: once its life is up, laid anew out from `middle` as `eye` sees it.
    fn step(&mut self, arcs: &Arcs, middle: Vec3, eye: Vec3, rng: &mut FxRng) {
        self.left = self.left.saturating_sub(1);
        if self.left == 0 {
            self.life = rng.ticks(arcs.life.0, arcs.life.1).max(1);
            self.left = self.life;
            self.points = lay_arc(arcs, middle, eye, rng);
        }
    }

    /// How much of its alpha it shows: all as it is laid, a share less each tick.
    fn shade(&self) -> f32 {
        self.left as f32 / self.life.max(1) as f32
    }
}

/// An arc's points from the middle, laid out from `middle` as 0x3410E8 lays them: a step
/// of the arcs' length any way, then each next step turned from the last about the way
/// from `eye` to where it starts, by a random angle, one way and then the other.
pub(crate) fn lay_arc(arcs: &Arcs, middle: Vec3, eye: Vec3, rng: &mut FxRng) -> Vec<Vec3> {
    let mut step = rng.direction(None) * arcs.step;
    let (mut at, mut sign) = (middle, 1.0);
    let mut points = vec![Vec3::ZERO];
    for _ in 0..arcs.steps {
        let axis = (at - eye).try_normalize().unwrap_or(Vec3::Z);
        step = Quat::from_axis_angle(axis, sign * rng.between(arcs.turn).to_radians()) * step;
        sign = -sign;
        at += step;
        points.push(at - middle);
    }
    points
}

/// One trail: its head from the middle, the axis it turns about, and its places from the
/// oldest to the head.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Orbit {
    head: Vec3,
    axis: Vec3,
    places: Vec<Vec3>,
}

impl Orbit {
    fn new((head, axis): (Vec3, Vec3)) -> Self {
        Self {
            head,
            axis,
            places: Vec::new(),
        }
    }

    /// A tick on: the head turned about its axis, nudged, kept within the trails' radius,
    /// and added to its places, the oldest dropped past the trails' points.
    fn step(&mut self, trails: &Trails, rng: &mut FxRng) {
        let jitter = (-trails.jitter, trails.jitter);
        let mut head = Quat::from_axis_angle(self.axis, trails.turn.to_radians()) * self.head;
        head += Vec3::new(
            rng.between(jitter),
            rng.between(jitter),
            rng.between(jitter),
        );
        let (near, far) = trails.radius;
        let out = head.length();
        if out > far {
            head *= far / out;
        } else if out < near && out > 0.0 {
            head *= near / out;
        }
        self.head = head;
        self.places.push(head);
        if self.places.len() > trails.points {
            self.places.remove(0);
        }
    }

    /// What 0x342848 draws of it: its places but the head's.
    fn drawn(&self) -> &[Vec3] {
        &self.places[..self.places.len().saturating_sub(1)]
    }
}

/// Camera-facing quads gathered into one mesh, each with its texture whole along it and a
/// shade in its vertices' alpha at each end.
#[derive(Debug, Default)]
pub(crate) struct Quads {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Quads {
    /// A quad from `from` to `to`, `sides` out either side of each, its texture's u running
    /// along it and v across.
    fn quad(&mut self, (from, to): (Vec3, Vec3), sides: (Vec3, Vec3), shades: (f32, f32)) {
        let first = self.positions.len() as u32;
        self.positions.extend(
            [from + sides.0, from - sides.0, to + sides.1, to - sides.1]
                .map(|corner| corner.to_array()),
        );
        self.uvs
            .extend([[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]]);
        let (a, b) = shades;
        self.colours.extend([
            [1.0, 1.0, 1.0, a],
            [1.0, 1.0, 1.0, a],
            [1.0, 1.0, 1.0, b],
            [1.0, 1.0, 1.0, b],
        ]);
        self.indices
            .extend([first, first + 1, first + 2, first + 2, first + 1, first + 3]);
    }

    fn mesh(mut self) -> Mesh {
        if self.positions.is_empty() {
            // Nothing to show, but never an empty buffer for the renderer.
            self.quad(
                (Vec3::ZERO, Vec3::ZERO),
                (Vec3::ZERO, Vec3::ZERO),
                (0.0, 0.0),
            );
        }
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

/// `half` out to the side of a strip going `way` at `at`, as `eye` sees it.
fn side(way: Vec3, at: Vec3, eye: Vec3, half: f32) -> Vec3 {
    way.cross(at - eye).try_normalize().unwrap_or(Vec3::Y) * half
}

/// The arcs' cores and glows, from the middle, as 0x407240 draws each: each point's side
/// from the piece coming into it (the first's from the piece leaving it), each piece a
/// core quad and a glow quad `times` as wide, and the last piece back to the first point.
pub(crate) fn arc_meshes(arcs: &Arcs, laid: &[Arc], eye: Vec3) -> (Mesh, Mesh) {
    let (mut cores, mut glows) = (Quads::default(), Quads::default());
    for arc in laid {
        let points = &arc.points;
        if points.len() < 2 {
            continue;
        }
        let sides: Vec<Vec3> = (0..points.len())
            .map(|index| {
                let way = match index {
                    0 => points[1] - points[0],
                    _ => points[index] - points[index - 1],
                };
                side(way, points[index], eye, arcs.width / 2.0)
            })
            .collect();
        let shade = arc.shade();
        for from in 0..points.len() {
            let to = (from + 1) % points.len();
            let ends = (points[from], points[to]);
            let sides = (sides[from], sides[to]);
            cores.quad(ends, sides, (shade, shade));
            glows.quad(
                ends,
                (sides.0 * arcs.times, sides.1 * arcs.times),
                (shade, shade),
            );
        }
    }
    (cores.mesh(), glows.mesh())
}

/// The trails' cores and glows, from the middle, as 0x342848 draws each: the core through
/// its places, each point's side from the piece coming into it, and the glow a quad a
/// piece, its ends first taken out from the camera to the farther one's distance and then
/// stretched on past both; both fading in from nothing at the tail.
pub(crate) fn trail_meshes(trails: &Trails, orbits: &[Orbit], eye: Vec3) -> (Mesh, Mesh) {
    let (mut cores, mut glows) = (Quads::default(), Quads::default());
    for orbit in orbits {
        let points = orbit.drawn();
        if points.len() < 2 {
            continue;
        }
        let last = (orbit.places.len() - 1) as f32;
        let sides: Vec<Vec3> = (0..points.len())
            .map(|index| {
                let way = points[index.max(1)] - points[index.max(1) - 1];
                side(way, points[index], eye, trails.width / 2.0)
            })
            .collect();
        for from in 0..points.len() - 1 {
            let to = from + 1;
            let shades = (from as f32 / last, to as f32 / last);
            cores.quad((points[from], points[to]), (sides[from], sides[to]), shades);
            let (near, far) = (points[from] - eye, points[to] - eye);
            let out = near.length().max(far.length());
            let (a, b) = (
                eye + near.normalize_or_zero() * out,
                eye + far.normalize_or_zero() * out,
            );
            let stretch = (b - a) * trails.stretch;
            let (a, b) = (a - stretch, b + stretch);
            let sides = side(b - a, a, eye, trails.glow / 2.0);
            glows.quad((a, b), (sides, sides), shades);
        }
    }
    (cores.mesh(), glows.mesh())
}

/// A shell's arcs or trails: their two meshes, the core's first, and the glow's material.
#[derive(Debug)]
struct Strips {
    meshes: [Handle<Mesh>; 2],
    glow: Handle<StandardMaterial>,
}

/// One hero's shell as shown.
#[derive(Debug)]
pub(crate) struct Shown {
    parts: Vec<Entity>,
    materials: [Handle<StandardMaterial>; 2],
    points: f32,
    /// Ticks of its flash left.
    flash: f32,
    arcs: Vec<Arc>,
    orbits: Vec<Orbit>,
    arc_strips: Option<Strips>,
    trail_strips: Option<Strips>,
    /// The part of a tick since its arcs and trails last stepped.
    due: f32,
}

/// The shells shown, by player.
#[derive(Resource, Debug, Default)]
pub(crate) struct Shells(BTreeMap<usize, Shown>);

/// A part of a shell.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellPart;

/// The shield's gun among a hero's: its recipe's key and level.
fn shield_gun<'a>(armory: &'a Armory, arsenal: &Arsenal) -> Option<(&'a str, u8)> {
    arsenal.guns.iter().find_map(|gun| {
        let def = armory.get(gun.weapon)?;
        matches!(def.fire, FireMode::Shield(_)).then_some((def.effects.as_str(), gun.level))
    })
}

/// Shows a shell round each hero whose shield is up, in its colour, with its arcs and
/// trails, and drops it as the shield goes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_shells(
    time: Res<Time>,
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    heroes: Query<(&Hero, &Arsenal, &HeroBody, &Transform)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut places: Query<&mut Transform, (With<ShellPart>, Without<HeroBody>)>,
    mut shells: ResMut<Shells>,
    mut rng: ResMut<FxRng>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    let ticks = time.delta_secs() * 60.0;
    let eye = eye(&cameras);
    let mut up = BTreeMap::new();
    for (hero, arsenal, body, placed) in &heroes {
        let shield = &body.0.shield;
        let Some(shell) = shield
            .is_up()
            .then(|| shield_gun(&armory, arsenal))
            .flatten()
            .and_then(|(key, level)| recipes.shell(key, level))
        else {
            continue;
        };
        let shown = shells.0.entry(hero.player).or_insert_with(|| {
            let mut material = |texture, colour, blend| {
                fx_material(&mut materials, &images, texture, colour, blend)
            };
            let dome = material(shell.dome.texture, [1.0; 4], Blend::Alpha);
            let rim = material(shell.rim.texture, [1.0; 4], Blend::Add);
            let arc_looks = shell.arcs.as_ref().map(|arcs| {
                let [r, g, b] = arcs.edge;
                [
                    material(arcs.core, [0.992, 0.992, 0.992, arcs.alpha], Blend::Add),
                    material(arcs.glow, [r, g, b, arcs.alpha], Blend::Add),
                ]
            });
            let trail_looks = shell.trails.as_ref().map(|trails| {
                [
                    material(
                        trails.texture,
                        [0.992, 0.992, 0.992, trails.alpha],
                        Blend::Add,
                    ),
                    material(trails.texture, [1.0, 1.0, 1.0, trails.alpha], Blend::Add),
                ]
            });
            let mut parts = Vec::new();
            let mut part = |mesh: Handle<Mesh>,
                            material: &Handle<StandardMaterial>,
                            scale: f32,
                            facing: bool| {
                let mut entity = commands.spawn((
                    Name::new("shield shell"),
                    DemoEntity,
                    ShellPart,
                    NotShadowCaster,
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3::splat(scale)),
                ));
                if facing {
                    entity.insert(Billboard::default());
                } else {
                    // Rebuilt each frame, so its first bounds would hide it.
                    entity.insert(NoFrustumCulling);
                }
                parts.push(entity.id());
            };
            let radius = shell.dome.radius;
            part(meshes.add(dome_mesh()), &dome, radius, true);
            let rim_mesh = meshes.add(rim_mesh(shell.rim.outer / radius));
            part(rim_mesh, &rim, radius, true);
            let mut strips = |looks: Option<[Handle<StandardMaterial>; 2]>| {
                let [core, glow] = looks?;
                let handles = [(); 2].map(|_| meshes.add(Quads::default().mesh()));
                part(handles[0].clone(), &core, 1.0, false);
                part(handles[1].clone(), &glow, 1.0, false);
                Some(Strips {
                    meshes: handles,
                    glow,
                })
            };
            let arc_strips = strips(arc_looks);
            let trail_strips = strips(trail_looks);
            Shown {
                parts,
                materials: [dome, rim],
                points: shield.points,
                flash: 0.0,
                arcs: vec![Arc::default(); shell.arcs.as_ref().map_or(0, |arcs| arcs.count)],
                orbits: orbits().map(Orbit::new).to_vec(),
                arc_strips,
                trail_strips,
                due: 0.0,
            }
        });
        if shield.points != shown.points {
            shown.points = shield.points;
            shown.flash = shell.flash as f32;
        }
        shown.flash = (shown.flash - ticks).max(0.0);
        let [r, g, b] = colour(shell, shown.flash / shell.flash.max(1) as f32);
        for (handle, alpha) in shown
            .materials
            .iter()
            .zip([shell.dome.alpha, shell.rim.alpha])
        {
            if let Some(mut material) = materials.get_mut(handle) {
                material.base_color = Color::srgba(r, g, b, alpha);
            }
        }
        let middle = placed.translation + Vec3::Y * shell.above;
        // The game steps them each frame as it draws; the kit's whole ticks, at most a
        // few a frame.
        shown.due += ticks;
        let steps = shown.due.floor();
        shown.due -= steps;
        for _ in 0..(steps as u32).min(4) {
            if let Some(arcs) = &shell.arcs {
                for arc in &mut shown.arcs {
                    arc.step(arcs, middle, eye, &mut rng);
                }
            }
            if let Some(trails) = &shell.trails {
                for orbit in &mut shown.orbits {
                    orbit.step(trails, &mut rng);
                }
            }
        }
        if let (Some(arcs), Some(strips)) = (&shell.arcs, &shown.arc_strips) {
            let (core, glow) = arc_meshes(arcs, &shown.arcs, eye - middle);
            put(&mut meshes, &strips.meshes, [core, glow]);
        }
        if let (Some(trails), Some(strips)) = (&shell.trails, &shown.trail_strips) {
            let (core, glow) = trail_meshes(trails, &shown.orbits, eye - middle);
            put(&mut meshes, &strips.meshes, [core, glow]);
            if let Some(mut material) = materials.get_mut(&strips.glow) {
                material.base_color = Color::srgba(r, g, b, trails.alpha);
            }
        }
        for &part in &shown.parts {
            if let Ok(mut place) = places.get_mut(part) {
                place.translation = middle;
            }
        }
        up.insert(hero.player, ());
    }
    shells.0.retain(|player, shown| {
        let keep = up.contains_key(player);
        if !keep {
            for &part in &shown.parts {
                commands.entity(part).despawn();
            }
        }
        keep
    });
}

/// Puts this frame's meshes in place of last frame's.
fn put(meshes: &mut Assets<Mesh>, handles: &[Handle<Mesh>; 2], made: [Mesh; 2]) {
    for (handle, mesh) in handles.iter().zip(made) {
        if let Some(mut old) = meshes.get_mut(handle) {
            *old = mesh;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn points(mesh: &Mesh) -> Vec<Vec3> {
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        points.iter().copied().map(Vec3::from).collect()
    }

    #[test]
    fn the_dome_bulges_toward_the_camera_and_shows_its_texture_whole_from_in_front() {
        let mesh = dome_mesh();
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("uvs");
        };
        for (point, uv) in points(&mesh).iter().zip(uvs) {
            assert!((point.length() - 1.0).abs() < 1e-5 && point.z >= -1e-6);
            // Straight on: where it shows across the disc is where it samples.
            assert!((uv[0] - (0.5 + point.x / 2.0)).abs() < 1e-5);
            assert!((uv[1] - (0.5 - point.y / 2.0)).abs() < 1e-5);
        }
    }

    #[test]
    fn the_rim_runs_from_the_domes_edge_out_from_the_glows_middle_to_its_edge() {
        let mesh = rim_mesh(1.316 / 0.9);
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("uvs");
        };
        for (point, uv) in points(&mesh).iter().zip(uvs) {
            let inner = (point.length() - 1.0).abs() < 1e-5;
            assert!(inner || (point.length() - 1.316 / 0.9).abs() < 1e-4);
            assert_eq!(uv[1], if inner { 0.5 } else { 1.0 });
        }
    }

    fn arcs() -> Arcs {
        Arcs {
            count: 5,
            steps: 8,
            step: 0.15,
            turn: (5.0, 75.0),
            life: (3, 45),
            alpha: 0.25,
            width: 0.08,
            core: Tex::Effect(23),
            times: 4.0,
            glow: Tex::Effect(14),
            edge: [1.0, 0.625, 0.25],
        }
    }

    fn trails() -> Trails {
        Trails {
            points: 20,
            turn: 6.0,
            radius: (0.9, 1.1),
            jitter: 0.025,
            texture: Tex::Effect(44),
            width: 0.04,
            glow: 0.14,
            stretch: 1.3,
            alpha: 0.25,
        }
    }

    fn alphas(mesh: &Mesh) -> Vec<f32> {
        let Some(VertexAttributeValues::Float32x4(colours)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("colours");
        };
        colours.iter().map(|colour| colour[3]).collect()
    }

    #[test]
    fn an_arc_zigzags_out_from_the_middle_in_steps_of_its_length() {
        let middle = Vec3::new(2.0, 1.0, -3.0);
        let eye = Vec3::new(2.0, 3.0, 5.0);
        let mut rng = FxRng(5);
        for _ in 0..20 {
            let points = lay_arc(&arcs(), middle, eye, &mut rng);
            assert_eq!(points.len(), 9);
            assert_eq!(points[0], Vec3::ZERO);
            let steps: Vec<Vec3> = points.windows(2).map(|pair| pair[1] - pair[0]).collect();
            for pair in steps.windows(2) {
                assert!((pair[1].length() - 0.15).abs() < 1e-5);
                // Turned about an axis by 5-75°, so by no more than 75°.
                let angle = pair[0].angle_between(pair[1]).to_degrees();
                assert!(angle < 75.0 + 1e-3, "{angle}");
            }
        }
    }

    #[test]
    fn an_arc_fades_over_its_life_and_is_laid_anew() {
        let mut rng = FxRng(9);
        let mut arc = Arc::default();
        arc.step(&arcs(), Vec3::ZERO, Vec3::Z * 5.0, &mut rng);
        let (life, first) = (arc.life, arc.points.clone());
        assert!((3..=45).contains(&life));
        assert_eq!(arc.shade(), 1.0);
        for _ in 1..life {
            arc.step(&arcs(), Vec3::ZERO, Vec3::Z * 5.0, &mut rng);
            assert_eq!(arc.points, first);
        }
        assert!((arc.shade() - 1.0 / life as f32).abs() < 1e-6);
        arc.step(&arcs(), Vec3::ZERO, Vec3::Z * 5.0, &mut rng);
        assert_eq!(arc.shade(), 1.0);
        assert_ne!(arc.points, first);
    }

    #[test]
    fn an_arcs_pieces_close_back_to_the_middle_in_a_glow_four_times_its_core() {
        let mut rng = FxRng(3);
        let mut arc = Arc::default();
        arc.step(&arcs(), Vec3::ZERO, Vec3::Z * 5.0, &mut rng);
        let (cores, glows) = arc_meshes(&arcs(), std::slice::from_ref(&arc), Vec3::Z * 5.0);
        let (cores, glows) = (points(&cores), points(&glows));
        // Nine pieces of four corners, the last ending at the first point.
        assert_eq!(cores.len(), 9 * 4);
        let last = &cores[8 * 4..];
        assert!(((last[2] + last[3]) / 2.0).length() < 1e-6);
        for (core, glow) in cores.chunks(2).zip(glows.chunks(2)) {
            let (core, glow) = ((core[0] - core[1]).length(), (glow[0] - glow[1]).length());
            assert!((core - 0.08).abs() < 1e-5 && (glow - 0.32).abs() < 1e-5);
        }
    }

    #[test]
    fn a_trail_orbits_its_axis_within_its_radius_keeping_its_last_places() {
        let mut rng = FxRng(11);
        let still = Trails {
            jitter: 0.0,
            ..trails()
        };
        let mut orbit = Orbit::new(orbits()[0]);
        for _ in 0..30 {
            orbit.step(&still, &mut rng);
        }
        assert_eq!(orbit.places.len(), 20);
        assert_eq!(orbit.drawn().len(), 19);
        for pair in orbit.places.windows(2) {
            assert!((pair[0].angle_between(pair[1]).to_degrees() - 6.0).abs() < 1e-2);
            assert!(pair[1].y.abs() < 1e-6);
        }
        let mut orbit = Orbit::new(orbits()[3]);
        for _ in 0..200 {
            orbit.step(&trails(), &mut rng);
            let out = orbit.head.length();
            assert!((0.9 - 1e-5..=1.1 + 1e-5).contains(&out), "{out}");
        }
    }

    #[test]
    fn a_trails_glow_stretches_past_its_pieces_and_fades_in_from_the_tail() {
        let eye = Vec3::Z * 10.0;
        // Places round the eye at one distance, so the glow's ends stay where they are.
        let places: Vec<Vec3> = (0..5)
            .map(|index| eye + Quat::from_rotation_y(index as f32 * 0.02) * Vec3::NEG_Z * 10.0)
            .collect();
        let orbit = Orbit {
            head: places[4],
            axis: Vec3::Y,
            places: places.clone(),
        };
        let (cores, glows) = trail_meshes(&trails(), std::slice::from_ref(&orbit), eye);
        // Three pieces through the four places before the head.
        assert_eq!(points(&cores).len(), 3 * 4);
        let piece = (places[1] - places[0]).length();
        let glow = points(&glows);
        let along = ((glow[2] + glow[3]) - (glow[0] + glow[1])).length() / 2.0;
        assert!((along - piece * 3.6).abs() < 1e-4, "{along} {piece}");
        assert!(((glow[0] - glow[1]).length() - 0.14).abs() < 1e-5);
        let shades = alphas(&cores);
        assert_eq!(shades[0], 0.0);
        assert!((shades[shades.len() - 1] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn the_colour_flashes_to_the_hit_colour_and_fades_back() {
        let shell = Shell {
            above: 0.75,
            dome: Dome {
                texture: Tex::Effect(21),
                radius: 0.9,
                alpha: 0.1875,
            },
            rim: Rim {
                texture: Tex::Effect(11),
                outer: 1.316,
                alpha: 0.5,
            },
            rest: [1.125, 0.25, 0.25],
            hit: [0.25, 0.25, 1.25],
            flash: 30,
            arcs: None,
            trails: None,
        };
        assert_eq!(colour(&shell, 1.0), shell.hit);
        assert_eq!(colour(&shell, 0.0), shell.rest);
        assert_eq!(colour(&shell, 0.5), [0.6875, 0.25, 0.75]);
    }
}
