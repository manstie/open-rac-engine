//! The Plasma Whip's rope, drawn from the hero's hand (`weapons.md`, "Plasma Whip").
//!
//! - The rope: the whip's handle leads nine links, each piece's end trailing towards where
//!   it would lie, so it hangs back at rest and pulls taut in a swing, as UYA's
//!   `whip_rope_ends` (0x357678) moves them. It follows the hand the rig animates, so it is
//!   drawn here, not simulated: the kit's whip hits along a line of the rope's reach
//!   (`rac_combat`).
//!
//! The links are a game's models where it gives them ([`RopeModels`]: UYA's classes 0x183A
//! and 0x183C), translucent and glowing in the whip's colour; else glowing translucent bars
//! from link to link. The Shield Charger's shield is an effect (`rac_demo::fx`), as UYA
//! draws it.

use std::f32::consts::FRAC_PI_2;

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
/// The rope's points: the hand, then each piece's end.
pub const ROPE_POINTS: usize = PIECES + 1;
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
/// How far along each link the next piece starts: its joint 0, which class 0x183A has
/// 0.971 u out at its class's scale, drawn at 0.2 of that (0x3572B0) [data]. Each piece's
/// end is its own joint 0 plus the stretch (`whip_rope_ends`), so the rope runs nine
/// stretches and eight of these.
pub const LINK_LEAD: f32 = 0.194;
/// The links' opacity: 0x3C of 0x80 (0x3572B0).
const LINK_ALPHA: f32 = 0x3C as f32 / 128.0;

/// The links' glow by the whip's level (0x356110 each frame, `moby_set_colour`, 0x80
/// being 1.0): (0xBF, 0x40, 0x20) at V1–V2 and (0x20, 0x40, 0xBF) at V3–V4. From V5 the
/// game sets none and clears the flag its spawn set below V5, so they show plainly.
fn link_glow(level: u8) -> Option<[f32; 3]> {
    let glow = |[r, g, b]: [u8; 3]| Some([r, g, b].map(|byte| byte as f32 / 128.0));
    match level {
        0..=1 => glow([0xBF, 0x40, 0x20]),
        2..=3 => glow([0x20, 0x40, 0xBF]),
        _ => None,
    }
}

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
    /// Each piece's end, in the world, and which way the piece runs to it: the way the
    /// link at its end lies.
    ends: [Vec3; PIECES],
    ways: [Vec3; PIECES],
    /// The links lie flat, as in a combo swing.
    flat: bool,
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
            ways: [Vec3::NEG_Z; PIECES],
            flat: false,
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
            for k in 0..PIECES {
                let pieces = (k + 1) as f32;
                self.ends[k] = hand + along * (self.stretch * pieces + LINK_LEAD * k as f32);
                self.ways[k] = along;
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
        let lie = |way: Vec3| {
            if swinging {
                Vec3::new(way.x, 0.0, way.z).normalize_or(way)
            } else {
                way
            }
        };
        self.flat = swinging;
        let mut start = hand;
        let mut way = lie(along);
        for k in 0..PIECES {
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
            // The link at its end lies away from where the piece started, and the next
            // piece starts at that link's joint 0.
            way = lie((end - start).normalize_or(way));
            self.ways[k] = way;
            start = end + way * LINK_LEAD;
        }
    }

    /// Where each link's model sits: at the end of the piece before it, its length (the
    /// game's +x, the kit's −Z) along that piece, stood on its edge by a roll of 90° but
    /// lying flat in a combo (`whip_link_place` 0x357778).
    fn places(&self) -> impl Iterator<Item = Transform> + '_ {
        (0..PIECES).map(move |k| {
            let place = Transform::from_translation(self.ends[k]).looking_to(self.ways[k], Vec3::Y);
            if self.flat {
                place
            } else {
                place.with_rotation(place.rotation * Quat::from_rotation_z(FRAC_PI_2))
            }
        })
    }

    /// Each link from the handle out: where it starts and ends; the last is the tip.
    pub fn links(&self, hand: Vec3) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
        (0..PIECES).map(move |k| {
            let from = if k == 0 { hand } else { self.ends[k - 1] };
            (from, self.ends[k])
        })
    }

    /// The hand, then each piece's end.
    fn points(&self, hand: Vec3) -> [Vec3; ROPE_POINTS] {
        std::array::from_fn(|k| if k == 0 { hand } else { self.ends[k - 1] })
    }
}

/// Where a hero's rope lies as last drawn, for effects drawn along it (UYA's strands and
/// sparks, `effects.md`, "Plasma Whip").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RopeShape {
    /// The hand, then each piece's end; the last is the tip.
    pub points: [Vec3; ROPE_POINTS],
    /// He is swinging (UYA's states 0x13 and 0x14, the combo and the jump attack) or the
    /// whip's end is thrown, which thins its strands.
    pub attacking: bool,
}

/// One drawn link of a hero's rope.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RopeLink;

/// A game's model of one of the rope's links, in the link's own space: its length along
/// −Z from where it sits, at the size it is drawn; each part a mesh and its image.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RopeModel {
    pub parts: Vec<(Handle<Mesh>, Option<Handle<Image>>)>,
}

/// The models a game gives the rope's links: eight of `link` and the `tip` (UYA's classes
/// 0x183A and 0x183C). Without a link model the kit draws its own bars.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct RopeModels {
    pub link: Option<RopeModel>,
    pub tip: Option<RopeModel>,
}

/// What a hero's links were made as: from a game's models or as the kit's bars, and their
/// glow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinkLook {
    models: bool,
    glow: Option<[f32; 3]>,
}

/// A link's material: its image, see-through at the game's opacity, lit by its glow alone
/// while it has one.
fn link_material(image: Option<Handle<Image>>, glow: Option<[f32; 3]>) -> StandardMaterial {
    let [r, g, b] = glow.unwrap_or([1.0; 3]);
    StandardMaterial {
        base_color: Color::linear_rgba(r, g, b, LINK_ALPHA),
        base_color_texture: image,
        alpha_mode: AlphaMode::Blend,
        unlit: glow.is_some(),
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

/// The kit's link look, made once.
#[derive(Resource, Debug, Clone)]
pub(crate) struct RopeLooks {
    link: Handle<Mesh>,
    glow: Handle<StandardMaterial>,
}

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

/// A hero's links from a game's models: eight of `link` and the tip, each part in its own
/// material, hidden until placed.
fn spawn_model_links(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    link: &RopeModel,
    tip: Option<&RopeModel>,
    look: LinkLook,
) -> Vec<Entity> {
    (0..PIECES)
        .map(|k| {
            let model = if k == PIECES - 1 {
                tip.unwrap_or(link)
            } else {
                link
            };
            let parts: Vec<_> = model
                .parts
                .iter()
                .map(|(mesh, image)| {
                    let material = materials.add(link_material(image.clone(), look.glow));
                    (
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        NotShadowCaster,
                    )
                })
                .collect();
            let mut entity = commands.spawn((
                Name::new("whip link"),
                RopeLink,
                Transform::default(),
                Visibility::Hidden,
            ));
            entity.with_children(|children| {
                for part in parts {
                    children.spawn(part);
                }
            });
            entity.id()
        })
        .collect()
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
    });
}

/// Moves each hero's rope on and draws its links in the world.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_ropes(
    time: Res<Time>,
    looks: Option<Res<RopeLooks>>,
    models: Res<RopeModels>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    heroes: Query<(&HeroBody, &Transform)>,
    mut rigs: Query<&mut HeroRig>,
    mut parts: Query<(&mut Transform, &mut Visibility), (With<RopeLink>, Without<HeroBody>)>,
    mut commands: Commands,
) {
    let Some(looks) = looks else {
        return;
    };
    let seconds = time.delta_secs();
    for mut rig in &mut rigs {
        let Ok((body, placed)) = heroes.get(rig.hero) else {
            continue;
        };
        let hero = &body.0;
        let rig = &mut *rig;
        let grip = placed.mul_transform(rig.grip);
        let (hand, along) = (grip.translation, grip.rotation * Vec3::NEG_Z);
        let shown = rig.rope.step(hero, hand, along, seconds);
        rig.rope_shape = shown.then(|| RopeShape {
            points: rig.rope.points(hand),
            attacking: hero.thrown.is_some()
                || matches!(hero.state, HeroState::Combo(_) | HeroState::JumpAttack),
        });
        let look = LinkLook {
            models: models.link.is_some(),
            glow: link_glow(hero.gun.level),
        };
        if rig.link_look.is_some_and(|made| made != look) {
            for link in rig.links.drain(..) {
                commands.entity(link).despawn();
            }
        }
        if rig.links.is_empty() && shown {
            rig.link_look = Some(look);
            rig.links = match (&models.link, materials.as_deref_mut()) {
                (Some(link), Some(materials)) => {
                    spawn_model_links(&mut commands, materials, link, models.tip.as_ref(), look)
                }
                _ => (0..PIECES)
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
                    .collect(),
            };
        }
        let bars = rig.rope.links(hand).map(|(from, to)| {
            Transform::from_translation(from)
                .looking_at(to, Vec3::Y)
                .with_scale(Vec3::new(1.0, 1.0, from.distance(to).max(1e-3)))
        });
        let places: Vec<Transform> = if look.models {
            rig.rope.places().collect()
        } else {
            bars.collect()
        };
        for (&link, place) in rig.links.iter().zip(places) {
            let Ok((mut transform, mut visibility)) = parts.get_mut(link) else {
                continue;
            };
            visibility.set_if_neq(if shown {
                Visibility::Visible
            } else {
                Visibility::Hidden
            });
            *transform = place;
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
        // Nine stretches and the eight links between them: at rest a stretch of 0.2, so
        // under 3.4 u laid straight.
        let links = 8.0 * LINK_LEAD;
        let rest = reach(HeroState::Idle, 120);
        assert!(rest < 9.0 * 0.2 + links + 1e-3, "{rest}");
        // Swings 1 and 2 pull out to about 9 × 0.9 and the links, swing 3 to 9 × 0.6.
        let swing = reach(HeroState::Combo(0), 120);
        assert!((swing - (8.1 + links)).abs() < 0.3, "{swing}");
        let third = reach(HeroState::Combo(2), 120);
        assert!((third - (5.4 + links)).abs() < 0.3, "{third}");
    }

    #[test]
    fn each_piece_starts_a_links_lead_past_the_last_ones_end() {
        let hand = Vec3::new(0.0, 1.0, 0.0);
        let mut rope = Rope::default();
        for tick in 0..240 {
            rope.step(
                &with_whip(HeroState::Combo(0), tick),
                hand,
                Vec3::NEG_Z,
                1.0 / 60.0,
            );
        }
        // Laid straight out along −Z, each end the stretch past the last link's joint 0.
        for pair in rope.ends.windows(2) {
            let apart = pair[0].distance(pair[1]);
            assert!((apart - (rope.stretch + LINK_LEAD)).abs() < 1e-3, "{apart}");
        }
    }

    #[test]
    fn a_link_lies_along_its_piece_on_its_edge_but_flat_in_a_swing() {
        let hand = Vec3::new(0.0, 1.0, 0.0);
        for (state, flat) in [(HeroState::Idle, false), (HeroState::Combo(0), true)] {
            let mut rope = Rope::default();
            for tick in 0..120 {
                rope.step(&with_whip(state, tick), hand, Vec3::NEG_Z, 1.0 / 60.0);
            }
            for (k, place) in rope.places().enumerate() {
                assert_eq!(place.translation, rope.ends[k]);
                assert!((place.forward().as_vec3() - rope.ways[k]).length() < 1e-4);
                // Its width (the game's y, the kit's x) lies level, or stands up.
                let across = place.rotation * Vec3::X;
                assert_eq!(across.y.abs() > 0.99, !flat, "{across}");
            }
        }
    }

    #[test]
    fn the_links_glow_red_then_blue_by_level_and_plainly_from_v5() {
        assert_eq!(link_glow(0), Some([0xBF as f32 / 128.0, 0.5, 0.25]));
        assert_eq!(link_glow(3), Some([0.25, 0.5, 0xBF as f32 / 128.0]));
        assert_eq!(link_glow(4), None);
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
