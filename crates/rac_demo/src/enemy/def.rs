//! The demo's enemies as data: each archetype's numbers, from RON
//! (`assets/enemies.ron`).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The bundled definitions.
pub const ENEMIES_RON: &str = include_str!("../../assets/enemies.ron");

/// Which archetype an enemy is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnemyKind {
    /// Runs at the hero and lunges.
    Charger,
    /// Keeps its distance and shoots.
    Shooter,
}

/// Every archetype's definition.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bestiary {
    /// How fast an enemy off the ground falls, u/s².
    pub gravity: f32,
    pub charger: EnemyDef,
    pub shooter: EnemyDef,
}

#[derive(Debug, thiserror::Error)]
#[error("enemy definitions: {0}")]
pub struct BestiaryError(#[from] ron::error::SpannedError);

impl Bestiary {
    pub fn from_ron(text: &str) -> Result<Self, BestiaryError> {
        Ok(ron::from_str(text)?)
    }

    pub fn get(&self, kind: EnemyKind) -> &EnemyDef {
        match kind {
            EnemyKind::Charger => &self.charger,
            EnemyKind::Shooter => &self.shooter,
        }
    }
}

impl Default for Bestiary {
    fn default() -> Self {
        Self::from_ron(ENEMIES_RON).expect("the bundled enemies parse")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyDef {
    /// Its body against walls and crowding, an upright cylinder from its feet.
    pub radius: f32,
    pub height: f32,
    pub hitpoints: f32,
    /// What killing it gives the gun that did it (`rac_combat::KillXp`).
    pub kill_xp: u32,
    pub senses: Senses,
    pub moves: Moves,
    pub attack: Attack,
    /// Ticks it reels from a hit, unable to attack.
    pub hurt_ticks: u32,
    /// Ticks it lies dead before it vanishes, then before it comes back at home.
    pub death_ticks: u32,
    pub respawn_ticks: u32,
    pub drops: DropTable,
}

/// When it takes notice of a hero, and when it gives up on him.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Senses {
    /// It sees a hero this near, within `sight_degrees` either side of its facing, with
    /// nothing in the way.
    pub sight: f32,
    pub sight_degrees: f32,
    /// It hears a hero this near, whichever way it faces.
    pub hearing: f32,
    /// It gives up on a hero this far away, or once it is `leash` from home.
    pub lose: f32,
    pub leash: f32,
    /// After giving up it ignores heroes for this long, unless hit, so it gets home.
    pub calm_ticks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Moves {
    /// Patrolling, and the shooter's keeping its distance.
    pub walk_speed: f32,
    /// Chasing.
    pub run_speed: f32,
    /// u/s², speeding up, slowing down and shedding a hit's push.
    pub accel: f32,
    pub turn_degrees_per_second: f32,
    /// How long it waits at each patrol point, a range of ticks.
    pub wait_ticks: (u32, u32),
}

/// How it attacks, which is what sets the archetypes apart.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Attack {
    /// Runs at the hero, rears back and lunges: the melee charger.
    Lunge(Lunge),
    /// Keeps its distance, aims and fires: the ranged shooter.
    Shoot(Shoot),
}

/// An attack's wind-up, its strike (a lunge or the shots) and its recovery, back to back.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lunge {
    /// It starts the attack with the hero this near, facing within `face_degrees` of him.
    pub range: f32,
    pub face_degrees: f32,
    /// The tell, standing and turning to the hero.
    pub wind_up_ticks: u32,
    /// Straight ahead at `lunge_speed`, hitting a hero whose body comes within `reach`
    /// of its front.
    pub lunge_ticks: u32,
    pub lunge_speed: f32,
    pub reach: f32,
    pub recover_ticks: u32,
    /// After the recovery, before it can start another.
    pub cooldown_ticks: u32,
    pub hit: HeroHit,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Shoot {
    /// It backs off from a hero nearer than `near`, closes in on one beyond `far`, and
    /// strafes between, changing sides every `strafe_ticks`.
    pub near: f32,
    pub far: f32,
    pub strafe_speed: f32,
    pub strafe_ticks: (u32, u32),
    /// It starts the attack with a hero this near that it can see, facing within
    /// `face_degrees` of him.
    pub range: f32,
    pub face_degrees: f32,
    /// The tell: standing and turning to the hero.
    pub aim_ticks: u32,
    /// Shots fired, `shot_every` ticks apart, each along its facing and tilted to the
    /// hero, so a hero who outruns its turn is missed.
    pub shots: u32,
    pub shot_every: u32,
    pub recover_ticks: u32,
    pub cooldown_ticks: (u32, u32),
    /// Where shots leave: above its feet, and ahead of its middle.
    pub muzzle_height: f32,
    pub muzzle_ahead: f32,
    pub shot: ShotDef,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShotDef {
    pub speed: f32,
    pub radius: f32,
    /// Ticks before it fizzles out.
    pub life_ticks: u32,
    pub hit: HeroHit,
}

/// What hitting a hero does (`rac_hero::HurtHero`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeroHit {
    pub damage: u32,
    /// u/s along the attack, and up.
    pub knockback: f32,
    pub lift: f32,
}

/// What it leaves when it dies (`crate::drops`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DropTable {
    /// How many bolts, a range, each worth `bolt_value`.
    pub bolts: (u32, u32),
    pub bolt_value: u32,
    /// The chance of a nanotech orb, which heals `nanotech`.
    pub nanotech_chance: f32,
    pub nanotech: u32,
    /// The chance of an ammo pickup for a gun of the hero who killed it
    /// (`rac_combat::pickups::ammo_for`).
    #[serde(default)]
    pub ammo_chance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_enemies_parse_with_one_attack_each() {
        let bestiary = Bestiary::default();
        assert!(matches!(bestiary.charger.attack, Attack::Lunge(_)));
        assert!(matches!(bestiary.shooter.attack, Attack::Shoot(_)));
    }

    #[test]
    fn a_broken_file_is_an_error() {
        assert!(Bestiary::from_ron("(gravity: 30.0)").is_err());
    }
}
