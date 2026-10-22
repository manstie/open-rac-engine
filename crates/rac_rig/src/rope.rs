//! The Plasma Whip's rope and the Shield Charger's shield, drawn round the hero
//! (`weapons.md`, "Plasma Whip" and "Shield Charger").
//!
//! - The rope: the whip's handle leads nine links, each piece's end trailing towards where
//!   it would lie, so it hangs back at rest and pulls taut in a swing, as UYA's
//!   `whip_rope_ends` (0x357678) moves them. It follows the hand the rig animates, so it is
//!   drawn here, not simulated: the kit's whip hits along a line of the rope's reach
//!   (`rac_combat`).
//! - The shield: a bubble 0.75 u above his feet while it is up, as UYA keeps its moby.
//!
//! Both are the kit's own placeholders in their looks: a glowing translucent segment for
//! each link (UYA's are classes 0x183A and 0x183C in each level's core) and a faceted
//! sphere for the shield (class 0xA5C).

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_hero::{HeroBody, HeroMotion, HeroState};

use crate::HeroRig;
use crate::hull::convex_hull;
use crate::shapes::beam;
use crate::wear::{Paint, Surface};

/// The pieces whose ends trail: the handle, then links 1–8. A ninth link, the tip, sits at
/// the last end.
const PIECES: usize = 9;
/// What each piece's end may come no nearer the ground than (`whip_rope_ends`), the floor
/// taken as level with his feet [simplified: UYA finds it with a line from 0.5 u above].
const ABOVE_GROUND: f32 = 0.15;
/// How far the ends move to where they would lie each tick: this plus the rope's second
/// value.
const PULL: f32 = 0.1;
/// In a combo swing, links 1 and 2 add the hand's travel since the last tick this many
/// times, so the first links trail the swing.
const TRAVEL: [f32; 2] = [4.0, 2.0];
/// The links hide this many ticks after the whip runs out of rounds.
const EMPTY_TICKS: u32 = 300;

/// Where the rope's stretch and second value head, and their steps a tick (`None`: at
/// once), by the hero's state (`weapons.md`, "Plasma Whip", its table).
fn goals(hero: &HeroMotion) -> [(f32, Option<f32>); 2] {
    let frame = hero.ticks;
    if hero.thrown.is_some() {
        return [(0.5, Some(1.0 / 30.0)), (0.9, Some(1.0 / 30.0))];
    }
    match hero.state {
        HeroState::Combo(_) if frame < 7 => [(0.25, None), (0.5, None)],
        HeroState::Combo(2) => [(0.6, Some(0.2)), (0.8, Some(0.3))],
        HeroState::Combo(_) => [(0.9, Some(0.2)), (0.9, Some(0.3))],
        HeroState::JumpAttack if (7..=20).contains(&frame) => [(0.7, Some(0.1)), (0.7, Some(0.1))],
        HeroState::JumpAttack if frame > 20 => [(0.8, None), (0.8, None)],
        _ => [(0.2, None), (0.0, None)],
    }
}

/// Towards `goal` by at most `step` (`approach`, 0x4450C0), or straight there.
fn approach(value: f32, (goal, step): (f32, Option<f32>)) -> f32 {
    match step {
        Some(step) => value + (goal - value).clamp(-step, step),
        None => goal,
    }
}

/// One hero's rope.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rope {
    stretch: f32,
    second: f32,
    /// Each piece's end, in the world.
    ends: [Vec3; PIECES],
    /// The hand as last ticked, and whether the rope has been laid out yet.
    hand: Vec3,
    laid: bool,
    /// Ticks since the whip last had a round.
    empty: u32,
    pending: f32,
}

impl Default for Rope {
    fn default() -> Self {
        Self {
            stretch: 0.2,
            second: 0.0,
            ends: [Vec3::ZERO; PIECES],
            hand: Vec3::ZERO,
            laid: false,
            empty: 0,
            pending: 0.0,
        }
    }
}

impl Rope {
    /// Moves on by `seconds` with the handle at `hand`, pointing `along`, and returns
    /// whether the links show.
    pub fn step(&mut self, hero: &HeroMotion, hand: Vec3, along: Vec3, seconds: f32) -> bool {
        let Some(whip) = hero.gun.whip.filter(|_| hero.gun.item.is_some()) else {
            self.laid = false;
            return false;
        };
        if !self.laid {
            self.laid = true;
            self.hand = hand;
            for (k, end) in self.ends.iter_mut().enumerate() {
                *end = hand + along * self.stretch * (k + 1) as f32;
            }
        }
        self.pending += seconds * 60.0;
        let whole = self.pending.floor();
        self.pending -= whole;
        for _ in 0..whole as u32 {
            self.empty = if whip.loaded { 0 } else { self.empty + 1 };
            self.tick(hero, hand, along);
        }
        self.empty < EMPTY_TICKS
    }

    fn tick(&mut self, hero: &HeroMotion, hand: Vec3, along: Vec3) {
        let [stretch, second] = goals(hero);
        self.stretch = approach(self.stretch, stretch);
        self.second = approach(self.second, second);
        let travel = hand - self.hand;
        self.hand = hand;
        let swinging = matches!(hero.state, HeroState::Combo(_));
        let floor = hero.position.y + ABOVE_GROUND;
        let pull = (PULL + self.second).min(1.0);
        let mut start = hand;
        let mut way = along;
        for k in 0..PIECES {
            if swinging {
                way = Vec3::new(way.x, 0.0, way.z).normalize_or(way);
            }
            let mut wanted = match hero.thrown {
                // Its end follows the thrown wrench.
                Some(thrown) => hand + (thrown.position - hand) * (k + 1) as f32 / PIECES as f32,
                None => start + way * self.stretch,
            };
            wanted.y = wanted.y.max(floor);
            let mut end = self.ends[k].lerp(wanted, pull);
            if swinging && let Some(times) = k.checked_sub(1).and_then(|i| TRAVEL.get(i)) {
                end += travel * *times;
            }
            self.ends[k] = end;
            way = (end - start).normalize_or(way);
            start = end;
        }
    }

    /// Each link from the handle out: where it starts and ends; the last is the tip.
    pub fn links(&self, hand: Vec3) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
        (0..PIECES).map(move |k| {
            let from = if k == 0 { hand } else { self.ends[k - 1] };
            (from, self.ends[k])
        })
    }
}

/// One drawn link of a hero's rope.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RopeLink;

/// A hero's shield bubble.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldBubble;

/// The rope's and the shield's looks, made once.
#[derive(Resource, Debug, Clone)]
pub(crate) struct RopeLooks {
    link: Handle<Mesh>,
    glow: Handle<StandardMaterial>,
    bubble: Handle<Mesh>,
    shield: Handle<StandardMaterial>,
}

/// How far the shield's middle is above his feet, and its size: UYA's zap ball (0.9 u).
const SHIELD_HEIGHT: f32 = 0.75;
const SHIELD_RADIUS: f32 = 0.9;

/// A link: a glowing bar a unit long along −Z, which each link stretches to its length.
fn link_mesh() -> Mesh {
    let mut surface = Surface::new(Paint::Own(Handle::default()), false);
    let points = beam(
        Vec3::ZERO,
        Vec3::NEG_Z,
        Vec3::X,
        Vec2::splat(0.03),
        Vec2::splat(0.03),
        0.0,
    );
    surface.push_rigid(&convex_hull(&points), 0);
    surface.mesh(false, Vec3::ZERO)
}

/// A faceted sphere: the hull of points spread evenly over it.
fn bubble_mesh(radius: f32) -> Mesh {
    const POINTS: usize = 160;
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    let points: Vec<Vec3> = (0..POINTS)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / POINTS as f32;
            let across = (1.0 - y * y).sqrt();
            let turn = golden * i as f32;
            Vec3::new(turn.cos() * across, y, turn.sin() * across) * radius
        })
        .collect();
    let mut surface = Surface::new(Paint::Own(Handle::default()), false);
    surface.push_rigid(&convex_hull(&points), 0);
    surface.mesh(false, Vec3::ZERO)
}

pub(crate) fn register_rope_looks(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    // The handle's tint, (0x40, 0x10, 0x80), at the links' alpha, 0x3C [code].
    let purple = Color::srgba(0.5, 0.125, 1.0, 0x3C as f32 / 128.0);
    commands.insert_resource(RopeLooks {
        link: meshes.add(link_mesh()),
        glow: materials.add(StandardMaterial {
            base_color: purple,
            emissive: purple.to_linear() * 4.0,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        bubble: meshes.add(bubble_mesh(SHIELD_RADIUS)),
        shield: materials.add(StandardMaterial {
            base_color: Color::srgba(0.45, 0.8, 1.0, 0.25),
            emissive: LinearRgba::rgb(0.2, 0.5, 0.9),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

/// Moves each hero's rope on and draws its links in the world, and shows his shield.
pub(crate) fn draw_ropes(
    time: Res<Time>,
    looks: Option<Res<RopeLooks>>,
    heroes: Query<(&HeroBody, &Transform)>,
    mut rigs: Query<(Entity, &mut HeroRig)>,
    mut parts: Query<
        (&mut Transform, &mut Visibility),
        (Or<(With<RopeLink>, With<ShieldBubble>)>, Without<HeroBody>),
    >,
    mut commands: Commands,
) {
    let Some(looks) = looks else {
        return;
    };
    let seconds = time.delta_secs();
    for (root, mut rig) in &mut rigs {
        let Ok((body, placed)) = heroes.get(rig.hero) else {
            continue;
        };
        let hero = &body.0;
        let rig = &mut *rig;
        let grip = placed.mul_transform(rig.grip);
        let (hand, along) = (grip.translation, grip.rotation * Vec3::NEG_Z);
        let shown = rig.rope.step(hero, hand, along, seconds);
        if rig.links.is_empty() && shown {
            rig.links = (0..PIECES)
                .map(|_| {
                    commands
                        .spawn((
                            Name::new("whip link"),
                            RopeLink,
                            NotShadowCaster,
                            Mesh3d(looks.link.clone()),
                            MeshMaterial3d(looks.glow.clone()),
                            Transform::default(),
                            Visibility::Hidden,
                        ))
                        .id()
                })
                .collect();
        }
        for (&link, (from, to)) in rig.links.iter().zip(rig.rope.links(hand)) {
            let Ok((mut transform, mut visibility)) = parts.get_mut(link) else {
                continue;
            };
            visibility.set_if_neq(if shown {
                Visibility::Visible
            } else {
                Visibility::Hidden
            });
            let length = from.distance(to);
            *transform = Transform::from_translation(from)
                .looking_at(to, Vec3::Y)
                .with_scale(Vec3::new(1.0, 1.0, length.max(1e-3)));
        }

        let up = hero.shield.is_up();
        if rig.bubble.is_none() && up {
            let bubble = commands
                .spawn((
                    Name::new("shield"),
                    ShieldBubble,
                    NotShadowCaster,
                    Mesh3d(looks.bubble.clone()),
                    MeshMaterial3d(looks.shield.clone()),
                    Transform::from_xyz(0.0, SHIELD_HEIGHT, 0.0),
                    Visibility::Hidden,
                ))
                .id();
            commands.entity(root).add_child(bubble);
            rig.bubble = Some(bubble);
        }
        if let Some(bubble) = rig.bubble
            && let Ok((_, mut visibility)) = parts.get_mut(bubble)
        {
            visibility.set_if_neq(if up {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::{GunHand, WhipHand};

    fn with_whip(state: HeroState, ticks: u32) -> HeroMotion {
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.state = state;
        hero.ticks = ticks;
        hero.gun = GunHand {
            out: true,
            item: Some(0x7F),
            whip: Some(WhipHand {
                loaded: true,
                ..default()
            }),
            ..default()
        };
        hero
    }

    /// How far the rope's last end lies from the hand after `ticks` in `state`.
    fn reach(state: HeroState, ticks: u32) -> f32 {
        let hand = Vec3::new(0.0, 1.0, 0.0);
        let mut rope = Rope::default();
        for tick in 0..ticks {
            rope.step(&with_whip(state, tick), hand, Vec3::NEG_Z, 1.0 / 60.0);
        }
        rope.ends[PIECES - 1].distance(hand)
    }

    #[test]
    fn the_rope_hangs_short_at_rest_and_pulls_out_about_nine_stretches_in_a_swing() {
        // At rest a stretch of 0.2: under 2 u laid straight.
        let rest = reach(HeroState::Idle, 120);
        assert!(rest < 9.0 * 0.2 + 1e-3, "{rest}");
        // Swings 1 and 2 pull out to about 9 × 0.9, swing 3 to 9 × 0.6.
        let swing = reach(HeroState::Combo(0), 120);
        assert!((swing - 8.1).abs() < 0.3, "{swing}");
        let third = reach(HeroState::Combo(2), 120);
        assert!((third - 5.4).abs() < 0.3, "{third}");
    }

    #[test]
    fn the_rope_keeps_off_the_ground() {
        let hand = Vec3::new(0.0, 0.5, 0.0);
        let mut rope = Rope::default();
        for tick in 0..120 {
            rope.step(
                &with_whip(HeroState::Idle, tick),
                hand,
                Vec3::NEG_Y,
                1.0 / 60.0,
            );
        }
        assert!(rope.ends.iter().all(|end| end.y >= ABOVE_GROUND - 1e-5));
    }

    #[test]
    fn the_links_hide_without_the_whip_and_long_after_its_rounds_run_out() {
        let mut rope = Rope::default();
        let mut hero = with_whip(HeroState::Idle, 0);
        assert!(rope.step(&hero, Vec3::Y, Vec3::NEG_Z, 1.0 / 60.0));
        hero.gun.whip = hero.gun.whip.map(|whip| WhipHand {
            loaded: false,
            ..whip
        });
        for _ in 0..EMPTY_TICKS - 2 {
            assert!(rope.step(&hero, Vec3::Y, Vec3::NEG_Z, 1.0 / 60.0));
        }
        rope.step(&hero, Vec3::Y, Vec3::NEG_Z, 1.0 / 60.0);
        assert!(!rope.step(&hero, Vec3::Y, Vec3::NEG_Z, 1.0 / 60.0));
        hero.gun = GunHand::default();
        assert!(!rope.step(&hero, Vec3::Y, Vec3::NEG_Z, 1.0 / 60.0));
    }
}
