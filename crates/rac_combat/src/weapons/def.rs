//! What a game's guns are, read from RON (`assets/uya_weapons.ron` for UYA NTSC-U): each
//! gun's levels, how it fires and its mods, and the rules all its guns share
//! (`docs/research/weapons.md`).

use bevy::prelude::*;
use rac_hero::GunPose;
use rac_hero::profile::Spring;
use serde::{Deserialize, Serialize};

/// UYA NTSC-U's twenty guns.
pub const UYA_RON: &str = include_str!("../../assets/uya_weapons.ron");

/// A weapon by its place in [`Armory::weapons`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WeaponId(pub usize);

/// A game's guns and the rules they share. Changing it changes the guns from the next tick.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Armory {
    pub name: String,
    pub rules: GunRules,
    pub weapons: Vec<WeaponDef>,
}

#[derive(Debug, thiserror::Error)]
#[error("weapon definitions: {0}")]
pub struct ArmoryError(#[from] ron::error::SpannedError);

impl Armory {
    pub fn from_ron(text: &str) -> Result<Self, ArmoryError> {
        Ok(ron::from_str(text)?)
    }

    /// The bundled UYA NTSC-U guns.
    pub fn uya() -> Self {
        // A test parses the bundled file, so this cannot fail at run time.
        Self::from_ron(UYA_RON).expect("the bundled UYA weapons parse")
    }

    pub fn get(&self, weapon: WeaponId) -> Option<&WeaponDef> {
        self.weapons.get(weapon.0)
    }

    /// The weapon called `name` (its V1 name).
    pub fn find(&self, name: &str) -> Option<WeaponId> {
        self.weapons
            .iter()
            .position(|weapon| weapon.name == name)
            .map(WeaponId)
    }
}

impl Default for Armory {
    fn default() -> Self {
        Self::uya()
    }
}

fn yes() -> bool {
    true
}

/// What every gun of a game shares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GunRules {
    /// Ticks from the button that brings a gun out until it is in his hand.
    pub draw_ticks: u32,
    /// The quick-select button let go within this many ticks of its press switches guns.
    pub quick_switch_ticks: u32,
    /// How many of the most recently chosen guns the quick switch goes round, of the
    /// `recent_guns` remembered.
    pub quick_switch_guns: usize,
    pub recent_guns: usize,
    /// The level bought rather than reached by XP (Mega): XP starts again from 0 there.
    pub bought_level: u8,
    /// Where the gun's muzzle is from his feet: right, up and forward.
    pub muzzle: [f32; 3],
    /// In the lock-strafe and first-person cameras, and the look, shots aim at the point
    /// the view centres on, looked for this far, bending from his facing by at most
    /// `bend_degrees`, a tenth of that when the point is within `bend_near`.
    pub view_range: f32,
    pub bend_degrees: f32,
    pub bend_near: f32,
    pub shock: ShockRules,
    /// UYA's small bullet, which the agents' guns and the Nitro Eruptor fire; UYA's when a
    /// file leaves it out.
    #[serde(default)]
    pub slug: SlugRules,
    /// How many targets can burn with acid at once.
    pub acid_slots: usize,
    /// How the quick-select ring is worked; UYA's when a file leaves it out.
    #[serde(default)]
    pub ring: super::ring::RingRules,
    /// The wrench's crosshair in the look.
    #[serde(default)]
    pub wrench_sight: Option<Crosshair>,
}

/// The shock mod: each hit arcs to another enemy near the target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShockRules {
    /// The arc jumps to the first other living enemy in sight within this.
    pub range: f32,
    /// The arc does this share of the gun's damage.
    pub share: f32,
    /// It reaches its target, and strikes, this many ticks after the hit.
    pub reach_ticks: u32,
}

/// UYA's small bullet (class 0x1855, `enemy_bullet_spawn` 0x35D8E8, its flight 0x35D9C0):
/// straight on for `life_ticks` while within `leash` of the hero, testing each step's
/// line. What it meets takes a ball of its damage where it meets it, `world_radius` on
/// the world and `target_radius` on a target, pushed by its velocity a tick × `push`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SlugRules {
    pub life_ticks: u32,
    pub leash: f32,
    pub world_radius: f32,
    pub target_radius: f32,
    pub push: f32,
}

impl Default for SlugRules {
    fn default() -> Self {
        Self {
            life_ticks: 180,
            leash: 80.0,
            world_radius: 0.6,
            target_radius: 1.0,
            push: 3.0,
        }
    }
}

/// One gun.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponDef {
    pub name: String,
    /// Its name from V5 on.
    pub upgraded_name: String,
    /// The game's item number (`weapons.md`, "Items").
    pub item: u16,
    /// The rounds one of its ammo pickups holds (`crate::pickups`); 0: it has none.
    #[serde(default)]
    pub pickup_rounds: u32,
    /// Ammo crates throw out its ammo. UYA's never do for the Flux Rifle, Shield Charger,
    /// Agents of Doom and RY3NO (`ammo_crate_gun_allowed`, 0x440E38).
    #[serde(default = "yes")]
    pub crate_ammo: bool,
    /// Fired by holding the button (row +0x30): his walk changes while it fires.
    pub held: bool,
    /// While it fires his walk turns on the profile's steady firing turn.
    pub steady: bool,
    /// Ticks the gun is in his hand before it can fire.
    pub ready_ticks: u32,
    /// Ticks the firing animation plays after each shot, which keeps him `firing`.
    pub fire_ticks: u32,
    /// In the look, ticks it waits before it fires.
    pub look_wait_ticks: u32,
    /// How he holds it and the animations he plays as it fires (row +0x18, +0x24, +0x28
    /// and +0x2C).
    #[serde(default)]
    pub pose: GunPose,
    /// V1 to V5, then Mega, Giga and Omega.
    pub levels: Vec<Level>,
    pub mods: ModLevels,
    /// What its acid does, while the acid mod is on. Empty: nothing.
    #[serde(default)]
    pub acid: ByLevel<Acid>,
    pub fire: FireMode,
    /// The gun's name in a game's tables of sounds and looks (`"n60"`), which its
    /// [`crate::GunEvent`]s and [`crate::weapons::Sustain`] are keyed by.
    #[serde(default)]
    pub effects: String,
    /// What it draws to aim by.
    #[serde(default)]
    pub sight: Sight,
}

/// What a gun draws to aim by (`weapons.md`, "Aim indicators"), for a HUD: the game's
/// effect texture numbers name its images, and sizes are pixels of UYA's 512 × 416 frame.
/// The lock-on reticle is every gun's ([`crate::weapons::Sights`]).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sight {
    #[serde(default)]
    pub crosshair: Option<Crosshair>,
    #[serde(default)]
    pub mark: Mark,
    #[serde(default)]
    pub scope: Option<Scope>,
}

/// The Flux Rifle's scope (0x3444B0; `weapons.md`, "Aim indicators"). In the look, once
/// its view is ready, it zooms the view and draws a scope over the screen, with a ring that
/// blinks while the eye's ray meets a live enemy first and the gun can fire
/// ([`crate::weapons::Sights::scope_on`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scope {
    /// How far the eye's ray reaches.
    pub reach: f32,
    /// The narrowest field of view it zooms to, degrees, by level.
    pub narrowest: ByLevel<f32>,
    /// Radians a tick the zoom moves while a zoom button is held: R1 in and R2 out in the
    /// third-person setting, the left stick up and down in the others.
    pub zoom_step: f32,
    /// Each look starts this share of the way in.
    pub start_share: f32,
    /// The camera's spring to the zoom, and back out when the look ends.
    pub zoom_in: Spring,
    pub zoom_out: Spring,
}

/// A crosshair at the screen's centre.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crosshair {
    pub texture: u16,
    /// Width and height, and how far it is turned.
    pub size: (f32, f32),
    #[serde(default)]
    pub turn_degrees: f32,
    /// It turns red while the view's target weight is above this; none, never.
    pub red_above: Option<f32>,
    /// It pulses between green and white; otherwise it keeps `colour`.
    #[serde(default = "yes")]
    pub pulse: bool,
    /// Its colour when it doesn't pulse (sRGB, 0 to 1).
    #[serde(default)]
    pub colour: Option<[f32; 3]>,
    pub when: Showing,
}

/// When a crosshair shows. Aiming is the look (UYA's state 29) or a camera other than the
/// follow camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Showing {
    Aiming,
    /// Aiming with no lock-on target.
    AimingUnlocked,
    /// Only in the look.
    Look,
}

/// What a gun marks in the world.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Mark {
    #[default]
    None,
    /// The shared target mark on its target ([`crate::weapons::Sights::target`]), or the
    /// lock-on target with `or_lock`: only in the follow camera with `follow_only`, never
    /// in the look with `outside_look`, never with the arm raised against a wall with
    /// `not_raised`, only while it can fire with `ready_only`.
    Target {
        #[serde(default)]
        or_lock: bool,
        #[serde(default)]
        follow_only: bool,
        #[serde(default)]
        outside_look: bool,
        #[serde(default)]
        not_raised: bool,
        #[serde(default)]
        ready_only: bool,
    },
    /// The shared mark on each of its marks below `below_level`, with no lock-on target,
    /// all sharing one spin (the RY3NO's).
    Each { below_level: u8 },
    /// Its marks as downward triangles (texture 0x1F), the newest in red-orange, only
    /// those within `within_degrees` of the view (the Spitting Hydra's,
    /// `hydra_marks_draw` 0x3213A8).
    Triangles { within_degrees: f32 },
}

impl WeaponDef {
    /// The level's numbers, or the last level's past the end.
    pub fn level(&self, level: u8) -> &Level {
        let last = self.levels.len().saturating_sub(1);
        &self.levels[usize::from(level).min(last)]
    }

    pub fn name_at(&self, level: u8) -> &str {
        if level >= 4 {
            &self.upgraded_name
        } else {
            &self.name
        }
    }

    /// The mods its levels turn on at `level`.
    pub fn mods_at(&self, level: u8) -> Mods {
        let on = |from: Option<u8>| from.is_some_and(|from| level >= from);
        Mods {
            lock_on: on(self.mods.lock_on),
            shock: on(self.mods.shock),
            acid: on(self.mods.acid),
        }
    }
}

/// One level of a gun (one of its weapon-table rows).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Level {
    /// Row +0x6C.
    pub damage: f32,
    /// The second damage, row +0x70: a blast's or a splash's.
    #[serde(default)]
    pub splash: f32,
    /// The most ammo it holds, row +0x8A; 0 uses none.
    pub ammo: u32,
    /// The total XP at which it leaves this level, row +0x68; 0 for none.
    pub xp: u32,
    /// What the vendor asks for one round, in bolts, row +0x84 (`crate::pickups::vendor`).
    #[serde(default)]
    pub round_price: u32,
}

/// The level each mod comes on at (0 is V1); `None` never.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModLevels {
    pub lock_on: Option<u8>,
    pub shock: Option<u8>,
    pub acid: Option<u8>,
}

/// The mods a gun has on (UYA's `item_flags` bits 0x20, 0x02 and 0x08).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Mods {
    /// Lock-on is allowed while it is the gun.
    pub lock_on: bool,
    /// Each hit arcs to another enemy near the target.
    pub shock: bool,
    /// Hits leave acid that burns the target.
    pub acid: bool,
}

impl Mods {
    pub fn or(self, other: Mods) -> Mods {
        Mods {
            lock_on: self.lock_on || other.lock_on,
            shock: self.shock || other.shock,
            acid: self.acid || other.acid,
        }
    }
}

/// A value that changes at some levels: each entry holds from its level (0 is V1) until the
/// next entry's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByLevel<T>(pub Vec<(u8, T)>);

impl<T> Default for ByLevel<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Copy> ByLevel<T> {
    pub fn at(&self, level: u8) -> Option<T> {
        self.0
            .iter()
            .rev()
            .find(|(from, _)| *from <= level)
            .map(|&(_, value)| value)
    }
}

/// Acid's burn: health off the target each second, for a time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Acid {
    pub per_second: f32,
    pub seconds: f32,
}

/// How a gun fires: the five archetypes of PLAN.md P1.9, and the modes of UYA's other
/// guns (`weapons.md`, "Each gun").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FireMode {
    /// Rays that hit at once: the Shock Blaster.
    Hitscan(Hitscan),
    /// Shots that fly straight: the N60 Storm's bullets, the Flux Rifle's shot.
    Projectile(Projectile),
    /// Charges thrown in an arc that burst: the Nitro Launcher.
    Lobbed(Lobbed),
    /// A stream of falling points while the button is held: the Lava Gun.
    Stream(Stream),
    /// Missiles that turn after a target: the Annihilator.
    Homing(Homing),
    /// A slow ball that sticks to what it hits and arcs to targets near it: the Plasma
    /// Coil.
    Coil(Coil),
    /// A canister that bursts into bomblets: the Bouncer.
    Cluster(Cluster),
    /// A bouncing blob that infects enemies: the Infector.
    Infect(Infect),
    /// A disc that seeks and bounces: the Disc Blade Gun.
    Disc(Disc),
    /// A volley of missiles at marked targets: the RY3NO.
    Volley(Volley),
    /// Targets marked while held, struck on letting go: the Spitting Hydra.
    Hydra(Hydra),
    /// A hole that swallows weak enemies: the Rift Inducer.
    Rift(Rift),
    /// What it sucks in is what it fires: the Suck Cannon.
    Suck(Suck),
    /// A beam that turns an enemy into a duck: the Qwack-O-Ray.
    Beam(Beam),
    /// Thrown turrets that shoot: the Miniturret Glove.
    Turret(Turret),
    /// Thrown walls: the Holo Shield Glove.
    Barrier(Barrier),
    /// Thrown pods of agents that run at enemies and blow up: the Agents of Doom.
    Agents(Agents),
    /// A shield round the hero: the Shield Charger.
    Shield(ShieldCharger),
    /// The wrench's moves with a rope: the Plasma Whip.
    Whip(Whip),
}

impl FireMode {
    /// The aim assist the gun picks its target with at `level`, if it has one of its own.
    pub fn assist(&self, level: u8) -> Option<AimAssist> {
        match self {
            FireMode::Projectile(fire) => fire.assist,
            FireMode::Lobbed(fire) => Some(fire.assist),
            FireMode::Homing(fire) => Some(fire.assist),
            FireMode::Coil(fire) => Some(fire.assist),
            FireMode::Disc(fire) => fire.assist.at(level),
            FireMode::Volley(fire) => Some(fire.assist),
            FireMode::Hydra(fire) => Some(fire.assist),
            FireMode::Rift(fire) => Some(fire.assist),
            FireMode::Suck(fire) => Some(fire.assist),
            _ => None,
        }
    }
}

/// When a gun may fire again.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Timing {
    /// `rate` shots a second: after a shot, none until `1 / rate` seconds have passed.
    Rate(f32),
    /// While the button is held, a shot at once and then every `ticks`; letting go starts
    /// again.
    Repeat(u32),
}

impl Timing {
    /// Ticks after a shot until the next may fire.
    pub fn ticks(self) -> u32 {
        match self {
            // The game counts the cooldown down by a frame's seconds until it is under 0.
            Timing::Rate(rate) if rate > 0.0 => (60.0 / rate).ceil() as u32,
            Timing::Rate(_) => 0,
            Timing::Repeat(ticks) => ticks,
        }
    }
}

/// The gun aim assist (`weapons.md`, "Gun aim assist"): enemies within `range` inside a cone
/// round the aim, whose half-angles (yaw, pitch, in degrees) go from `near` close by to
/// `far` at range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AimAssist {
    pub range: f32,
    pub near: (f32, f32),
    pub far: (f32, f32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hitscan {
    /// Below `charge`'s level: a shot while the button is held, at this rate, counted from
    /// each shot's hit.
    pub rate: f32,
    /// Ticks from a shot to its hit: `hit_ticks`, or `busy_hit_ticks` while the firing
    /// animation already plays or he moves.
    pub hit_ticks: u32,
    pub busy_hit_ticks: u32,
    /// Ticks after a hit before the next shot.
    pub after_hit_ticks: u32,
    /// A ball at the gun hits what it touches.
    pub ball_radius: f32,
    pub ray_length: f32,
    /// Rays at random within `spread_degrees` of the aim, turned any way round it, by
    /// level...
    pub random_rays: ByLevel<u32>,
    pub spread_degrees: f32,
    /// ...and rays `down_degrees` below it, `down_spread_degrees` apart.
    pub down_rays: u32,
    pub down_degrees: f32,
    pub down_spread_degrees: f32,
    pub push: f32,
    /// From its level on, the button fires once and then charges.
    pub charge: Option<Charge>,
}

/// The Shock Blaster's charged shot from V2.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Charge {
    pub from_level: u8,
    /// Holding the button tries to fire on each of its first `try_ticks`.
    pub try_ticks: u32,
    /// Let go after more than `ticks` held, a charged shot of `damage_scale` times the
    /// damage.
    pub ticks: u32,
    pub damage_scale: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Projectile {
    pub timing: Timing,
    /// u/s, chosen at random between the two for each shot.
    pub speed: (f32, f32),
    /// Ticks a shot flies.
    pub lifetime: ByLevel<u32>,
    /// From this level a shot bounces off whatever isn't a target.
    pub bounce_from: Option<u8>,
    /// A shot that stops damages a ball this big where it stops: `world_radius` on the
    /// world, `target_radius` on a target (0: only the target hit).
    pub world_radius: f32,
    pub target_radius: f32,
    /// A ball at the muzzle that hits what it touches as the shot leaves (0: none).
    pub muzzle_ball: f32,
    /// Shots go from each of these in turn: right, up and forward of the muzzle.
    pub barrels: Vec<[f32; 3]>,
    pub assist: Option<AimAssist>,
    /// In the look, the aim bends to the nearest target within `scope.0` u and `scope.1`
    /// degrees of it.
    pub scope: Option<(f32, f32)>,
    /// Scoped in the look, a beam goes with each shot from the view's eye along the aim.
    #[serde(default)]
    pub scope_beam: Option<ScopeBeam>,
    pub push: f32,
    /// From this level, the N90 Hurricane's timing and bullets.
    pub hurricane: Option<Hurricane>,
}

/// The Flux Rifle's scoped beam (class 0x186F: `flux_scope_beam_spawn` 0x37C818, its update
/// 0x37CB68). Its head flies `step` u a tick for `ticks` ticks, testing the line of each
/// step, and its first hit stops it. Below `pierce_from` one test, whose hit takes a ball
/// of the gun's damage; from it up to `tests` a tick, each passing over only what the one
/// before hit, so it reaches the target behind the first. Each hit's ball is `radius` u
/// and pushes along the aim × `push`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeBeam {
    pub step: f32,
    pub ticks: u32,
    pub pierce_from: u8,
    pub tests: u32,
    pub radius: ByLevel<f32>,
    pub push: ByLevel<f32>,
    /// How far ahead the beam is drawn (`GunEventKind::Ray`).
    pub shown: f32,
}

/// The N90 Hurricane (0x322E68, bullets 0x384068, class 0x1881, their flight 0x384338): a
/// shot every `repeat_ticks` held; a bullet lives the ticks it takes to fly `reach` u plus
/// `extra_ticks`, and its bounce after `bounces` ends it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Hurricane {
    pub from_level: u8,
    pub repeat_ticks: u32,
    pub reach: f32,
    pub extra_ticks: u32,
    pub bounces: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lobbed {
    pub rate: f32,
    /// It lobs at the lock-on target within this, else at the aim assist's.
    pub lock_range: f32,
    pub assist: AimAssist,
    /// With no target, at a point this far ahead.
    pub reach: f32,
    /// The arc: across the ground at `ground_speed`, its upward speed solved to reach the
    /// aim point's height under `aim_gravity`. A launch faster than `max_launch` is cut
    /// to `cut_launch` u/s.
    pub ground_speed: f32,
    pub aim_gravity: f32,
    pub max_launch: f32,
    pub cut_launch: f32,
    /// The charge falls under this, not the gravity it was aimed with.
    pub gravity: f32,
    /// It bursts on a target within `target_radius`, `fuse_ticks` after bouncing off the
    /// world, on the ground once it has flown `ground_after_ticks` (a probe
    /// `ground_probe` down), or at the end of `lifetime`.
    pub target_radius: f32,
    pub fuse_ticks: u32,
    pub ground_after_ticks: u32,
    pub ground_probe: f32,
    pub lifetime: u32,
    pub blast: Blast,
}

/// A blast that flashes, then grows, hitting each target it touches once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blast {
    pub flash_ticks: u32,
    pub grow_ticks: u32,
    pub radius_from: f32,
    pub radius_to: ByLevel<f32>,
    pub push: f32,
    /// From this level, the Nitro Eruptor's blast instead.
    pub eruption: Option<Eruption>,
}

/// The Nitro Eruptor's blast (0x38A668, class 0x1888, its update 0x38A7B8): after
/// `wait_ticks`, for `grow_ticks` a ball growing from `radius.0` to `radius.1` strikes
/// everything in it every tick, pushed `push` away.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Eruption {
    pub from_level: u8,
    pub wait_ticks: u32,
    pub grow_ticks: u32,
    pub radius: (f32, f32),
    pub push: f32,
    /// What it fires as the charge bursts.
    #[serde(default)]
    pub spray: Option<Spray>,
}

/// The Nitro Eruptor's burst of `count` small bullets ([`SlugRules`]), each `damage`, at
/// a random `speed` u/s: one at each target farther than `near` of the first `aimed`
/// within `within` of the burst, the rest any way round, from `down_degrees` below level
/// to `up_degrees` above.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spray {
    pub count: u32,
    pub damage: f32,
    pub speed: (f32, f32),
    pub near: f32,
    pub within: f32,
    pub aimed: usize,
    pub down_degrees: f32,
    pub up_degrees: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    /// The stream's strength springs to `full` while the button is held and to 0 when it
    /// is let go.
    pub full: f32,
    pub rise: Spring,
    pub fall: Spring,
    /// A round every this many ticks held.
    pub ammo_ticks: u32,
    /// Points alive at once; a new one replaces the oldest.
    pub points: usize,
    /// A point leaves at `strength × speed_per_strength` u/s along the aim raised by
    /// `raise`, or tilted `moving_tilt_degrees` down while he moves.
    pub speed_per_strength: f32,
    pub raise: f32,
    pub moving_tilt_degrees: f32,
    pub lifetime: u32,
    pub gravity: f32,
    /// A point that hits a target also does the splash damage to everything within this.
    pub splash_radius: f32,
    /// A ball at the nozzle hits what it touches while the stream runs.
    pub nozzle_radius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Homing {
    pub rate: f32,
    /// The target search, from `assist_height` above his feet: `assist`, or `aimed_assist`
    /// in the look and the lock-strafe camera.
    pub assist: AimAssist,
    pub aimed_assist: AimAssist,
    pub assist_height: f32,
    /// The best candidate becomes the target after `pick_ticks`; another best for
    /// `drop_ticks` drops it.
    pub pick_ticks: u32,
    pub drop_ticks: u32,
    /// With no target, an enemy within `near_radius` of a point `near_ahead` past the
    /// muzzle becomes the missile's.
    pub near_radius: f32,
    pub near_ahead: f32,
    pub speed: f32,
    pub lifetime: u32,
    /// It turns after its target for at most `homing_ticks`, by up to `turn_degrees` a
    /// tick.
    pub homing_ticks: u32,
    pub turn_degrees: f32,
    pub push: f32,
    /// Its blast does the splash damage within this of where it hits.
    pub blast_radius: f32,
    /// The mines its blast scatters, by level: the Bouncer's bomblets, marked as the
    /// Annihilator's (V2–V4; none from V5).
    #[serde(default)]
    pub mines: ByLevel<Mines>,
    /// From V5 (the Decimator), the seekers its blast sends instead.
    #[serde(default)]
    pub seekers: Option<Seekers>,
}

/// A throw at a point (`lob_velocity`, 0x4C3888): the lock-on target's middle, else a
/// point `most` ahead at the height of his feet, kept between `least` and `most` away, the
/// throw crossing the ground at `ground_speed` with its upward speed solved for `gravity`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Throw {
    pub least: f32,
    pub most: f32,
    /// In the look, `most` is this instead.
    pub most_aimed: f32,
    pub ground_speed: f32,
    pub gravity: f32,
}

/// How a thrown thing bounces: off the world `keep` of its speed comes back, and, if
/// `home_range` is set, its ground speed turns at the nearest living enemy within it, at
/// least `home_speed`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounce {
    pub keep: f32,
    pub home_range: Option<f32>,
    pub home_speed: f32,
}

/// The Plasma Coil (0x315638, ball 0x30BF18).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coil {
    /// Held, it fires every `cooldown_ticks`; the ball leaves `spin_ticks` after.
    pub cooldown_ticks: u32,
    pub spin_ticks: u32,
    /// At most this many balls at once.
    pub most: usize,
    pub assist: AimAssist,
    /// Anything this near the muzzle is aimed at.
    pub muzzle_reach: f32,
    pub speed: f32,
    pub lifetime: u32,
    /// The ball's reach for targets.
    pub radius: f32,
    /// Stuck, it arcs `stick_ticks` later to up to `arcs` other targets within `arc_range`,
    /// each arc taking `arc_ticks` to reach its target.
    pub stick_ticks: u32,
    pub arcs: usize,
    pub arc_range: f32,
    pub arc_ticks: u32,
    pub push: f32,
    /// From this level, the Plasma Storm's ball in place of the coil's.
    pub storm: Option<Storm>,
}

/// The Plasma Storm's ball (0x3098F8, class 0x9A9, its update 0x309480): it flies at
/// `speed` for `fly_ticks`, bouncing off the world and targets keeping `bounce_keep`, a
/// target it touches taking the damage; each tick it arcs to the targets within `reach`
/// that it can see (0x30A148), each again only after `again_ticks`, at most `arcs` at a
/// time; then it fades for `fade_ticks` and goes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Storm {
    pub from_level: u8,
    pub speed: f32,
    pub fly_ticks: u32,
    pub fade_ticks: u32,
    pub bounce_keep: f32,
    pub reach: f32,
    pub arcs: usize,
    pub again_ticks: u32,
}

/// The mines the Annihilator's missile scatters as it bursts (`bouncer_bomblet_spawn`,
/// 0x31E2D8, from 0x396A20): `count` bomblets, each `share` of the missile's damage,
/// falling under `gravity`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mines {
    pub count: u32,
    pub share: f32,
    pub gravity: f32,
    pub bomblet: Bomblet,
}

/// The Decimator's seekers: `count` small missiles leaving fanned up to `spread_degrees`
/// either side of the way back towards the hero (the game's camera), each at the nearest
/// living enemy within `range`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Seekers {
    pub from_level: u8,
    pub count: u32,
    pub spread_degrees: f32,
    pub range: f32,
    pub missile: SmallMissile,
}

/// The small missile (class 0xA82, `small_missile_spawn` 0x3441D0, its flight 0x343330),
/// which the Decimator and the Agents of Dread fire: `share` of the gun's damage,
/// starting at a speed between `speed` and gaining `accel` u/s a tick to its `top`.
/// How it turns after its enemy, how long it flies and its blast aren't read:
/// `turn_degrees` a tick, `lifetime` ticks, and what it meets sets off a ball of
/// `blast_radius`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmallMissile {
    pub share: f32,
    pub speed: (f32, f32),
    pub accel: (f32, f32),
    pub top: (f32, f32),
    pub turn_degrees: f32,
    pub lifetime: u32,
    pub blast_radius: f32,
}

/// The Bouncer (0x31C890, canister 0x31D0B0, bomblets 0x31E488).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cluster {
    pub rate: f32,
    pub throw: Throw,
    pub gravity: f32,
    /// The canister bursts on a target within `radius + reach`; after `bounces` bounces it
    /// rolls, keeping `roll_keep` of its speed a tick, and bursts `roll_ticks` later.
    pub radius: f32,
    pub reach: f32,
    pub bounces: u32,
    pub bounce: Bounce,
    pub roll_keep: f32,
    pub roll_ticks: u32,
    /// Ticks it may fly before it bursts anyway.
    pub lifetime: u32,
    pub bomblets: u32,
    pub bomblet: Bomblet,
    /// From this level, the Heavy Bouncer's canister sweep and bomblets.
    pub heavy: Option<Heavy>,
}

/// The Heavy Bouncer (class 0xE9E, the same update as the Bouncer's).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Heavy {
    pub from_level: u8,
    pub radius: f32,
    pub bomblet: Bomblet,
}

impl Cluster {
    /// The canister's sweep and its bomblets at `level`.
    pub fn at(&self, level: u8) -> (f32, &Bomblet) {
        match &self.heavy {
            Some(heavy) if level >= heavy.from_level => (heavy.radius, &heavy.bomblet),
            _ => (self.radius, &self.bomblet),
        }
    }
}

/// One of the Bouncer's bomblets.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bomblet {
    /// Up from the ground, in degrees, and its speed, each at random between the two.
    pub up_degrees: (f32, f32),
    pub speed: (f32, f32),
    pub radius: f32,
    pub bounce: Bounce,
    /// Ticks it lives, at random between the two.
    pub life: (u32, u32),
    /// Armed this many ticks after the burst, plus its number, it bursts on the first
    /// target with hit points within `hit_radius`.
    pub armed_ticks: u32,
    pub hit_radius: f32,
}

/// The Infector (0x3688E8, blob 0x3DE4F0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Infect {
    pub cooldown_ticks: u32,
    /// It leaves this far below the muzzle.
    pub below: f32,
    pub speed: f32,
    pub gravity: f32,
    /// The share of its speed it keeps each tick.
    pub drag: f32,
    pub radius: f32,
    pub bounce: Bounce,
    /// Ticks it lives, at random between the two; slower than `rest_speed` it bursts.
    pub life: (u32, u32),
    pub rest_speed: f32,
    /// What it touches: an enemy with `weak_health` or less is infected as it flies on;
    /// any other target with hit points bursts it.
    pub touch: f32,
    pub weak_health: f32,
    /// The burst infects the enemies within `burst_radius`, and a blast of `blast_damage`
    /// within `blast_radius`.
    pub burst_radius: f32,
    pub blast_radius: f32,
    pub blast_damage: f32,
    pub blast_push: f32,
    /// An infection lasts the gun's damage / the enemy's hit points × this many ticks.
    pub infect_ticks: f32,
    /// From this level, the Infecto-Bomb in place of the blob.
    pub bomb: Option<InfectBomb>,
}

/// The Infecto-Bomb (0x389F58, bomb class 0x188A 0x38E3A0, burst class 0x188B 0x38EF90):
/// lobbed (`lob_velocity`), it bursts where it first meets something, infecting every
/// enemy within `burst_radius`, and a blast of the blob's damage within `blast_radius`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InfectBomb {
    pub from_level: u8,
    pub throw: Throw,
    pub burst_radius: f32,
    pub blast_radius: f32,
}

/// The Disc Blade Gun (0x36EFC0, disc 0x36FA78).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disc {
    pub cooldown_ticks: u32,
    pub assist: ByLevel<AimAssist>,
    pub speed: f32,
    /// It leaves turned this far to his side, or `side_aimed_degrees` with a target, and
    /// curves `curve_degrees` a tick.
    pub side_degrees: f32,
    pub side_aimed_degrees: f32,
    pub curve_degrees: f32,
    pub lifetime: ByLevel<u32>,
    /// It goes this far from the hero.
    pub leash: f32,
    pub radius: f32,
    pub push: f32,
    /// From these levels each bounce turns it at an enemy within `retarget_range`, and it
    /// bounces off enemies too.
    pub retarget_from: Option<u8>,
    pub through_from: Option<u8>,
    pub retarget_range: f32,
    /// From this level its end is a blast of `share` of its damage within `radius`.
    pub end_blast: Option<EndBlast>,
    /// From this level (the Multi-Disc Gun) a bounce off an enemy may split it.
    pub split: Option<Split>,
}

/// The Multi-Disc Gun's split (0x36FA78): bouncing off an enemy while fewer than `most`
/// discs are out, its nth split has odds of 1 in 2^((n − 1) / 2) of a new disc, `jitter`
/// added to each axis of its velocity a frame; the same odds again keep it going, else
/// it ends.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub from_level: u8,
    pub most: usize,
    pub jitter: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EndBlast {
    pub from_level: u8,
    pub radius: f32,
    pub share: f32,
}

/// The RY3NO (0x3805B0): a volley of missiles at the targets it marks (V1's 0x382E18,
/// later levels' 0x3DAEA8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Volley {
    /// Shots a second; slower by `slow` from `slow_from`.
    pub rate: f32,
    pub slow_from: Option<u8>,
    pub slow: f32,
    pub missiles: ByLevel<u32>,
    pub assist: AimAssist,
    /// u/s, times the level's share.
    pub speed: f32,
    pub speed_share: ByLevel<f32>,
    pub lifetime: ByLevel<u32>,
    /// Each missile spirals round the line it sets off along: tilted off it a random
    /// `tilt` (degrees, by level), its roll turning a random `spin` degrees a tick either
    /// way.
    pub tilt: ByLevel<(f32, f32)>,
    pub spin: (f32, f32),
    /// Each waits up to this many ticks (random) before it flies.
    pub wait: ByLevel<u32>,
    /// Within this of its target's place it also steps straight at it.
    pub home_within: f32,
    /// It goes, with no blast, when its time is up or it is this far from the hero.
    pub leash: f32,
    /// What it flies into takes the damage, pushed along its flight tipped up by this.
    pub push_up: f32,
    pub blast: ByLevel<RocketBlast>,
    /// From this level the volley goes at a point `ahead` along his facing and `up`.
    pub ahead_from: Option<u8>,
    pub ahead: f32,
    pub up: f32,
}

/// A RY3NO missile's burst.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RocketBlast {
    pub radius: f32,
    /// V1's does the splash to all but what the missile hit, pushed `push.0` out and
    /// `push.1` up (0x383CC0); later levels' the damage to all, pushed `push.0` along its
    /// flight (0x3DC180).
    pub splash: bool,
    pub push: (f32, f32),
}

/// The Spitting Hydra (0x31FB90).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hydra {
    pub most_targets: ByLevel<usize>,
    /// While held, it marks a target every this many ticks, and at `max_hold`.
    pub mark_every: ByLevel<u32>,
    pub max_hold: u32,
    /// Let go before this, it fizzles.
    pub tap_ticks: u32,
    pub assist: AimAssist,
    /// Each marked target is struck within `strike_radius` of its middle.
    pub strike_radius: f32,
    pub push: f32,
    /// The bolt that strikes it (0x34DAA0), lasting `bolt_ticks`, does this share of the
    /// damage again as it strikes ([`BOLT_STRIKE_TICKS`](super::shots::BOLT_STRIKE_TICKS)
    /// in).
    pub bolt_share: f32,
    pub bolt_ticks: u32,
    /// With none marked, the spit: the level's most targets of arcs reach for
    /// `spit_ticks` to points `spit_reach` along his aim, each `spit_jitter` off it
    /// (0x321108). At `spit_strike` ticks before the end (0x321580) each that's within
    /// `spit_range` and meets something on the way does the damage to what it meets,
    /// pushed `spit_push` along it, and a blast of `spit_blast` round its end.
    pub spit_ticks: ByLevel<u32>,
    pub spit_reach: f32,
    pub spit_jitter: f32,
    pub spit_strike: u32,
    pub spit_range: f32,
    pub spit_push: f32,
    pub spit_blast: ByLevel<f32>,
}

/// The Rift Inducer (0x372990, rift 0x373990).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rift {
    pub cooldown_ticks: u32,
    pub assist: AimAssist,
    pub speed: (f32, f32),
    /// The share of its speed it keeps each tick.
    pub keep: f32,
    pub fly_ticks: u32,
    /// What it meets as it flies is knocked by its velocity a frame times this.
    pub knock: f32,
    pub open_ticks: u32,
    pub leash: f32,
    /// Open, it takes an enemy within `reach` whose hit points are at most `size` × 2 × the
    /// gun's damage, pulling it in over `pull_ticks`.
    pub reach: f32,
    pub size: f32,
    pub pull_ticks: u32,
    /// From this level, the Rift Ripper's bolts.
    pub ripper: Option<Ripper>,
}

/// The Rift Ripper (0x373990 from V5): open, each tick each enemy within its size +
/// `reach` has a 1 in `odds` chance of a bolt (0x34DAA0), lasting `bolt_ticks`, striking
/// it for `share` of the rift's damage, pushed `push`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ripper {
    pub from_level: u8,
    pub reach: f32,
    pub odds: u32,
    pub share: f32,
    pub bolt_ticks: u32,
    pub push: f32,
}

/// The Suck Cannon (0x3619C8), and what it fires (0x382830, its flight 0x381B98).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suck {
    pub capacity: ByLevel<u32>,
    /// Every `grab_every` ticks held, balls along its vortex (how far, how big), which runs
    /// `vortex` along its aim, take what they touch: an enemy with at most
    /// `most_hitpoints`, or anything else with hit points; a bigger enemy takes
    /// `grab_damage`.
    pub grab_every: u32,
    pub vortex: f32,
    pub grabs: Vec<(f32, f32)>,
    pub most_hitpoints: f32,
    pub grab_damage: f32,
    pub cooldown_ticks: u32,
    pub assist: AimAssist,
    /// u/s; with a target it is also pulled `pull` u/s at it.
    pub speed: f32,
    pub pull: f32,
    /// After this it bursts in a blast of `end_radius`.
    pub lifetime: u32,
    pub end_radius: f32,
    /// The line of its step, or a ball of `sweep` round it, meeting a target bursts it in
    /// a blast of `radius` pushed by its velocity a frame times `target_push`; meeting the
    /// world, `world_push`, or from `ricochet_from` it bounces off.
    pub sweep: f32,
    pub radius: f32,
    pub target_push: f32,
    pub world_push: f32,
    pub ricochet_from: Option<u8>,
}

/// The Qwack-O-Ray (0x37DEB0, beam 0x37D7D8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub range: f32,
    pub cone_degrees: f32,
    /// Or one within `near`, `near_degrees` of his facing and `near_height` of his feet.
    pub near: f32,
    pub near_degrees: f32,
    pub near_height: f32,
    /// A new target is held at least this long.
    pub hold_ticks: u32,
    /// What its target becomes, by level: the enemy is gone and a duck stands there.
    pub duck: ByLevel<Duck>,
    /// The eggs the ducks lay (class 0x1C59, 0x3D4768, its flight 0x3D48B8).
    pub egg: Egg,
}

/// A duck (V1–V3 class 0x1BC3, 0x3D2480, 0x3D1728; V4 class 0x189B, 0x39E5D8, 0x39DB60;
/// the Qwack-O-Blitzer's class 0x1D3E, 0x3D8840, 0x3D7998).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Duck {
    /// It waddles about at `speed`, turning a new way every `turn_every` ticks, until it
    /// goes in a puff after `ticks`; with `eggs` it lays them as it goes.
    Waddle {
        speed: f32,
        turn_every: (u32, u32),
        ticks: u32,
        eggs: bool,
    },
    /// It waddles, and every `look_every` ticks a `look_chance` sends it running at
    /// `speed` at the nearest enemy, laying eggs; within `reach` of it, it blows up, a ball
    /// of `radius` taking `share` of the damage.
    Soldier {
        waddle: f32,
        speed: f32,
        turn_every: (u32, u32),
        look_every: u32,
        look_chance: f32,
        reach: f32,
        radius: f32,
        share: f32,
    },
    /// It flies with the hero, after `wait` ticks, and at an enemy within `sight` in view
    /// at `speed`; within `reach` it pecks, a ball of `peck_radius` taking `share` of the
    /// damage each tick, and after `pecks` it blows up, a ball of `radius` taking `share`.
    Blitzer {
        wait: u32,
        sight: f32,
        speed: f32,
        /// Back to the hero, and about him.
        back_speed: f32,
        near_speed: f32,
        /// Where it keeps: this high over him, this far behind.
        over: f32,
        behind: f32,
        reach: f32,
        peck_radius: f32,
        pecks: u32,
        radius: f32,
        share: f32,
    },
}

/// A duck's egg: laid every `every` ticks behind it, `back_degrees` either side of
/// straight back, `up_degrees` up, at `speed`; it falls under `gravity`, bouncing, and
/// lives `life`. An enemy within `reach` sets it off for the gun's damage, pushed `push`.
/// From `seek_from` each bounce turns it at the nearest enemy within `seek_range`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Egg {
    pub every: (u32, u32),
    pub back_degrees: f32,
    pub up_degrees: (f32, f32),
    pub speed: (f32, f32),
    pub height: f32,
    pub gravity: f32,
    pub bounce_keep: f32,
    pub life: (u32, u32),
    pub reach: f32,
    pub push: f32,
    pub seek_from: Option<u8>,
    pub seek_range: f32,
    pub seek_speed: f32,
}

/// The Miniturret Glove (0x346078, turret 0x32AFB0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turret {
    pub deploy: Deploy,
    pub gravity: f32,
    /// Standing, it unfolds for `unfold_ticks`, then looks within `range` all round.
    pub unfold_ticks: u32,
    pub range: f32,
    pub barrel_height: f32,
    /// Its turn to a target: a share growing this much a tick, firing past `fire_share`.
    pub turn_step: f32,
    pub fire_share: f32,
    pub rate: ByLevel<f32>,
    pub shots: ByLevel<u32>,
    pub speed: f32,
    pub life_ticks: u32,
    pub leash: f32,
    pub push: f32,
    /// From this level, the Megaturret's missiles and its walk back to the hero.
    pub mega: Option<Mega>,
}

/// The Megaturret (class 0x1367, 0x32DE00): every `every_ticks` it fires a guided missile
/// (0x3441D0, class 0xA82, its flight 0x343330) from `speed.0` gaining `accel` u/s a tick
/// to `speed.1`, turning up to `turn_degrees` a tick after its target, living
/// `missile_ticks`; what it meets sets it off, a ball of `blast_radius` taking the damage.
/// When the hero is more than `beyond` away with an enemy within `beyond` of him, every
/// `walk_every` ticks it walks at `walk_speed` to a point `round` from him.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mega {
    pub from_level: u8,
    pub every_ticks: u32,
    pub speed: (f32, f32),
    pub accel: f32,
    pub turn_degrees: f32,
    pub missile_ticks: u32,
    pub blast_radius: f32,
    pub beyond: f32,
    pub walk_every: (u32, u32),
    pub round: (f32, f32),
    pub walk_speed: f32,
}

/// How a glove throws: `throw_ticks` after the press it leaves, and the glove is ready
/// `ready_ticks` later, at least `gap_ticks` since the last; at most `most` out at once.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Deploy {
    pub throw: Throw,
    pub throw_ticks: u32,
    pub ready_ticks: u32,
    pub gap_ticks: u32,
    pub most: usize,
}

/// The Holo Shield Glove (0x3717A8, shield 0x376B88).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Barrier {
    pub deploy: Deploy,
    pub gravity: f32,
    /// Where it lands, a ball of `land_radius` takes `land_damage`.
    pub land_radius: f32,
    pub land_damage: f32,
    pub land_push: f32,
    pub stand_ticks: u32,
    pub fold_ticks: u32,
    pub leash: f32,
    pub hits: u32,
    /// The wall it stands as: half its width, its height.
    pub half_width: f32,
    pub height: f32,
    /// From this level each hit sends a bolt (0x34DAA0) at the first living enemy within
    /// `bolt_range`, `bolt_after` ticks later, which lasts `bolt_ticks` and strikes for the
    /// gun's damage, pushed `bolt_push`.
    pub bolt_from: Option<u8>,
    pub bolt_range: f32,
    pub bolt_after: u32,
    pub bolt_ticks: u32,
    pub bolt_push: f32,
    /// From this level, in place of the bolt, its pulses.
    pub pulse: Option<Pulse>,
    /// From this level, while it has hits left and he is hurt, it links to everything
    /// within `drain_reach` (0x37BB28). While an enemy is linked (0x37BDC0): every
    /// `drain_every` ticks a linked enemy takes `drain_damage`, and with him linked and
    /// within `heal_reach`, every `heal_every` ticks he gains a point of health for one
    /// of its hits.
    pub heal_from: Option<u8>,
    pub drain_reach: f32,
    pub drain_every: u32,
    pub drain_damage: f32,
    pub heal_reach: f32,
    pub heal_every: u32,
}

/// The Ultrashield's pulses (0x372338, class 0x1869, its flight 0x371F48): every `every`
/// ticks of a hit's `react_ticks`, one leaves at the first enemy within `range` it can
/// see, for `share` of the damage, gaining `accel` u/s a tick; it strikes the first thing
/// it meets, pushed `push` along its way, and fades over its last ticks from `fade_from`
/// to `ticks`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pulse {
    pub from_level: u8,
    pub react_ticks: u32,
    pub every: u32,
    pub range: f32,
    pub share: f32,
    pub accel: f32,
    pub fade_from: u32,
    pub ticks: u32,
    pub push: f32,
}

/// The Agents of Doom (0x35C9F0, pod 0x35CEC8, agent 0x3BD298).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agents {
    pub deploy: Deploy,
    pub gravity: f32,
    pub per_pod: u32,
    /// From this level they come to the hero at once instead of from a pod.
    pub direct_from: Option<u8>,
    /// Out from where they are let out, along his facing and sideways.
    pub spread: f32,
    pub sight: AgentSight,
    pub moves: AgentMoves,
    /// It ends when a ball of `touch`, `touch_height` above its feet, touches its target,
    /// when it is within `reach` of the target's feet, or `chase_ticks` into the chase.
    pub touch: f32,
    pub touch_height: f32,
    pub reach: f32,
    pub chase_ticks: u32,
    /// It also ends `life_ticks` after it comes out, or this far from the hero.
    pub life_ticks: u32,
    pub leash: f32,
    /// How it ends, by level (0x3C2140).
    pub end: ByLevel<AgentEnd>,
    /// Its gun, by level (`agent_tick` 0x3C0128); none at V1.
    #[serde(default)]
    pub gun: ByLevel<AgentGun>,
}

/// How an agent finds its target (`target_search_level` 0x44CD90, scoring 0x44CC18): as
/// it comes out, the best of anything with hit points within `first`; then each tick
/// without a target, the best enemy within `range` that no agent of the hero's has
/// claimed. Both take only targets within `level_degrees` of level, from its feet to the
/// target's, and score them by distance × (1 + the angle of their bearing off
/// `bearing_degrees`, a bearing fixed in the world). The one found is claimed for
/// `claim_ticks` (`target_claim`, 0x4C4640).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AgentSight {
    pub first: f32,
    pub range: f32,
    pub level_degrees: f32,
    pub bearing_degrees: f32,
    pub claim_ticks: u32,
}

/// How an agent walks (`walker_set_params` 0x458A70, run by 0x4577D0): it turns towards
/// where it goes, and moves only while facing within `facing_degrees` of it, slowing to
/// stop `stop` short of it (`wander_stop` wandering).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AgentMoves {
    pub follow: Gait,
    pub chase: Gait,
    pub wander: Gait,
    pub facing_degrees: f32,
    pub stop: f32,
    pub wander_stop: f32,
    /// Idle, it follows the hero once he is farther than `follow_beyond`; following, it
    /// idles again within a distance drawn from `idle_within` as it sets off.
    pub follow_beyond: f32,
    pub idle_within: (f32, f32),
    /// Idle with no target, on these ticks of each `idle_cycle` (its idle animations'
    /// frames 30, 35 and 59), one time in `wander_odds` it wanders to a random point
    /// `wander_round` from him; wandering, it follows once he is farther than
    /// `wander_leave`.
    pub wander_ticks: [u32; 3],
    pub idle_cycle: u32,
    pub wander_odds: u32,
    pub wander_round: (f32, f32),
    pub wander_leave: f32,
}

/// A walker's acceleration (u/s²) and top speed (u/s, drawn between the two for each
/// agent), and its turn's acceleration and top speed (degrees a tick² and a tick).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Gait {
    pub accel: f32,
    pub top: (f32, f32),
    pub turn_accel: f32,
    pub turn_top: f32,
}

/// An agent's gun (`agent_tick`, 0x3C0128).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AgentGun {
    /// Small bullets ([`SlugRules`]) of `share` of the gun's damage at `speed` u/s, at the
    /// best enemy within `range` all round (the level search, bearing `bearing_degrees`)
    /// in a clear line from `height` above its feet, the aim jittered up to `jitter` u
    /// each way: first `first_ticks` after it comes out, then every `every_ticks`, while
    /// its `rounds` last, each agent's own or `shared` by the hero's agents and filled
    /// again as each comes out.
    Bullets {
        range: f32,
        bearing_degrees: f32,
        height: f32,
        jitter: f32,
        speed: f32,
        share: f32,
        first_ticks: u32,
        every_ticks: u32,
        rounds: u32,
        shared: bool,
    },
    /// The small missile at an enemy within `range` it can see from `height`, every
    /// `every` ticks (drawn between the two), from the hero's agents' shared `rounds`;
    /// only while it keeps by him (the game's hovering state 7).
    Missiles {
        range: f32,
        height: f32,
        every: (u32, u32),
        rounds: u32,
        missile: SmallMissile,
    },
}

/// How an agent ends (0x3C2140).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AgentEnd {
    /// A blast of the gun's damage within `radius`, pushed `push` out.
    Blast { radius: f32, push: f32 },
    /// A comet (0x395480, class 0x1893, its flight 0x394068) at a random `speed` u/s at
    /// the target, else along his facing, living `life` ticks; what it meets is struck by
    /// a blast of the gun's damage within `radius`, pushed `push` out.
    Comet {
        speed: (f32, f32),
        life: u32,
        radius: f32,
        push: f32,
    },
    /// The Agents of Dread's blast (0x3914C0, class 0x188F): it grows to `radius` over
    /// `ticks`, striking what it reaches, pushed `push` out.
    Grow { radius: f32, ticks: u32, push: f32 },
}

/// The Shield Charger (0x3410E8, firing 0x340DD0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShieldCharger {
    /// Its points at each level.
    pub points: ByLevel<f32>,
    /// While it is up, every `zap_ticks` a ball of `zap_radius`, `height` above his feet,
    /// strikes the enemies touching it.
    pub height: f32,
    pub zap_ticks: u32,
    pub zap_radius: f32,
    pub push: f32,
    /// From this level, in place of the zaps, the Tesla Barrier's bolts.
    pub tesla: Option<Tesla>,
}

/// The Tesla Barrier (0x3410E8 from V5): a tick at a time, while fewer than `most` bolts
/// are out, a new one (0x34DAA0) strikes an enemy within `reach` that has none, for the
/// damage, pushed `push`, and lasts a random `bolt_ticks`; after `bolts` bolts the shield
/// goes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tesla {
    pub from_level: u8,
    pub most: usize,
    pub reach: f32,
    pub bolt_ticks: (u32, u32),
    pub push: f32,
    pub bolts: u32,
}

/// The Plasma Whip (0x356110): the wrench's moves on the fire button, hitting along a
/// rope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Whip {
    /// The rope: `links` pieces, each the stretch long (swings 1 and 2, swing 3, the jump
    /// attack, the throw), each after the first starting `lead` along the link at the
    /// last one's end; it hits to `tip` past its last end.
    pub links: u32,
    pub stretch: (f32, f32, f32, f32),
    pub lead: f32,
    pub tip: f32,
    /// It hits from these ticks into the combo and the jump attack.
    pub combo_from: u32,
    pub jump_from: u32,
    pub push: f32,
    pub lift: f32,
    /// From this level the jump attack's shock wave.
    pub wave: Option<Wave>,
    /// From this level the combo's cutting waves.
    pub cut: Option<Cut>,
}

/// The Quantum Whip's cutting wave (class 0x1CB8, 0x3D6578, its flight 0x3D6740).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cut {
    pub from_level: u8,
    /// It leaves `at_ticks` into swings 1, 2 and 3, along his facing turned by `turns`
    /// degrees, at `speed`, living `ticks`.
    pub at_ticks: [u32; 3],
    pub turns: [f32; 3],
    pub speed: f32,
    pub ticks: u32,
    /// Each tick the line across it, `width` long, strikes what it crosses for the damage,
    /// pushed by its velocity a frame times `push`.
    pub width: f32,
    pub push: f32,
}

/// The Plasma Whip's shock wave (class 0x1890, 0x393690).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wave {
    pub from_level: u8,
    /// It leaves `at_tick` into the jump attack, along his facing at `speed`, living
    /// `ticks`; from `first` every `every` ticks a ray `probe` down from where it was
    /// `behind` ticks before finds the ground, and a ball of `radius` there takes `share`
    /// of the damage.
    pub at_tick: u32,
    pub speed: f32,
    pub ticks: u32,
    pub first: u32,
    pub every: u32,
    pub behind: u32,
    pub probe: f32,
    pub radius: f32,
    pub share: f32,
    pub push: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_guns_parse_with_every_level() {
        let armory = Armory::uya();
        let names: Vec<&str> = armory.weapons.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Shock Blaster",
                "N60 Storm",
                "Nitro Launcher",
                "Lava Gun",
                "Annihilator",
                "Flux Rifle",
                "Infector",
                "Spitting Hydra",
                "Disc Blade Gun",
                "Agents of Doom",
                "Rift Inducer",
                "Holo Shield Glove",
                "Plasma Whip",
                "Suck Cannon",
                "Qwack-O-Ray",
                "RY3NO",
                "Plasma Coil",
                "Bouncer",
                "Miniturret Glove",
                "Shield Charger",
            ]
        );
        for weapon in &armory.weapons {
            // The RY3NO has V1 to V5 only.
            let levels = if weapon.name == "RY3NO" { 5 } else { 8 };
            assert_eq!(weapon.levels.len(), levels, "{}", weapon.name);
            // V5 and Omega end their chains; Mega is bought.
            assert_eq!(weapon.level(4).xp, 0, "{}", weapon.name);
            assert_eq!(weapon.level(7).xp, 0, "{}", weapon.name);
        }
        assert_eq!(armory.find("Flux Rifle"), Some(WeaponId(5)));
        // Each game item is one gun.
        let mut items: Vec<u16> = armory.weapons.iter().map(|w| w.item).collect();
        items.sort_unstable();
        items.dedup();
        assert_eq!(items.len(), armory.weapons.len());
    }

    #[test]
    fn rates_become_the_ticks_the_games_cooldowns_last() {
        // 0.5 s, 1 / 1.7 s (35.3 frames, so under 0 on the 36th), 2 s and 1 s.
        assert_eq!(Timing::Rate(2.0).ticks(), 30);
        assert_eq!(Timing::Rate(1.7).ticks(), 36);
        assert_eq!(Timing::Rate(0.5).ticks(), 120);
        assert_eq!(Timing::Rate(1.0).ticks(), 60);
        assert_eq!(Timing::Repeat(14).ticks(), 14);
    }

    #[test]
    fn values_by_level_hold_from_their_level_on() {
        let lifetime = ByLevel(vec![(0, 30), (1, 46)]);
        assert_eq!(lifetime.at(0), Some(30));
        assert_eq!(lifetime.at(1), Some(46));
        assert_eq!(lifetime.at(7), Some(46));
        assert_eq!(ByLevel::<u32>::default().at(3), None);
    }

    #[test]
    fn mods_come_on_at_their_levels() {
        let armory = Armory::uya();
        let blaster = &armory.weapons[0];
        assert_eq!(blaster.mods_at(1), Mods::default());
        assert!(blaster.mods_at(2).lock_on && !blaster.mods_at(2).shock);
        assert!(blaster.mods_at(3).shock);
        assert_eq!(blaster.name_at(3), "Shock Blaster");
        assert_eq!(blaster.name_at(4), "Shock Cannon");
    }
}
