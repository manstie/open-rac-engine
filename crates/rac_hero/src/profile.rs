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
    pub wall_jump: WallJumpProfile,
    pub ledge: LedgeProfile,
    pub hit: HitProfile,
    pub death: DeathProfile,
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
    /// A jump from walking is a run jump when the stick is past this.
    pub run_jump_stick: f32,
    /// On the ground a jump press is kept this many ticks, standing and walking, so one
    /// made just before landing still jumps.
    pub jump_buffer_idle: u32,
    pub jump_buffer_walk: u32,
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
    /// ...and how far the stick must be pushed.
    pub min_stick: f32,
    /// Added along the facing on push-off tick t, as `t × forward_push`.
    pub forward_push: f32,
    /// A jump from a crouch waits until the crouch is this many ticks old.
    pub crouch_ticks: u32,
    /// Jump presses this recent count for a jump from a crouch.
    pub press_window: u32,
    /// Crouching in a standing or run jump before this tick, within `crouch_window` ticks
    /// of the press, turns it into a long jump.
    pub from_jump_before_tick: u32,
    pub crouch_window: u32,
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
    /// A double jump can start once the first jump is past this tick...
    pub after_tick: u32,
    /// ...with a press from the last `t − press_skip` ticks, clamped to these bounds, so
    /// the take-off press doesn't count but an early one is kept.
    pub press_skip: u32,
    pub press_window_min: u32,
    pub press_window_max: u32,
    /// Past the apex it also needs this much height above the ground, and a vertical
    /// speed below `past_apex_max_speed`.
    pub past_apex_min_height: f32,
    pub past_apex_max_speed: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlideProfile {
    /// Holding jump glides once a double jump is past this tick and higher than
    /// `double_jump_min_height` above the ground...
    pub after_double_jump_tick: u32,
    pub double_jump_min_height: f32,
    /// ...or once a heli jump is past this tick and higher than `heli_jump_min_height`.
    pub after_heli_jump_tick: u32,
    pub heli_jump_min_height: f32,
    pub sink: f32,
    /// The spring easing the vertical speed towards the sink, per tick.
    pub spring: TurnSpring,
    pub speed: f32,
    pub accel: f32,
    pub brake: f32,
}

/// The wall jump and how walls are found (`hero-controller.md`, "Wall jump").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallJumpProfile {
    pub params: JumpParams,
    /// The target heights off a wall-jump surface.
    pub surface_height_min: f32,
    pub surface_height_max: f32,
    /// Speed away from the wall once the cling is over.
    pub speed: f32,
    /// The tick the push away from the wall starts.
    pub away_from_tick: u32,
    /// Turning to face away from the wall, from `turn_from_tick` on.
    pub turn: TurnSpring,
    pub turn_from_tick: u32,
    /// How long a wall stays in reach after it was seen, in ticks.
    pub window_ticks: u32,
    /// Jump presses this recent start a wall jump.
    pub press_window: u32,
    /// A wall jump may follow another one past this tick of it.
    pub again_after_tick: u32,
    /// The tall-wall probe's height above the feet, and in a wall jump.
    pub probe_height: f32,
    pub probe_height_chained: f32,
    /// How far past the capsule's radius the probes reach.
    pub probe_reach: f32,
    /// The distance probe's height above the feet, and the most distance from the axis
    /// to the wall, and that in a wall jump.
    pub foot_probe_height: f32,
    pub max_distance: f32,
    pub max_distance_chained: f32,
    /// A wall is steeper than this, in degrees from the horizontal...
    pub min_steepness_degrees: f32,
    /// ...and faces the hero within this many degrees.
    pub max_facing_degrees: f32,
    /// A chained wall jump's wall faces at least this far round from the last one's.
    pub min_turn_degrees: f32,
    /// Walls are looked for only this high above the ground, and falling slower than
    /// `max_fall_speed`.
    pub min_height: f32,
    pub max_fall_speed: f32,
    /// How many wall jumps in a row ordinary walls allow.
    pub ordinary_chain: u32,
    /// A ledge the current trajectory reaches this many ticks ahead wins over a wall.
    pub ledge_lookahead_ticks: u32,
}

/// Grabbing, hanging from and climbing ledges (`hero-controller.md`, "Ledges").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgeProfile {
    /// The downward probe for the top: from this far above the feet to this far below.
    pub probe_above: f32,
    pub probe_below: f32,
    /// It starts at the front of the capsule and steps this far out each try.
    pub probe_step: f32,
    pub probe_tries: u32,
    /// The top must be flatter than this, in degrees.
    pub max_top_slope_degrees: f32,
    /// The top must be this far above the feet...
    pub min_rise: f32,
    pub max_rise: f32,
    /// ...and this far above the ground under the hero.
    pub min_drop: f32,
    /// The lip must be clear of anything for this far back towards the hero.
    pub clear_back: f32,
    /// The wall face is probed this far below the top, retried lower `face_tries` times by
    /// `face_step`, reaching this far past the top's hit.
    pub face_below: f32,
    pub face_tries: u32,
    pub face_step: f32,
    pub face_reach: f32,
    /// The lip is searched for in steps of `lip_step` out from the wall, up to `lip_search`.
    pub lip_step: f32,
    pub lip_search: f32,
    /// The hanging feet are this far out from the wall and below the lip.
    pub hang_out: f32,
    pub hang_below: f32,
    /// A jump grabs once past its apex or rising slower than this, u/s.
    pub grab_rising_below: f32,
    /// A double jump grabs only past this tick.
    pub double_jump_after_tick: u32,
    /// Moving to the hang point, at most this fast.
    pub grab_speed: f32,
    pub grab_turn: TurnSpring,
    /// The grab settles into hanging after this many ticks.
    pub grab_ticks: u32,
    /// After letting go, ledges are ignored this long; after letting go mid-shimmy, longer.
    pub no_grab_ticks: u32,
    pub no_grab_ticks_shimmy: u32,
    /// Jump presses this recent climb, from hanging and from a shimmy.
    pub climb_press_window: u32,
    pub shimmy_climb_press_window: u32,
    /// Jump with the stick pulled further than this from the facing lets go, in degrees.
    pub let_go_degrees: f32,
    /// The stick past this and within `shimmy_degrees` of a side shimmies that way, if a
    /// ledge goes on `shimmy_probe` to that side.
    pub shimmy_stick: f32,
    pub shimmy_degrees: f32,
    pub shimmy_probe: f32,
    pub shimmy_speed: f32,
    pub shimmy_turn: TurnSpring,
    /// The climb: a jump with its own forward speed, timed like its animation.
    pub climb: JumpParams,
    pub climb_forward_speed: f32,
    pub climb_forward_accel: f32,
    pub climb_forward_from_tick: u32,
    pub climb_forward_until_tick: u32,
    pub climb_forward_stop_tick: u32,
    /// After this tick of the climb the hero turns like in a jump.
    pub climb_turn_after_tick: u32,
}

/// Being knocked back (`hero-controller.md`, "Getting hit and dying").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitProfile {
    /// The knockback velocity is capped at this.
    pub max_speed: f32,
    /// The knockback slows at `brake`, and on the ground from `brake_later_from_tick` at
    /// `brake_later`.
    pub brake: f32,
    pub brake_later: f32,
    pub brake_later_from_tick: u32,
    /// Back on the ground after this many ticks, the hit is over.
    pub ticks: u32,
    /// After a hit, and after a hazard's, the hero can't be hurt again for this long.
    pub safe_ticks: u32,
    pub hazard_safe_ticks: u32,
    /// What a hazard surface takes, and how hard it throws the hero up and back.
    pub hazard_damage: u32,
    pub hazard_knockback: f32,
    pub hazard_lift: f32,
    pub max_health: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeathProfile {
    /// How long the hero lies dead before he comes back at his checkpoint.
    pub ticks: u32,
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
