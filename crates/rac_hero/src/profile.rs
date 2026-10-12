//! Movement profiles: every number the controller uses, loaded from RON.
//!
//! Units are per second at 60 Hz: speeds in u/s, accelerations in u/s², frame counts at
//! 60 Hz. Turning springs keep the games' per-frame gains, which are per tick here because
//! the kit also ticks at 60 Hz. The bundled profiles say in comments where each value comes
//! from (`docs/research/hero-controller.md` and its status labels).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// UYA NTSC-U's tuning, the kit's default (PLAN.md section 5, 2026-10-12).
pub const UYA_RON: &str = include_str!("../assets/uya.ron");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeroProfile {
    pub name: String,
    pub body: BodyProfile,
    pub stick: StickProfile,
    pub walk: WalkProfile,
    pub air: AirProfile,
    pub fall: FallProfile,
    pub jump: JumpParams,
    pub run_jump: JumpParams,
    pub long_jump: LongJumpProfile,
    pub flip: FlipProfile,
    pub heli_jump: HeliJumpProfile,
    pub double_jump: DoubleJumpProfile,
    pub glide: GlideProfile,
}

/// The collision capsule. The hero's origin is at his feet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyProfile {
    pub radius: f32,
    /// Height of the lower sphere's centre on the ground. The capsule floats above the
    /// ground by `bottom - radius`, which is also the highest step walked up without
    /// jumping.
    pub bottom: f32,
    /// Height of the lower sphere's centre in the air.
    pub bottom_airborne: f32,
    /// Height of the upper sphere's centre.
    pub top: f32,
    /// The steepest walkable ground, in degrees.
    pub slope_limit_degrees: f32,
    /// How far below his feet ground is still found while walking, so he follows slopes
    /// and steps down instead of taking off.
    pub ground_snap: f32,
}

/// How the movement stick is read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StickProfile {
    /// Counts from centre each axis ignores.
    pub dead_zone: f32,
    /// Counts past the dead zone that reach full output.
    pub live_range: f32,
    /// Stick vectors shorter than this are ignored.
    pub cut_off: f32,
}

/// A damped spring for turning: each tick the rate gains `gain × error`, loses
/// `damping × rate`, and is capped at `cap` radians per tick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TurnSpring {
    pub gain: f32,
    pub damping: f32,
    pub cap: f32,
}

/// A turning spring whose gain and cap scale with the stick magnitude m.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScaledTurnSpring {
    pub gain_per_m: f32,
    pub gain_min: f32,
    pub gain_max: f32,
    pub damping: f32,
    pub cap_per_m: f32,
    pub cap_min: f32,
    pub cap_max: f32,
}

impl ScaledTurnSpring {
    pub fn at(&self, magnitude: f32) -> TurnSpring {
        TurnSpring {
            gain: (self.gain_per_m * magnitude).clamp(self.gain_min, self.gain_max),
            damping: self.damping,
            cap: (self.cap_per_m * magnitude).clamp(self.cap_min, self.cap_max),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkProfile {
    pub speed: f32,
    pub accel: f32,
    /// Braking towards a lower target speed.
    pub brake: f32,
    /// Below this speed the walk is "setting off" and turns with `turn_setting_off`.
    pub setting_off_below: f32,
    pub turn_setting_off: ScaledTurnSpring,
    pub turn_moving: TurnSpring,
    pub turn_skidding: TurnSpring,
    pub skid_brake: f32,
    /// Ticks of skid braking after the stick is released.
    pub stop_skid_ticks: u32,
    /// The most speed left when the hero settles into idle.
    pub stop_residual: f32,
    pub stop_brake: f32,
    /// A stick this far round from the facing (radians) at more than `skid_above` u/s
    /// starts a skid.
    pub skid_angle: f32,
    pub skid_above: f32,
    pub crouch_brake: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirProfile {
    /// The downward speed during push-off, as the game writes it (u/s²); it is applied as a
    /// velocity of `pushoff_press / 60` u/s.
    pub pushoff_press: f32,
    pub pushoff_towards_cap: f32,
    pub pushoff_brake: f32,
    pub fall_speed_cap: f32,
    pub control_accel: f32,
    /// The steering step grows by `1.5 × (1 − cos θ)`, clamped to these bounds.
    pub control_factor_min: f32,
    pub control_factor_max: f32,
    pub release_below: f32,
    pub release_brake: f32,
    /// Vertical speed below which a jump is past its apex.
    pub apex_below: f32,
    pub turn: TurnSpring,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FallProfile {
    pub gravity: f32,
    pub air_speed: f32,
    pub release_brake: f32,
}

/// One jump-type move's parameters (`hero-controller.md`, "Jump parameters by state").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JumpParams {
    pub height_min: f32,
    pub height_max: f32,
    pub hold_ticks: u32,
    pub pushoff_ticks: u32,
    pub gravity: f32,
    /// Gravity is multiplied by this after the apex.
    pub apex_gravity_factor: f32,
    pub air_speed: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongJumpProfile {
    pub params: JumpParams,
    /// The ground speed a crouch must still have for a jump to become a long jump.
    pub min_speed: f32,
    /// Added along the facing on push-off tick t, as `t × forward_push`.
    pub forward_push: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlipProfile {
    pub params: JumpParams,
    pub speed: f32,
    pub min_speed: f32,
    pub start_speed: f32,
    pub accel: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeliJumpProfile {
    pub params: JumpParams,
    pub thrust: f32,
    pub thrust_from_tick: u32,
    pub thrust_to_tick: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoubleJumpProfile {
    pub gravity: f32,
    pub boost_accel: f32,
    pub cap: f32,
    pub boost_ticks: u32,
    pub hold_ticks: u32,
    pub air_speed: f32,
    /// Above `high_above` u over the first take-off the cap is multiplied by `high_factor`,
    /// and above `higher_above` by `higher_factor`.
    pub high_above: f32,
    pub high_factor: f32,
    pub higher_above: f32,
    pub higher_factor: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlideProfile {
    /// A double jump turns into a glide once falling faster than this, u/s.
    pub enter_below: f32,
    pub sink: f32,
    /// The spring easing the vertical speed towards the sink, per tick.
    pub spring: TurnSpring,
    pub speed: f32,
    pub accel: f32,
    pub brake: f32,
}

#[derive(Debug, Error)]
#[error("invalid hero profile: {0}")]
pub struct ProfileError(#[from] ron::error::SpannedError);

impl HeroProfile {
    pub fn from_ron(text: &str) -> Result<Self, ProfileError> {
        Ok(ron::from_str(text)?)
    }

    /// The bundled UYA NTSC-U profile.
    pub fn uya() -> Self {
        // The bundled file is checked by a test, so this cannot fail at run time.
        Self::from_ron(UYA_RON).expect("the bundled UYA profile parses")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_profile_parses() {
        let profile = HeroProfile::uya();
        assert_eq!(profile.walk.speed, 6.17);
        assert_eq!(profile.jump.hold_ticks, 17);
    }

    #[test]
    fn bad_profiles_are_errors() {
        assert!(HeroProfile::from_ron("(name: \"x\")").is_err());
    }

    #[test]
    fn scaled_springs_clamp() {
        let spring = HeroProfile::uya().walk.turn_setting_off;
        assert_eq!(spring.at(0.0).gain, spring.gain_min);
        assert_eq!(spring.at(1.0).gain, 0.02);
        assert_eq!(spring.at(5.0).cap, spring.cap_max);
    }
}
