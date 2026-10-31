//! The guns' effects (PLAN.md P3.18): flames at the muzzle, sprites on shots in flight,
//! beams for rays and arcs, and glows and bursts of particles where shots land, drawn as
//! the game draws its own, as camera-facing sprites in its effect and particle textures
//! (`docs/research/effects.md`).
//!
//! - Each gun's are a recipe in `assets/effects.ron` ([`FxRecipes`]), by its effects key.
//! - Textures are named by the game's numbers ([`Tex`]): an effect texture, or an entry of
//!   a particle type's list. The kit paints a placeholder for each the recipes name
//!   ([`FxImages`]); a game puts its own in their place, as the app does from the user's
//!   disc.
//! - Sprites and particles move at the simulation's 60 Hz, as the game's do frame by
//!   frame, and are drawn between ticks. Their randomness is their own, never the level's:
//!   they don't touch the simulation.

mod charge;
mod fan;
mod funnel;
mod light;
mod mark;
mod marker;
mod model;
mod open;
mod paint;
mod panel;
mod ribbon;
mod rope;
mod shake;
mod shell;
mod sweep;

use std::collections::BTreeMap;

use bevy::ecs::system::SystemParam;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::pickups::CrateBlast;
#[cfg(doc)]
use rac_combat::weapons::ShotKind;
use rac_combat::{Armory, Arsenal, GunEvent, GunEventKind, WeaponId};
use rac_hero::{CollisionScene, CollisionWorld, Hero};
use rac_sim::SimTransform;
use serde::Deserialize;

pub use charge::{Charge, Crackle, Reach};
pub use fan::Fan;
pub use funnel::{Funnel, SpiralStrip, Spirals};
pub use light::ShotLight;
pub use mark::{Flame, Flat, Mark};
pub use marker::Marker;
pub use model::{FxModel, FxModels};
pub use open::{Open, OpenRays};
pub use paint::{Paint, Shape};
pub use panel::{Panel, Panels};
pub use ribbon::Ribbon;
pub use rope::{AlongRope, Strand};
pub use shake::{CameraShake, Shaking};
pub use shell::{Arcs, Dome, Rim, Shell, Trails};
pub use sweep::Sweep;

use crate::DemoEntity;

/// The guns' effects: the recipes, and what the placeholders look like.
pub const EFFECTS_RON: &str = include_str!("../../assets/effects.ron");

/// A texture by the game's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum Tex {
    /// An effect texture (0–99 the engine's own, in every level).
    Effect(u16),
    /// Entry `n` of particle type `t`'s list, which differs from level to level.
    Particle(u8, u8),
}

/// How a sprite meets what's behind it (the game's blends 1 and 0; its brightening blend
/// is drawn as `Add`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Blend {
    #[default]
    Add,
    Alpha,
}

impl Blend {
    pub fn alpha_mode(self) -> AlphaMode {
        match self {
            Blend::Add => AlphaMode::Add,
            Blend::Alpha => AlphaMode::Blend,
        }
    }
}

fn opaque() -> [f32; 4] {
    [1.0; 4]
}

fn whole() -> f32 {
    1.0
}

/// Where a burst's particles head.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Toward {
    /// Round the shot's way (all round where an event has none).
    #[default]
    Way,
    /// Round the way to the camera, as hits throw their sparks back at the eye.
    Camera,
    /// Level, any way round, as a ring along the ground.
    Level,
}

/// One thing an event shows. Sizes are in world units, times in 60 Hz ticks, colours sRGB
/// with alpha (1 shows the texture as it is), angles in degrees.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Emit {
    /// A sprite along the shot's way from the muzzle, `length` by `width`: a flame.
    Flash {
        texture: Tex,
        length: f32,
        width: f32,
        ticks: u32,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        blend: Blend,
    },
    /// A sprite facing the camera, `size` across growing to `end_size` as its alpha goes to
    /// `end_alpha`, turned by an angle within `roll` and turning `spin` a tick, and going
    /// `ahead` units a second along the shot's way; shown `delay` ticks after the event,
    /// `nearer` units toward the camera (as the game keeps a spark off the wall it is on).
    Glow {
        texture: Tex,
        size: f32,
        end_size: f32,
        ticks: u32,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        end_alpha: f32,
        #[serde(default)]
        roll: Option<(f32, f32)>,
        #[serde(default)]
        spin: f32,
        #[serde(default)]
        ahead: f32,
        #[serde(default)]
        delay: u32,
        #[serde(default)]
        nearer: f32,
        #[serde(default)]
        blend: Blend,
    },
    /// `count` particles thrown out at up to `speed` units a second (from `speed_from`, or
    /// 0.4 of it), all round or within `cone` degrees of the way they head, and rising at
    /// between the two of `rise` on top; falling at `gravity` and keeping `keep` of their
    /// speed each tick. Each is `size` across (or
    /// between it and `size_to`), changing to `end_size` in proportion while its alpha goes
    /// to `end_alpha` and its colour to `end_colour` over `ticks` (to `ticks_to`, or give
    /// or take a third). They start at any roll and turn up to `spin` a tick either way.
    /// They start spread over `line` back along the shot's way, and up to `jitter` from
    /// where they would; with `ground`, on the floor within that many units below where
    /// they would, as the game puts its gas clouds, or not at all where there's none. With
    /// a `streak` length each is drawn that long along its way, `size` across. Each shows
    /// `delay` ticks after the event, plus up to `over` more.
    Burst {
        texture: Tex,
        count: u32,
        speed: f32,
        #[serde(default)]
        speed_from: Option<f32>,
        #[serde(default)]
        cone: Option<f32>,
        #[serde(default)]
        toward: Toward,
        #[serde(default)]
        rise: (f32, f32),
        #[serde(default)]
        gravity: f32,
        #[serde(default = "whole")]
        keep: f32,
        size: f32,
        #[serde(default)]
        size_to: Option<f32>,
        end_size: f32,
        ticks: u32,
        #[serde(default)]
        ticks_to: Option<u32>,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        end_alpha: f32,
        #[serde(default)]
        end_colour: Option<[f32; 3]>,
        #[serde(default)]
        spin: f32,
        #[serde(default)]
        line: f32,
        #[serde(default)]
        jitter: f32,
        #[serde(default)]
        ground: Option<f32>,
        #[serde(default)]
        streak: Option<f32>,
        #[serde(default)]
        delay: u32,
        #[serde(default)]
        over: u32,
        #[serde(default)]
        blend: Blend,
    },
    /// The whole view brightened by `colour`, added as the game's full-screen flashes are,
    /// for `ticks` as its alpha goes to `end_alpha`; shown `delay` ticks after the event.
    /// The colour is the game's bytes over 0x80 as elsewhere, though with no texture
    /// under it 0x80 adds half of full white.
    Screen {
        ticks: u32,
        colour: [f32; 4],
        #[serde(default)]
        end_alpha: f32,
        #[serde(default)]
        delay: u32,
    },
    /// A model of the game's class `class` (a game gives it, [`FxModels`]; else the
    /// kit's crown), standing level at any yaw, `scale` times its size (between the two)
    /// and growing `grow` times a tick, as its alpha goes to `end_alpha` over `ticks`: the
    /// ring 0x307 the Infector's burst and the Plasma Whip's wave leave.
    Model {
        class: u32,
        scale: (f32, f32),
        grow: f32,
        ticks: u32,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        end_alpha: f32,
        #[serde(default)]
        blend: Blend,
    },
    /// A point light of `colour` reaching `radius`, `ahead` along the shot's way and
    /// `above`, for `ticks`: climbing from nothing by `rise` a tick to its colour if
    /// given, then falling by `fall` a tick, or evenly over its ticks if not given, as
    /// the light moby's channels do (0x300820). 1.0 is the game's 0x80. Only the game's
    /// eight are made at once.
    Light {
        radius: f32,
        colour: [f32; 3],
        ticks: u32,
        #[serde(default)]
        ahead: f32,
        #[serde(default)]
        above: f32,
        #[serde(default)]
        rise: Option<[f32; 3]>,
        #[serde(default)]
        fall: Option<[f32; 3]>,
    },
    /// The camera shaken by `amplitude` for `ticks` along each of `along`, less `lose` for
    /// each unit the event is from the camera; from `within` of it, by `beyond` if given
    /// (which can stop a shake: the Nitro's 0), else left as it is.
    Shake {
        amplitude: f32,
        ticks: u32,
        #[serde(default = "up")]
        along: Vec<Shaking>,
        #[serde(default)]
        lose: f32,
        #[serde(default)]
        within: Option<f32>,
        #[serde(default)]
        beyond: Option<f32>,
    },
    /// The emits of one of [`FxRecipes::sets`].
    Set(String),
}

fn up() -> Vec<Shaking> {
    vec![Shaking::Up]
}

impl Emit {
    fn texture(&self) -> Option<Tex> {
        match self {
            Emit::Flash { texture, .. }
            | Emit::Glow { texture, .. }
            | Emit::Burst { texture, .. } => Some(*texture),
            Emit::Screen { .. }
            | Emit::Model { .. }
            | Emit::Light { .. }
            | Emit::Shake { .. }
            | Emit::Set(_) => None,
        }
    }
}

/// A beam from one point to another, `width` across, fading over `ticks` as it narrows
/// to `end_width` (to nothing if not given); bent, with a `jag`, through points aside.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Beam {
    pub texture: Tex,
    pub width: f32,
    #[serde(default)]
    pub end_width: Option<f32>,
    pub ticks: u32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
    #[serde(default)]
    pub jag: Option<Jag>,
    /// Laid through the points of the beam before it in its list, as a glow round a core.
    #[serde(default)]
    pub same_bends: bool,
}

/// A beam's bends, as the lightning bolt's (0x34DFD0): `points` points along it, ends
/// included, each between them lifted `wave` × the sine of its angle, moved level across
/// the beam `around` × its cosine, and lifted up to `jitter` more either way; the angles
/// go up `turn` degrees a point from one at random. With `bows`, the points lie on the
/// Plasma Coil's curve (0x30D500) instead: at t along the way, less w1 t(1 − t)² and
/// w2 t²(1 − t), each axis of w1 and w2 up to between the two of its range either way.
/// Drawn afresh each time the beam is.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Jag {
    pub points: u32,
    #[serde(default)]
    pub wave: f32,
    #[serde(default)]
    pub around: f32,
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub jitter: f32,
    #[serde(default)]
    pub bows: Option<((f32, f32), (f32, f32))>,
}

impl Jag {
    /// The points a beam from `from` to `to` bends through, both ends included.
    pub(crate) fn points(&self, from: Vec3, to: Vec3, rng: &mut FxRng) -> Vec<Vec3> {
        let way = to - from;
        // As the bolt's draw (0x34E688) lays them: across is level, up square to both.
        let across = way.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
        let up = across.cross(way.normalize_or_zero());
        let start = rng.between((0.0, std::f32::consts::TAU));
        // Each axis a wavering amplitude × the sine of its own phase, as 0x30C5D8 sets.
        let mut bow =
            |range| {
                Vec3::from_array([0; 3].map(|_| {
                    rng.between(range) * ops::sin(rng.between((0.0, std::f32::consts::TAU)))
                }))
            };
        let bows = self.bows.map(|(first, second)| (bow(first), bow(second)));
        let last = self.points.max(2) - 1;
        (0..=last)
            .map(|point| {
                if point == 0 || point == last {
                    return if point == 0 { from } else { to };
                }
                let t = point as f32 / last as f32;
                let angle = start + (self.turn * point as f32).to_radians();
                let lift = self.wave * ops::sin(angle) + rng.between((-self.jitter, self.jitter));
                let bent = bows.map_or(Vec3::ZERO, |(first, second)| {
                    -first * t * (1.0 - t) * (1.0 - t) - second * t * t * (1.0 - t)
                });
                from + way * t + bent + up * lift + across * (self.around * ops::cos(angle))
            })
            .collect()
    }
}

/// One of the sprites drawn on each of a gun's shots in flight, in place of the kit's
/// ball: turning `spin` degrees a tick, or to a new roll each tick if it `flickers`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShotSprite {
    pub texture: Tex,
    pub size: f32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub spin: f32,
    #[serde(default)]
    pub flickers: bool,
    /// How much narrower it gets each tick, starting over at `size` before it is gone.
    #[serde(default)]
    pub shrink: f32,
    #[serde(default)]
    pub blend: Blend,
}

/// A sprite laid along a shot's way once it has flown `after` units: from `behind` it
/// back, `length` long and `width` across, each picked afresh every tick between the two.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Streak {
    pub texture: Tex,
    pub behind: f32,
    pub length: (f32, f32),
    pub width: (f32, f32),
    #[serde(default)]
    pub after: f32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// Particles a shot leaves along its way, until it is `until` ticks old if that is given:
/// `per_tick` a tick (a fraction being a chance) and one for every `spacing` units of the
/// tick's path, spread along it. Each is in one of `textures` and
/// `colours`, growing from `size` to `end_size` as its alpha goes to `end_alpha` and its
/// colour to `end_colour` over `ticks` to `ticks_to`. It sets off rising at `rise` units
/// a second, going back at `back` times the shot's speed and up to `scatter` units a
/// second either way along each axis; then falls at `gravity`, keeps `keep` of its speed
/// each tick and turns up to `spin` degrees a tick either way. With `ground`, each is laid
/// on the floor within that many units below, or not at all where there's none. With a
/// `shell`, it starts that far out from the shot any way round (or round it, if `level`),
/// heading in to it at between the two of `drawn_in` units a second.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Trail {
    #[serde(default)]
    pub per_tick: f32,
    #[serde(default)]
    pub spacing: f32,
    #[serde(default)]
    pub until: Option<u32>,
    pub textures: Vec<Tex>,
    pub colours: Vec<[f32; 4]>,
    pub size: f32,
    pub end_size: f32,
    pub ticks: u32,
    pub ticks_to: u32,
    #[serde(default)]
    pub rise: (f32, f32),
    #[serde(default)]
    pub back: f32,
    #[serde(default)]
    pub scatter: f32,
    #[serde(default)]
    pub gravity: f32,
    #[serde(default = "whole")]
    pub keep: f32,
    #[serde(default)]
    pub spin: f32,
    #[serde(default)]
    pub end_alpha: f32,
    #[serde(default)]
    pub end_colour: Option<[f32; 3]>,
    #[serde(default)]
    pub ground: Option<f32>,
    #[serde(default)]
    pub shell: Option<(f32, f32)>,
    #[serde(default)]
    pub level: bool,
    #[serde(default)]
    pub drawn_in: (f32, f32),
    #[serde(default)]
    pub blend: Blend,
}

impl Trail {
    /// How many it lays over a tick's path `length` long, `roll` (0 to 1) deciding the
    /// fraction of `per_tick`.
    fn count(&self, length: f32, roll: f32) -> u32 {
        let spaced = if self.spacing > 0.0 {
            (length / self.spacing) as u32
        } else {
            0
        };
        self.per_tick as u32 + u32::from(roll < self.per_tick.fract()) + spaced
    }
}

/// Rays fanned round a point, one laid each tick, until the shot is `until` ticks old if
/// that is given (the Spitting Hydra's glare, 0x322B80 through 0x46FE70, type 0x6F):
/// each its head a random `near` from the point any way round, reaching `length` straight
/// out from it and `width` across, creeping back toward it `creep` units a tick, in
/// `colour` throughout its `ticks`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Glare {
    pub texture: Tex,
    pub near: (f32, f32),
    pub length: f32,
    pub width: f32,
    #[serde(default)]
    pub creep: f32,
    pub ticks: u32,
    #[serde(default)]
    pub until: Option<u32>,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// A gun's effects; what it leaves out it takes from the defaults. From each level in
/// `from_level` (0 is V1), what that level's entry gives takes the place of this.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
pub struct GunFx {
    #[serde(default)]
    pub fire: Option<Vec<Emit>>,
    #[serde(default)]
    pub shot: Option<Vec<ShotSprite>>,
    #[serde(default)]
    pub streak: Option<Streak>,
    #[serde(default)]
    pub trail: Option<Vec<Trail>>,
    /// Rays round each shot of the kind each tick, as the Spitting Hydra's spit's glare.
    #[serde(default)]
    pub glare: Option<Glare>,
    /// Beams laid along each tick of a shot's way, as the Flux Rifle's.
    #[serde(default)]
    pub wake: Option<Vec<Beam>>,
    /// Ribbons through the gun's shots of a kind in flight, as the Lava Gun's stream.
    #[serde(default)]
    pub ribbon: Option<Vec<Ribbon>>,
    /// Sprites in the plane of a shot of the kind once it stands, as the Holo Shield's
    /// wall, in place of the kit's.
    #[serde(default)]
    pub panels: Option<Panels>,
    /// What a shot of the kind shows once it stands open, as a rift.
    #[serde(default)]
    pub open: Option<Open>,
    /// The funnel a gun's rays are drawn as, as the Qwack-O-Ray's.
    #[serde(default)]
    pub funnel: Option<Funnel>,
    /// The mark where a thrown shot would land.
    #[serde(default)]
    pub marker: Option<Marker>,
    /// Flat ribbons through where each shot has been, as the Disc Blade's.
    #[serde(default)]
    pub sweep: Option<Vec<Sweep>>,
    /// Strands from a shot of the kind to each of its ends, as the Spitting Hydra's spit.
    #[serde(default)]
    pub fan: Option<Fan>,
    /// What shows at the muzzle while the gun charges, as the Spitting Hydra's.
    #[serde(default)]
    pub charge: Option<Charge>,
    /// Strands through the Plasma Whip's rope while its links show.
    #[serde(default)]
    pub rope: Option<Vec<Strand>>,
    /// Particles laid along the Plasma Whip's rope each tick.
    #[serde(default)]
    pub along_rope: Option<Vec<AlongRope>>,
    #[serde(default)]
    pub impact: Option<Vec<Emit>>,
    /// What a shot leaves on the world where it ends there, as the Lava Gun's pools.
    #[serde(default)]
    pub mark: Option<Mark>,
    /// A light riding each shot of the kind, as the Holo Shield's in flight.
    #[serde(default)]
    pub light: Option<ShotLight>,
    /// The shell round him while the gun's shield is up: the Shield Charger's.
    #[serde(default)]
    pub shell: Option<Shell>,
    /// What shows where a shot glances off something and flies on.
    #[serde(default)]
    pub bounce: Option<Vec<Emit>>,
    #[serde(default)]
    pub ray: Option<Vec<Beam>>,
    /// What shows where each of a hitscan's rays stops.
    #[serde(default)]
    pub ray_end: Option<Vec<Emit>>,
    /// The beams of each arc from what a shot hit to another target: the shock mod's, the
    /// Plasma Coil's and the Spitting Hydra's.
    #[serde(default)]
    pub arc: Option<Vec<Beam>>,
    /// What shows where each arc ends.
    #[serde(default)]
    pub arc_end: Option<Vec<Emit>>,
    /// Whether the gun's blasts show as the kit's sphere of their reach (the default), or
    /// only through their `impact`, as the game draws the Nitro's.
    #[serde(default)]
    pub blast_sphere: Option<bool>,
    /// What the gun's shots of each kind show, by the kit's name for the kind
    /// ([`ShotKind::name`]), in place of what this gives: the events of a turret and its
    /// pellets, a canister's bomblets, and the look and trails of each in flight.
    #[serde(default)]
    pub shots: BTreeMap<String, GunFx>,
    #[serde(default)]
    pub from_level: BTreeMap<u8, GunFx>,
}

impl GunFx {
    /// What `part` gives at `level`: the highest of `from_level`'s up to it that gives it,
    /// else this.
    fn at<'a, T: ?Sized>(
        &'a self,
        level: u8,
        part: impl Fn(&'a GunFx) -> Option<&'a T>,
    ) -> Option<&'a T> {
        self.from_level
            .range(..=level)
            .rev()
            .find_map(|(_, fx)| part(fx))
            .or_else(|| part(self))
    }
}

/// The guns' effects, from `assets/effects.ron`.
#[derive(Resource, Debug, Clone, PartialEq, Deserialize)]
pub struct FxRecipes {
    /// What each texture the recipes name looks like until a game gives its own.
    pub placeholders: BTreeMap<Tex, Paint>,
    /// Lists of emits the recipes share by name ([`Emit::Set`]).
    #[serde(default)]
    pub sets: BTreeMap<String, Vec<Emit>>,
    /// What a gun shows that its recipe doesn't name.
    pub default: GunFx,
    pub guns: BTreeMap<String, GunFx>,
}

impl Default for FxRecipes {
    fn default() -> Self {
        ron::from_str(EFFECTS_RON).expect("the kit's effects.ron parses")
    }
}

impl FxRecipes {
    /// What `part` gives for gun `key`'s `shot` (by [`ShotKind::name`], `None` for the gun
    /// itself) at `level`: the gun's entry for that kind of shot, else the defaults' entry
    /// for it (the lightning bolt four guns share), else the gun's own.
    fn own<'a, T: ?Sized>(
        &'a self,
        key: &str,
        level: u8,
        shot: Option<&str>,
        part: impl Fn(&'a GunFx) -> Option<&'a T> + Copy,
    ) -> Option<&'a T> {
        let gun = self.guns.get(key)?;
        let kind = |fx: &'a GunFx| shot.and_then(|shot| fx.shots.get(shot));
        kind(gun)
            .and_then(|fx| fx.at(level, part))
            .or_else(|| kind(&self.default).and_then(|fx| fx.at(level, part)))
            .or_else(|| gun.at(level, part))
    }

    /// The same, else the defaults'.
    fn part<'a, T: ?Sized>(
        &'a self,
        key: &str,
        level: u8,
        shot: Option<&str>,
        part: impl Fn(&'a GunFx) -> Option<&'a T> + Copy,
    ) -> Option<&'a T> {
        self.own(key, level, shot, part)
            .or_else(|| self.default.at(level, part))
    }

    pub fn fire(&self, key: &str, level: u8, shot: Option<&str>) -> &[Emit] {
        self.part(key, level, shot, |fx| fx.fire.as_deref())
            .unwrap_or(&[])
    }

    pub fn impact(&self, key: &str, level: u8, shot: Option<&str>) -> &[Emit] {
        self.part(key, level, shot, |fx| fx.impact.as_deref())
            .unwrap_or(&[])
    }

    pub fn mark(&self, key: &str, level: u8, shot: Option<&str>) -> Option<&Mark> {
        self.own(key, level, shot, |fx| fx.mark.as_ref())
    }

    pub fn light(&self, key: &str, level: u8, shot: Option<&str>) -> Option<&ShotLight> {
        self.own(key, level, shot, |fx| fx.light.as_ref())
    }

    pub fn shell(&self, key: &str, level: u8) -> Option<&Shell> {
        self.own(key, level, None, |fx| fx.shell.as_ref())
    }

    pub fn bounce(&self, key: &str, level: u8, shot: Option<&str>) -> &[Emit] {
        self.part(key, level, shot, |fx| fx.bounce.as_deref())
            .unwrap_or(&[])
    }

    pub fn arc(&self, key: &str, level: u8, shot: Option<&str>) -> &[Beam] {
        self.part(key, level, shot, |fx| fx.arc.as_deref())
            .unwrap_or(&[])
    }

    pub fn arc_end(&self, key: &str, level: u8, shot: Option<&str>) -> &[Emit] {
        self.part(key, level, shot, |fx| fx.arc_end.as_deref())
            .unwrap_or(&[])
    }

    pub fn ray(&self, key: &str, level: u8, shot: Option<&str>) -> &[Beam] {
        self.part(key, level, shot, |fx| fx.ray.as_deref())
            .unwrap_or(&[])
    }

    pub fn ray_end(&self, key: &str, level: u8, shot: Option<&str>) -> &[Emit] {
        self.part(key, level, shot, |fx| fx.ray_end.as_deref())
            .unwrap_or(&[])
    }

    /// A shot's sprites; none, and it is drawn as the kit's ball.
    pub fn shot(&self, key: &str, level: u8, shot: Option<&str>) -> &[ShotSprite] {
        self.own(key, level, shot, |fx| fx.shot.as_deref())
            .unwrap_or(&[])
    }

    pub fn streak(&self, key: &str, level: u8, shot: Option<&str>) -> Option<&Streak> {
        self.own(key, level, shot, |fx| fx.streak.as_ref())
    }

    pub fn trail(&self, key: &str, level: u8, shot: Option<&str>) -> &[Trail] {
        self.own(key, level, shot, |fx| fx.trail.as_deref())
            .unwrap_or(&[])
    }

    pub fn blast_sphere(&self, key: &str, level: u8, shot: Option<&str>) -> bool {
        self.part(key, level, shot, |fx| fx.blast_sphere.as_ref())
            .copied()
            .unwrap_or(true)
    }

    pub fn wake(&self, key: &str, level: u8, shot: Option<&str>) -> &[Beam] {
        self.own(key, level, shot, |fx| fx.wake.as_deref())
            .unwrap_or(&[])
    }

    pub fn ribbon(&self, key: &str, level: u8, shot: Option<&str>) -> &[Ribbon] {
        self.own(key, level, shot, |fx| fx.ribbon.as_deref())
            .unwrap_or(&[])
    }

    pub fn panels(&self, key: &str, level: u8, shot: Option<&str>) -> Option<&Panels> {
        self.own(key, level, shot, |fx| fx.panels.as_ref())
    }

    pub fn open(&self, key: &str, level: u8, shot: Option<&str>) -> Option<&Open> {
        self.own(key, level, shot, |fx| fx.open.as_ref())
    }

    pub fn funnel(&self, key: &str, level: u8) -> Option<&Funnel> {
        self.own(key, level, None, |fx| fx.funnel.as_ref())
    }

    pub fn marker(&self, key: &str, level: u8) -> Option<&Marker> {
        self.own(key, level, None, |fx| fx.marker.as_ref())
    }

    pub fn sweep(&self, key: &str, level: u8, shot: Option<&str>) -> &[Sweep] {
        self.own(key, level, shot, |fx| fx.sweep.as_deref())
            .unwrap_or(&[])
    }

    pub fn fan(&self, key: &str, level: u8, shot: Option<&str>) -> Option<&Fan> {
        self.own(key, level, shot, |fx| fx.fan.as_ref())
    }

    pub fn glare(&self, key: &str, level: u8, shot: Option<&str>) -> Option<&Glare> {
        self.own(key, level, shot, |fx| fx.glare.as_ref())
    }

    pub fn charge(&self, key: &str, level: u8) -> Option<&Charge> {
        self.own(key, level, None, |fx| fx.charge.as_ref())
    }

    pub fn rope(&self, key: &str, level: u8) -> &[Strand] {
        self.own(key, level, None, |fx| fx.rope.as_deref())
            .unwrap_or(&[])
    }

    pub fn along_rope(&self, key: &str, level: u8) -> &[AlongRope] {
        self.own(key, level, None, |fx| fx.along_rope.as_deref())
            .unwrap_or(&[])
    }

    /// `emits` with each [`Emit::Set`] in it replaced by its set's emits (sets don't nest).
    pub fn expand<'a>(&'a self, emits: &'a [Emit]) -> impl Iterator<Item = &'a Emit> {
        emits.iter().flat_map(move |emit| match emit {
            Emit::Set(name) => self.sets.get(name).map_or(&[][..], Vec::as_slice),
            _ => std::slice::from_ref(emit),
        })
    }

    /// Every texture the recipes draw with.
    pub fn textures(&self) -> Vec<Tex> {
        fn gun(fx: &GunFx, textures: &mut Vec<Tex>) {
            let beams = fx.ray.iter().chain(&fx.wake).chain(&fx.arc).flatten();
            textures.extend(beams.map(|beam| beam.texture));
            textures.extend(fx.shot.iter().flatten().map(|shot| shot.texture));
            textures.extend(fx.streak.as_ref().map(|streak| streak.texture));
            textures.extend(fx.ribbon.iter().flatten().map(|ribbon| ribbon.texture));
            textures.extend(fx.sweep.iter().flatten().map(|sweep| sweep.texture));
            let fans = fx.fan.iter().flat_map(|fan| &fan.strands);
            let charges = fx.charge.iter();
            let crackles = charges.clone().flat_map(|charge| &charge.crackles);
            textures.extend(
                fx.rope
                    .iter()
                    .flatten()
                    .chain(fans)
                    .chain(crackles.flat_map(|crackle| &crackle.strands))
                    .map(|strand| strand.texture),
            );
            let glows = charges.clone().flat_map(|charge| &charge.glow);
            textures.extend(glows.flat_map(|glow| glow.textures.iter().copied()));
            let glares = charges.filter_map(|charge| charge.glare.as_ref());
            textures.extend(fx.glare.iter().chain(glares).map(|glare| glare.texture));
            let along = fx.along_rope.iter().flatten();
            textures.extend(along.filter_map(|along| along.emit.texture()));
            textures.extend(fx.mark.iter().flat_map(Mark::textures));
            let shells = fx.shell.iter();
            for shell in shells {
                textures.extend([shell.dome.texture, shell.rim.texture]);
                textures.extend(shell.arcs.iter().flat_map(|arcs| [arcs.core, arcs.glow]));
                textures.extend(shell.trails.iter().map(|trails| trails.texture));
            }
            let panels = fx.panels.iter().flat_map(|panels| &panels.layers);
            textures.extend(panels.map(|panel| panel.texture));
            let open = fx.open.iter();
            textures.extend(
                open.clone()
                    .flat_map(|open| &open.sprites)
                    .map(|s| s.texture),
            );
            textures.extend(open.clone().flat_map(|open| &open.rays).map(|r| r.texture));
            let open_trails = open.flat_map(|open| &open.trail);
            let trails = fx.trail.iter().flatten().chain(open_trails);
            textures.extend(trails.flat_map(|trail| trail.textures.iter().copied()));
            if let Some(funnel) = &fx.funnel {
                textures.extend(funnel.textures());
                textures.extend(funnel.emits().filter_map(Emit::texture));
            }
            textures.extend(fx.marker.iter().flat_map(|marker| marker.layers));
            let emits = fx.fire.iter().chain(&fx.impact).chain(&fx.bounce);
            for emits in emits.chain(&fx.ray_end).chain(&fx.arc_end) {
                textures.extend(emits.iter().filter_map(Emit::texture));
            }
            for fx in fx.from_level.values().chain(fx.shots.values()) {
                gun(fx, textures);
            }
        }
        let mut textures = Vec::new();
        for fx in std::iter::once(&self.default).chain(self.guns.values()) {
            gun(fx, &mut textures);
        }
        for emits in self.sets.values() {
            textures.extend(emits.iter().filter_map(Emit::texture));
        }
        textures.sort();
        textures.dedup();
        textures
    }

    /// Every texture the recipes scroll, whose images must repeat: a funnel's and an open
    /// shot's rays'.
    pub fn scrolled(&self) -> Vec<Tex> {
        fn gun(fx: &GunFx, textures: &mut Vec<Tex>) {
            if let Some(funnel) = &fx.funnel {
                textures.extend(funnel.textures());
            }
            let rays = fx.open.iter().flat_map(|open| &open.rays);
            textures.extend(rays.map(|rays| rays.texture));
            for fx in fx.from_level.values().chain(fx.shots.values()) {
                gun(fx, textures);
            }
        }
        let mut textures = Vec::new();
        for fx in std::iter::once(&self.default).chain(self.guns.values()) {
            gun(fx, &mut textures);
        }
        textures.sort();
        textures.dedup();
        textures
    }
}

/// The images the effects draw with, by the game's numbers: the kit's placeholders, or a
/// game's own put in their place.
#[derive(Resource, Debug, Default, Clone)]
pub struct FxImages(pub BTreeMap<Tex, Handle<Image>>);

/// The one quad every sprite is, facing +z.
#[derive(Resource, Debug, Clone)]
pub struct FxQuad(pub Handle<Mesh>);

/// A sprite turned to face the camera each frame, rolled by `roll` radians, or along an
/// axis to face it as nearly as it can (a flame or a beam).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Billboard {
    pub axis: Option<Vec3>,
    pub roll: f32,
}

/// A sprite kept just in front of the camera and wide enough to cover its view.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
struct CoversView;

/// How far in front of the eye a [`CoversView`] sprite sits, and how wide it is: wide
/// enough for any field of view under 175°, with room for the camera moving between the
/// frame that places it and the next.
const VIEW_COVER: (f32, f32) = (1.0, 100.0);

/// A sprite or particle living out its ticks.
#[derive(Component, Debug, Clone)]
struct Fx {
    velocity: Vec3,
    gravity: f32,
    ticks_left: u32,
    total: u32,
    /// Its scale when new and at its end.
    size: Vec2,
    end: Vec2,
    /// Its alpha when new and at its end.
    alpha: f32,
    end_alpha: f32,
    /// Its colour when new and at its end, if that changes.
    fade: Option<(Vec3, Vec3)>,
    /// The share of its velocity it keeps each tick.
    keep: f32,
    /// Ticks before it shows.
    wait: u32,
    /// Radians a tick.
    spin: f32,
    material: Handle<StandardMaterial>,
}

impl Fx {
    /// One that stays where it is put.
    fn still(
        ticks: u32,
        size: Vec2,
        end: Vec2,
        alpha: f32,
        material: Handle<StandardMaterial>,
    ) -> Self {
        Fx {
            velocity: Vec3::ZERO,
            gravity: 0.0,
            ticks_left: ticks.max(1),
            total: ticks.max(1),
            size,
            end,
            alpha,
            end_alpha: 0.0,
            fade: None,
            keep: 1.0,
            wait: 0,
            spin: 0.0,
            material,
        }
    }
}

/// One of a shot's sprites, turning `spin` radians a tick, or to a new roll each tick if
/// it `flickers`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShotLayer {
    pub spin: f32,
    pub flickers: bool,
    /// How wide it starts and how much narrower it gets a tick, if it does.
    pub size: f32,
    pub shrink: f32,
}

/// The streak laid along a shot's way.
#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct ShotStreak(pub Streak);

/// The effects' own random numbers.
#[derive(Resource, Debug, Clone)]
pub(crate) struct FxRng(u32);

impl FxRng {
    pub(crate) fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A tumble a tick: 3 to 6 degrees about a random axis (0x31D0B0, 0x31E488).
    pub(crate) fn tumble(&mut self) -> Quat {
        let axis = Vec3::new(self.unit() - 0.5, self.unit() - 0.5, self.unit() - 0.5);
        let angle = self.between((3.0, 6.0)).to_radians();
        Quat::from_axis_angle(axis.normalize_or(Vec3::Y), angle)
    }

    pub(crate) fn between(&mut self, (low, high): (f32, f32)) -> f32 {
        low + (high - low) * self.unit()
    }

    /// A whole number from `low` to `high`, both included.
    pub(crate) fn ticks(&mut self, low: u32, high: u32) -> u32 {
        let span = high.saturating_sub(low) + 1;
        low + ((self.unit() * span as f32) as u32).min(span - 1)
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        let index = (self.unit() * items.len() as f32) as usize;
        items.get(index.min(items.len().saturating_sub(1)))
    }

    /// A roll within `range` degrees, in radians.
    pub(crate) fn roll(&mut self, range: Option<(f32, f32)>) -> f32 {
        range.map_or(0.0, |range| self.between(range).to_radians())
    }

    /// A direction round `around` within `cone` degrees, or any direction.
    fn direction(&mut self, around: Option<(Vec3, f32)>) -> Vec3 {
        let (axis, cone) = around.unwrap_or((Vec3::Y, 180.0));
        let cos = 1.0 - (1.0 - ops::cos(cone.to_radians())) * self.unit();
        let sin = ops::sqrt((1.0 - cos * cos).max(0.0));
        let turn = self.unit() * std::f32::consts::TAU;
        let local = Vec3::new(sin * ops::cos(turn), sin * ops::sin(turn), cos);
        Quat::from_rotation_arc(Vec3::Z, axis.normalize_or(Vec3::Y)) * local
    }
}

/// The guns' effects. It needs Bevy's asset and render plugins for its images and quad;
/// without them (as in tests) nothing is drawn.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxRecipes>()
            .insert_resource(FxRng(0x6F1D_2C35))
            .init_resource::<TrailFrom>()
            .init_resource::<ribbon::Ribbons>()
            .init_resource::<sweep::Sweeps>()
            .init_resource::<rope::Strands>()
            .init_resource::<fan::Fans>()
            .init_resource::<charge::Charges>()
            .init_resource::<charge::ChargeLights>()
            .init_resource::<panel::ShownPanels>()
            .init_resource::<FxModels>()
            .init_resource::<CameraShake>()
            .init_resource::<light::ShotLights>()
            .init_resource::<shell::Shells>()
            .init_resource::<funnel::Funnels>()
            .init_resource::<open::ShownRays>()
            .init_resource::<marker::Markers>()
            .add_systems(Startup, paint_placeholders)
            .add_systems(
                FixedUpdate,
                (
                    emit_gun_fx,
                    funnel::draw_funnels,
                    open::step_open_rays,
                    marker::draw_markers,
                    emit_crate_blasts,
                    emit_trails,
                    rope::emit_along_ropes,
                    charge::emit_charges,
                    step_fx,
                    model::step_models,
                    mark::step_marks,
                    light::step_lights,
                    light::ride_shots,
                    shake::step_shake,
                )
                    .chain()
                    .in_set(rac_sim::SimSet::Particles)
                    .run_if(resource_exists::<FxQuad>),
            )
            .add_systems(Update, (face_camera, shake::shake_cameras))
            .add_systems(
                Update,
                (
                    ribbon::draw_ribbons,
                    sweep::draw_sweeps,
                    panel::draw_panels,
                    fan::draw_fans,
                    charge::draw_charges,
                    shell::draw_shells,
                    rope::draw_strands.after(rac_rig::RigSystems),
                )
                    .run_if(resource_exists::<FxImages>),
            );
    }
}

/// Paints a placeholder for every texture the recipes use, a plain glow for any the
/// recipes give no look.
fn paint_placeholders(
    recipes: Res<FxRecipes>,
    images: Option<ResMut<Assets<Image>>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    mut commands: Commands,
) {
    let (Some(mut images), Some(mut meshes)) = (images, meshes) else {
        commands.init_resource::<FxImages>();
        return;
    };
    let mut handles = BTreeMap::new();
    for texture in recipes.textures() {
        let paint = recipes
            .placeholders
            .get(&texture)
            .copied()
            .unwrap_or(Paint {
                shape: Shape::Glow,
                colour: [1.0, 1.0, 1.0],
            });
        handles.insert(texture, images.add(paint::image(paint)));
    }
    commands.insert_resource(FxImages(handles));
    commands.insert_resource(FxQuad(meshes.add(Rectangle::new(1.0, 1.0))));
    commands.insert_resource(model::Crown(meshes.add(model::crown())));
}

/// An unlit material of `texture` in `colour`, seen from both sides.
fn fx_material(
    materials: &mut Assets<StandardMaterial>,
    images: &FxImages,
    texture: Tex,
    colour: [f32; 4],
    blend: Blend,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgba(colour[0], colour[1], colour[2], colour[3]),
        base_color_texture: images.0.get(&texture).cloned(),
        alpha_mode: blend.alpha_mode(),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    })
}

/// What spawning sprites, models, marks and lights and shaking the camera need.
struct Spawner<'w, 's, 'a> {
    commands: Commands<'w, 's>,
    materials: &'a mut Assets<StandardMaterial>,
    meshes: &'a mut Assets<Mesh>,
    images: &'a FxImages,
    quad: Handle<Mesh>,
    models: &'a FxModels,
    crown: Handle<Mesh>,
    /// The world, to lay things on the floor.
    world: Option<&'a dyn CollisionWorld>,
    shake: &'a mut CameraShake,
    /// The light slots not taken.
    lights_free: usize,
}

/// A system's way to a [`Spawner`].
#[derive(SystemParam)]
struct Making<'w, 's> {
    commands: Commands<'w, 's>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    images: Res<'w, FxImages>,
    quad: Res<'w, FxQuad>,
    models: Res<'w, FxModels>,
    crown: Res<'w, model::Crown>,
    scene: Option<Res<'w, CollisionScene>>,
    shake: ResMut<'w, CameraShake>,
    lights: Query<'w, 's, (), Or<(With<light::Lit>, With<light::Riding>)>>,
}

impl<'w> Making<'w, '_> {
    fn spawner(&mut self) -> Spawner<'w, '_, '_> {
        Spawner {
            commands: self.commands.reborrow(),
            materials: &mut self.materials,
            meshes: &mut self.meshes,
            images: &self.images,
            quad: self.quad.0.clone(),
            models: &self.models,
            crown: self.crown.0.clone(),
            world: self
                .scene
                .as_deref()
                .map(|scene| &scene.0 as &dyn CollisionWorld),
            lights_free: light::SLOTS.saturating_sub(self.lights.iter().count()),
            shake: &mut self.shake,
        }
    }
}

/// Where something put at `at` lies on the floor within `reach` below it, just above it
/// as the game puts its gas clouds (0x467BD8); `None` where there's no floor in reach.
/// Without a world, where it is put.
fn on_ground(world: Option<&dyn CollisionWorld>, at: Vec3, reach: f32) -> Option<Vec3> {
    let Some(world) = world else {
        return Some(at);
    };
    let down = Vec3::NEG_Y * reach;
    let hit = world.cast_ray(at, down)?;
    Some(at + down * hit.fraction + Vec3::Y * 0.05)
}

impl Spawner<'_, '_, '_> {
    /// One of `trail`'s particles at `at`, `scale` times its size, from something whose
    /// way over the last tick was `path`.
    fn trail(&mut self, trail: &Trail, at: Vec3, path: Vec3, scale: f32, rng: &mut FxRng) {
        let (Some(&texture), Some(&colour)) = (rng.pick(&trail.textures), rng.pick(&trail.colours))
        else {
            return;
        };
        let (shell, drawn_in) = match trail.shell {
            Some(shell) => {
                let heading = Vec3::new(
                    rng.between((-1.0, 1.0)),
                    if trail.level {
                        0.0
                    } else {
                        rng.between((-1.0, 1.0))
                    },
                    rng.between((-1.0, 1.0)),
                )
                .normalize_or(Vec3::X);
                (
                    heading * rng.between(shell),
                    -heading * rng.between(trail.drawn_in),
                )
            }
            None => (Vec3::ZERO, Vec3::ZERO),
        };
        let at = match trail.ground {
            Some(reach) => match on_ground(self.world, at, reach) {
                Some(at) => at + shell,
                None => return,
            },
            None => at + shell,
        };
        let life = rng.ticks(trail.ticks, trail.ticks_to);
        let billboard = Billboard {
            axis: None,
            roll: rng.roll(Some((0.0, 360.0))),
        };
        let scatter = (-trail.scatter, trail.scatter);
        let scatter = Vec3::new(
            rng.between(scatter),
            rng.between(scatter),
            rng.between(scatter),
        );
        // The path is one tick's, so 60 times it is the velocity.
        let back = -path * 60.0 * trail.back;
        let fx = Fx {
            velocity: Vec3::Y * rng.between(trail.rise) + back + scatter + drawn_in,
            gravity: trail.gravity,
            keep: trail.keep,
            end_alpha: trail.end_alpha,
            fade: fade(colour, trail.end_colour),
            spin: rng.between((-trail.spin, trail.spin)).to_radians(),
            ..Fx::still(
                life,
                Vec2::splat(trail.size * scale),
                Vec2::splat(trail.end_size * scale),
                colour[3],
                self.material(texture, colour, trail.blend),
            )
        };
        self.spawn(at, billboard, fx, "trail");
    }

    /// One of `glare`'s rays round `at`, the sprite laid along it from its head out.
    fn glare(&mut self, glare: &Glare, at: Vec3, rng: &mut FxRng) {
        let out = rng.direction(None);
        let head = at + out * rng.between(glare.near);
        let size = Vec2::new(glare.length, glare.width);
        let fx = Fx {
            velocity: -out * glare.creep * 60.0,
            end_alpha: glare.colour[3],
            ..Fx::still(
                glare.ticks,
                size,
                size,
                glare.colour[3],
                self.material(glare.texture, glare.colour, glare.blend),
            )
        };
        let billboard = Billboard {
            axis: Some(out),
            roll: 0.0,
        };
        self.spawn(head + out * glare.length / 2.0, billboard, fx, "glare");
    }

    fn material(
        &mut self,
        texture: Tex,
        colour: [f32; 4],
        blend: Blend,
    ) -> Handle<StandardMaterial> {
        fx_material(self.materials, self.images, texture, colour, blend)
    }

    fn spawn(&mut self, at: Vec3, billboard: Billboard, fx: Fx, name: &'static str) -> Entity {
        // Hidden while it waits.
        let scale = if fx.wait > 0 {
            Vec3::ZERO
        } else {
            fx.size.extend(1.0)
        };
        // Bevy's shadow pass takes blended meshes too, and the game's effects cast none.
        self.commands
            .spawn((
                Name::new(name),
                DemoEntity,
                Mesh3d(self.quad.clone()),
                MeshMaterial3d(fx.material.clone()),
                NotShadowCaster,
                billboard,
                SimTransform::from_translation(at),
                Transform::from_translation(at).with_scale(scale),
                fx,
            ))
            .id()
    }

    /// Shows `emit` at `at`, where a shot heading `way` was fired or landed, seen from
    /// `eye`.
    fn emit(&mut self, emit: &Emit, at: Vec3, way: Option<Vec3>, eye: Vec3, rng: &mut FxRng) {
        match *emit {
            Emit::Flash {
                texture,
                length,
                width,
                ticks,
                colour,
                blend,
            } => {
                let way = way.unwrap_or(Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);
                let material = self.material(texture, colour, blend);
                let size = Vec2::new(length, width);
                let billboard = Billboard {
                    axis: Some(way),
                    roll: 0.0,
                };
                let fx = Fx::still(ticks, size, size, colour[3], material);
                self.spawn(at + way * length / 2.0, billboard, fx, "flash");
            }
            Emit::Glow {
                texture,
                size,
                end_size,
                ticks,
                colour,
                end_alpha,
                roll,
                spin,
                ahead,
                delay,
                nearer,
                blend,
            } => {
                let material = self.material(texture, colour, blend);
                let billboard = Billboard {
                    axis: None,
                    roll: rng.roll(roll),
                };
                let at = at + (eye - at).normalize_or_zero() * nearer;
                let fx = Fx {
                    velocity: way.map_or(Vec3::ZERO, Vec3::normalize_or_zero) * ahead,
                    wait: delay,
                    end_alpha,
                    spin: spin.to_radians(),
                    ..Fx::still(
                        ticks,
                        Vec2::splat(size),
                        Vec2::splat(end_size),
                        colour[3],
                        material,
                    )
                };
                self.spawn(at, billboard, fx, "glow");
            }
            Emit::Burst {
                texture,
                count,
                speed,
                speed_from,
                cone,
                toward,
                rise,
                gravity,
                keep,
                size,
                size_to,
                end_size,
                ticks,
                ticks_to,
                colour,
                end_alpha,
                end_colour,
                spin,
                line,
                jitter,
                ground,
                streak,
                delay,
                over,
                blend,
            } => {
                // Each fades on its own life, so each has its own material.
                let heading = match toward {
                    Toward::Way => way,
                    Toward::Camera => Some(eye - at),
                    Toward::Level => None,
                };
                let back = way.map_or(Vec3::ZERO, |way| -way.normalize_or_zero());
                for index in 0..count {
                    let around = heading.zip(cone);
                    let speed = rng.between((speed_from.unwrap_or(0.4 * speed), speed));
                    let mut velocity = rng.direction(around);
                    if toward == Toward::Level {
                        velocity = Vec3::new(velocity.x, 0.0, velocity.z).normalize_or(Vec3::X);
                    }
                    let velocity = velocity * speed + Vec3::Y * rng.between(rise);
                    let life = match ticks_to {
                        Some(to) => rng.ticks(ticks, to),
                        None => (ticks as f32 * (0.67 + 0.67 * rng.unit())).max(1.0) as u32,
                    };
                    let along = if count > 1 {
                        line * index as f32 / (count - 1) as f32
                    } else {
                        0.0
                    };
                    let from = at + back * along + rng.direction(None) * jitter * rng.unit();
                    let from = match ground {
                        Some(reach) => match on_ground(self.world, from, reach) {
                            Some(from) => from,
                            None => continue,
                        },
                        None => from,
                    };
                    let start = size_to.map_or(size, |to| rng.between((size, to)));
                    let end = end_size * start / size;
                    let (billboard, start, end) = match streak {
                        Some(length) => (
                            Billboard {
                                axis: Some(velocity.normalize_or(Vec3::Y)),
                                roll: 0.0,
                            },
                            Vec2::new(length, start),
                            Vec2::new(length, end),
                        ),
                        None => (
                            Billboard {
                                axis: None,
                                roll: rng.roll(Some((0.0, 360.0))),
                            },
                            Vec2::splat(start),
                            Vec2::splat(end),
                        ),
                    };
                    let fx = Fx {
                        velocity,
                        gravity,
                        keep,
                        wait: delay + rng.ticks(0, over),
                        end_alpha,
                        fade: fade(colour, end_colour),
                        spin: rng.between((-spin, spin)).to_radians(),
                        ..Fx::still(
                            life,
                            start,
                            end,
                            colour[3],
                            self.material(texture, colour, blend),
                        )
                    };
                    self.spawn(from, billboard, fx, "particle");
                }
            }
            Emit::Screen {
                ticks,
                colour,
                end_alpha,
                delay,
            } => {
                let [r, g, b, a] = colour;
                let material = self.materials.add(StandardMaterial {
                    base_color: Color::srgba(r * 0.5, g * 0.5, b * 0.5, a),
                    alpha_mode: Blend::Add.alpha_mode(),
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
                let size = Vec2::splat(VIEW_COVER.1);
                let fx = Fx {
                    wait: delay,
                    end_alpha,
                    ..Fx::still(ticks, size, size, a, material)
                };
                let entity = self.spawn(eye, Billboard::default(), fx, "screen flash");
                self.commands.entity(entity).insert(CoversView);
            }
            Emit::Model {
                class,
                scale,
                grow,
                ticks,
                colour,
                end_alpha,
                blend,
            } => {
                let parts = self.models.0.get(&class).map_or_else(
                    || vec![(self.crown.clone(), None)],
                    |model| model.parts.clone(),
                );
                let place = Transform::from_translation(at)
                    .with_rotation(Quat::from_rotation_y(
                        rng.between((0.0, 360.0)).to_radians(),
                    ))
                    .with_scale(Vec3::splat(rng.between(scale)));
                for (mesh, image) in parts {
                    let [r, g, b, a] = colour;
                    let material = self.materials.add(StandardMaterial {
                        base_color: Color::srgba(r, g, b, a),
                        base_color_texture: image,
                        alpha_mode: blend.alpha_mode(),
                        unlit: true,
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    });
                    self.commands.spawn((
                        Name::new("model"),
                        DemoEntity,
                        Mesh3d(mesh),
                        MeshMaterial3d(material.clone()),
                        NotShadowCaster,
                        place,
                        model::Grows {
                            ticks_left: ticks.max(1),
                            total: ticks.max(1),
                            grow,
                            alpha: a,
                            end_alpha,
                            material,
                        },
                    ));
                }
            }
            Emit::Light {
                radius,
                colour,
                ticks,
                ahead,
                above,
                rise,
                fall,
            } => {
                if self.lights_free == 0 {
                    return;
                }
                self.lights_free -= 1;
                let ticks = ticks.max(1);
                let fall = fall.unwrap_or(colour.map(|channel| channel / ticks as f32));
                let lit = light::Lit::new(radius, colour, ticks, rise, fall);
                let at = at + way.map_or(Vec3::ZERO, |way| way.normalize_or_zero() * ahead);
                self.commands.spawn((
                    Name::new("light"),
                    DemoEntity,
                    light::point_light(radius, lit.colour),
                    Transform::from_translation(at + Vec3::Y * above),
                    lit,
                ));
            }
            Emit::Shake {
                amplitude,
                ticks,
                ref along,
                lose,
                within,
                beyond,
            } => {
                let far = at.distance(eye);
                if let Some(amplitude) = shake::amplitude_at(amplitude, lose, within, beyond, far) {
                    for &along in along {
                        self.shake.set(along, amplitude, ticks);
                    }
                }
            }
            Emit::Set(_) => {}
        }
    }

    /// Each of `beams` from `from` to `to`, bent as its recipe has it.
    fn beams(&mut self, beams: &[Beam], from: Vec3, to: Vec3, rng: &mut FxRng) {
        let mut points = vec![from, to];
        for beam in beams {
            if !beam.same_bends {
                points = beam
                    .jag
                    .map_or_else(|| vec![from, to], |jag| jag.points(from, to, rng));
            }
            for pair in points.windows(2) {
                self.segment(beam, pair[0], pair[1]);
            }
        }
    }

    /// A straight piece of `beam`.
    fn segment(&mut self, beam: &Beam, from: Vec3, to: Vec3) {
        let length = from.distance(to);
        if length < 1e-3 {
            return;
        }
        let material = self.material(beam.texture, beam.colour, beam.blend);
        let size = Vec2::new(length, beam.width);
        let billboard = Billboard {
            axis: Some(to - from),
            roll: 0.0,
        };
        let fx = Fx::still(
            beam.ticks,
            size,
            Vec2::new(length, beam.end_width.unwrap_or(0.0)),
            beam.colour[3],
            material,
        );
        self.spawn((from + to) / 2.0, billboard, fx, "beam");
    }
}

/// A colour's fade to `end`, if it has one.
fn fade(colour: [f32; 4], end: Option<[f32; 3]>) -> Option<(Vec3, Vec3)> {
    end.map(|end| (Vec3::new(colour[0], colour[1], colour[2]), Vec3::from(end)))
}

/// The eye the effects are seen from, for what turns to it: the active 3D camera's.
fn eye(cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>) -> Vec3 {
    cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map_or(Vec3::ZERO, |(_, place)| place.translation())
}

/// The level of `player`'s `weapon`, 0 (V1) if they have none.
fn level(heroes: &Query<(&Hero, &Arsenal)>, player: usize, weapon: WeaponId) -> u8 {
    heroes
        .iter()
        .filter(|(hero, _)| hero.player == player)
        .find_map(|(_, arsenal)| arsenal.gun(weapon))
        .map_or(0, |gun| gun.level)
}

/// Shows each gun event as its gun's recipe has it at the gun's level.
#[allow(clippy::too_many_arguments)]
/// The shared explosion where an explosive crate goes off, as `crate_break` sets one off
/// with its blast; at the set's own size, not the call's (`effects.md`, "The shared
/// explosion").
fn emit_crate_blasts(
    recipes: Res<FxRecipes>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut rng: ResMut<FxRng>,
    mut blasts: MessageReader<CrateBlast>,
    mut making: Making,
) {
    let mut spawner = making.spawner();
    let eye = eye(&cameras);
    let explosion = recipes.sets.get("explosion").map_or(&[][..], Vec::as_slice);
    for blast in blasts.read() {
        for emit in recipes.expand(explosion) {
            spawner.emit(emit, blast.at, None, eye, &mut rng);
        }
    }
}

fn emit_gun_fx(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    marks: Query<&mark::MarkPiece>,
    mut rng: ResMut<FxRng>,
    mut events: MessageReader<GunEvent>,
    mut making: Making,
) {
    let mut spawner = making.spawner();
    let eye = eye(&cameras);
    let mut shown = mark::shown(&marks);
    for event in events.read() {
        let key = armory
            .get(event.weapon)
            .map_or("", |def| def.effects.as_str());
        let level = level(&heroes, event.player, event.weapon);
        let shot = event.shot;
        match event.kind {
            GunEventKind::Fired { at, direction } => {
                for emit in recipes.expand(recipes.fire(key, level, shot)) {
                    spawner.emit(emit, at, Some(direction), eye, &mut rng);
                }
            }
            GunEventKind::Impact { at, surface } => {
                for emit in recipes.expand(recipes.impact(key, level, shot)) {
                    spawner.emit(emit, at, None, eye, &mut rng);
                }
                let (Some(normal), Some(mark)) = (surface, recipes.mark(key, level, shot)) else {
                    continue;
                };
                let count = shown.entry(key.to_owned()).or_insert(0);
                if *count < mark.most && rng.ticks(1, mark.chance.max(1)) == 1 {
                    *count += 1;
                    spawner.mark(mark, key, at, normal, &mut rng);
                }
            }
            GunEventKind::Bounce { at } => {
                for emit in recipes.expand(recipes.bounce(key, level, shot)) {
                    spawner.emit(emit, at, None, eye, &mut rng);
                }
            }
            GunEventKind::Ray { from, to } => {
                spawner.beams(recipes.ray(key, level, shot), from, to, &mut rng);
                for emit in recipes.expand(recipes.ray_end(key, level, shot)) {
                    spawner.emit(emit, to, Some(to - from), eye, &mut rng);
                }
            }
            GunEventKind::Arc { from, to } => {
                spawner.beams(recipes.arc(key, level, shot), from, to, &mut rng);
                for emit in recipes.expand(recipes.arc_end(key, level, shot)) {
                    spawner.emit(emit, to, Some(to - from), eye, &mut rng);
                }
            }
            GunEventKind::LevelUp { .. }
            | GunEventKind::Empty { .. }
            | GunEventKind::Marked { .. }
            | GunEventKind::Fizzled { .. } => {}
        }
    }
}

/// Where each shot with a trail was last tick, by its hero's player and its serial.
#[derive(Resource, Debug, Default)]
struct TrailFrom(BTreeMap<(usize, u32), Vec3>);

/// Lays each shot's trail along the way it flew this tick.
#[allow(clippy::too_many_arguments)]
fn emit_trails(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut from: ResMut<TrailFrom>,
    mut rng: ResMut<FxRng>,
    mut making: Making,
) {
    let mut spawner = making.spawner();
    let mut now = BTreeMap::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let kind = Some(shot.kind.name());
            if let Some(glare) = recipes.glare(key, shot.level, kind)
                && glare.until.is_none_or(|until| shot.ticks < until)
            {
                spawner.glare(glare, shot.position, &mut rng);
            }
            let open = recipes
                .open(key, shot.level, kind)
                .filter(|_| crate::guns::standing(&shot.kind));
            let trails = open.map_or_else(
                || recipes.trail(key, shot.level, kind),
                |open| open.trail.as_slice(),
            );
            let wake = recipes.wake(key, shot.level, kind);
            if trails.is_empty() && wake.is_empty() {
                continue;
            }
            let id = (hero.player, shot.serial);
            now.insert(id, shot.position);
            let Some(&last) = from.0.get(&id) else {
                continue;
            };
            spawner.beams(wake, last, shot.position, &mut rng);
            let path = shot.position - last;
            for trail in trails {
                if trail.until.is_some_and(|until| shot.ticks >= until) {
                    continue;
                }
                let count = trail.count(path.length(), rng.unit());
                for index in 0..count {
                    let at = last + path * (index + 1) as f32 / count as f32;
                    spawner.trail(trail, at, path, 1.0, &mut rng);
                }
            }
        }
    }
    from.0 = now;
}

/// Moves, grows, turns and fades every sprite and particle a tick, dropping those whose
/// time is up.
fn step_fx(
    mut fx: Query<(
        Entity,
        &mut Fx,
        &mut Billboard,
        &mut SimTransform,
        &mut Transform,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    const TICK: f32 = 1.0 / 60.0;
    for (entity, mut fx, mut billboard, mut place, mut transform) in &mut fx {
        // Drawn first as it is at its age, as the game draws a particle the frame it is
        // made: one of `ticks` ticks shows for that many.
        if fx.wait > 0 {
            fx.wait -= 1;
            continue;
        }
        if fx.ticks_left == 0 {
            commands.entity(entity).despawn();
            continue;
        }
        let left = fx.ticks_left as f32 / fx.total as f32;
        transform.scale = fx.end.lerp(fx.size, left).extend(1.0);
        if let Some(mut material) = materials.get_mut(&fx.material) {
            let alpha = fx.end_alpha + (fx.alpha - fx.end_alpha) * left;
            if let Some((from, to)) = fx.fade {
                let colour = to.lerp(from, left);
                material.base_color = Color::srgb(colour.x, colour.y, colour.z);
            }
            material.base_color.set_alpha(alpha);
        }
        fx.ticks_left -= 1;
        fx.velocity.y -= fx.gravity * TICK;
        let keep = fx.keep;
        fx.velocity *= keep;
        place.translation += fx.velocity * TICK;
        billboard.roll += fx.spin;
    }
}

/// Turns the sprites to the camera, and keeps those that cover its view in front of it.
fn face_camera(
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut sprites: Query<(
        &Billboard,
        &mut Transform,
        &GlobalTransform,
        Has<CoversView>,
    )>,
) {
    let Some((_, camera)) = cameras.iter().find(|(camera, _)| camera.is_active) else {
        return;
    };
    let (eye, facing) = (camera.translation(), camera.rotation());
    for (billboard, mut transform, place, covers) in &mut sprites {
        if covers {
            // After the interpolation has put it where the simulation left it.
            transform.translation = eye + facing * Vec3::NEG_Z * VIEW_COVER.0;
        }
        let rolled = facing * Quat::from_rotation_z(billboard.roll);
        transform.rotation = match billboard.axis {
            None => rolled,
            Some(axis) => along(axis, eye - place.translation()).unwrap_or(rolled),
        };
    }
}

/// The turn that lays a quad's +x along `axis` and turns its face (+z) as nearly toward
/// `view` as that allows.
fn along(axis: Vec3, view: Vec3) -> Option<Quat> {
    let x = axis.try_normalize()?;
    let z = (view - x * view.dot(x)).try_normalize()?;
    let y = z.cross(x);
    Some(Quat::from_mat3(&Mat3::from_cols(x, y, z)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_has_effects_and_every_texture_a_look() {
        let recipes = FxRecipes::default();
        let armory = Armory::uya();
        for def in &armory.weapons {
            assert!(recipes.guns.contains_key(&def.effects), "{}", def.effects);
        }
        for texture in recipes.textures() {
            assert!(recipes.placeholders.contains_key(&texture), "{texture:?}");
        }
        assert!(recipes.expand(recipes.impact("n60", 0, None)).count() > 0);
        assert!(!recipes.ray("shock_blaster", 0, None).is_empty());
        // The game draws the Nitro's blast only as its effects; the kit's own keep the
        // sphere.
        assert!(!recipes.blast_sphere("nitro", 2, None));
        assert!(recipes.blast_sphere("agents", 2, None));
        let flashes = |emits: &[Emit]| {
            recipes
                .expand(emits)
                .any(|emit| matches!(emit, Emit::Screen { .. }))
        };
        assert!(flashes(recipes.fire("shock_blaster", 0, None)));
        // The Flux Rifle lights its muzzle at every level and the Nitro's blast shakes the
        // camera; the shield's and the rift's lights ride them in flight, not the rift's
        // bolts.
        let has = |emits: &[Emit], which: fn(&Emit) -> bool| recipes.expand(emits).any(which);
        let light = |emit: &Emit| matches!(emit, Emit::Light { .. });
        let shake = |emit: &Emit| matches!(emit, Emit::Shake { .. });
        assert!((0..4).all(|level| has(recipes.fire("flux", level, None), light)));
        assert!((0..4).all(|level| has(recipes.impact("nitro", level, None), shake)));
        assert!(has(recipes.impact("annihilator", 2, None), shake));
        assert!(!has(recipes.impact("n60", 0, None), shake));
        let barrier = recipes.light("holo", 0, Some("barrier")).unwrap();
        assert!(barrier.flying && barrier.radius == 5.0);
        assert!(recipes.light("rift", 0, Some("rift")).is_some());
        assert!(recipes.light("rift", 0, Some("bolt")).is_none());
        // The shield's shell at every level, red at V1 and green at V3, flashing blue and
        // red.
        let shell = |level| recipes.shell("shield", level).expect("a shell");
        assert_eq!(shell(0).rest, [1.125, 0.25, 0.25]);
        assert_eq!(shell(2).rest, [0.25, 0.625, 0.25]);
        assert_eq!(shell(2).hit, [1.25, 0.375, 0.375]);
        assert_eq!(shell(7).rest, shell(4).rest);
        assert!(recipes.shell("agents", 0).is_none());
        assert!((0..4).all(|level| flashes(recipes.impact("nitro", level, None))));
        // A canister's burst flashes and its bomblets' don't; the Annihilator's mines are
        // the same bomblets from V2.
        assert!(flashes(recipes.impact("bouncer", 0, None)));
        let bomblet = Some("bomblet");
        assert!(!flashes(recipes.impact("bouncer", 0, bomblet)));
        assert!(!flashes(recipes.impact("bouncer", 4, bomblet)));
        assert_ne!(
            recipes.impact("bouncer", 4, bomblet),
            recipes.impact("bouncer", 0, bomblet)
        );
        assert_eq!(
            recipes.impact("annihilator", 1, bomblet),
            recipes.impact("bouncer", 0, bomblet)
        );
        // A kind of shot a recipe doesn't name shows the gun's own.
        assert_eq!(
            recipes.impact("bouncer", 0, Some("canister")),
            recipes.impact("bouncer", 0, None)
        );
        // The glove's throw shows nothing; its turret's shots and their hits do, blue from
        // V3.
        assert!(recipes.fire("turret", 0, None).is_empty());
        assert!(!recipes.fire("turret", 0, Some("turret")).is_empty());
        let pellet = Some("pellet");
        let tracer = |level| recipes.streak("turret", level, pellet).map(|s| s.colour);
        assert!(tracer(0).is_some());
        assert_ne!(tracer(1), tracer(2));
        assert!(!recipes.impact("turret", 0, pellet).is_empty());
        assert_ne!(
            recipes.impact("turret", 0, Some("turret")),
            recipes.impact("turret", 0, pellet)
        );
        // An agent's end at V1, its comet's and its bullets' are each their own, and the
        // bullets' hits turn to embers from V3.
        let agents = |level, shot| recipes.impact("agents", level, Some(shot));
        assert_ne!(agents(0, "agent"), agents(1, "comet"));
        assert_ne!(agents(1, "slug"), agents(2, "slug"));
        assert_eq!(agents(3, "slug"), agents(2, "slug"));
        assert!(recipes.fire("agents", 0, None).is_empty());
        // The coil's arcs are its own, five bowed cores each with its glow; other guns'
        // are the default lightning, but the Hydra's spit is a fan of arcs of its own,
        // fanned wider from V3 and V5.
        let coil = recipes.arc("coil", 0, None);
        assert_eq!(coil.len(), 10);
        assert!(
            coil.iter()
                .step_by(2)
                .all(|core| core.jag.is_some_and(|jag| jag.bows.is_some()))
        );
        assert!(coil.iter().skip(1).step_by(2).all(|glow| glow.same_bends));
        assert_eq!(recipes.arc("n60", 0, None)[0].texture, Tex::Effect(47));
        let spit = Some("spit");
        assert!(recipes.arc("hydra", 0, spit).is_empty());
        assert!(recipes.arc_end("hydra", 0, spit).is_empty());
        let roll = |level| recipes.fan("hydra", level, spit).map(|fan| fan.roll);
        assert_eq!(roll(0), Some((-107.0, 107.0)));
        assert_eq!(roll(2), Some((-107.5, 52.5)));
        assert_eq!(roll(4), Some((-90.0, 30.0)));
        // The four guns' bolts are the defaults' lightning, the Rift Ripper's in its own
        // colour; each strikes where it reaches.
        let bolt = Some("bolt");
        let lightning = recipes.arc("hydra", 0, bolt);
        assert_eq!(lightning.len(), 4);
        assert!(lightning.iter().step_by(2).all(|core| core.jag.is_some()));
        assert!(
            lightning
                .iter()
                .skip(1)
                .step_by(2)
                .all(|glow| glow.same_bends)
        );
        assert_eq!(recipes.arc("holo", 2, bolt), lightning);
        assert_eq!(recipes.arc("shield", 4, bolt), lightning);
        assert_ne!(recipes.arc("rift", 4, bolt), lightning);
        assert!(recipes.arc_end("hydra", 0, bolt).is_empty());
        // The Lava Gun's stream is three ribbons through its points, the glow six times
        // as wide as the core; its points show nothing of their own.
        let stream = recipes.ribbon("lava", 0, Some("stream"));
        assert_eq!(stream.len(), 3);
        assert_eq!(stream[0].width.0, stream[2].width.0 * 6.0);
        assert!(recipes.shot("lava", 0, Some("stream")).is_empty());
        assert!(!recipes.trail("lava", 0, Some("stream")).is_empty());
        // A holo shield flies as its sprites and unfolds in puffs where it lands.
        let barrier = Some("barrier");
        assert_eq!(recipes.shot("holo", 0, barrier).len(), 5);
        assert!(recipes.shot("holo", 0, None).is_empty());
        assert!(!recipes.impact("holo", 0, barrier).is_empty());
        // It stands as its wall's sprites, in clouds of 67 from V3.
        let wall = |level| {
            recipes
                .panels("holo", level, barrier)
                .map(|p| p.layers.clone())
        };
        assert!(wall(0).is_some_and(|layers| layers.len() == 8));
        assert_eq!(wall(1), wall(0));
        assert!(wall(2).is_some_and(|layers| layers.iter().any(|l| l.texture == Tex::Effect(67))));
        assert_ne!(
            recipes.impact("holo", 2, bolt),
            recipes.impact("holo", 2, None)
        );
        // A disc glances and bursts by level: its sprites to V4, its trail from V2, longer
        // kept at V5, which bursts small and red.
        let disc = Some("disc");
        assert_eq!(recipes.shot("disc", 3, disc).len(), 6);
        assert!(recipes.shot("disc", 4, disc).is_empty());
        assert!(recipes.sweep("disc", 0, disc).is_empty());
        let kept = |level| {
            recipes
                .sweep("disc", level, disc)
                .first()
                .map(|s| (s.every, s.points))
        };
        assert_eq!(kept(1), Some((6, 6)));
        assert_eq!(kept(3), Some((6, 6)));
        assert_eq!(kept(4), Some((4, 4)));
        let bursts: Vec<_> = (0..5)
            .map(|level| recipes.impact("disc", level, disc))
            .collect();
        assert_eq!(bursts[0], bursts[1]);
        assert!(bursts.windows(2).skip(1).all(|pair| pair[0] != pair[1]));
        assert_ne!(
            recipes.bounce("disc", 1, disc),
            recipes.bounce("disc", 2, disc)
        );
        assert!(recipes.bounce("n60", 0, None).is_empty());
        for (name, emits) in &recipes.sets {
            assert!(
                !emits.iter().any(|emit| matches!(emit, Emit::Set(_))),
                "{name} nests"
            );
        }
        fn named(fx: &GunFx, recipes: &FxRecipes) {
            let emits = fx.fire.iter().chain(&fx.impact).chain(&fx.bounce);
            let emits = emits.chain(&fx.ray_end);
            let along = fx.along_rope.iter().flatten().map(|along| &along.emit);
            for emit in emits.chain(&fx.arc_end).flatten().chain(along) {
                if let Emit::Set(name) = emit {
                    assert!(recipes.sets.contains_key(name), "no set {name}");
                }
            }
            for fx in fx.from_level.values().chain(fx.shots.values()) {
                named(fx, recipes);
            }
        }
        for fx in recipes.guns.values() {
            named(fx, &recipes);
        }
    }

    #[test]
    fn from_a_level_its_entry_takes_the_place_of_the_parts_it_gives() {
        let recipes: FxRecipes = ron::from_str(
            r#"(
                placeholders: {},
                default: (ray: Some([(texture: Effect(43), width: 0.1, ticks: 1)])),
                guns: {
                    "gun": (
                        fire: Some([]),
                        impact: Some([Set("a")]),
                        from_level: {
                            2: (impact: Some([])),
                            4: (fire: Some([Set("b")]), impact: Some([Set("c"), Set("d")])),
                        },
                    ),
                },
            )"#,
        )
        .unwrap();
        let impacts: Vec<_> = (0..6)
            .map(|level| recipes.impact("gun", level, None).len())
            .collect();
        assert_eq!(impacts, [1, 1, 0, 0, 2, 2]);
        assert_eq!(recipes.fire("gun", 3, None).len(), 0);
        assert_eq!(recipes.fire("gun", 4, None).len(), 1);
        // What no level gives comes from the defaults.
        assert!(!recipes.ray("gun", 5, None).is_empty());
    }

    #[test]
    fn a_beam_lies_along_its_way_and_faces_the_eye() {
        let turn = along(Vec3::X, Vec3::new(0.0, 0.0, 5.0)).unwrap();
        assert!((turn * Vec3::X).abs_diff_eq(Vec3::X, 1e-5));
        assert!((turn * Vec3::Z).abs_diff_eq(Vec3::Z, 1e-5));
        let tilted = along(Vec3::X, Vec3::new(1.0, 3.0, 0.0)).unwrap();
        assert!((tilted * Vec3::Z).abs_diff_eq(Vec3::Y, 1e-5));
        assert_eq!(along(Vec3::X, Vec3::X), None);
    }

    #[test]
    fn a_bent_beam_keeps_its_ends_and_bends_within_its_reach() {
        let jag = Jag {
            points: 10,
            wave: 0.1,
            around: 0.5,
            turn: 36.0,
            jitter: 0.2,
            bows: None,
        };
        let mut rng = FxRng(7);
        let (from, to) = (Vec3::ZERO, Vec3::new(9.0, 0.0, 0.0));
        let points = jag.points(from, to, &mut rng);
        assert_eq!(points.len(), 10);
        assert_eq!((points[0], points[9]), (from, to));
        // Each its share of the way along, lifted up to 0.3 and moved up to 0.5 across.
        for (index, point) in points.iter().enumerate() {
            assert!((point.x - index as f32).abs() < 1e-4);
            assert!(point.y.abs() <= 0.3 + 1e-4 && point.z.abs() <= 0.5 + 1e-4);
        }
        assert!(points[1..9].iter().any(|point| point.y.abs() > 1e-3));
        // Bowed as the coil's arcs, at most 4/27 of each bow's reach off the way.
        let bowed = Jag {
            points: 24,
            wave: 0.0,
            around: 0.0,
            turn: 0.0,
            jitter: 0.0,
            bows: Some(((3.25, 6.75), (1.25, 4.75))),
        };
        let points = bowed.points(from, to, &mut rng);
        assert_eq!((points.len(), points[0], points[23]), (24, from, to));
        let most = 4.0 / 27.0 * (6.75 + 4.75) * 3f32.sqrt();
        let off = |(index, point): (usize, &Vec3)| point.distance(to * index as f32 / 23.0);
        assert!(
            points
                .iter()
                .enumerate()
                .map(off)
                .all(|off| off <= most + 1e-4)
        );
        assert!(points.iter().enumerate().map(off).any(|off| off > 0.1));
    }

    #[test]
    fn a_trail_lays_one_each_spacing_and_its_share_of_a_tick() {
        let trail: Trail = ron::from_str(
            "(per_tick: 0.25, spacing: 0.1, textures: [], colours: [], size: 1.0, \
             end_size: 1.0, ticks: 1, ticks_to: 1)",
        )
        .unwrap();
        // The N60's bullet flies 1.25 u a tick at 75 u/s: twelve along it.
        assert_eq!(trail.count(1.25, 0.5), 12);
        assert_eq!(trail.count(1.25, 0.2), 13);
        assert_eq!(trail.count(0.0, 0.9), 0);
        assert_eq!(trail.keep, 1.0, "keeps all its speed unless it says");
    }

    #[test]
    fn the_lava_gun_leaves_pools_and_the_infectors_clouds_lie_on_the_ground() {
        let recipes = FxRecipes::default();
        let stream = Some("stream");
        let pool = recipes.mark("lava", 0, stream).expect("a pool");
        assert_eq!((pool.chance, pool.most, pool.hold), (11, 10, 60));
        assert_eq!((pool.flat.len(), pool.flames.len()), (2, 2));
        // From V5 only its glows, in blue.
        let blue = recipes.mark("lava", 4, stream).expect("a pool");
        assert!(blue.flames.is_empty() && blue.flat[0].colour[2] > blue.flat[0].colour[0]);
        assert!(recipes.mark("n60", 0, None).is_none());
        let clouds = |trails: &[Trail]| {
            trails
                .iter()
                .filter(|trail| trail.textures.contains(&Tex::Particle(0x34, 0)))
                .all(|trail| trail.ground == Some(5.0))
        };
        assert!(clouds(recipes.trail("infector", 0, Some("blob"))));
        let rings = |emits: &[Emit]| {
            recipes
                .expand(emits)
                .filter(|emit| matches!(emit, Emit::Model { class: 0x307, .. }))
                .count()
        };
        assert_eq!(rings(recipes.impact("infector", 0, Some("blob"))), 1);
        assert_eq!(rings(recipes.impact("whip", 3, Some("wave"))), 1);
    }

    #[test]
    fn something_put_on_the_ground_lies_on_the_floor_below_it_if_any() {
        use rac_hero::{BodyPlacement, Collider, ParryWorld};
        let mut world = ParryWorld::default();
        world.set_bodies(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(10.0, 0.5, 10.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        )]);
        let world: &dyn CollisionWorld = &world;
        let at = on_ground(Some(world), Vec3::new(1.0, 3.0, 2.0), 5.0).unwrap();
        assert!(at.distance(Vec3::new(1.0, 0.05, 2.0)) < 1e-4, "{at}");
        assert_eq!(on_ground(Some(world), Vec3::new(1.0, 6.0, 2.0), 5.0), None);
        assert_eq!(on_ground(Some(world), Vec3::new(20.0, 1.0, 0.0), 5.0), None);
        let anywhere = Vec3::new(20.0, 1.0, 0.0);
        assert_eq!(on_ground(None, anywhere, 5.0), Some(anywhere));
    }

    #[test]
    fn particles_go_within_their_cone() {
        let mut rng = FxRng(1);
        for _ in 0..200 {
            let way = rng.direction(Some((Vec3::NEG_Z, 20.0)));
            assert!((way.length() - 1.0).abs() < 1e-4);
            assert!(way.dot(Vec3::NEG_Z) >= ops::cos(20f32.to_radians()) - 1e-4);
        }
    }
}
