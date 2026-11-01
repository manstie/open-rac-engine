//! The Suck Cannon's vortex while it sucks (`effects.md`, "The Suck Cannon's vortex"; 0x363E98,
//! drawn by 0x3633F8 and 0x365328): a tube of ten rings of twenty points on a rope that lags
//! behind his aim, showing a ring at a time as it reaches out and stopping where it meets the
//! world, its side towards the camera drawn in a scrolling texture; arcs spinning fast round the
//! way, one for each ring fully out; and smoke and dust flowing round its wall to the gun.
//! The game's refractions (0x3630E0) aren't drawn.

use std::collections::BTreeMap;
use std::f32::consts::{FRAC_1_SQRT_2, TAU};

use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::{Armory, Arsenal, GunEvent, GunEventKind};
use rac_hero::Hero;
use serde::Deserialize;

use super::funnel::Quads;
use super::{Blend, FxImages, FxRecipes, FxRng, Making, Tex, eye, fx_material, level};
use crate::DemoEntity;

const RINGS: usize = 10;
const ROUND: usize = 20;
/// The rope's step, the game's 0x1E1B64.
const STEP: f32 = 1.0;
const MOST_MOTES: usize = 200;
const MOTES_A_TICK: usize = 5;
/// The most a piece of the rope lags behind the piece before it, in radians.
const MOST_LAG: f32 = 0.31415927;
/// The most a ring's counter is drawn at, in the game's alpha, and how much each ring is
/// behind the one before it.
const SHOWN: i32 = 0x20;

/// How a Suck Cannon's vortex looks by its level: the tube and arcs' textures, and what flows
/// round it. Colours are the game's over 0x80.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Vortex {
    pub tube: Tex,
    pub tube_colour: [f32; 3],
    pub arcs: Tex,
    pub arc_colour: [f32; 4],
    pub smoke: Tex,
    pub smoke_colour: [f32; 3],
    /// One of these is chosen for each mote of dust.
    pub dust: Vec<Tex>,
    pub dust_colour: [f32; 3],
    pub dust_half: (f32, f32),
    /// All dust is blended, not one in four as smoke is.
    #[serde(default)]
    pub dust_blended: bool,
}

impl Vortex {
    pub(crate) fn textures(&self) -> impl Iterator<Item = Tex> + '_ {
        [self.tube, self.arcs, self.smoke]
            .into_iter()
            .chain(self.dust.iter().copied())
    }

    /// The texture, colour and blend of each mesh a vortex shows: the tube, its arcs, and
    /// its smoke and each dust both added and blended.
    fn looks(&self) -> Vec<(Tex, [f32; 4], Blend)> {
        let [r, g, b] = self.tube_colour;
        let mut looks = vec![
            (self.tube, [r, g, b, 1.0], Blend::Add),
            (self.arcs, self.arc_colour, Blend::Add),
        ];
        let [r, g, b] = self.smoke_colour;
        looks.push((self.smoke, [r, g, b, 1.0], Blend::Add));
        looks.push((self.smoke, [r, g, b, 1.0], Blend::Alpha));
        let [r, g, b] = self.dust_colour;
        for &texture in &self.dust {
            looks.push((texture, [r, g, b, 1.0], Blend::Add));
            looks.push((texture, [r, g, b, 1.0], Blend::Alpha));
        }
        looks
    }
}

/// A sprite flowing round the tube to the gun.
#[derive(Debug, Clone)]
struct Mote {
    /// The ring it is flowing from, towards the one before it.
    ring: usize,
    along: f32,
    speed: f32,
    /// The ring's point it is at, moving to the one before it.
    point: usize,
    between: f32,
    turn: f32,
    half: f32,
    at: Vec3,
    roll: f32,
    spin: f32,
    /// It is dust, not smoke: further out, in the [`Vortex::dust`] of this number.
    dust: Option<usize>,
    added: bool,
    /// The game's life, kept at 200 and falling only near the gun; its alpha follows it.
    life: f32,
}

/// A vortex drawn now.
#[derive(Debug)]
struct Shown {
    rope: [Vec3; RINGS],
    /// The speed of each piece of the rope's turning.
    swing: [f32; RINGS],
    /// Each ring's counter: its alpha, once within 0 to [`SHOWN`].
    shows: [i32; RINGS],
    /// Each ring's points from the rope.
    rings: [[Vec3; ROUND]; RINGS],
    /// Where the rope's start wanders in view, from `.0` to `.1` over 60 ticks.
    wander: (Vec2, Vec2),
    wander_ticks: u32,
    wander_turn: f32,
    pulse: f32,
    along: f32,
    around: f32,
    arc_turns: [f32; RINGS],
    motes: Vec<Mote>,
    parts: Vec<(Entity, Handle<Mesh>)>,
}

/// The vortices drawn now, by player and gun.
#[derive(Resource, Debug, Default)]
pub(crate) struct Vortices(BTreeMap<(usize, usize), Shown>);

impl Shown {
    /// A new vortex (0x363C60): the rope straight along his aim, no ring showing.
    fn new(from: Vec3, aim: Vec3) -> Self {
        let mut rope = [from; RINGS];
        for (ring, point) in rope.iter_mut().enumerate() {
            *point = from + aim * STEP * ring as f32;
        }
        Self {
            rope,
            swing: [0.0; RINGS],
            shows: [0; RINGS],
            rings: [[Vec3::ZERO; ROUND]; RINGS],
            wander: (Vec2::ZERO, Vec2::ZERO),
            wander_ticks: 0,
            wander_turn: 0.0,
            pulse: 0.0,
            along: 0.0,
            around: 0.0,
            arc_turns: [0.0; RINGS],
            motes: Vec::new(),
            parts: Vec::new(),
        }
    }
}

/// A ring's points, each `radius` from its centre, square to `way` (the table 0x2B37B0).
fn ring_points(way: Vec3, up: Vec3, radius: f32) -> ([Vec3; ROUND], Vec3) {
    let side = way.cross(up).normalize_or(Vec3::X);
    let up = side.cross(way);
    let points = std::array::from_fn(|point| {
        // The table starts 18° round from its y axis.
        let angle = (point + 1) as f32 * TAU / ROUND as f32;
        (side * -ops::cos(angle) + up * -ops::sin(angle)) * radius
    });
    (points, up)
}

/// How wide ring `ring` is `pulse` into its breathing.
fn ring_radius(ring: usize, pulse: f32) -> f32 {
    let along = ring as f32;
    along * 0.2 + 0.1 + ops::sin(pulse + along * 0.7) * 0.1
}

/// Moves each ring's counter on a tick: out by one ring every eight ticks, or back from where
/// it met the world, `reach` from the gun.
fn step_shows(shows: &mut [i32; RINGS], reach: f32) {
    let met = (reach / STEP) as usize;
    if met >= RINGS {
        shows[0] += 4;
        for ring in 1..RINGS {
            shows[ring] = shows[ring - 1] - SHOWN;
        }
        return;
    }
    for show in &mut shows[met..] {
        *show = ((*show).min(SHOWN) - 4).max(0);
    }
    for ring in (0..met).rev() {
        shows[ring] = shows[ring + 1] + SHOWN;
    }
}

/// Moves the rope on a tick: each piece turns towards the way of the piece before it, on a
/// spring, lagging no more than [`MOST_LAG`], and the far pieces keep part of where they were.
fn lay_rope(shown: &mut Shown, from: Vec3, aim: Vec3, rng: &mut FxRng) {
    let side = aim.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = side.cross(aim);
    let share = shown.wander_ticks as f32 / 60.0;
    let wander = shown.wander.0.lerp(shown.wander.1, share);
    shown.wander_ticks += 1;
    if shown.wander_ticks >= 60 {
        shown.wander_ticks = 0;
        shown.wander.0 = shown.wander.1;
        shown.wander_turn += rng.between((45.0, 315.0)).to_radians();
        let (sin, cos) = (ops::sin(shown.wander_turn), ops::cos(shown.wander_turn));
        shown.wander.1 = Vec2::new(sin, cos) * 0.1;
    }
    let mut target = aim * STEP + side * wander.x + up * wander.y;
    let old = shown.rope;
    let (first, mut ring_up) = ring_points(target.normalize_or(aim), up, 0.1);
    shown.rope[0] = from;
    shown.rings[0] = first;
    for (ring, &old) in old.iter().enumerate().skip(1) {
        let was = old - shown.rope[ring - 1];
        let length = was.length() * target.length();
        let cos = if length > 0.0 {
            was.dot(target) / length
        } else {
            1.0
        };
        let angle = ops::acos(cos.clamp(-1.0, 1.0));
        let swing = shown.swing[ring] * 0.8 + 0.06 * angle;
        let swing = swing.clamp(-angle, angle);
        shown.swing[ring] = swing;
        let turn = swing.max(angle - MOST_LAG);
        let axis = was.cross(target).try_normalize();
        let turned = axis.map_or(was, |axis| Quat::from_axis_angle(axis, turn) * was);
        let mut way = turned.normalize_or(aim) * STEP;
        if ring > 5 {
            way = way.lerp(was, ring as f32 / 10.0);
        }
        shown.rope[ring] = shown.rope[ring - 1] + way;
        let along = way.normalize_or(aim);
        target = along * STEP;
        // Near straight up the world's up gives no side: it keeps the last ring's.
        let wanted = if along.y.abs() > 0.75 {
            ring_up
        } else {
            Vec3::Y
        };
        let (points, up) = ring_points(along, wanted, ring_radius(ring, shown.pulse));
        shown.rings[ring] = points;
        ring_up = up;
    }
    shown.pulse = (shown.pulse + 0.1).rem_euclid(TAU);
}

/// A new mote at the furthest ring fully out, or none if no ring past the first is.
fn new_mote(shown: &Shown, vortex: &Vortex, rng: &mut FxRng) -> Option<Mote> {
    let ring = (1..RINGS).rev().find(|&ring| shown.shows[ring] >= SHOWN)?;
    let dust = ring >= 2 && rng.unit() < 0.1 && !vortex.dust.is_empty();
    let added = if dust && vortex.dust_blended {
        false
    } else {
        rng.unit() >= 0.25
    };
    let spin = if rng.unit() < 0.5 { 0.1473 } else { -0.1473 };
    let point = (rng.unit() * ROUND as f32) as usize % ROUND;
    Some(if dust {
        let which = (rng.unit() * vortex.dust.len() as f32) as usize % vortex.dust.len();
        Mote {
            ring,
            along: 0.0,
            speed: rng.between((0.5, 0.9)),
            point,
            between: 0.0,
            turn: rng.between((0.2, 0.5)),
            half: rng.between(vortex.dust_half),
            at: shown.rope[ring],
            roll: rng.between((0.0, TAU)),
            spin,
            dust: Some(which),
            added,
            life: 200.0,
        }
    } else {
        Mote {
            ring,
            along: 0.0,
            speed: rng.between((0.1, 0.4)),
            point,
            between: 0.0,
            turn: rng.between((0.1, 0.3)),
            half: rng.between((90000.0 / 210000.0, 270000.0 / 210000.0)),
            at: shown.rope[ring],
            roll: rng.between((0.0, TAU)),
            spin,
            dust: None,
            added,
            life: 200.0,
        }
    })
}

/// Moves a mote on a tick; false when it has reached the gun.
fn step_mote(mote: &mut Mote, rope: &[Vec3; RINGS], rings: &[[Vec3; ROUND]; RINGS]) -> bool {
    mote.along += mote.speed;
    mote.roll += mote.spin;
    let mut length = rope[mote.ring - 1].distance(rope[mote.ring]);
    if mote.along > length {
        mote.ring -= 1;
        if mote.ring == 0 {
            return false;
        }
        mote.along -= length;
        length = rope[mote.ring - 1].distance(rope[mote.ring]);
    }
    let share = mote.along / length.max(0.001);
    mote.life = match mote.ring {
        1 => ((1.0 - share) * 100.0).max(60.0),
        2 => 100.0,
        _ => 200.0,
    };
    if mote.ring == 1 {
        // The last piece it just goes for the gun, shrinking.
        mote.at += (rope[0] - mote.at).normalize_or_zero() * mote.speed;
        mote.half *= 0.95;
        return true;
    }
    let at_way = rope[mote.ring].lerp(rope[mote.ring - 1], share);
    let next = (mote.point + ROUND - 1) % ROUND;
    let here = rings[mote.ring][mote.point].lerp(rings[mote.ring - 1][mote.point], share);
    let there = rings[mote.ring][next].lerp(rings[mote.ring - 1][next], share);
    let mut out = here.lerp(there, mote.between);
    mote.between += mote.turn;
    if mote.between >= 1.0 {
        mote.between -= 1.0;
        mote.point = next;
    }
    if mote.dust.is_some() {
        out *= 2.0;
    }
    mote.at = at_way + out;
    true
}

/// The tube's quads turned to `eye`, from the gun (0x3633F8).
fn tube_mesh(shown: &Shown, eye: Vec3) -> Mesh {
    let mut quads = Quads::default();
    let alpha = |ring: usize| shown.shows[ring].clamp(0, SHOWN) as f32 / 128.0;
    let start = shown.rope[0];
    for ring in 1..RINGS {
        let far_u = shown.along + ring as f32 * 0.3;
        let near_u = far_u - 0.3;
        let far_alpha = if ring == RINGS - 1 { 0.0 } else { alpha(ring) };
        let near_alpha = if ring == 1 {
            SHOWN as f32 / 128.0
        } else {
            alpha(ring - 1)
        };
        let v = shown.around + (ring - 1) as f32 * 0.006;
        for point in 0..ROUND {
            let next = (point + 1) % ROUND;
            let a = shown.rope[ring] + shown.rings[ring][point];
            let b = shown.rope[ring] + shown.rings[ring][next];
            let c = shown.rope[ring - 1] + shown.rings[ring - 1][point];
            let d = shown.rope[ring - 1] + shown.rings[ring - 1][next];
            if (c - b).cross(a - b).dot(b - eye) >= 0.0 {
                continue;
            }
            let (low, high) = (v + point as f32 * 0.05, v + (point + 1) as f32 * 0.05);
            quads.push([
                (a - start, [far_u, low], far_alpha),
                (b - start, [far_u, high], far_alpha),
                (c - start, [near_u, low], near_alpha),
                (d - start, [near_u, high], near_alpha),
            ]);
        }
    }
    quads.mesh()
}

/// An arc round each ring fully out, spun on and of a new span each tick, on a ball round the
/// ring from 15° to 35° up towards the rope's way (0x3650A0).
fn arcs_mesh(shown: &mut Shown, rng: &mut FxRng) -> Mesh {
    let (inner, outer) = (15.0_f32.to_radians(), 35.0_f32.to_radians());
    let start = shown.rope[0];
    let mut quads = Quads::default();
    for ring in 1..RINGS {
        if shown.shows[ring] <= SHOWN {
            break;
        }
        shown.arc_turns[ring] =
            (shown.arc_turns[ring] + rng.between((17.0, 25.0)).to_radians()).rem_euclid(TAU);
        let spun = shown.arc_turns[ring];
        let span = rng.between((40.0, 150.0)).to_radians();
        let way = (shown.rope[ring] - shown.rope[ring - 1]).normalize_or(Vec3::NEG_Z);
        let side = way.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = side.cross(way);
        let radius = ring as f32 * 0.17 + 0.35;
        let at = shown.rope[ring] - start;
        let point = |turn: f32, height: f32| {
            let round = side * ops::cos(turn) + up * ops::sin(turn);
            at + (round * ops::cos(height) + way * ops::sin(height)) * radius
        };
        for column in 0..20 {
            let (u, next) = (column as f32 / 20.0, (column + 1) as f32 / 20.0);
            let (turn, turn_next) = (spun + span * u, spun + span * next);
            quads.push([
                (point(turn, inner), [u, 0.0], 1.0),
                (point(turn_next, inner), [next, 0.0], 1.0),
                (point(turn, outer), [u, 1.0], 1.0),
                (point(turn_next, outer), [next, 1.0], 1.0),
            ]);
        }
    }
    quads.mesh()
}

/// The sprites of each kind, in the order of [`Vortex::looks`] after its tube and arcs.
fn motes_meshes(shown: &Shown, vortex: &Vortex, eye: Vec3) -> Vec<Mesh> {
    let mut quads: Vec<Quads> = (0..2 + 2 * vortex.dust.len())
        .map(|_| Quads::default())
        .collect();
    let start = shown.rope[0];
    for mote in &shown.motes {
        let toward = (eye - mote.at).normalize_or(Vec3::Z);
        let turned = Quat::from_axis_angle(toward, mote.roll);
        // The game's size is half the sprite's diagonal.
        let half = mote.half * FRAC_1_SQRT_2;
        let right = turned * toward.cross(Vec3::Y).normalize_or(Vec3::X) * half;
        let up = turned * right.cross(toward).normalize_or(Vec3::Y) * half;
        let at = mote.at - start;
        // The game's +0x2A: the alpha at a life of 200.
        let (kind, alpha) = match mote.dust {
            None => (0, 26.0 / 128.0),
            Some(which) => (2 + 2 * which, 47.0 / 128.0),
        };
        let alpha = alpha * mote.life / 200.0;
        let look = kind + usize::from(!mote.added);
        quads[look].push([
            (at - right + up, [0.0, 0.0], alpha),
            (at + right + up, [1.0, 0.0], alpha),
            (at - right - up, [0.0, 1.0], alpha),
            (at + right - up, [1.0, 1.0], alpha),
        ]);
    }
    quads.into_iter().map(Quads::mesh).collect()
}

/// Draws each vortex its gun's sucking asks for this tick, and drops those that stopped.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_vortices(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    images: Res<FxImages>,
    mut events: MessageReader<GunEvent>,
    mut vortices: ResMut<Vortices>,
    mut rng: ResMut<FxRng>,
    mut places: Query<&mut Transform>,
    mut making: Making,
) {
    let eye = eye(&cameras);
    let mut drawn = Vec::new();
    for event in events.read() {
        let GunEventKind::Sucking { from, aim, reach } = event.kind else {
            continue;
        };
        let key = armory
            .get(event.weapon)
            .map_or("", |def| def.effects.as_str());
        let level = level(&heroes, event.player, event.weapon);
        let Some(vortex) = recipes.vortex(key, level) else {
            continue;
        };
        let id = (event.player, event.weapon.0);
        drawn.push(id);
        let shown = vortices
            .0
            .entry(id)
            .or_insert_with(|| Shown::new(from, aim));
        lay_rope(shown, from, aim, &mut rng);
        step_shows(&mut shown.shows, reach);
        shown.along = (shown.along + 0.001).rem_euclid(7.0);
        shown.around = (shown.around + 0.006 * (RINGS - 1) as f32).rem_euclid(7.0);
        let room = MOST_MOTES.saturating_sub(shown.motes.len());
        for _ in 0..room.min(MOTES_A_TICK) {
            if let Some(mote) = new_mote(shown, vortex, &mut rng) {
                shown.motes.push(mote);
            }
        }
        let (rope, rings) = (shown.rope, shown.rings);
        shown
            .motes
            .retain_mut(|mote| step_mote(mote, &rope, &rings));
        let mut meshes = vec![tube_mesh(shown, eye), arcs_mesh(shown, &mut rng)];
        meshes.extend(motes_meshes(shown, vortex, eye));
        if shown.parts.is_empty() {
            for (mesh, (texture, colour, blend)) in meshes.into_iter().zip(vortex.looks()) {
                let handle = making.meshes.add(mesh);
                let material = fx_material(&mut making.materials, &images, texture, colour, blend);
                // Its bounds change each tick.
                let entity = making
                    .commands
                    .spawn((
                        Name::new("vortex"),
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
    }
    vortices.0.retain(|id, shown| {
        let keep = drawn.contains(id);
        if !keep {
            for (entity, _) in &shown.parts {
                making.commands.entity(*entity).despawn();
            }
        }
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_show_one_by_one_as_it_reaches_out_and_back_where_it_meets_the_world() {
        let mut shows = [0; RINGS];
        for _ in 0..8 {
            step_shows(&mut shows, 10.0);
        }
        assert_eq!(shows[0], SHOWN);
        assert_eq!(shows[1], 0);
        for _ in 0..8 {
            step_shows(&mut shows, 10.0);
        }
        assert_eq!(shows[1], SHOWN);
        for _ in 0..100 {
            step_shows(&mut shows, 10.0);
        }
        assert!(shows[9] >= SHOWN);
        // A wall 3 u off puts out the rings past it, a little at a time.
        step_shows(&mut shows, 3.5);
        assert_eq!(shows[3], SHOWN - 4);
        assert_eq!(shows[2], SHOWN * 2 - 4);
        for _ in 0..8 {
            step_shows(&mut shows, 3.5);
        }
        assert_eq!(shows[9], 0);
    }

    #[test]
    fn the_rings_widen_away_from_the_gun() {
        assert!((ring_radius(1, 0.0) - (0.3 + ops::sin(0.7) * 0.1)).abs() < 0.001);
        assert!(ring_radius(9, 0.0) > ring_radius(4, 0.0));
        let (points, _) = ring_points(Vec3::NEG_Z, Vec3::Y, 2.0);
        for point in points {
            assert!((point.length() - 2.0).abs() < 0.001);
            assert!(point.z.abs() < 0.001);
        }
    }

    #[test]
    fn the_rope_lags_behind_a_turned_aim_then_catches_up() {
        let mut rng = FxRng::for_tick(1, 1);
        let mut shown = Shown::new(Vec3::ZERO, Vec3::NEG_Z);
        lay_rope(&mut shown, Vec3::ZERO, Vec3::X, &mut rng);
        let tip = (shown.rope[RINGS - 1] - shown.rope[RINGS - 2]).normalize();
        assert!(tip.dot(Vec3::X) < 0.5, "{tip}");
        for _ in 0..200 {
            lay_rope(&mut shown, Vec3::ZERO, Vec3::X, &mut rng);
        }
        let tip = (shown.rope[RINGS - 1] - shown.rope[0]).normalize();
        assert!(tip.dot(Vec3::X) > 0.98, "{tip}");
    }

    #[test]
    fn a_mote_flows_to_the_gun_and_ends_there() {
        let mut rng = FxRng::for_tick(1, 1);
        let mut shown = Shown::new(Vec3::ZERO, Vec3::NEG_Z);
        for _ in 0..200 {
            lay_rope(&mut shown, Vec3::ZERO, Vec3::NEG_Z, &mut rng);
            step_shows(&mut shown.shows, 10.0);
        }
        let vortex = Vortex {
            tube: Tex::Effect(21),
            tube_colour: [0.5; 3],
            arcs: Tex::Effect(29),
            arc_colour: [0.5, 0.5, 0.75, 0.25],
            smoke: Tex::Particle(0x17, 0),
            smoke_colour: [1.0; 3],
            dust: Vec::new(),
            dust_colour: [1.0; 3],
            dust_half: (0.1, 0.2),
            dust_blended: false,
        };
        let mut mote = new_mote(&shown, &vortex, &mut rng).expect("all rings show");
        assert_eq!(mote.ring, 9);
        let mut ticks = 0;
        while step_mote(&mut mote, &shown.rope, &shown.rings) {
            ticks += 1;
            assert!(ticks < 200, "it never reached the gun");
        }
        assert!(mote.ring == 0);
    }
}
