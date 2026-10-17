//! Movement profiles: every number the controller uses, loaded from RON.
//!
//! Units are per second at 60 Hz: speeds in u/s, accelerations in u/s², frame counts at
//! 60 Hz. Turning springs keep the games' per-frame gains, which are per tick here because
//! the kit also ticks at 60 Hz. The bundled profiles say in comments where each value comes
//! from (`docs/research/hero-controller.md` and its status labels).
//!
//! Each game has its own profile, so where the games differ (Deadlocked walks faster than
//! UYA and its double jump is lower) only the numbers change, not the controller.
//! [`BUNDLED`] lists the ones that ship with the kit; any other RON file of the same shape
//! loads with [`HeroProfile::from_ron`].

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// UYA NTSC-U's tuning, the kit's default (PLAN.md section 5, 2026-10-12).
pub const UYA_RON: &str = include_str!("../assets/uya.ron");

/// Deadlocked NTSC-U's tuning (`hero-controller.md`, "Differences between the games").
pub const DL_RON: &str = include_str!("../assets/dl.ron");

/// The profiles that ship with the kit, by the short name settings and command lines use.
pub const BUNDLED: &[(&str, &str)] = &[("uya", UYA_RON), ("dl", DL_RON)];

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
    pub strafe: StrafeProfile,
    pub lock_on: LockOnProfile,
    pub heli_jump: HeliJumpProfile,
    pub double_jump: DoubleJumpProfile,
    pub glide: GlideProfile,
    pub wall_jump: WallJumpProfile,
    pub ledge: LedgeProfile,
    pub charge: ChargeProfile,
    pub grind: GrindProfile,
    pub zip_line: ZipLineProfile,
    pub swim: SwimProfile,
    pub melee: MeleeProfile,
    pub hit: HitProfile,
    pub death: DeathProfile,
    pub camera: CameraProfile,
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
    /// The furthest the hero moves sideways in one tick, whatever his speed.
    pub max_step: f32,
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
    /// What counts as a flick of the stick, which the crouch flip looks for.
    pub flick: FlickProfile,
}

/// UYA's pad layer marks a flick of the stick as a press of its own (0x45D5C8;
/// `hero-controller.md`, "Flips"): the stick reaching past `to`, either from under `from`
/// within `within_ticks`, or swung more than `swing_degrees` from where it pointed in any
/// of the last `swing_ticks`. Both are measured after the dead zone, before the cut-off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlickProfile {
    pub from: f32,
    pub to: f32,
    pub within_ticks: usize,
    pub swing_degrees: f32,
    pub swing_ticks: usize,
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
    /// The walk's skid: after a sharp turn, and for a couple of ticks after the stick is
    /// released.
    pub skid_brake: f32,
    /// The skid state's braking (state 3): after a long jump's landing or a charge.
    pub skid_state_brake: f32,
    /// Ticks of skid braking after the stick is released.
    pub stop_skid_ticks: u32,
    /// The most speed left when the hero settles into idle.
    pub stop_residual: f32,
    pub stop_brake: f32,
    /// Under `slide_below` u/s the walk moves the way the stick points, until he faces
    /// within `slide_until_degrees` of it.
    pub slide_below: f32,
    pub slide_until_degrees: f32,
    /// A full stick starts the walk's skid when it is more than `skid_slow_degrees` round
    /// from the facing under `slide_below`, or when it swung more than
    /// `skid_swing_degrees` in the last `skid_swing_ticks` and is more than
    /// `skid_swing_error_degrees` round. The skid lasts until he faces within
    /// `skid_until_degrees`.
    pub skid_slow_degrees: f32,
    pub skid_swing_degrees: f32,
    pub skid_swing_ticks: usize,
    pub skid_swing_error_degrees: f32,
    pub skid_until_degrees: f32,
    /// While skidding the target speed is at most the walk speed less `skid_speed_loss`
    /// per half turn of error, scaled by `skid_scale_degrees` (as radians) less the error,
    /// clamped to `skid_scale_min..=1`.
    pub skid_speed_loss: f32,
    pub skid_scale_degrees: f32,
    pub skid_scale_min: f32,
    /// A stick under `turn_slow_stick` more than `turn_slow_degrees` round cuts the target
    /// speed by `turn_slow_per_radian` of it per radian; under `turn_stall_stick` and more
    /// than `turn_stall_degrees` round, the speed doesn't rise.
    pub turn_slow_stick: f32,
    pub turn_slow_degrees: f32,
    pub turn_slow_per_radian: f32,
    pub turn_stall_stick: f32,
    pub turn_stall_degrees: f32,
    pub crouch_brake: f32,
    /// Crouching with the stick past `crouch_turn_stick` turns him on a spring of its own.
    pub crouch_turn: TurnSpring,
    pub crouch_turn_stick: f32,
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
    /// When the speed the stick asks for is under this share of the walk speed, the hero
    /// brakes at `release_brake` instead of steering.
    pub release_below: f32,
    pub release_brake: f32,
    /// Vertical speed below which a jump is past its apex.
    pub apex_below: f32,
    pub turn: TurnSpring,
    /// A jump that lands with the stick under `land_release_stick` keeps at most
    /// `land_release_speed` of its ground speed.
    pub land_release_stick: f32,
    pub land_release_speed: f32,
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

/// Flips (state 11, `hero-controller.md`, "Flips"). The strafe flip, X while strafing,
/// moves with the stick at `speed`; the crouch flip has its own motion in `crouch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlipProfile {
    pub params: JumpParams,
    pub speed: f32,
    pub min_speed: f32,
    pub start_speed: f32,
    pub accel: f32,
    /// A flip moves as the strafe flip only while strafing with the stick past this.
    pub strafe_stick: f32,
    pub crouch: CrouchFlipProfile,
}

/// The flip from a crouch: R1 held, the stick pushed a new way to a side or back, and X,
/// the two presses close together (UYA 0x502ED8, 0x502368).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrouchFlipProfile {
    /// The jump press and the stick press must be fewer than this many ticks apart.
    pub press_gap: u32,
    /// From a crouch, both presses within this many ticks. From idle or a walk, within
    /// the walk's jump buffer plus `press_gap`.
    pub crouch_window: u32,
    /// A standing, run, long or heli jump still on the ground before `early_before_tick`
    /// becomes the flip on a stick press within `early_window`, with crouch held.
    pub early_window: u32,
    pub early_before_tick: u32,
    /// The flip's own speed eases from 0 to `speed` at `accel` along the stick...
    pub speed: f32,
    pub accel: f32,
    /// ...and back to 0 once past the apex with `slow_within_ticks` or fewer to the landing.
    pub slow_within_ticks: u32,
    /// The ground speed he had, if over `carry_above`, carries on along the way he faced
    /// `facing_ticks_back` ticks before the flip, scaled by the sine of the angle from
    /// there to the flip's way. He turns back to that facing on `turn`.
    pub carry_above: f32,
    pub facing_ticks_back: usize,
    pub turn: TurnSpring,
    /// From tick `edge_from_tick`, under `edge_height` above the ground, the flip stops short
    /// of a drop: when no ground lies between `edge_rise` above and `edge_drop` below the
    /// take-off height, either `edge_near` or `edge_far` ahead, its own speed is zeroed.
    pub edge_from_tick: u32,
    pub edge_height: f32,
    pub edge_near: f32,
    pub edge_far: f32,
    pub edge_rise: f32,
    pub edge_drop: f32,
}

/// Strafing, UYA's walk with L2 or R2 held (`hero-controller.md`, "Strafing"): the walk's
/// speeds along the stick, while he turns to face the camera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrafeProfile {
    /// Turning to the camera, with a rate kept apart from the walk's.
    pub turn: TurnSpring,
    /// Changing sides: for `reverse_ticks - 1` ticks after strafing right, strafing left
    /// stops him, and after strafing left, strafing right scales his speed by 1/3, then
    /// 2/3 (with 3).
    pub reverse_ticks: u32,
    /// Whether a jump while strafing can flip. UYA's walk flips; DL's computes the flip and
    /// then always jumps, so there a strafing jump is a standing jump.
    pub flips: bool,
    /// A jump is a flip only with the stick past this and not pushed forward.
    pub flip_stick: f32,
    /// Strafing backwards, which walks backwards, needs the stick past this.
    pub back_stick: f32,
    /// With the stick let go a strafe keeps walking, braking, until he faces within this
    /// of the camera.
    pub idle_within_degrees: f32,
    /// Standing, he only steps round to the camera once it is more than this off his
    /// facing.
    pub turn_after_degrees: f32,
}

/// Locking on to a target, UYA's L2 and R2 held together (`hero-controller.md`,
/// "Lock-on").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockOnProfile {
    /// Which target: scored as the wrench's aim is, around the camera's direction, or
    /// failing that around his facing.
    pub aim: AimProfile,
    /// The target must be in sight from this far above his feet...
    pub sight_height: f32,
    /// ...along a line that stops this short of the target's middle, so the target's own
    /// collider doesn't hide it [simplified: the game's line ignores objects].
    pub sight_short: f32,
    /// A target further than this is let go.
    pub keep_range: f32,
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
    /// ledge goes on `shimmy_probe` to that side. At the end of each cycle of the shimmy's
    /// animation the same test decides whether it goes on or hangs.
    pub shimmy_stick: f32,
    pub shimmy_degrees: f32,
    pub shimmy_probe: f32,
    /// Each shimmy tick the ledge is looked for `shimmy_look` to the side, and the hero
    /// steps towards it by `shimmy_step_scale` × the entry of `shimmy_steps` for the
    /// animation's frame. The entry sets the step's length; a negative one steps back.
    pub shimmy_look: f32,
    pub shimmy_steps: Vec<f32>,
    pub shimmy_step_scale: f32,
    /// The animation is at frame 0 until tick `shimmy_blend_ticks`, then advances
    /// `shimmy_frames_per_tick` a tick; a cycle is `shimmy_cycle_frames` long.
    pub shimmy_blend_ticks: u32,
    pub shimmy_frames_per_tick: f32,
    pub shimmy_cycle_frames: f32,
    pub shimmy_turn: TurnSpring,
    /// The climb: a jump with its own forward speed, timed like its animation. Before
    /// `climb_forward_from_tick` and after `climb_forward_stop_tick` the speed is left as
    /// it is: 0 at first, and whatever it had reached at the end.
    pub climb: JumpParams,
    pub climb_forward_speed: f32,
    pub climb_forward_accel: f32,
    pub climb_forward_from_tick: u32,
    pub climb_forward_until_tick: u32,
    pub climb_forward_stop_tick: u32,
    /// After this tick of the climb the hero turns like in a jump.
    pub climb_turn_after_tick: u32,
}

/// The charge boots' dash, state 125, and the bounce off a wall that can end it, state 121
/// (`hero-controller.md`, "Charge boots").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChargeProfile {
    /// A crouch press starts a charge when the one before it came at most this many ticks
    /// earlier.
    pub tap_window: u32,
    /// Ticks after a charge before another can start.
    pub cooldown_ticks: u32,
    /// The speed heads for `speed` before tick `fast_ticks` and for `late_speed` after,
    /// rising at `accel` and slowing at `brake`.
    pub speed: f32,
    pub fast_ticks: u32,
    pub late_speed: f32,
    pub accel: f32,
    pub brake: f32,
    /// The charge ends after this tick.
    pub last_tick: u32,
    /// Steering: the stick's sideways push, eased by `steer_step` a tick, turns the hero
    /// at up to `turn_rate` rad/s times the stick's magnitude, ramping up over the first
    /// `turn_ramp_ticks`. A stick at or under `turn_min_stick` doesn't steer.
    pub turn_rate: f32,
    pub turn_ramp_ticks: u32,
    pub turn_min_stick: f32,
    pub steer_step: f32,
    /// Off the ground the hero's height follows the ground below on this spring, sinking
    /// at most `descent_cap` u/s.
    pub descent_spring: Spring,
    pub descent_cap: f32,
    /// The charge ends in a skid, with the stick and buttons ignored for this long.
    pub end_lock_ticks: u32,
    /// Running into a wall bounces off it when the hero moved less than `blocked_below` of
    /// the way that tick, or a ray `wall_probe_height` up from his front finds the wall
    /// within `wall_probe_reach`.
    pub blocked_below: f32,
    pub wall_probe_height: f32,
    pub wall_probe_reach: f32,
    /// The bounce goes back at `bounce_speed`, slowing at `bounce_brake`, falls at
    /// `bounce_gravity` off the ground, and lasts `bounce_ticks`.
    pub bounce_speed: f32,
    pub bounce_brake: f32,
    pub bounce_gravity: f32,
    pub bounce_ticks: u32,
}

/// Riding grind rails (`hero-controller.md`, "Grinding").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrindProfile {
    /// The hero gets on a rail whose nearest point is within `reach` horizontally
    /// (`reach_on_ground` standing or walking) and `reach_vertical` vertically, with his
    /// feet at most `below` under it and less than `above` over it.
    pub reach: f32,
    pub reach_on_ground: f32,
    pub reach_vertical: f32,
    pub below: f32,
    pub above: f32,
    /// After leaving a rail's end, rails are ignored this long.
    pub no_rail_ticks: u32,
    /// The base speed eases towards `speed` at `accel`.
    pub speed: f32,
    pub accel: f32,
    /// The slope term heads for `uphill_per_radian` less, or `downhill_per_radian` more,
    /// per radian of the rail's pitch over the next `pitch_probe`; it rises at
    /// `slope_rise`, falls at `slope_fall` and stays within `slope_min..=slope_max`.
    pub pitch_probe: f32,
    pub uphill_per_radian: f32,
    pub downhill_per_radian: f32,
    pub slope_rise: f32,
    pub slope_fall: f32,
    pub slope_min: f32,
    pub slope_max: f32,
    /// Turning to face the rail: he rides side on, turned `stance_degrees` from the way
    /// it goes.
    pub turn: TurnSpring,
    pub stance_degrees: f32,
    /// A jump press this recent jumps off the rail.
    pub jump_press_window: u32,
    /// The grind jump and the switch jump (states 40 and 41).
    pub jump: JumpParams,
    pub switch_jump: JumpParams,
    /// In a grind jump's first `switch_window_ticks`, a stick past `switch_stick` and
    /// within `switch_degrees` of square to the rail looks for another rail that far to
    /// the side: `switch_lookahead_ticks` of motion ahead, then from `switch_near` to
    /// `switch_far` in steps of `switch_step`. The move across takes `switch_ticks`.
    pub switch_window_ticks: u32,
    pub switch_stick: f32,
    pub switch_degrees: f32,
    pub switch_lookahead_ticks: u32,
    pub switch_near: f32,
    pub switch_far: f32,
    pub switch_step: f32,
    pub switch_ticks: u32,
}

/// Zip lines, UYA's state 118 (`hero-controller.md`, "Zip lines"): rails of
/// [`crate::RailKind::ZipLine`], caught with the hands. Speeds are u/s; the hang spring
/// works per tick, as in the game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZipLineProfile {
    /// The hands, `hands_up` above the feet, catch a cable whose nearest point is within
    /// `reach` of them horizontally and `reach_vertical` vertically, with the hands at
    /// most `below` under it and less than `above` over it.
    pub hands_up: f32,
    pub reach: f32,
    pub reach_vertical: f32,
    pub below: f32,
    pub above: f32,
    /// After letting go, cables are ignored this long.
    pub no_catch_ticks: u32,
    /// The speed along the cable starts at `catch_share` of the hero's velocity along it,
    /// never below 0, and rises at `accel` to `speed`, whatever the cable's slope.
    pub catch_share: f32,
    pub speed: f32,
    pub accel: f32,
    /// Riding, the feet hang `hang_below` under the cable. From where he caught it they
    /// get there on the `hang` spring, at most `hang_cap` u per tick.
    pub hang_below: f32,
    pub hang: Spring,
    pub hang_cap: f32,
    /// Turning to face along the cable.
    pub turn: TurnSpring,
    /// Past the cable's end he flies on with its last step, gaining `end_gravity`, for
    /// `end_ticks`, then falls.
    pub end_ticks: u32,
    pub end_gravity: f32,
}

/// Swimming on and under the water (`hero-controller.md`, "Swimming"). Springs work per
/// tick, as in the game; speeds are u/s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwimProfile {
    /// A falling hero gets in with his feet `entry_below` under the surface, give or take
    /// `entry_slack` or `entry_slack_extra` plus a tick of his fall if more, in water more
    /// than `entry_depth` deep. Any hero gets in with his feet `deep_below` under, and a
    /// water jump comes back in with them `jump_back_below` under.
    pub entry_below: f32,
    pub entry_slack: f32,
    pub entry_slack_extra: f32,
    pub entry_depth: f32,
    pub deep_below: f32,
    pub jump_back_below: f32,
    /// Rising under water with his feet less than this under, the hero surfaces.
    pub surface_below: f32,
    /// On the surface his feet float this far under it, at a height that follows the
    /// water's with `surface_follow`.
    pub float_below: f32,
    pub surface_follow: Spring,
    /// Getting in, the bob's rate starts at `bob_entry_scale` × his vertical velocity, at
    /// most `bob_entry_max` downwards.
    pub bob_entry_scale: f32,
    pub bob_entry_max: f32,
    /// Idle, once `bob_free_ticks` are over, the bob's rate loses `bob_stiffness` × its
    /// offset + `bob_damping` × itself each tick.
    pub bob_free_ticks: u32,
    pub bob_stiffness: f32,
    pub bob_damping: f32,
    /// Swimming, the bob springs to rest at up to `bob_swim_cap`, and sinks at `bob_sink`
    /// from above it.
    pub bob_swim: Spring,
    pub bob_swim_cap: f32,
    pub bob_sink: f32,
    /// Getting in, his horizontal velocity drifts on, up to `drift_max`, slowing at
    /// `drift_brake`.
    pub drift_max: f32,
    pub drift_brake: f32,
    /// The surface swim heads for `surface_speed` × the stick, at least
    /// `surface_min_speed` with the stick held, and the full speed for the first
    /// `stroke_ticks`; not at all while the stick points more than
    /// `surface_turn_limit_degrees` from his facing.
    pub surface_speed: f32,
    pub surface_min_speed: f32,
    pub stroke_ticks: u32,
    pub surface_turn_limit_degrees: f32,
    pub surface_accel: f32,
    pub surface_brake: f32,
    /// The velocity is the speed × this × the stroke factor.
    pub surface_stroke: f32,
    /// The stroke animation's rate on the surface.
    pub surface_stroke_rate: f32,
    /// Turning towards the stick, on and under the water.
    pub turn: TurnSpring,
    /// A jump press this recent jumps out of the water, from idle only after
    /// `idle_jump_after_tick`.
    pub jump_press_window: u32,
    pub idle_jump_after_tick: u32,
    /// The water jump (state 18).
    pub jump: JumpParams,
    /// A melee press this recent dives; from idle a crouch or strafe press does too.
    pub dive_press_window: u32,
    /// Swimming stops with the stick under `stop_stick`, and starts with it past
    /// `start_stick`, from surface idle once `start_after_tick` is past.
    pub stop_stick: f32,
    pub start_stick: f32,
    pub start_after_tick: u32,
    /// In water shallower than this, the surface swim and idle stand up.
    pub stand_depth_swimming: f32,
    pub stand_depth_idle: f32,
    /// Under water, the pitch (positive nose down) heads for ± `pitch_max` with a melee or
    /// jump press held, through `pitch_spring` capped at `pitch_cap`, plus
    /// `pitch_cap_pressed` with one held (rad per tick).
    pub pitch_max: f32,
    pub pitch_spring: Spring,
    pub pitch_cap: f32,
    pub pitch_cap_pressed: f32,
    /// Diving from the surface: `dive_ticks` long, pitched down at least
    /// `dive_min_pitch` × `pitch_max`, heading for `dive_speed`.
    pub dive_ticks: u32,
    pub dive_min_pitch: f32,
    pub dive_speed: f32,
    /// Speeds under water: the swim, and the fast swim with crouch or strafe held or not.
    pub under_speed: f32,
    pub fast_speed: f32,
    pub fast_coast_speed: f32,
    /// The target is scaled by the stick, at least `min_scale` (`fast_min_scale` in the
    /// fast swim) with the stick past `scale_from`.
    pub scale_from: f32,
    pub min_scale: f32,
    pub fast_min_scale: f32,
    pub under_accel: f32,
    pub under_brake: f32,
    pub fast_accel: f32,
    pub fast_brake: f32,
    /// The swim's velocity is the speed × this × the stroke factor, at least
    /// `after_fast_min_speed` for `after_fast_ticks` after a fast swim.
    pub under_stroke: f32,
    pub after_fast_ticks: u32,
    pub after_fast_min_speed: f32,
    /// The underwater idle slows his drift at `idle_brake`, or `idle_brake_after_fast`.
    pub idle_brake: f32,
    pub idle_brake_after_fast: f32,
    /// The swim turns fast once this far in with crouch or strafe held, and the fast swim
    /// back with the stick past `fast_to_swim_stick`, or idle under `fast_to_idle_stick`.
    pub fast_after_tick: u32,
    pub fast_to_swim_stick: f32,
    pub fast_to_idle_stick: f32,
    /// The swim idles below `idle_speed`, between these frames of its stroke.
    pub idle_speed: f32,
    pub idle_stroke_from: f32,
    pub idle_stroke_to: f32,
    /// The stroke: how hard each frame of the arms' cycle pushes. The animation advances
    /// `stroke_frames_per_rate` × its rate frames a tick; under water the rate is
    /// `under_stroke_rate_per_speed` × the speed, within the min and max.
    pub stroke: Vec<f32>,
    pub stroke_frames_per_rate: f32,
    pub under_stroke_rate_per_speed: f32,
    pub under_stroke_rate_min: f32,
    pub under_stroke_rate_max: f32,
}

/// The wrench: the three-swing combo (state 19), the jump attack (20) and the throw (21)
/// (`hero-controller.md`, "Melee").
///
/// The attacks are timed by the game's animation frames. Each attack's animation blends in
/// for `blend_ticks`, starting at `start_frame`, then advances `frames_per_tick` a tick at
/// full rate; every window below is in those frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeleeProfile {
    pub frames_per_tick: f32,
    /// An attack's animation blends in by creeping its frame from the start frame, then
    /// starts over from it (UYA). Without this the frame runs from the start, already a
    /// tick on when the attack begins, and nothing waits for a blend (DL).
    pub blend_creeps: bool,
    /// How old a melee press may be to start an attack: standing, walking or skidding;
    /// crouching (a throw); and in the air, where it waits `air_after_tick` into the jump,
    /// or longer into a double jump or a flip, which also remember the press longer.
    pub ground_press_window: u32,
    pub crouch_press_window: u32,
    pub air_press_window: u32,
    pub air_press_window_late: u32,
    pub air_after_tick: u32,
    pub double_jump_after_tick: u32,
    pub flip_after_tick: u32,
    /// From the air, closer to the ground than this, the attack is a combo swing.
    pub combo_below_height: f32,
    /// A press during an attack counts from the attack's `buffer_from` frame, two ticks a
    /// frame, but at most this many ticks back.
    pub chain_press_max: u32,
    /// The combo goes on from where it stopped if it stopped less than this long ago.
    pub chain_memory_ticks: u32,
    /// During an attack, a press with crouch held throws instead of swinging, and a throw
    /// can throw again (UYA; DL always swings).
    pub chain_throw: bool,
    /// A jump out of an attack with crouch held is a jump from a crouch (UYA; DL's is
    /// always a standing jump).
    pub crouch_jump_out: bool,
    /// A combo swing or throw walks off past `walk_out_frame` with the stick past
    /// `walk_out_stick`; once over, it walks with the stick past `end_walk_stick` and
    /// otherwise idles `end_idle_frames` later.
    pub walk_out_frame: f32,
    pub walk_out_stick: f32,
    pub end_walk_stick: f32,
    pub end_idle_frames: f32,
    /// Combo swings and throws reach their lunge speed at `lunge_accel` and slow at
    /// `lunge_brake`; off the ground they fall at `air_gravity`.
    pub lunge_accel: f32,
    pub lunge_brake: f32,
    pub air_gravity: f32,
    /// Turning towards the aim: combo swings and the jump attack, and the throw.
    pub turn: TurnSpring,
    pub throw_turn: TurnSpring,
    /// The stick past this sets the attack's direction.
    pub lock_stick: f32,
    /// A combo swing on the ground stops short of an edge: it looks this many ticks of
    /// its motion ahead for ground between `edge_probe_above` and `edge_probe_below`.
    pub edge_lookahead_ticks: f32,
    pub edge_probe_above: f32,
    pub edge_probe_below: f32,
    pub combo: Vec<AttackProfile>,
    pub jump_attack: AttackProfile,
    pub throw: AttackProfile,
    pub combo_aim: AimProfile,
    pub jump_aim: AimProfile,
    pub throw_aim: AimProfile,
    pub jump: JumpAttackProfile,
    pub thrown: ThrownWrenchProfile,
    pub grind: GrindAttackProfile,
    pub clank: ClankProfile,
}

/// One attack's timing, motion and hit (a row of UYA's table at 0x2F6310, with the
/// handler's numbers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackProfile {
    pub start_frame: f32,
    pub blend_ticks: u32,
    /// A combo swing started before this frame of this one is the next swing.
    pub chain_before: f32,
    /// Presses count from this frame.
    pub buffer_from: f32,
    /// A new attack can start from this frame.
    pub chain_from: f32,
    /// A jump press past this frame jumps out.
    pub jump_from: f32,
    /// The attack is over after this frame.
    pub end: f32,
    /// The wrench hits from `hit_from` to `hit_to`.
    pub hit_from: f32,
    pub hit_to: f32,
    /// The lunge: up to each frame, its speed in u/s; it ends at the last frame.
    pub lunge: Vec<(f32, f32)>,
    /// With a target, a combo swing lunges harder to reach it.
    pub reach: Option<LungeReach>,
    pub damage: f32,
    /// How hard a hit pushes, as the game's push strength, and which way.
    pub knockback: f32,
    pub push: Push,
    /// The ball round the wrench's head that hits besides its swept shaft.
    pub head_radius: f32,
    /// Where the wrench is through the hit window, by frame.
    pub swing: Vec<SwingKey>,
}

/// A combo swing's lunge with a target: its scale is `(distance - 0.5 - offset) / 0.9`, at
/// most `max_scale`, when that is over 1 and the target is within `range + 3` of the
/// offset.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LungeReach {
    pub offset: f32,
    pub range: f32,
    pub max_scale: f32,
}

/// Which way a hit pushes: across the wrench's line to the hero's left or right, as it
/// swings, or the way he faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Push {
    Left,
    Right,
    Forward,
}

/// The wrench at one animation frame, relative to the hero's feet: (forward, left, up).
/// `hand` and `tip` are the ends of the line that hits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SwingKey {
    pub frame: f32,
    pub hand: [f32; 3],
    pub tip: [f32; 3],
}

/// Which target an attack turns to: the one within `range` and `cone_degrees` of the aim
/// with the least distance × (1 + angle off), and within `rise_degrees` up or down (0 for
/// any).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AimProfile {
    pub range: f32,
    pub cone_degrees: f32,
    pub rise_degrees: f32,
}

/// The jump attack's own motion (`hero-controller.md`, "Jump attack").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JumpAttackProfile {
    /// The upward speed it steps to at `rise_accel` until `rise_until_tick`: `rise`, or
    /// less when already high above the ground.
    pub rise: f32,
    pub rise_high: f32,
    pub high_above: f32,
    pub rise_higher: f32,
    pub higher_above: f32,
    pub rise_accel: f32,
    pub rise_until_tick: u32,
    /// After `hold_until_tick` it falls at `gravity`, at most `fall_cap`.
    pub hold_until_tick: u32,
    pub gravity: f32,
    pub fall_cap: f32,
    /// Horizontally it brakes at `brake`, but keeps `carry_speed` while more than
    /// `carry_above` up, until `carry_until_tick`. With a target `target_near` to
    /// `target_far` away it keeps `carry_speed`, changing at `target_accel`.
    pub brake: f32,
    pub carry_speed: f32,
    pub carry_above: f32,
    pub carry_until_tick: u32,
    pub target_accel: f32,
    pub target_near: f32,
    pub target_far: f32,
    /// It looks for a target until this frame.
    pub aim_until_frame: f32,
    /// The animation runs at `rise_rate` going up, then at the rate that brings
    /// `strike_frame` to the landing, but not under `low_rate_min` below `low_rate_below`;
    /// on the ground it runs at `landed_rate` until `strike_frame`.
    pub rise_rate: f32,
    pub strike_frame: f32,
    pub low_rate_below: f32,
    pub low_rate_min: f32,
    pub landed_rate: f32,
    /// In the air at least `hover_ticks` from landing, the animation all but stops, at
    /// `hover_rate` (DL; 0 ticks for never).
    pub hover_ticks: u32,
    pub hover_rate: f32,
    /// Blends in longer right after a flip.
    pub blend_after_flip: u32,
    pub after_flip_ticks: u32,
    /// Coming down past `slam_from_frame`, a ball `slam_below` under his feet hits too.
    pub slam_from_frame: f32,
    pub slam_below: f32,
    pub slam_radius: f32,
    pub slam_damage: f32,
    pub slam_knockback: f32,
}

/// The thrown wrench (`hero-controller.md`, "Throw").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThrownWrenchProfile {
    /// It leaves the hand, `hand` from his feet (forward, left, up), at this frame.
    pub release_frame: f32,
    pub hand: [f32; 3],
    /// Out at `speed`, slowing harder each tick by `jerk` (u/s³) until it stops; then back,
    /// speeding up harder each tick by `return_jerk`.
    pub speed: f32,
    pub jerk: f32,
    pub return_jerk: f32,
    /// Within `height_within` of the ground it keeps `height` above it, rising at `rise`
    /// and sinking at `sink`.
    pub height: f32,
    pub height_within: f32,
    pub rise: f32,
    pub sink: f32,
    /// Hitting a wall sends it back at this share of its speed.
    pub wall_slow: f32,
    /// Further than this from the hand it is back at once.
    pub lost_beyond: f32,
    pub radius: f32,
    pub damage: f32,
    /// Push strength on the way out, and on the way back (towards the hero).
    pub knockback_out: f32,
    pub knockback_back: f32,
    /// The hero's animation holds at `hold_frame` while it is out, and jumps to
    /// `catch_frame` when it is under `catch_ahead_ticks` from the hand, or
    /// `catch_after_tick` into the throw.
    pub hold_frame: f32,
    pub catch_frame: f32,
    pub catch_ahead_ticks: f32,
    pub catch_after_tick: u32,
    /// Blends in faster when thrown again straight after a throw.
    pub blend_again: u32,
    /// Thrown again from a throw past this tick, with crouch held.
    pub again_after_tick: u32,
    /// A jump out of a throw counts presses this many ticks back.
    pub jump_out_window: u32,
}

/// The grind attack, a swing on a rail (UYA's state 42; `hero-controller.md`, "The grind
/// attack"). He rides on as he swings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrindAttackProfile {
    /// A melee press this recent on a rail swings.
    pub press_window: u32,
    /// A jump press this recent, from the swing's `jump_from` frame, jumps off the rail.
    pub jump_press_window: u32,
    /// The first swing, and the other one, which a press past a swing's `chain_from` frame
    /// starts in its place. The other fields are as a combo swing's.
    pub swings: Vec<AttackProfile>,
    /// For his first `ball_ticks` a ball of `ball_radius` round a point `ball_above` his
    /// feet hits as well, for `ball_damage`, pushing at `ball_knockback` along the rail
    /// turned `ball_push_degrees` to the left.
    pub ball_ticks: u32,
    pub ball_radius: f32,
    pub ball_above: f32,
    pub ball_damage: f32,
    pub ball_knockback: f32,
    pub ball_push_degrees: f32,
}

/// The wrench clanking off a wall (`hero-controller.md`, "What the wrench hits"): once an
/// attack, a ray from `above` his feet towards the wrench's tip, `reach` long, finds the
/// wall. It looks only past `after_tick` and while the tip is within `within_degrees` of
/// his facing; the jump attack's tip may be anywhere. The ray shortens as the tip comes
/// round, as the wrench does: to `1 - shorten` of its length at `shorten_degrees`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClankProfile {
    pub after_tick: u32,
    pub within_degrees: f32,
    pub above: f32,
    pub reach: f32,
    pub shorten: f32,
    pub shorten_degrees: f32,
    /// What it hits there takes `damage`, pushed the way the ray went at `knockback`.
    pub damage: f32,
    pub knockback: f32,
    /// The jump attack also clanks when its frame passes `ground_frame`, on ground found
    /// from `ground_above` over the tip to `ground_below` under it.
    pub ground_frame: f32,
    pub ground_above: f32,
    pub ground_below: f32,
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

/// A damped spring without a cap: each tick the rate gains `gain × error` and loses
/// `damping × rate`, and never carries the value past its target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spring {
    pub gain: f32,
    pub damping: f32,
}

/// Which camera the player watches the hero through: the games' camera setting
/// (`camera.md`, "The setting and the hero's camera mode").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CameraSetting {
    /// The follow camera.
    ThirdPerson,
    /// The free-look camera behind the hero's eye, turned only by the player.
    LockStrafe,
    /// The free-look camera at the hero's eye.
    FirstPerson,
}

impl CameraSetting {
    pub const ALL: [CameraSetting; 3] = [
        CameraSetting::ThirdPerson,
        CameraSetting::LockStrafe,
        CameraSetting::FirstPerson,
    ];
}

/// The hero's cameras (`camera.md`): the third-person follow camera, the free-look camera
/// behind the lock-strafe and first-person views, and the blends between them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraProfile {
    /// The camera a player starts with.
    pub default_setting: CameraSetting,
    /// The follow distance and height at rest, and the height of the aim point above the
    /// hero's feet.
    pub distance: f32,
    pub height: f32,
    pub look_height: f32,
    pub distance_spring: Spring,
    pub height_spring: Spring,
    pub look_height_spring: Spring,
    /// How the look point trails the hero, horizontally and vertically.
    pub look_spring: Spring,
    pub look_spring_vertical: Spring,
    /// The share of the hero's movement the arm takes up each tick.
    pub drag: f32,
    /// The actual arm's yaw and tilt chase the dragged arm with this spring...
    pub arm_spring: Spring,
    /// ...and its length with this one, stiffening to `arm_length_spring_approach` when
    /// the hero runs at the camera at full speed.
    pub arm_length_spring: Spring,
    pub arm_length_spring_approach: Spring,
    /// Running at the camera (motion against its facing beyond `approach_facing`) pushes
    /// the distance target out to `approach_distance` at `approach_full_speed` u/s.
    pub approach_distance: f32,
    pub approach_full_speed: f32,
    pub approach_facing: f32,
    /// The arm tilts up or down at most this far, moving at most `pitch_step` rad a tick.
    pub pitch_limit_degrees: f32,
    pub pitch_step: f32,
    /// The look stick turns the arm at `stick_turn + stick_turn_per_speed × speed / 100`
    /// rad a tick at full tilt, for the player's camera-speed option (0 to 100).
    pub stick_turn: f32,
    pub stick_turn_per_speed: f32,
    /// Past `stick_tilt_dead_zone` the stick tilts the arm, up to the pitch limit at full
    /// tilt, and lets it come back when released. The tilt follows the stick by at most
    /// `stick_tilt_out` of the range a tick going out and `stick_tilt_back` coming back.
    /// (The games' spring for it, gain 2 and damping 4, always gets there within those.)
    pub stick_tilt_dead_zone: f32,
    pub stick_tilt_out: f32,
    pub stick_tilt_back: f32,
    /// Strafing with the strafe buttons held, the arm isn't dragged, the aim moves to the
    /// look point, and the look point follows on this horizontal spring.
    pub strafe_look_spring: Spring,
    /// Strafing or locked on, the aim moves from the follow point to the lagging look
    /// point over this many ticks, and back over as many once neither holds it there.
    pub aim_blend_ticks: u32,
    pub vertical: VerticalProfile,
    pub clearance: ClearanceProfile,
    pub obstacles: ObstacleProfile,
    /// The camera ends up at least this far from geometry.
    pub collision_radius: f32,
    /// Locked on, the camera turns to look past the hero at the target as the right stick
    /// would, at up to `lock_turn` rad a tick, a share that grows from 0 to all of it
    /// over `lock_ramp_ticks` (`camera.md`, "Lock-on").
    pub lock_turn: f32,
    pub lock_ramp_ticks: u32,
    /// Locked on but not strafing, the look point follows on this horizontal spring.
    pub lock_look_spring: Spring,
    /// A camera volume's distance and height spring at this gain. It stays after the
    /// volume, until running at the camera sets the usual gain again.
    pub volume_gain: f32,
    pub free_look: FreeLookProfile,
    pub blends: CameraBlends,
}

/// The follow point's height per hero state (`camera.md`, "The follow point's height").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerticalProfile {
    /// Timed rises: the heli jump's and the double jump's to above the ground below him,
    /// the flip's to above where he starts it, and back down after it.
    pub heli_rise: Rise,
    pub double_jump_rise: Rise,
    pub flip_rise: Rise,
    /// A sphere this big at the camera stops a rise where it touches anything.
    pub rise_stop_radius: f32,
    /// Past the apex with fewer ticks than this to land, ground at or above the held height
    /// lifts it.
    pub land_ticks: f32,
    /// Past the apex and more than `follow_down_above` u above the ground, the held height
    /// follows him down once his head drops `follow_down_angle` rad under the view.
    pub follow_down_above: f32,
    pub follow_down_angle: f32,
    /// The jump attack's halfway height ends when he falls or has been off the ground
    /// `attack_air_ticks`, and lasts `attack_after_ticks` after the attack.
    pub attack_air_ticks: u32,
    pub attack_after_ticks: u32,
    /// In the air and for `air_after_land_ticks` after landing, the look point's vertical
    /// gain is `air_gain + air_gain_gap × min(gap, air_gap) / air_gap`, where gap is how
    /// far it is from the follow point's height.
    pub air_gain: f32,
    pub air_gain_gap: f32,
    pub air_gap: f32,
    pub air_after_land_ticks: u32,
}

/// A rise of the follow point by `height` over `ticks`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rise {
    pub height: f32,
    pub ticks: u32,
}

/// The clearance point, which keeps the camera's pivot between floor and ceiling
/// (`camera.md`, "Obstacles and collision").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClearanceProfile {
    /// Rays this long go up and down from this far above his feet.
    pub ray_from: f32,
    pub ray_length: f32,
    /// How much room the pivot wants above and below it.
    pub radius: f32,
    /// Under a low ceiling the height springs to the clearance point at this gain.
    pub height_gain: f32,
}

/// How the follow camera gets round obstacles (`camera.md`, "Obstacles and collision").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObstacleProfile {
    /// Spheres swept along the arm as (distance from the pivot, radius) at the rest
    /// distance, scaled with the arm. One that hits pushes the arm sideways until its
    /// line passes the contact by `sweep_clear_scale × (sweep_clear + sweep_clear_along ×
    /// t)`, t being how far along the arm the contact is.
    pub sweep: Vec<(f32, f32)>,
    pub sweep_clear: f32,
    pub sweep_clear_along: f32,
    pub sweep_clear_scale: f32,
    /// The camera's end stops this far short of a wall it would pass through.
    pub end_skin: f32,
    /// Turning the camera into a wall (the stick past `wall_stick`) pulls it in by
    /// `wall_pull` a tick.
    pub wall_stick: f32,
    pub wall_pull: f32,
    /// The arm is never shorter than `min_arm`; a wall alone can push it to `min_arm_wall`.
    pub min_arm: f32,
    pub min_arm_wall: f32,
    /// The pull-in's timer rests at `pull_in_rest` ticks, and a wall pull sets it to
    /// `pull_in_wall`. Clear of obstacles, it counts down and the pull-in eases out by
    /// `(1 − cos(π × timer / pull_in_rest)) / 2` a tick.
    pub pull_in_rest: u32,
    pub pull_in_wall: u32,
    /// The stick counts as resting under this on both axes.
    pub idle_stick: f32,
    /// A view blocked for this many ticks running, or a camera this far from where it
    /// should be, starts again behind the hero, stopping `reset_skin` short of anything in
    /// the way.
    pub reset_blocked_ticks: u32,
    pub reset_distance: f32,
    pub reset_skin: f32,
}

/// The free-look camera behind the lock-strafe and first-person views and the look
/// (`camera.md`, "The free-look camera"). Angles in radians and per tick unless named in
/// degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeLookProfile {
    /// The stick is squared, keeping its sign (UYA), or taken as it is (Deadlocked).
    pub squared: bool,
    /// Turning speeds at full tilt, reached at these rates a tick per tick, all times
    /// `0.25 + speed / 64` for the camera-speed option.
    pub yaw_speed: f32,
    pub yaw_accel: f32,
    pub pitch_speed: f32,
    pub pitch_accel: f32,
    /// The quick turn: once the stick's x is past `quick_stick` for `quick_after_ticks`
    /// ticks running.
    pub quick_stick: f32,
    pub quick_after_ticks: u32,
    pub quick_speed: f32,
    pub quick_accel: f32,
    /// In the look, each axis is the further pushed of the two sticks', and a stick under
    /// this turns nothing.
    pub look_stick_threshold: f32,
    /// How far down and up the view goes in lock-strafe and in first person. While either
    /// is shown, the up limit eases towards `up_limit_rest_degrees` at
    /// `up_limit_ease_degrees` a tick.
    pub lock_strafe_limits: PitchLimits,
    pub first_person_limits: PitchLimits,
    pub up_limit_rest_degrees: f32,
    pub up_limit_ease_degrees: f32,
    /// The look button's first person in lock-strafe sets the first-person limits again
    /// every tick it is held, so the up limit only ever takes one step (UYA).
    pub hold_resets_limits: bool,
    /// The look button must be held this long for first person in lock-strafe.
    pub first_person_lead_ticks: u32,
    /// The eye's height above his feet in lock-strafe and first person (`eye_crouch` in a
    /// crouch). Switching the camera sets it to `eye_on_switch` if there is one, otherwise
    /// to the new target; it then moves at up to `eye_speed`, speeding up and braking by
    /// `eye_accel`, u a tick.
    pub eye_lock_strafe: f32,
    pub eye_first_person: f32,
    pub eye_crouch: f32,
    pub eye_on_switch: Option<f32>,
    pub eye_accel: f32,
    pub eye_speed: f32,
    /// Jumping in lock-strafe holds the eye `eye_lock_strafe` above the ground below him,
    /// until he is `jump_hold_until` above it. `jump_hold_ledge` holds it in a ledge jump
    /// too.
    pub jump_hold_until: f32,
    pub jump_hold_ledge: bool,
    /// Looking straight down moves the eye this far forward.
    pub look_down_forward: f32,
    /// A sphere of `eye_guard_radius` swept from `eye_guard_from` above his feet to the eye
    /// keeps the eye out of walls.
    pub eye_guard_from: f32,
    pub eye_guard_radius: f32,
    /// The lock-strafe camera sits `offset` behind the eye along the view. A ray
    /// `offset_ray_extra` longer that hits something cuts it to the hit less that much at
    /// once; it eases back out at up to `offset_speed`, speeding up and braking by
    /// `offset_accel`, u a tick.
    pub offset: f32,
    pub offset_ray_extra: f32,
    pub offset_accel: f32,
    pub offset_speed: f32,
    /// A sphere this big swept over the offset's last unit stops the camera at walls
    /// (UYA).
    pub end_sphere_radius: Option<f32>,
}

/// Degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PitchLimits {
    pub down_degrees: f32,
    pub up_degrees: f32,
}

/// How the shown camera blends from one camera to another (`camera.md`, "Blending between
/// cameras").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraBlends {
    /// The look starts once the button has been held this long; meanwhile the follow
    /// camera turns to his facing as lock-on turns it, at up to `look_steer` rad a tick.
    pub look_lead_ticks: u32,
    pub look_steer: f32,
    /// Leaving the look with the follow camera this far off his facing starts it again
    /// behind him.
    pub look_reset_degrees: f32,
    pub into_look: Blend,
    pub out_of_look: Blend,
    pub into_lock_strafe: Blend,
    pub out_of_lock_strafe: Blend,
    pub out_of_first_person: Blend,
}

/// A blend that starts `start` of the way and moves `rate` of the way a tick, eased by
/// `(1 − cos πt) / 2`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Blend {
    pub start: f32,
    pub rate: f32,
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
        // The bundled files are checked by a test, so this cannot fail at run time.
        Self::from_ron(UYA_RON).expect("the bundled UYA profile parses")
    }

    /// The bundled Deadlocked NTSC-U profile.
    pub fn dl() -> Self {
        Self::from_ron(DL_RON).expect("the bundled DL profile parses")
    }

    /// A bundled profile by its short name in [`BUNDLED`].
    pub fn bundled(name: &str) -> Option<Self> {
        BUNDLED
            .iter()
            .find(|(bundled, _)| *bundled == name)
            .map(|(_, text)| Self::from_ron(text).expect("the bundled profiles parse"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_profiles_parse() {
        let profile = HeroProfile::uya();
        assert_eq!(profile.walk.speed, 6.17);
        assert_eq!(profile.jump.hold_ticks, 17);
        for (name, _) in BUNDLED {
            assert!(HeroProfile::bundled(name).is_some(), "{name}");
        }
        assert_eq!(HeroProfile::bundled("dl"), Some(HeroProfile::dl()));
        assert_eq!(HeroProfile::bundled("gc"), None);
        assert_eq!(profile.camera.default_setting, CameraSetting::ThirdPerson);
        assert_eq!(
            HeroProfile::dl().camera.default_setting,
            CameraSetting::LockStrafe
        );
    }

    /// The dotted paths of the fields whose values differ.
    fn differences(a: &HeroProfile, b: &HeroProfile) -> Vec<String> {
        fn value(profile: &HeroProfile) -> ron::Value {
            ron::from_str(&ron::to_string(profile).unwrap()).unwrap()
        }
        fn compare(path: String, a: &ron::Value, b: &ron::Value, found: &mut Vec<String>) {
            match (a, b) {
                (ron::Value::Map(a), ron::Value::Map(b)) => {
                    for (key, value) in a.iter() {
                        let ron::Value::String(name) = key else {
                            unreachable!("profile fields have names")
                        };
                        let path = if path.is_empty() {
                            name.clone()
                        } else {
                            format!("{path}.{name}")
                        };
                        compare(path, value, &b[key], found);
                    }
                }
                // A swing's keys differ as a whole when the games' animations do.
                (ron::Value::Seq(a), ron::Value::Seq(b))
                    if a.len() == b.len() && !path.ends_with(".swing") =>
                {
                    for (index, (a, b)) in a.iter().zip(b).enumerate() {
                        compare(format!("{path}[{index}]"), a, b, found);
                    }
                }
                _ if a != b => found.push(path),
                _ => {}
            }
        }
        let mut found = Vec::new();
        compare(String::new(), &value(a), &value(b), &mut found);
        found
    }

    /// Everything else in DL's profile is UYA's, either read the same in DL's code or not
    /// read there yet (the comments in `dl.ron` say which), so a change to one profile
    /// that misses the other shows up here.
    #[test]
    fn deadlocked_differs_from_uya_only_where_the_games_do() {
        assert_eq!(
            differences(&HeroProfile::uya(), &HeroProfile::dl()),
            // In the order of the field names, as RON's maps keep them. (The default
            // camera setting differs too, but RON reads both names as the same value.)
            [
                "camera.distance",
                "camera.free_look.end_sphere_radius",
                "camera.free_look.eye_accel",
                "camera.free_look.eye_first_person",
                "camera.free_look.eye_on_switch",
                "camera.free_look.eye_speed",
                "camera.free_look.first_person_lead_ticks",
                "camera.free_look.first_person_limits.up_degrees",
                "camera.free_look.hold_resets_limits",
                "camera.free_look.jump_hold_ledge",
                "camera.free_look.jump_hold_until",
                "camera.free_look.lock_strafe_limits.up_degrees",
                "camera.free_look.pitch_accel",
                "camera.free_look.pitch_speed",
                "camera.free_look.quick_accel",
                "camera.free_look.quick_after_ticks",
                "camera.free_look.squared",
                "camera.free_look.up_limit_rest_degrees",
                "camera.free_look.yaw_accel",
                "camera.free_look.yaw_speed",
                "double_jump.after_tick",
                "double_jump.cap",
                "fall.air_speed",
                "glide.sink",
                "glide.speed",
                "jump.air_speed",
                "ledge.climb.air_speed",
                "ledge.climb.pushoff_ticks",
                // "Deadlocked's melee".
                "melee.blend_creeps",
                "melee.chain_throw",
                "melee.clank.damage",
                "melee.clank.ground_frame",
                "melee.combo[0].head_radius",
                "melee.combo[0].lunge[0][0]",
                "melee.combo[0].lunge[0][1]",
                "melee.combo[0].start_frame",
                "melee.combo[0].swing",
                "melee.combo[1].head_radius",
                "melee.combo[1].lunge[0][0]",
                "melee.combo[1].lunge[0][1]",
                "melee.combo[1].lunge[1][0]",
                "melee.combo[1].lunge[1][1]",
                "melee.combo[1].lunge[2][0]",
                "melee.combo[1].lunge[2][1]",
                "melee.combo[1].swing",
                "melee.combo[2].head_radius",
                "melee.combo[2].lunge[0][0]",
                "melee.combo[2].lunge[0][1]",
                "melee.combo[2].swing",
                "melee.crouch_jump_out",
                "melee.double_jump_after_tick",
                "melee.grind.press_window",
                "melee.jump.hover_rate",
                "melee.jump.hover_ticks",
                "melee.jump.slam_damage",
                "melee.jump_attack.damage",
                "melee.jump_attack.head_radius",
                "melee.jump_attack.hit_to",
                "melee.jump_attack.swing",
                "melee.thrown.catch_ahead_ticks",
                "melee.thrown.radius",
                "name",
                "run_jump.air_speed",
                "strafe.flips",
                "swim.jump.air_speed",
                "walk.speed",
            ]
        );
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
