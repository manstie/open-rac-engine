//! What a game's guns are, read from RON (`assets/uya_weapons.ron` for UYA NTSC-U): each
//! gun's levels, how it fires and its mods, and the rules all its guns share
//! (`docs/research/weapons.md`).

use bevy::prelude::*;
use rac_hero::profile::Spring;
use serde::{Deserialize, Serialize};

/// UYA NTSC-U's six guns whose code is read: the Shock Blaster, N60 Storm, Nitro Launcher,
/// Lava Gun, Annihilator and Flux Rifle.
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
    /// How many targets can burn with acid at once.
    pub acid_slots: usize,
}

/// The shock mod: each hit arcs to another enemy near the target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShockRules {
    /// The arc jumps to the first other living target within this.
    pub range: f32,
    /// The arc does this share of the gun's damage.
    pub share: f32,
}

/// One gun.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponDef {
    pub name: String,
    /// Its name from V5 on.
    pub upgraded_name: String,
    /// The game's item number (`weapons.md`, "Items").
    pub item: u16,
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
    /// V1 to V5, then Mega, Giga and Omega.
    pub levels: Vec<Level>,
    pub mods: ModLevels,
    /// What its acid does, while the acid mod is on. Empty: nothing.
    #[serde(default)]
    pub acid: ByLevel<Acid>,
    pub fire: FireMode,
    #[serde(default)]
    pub effects: Effects,
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

/// Names the game gives the effects and sounds of a gun's events ([`crate::GunEvent`]).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effects {
    pub fire: Option<String>,
    pub impact: Option<String>,
}

/// How a gun fires: the five modes of PLAN.md P1.9.
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
    /// Rays at random within `spread_degrees` of the aim, turned any way round it...
    pub random_rays: u32,
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
    pub push: f32,
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
                "Flux Rifle"
            ]
        );
        for weapon in &armory.weapons {
            assert_eq!(weapon.levels.len(), 8, "{}", weapon.name);
            // V5 and Omega end their chains; Mega is bought.
            assert_eq!(weapon.level(4).xp, 0, "{}", weapon.name);
            assert_eq!(weapon.level(7).xp, 0, "{}", weapon.name);
        }
        assert_eq!(armory.find("Flux Rifle"), Some(WeaponId(5)));
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
