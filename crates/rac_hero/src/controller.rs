//! The hero's moves, one 60 Hz tick at a time, after `docs/research/hero-controller.md`.
//!
//! [`step`] is a plain function of the hero's state, one tick's input, the camera's yaw,
//! the profile and the collision world. It reads no clock and no global state, so the same
//! inputs always give the same motion, and tests can drive it directly.
//!
//! Directions: y is up, and a yaw of 0 faces −z. The stick is read relative to the
//! camera's yaw, as in the games.
//!
//! Tick counts follow the games' transition code: a rule the game writes as `t > 14` is
//! `ticks > 14` here, checked at the start of a tick.

use bevy::{math::ops, prelude::*};
use rac_input::{Action, PlayerInput};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

mod melee;

pub use melee::{Clank, Strike, StrikeShape, ThrownWrench, best_target, target_score};

use crate::{
    camera::CameraSetting,
    collision::{BodyId, CollisionWorld, Material},
    gun::GunHand,
    motor::{self, Ground},
    profile::{HeroProfile, JumpParams, Spring, SwimProfile, TurnSpring},
    rail::{GrindRail, RailKind, RailPoint},
    stick::{Stick, StickPresses, read_axes, read_stick},
    water,
};

/// Seconds per tick.
pub const TICK: f32 = 1.0 / 60.0;

/// How far down [`step`] looks for the ground under an airborne hero, as the games do.
const GROUND_SEARCH: f32 = 32.0;

/// Ticks of stick history kept, as many as UYA's pad keeps.
const STICK_HISTORY: usize = 30;

/// Ticks of facing kept, as many as UYA's hero keeps (0x1A724C).
const YAW_HISTORY: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JumpKind {
    /// From standing.
    Standing,
    /// From walking.
    Run,
    /// From a running crouch.
    Long,
    /// To a side or back: while strafing, or from a crouch.
    Flip,
    /// From a standing crouch: UYA's jump with the heli-pack's thrust.
    Heli,
    /// Off a wall, after clinging to it.
    Wall,
    /// Up from hanging on a ledge.
    Ledge,
    /// Off a grind rail, still following it.
    Grind,
    /// Across to another rail.
    GrindSwitch,
    /// Out of the water from its surface.
    Water,
}

/// Which way a shimmy goes, seen from the hero facing the wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeroState {
    Idle,
    /// Standing to look round with the first-person button, turning with the camera.
    Look,
    /// Walking, including the walk's own skid after a sharp turn.
    Walk,
    /// Braking to a stop, with no turning: after a long jump's landing or a charge.
    Skid,
    Crouch,
    Fall,
    Jump(JumpKind),
    DoubleJump,
    Glide,
    /// Pulling up to a ledge just caught.
    LedgeGrab,
    LedgeHang,
    LedgeShimmy(Side),
    /// Riding a rail.
    Grind,
    /// A wrench swing while riding a rail, 0 or 1.
    GrindAttack(u8),
    /// Hanging from a zip line and sliding along it.
    ZipLine,
    /// The charge boots' dash.
    Charge,
    /// Bounced back off a wall by a charge.
    Bounce,
    /// Swimming on the surface, and floating there.
    SurfaceSwim,
    SurfaceIdle,
    /// Swimming under water, drifting there, and the fast swim.
    UnderwaterSwim,
    UnderwaterIdle,
    UnderwaterFast,
    /// A swing of the wrench combo, 0 to 2.
    Combo(u8),
    /// The wrench brought down from a jump.
    JumpAttack,
    /// Throwing the wrench, and waiting for it to come back.
    Throw,
    /// Knocked back by a hit.
    Hit,
    Dead,
}

impl HeroState {
    /// The games' number for the state (libuya's `PlayerState`), for comparing with
    /// traces (`hero-controller.md`, "State numbers").
    pub fn game_number(self) -> u32 {
        match self {
            HeroState::Idle => 0,
            HeroState::Look => 1,
            HeroState::Walk => 2,
            HeroState::Skid => 3,
            HeroState::Crouch => 4,
            HeroState::Fall => 6,
            HeroState::Jump(JumpKind::Standing) => 7,
            HeroState::Glide => 8,
            HeroState::Jump(JumpKind::Run) => 9,
            HeroState::Jump(JumpKind::Long) => 10,
            HeroState::Jump(JumpKind::Flip) => 11,
            HeroState::DoubleJump => 14,
            HeroState::Jump(JumpKind::Heli) => 15,
            HeroState::Jump(JumpKind::Wall) => 17,
            HeroState::Jump(JumpKind::Water) => 18,
            HeroState::Combo(_) => 19,
            HeroState::JumpAttack => 20,
            HeroState::Throw => 21,
            HeroState::Hit => 22,
            HeroState::LedgeGrab => 23,
            HeroState::LedgeHang => 24,
            HeroState::LedgeShimmy(Side::Left) => 25,
            HeroState::LedgeShimmy(Side::Right) => 26,
            HeroState::Jump(JumpKind::Ledge) => 27,
            HeroState::Grind => 39,
            HeroState::GrindAttack(_) => 42,
            HeroState::Jump(JumpKind::Grind) => 40,
            HeroState::Jump(JumpKind::GrindSwitch) => 41,
            HeroState::UnderwaterSwim => 50,
            HeroState::UnderwaterIdle => 51,
            HeroState::UnderwaterFast => 52,
            HeroState::SurfaceSwim => 53,
            HeroState::SurfaceIdle => 54,
            HeroState::Dead => 57,
            HeroState::ZipLine => 118,
            HeroState::Bounce => 121,
            HeroState::Charge => 125,
        }
    }

    pub fn is_airborne(self) -> bool {
        matches!(
            self,
            HeroState::Fall | HeroState::Jump(_) | HeroState::DoubleJump | HeroState::Glide
        )
    }

    pub fn is_on_ledge(self) -> bool {
        matches!(
            self,
            HeroState::LedgeGrab | HeroState::LedgeHang | HeroState::LedgeShimmy(_)
        )
    }

    pub fn is_on_surface(self) -> bool {
        matches!(self, HeroState::SurfaceSwim | HeroState::SurfaceIdle)
    }

    pub fn is_underwater(self) -> bool {
        matches!(
            self,
            HeroState::UnderwaterSwim | HeroState::UnderwaterIdle | HeroState::UnderwaterFast
        )
    }

    pub fn is_swimming(self) -> bool {
        self.is_on_surface() || self.is_underwater()
    }

    pub fn is_attacking(self) -> bool {
        matches!(
            self,
            HeroState::Combo(_)
                | HeroState::JumpAttack
                | HeroState::Throw
                | HeroState::GrindAttack(_)
        )
    }

    /// Riding a rail, swinging or not.
    pub fn is_grinding(self) -> bool {
        matches!(self, HeroState::Grind | HeroState::GrindAttack(_))
    }
}

/// Which way the stick points while strafing, relative to the camera: UYA's four 90°
/// sectors (0x4EAA78), where an exact diagonal counts as forward.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrafeSide {
    Left,
    Right,
    #[default]
    Forward,
    Back,
}

impl StrafeSide {
    /// The sector of a stick vector, x to the right and y forward.
    pub fn of(stick: Vec2) -> Self {
        if stick.x > stick.y.abs() {
            StrafeSide::Right
        } else if -stick.x > stick.y.abs() {
            StrafeSide::Left
        } else if -stick.y > stick.x.abs() {
            StrafeSide::Back
        } else {
            StrafeSide::Forward
        }
    }
}

/// Moves the games grant as items rather than from the start (PLAN.md section 5,
/// 2026-10-12). Each game sets them from its own items and save, so a game only gets the
/// moves it had; the kit's demo turns them all on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Abilities {
    /// Clank's heli-pack on the hero's back: the long jump, the heli jump and the glide.
    /// UYA's code checks the back slot for them; without it a jump from a crouch is a
    /// standing jump.
    pub heli_pack: bool,
    /// The charge boots: a double tap of crouch dashes off.
    pub charge_boots: bool,
    /// The grind boots: landing on a rail rides it.
    pub grind_boots: bool,
}

impl Abilities {
    pub const ALL: Abilities = Abilities {
        heli_pack: true,
        charge_boots: true,
        grind_boots: true,
    };
    pub const NONE: Abilities = Abilities {
        heli_pack: false,
        charge_boots: false,
        grind_boots: false,
    };
}

/// A ledge found by [`detect_ledge`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ledge {
    /// Where the hero's feet hang from it.
    pub hang: Vec3,
    /// The top edge.
    pub lip: Vec3,
    /// Horizontal, out of the wall below the lip.
    pub normal: Vec3,
    pub body: BodyId,
}

/// A wall in reach for a wall jump.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Wall {
    /// Horizontal, out of the wall.
    normal: Vec3,
    material: Material,
}

/// A jump's vertical state (`hero-controller.md`, "Vertical model").
#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct JumpBlock {
    /// The height the jump is aiming for.
    height: f32,
    /// How much `height` grows per tick while the button is held.
    step: f32,
    holds_left: u32,
    /// Still holding since take-off. Once released, the height stops growing.
    holding: bool,
    /// Upward speed already given.
    lift: f32,
    /// Gravity now: the move's own until the apex, then possibly more.
    gravity: f32,
    past_apex: bool,
    /// A ceiling was hit, so no more lift.
    blocked: bool,
}

/// Ticks since a button was last pressed, so presses can be buffered as the games do.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PressAge {
    ticks: u32,
    /// The press has already started a move.
    used: bool,
}

impl PressAge {
    const NEVER: PressAge = PressAge {
        ticks: u32::MAX,
        used: true,
    };

    fn update(&mut self, pressed: bool) {
        if pressed {
            *self = PressAge {
                ticks: 0,
                used: false,
            };
        } else {
            self.ticks = self.ticks.saturating_add(1);
        }
    }

    /// An unused press in the last `window` ticks.
    fn within(&self, window: u32) -> bool {
        !self.used && self.ticks < window
    }
}

/// The water around the hero this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WaterLevel {
    surface: f32,
    /// From the surface down to the floor, up to 32 u.
    depth: f32,
}

/// What a tick did that the caller may want to act on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StepEvents {
    /// The hero stands on a [`Material::Hazard`] and can be hurt.
    pub hazard: bool,
}

/// Everything the controller keeps about one hero between ticks.
#[derive(Debug, Clone, PartialEq)]
pub struct HeroMotion {
    /// Where his feet are.
    pub position: Vec3,
    pub yaw: f32,
    /// Radians per tick.
    pub yaw_rate: f32,
    /// u/s. On the ground the horizontal part is `speed` along the facing, apart from
    /// strafing.
    pub velocity: Vec3,
    /// Ground speed, u/s.
    pub speed: f32,
    pub state: HeroState,
    /// Ticks since the state began; 0 on its first tick.
    pub ticks: u32,
    pub ground: Option<Ground>,
    /// How far below his feet the ground is, up to 32 u.
    pub height_above_ground: f32,
    /// The ledge held, while on one.
    pub ledge: Option<Ledge>,
    /// Ticks left before he can be hurt again.
    pub safe_ticks: u32,
    /// The moves he has the items for.
    pub abilities: Abilities,
    /// Ticks left of ignoring the stick and buttons, as after a charge or a far fall.
    pub input_lock: u32,
    /// The fall under way has lasted or dropped far enough to land heavily.
    pub fell_far: bool,
    /// The rail ridden, or followed by a jump off it, or the zip line hung from.
    pub rail: Option<RailPoint>,
    /// How far his nose points down from level, in radians: swimming under water.
    pub pitch: f32,
    /// The thrown wrench, while it is out of his hand.
    pub thrown: Option<ThrownWrench>,
    /// What the wrench can hit this tick.
    pub strikes: Vec<Strike>,
    /// Where the wrench clanked off something this tick, for sparks and a sound.
    pub clank: Option<Clank>,
    /// Strafe or lock-on is held: on the ground he faces the camera and moves with the
    /// stick (`hero-controller.md`, "Strafing").
    pub strafing: bool,
    /// Where the stick last pointed while strafing.
    pub strafe_side: StrafeSide,
    /// Which way the current or last flip went, relative to his facing; its animation
    /// follows it.
    pub flip_side: StrafeSide,
    /// Strafing backwards with the stick past `strafe.back_stick`, which walks backwards.
    pub strafing_back: bool,
    /// The camera's yaw this tick, which strafing turns him to.
    camera_yaw: f32,
    /// The camera looks where he does in [`HeroState::Look`], so the look turns him with
    /// it. The look's camera sets it false while it blends in (`camera.md`, "Which camera
    /// mode shows it").
    pub look_ready: bool,
    /// The setting of the camera he is seen through, which the camera sets.
    pub camera_setting: CameraSetting,
    /// What is in his hand and whether it fires, which the game's weapons code sets each
    /// tick before he moves.
    pub gun: GunHand,
    /// The strafe's own turning rate, kept apart from the walk's, radians per tick.
    strafe_yaw_rate: f32,
    /// The way he strafes: the stick's direction on the ground, kept when it is let go.
    strafe_heading: Vec3,
    /// Ticks left of the damping that follows strafing right, and left.
    after_right: u32,
    after_left: u32,
    /// The state before this one.
    previous_state: HeroState,
    /// Walking, but not yet past the setting-off speed.
    setting_off: bool,
    /// The walk's skid after a sharp turn.
    skidding: bool,
    /// The walk moves the way the stick points, not the way he faces.
    sliding: bool,
    /// The stick's angle over the last ticks, newest at `stick_newest`, as the games keep
    /// it to spot a stick swung round.
    stick_angles: [f32; STICK_HISTORY],
    stick_newest: usize,
    /// Ticks of braking done since the stick was released.
    stop_ticks: u32,
    /// Ground ticks since a jump-type move last landed, in which a jump can still flip
    /// (`strafe.landing_window`).
    landed_ticks: u32,
    jump: JumpBlock,
    take_off_height: f32,
    double_jumped: bool,
    flip_direction: Vec3,
    flip_speed: f32,
    /// The flip moves as the strafe flip; otherwise as the crouch flip, which turns back
    /// to `flip_facing` and carries `flip_carry` (u/s) along it.
    flip_strafing: bool,
    flip_facing: f32,
    flip_carry: f32,
    /// The crouch flip found no ground ahead to land on this tick.
    flip_over_drop: bool,
    /// His yaw at the start of each tick, newest at `yaw_newest`.
    yaw_history: [f32; YAW_HISTORY],
    yaw_newest: usize,
    /// The long jump's base speed, before its per-tick push.
    long_jump_base: f32,
    /// The glide's vertical spring rate, u/s per tick.
    glide_rate: f32,
    jump_press: PressAge,
    crouch_press: PressAge,
    /// The stick pushed a new way ([`StickPresses`]). Never used up, as in the game.
    stick_press: PressAge,
    stick_presses: StickPresses,
    /// A wall in reach, and for how many more ticks.
    wall: Option<Wall>,
    wall_ticks: u32,
    /// The wall the current wall jump left from.
    last_wall: Option<Vec3>,
    /// Wall jumps in a row off ordinary walls.
    ordinary_wall_jumps: u32,
    /// Ticks left before a ledge can be grabbed again.
    no_grab_ticks: u32,
    /// The climb's forward speed.
    climb_speed: f32,
    /// The current hit kills him when it ends.
    hit_fatal: bool,
    /// Ticks left in which a crouch press starts a charge.
    charge_window: u32,
    /// Ticks left before a charge can start again.
    charge_cooldown: u32,
    /// The charge's eased sideways stick.
    charge_steer: f32,
    /// The charge's sinking rate off the ground, u per tick.
    charge_sink: f32,
    /// The skid after a charge, whose slide stops when the controls come back.
    charge_skid: bool,
    /// The grind's base speed and slope term, and their sum, u/s.
    grind_base: f32,
    grind_slope: f32,
    grind_speed: f32,
    /// The way the rail ridden goes.
    rail_yaw: f32,
    /// How far the hero still is from the rail he got on, and how fast he closes it.
    rail_offset: Vec3,
    rail_pull: f32,
    /// The rail a switch jump is leaving, and how far across it is.
    switch_from: Option<RailPoint>,
    switch_ticks: u32,
    /// Ticks left before a rail can be got on again.
    no_rail_ticks: u32,
    /// The speed along a zip line, u/s.
    zip_speed: f32,
    /// Ticks since a zip line ran out under him; 0 while on it.
    zip_end: u32,
    /// Ticks left before a zip line can be caught again.
    no_zip_ticks: u32,
    /// The water he is in or over, found each tick.
    water: Option<WaterLevel>,
    /// The pitch's spring rate, radians per tick.
    pitch_rate: f32,
    /// On the surface: the height his feet float at follows the water's, and an offset
    /// bobs about it (u, and u per tick).
    float_height: f32,
    float_rate: f32,
    bob: f32,
    bob_rate: f32,
    bob_spring_rate: f32,
    /// The velocity he got into the water with, drifting on, u/s.
    drift: Vec3,
    /// The swimming stroke's animation frame, and how far it moved last tick.
    stroke_frame: f32,
    stroke_step: f32,
    /// Ticks left of a surface stroke that finishes whatever the stick does.
    stroke_left: u32,
    /// Ticks left of a dive from the surface.
    dive_left: u32,
    melee_press: PressAge,
    strafe_press: PressAge,
    fire_press: PressAge,
    /// The attack under way, or the last one.
    attack: melee::Attack,
    /// Attacks and throws so far, which number them.
    attack_serial: u32,
}

impl HeroMotion {
    pub fn new(position: Vec3, yaw: f32) -> Self {
        Self {
            position,
            yaw,
            yaw_rate: 0.0,
            velocity: Vec3::ZERO,
            speed: 0.0,
            state: HeroState::Idle,
            ticks: 0,
            ground: None,
            height_above_ground: 0.0,
            ledge: None,
            safe_ticks: 0,
            abilities: Abilities::ALL,
            input_lock: 0,
            fell_far: false,
            rail: None,
            pitch: 0.0,
            thrown: None,
            strikes: Vec::new(),
            clank: None,
            look_ready: true,
            camera_setting: CameraSetting::ThirdPerson,
            gun: GunHand::default(),
            strafing: false,
            strafe_side: StrafeSide::Forward,
            flip_side: StrafeSide::Forward,
            strafing_back: false,
            camera_yaw: yaw,
            strafe_yaw_rate: 0.0,
            strafe_heading: facing(yaw),
            after_right: 0,
            after_left: 0,
            previous_state: HeroState::Idle,
            setting_off: true,
            skidding: false,
            sliding: false,
            stick_angles: [0.0; STICK_HISTORY],
            stick_newest: 0,
            stop_ticks: 0,
            landed_ticks: u32::MAX,
            jump: JumpBlock::default(),
            take_off_height: position.y,
            double_jumped: false,
            flip_direction: Vec3::ZERO,
            flip_speed: 0.0,
            flip_strafing: false,
            flip_facing: yaw,
            flip_carry: 0.0,
            flip_over_drop: false,
            yaw_history: [yaw; YAW_HISTORY],
            yaw_newest: 0,
            long_jump_base: 0.0,
            glide_rate: 0.0,
            jump_press: PressAge::NEVER,
            crouch_press: PressAge::NEVER,
            stick_press: PressAge::NEVER,
            stick_presses: StickPresses::default(),
            wall: None,
            wall_ticks: 0,
            last_wall: None,
            ordinary_wall_jumps: 0,
            no_grab_ticks: 0,
            climb_speed: 0.0,
            hit_fatal: false,
            charge_window: 0,
            charge_cooldown: 0,
            charge_steer: 0.0,
            charge_sink: 0.0,
            charge_skid: false,
            grind_base: 0.0,
            grind_slope: 0.0,
            grind_speed: 0.0,
            rail_yaw: 0.0,
            rail_offset: Vec3::ZERO,
            rail_pull: 0.0,
            switch_from: None,
            switch_ticks: 0,
            no_rail_ticks: 0,
            zip_speed: 0.0,
            zip_end: 0,
            no_zip_ticks: 0,
            water: None,
            pitch_rate: 0.0,
            float_height: 0.0,
            float_rate: 0.0,
            bob: 0.0,
            bob_rate: 0.0,
            bob_spring_rate: 0.0,
            drift: Vec3::ZERO,
            stroke_frame: 0.0,
            stroke_step: 0.0,
            stroke_left: 0,
            dive_left: 0,
            melee_press: PressAge::NEVER,
            strafe_press: PressAge::NEVER,
            fire_press: PressAge::NEVER,
            attack: melee::Attack::default(),
            attack_serial: 0,
        }
    }

    /// The hero with only the moves in `abilities`.
    pub fn with_abilities(mut self, abilities: Abilities) -> Self {
        self.abilities = abilities;
        self
    }

    pub fn facing(&self) -> Vec3 {
        facing(self.yaw)
    }

    /// Dead long enough to come back at a checkpoint.
    pub fn respawn_due(&self, profile: &HeroProfile) -> bool {
        self.state == HeroState::Dead && self.ticks >= profile.death.ticks
    }

    /// Whether a hit now would hurt him.
    pub fn can_be_hurt(&self) -> bool {
        self.safe_ticks == 0 && self.state != HeroState::Dead
    }

    /// Keeps him from being hurt again for `ticks`.
    pub fn stay_safe_for(&mut self, ticks: u32) {
        self.safe_ticks = ticks;
    }

    /// Coming down: past a jump's apex, or falling in any other airborne state.
    pub fn past_apex(&self) -> bool {
        match self.state {
            HeroState::Jump(_) => self.jump.past_apex,
            _ => self.velocity.y <= 0.0,
        }
    }

    /// Marks a jump as past its apex, for tests of what follows it.
    #[cfg(test)]
    pub(crate) fn pass_apex(&mut self) {
        self.jump.past_apex = true;
    }

    /// About how many ticks until he lands on the ground below, at his vertical speed and
    /// the jump's gravity (`fall_gravity` outside jumps).
    pub fn ticks_to_land(&self, fall_gravity: f32) -> f32 {
        let gravity = match self.state {
            HeroState::Jump(_) | HeroState::DoubleJump if self.jump.gravity > 0.0 => {
                self.jump.gravity
            }
            _ => fall_gravity,
        };
        if gravity <= 0.0 {
            return f32::INFINITY;
        }
        let up = self.velocity.y;
        let seconds =
            (up + ops::sqrt(up * up + 2.0 * gravity * self.height_above_ground)) / gravity;
        seconds / TICK
    }

    /// The whole ticks until a jump's apex at its vertical speed and gravity, or 0 outside
    /// a jump or coming down (UYA's `hero_ticks_to_apex`, which times its animation).
    pub fn ticks_to_apex(&self) -> f32 {
        let gravity = self.jump.gravity;
        let jumping = matches!(self.state, HeroState::Jump(_) | HeroState::DoubleJump);
        if !jumping || gravity <= 0.0 || self.velocity.y <= 0.0 {
            return 0.0;
        }
        (self.velocity.y / gravity / TICK).floor()
    }

    fn set_state(&mut self, state: HeroState) {
        if self.state != state {
            self.restart_state(state);
        }
    }

    /// Enters `state` afresh, even if he is already in it, as a new attack does.
    fn restart_state(&mut self, state: HeroState) {
        self.previous_state = self.state;
        self.state = state;
        self.ticks = 0;
        self.skidding = false;
        self.sliding = false;
        self.charge_skid = false;
        self.dive_left = 0;
        self.fell_far = false;
    }

    fn record_stick(&mut self, stick: &Stick) {
        self.stick_newest = (self.stick_newest + 1) % STICK_HISTORY;
        // The games' atan2 gives 0 for a released stick, and so does this one.
        self.stick_angles[self.stick_newest] = ops::atan2(stick.vector.y, stick.vector.x);
    }

    fn record_yaw(&mut self) {
        self.yaw_newest = (self.yaw_newest + 1) % YAW_HISTORY;
        self.yaw_history[self.yaw_newest] = self.yaw;
    }

    /// His yaw at the start of the tick `ticks` before this one.
    fn yaw_back(&self, ticks: usize) -> f32 {
        let ticks = ticks.min(YAW_HISTORY - 1);
        self.yaw_history[(self.yaw_newest + YAW_HISTORY - ticks) % YAW_HISTORY]
    }

    /// How far the stick's direction turned over the last `ticks`.
    fn stick_swing(&self, ticks: usize) -> f32 {
        let ticks = ticks.min(STICK_HISTORY - 1);
        let then = (self.stick_newest + STICK_HISTORY - ticks) % STICK_HISTORY;
        wrap(self.stick_angles[self.stick_newest] - self.stick_angles[then]).abs()
    }

    fn horizontal_velocity(&self) -> Vec3 {
        Vec3::new(self.velocity.x, 0.0, self.velocity.z)
    }

    fn set_horizontal_velocity(&mut self, horizontal: Vec3) {
        self.velocity.x = horizontal.x;
        self.velocity.z = horizontal.z;
    }

    /// Pressed to the ground at the start of a jump. A wall jump's push-off is spent on
    /// the wall and a climb's on the ledge, so they don't count.
    fn pushing_off_ground(&self, profile: &HeroProfile) -> bool {
        match self.state {
            HeroState::Jump(
                JumpKind::Wall
                | JumpKind::Ledge
                | JumpKind::Grind
                | JumpKind::GrindSwitch
                | JumpKind::Water,
            ) => false,
            HeroState::Jump(kind) => self.ticks < jump_params(profile, kind).pushoff_ticks,
            _ => false,
        }
    }

    fn let_go(&mut self, no_grab_ticks: u32) {
        self.ledge = None;
        self.no_grab_ticks = no_grab_ticks;
        self.velocity = Vec3::ZERO;
        self.take_off_height = self.position.y;
        self.double_jumped = true;
        self.set_state(HeroState::Fall);
    }
}

fn facing(yaw: f32) -> Vec3 {
    Vec3::new(-ops::sin(yaw), 0.0, -ops::cos(yaw))
}

fn yaw_of(direction: Vec3) -> f32 {
    ops::atan2(-direction.x, -direction.z)
}

/// `angle` brought into −π..π.
fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// `value` moved towards `target` by at most `step`.
fn approach(value: f32, target: f32, step: f32) -> f32 {
    if value < target {
        (value + step).min(target)
    } else {
        (value - step).max(target)
    }
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z).normalize_or_zero()
}

fn flat_length(v: Vec3) -> f32 {
    Vec3::new(v.x, 0.0, v.z).length()
}

/// The stick as a direction on the ground, relative to the camera. Its length is the
/// stick's, which can be up to √2 on a diagonal.
fn stick_on_ground(stick: Vec2, camera_yaw: f32) -> Vec3 {
    let forward = facing(camera_yaw);
    let right = Vec3::new(ops::cos(camera_yaw), 0.0, -ops::sin(camera_yaw));
    right * stick.x + forward * stick.y
}

/// One tick of a turning spring (`hero-controller.md`, "Turning").
fn turn(hero: &mut HeroMotion, target_yaw: f32, spring: TurnSpring) {
    let error = wrap(target_yaw - hero.yaw);
    // The rate may not carry the yaw past the target.
    if hero.yaw_rate.signum() == error.signum() && hero.yaw_rate.abs() > error.abs() {
        hero.yaw_rate = error;
    }
    hero.yaw_rate += spring.gain * error - spring.damping * hero.yaw_rate;
    hero.yaw_rate = hero.yaw_rate.clamp(-spring.cap, spring.cap);
    hero.yaw = wrap(hero.yaw + hero.yaw_rate);
    if wrap(target_yaw - hero.yaw).abs() < 0.01 * spring.cap {
        hero.yaw = wrap(target_yaw);
        hero.yaw_rate = 0.0;
    }
}

pub fn jump_params(profile: &HeroProfile, kind: JumpKind) -> &JumpParams {
    match kind {
        JumpKind::Standing => &profile.jump,
        JumpKind::Run => &profile.run_jump,
        JumpKind::Long => &profile.long_jump.params,
        JumpKind::Flip => &profile.flip.params,
        JumpKind::Heli => &profile.heli_jump.params,
        JumpKind::Wall => &profile.wall_jump.params,
        JumpKind::Ledge => &profile.ledge.climb,
        JumpKind::Grind => &profile.grind.jump,
        JumpKind::GrindSwitch => &profile.grind.switch_jump,
        JumpKind::Water => &profile.swim.jump,
    }
}

/// How far below `feet` the ground is, up to [`GROUND_SEARCH`].
pub fn height_above_ground(world: &dyn CollisionWorld, feet: Vec3) -> f32 {
    // Start a little above the feet so ground they rest on is found.
    const LIFT: f32 = 0.05;
    world
        .cast_ray(feet + Vec3::Y * LIFT, Vec3::NEG_Y * (GROUND_SEARCH + LIFT))
        .map_or(GROUND_SEARCH, |hit| {
            (hit.fraction * (GROUND_SEARCH + LIFT) - LIFT).max(0.0)
        })
}

/// Looks for a ledge in front of a hero standing at `feet` and facing `yaw`, the way the
/// games do (`hero-controller.md`, "Ledges"). `height_above_ground` is the hero's own,
/// even when `feet` is somewhere else.
pub fn detect_ledge(
    world: &dyn CollisionWorld,
    profile: &HeroProfile,
    feet: Vec3,
    yaw: f32,
    height_above_ground: f32,
) -> Option<Ledge> {
    let ledge = &profile.ledge;
    let forward = facing(yaw);
    let front = feet + forward * (profile.body.radius + ledge.probe_ahead);
    let flattest = ops::cos(ledge.max_top_slope_degrees.to_radians());

    // 1. A flat top, cast down in front of the capsule, a little further out each try.
    let span = ledge.probe_above + ledge.probe_below;
    let top = (1..=ledge.probe_tries).find_map(|i| {
        let from = front + forward * (ledge.probe_step * i as f32) + Vec3::Y * ledge.probe_above;
        let hit = world.cast_ray(from, Vec3::NEG_Y * span)?;
        let usable = hit.fraction > 0.0
            && hit.normal.y >= flattest
            && world.material(hit.body) != Material::NoGrab;
        usable.then(|| (from + Vec3::NEG_Y * span * hit.fraction, hit.body))
    });
    let (top, body) = top?;

    // 2. High enough to hang from, but not so high it can't be reached, and well above
    // the ground below, so low ledges are landed on instead.
    let rise = top.y - feet.y;
    if rise > ledge.max_rise || rise < ledge.min_rise || rise + height_above_ground < ledge.min_drop
    {
        return None;
    }

    // 3. Nothing in the way just above the lip, back towards the hero (not in DL).
    let above_lip = top + Vec3::Y * 0.005;
    if ledge.clear_back > 0.0
        && let Some(hit) = world.cast_ray(
            above_lip - forward * ledge.clear_back,
            forward * ledge.clear_back,
        )
        && hit.body != body
    {
        return None;
    }

    // 4. The wall face under the lip, from the hero's axis.
    let normal = (0..ledge.face_tries).find_map(|i| {
        let height = top.y - ledge.face_below - ledge.face_step * i as f32;
        let from = Vec3::new(feet.x, height, feet.z);
        let to = Vec3::new(top.x, height, top.z) + forward * ledge.face_reach;
        let hit = world.cast_ray(from, to - from)?;
        let normal = flat(hit.normal);
        (normal != Vec3::ZERO).then_some(normal)
    })?;

    // 5. The lip: walk from the top's hit back out over the wall until the top runs out
    // (DL's walk never moves: `lip_walk`).
    let mut lip = top;
    let mut searched = 0.0;
    if ledge.lip_walk {
        loop {
            let from = lip + normal * ledge.lip_step + Vec3::Y * 0.05;
            match world.cast_ray(from, Vec3::NEG_Y * 0.1) {
                Some(hit) => lip = from + Vec3::NEG_Y * 0.1 * hit.fraction,
                None => break,
            }
            searched += ledge.lip_step;
            if searched >= ledge.lip_search {
                return None;
            }
        }
    }

    Some(Ledge {
        hang: lip + normal * ledge.hang_out - Vec3::Y * ledge.hang_below,
        lip,
        normal,
        body,
    })
}

/// Knocks the hero back with `knockback` (state 22, `hero-controller.md`, "Getting hit
/// and dying"). With `fatal` he dies when the hit is over.
pub fn hurt(hero: &mut HeroMotion, knockback: Vec3, fatal: bool, profile: &HeroProfile) {
    if hero.state == HeroState::Dead {
        return;
    }
    hero.ledge = None;
    hero.rail = None;
    hero.wall = None;
    hero.velocity = knockback.clamp_length_max(profile.hit.max_speed);
    hero.speed = hero.horizontal_velocity().length();
    if hero.velocity.y > 0.0 {
        hero.ground = None;
    }
    hero.safe_ticks = profile.hit.safe_ticks;
    hero.hit_fatal = fatal;
    hero.state = HeroState::Hit;
    hero.ticks = 0;
}

/// The push a hazard surface gives: up, and back the way he came.
pub fn hazard_knockback(hero: &HeroMotion, profile: &HeroProfile) -> Vec3 {
    -hero.facing() * profile.hit.hazard_knockback + Vec3::Y * profile.hit.hazard_lift
}

/// Kills the hero at once.
pub fn kill(hero: &mut HeroMotion) {
    hero.ledge = None;
    hero.rail = None;
    hero.set_state(HeroState::Dead);
}

/// Advances `hero` by one tick.
pub fn step(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    camera_yaw: f32,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) -> StepEvents {
    let stick = read_stick(input.move_stick, &profile.stick);
    let wish = stick_on_ground(stick.vector, camera_yaw);
    hero.record_stick(&stick);
    let axes = read_axes(input.move_stick, &profile.stick);
    let pushed = hero.stick_presses.update(axes, &profile.stick.flick);
    hero.stick_press.update(pushed);
    hero.record_yaw();
    hero.camera_yaw = camera_yaw;
    // Locking on holds both strafe buttons in UYA, so it strafes too, and in the
    // lock-strafe and first-person cameras so does anything in hand but the wrench
    // (`camera.md`, "The hero in these modes").
    let camera_strafes = hero.gun.out && hero.camera_setting != CameraSetting::ThirdPerson;
    hero.strafing = input.held(Action::Strafe) || input.held(Action::LockOn) || camera_strafes;
    if !hero.strafing {
        hero.strafing_back = false;
    } else if stick.is_active() {
        hero.strafe_side = StrafeSide::of(stick.vector);
        hero.strafing_back =
            hero.strafe_side == StrafeSide::Back && stick.magnitude > profile.strafe.back_stick;
        hero.strafe_heading = wish.normalize_or_zero();
    }

    hero.jump_press.update(input.pressed(Action::Jump));
    hero.crouch_press.update(input.pressed(Action::Crouch));
    hero.melee_press.update(input.pressed(Action::Melee));
    hero.strafe_press.update(input.pressed(Action::Strafe));
    hero.fire_press.update(input.pressed(Action::Fire));
    hero.wall_ticks = hero.wall_ticks.saturating_sub(1);
    if hero.wall_ticks == 0 {
        hero.wall = None;
    }
    hero.no_grab_ticks = hero.no_grab_ticks.saturating_sub(1);
    hero.safe_ticks = hero.safe_ticks.saturating_sub(1);
    hero.input_lock = hero.input_lock.saturating_sub(1);
    hero.charge_cooldown = hero.charge_cooldown.saturating_sub(1);
    hero.no_rail_ticks = hero.no_rail_ticks.saturating_sub(1);
    hero.no_zip_ticks = hero.no_zip_ticks.saturating_sub(1);

    // Ride whatever he stands on or hangs from.
    if let Some(ground) = hero.ground {
        hero.position += world.body_motion(ground.body);
    }
    if hero.state.is_on_ledge()
        && let Some(ledge) = hero.ledge.as_mut()
    {
        let motion = world.body_motion(ledge.body);
        ledge.hang += motion;
        ledge.lip += motion;
        hero.position += motion;
    }
    hero.height_above_ground = height_above_ground(world, hero.position);
    hero.flip_over_drop = flip_over_drop(hero, profile, world);

    // The charge boots: a crouch press within the window the previous one opened dashes
    // off.
    let crouch_tap = input.pressed(Action::Crouch);
    if crouch_tap && hero.charge_window > 0 && can_charge(hero, profile) {
        start_charge(hero);
    }
    hero.charge_window = hero.charge_window.saturating_sub(1);
    if crouch_tap {
        hero.charge_window = profile.charge.tap_window;
    }

    try_get_on_rail(hero, profile, world);
    try_catch_zip_line(hero, profile, world);
    hero.water = find_water(world, hero.position);
    enter_water(hero, profile);
    if hero.state.is_attacking() {
        melee::advance_frame(hero, profile);
    }
    melee::try_attack(hero, input, &stick, wish, profile, world);

    match hero.state {
        HeroState::Dead => dead_tick(hero, profile),
        HeroState::Hit => hit_tick(hero, profile),
        HeroState::Charge => charge_tick(hero, &stick, profile),
        HeroState::Bounce => bounce_tick(hero, profile),
        state if state.is_grinding() => grind_tick(hero, input, &stick, wish, profile, world),
        HeroState::ZipLine => zip_line_tick(hero, profile, world),
        state if state.is_on_surface() => surface_tick(hero, input, &stick, wish, profile),
        state if state.is_underwater() => underwater_tick(hero, input, &stick, wish, profile),
        state if state.is_on_ledge() => ledge_tick(hero, input, &stick, wish, profile, world),
        state if state.is_airborne() => air_tick(hero, input, &stick, wish, profile),
        state if state.is_attacking() => {
            melee::attack_tick(hero, input, &stick, wish, profile, world);
        }
        _ => ground_tick(hero, input, &stick, wish, profile),
    }
    // Counted before the move, so a landing this tick leaves 0 for the next.
    hero.landed_ticks = hero.landed_ticks.saturating_add(1);

    follow_rail_in_air(hero, &stick, wish, profile, world);
    move_hero(hero, &stick, profile, world);
    melee::thrown_tick(hero, profile, world);
    melee::update_strikes(hero, profile);
    melee::clank(hero, profile, world);

    let mut events = StepEvents::default();
    if let Some(ground) = hero.ground {
        match world.material(ground.body) {
            Material::Hazard if hero.can_be_hurt() => events.hazard = true,
            Material::Deadly if hero.state != HeroState::Dead => kill(hero),
            _ => {}
        }
    }
    events
}

/// Moves the capsule by this tick's velocity and settles what that means: landing,
/// leaving the ground, and the walls and ledges now in reach.
fn move_hero(
    hero: &mut HeroMotion,
    stick: &Stick,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let state = hero.state;
    if state.is_grinding() || state == HeroState::ZipLine {
        // The rail carries him; nothing else stops him.
        hero.position += hero.velocity * TICK;
        hero.ground = None;
        hero.ticks += 1;
        hero.height_above_ground = height_above_ground(world, hero.position);
        return;
    }
    if state.is_swimming() {
        // Water holds him up, so he slides along whatever he swims into, the floor too.
        let outcome = motor::move_body(
            world,
            &profile.body,
            hero.position,
            hero.velocity * TICK,
            false,
        );
        hero.position = outcome.position;
        hero.ground = None;
        hero.ticks += 1;
        hero.height_above_ground = height_above_ground(world, hero.position);
        return;
    }
    let on_ground = !state.is_airborne() && !state.is_on_ledge();
    let pushing_off = hero.pushing_off_ground(profile);
    // Hits, deaths, bounces, charges and attacks keep to the ground while they are on it.
    let keeps_to_ground = state.is_attacking()
        || matches!(
            state,
            HeroState::Hit | HeroState::Dead | HeroState::Bounce | HeroState::Charge
        );
    let grounded = match state {
        // A charge runs up slopes too.
        HeroState::Charge => hero.ground.is_some(),
        _ if keeps_to_ground => hero.ground.is_some() && hero.velocity.y <= 0.0,
        _ => on_ground || pushing_off,
    };
    let start = hero.position;
    // A charge would outrun the collision's step limit, so the game caps the velocity.
    let horizontal = hero.horizontal_velocity();
    hero.set_horizontal_velocity(horizontal.clamp_length_max(profile.body.max_step / TICK));
    let outcome = motor::move_body(
        world,
        &profile.body,
        hero.position,
        hero.velocity * TICK,
        grounded,
    );
    hero.position = outcome.position;
    hero.ticks += 1;

    if state.is_on_ledge() {
        hero.ground = None;
        return;
    }
    if keeps_to_ground {
        if outcome.ground.is_some() && (grounded || hero.velocity.y <= 0.0) {
            hero.velocity.y = 0.0;
            hero.ground = outcome.ground;
        } else {
            hero.ground = None;
        }
        if state == HeroState::Charge && outcome.hit_wall {
            let intended = flat_length(hero.velocity * TICK);
            let moved = flat_length(hero.position - start);
            if moved < profile.charge.blocked_below * intended || wall_ahead(hero, profile, world) {
                start_bounce(hero, profile);
            }
        }
        return;
    }
    if on_ground {
        hero.ground = outcome.ground;
        if outcome.ground.is_none() {
            // Walked off an edge.
            hero.velocity.y = 0.0;
            hero.take_off_height = hero.position.y;
            hero.double_jumped = true;
            hero.set_state(HeroState::Fall);
        }
        return;
    }
    if pushing_off {
        hero.ground = outcome.ground;
        return;
    }
    if outcome.hit_ceiling && hero.velocity.y > 0.0 {
        hero.velocity.y = 0.0;
        hero.jump.blocked = true;
    }
    if let Some(ground) = outcome.ground
        && hero.velocity.y <= 0.0
    {
        land(hero, ground, stick, profile);
        return;
    }
    hero.ground = None;
    hero.height_above_ground = height_above_ground(world, hero.position);

    // DL looks for no walls (`wall_jump.enabled`).
    if profile.wall_jump.enabled
        && matches!(
            state,
            HeroState::Jump(JumpKind::Standing | JumpKind::Run | JumpKind::Wall)
        )
        && let Some(wall) = detect_wall(hero, profile, world)
    {
        hero.wall = Some(wall);
        hero.wall_ticks =
            profile.wall_jump.window_ticks + u32::from(state == HeroState::Jump(JumpKind::Wall));
    }
    try_grab(hero, profile, world);
}

fn land(hero: &mut HeroMotion, ground: Ground, stick: &Stick, profile: &HeroProfile) {
    let horizontal = hero.horizontal_velocity();
    let landing_from_long_jump = hero.state == HeroState::Jump(JumpKind::Long);
    let landing_from_fall = hero.state == HeroState::Fall;
    // A jump pressed now can still flip (`strafe.landing_window`); the long jump skids
    // instead.
    if matches!(hero.state, HeroState::Jump(_) | HeroState::DoubleJump) && !landing_from_long_jump {
        hero.landed_ticks = 0;
    }
    hero.ground = Some(ground);
    hero.velocity.y = 0.0;
    hero.speed = match hero.state {
        // A crouch flip lands with the speed it carried, a back flip with none (UYA
        // 0x4EDE80).
        HeroState::Jump(JumpKind::Flip) if !hero.flip_strafing => {
            if hero.flip_side == StrafeSide::Back {
                0.0
            } else {
                hero.flip_carry
            }
        }
        _ => horizontal.length(),
    };
    // A jump that lands with the stick let go keeps little of its speed (UYA 0x503B68).
    if matches!(hero.state, HeroState::Jump(_)) && stick.magnitude < profile.air.land_release_stick
    {
        hero.speed = hero.speed.min(profile.air.land_release_speed);
    }
    hero.stop_ticks = 0;
    hero.wall = None;
    if landing_from_long_jump {
        hero.set_state(HeroState::Skid);
    } else if landing_from_fall {
        land_from_fall(hero, stick, profile);
    } else {
        // A landing walks on at once, without the setting-off phase.
        hero.setting_off = !stick.is_active();
        hero.set_state(HeroState::Walk);
    }
}

/// A fall comes down standing and stays still a moment if it fell far; a shorter one walks
/// on with the stick well over, skids if it's moving fast, or stands (UYA's state 6,
/// `hero-controller.md`, "Landings").
fn land_from_fall(hero: &mut HeroMotion, stick: &Stick, profile: &HeroProfile) {
    let landing = &profile.fall.landing;
    if hero.fell_far {
        hero.input_lock = hero.input_lock.max(landing.lock_after(hero.ticks));
        hero.set_state(HeroState::Idle);
    } else if stick.magnitude > landing.walk_stick {
        hero.setting_off = false;
        hero.set_state(HeroState::Walk);
    } else if hero.speed > landing.skid_above {
        hero.set_state(HeroState::Skid);
    } else {
        hero.set_state(HeroState::Idle);
    }
}

fn ground_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
) {
    let walk = &profile.walk;
    let wish_direction = wish.normalize_or_zero();

    if hero.state == HeroState::Skid {
        // The skid state brakes to a stop without turning. Once the controls are back, a
        // jump press jumps and holding crouch crouches (UYA's and DL's state 3; DL doesn't
        // crouch).
        let free = hero.input_lock == 0;
        if free && hero.jump_press.within(1) {
            hero.jump_press.used = true;
            start_jump(hero, JumpKind::Standing, profile);
            air_tick(hero, input, stick, wish, profile);
            return;
        }
        if free && crouch_held(input, profile) {
            hero.set_state(HeroState::Crouch);
        } else {
            if hero.charge_skid && free {
                // After a charge the slide stops when the controls come back.
                hero.speed = 0.0;
            }
            skid_to_a_stop(hero, profile);
            return;
        }
    }

    // Standing with the controls held back (after a far fall), he stays still until they
    // come back, as UYA's idle skips its transitions during its lockout (0x1A4DC0).
    if hero.state == HeroState::Idle && hero.input_lock > 0 {
        hero.speed = approach(hero.speed, 0.0, walk.stop_brake * TICK);
        let facing = hero.facing();
        hero.set_horizontal_velocity(facing * hero.speed);
        hero.velocity.y = 0.0;
        return;
    }

    // The first-person button stands him still to look round, and the look turns him to
    // the camera once it looks where he does (UYA's state 1; `camera.md`, "The hero in
    // these modes"). Letting go goes back to idle, or to a crouch with crouch held.
    if input.held(Action::FirstPerson)
        && matches!(
            hero.state,
            HeroState::Idle | HeroState::Walk | HeroState::Look
        )
    {
        hero.set_state(HeroState::Look);
        hero.speed = approach(hero.speed, 0.0, walk.stop_brake * TICK);
        if hero.look_ready {
            hero.yaw = hero.camera_yaw;
            hero.yaw_rate = 0.0;
        }
        let facing = hero.facing();
        hero.set_horizontal_velocity(facing * hero.speed);
        hero.velocity.y = 0.0;
        return;
    }

    let jump_buffer = if hero.state == HeroState::Walk {
        walk.jump_buffer_walk
    } else {
        walk.jump_buffer_idle
    };
    if hero.state != HeroState::Crouch && hero.jump_press.within(jump_buffer) {
        // With crouch held and the stick just pushed to a side or back, a jump is a flip;
        // so is one while strafing with the stick well over to a side or back; otherwise
        // it is a jump (UYA 0x502368). DL's walk doesn't flip while strafing, but a jump
        // pressed as one lands still does, in both games (UYA 0x503B68, DL 0x6A45F0).
        let side = flip_side(hero, wish_direction);
        let window = jump_buffer + profile.flip.crouch.press_gap;
        let crouch_flip = crouch_flip_due(hero, input, side, window, profile);
        let strafe = &profile.strafe;
        let landing = hero.landed_ticks < strafe.landing_ticks
            && hero.jump_press.within(strafe.landing_window);
        hero.jump_press.used = true;
        if crouch_flip
            || (hero.strafing
                && (strafe.flips || landing)
                && stick.magnitude > strafe.flip_stick
                && side != StrafeSide::Forward)
        {
            start_flip(hero, wish_direction, stick, side, profile);
        } else {
            // Strafing never run-jumps.
            let kind = if stick.magnitude > walk.run_jump_stick && !hero.strafing {
                JumpKind::Run
            } else {
                JumpKind::Standing
            };
            start_jump(hero, kind, profile);
        }
        air_tick(hero, input, stick, wish, profile);
        return;
    }

    if crouch_held(input, profile) || hero.state == HeroState::Crouch {
        let long = &profile.long_jump;
        // A flip from a crouch doesn't wait for it to be old enough (UYA 0x502ED8).
        if hero.state == HeroState::Crouch {
            let side = flip_side(hero, wish_direction);
            if crouch_flip_due(
                hero,
                input,
                side,
                profile.flip.crouch.crouch_window,
                profile,
            ) {
                hero.jump_press.used = true;
                start_flip(hero, wish_direction, stick, side, profile);
                air_tick(hero, input, stick, wish, profile);
                return;
            }
        }
        // A jump from a crouch waits until the crouch is old enough; the press is kept.
        if hero.state == HeroState::Crouch
            && hero.ticks >= long.crouch_ticks
            && hero.jump_press.within(long.press_window)
        {
            hero.jump_press.used = true;
            let kind = if !hero.abilities.heli_pack {
                JumpKind::Standing
            } else if stick.magnitude > long.min_stick && hero.speed > long.min_speed {
                JumpKind::Long
            } else {
                JumpKind::Heli
            };
            start_jump(hero, kind, profile);
            air_tick(hero, input, stick, wish, profile);
            return;
        }
        if crouch_held(input, profile) {
            hero.set_state(HeroState::Crouch);
            hero.speed = approach(hero.speed, 0.0, walk.crouch_brake * TICK);
            // A slow turn of its own, so a stick pushed to a side stays there for a flip.
            if stick.magnitude > walk.crouch_turn_stick {
                turn(hero, yaw_of(wish_direction), walk.crouch_turn);
            }
            let facing = hero.facing();
            hero.set_horizontal_velocity(facing * hero.speed);
            return;
        }
    }

    if hero.strafing && stick.is_active() {
        hero.set_state(HeroState::Walk);
        hero.stop_ticks = 0;
        hero.skidding = false;
        hero.sliding = false;
        strafe_walk(hero, profile);
        // The walk's speeds, but along the stick (`hero-controller.md`, "Strafing").
        let target = walk.speed * stick.magnitude;
        let rate = if hero.speed < target {
            walk.accel
        } else {
            walk.brake
        };
        hero.speed = approach(hero.speed, target, rate * TICK);
        hero.set_horizontal_velocity(wish_direction * hero.speed);
        hero.velocity.y = 0.0;
        return;
    }

    if stick.is_active() {
        let target_yaw = yaw_of(wish_direction);
        let error = wrap(target_yaw - hero.yaw).abs();
        hero.stop_ticks = 0;
        if hero.state != HeroState::Walk {
            hero.setting_off = hero.speed < walk.setting_off_below;
            hero.set_state(HeroState::Walk);
        }
        if hero.setting_off && hero.speed >= walk.setting_off_below {
            hero.setting_off = false;
        }

        // When the walk skids, and when it moves the way the stick points rather than the
        // way he faces (`hero-controller.md`, "Ground movement").
        let full = stick.magnitude >= 1.0;
        if hero.speed < walk.slide_below {
            hero.sliding = true;
            if full && error > walk.skid_slow_degrees.to_radians() {
                hero.skidding = true;
            }
        }
        if full
            && hero.stick_swing(walk.skid_swing_ticks) > walk.skid_swing_degrees.to_radians()
            && error > walk.skid_swing_error_degrees.to_radians()
        {
            hero.sliding = true;
            hero.skidding = true;
        }
        if error < walk.slide_until_degrees.to_radians() {
            hero.sliding = false;
        }
        if hero.skidding && error < walk.skid_until_degrees.to_radians() {
            hero.skidding = false;
            hero.setting_off = hero.speed < walk.setting_off_below;
        }

        // A held gun firing keeps the setting-off turn, and a steady gun firing makes it
        // its own (`weapons.md`, "The hero's side of firing").
        let gun = &profile.gun;
        let spring = if hero.skidding {
            walk.turn_skidding
        } else if hero.setting_off || hero.gun.firing_held {
            if hero.gun.firing && hero.gun.steady {
                if stick.magnitude <= gun.steady_part_stick {
                    gun.steady_part.at(stick.magnitude)
                } else {
                    gun.steady_full
                }
            } else {
                walk.turn_setting_off.at(stick.magnitude)
            }
        } else {
            walk.turn_moving
        };
        turn(hero, target_yaw, spring);

        let mut target = walk.speed * stick.magnitude;
        let mut brake = walk.brake;
        if hero.skidding {
            let most = (walk.speed - error * walk.skid_speed_loss / PI).max(0.0);
            let scale =
                (walk.skid_scale_degrees.to_radians() - error).clamp(walk.skid_scale_min, 1.0);
            target = target.min(most) * scale;
            brake = walk.skid_brake;
        }
        if stick.magnitude < walk.turn_slow_stick && error > walk.turn_slow_degrees.to_radians() {
            target *= (1.0 - error * walk.turn_slow_per_radian).max(0.0);
        }
        // A part-pushed stick far round from the facing turns him without speeding up, and
        // so does any stick while a held gun fires.
        let accel = if (stick.magnitude < walk.turn_stall_stick || hero.gun.firing_held)
            && error > walk.turn_stall_degrees.to_radians()
        {
            0.0
        } else {
            walk.accel
        };
        let rate = if hero.speed < target { accel } else { brake };
        hero.speed = approach(hero.speed, target, rate * TICK);

        let direction = if hero.sliding {
            wish_direction
        } else {
            hero.facing()
        };
        hero.set_horizontal_velocity(direction * hero.speed);
        hero.velocity.y = 0.0;
        return;
    } else {
        hero.skidding = false;
        hero.sliding = false;
        let off_camera = wrap(hero.camera_yaw - hero.yaw).abs();
        if hero.state == HeroState::Idle
            && hero.strafing
            && off_camera > profile.strafe.turn_after_degrees.to_radians()
        {
            // Standing, he steps round to the camera once it is well off his facing.
            hero.set_state(HeroState::Walk);
        }
        match hero.state {
            HeroState::Walk if hero.strafing => {
                // A strafe brakes at the skid rate, turning to the camera, and only idles
                // once he faces within a couple of degrees of it.
                strafe_walk(hero, profile);
                hero.speed = approach(hero.speed, 0.0, walk.skid_brake * TICK);
                let off = wrap(hero.camera_yaw - hero.yaw).abs();
                if off < profile.strafe.idle_within_degrees.to_radians() {
                    hero.speed = hero.speed.min(walk.stop_residual);
                    hero.set_state(HeroState::Idle);
                }
            }
            HeroState::Walk if !hero.gun.out && hero.stop_ticks < walk.stop_skid_ticks => {
                // Letting go with the wrench in hand: a couple of ticks of skid braking...
                hero.speed = approach(hero.speed, 0.0, walk.skid_brake * TICK);
                hero.stop_ticks += 1;
            }
            HeroState::Walk if hero.gun.out && hero.stop_ticks < profile.gun.stop_ticks => {
                // ...or with a gun, the walk's own braking, for a tick...
                hero.speed = approach(hero.speed, 0.0, walk.brake * TICK);
                hero.stop_ticks += 1;
            }
            HeroState::Walk => {
                // ...then idle, with what little speed is left.
                hero.speed = hero.speed.min(walk.stop_residual);
                hero.set_state(HeroState::Idle);
            }
            _ => {
                hero.set_state(HeroState::Idle);
                hero.speed = approach(hero.speed, 0.0, walk.stop_brake * TICK);
            }
        }
    }
    // A strafe slides to a stop the way it went.
    let direction = if hero.strafing {
        hero.strafe_heading
    } else {
        hero.facing()
    };
    hero.set_horizontal_velocity(direction * hero.speed);
    hero.velocity.y = 0.0;
}

/// What the walk does each tick while strafing, before the speed (UYA 0x4EA628): changing
/// sides damps the speed, and he turns to the camera.
fn strafe_walk(hero: &mut HeroMotion, profile: &HeroProfile) {
    let strafe = &profile.strafe;
    hero.after_right = hero.after_right.saturating_sub(1);
    hero.after_left = hero.after_left.saturating_sub(1);
    // The game's two timers aren't symmetric: right to left stops him for a couple of
    // ticks, left to right eases him in.
    match hero.strafe_side {
        StrafeSide::Left => {
            hero.after_left = strafe.reverse_ticks;
            if hero.after_right > 0 {
                hero.speed = 0.0;
            }
        }
        StrafeSide::Right => {
            hero.after_right = strafe.reverse_ticks;
            if hero.after_left > 0 {
                let ticks = strafe.reverse_ticks as f32;
                hero.speed *= (ticks - hero.after_left as f32) / ticks;
            }
        }
        StrafeSide::Forward | StrafeSide::Back => {}
    }
    turn_to_camera(hero, profile);
    // The walk's own turning starts afresh when the strafe ends.
    hero.yaw_rate = 0.0;
}

/// Turns him to the camera on the strafe's spring and its own rate (UYA 0x4EA5B0). Firing
/// in the lock-strafe camera turns him faster.
fn turn_to_camera(hero: &mut HeroMotion, profile: &HeroProfile) {
    let gun = &profile.gun;
    let mut spring = profile.strafe.turn;
    if hero.camera_setting == CameraSetting::LockStrafe
        && (hero.gun.firing || hero.fire_press.ticks < gun.fire_turn_ticks)
    {
        spring.gain *= gun.fire_turn_scale;
        spring.cap *= gun.fire_turn_scale;
    }
    hero.yaw_rate = hero.strafe_yaw_rate;
    turn(hero, hero.camera_yaw, spring);
    hero.strafe_yaw_rate = hero.yaw_rate;
}

fn start_jump(hero: &mut HeroMotion, kind: JumpKind, profile: &HeroProfile) {
    let params = jump_params(profile, kind);
    let from_wall_jump = hero.state == HeroState::Jump(JumpKind::Wall);
    hero.set_state(HeroState::Jump(kind));
    hero.ticks = 0;
    let hold_ticks = params.hold_ticks.max(1);
    hero.jump = JumpBlock {
        height: params.height_min,
        step: (params.height_max - params.height_min) / hold_ticks as f32,
        holds_left: hold_ticks,
        holding: true,
        lift: 0.0,
        gravity: params.gravity,
        past_apex: false,
        blocked: false,
    };
    // A water jump keeps the velocity it swam with.
    if !matches!(
        kind,
        JumpKind::Flip
            | JumpKind::Wall
            | JumpKind::Ledge
            | JumpKind::Grind
            | JumpKind::GrindSwitch
            | JumpKind::Water
    ) {
        let direction = if hero.strafing {
            hero.strafe_heading
        } else {
            hero.facing()
        };
        hero.set_horizontal_velocity(direction * hero.speed);
    }
    hero.velocity.y = 0.0;
    hero.long_jump_base = hero.speed;
    hero.take_off_height = hero.position.y;
    hero.double_jumped = false;
    hero.wall = None;
    hero.wall_ticks = 0;
    if kind != JumpKind::Wall {
        hero.last_wall = None;
    }
    if !from_wall_jump {
        hero.ordinary_wall_jumps = 0;
    }
}

/// Which way a flip from here would go (UYA 0x4FB758): where the stick points while
/// strafing, otherwise the stick against his facing, in the same 90° sectors. A centred
/// stick is forward, which doesn't flip.
fn flip_side(hero: &HeroMotion, wish_direction: Vec3) -> StrafeSide {
    if hero.strafing {
        return hero.strafe_side;
    }
    let facing = hero.facing();
    let right = facing.cross(Vec3::Y);
    StrafeSide::of(Vec2::new(
        wish_direction.dot(right),
        wish_direction.dot(facing),
    ))
}

/// Crouch held, in a game whose hero crouches. DL's doesn't: its code reads the button
/// where UYA's crouches, then ignores it (`walk.crouches`).
fn crouch_held(input: &PlayerInput, profile: &HeroProfile) -> bool {
    profile.walk.crouches && input.held(Action::Crouch)
}

/// UYA's flip from a crouch (0x502ED8, 0x502368): crouch held, the stick to a side or
/// back, and a jump press and a stick press both within `window` ticks, fewer than
/// `press_gap` ticks apart. So the stick has to be pushed at about the time of the jump;
/// held there, it only turns him.
fn crouch_flip_due(
    hero: &HeroMotion,
    input: &PlayerInput,
    side: StrafeSide,
    window: u32,
    profile: &HeroProfile,
) -> bool {
    crouch_held(input, profile)
        && side != StrafeSide::Forward
        && hero.jump_press.within(window)
        && hero.stick_press.within(window)
        && hero.jump_press.ticks.abs_diff(hero.stick_press.ticks) < profile.flip.crouch.press_gap
}

fn start_flip(
    hero: &mut HeroMotion,
    direction: Vec3,
    stick: &Stick,
    side: StrafeSide,
    profile: &HeroProfile,
) {
    // The game measures the speed before the jump changes anything (UYA 0x4FC518).
    let ground_speed = flat_length(hero.velocity);
    start_jump(hero, JumpKind::Flip, profile);
    let flip = &profile.flip;
    let crouch = &flip.crouch;
    hero.flip_side = side;
    hero.flip_direction = direction;
    hero.flip_facing = hero.yaw_back(crouch.facing_ticks_back);
    hero.flip_strafing = hero.strafing && stick.magnitude > flip.strafe_stick;
    if hero.flip_strafing {
        hero.flip_speed = flip.start_speed;
        hero.flip_carry = 0.0;
    } else {
        hero.flip_speed = 0.0;
        hero.flip_carry = if ground_speed > crouch.carry_above {
            ground_speed
        } else {
            0.0
        };
    }
}

/// A flip's own motion each tick (UYA 0x4EDE80, state 11). `falling` and `to_land` are
/// what the game worked out at the end of the last tick.
fn flip_tick(
    hero: &mut HeroMotion,
    stick: &Stick,
    falling: bool,
    to_land: f32,
    profile: &HeroProfile,
) {
    let flip = &profile.flip;
    let crouch = &flip.crouch;
    // While strafing he turns to the camera, otherwise back to the way he faced before.
    if hero.strafing {
        turn_to_camera(hero, profile);
    } else {
        turn(hero, hero.flip_facing, crouch.turn);
    }
    if hero.flip_over_drop {
        hero.flip_speed = 0.0;
    } else if hero.flip_strafing {
        // The stick sets the speed as it goes, not only at the start.
        let target = (flip.speed * stick.magnitude).max(flip.min_speed);
        hero.flip_speed = approach(hero.flip_speed, target, flip.accel * TICK);
    } else {
        // The game counts the ticks to the landing in whole ticks.
        let landing = falling && to_land.floor() <= crouch.slow_within_ticks as f32;
        let target = if landing { 0.0 } else { crouch.speed };
        hero.flip_speed = approach(hero.flip_speed, target, crouch.accel * TICK);
    }
    let mut velocity = hero.flip_direction * hero.flip_speed;
    if !hero.flip_strafing {
        // His run carries on the way he faced, as far as the flip goes across it.
        let across = ops::sin(wrap(yaw_of(hero.flip_direction) - hero.flip_facing).abs());
        velocity += facing(hero.flip_facing) * hero.flip_carry * across;
    }
    hero.set_horizontal_velocity(velocity);
}

/// Ticks until he comes down to the ground below, as UYA works it out for the flip
/// (0x50C4A0): the t at which h + v·t − g·t(t + 1)/2 reaches 0, in per-tick units.
fn ticks_to_land(hero: &HeroMotion) -> f32 {
    let g = hero.jump.gravity * TICK * TICK;
    let b = hero.velocity.y * TICK + g / 2.0;
    (b + ops::sqrt(b * b + 2.0 * g * hero.height_above_ground)) / g
}

/// Whether a flip is carrying him off a drop (UYA 0x4EDE80): low over the ground late in
/// the flip, with no ground near his take-off height either just ahead along his motion
/// or a good way further.
fn flip_over_drop(hero: &HeroMotion, profile: &HeroProfile, world: &dyn CollisionWorld) -> bool {
    let crouch = &profile.flip.crouch;
    if hero.state != HeroState::Jump(JumpKind::Flip)
        || hero.ticks < crouch.edge_from_tick
        || hero.height_above_ground >= crouch.edge_height
    {
        return false;
    }
    let ahead = hero.velocity.normalize_or_zero();
    let ground_at = |distance: f32| {
        let point = hero.position + ahead * distance;
        let top = Vec3::new(point.x, hero.take_off_height + crouch.edge_rise, point.z);
        let down = Vec3::NEG_Y * (crouch.edge_rise + crouch.edge_drop);
        world.cast_ray(top, down).is_some()
    };
    !ground_at(crouch.edge_near) && !ground_at(crouch.edge_far)
}

fn start_wall_jump(hero: &mut HeroMotion, wall: Wall, profile: &HeroProfile) {
    let wall_jump = &profile.wall_jump;
    let surface = wall.material == Material::WallJump;
    if hero.state == HeroState::Jump(JumpKind::Wall) && !surface {
        hero.ordinary_wall_jumps += 1;
    } else if surface {
        hero.ordinary_wall_jumps = 0;
    }
    start_jump(hero, JumpKind::Wall, profile);
    if surface {
        let hold_ticks = wall_jump.params.hold_ticks.max(1) as f32;
        hero.jump.height = wall_jump.surface_height_min;
        hero.jump.step = (wall_jump.surface_height_max - wall_jump.surface_height_min) / hold_ticks;
    }
    // The game keeps the old velocity during the cling, and the wall blocks the part going
    // into it.
    let horizontal = hero.horizontal_velocity();
    hero.set_horizontal_velocity(horizontal - wall.normal * horizontal.dot(wall.normal).min(0.0));
    hero.last_wall = Some(wall.normal);
}

/// Looks for a wall to jump off (`hero-controller.md`, "Wall jump").
fn detect_wall(
    hero: &HeroMotion,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) -> Option<Wall> {
    let wall_jump = &profile.wall_jump;
    if hero.height_above_ground < wall_jump.min_height
        || hero.velocity.y < -wall_jump.max_fall_speed
    {
        return None;
    }
    let chained = hero.state == HeroState::Jump(JumpKind::Wall);
    let forward = hero.facing();
    let reach = forward * (profile.body.radius + wall_jump.probe_reach);

    // A tall wall, steep and facing him.
    let probe_height = if chained {
        wall_jump.probe_height_chained
    } else {
        wall_jump.probe_height
    };
    let tall = world.cast_ray(hero.position + Vec3::Y * probe_height, reach)?;
    let material = world.material(tall.body);
    let steepest_normal_y = ops::cos(wall_jump.min_steepness_degrees.to_radians());
    let normal = flat(tall.normal);
    if material == Material::NoGrab
        || tall.normal.y > steepest_normal_y
        || normal.dot(-forward) < ops::cos(wall_jump.max_facing_degrees.to_radians())
    {
        return None;
    }

    // Close to his feet.
    let foot = world.cast_ray(hero.position + Vec3::Y * wall_jump.foot_probe_height, reach)?;
    let max_distance = if chained {
        wall_jump.max_distance_chained
    } else {
        wall_jump.max_distance
    };
    if foot.fraction * reach.length() > max_distance {
        return None;
    }
    let normal = flat(foot.normal);

    // Chains alternate between facing walls, and ordinary walls allow only a few in a row.
    if chained
        && let Some(last) = hero.last_wall
        && last.dot(normal) > ops::cos(wall_jump.min_turn_degrees.to_radians())
    {
        return None;
    }
    if material != Material::WallJump && hero.ordinary_wall_jumps + 1 >= wall_jump.ordinary_chain {
        return None;
    }

    // A ledge his flight reaches soon wins over the wall.
    let ahead = wall_jump.ledge_lookahead_ticks as f32 * TICK;
    let predicted = hero.position
        + hero.horizontal_velocity() * ahead
        + Vec3::Y * (hero.velocity.y * ahead - 0.5 * hero.jump.gravity * ahead * ahead);
    if detect_ledge(
        world,
        profile,
        predicted,
        hero.yaw,
        hero.height_above_ground,
    )
    .is_some()
    {
        return None;
    }

    Some(Wall { normal, material })
}

/// Grabs a ledge in front of an airborne hero, if his move allows it.
fn try_grab(hero: &mut HeroMotion, profile: &HeroProfile, world: &dyn CollisionWorld) {
    let ledge = &profile.ledge;
    if hero.no_grab_ticks > 0 {
        return;
    }
    let past_apex_or_slow = hero.jump.past_apex || hero.velocity.y < ledge.grab_rising_below;
    let allowed = match hero.state {
        HeroState::Fall | HeroState::Glide => true,
        HeroState::Jump(_) => past_apex_or_slow && past_pushoff_gate(hero, profile),
        HeroState::DoubleJump => hero.ticks > ledge.double_jump_after_tick && past_apex_or_slow,
        _ => false,
    };
    if !allowed {
        return;
    }
    if let Some(found) = detect_ledge(
        world,
        profile,
        hero.position,
        hero.yaw,
        hero.height_above_ground,
    ) {
        hero.ledge = Some(found);
        hero.velocity = Vec3::ZERO;
        hero.speed = 0.0;
        hero.wall = None;
        hero.set_state(HeroState::LedgeGrab);
    }
}

fn ledge_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let ledge = &profile.ledge;
    let Some(held) = hero.ledge else {
        hero.let_go(0);
        return;
    };
    let (yaw, position, height) = (hero.yaw, hero.position, hero.height_above_ground);
    // The stick's angle from his facing, and from either side.
    let wish_yaw = yaw_of(wish);
    let pushing = stick.magnitude > ledge.shimmy_stick;
    let pulling_away = pushing && wrap(wish_yaw - yaw).abs() > ledge.let_go_degrees.to_radians();
    let side_yaw = |side: Side| match side {
        Side::Left => yaw + FRAC_PI_2,
        Side::Right => yaw - FRAC_PI_2,
    };
    let towards = |side: Side| {
        pushing && wrap(wish_yaw - side_yaw(side)).abs() < ledge.shimmy_degrees.to_radians()
    };
    let ledge_to = |side: Side, distance: f32| {
        detect_ledge(
            world,
            profile,
            position + facing(side_yaw(side)) * distance,
            yaw,
            height,
        )
    };
    let wall_yaw = yaw_of(-held.normal);

    match hero.state {
        HeroState::LedgeGrab | HeroState::LedgeHang => {
            if hero.state == HeroState::LedgeGrab {
                if hero.ticks >= ledge.grab_ticks {
                    hero.set_state(HeroState::LedgeHang);
                }
            } else {
                if detect_ledge(
                    world,
                    profile,
                    hero.position,
                    hero.yaw,
                    hero.height_above_ground,
                )
                .is_none()
                {
                    hero.let_go(ledge.no_grab_ticks_lost);
                    return;
                }
                let let_go_press = hero.jump_press.within(ledge.shimmy_climb_press_window);
                if hero.crouch_press.within(1) || (pulling_away && let_go_press) {
                    hero.crouch_press.used = true;
                    hero.jump_press.used = true;
                    hero.let_go(ledge.no_grab_ticks);
                    return;
                }
                if hero.jump_press.within(ledge.climb_press_window) {
                    climb(hero, profile);
                    air_tick(hero, input, stick, wish, profile);
                    return;
                }
                for side in [Side::Left, Side::Right] {
                    if towards(side) && ledge_to(side, ledge.shimmy_probe).is_some() {
                        hero.set_state(HeroState::LedgeShimmy(side));
                        break;
                    }
                }
            }
            turn(hero, wall_yaw, ledge.grab_turn);
            hero.velocity = ((held.hang - hero.position) / TICK).clamp_length_max(ledge.grab_speed);
        }
        HeroState::LedgeShimmy(side) => {
            if hero.jump_press.within(ledge.shimmy_climb_press_window) {
                if pulling_away {
                    hero.jump_press.used = true;
                    hero.let_go(ledge.no_grab_ticks_shimmy);
                } else {
                    climb(hero, profile);
                    air_tick(hero, input, stick, wish, profile);
                }
                return;
            }
            turn(hero, wall_yaw, ledge.shimmy_turn);
            // The steps follow the shimmy's animation, and a cycle once begun is finished
            // even if the stick lets go. At each cycle's end the stick and the ledge ahead
            // decide whether another follows.
            let frame = hero.ticks.saturating_sub(ledge.shimmy_blend_ticks) as f32
                * ledge.shimmy_frames_per_tick;
            let cycle_frame = frame % ledge.shimmy_cycle_frames;
            hero.velocity = Vec3::ZERO;
            if frame > 0.0
                && cycle_frame == 0.0
                && !(towards(side) && ledge_to(side, ledge.shimmy_probe).is_some())
            {
                // Hang where the shimmy ended.
                if let Some(here) = detect_ledge(world, profile, position, yaw, height) {
                    hero.ledge = Some(here);
                }
                hero.set_state(HeroState::LedgeHang);
                return;
            }
            let entry = ledge.shimmy_steps.get(cycle_frame as usize).copied();
            let step = entry.unwrap_or(0.0) * ledge.shimmy_step_scale;
            if step != 0.0
                && let Some(next) = ledge_to(side, ledge.shimmy_look)
            {
                hero.velocity = (next.hang - position).normalize_or_zero() * step / TICK;
                hero.ledge = Some(next);
            }
        }
        _ => {}
    }
}

/// Up from a hang: the ledge jump.
fn climb(hero: &mut HeroMotion, profile: &HeroProfile) {
    hero.jump_press.used = true;
    hero.ledge = None;
    hero.velocity = Vec3::ZERO;
    hero.speed = 0.0;
    hero.climb_speed = 0.0;
    start_jump(hero, JumpKind::Ledge, profile);
}

fn hit_tick(hero: &mut HeroMotion, profile: &HeroProfile) {
    let hit = &profile.hit;
    let on_ground = hero.ground.is_some();
    if !on_ground && hero.ticks > hit.fall_after_tick && hero.height_above_ground > hit.fall_above {
        // Thrown off something high: an ordinary fall from here.
        hero.take_off_height = hero.position.y;
        hero.double_jumped = true;
        hero.set_state(HeroState::Fall);
        return;
    }
    if hero.ticks >= hit.ticks && (on_ground || hero.height_above_ground <= hit.fall_above) {
        if hero.hit_fatal {
            hero.set_state(HeroState::Dead);
        } else {
            hero.speed = 0.0;
            hero.set_state(HeroState::Idle);
        }
        hero.set_horizontal_velocity(Vec3::ZERO);
        return;
    }
    let rate = if on_ground && hero.ticks >= hit.brake_later_from_tick {
        hit.brake_later
    } else {
        hit.brake
    };
    brake_and_fall(hero, rate, profile);
}

fn dead_tick(hero: &mut HeroMotion, profile: &HeroProfile) {
    brake_and_fall(hero, profile.hit.brake_later, profile);
}

/// Slows the horizontal motion at `rate`, and falls when off the ground.
fn brake_and_fall(hero: &mut HeroMotion, rate: f32, profile: &HeroProfile) {
    let horizontal = hero.horizontal_velocity();
    let speed = approach(horizontal.length(), 0.0, rate * TICK);
    hero.set_horizontal_velocity(horizontal.normalize_or_zero() * speed);
    hero.speed = speed;
    if hero.ground.is_none() || hero.velocity.y > 0.0 {
        hero.velocity.y =
            (hero.velocity.y - profile.fall.gravity * TICK).max(-profile.air.fall_speed_cap);
    } else {
        hero.velocity.y = 0.0;
    }
}

/// Brakes the skid state towards a stop, without turning, and idles once stopped.
fn skid_to_a_stop(hero: &mut HeroMotion, profile: &HeroProfile) {
    hero.speed = approach(hero.speed, 0.0, profile.walk.skid_state_brake * TICK);
    if hero.speed <= 0.0 {
        hero.set_state(HeroState::Idle);
    }
    let facing = hero.facing();
    hero.set_horizontal_velocity(facing * hero.speed);
    hero.velocity.y = 0.0;
}

/// Whether a charge can start now: with the boots, the controls free, the last charge
/// cooled down, and on the ground (`hero-controller.md`, "Charge boots").
fn can_charge(hero: &HeroMotion, profile: &HeroProfile) -> bool {
    let on_ground = match hero.state {
        HeroState::Idle | HeroState::Walk | HeroState::Skid | HeroState::Crouch => true,
        // A jump still pushing off counts as on the ground.
        HeroState::Jump(_) => hero.pushing_off_ground(profile),
        _ => false,
    };
    on_ground && hero.abilities.charge_boots && hero.input_lock == 0 && hero.charge_cooldown == 0
}

fn start_charge(hero: &mut HeroMotion) {
    // The speed carries over. So does the eased steering, which DL never resets.
    hero.speed = hero.horizontal_velocity().length();
    hero.velocity.y = 0.0;
    hero.charge_sink = 0.0;
    hero.set_state(HeroState::Charge);
}

/// One tick of the charge boots' dash (state 125).
fn charge_tick(hero: &mut HeroMotion, stick: &Stick, profile: &HeroProfile) {
    let charge = &profile.charge;
    let t = hero.ticks;
    if t > charge.last_tick {
        // Over: a skid, with the controls ignored for a moment. It slides on the motion it
        // had, which the step limit kept under the charge's speed.
        hero.input_lock = charge.end_lock_ticks;
        hero.speed = hero.horizontal_velocity().length();
        hero.set_state(HeroState::Skid);
        hero.charge_skid = true;
        if hero.ground.is_some() {
            skid_to_a_stop(hero, profile);
        }
        return;
    }
    hero.charge_cooldown = charge.cooldown_ticks;

    let target = if t < charge.fast_ticks {
        charge.speed
    } else {
        charge.late_speed
    };
    let rate = if hero.speed < target {
        charge.accel
    } else {
        charge.brake
    };
    hero.speed = approach(hero.speed, target, rate * TICK);

    // The stick steers like a car: its sideways push, eased, turns the hero, gently at
    // first.
    hero.charge_steer =
        approach(hero.charge_steer, -stick.vector.x, charge.steer_step).clamp(-1.0, 1.0);
    if stick.magnitude > charge.turn_min_stick {
        let ramp = (t as f32 / charge.turn_ramp_ticks as f32).min(1.0);
        let turn = charge.turn_rate * TICK * stick.magnitude * hero.charge_steer * ramp;
        hero.yaw = wrap(hero.yaw + turn);
    }
    hero.yaw_rate = 0.0;
    let facing = hero.facing();

    if let Some(ground) = hero.ground {
        // Along the slope, so a fast charge keeps to the ground going downhill.
        let along = (facing - ground.normal * facing.dot(ground.normal)).normalize_or_zero();
        hero.velocity = along * hero.speed;
        hero.charge_sink = 0.0;
    } else {
        hero.set_horizontal_velocity(facing * hero.speed);
        // Off an edge the charge goes on, sinking towards the ground below.
        let error = -hero.height_above_ground;
        let spring = charge.descent_spring;
        let cap = charge.descent_cap * TICK;
        hero.charge_sink = hero.charge_sink.clamp(-error.abs(), error.abs());
        hero.charge_sink += spring.gain * error - spring.damping * hero.charge_sink;
        hero.charge_sink = hero
            .charge_sink
            .clamp(-cap, cap)
            .clamp(-error.abs(), error.abs());
        hero.velocity.y = hero.charge_sink / TICK;
    }
}

/// Something ahead at chest height, close enough to bounce off (DL's check before a
/// charge's bounce).
fn wall_ahead(hero: &HeroMotion, profile: &HeroProfile, world: &dyn CollisionWorld) -> bool {
    let charge = &profile.charge;
    let facing = hero.facing();
    let origin =
        hero.position + Vec3::Y * charge.wall_probe_height + facing * (profile.body.radius - 0.02);
    world
        .cast_ray(origin, facing * charge.wall_probe_reach)
        .is_some()
}

fn start_bounce(hero: &mut HeroMotion, profile: &HeroProfile) {
    hero.speed = profile.charge.bounce_speed;
    let back = -hero.facing() * hero.speed;
    hero.set_horizontal_velocity(back);
    hero.set_state(HeroState::Bounce);
}

/// Back off the wall a charge ran into, still facing it (state 121), for as long as the
/// game's animation.
fn bounce_tick(hero: &mut HeroMotion, profile: &HeroProfile) {
    let charge = &profile.charge;
    if hero.ticks >= charge.bounce_ticks {
        hero.set_horizontal_velocity(Vec3::ZERO);
        hero.set_state(HeroState::Idle);
        return;
    }
    hero.speed = approach(hero.speed, 0.0, charge.bounce_brake * TICK);
    let back = -hero.facing() * hero.speed;
    hero.set_horizontal_velocity(back);
    if hero.ground.is_none() {
        hero.velocity.y =
            (hero.velocity.y - charge.bounce_gravity * TICK).max(-profile.air.fall_speed_cap);
    } else {
        hero.velocity.y = 0.0;
    }
}

/// Gets on a rail in reach, if the hero may (`hero-controller.md`, "Grinding"): with the
/// grind boots, standing, walking, falling or past a jump's apex.
fn try_get_on_rail(hero: &mut HeroMotion, profile: &HeroProfile, world: &dyn CollisionWorld) {
    if !hero.abilities.grind_boots || hero.no_rail_ticks > 0 || world.rails().is_empty() {
        return;
    }
    let on_ground = matches!(
        hero.state,
        HeroState::Idle | HeroState::Walk | HeroState::Skid | HeroState::Crouch
    );
    let may = on_ground
        || match hero.state {
            HeroState::Fall => true,
            HeroState::Jump(_) => hero.jump.past_apex,
            HeroState::DoubleJump | HeroState::Glide => hero.velocity.y <= 0.0,
            _ => false,
        };
    if !may {
        return;
    }
    let grind = &profile.grind;
    let reach = if on_ground {
        grind.reach_on_ground
    } else {
        grind.reach
    };
    let feet = hero.position;
    for (index, rail) in world.rails().iter().enumerate() {
        if rail.kind != RailKind::Grind {
            continue;
        }
        let at = rail.nearest(index, feet);
        let point = rail.position(at);
        let across = Vec2::new(point.x - feet.x, point.z - feet.z).length();
        let rise = feet.y - point.y;
        if across < reach
            && rise.abs() < grind.reach_vertical
            && rise > -grind.below
            && rise < grind.above
        {
            start_grind(hero, rail, at);
            return;
        }
    }
}

fn start_grind(hero: &mut HeroMotion, rail: &GrindRail, at: RailPoint) {
    // From a grind jump the speed carries on; otherwise it starts as the hero's velocity
    // along the rail, which can be backwards.
    if !matches!(
        hero.state,
        HeroState::Jump(JumpKind::Grind | JumpKind::GrindSwitch)
    ) {
        hero.grind_base = hero.velocity.dot(rail.direction(at));
        hero.grind_slope = 0.0;
    }
    hero.rail_offset = hero.position - rail.position(at);
    hero.rail_pull = 0.0;
    hero.rail = Some(at);
    hero.switch_from = None;
    hero.ledge = None;
    hero.wall = None;
    hero.ground = None;
    hero.velocity = Vec3::ZERO;
    hero.set_state(HeroState::Grind);
}

/// One tick on a rail (states 39 and 42): the grind, or the grind attack riding it.
fn grind_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let grind = &profile.grind;
    let Some((rail, mut at)) = hero
        .rail
        .and_then(|at| world.rails().get(at.rail).map(|rail| (rail, at)))
    else {
        leave_rail(hero, Vec3::ZERO);
        return;
    };
    let along = rail.direction(at);
    hero.rail_yaw = yaw_of(flat(along));

    let jump_window = match hero.state {
        HeroState::GrindAttack(_) if !melee::grind_attack_may_jump(hero, profile) => 0,
        HeroState::GrindAttack(_) => profile.melee.grind.jump_press_window,
        _ => grind.jump_press_window,
    };
    if hero.jump_press.within(jump_window) {
        hero.jump_press.used = true;
        hero.set_horizontal_velocity(flat(along) * hero.grind_speed);
        start_jump(hero, JumpKind::Grind, profile);
        hero.rail = Some(at);
        air_tick(hero, input, stick, wish, profile);
        return;
    }
    melee::grind_attack_transitions(hero, profile);

    // The speed: a base towards 12 u/s, plus a term for the rail's slope.
    hero.grind_base = approach(hero.grind_base, grind.speed, grind.accel * TICK);
    let pitch = rail.pitch_ahead(at, grind.pitch_probe);
    let target = if pitch > 0.0 {
        -pitch * grind.uphill_per_radian
    } else {
        -pitch * grind.downhill_per_radian
    };
    let rate = if hero.grind_slope < target {
        grind.slope_rise
    } else {
        grind.slope_fall
    };
    hero.grind_slope =
        approach(hero.grind_slope, target, rate * TICK).clamp(grind.slope_min, grind.slope_max);
    hero.grind_speed = (hero.grind_base + hero.grind_slope).max(0.0);
    hero.speed = hero.grind_speed;
    // Side on to the rail.
    turn(
        hero,
        hero.rail_yaw + grind.stance_degrees.to_radians(),
        grind.turn,
    );

    if !rail.advance(&mut at, hero.grind_speed * TICK) {
        // Off the end, flying on the way the rail went.
        hero.no_rail_ticks = grind.no_rail_ticks;
        leave_rail(hero, along * hero.grind_speed);
        return;
    }
    hero.rail = Some(at);
    // Pulled onto the rail on the games' spring, at most 7 u/s.
    let gap = hero.rail_offset.length();
    hero.rail_pull = (hero.rail_pull + 0.03 * gap - 0.3 * hero.rail_pull).clamp(0.0, 7.0 * TICK);
    hero.rail_offset = hero.rail_offset.normalize_or_zero() * (gap - hero.rail_pull).max(0.0);
    let target = rail.position(at) + hero.rail_offset;
    hero.velocity = (target - hero.position) / TICK;
}

fn leave_rail(hero: &mut HeroMotion, velocity: Vec3) {
    hero.rail = None;
    hero.switch_from = None;
    hero.velocity = velocity;
    hero.speed = hero.horizontal_velocity().length();
    hero.take_off_height = hero.position.y;
    hero.double_jumped = true;
    hero.set_state(HeroState::Fall);
}

/// A jump off a rail keeps tracking it underneath: the rail sets where the hero is across
/// the ground, and the jump only his height. Early in a grind jump the stick can switch
/// to another rail to the side.
fn follow_rail_in_air(
    hero: &mut HeroMotion,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let HeroState::Jump(kind @ (JumpKind::Grind | JumpKind::GrindSwitch)) = hero.state else {
        return;
    };
    let grind = &profile.grind;
    let rails = world.rails();
    let Some(mut at) = hero.rail.filter(|at| at.rail < rails.len()) else {
        // Past the rail's end the jump flies on.
        return;
    };
    let along = rails[at.rail].direction(at);

    if kind == JumpKind::Grind
        && hero.ticks < grind.switch_window_ticks
        && hero.switch_from.is_none()
        && stick.magnitude > grind.switch_stick
        && let Some(next) = find_switch_rail(hero, wish, along, at.rail, profile, rails)
    {
        hero.switch_from = Some(at);
        hero.switch_ticks = 0;
        start_jump(hero, JumpKind::GrindSwitch, profile);
        at = next;
    }

    let rail = &rails[at.rail];
    let height_before = rail.position(at).y;
    if !rail.advance(&mut at, hero.grind_speed * TICK) {
        hero.rail = None;
        hero.switch_from = None;
        hero.set_horizontal_velocity(flat(along) * hero.grind_speed);
        return;
    }
    hero.rail = Some(at);
    let mut target = rail.position(at);
    hero.position.y += target.y - height_before;
    if let Some(mut from) = hero.switch_from {
        let old = &rails[from.rail];
        old.advance(&mut from, hero.grind_speed * TICK);
        hero.switch_ticks += 1;
        let across = (hero.switch_ticks as f32 / grind.switch_ticks as f32).min(1.0);
        target = old.position(from).lerp(target, across);
        hero.switch_from = (across < 1.0).then_some(from);
    }
    hero.velocity.x = (target.x - hero.position.x) / TICK;
    hero.velocity.z = (target.z - hero.position.z) / TICK;
    hero.rail_yaw = yaw_of(flat(rail.direction(at)));
    turn(
        hero,
        hero.rail_yaw + grind.stance_degrees.to_radians(),
        grind.turn,
    );
}

/// Another rail to the side the stick points, for a switch jump: looked for that far
/// ahead of the hero's motion, then a little further out each try.
fn find_switch_rail(
    hero: &HeroMotion,
    wish: Vec3,
    along: Vec3,
    current: usize,
    profile: &HeroProfile,
    rails: &[GrindRail],
) -> Option<RailPoint> {
    let grind = &profile.grind;
    let rail_yaw = yaw_of(flat(along));
    let stick_yaw = yaw_of(flat(wish));
    let off = wrap(stick_yaw - rail_yaw).abs();
    if (off - FRAC_PI_2).abs() >= grind.switch_degrees.to_radians() {
        return None;
    }
    let side_yaw = if wrap(stick_yaw - (rail_yaw + FRAC_PI_2)).abs() <= FRAC_PI_2 {
        rail_yaw + FRAC_PI_2
    } else {
        rail_yaw - FRAC_PI_2
    };
    let side = facing(side_yaw);
    let ahead =
        hero.position + hero.horizontal_velocity() * (grind.switch_lookahead_ticks as f32 * TICK);
    let mut distance = grind.switch_near;
    while distance <= grind.switch_far + 1e-4 {
        let probe = ahead + side * distance;
        for (index, rail) in rails.iter().enumerate() {
            if index == current || rail.kind != RailKind::Grind {
                continue;
            }
            let point = rail.position(rail.nearest(index, probe));
            let across = Vec2::new(point.x - probe.x, point.z - probe.z).length();
            if across < grind.reach && (probe.y - point.y).abs() < grind.reach_vertical {
                return Some(rail.nearest(index, hero.position));
            }
        }
        distance += grind.switch_step;
    }
    None
}

/// Catches a zip line's cable with the hands (`hero-controller.md`, "Zip lines"): jumping,
/// falling or grinding, as the games' state types 4, 2 and 15. A rail found by the grind
/// search came first.
fn try_catch_zip_line(hero: &mut HeroMotion, profile: &HeroProfile, world: &dyn CollisionWorld) {
    let may = hero.state.is_grinding()
        || matches!(
            hero.state,
            HeroState::Fall | HeroState::Jump(_) | HeroState::DoubleJump
        );
    let zip = &profile.zip_line;
    if !zip.enabled || !may || hero.no_zip_ticks > 0 {
        return;
    }
    let hands = hero.position + Vec3::Y * zip.hands_up;
    for (index, rail) in world.rails().iter().enumerate() {
        if rail.kind != RailKind::ZipLine {
            continue;
        }
        let at = rail.nearest(index, hands);
        let point = rail.position(at);
        let across = Vec2::new(point.x - hands.x, point.z - hands.z).length();
        let rise = hands.y - point.y;
        if across < zip.reach
            && rise.abs() < zip.reach_vertical
            && rise > -zip.below
            && rise < zip.above
        {
            start_zip_line(hero, rail, at, profile);
            return;
        }
    }
}

fn start_zip_line(hero: &mut HeroMotion, rail: &GrindRail, at: RailPoint, profile: &HeroProfile) {
    let zip = &profile.zip_line;
    hero.zip_speed = (hero.velocity.dot(rail.direction(at)) * zip.catch_share).max(0.0);
    hero.zip_end = 0;
    hero.rail_offset = hero.position - (rail.position(at) - Vec3::Y * zip.hang_below);
    hero.rail_pull = 0.0;
    hero.rail = Some(at);
    hero.switch_from = None;
    hero.ledge = None;
    hero.wall = None;
    hero.ground = None;
    hero.velocity = Vec3::ZERO;
    hero.set_state(HeroState::ZipLine);
}

/// One tick on a zip line (state 118). The cable carries him at a speed that only builds,
/// and nothing the player does changes it; at its end he drops off.
fn zip_line_tick(hero: &mut HeroMotion, profile: &HeroProfile, world: &dyn CollisionWorld) {
    let zip = &profile.zip_line;
    if hero.zip_end > 0 {
        // Past the end, flying on the way the cable went.
        hero.zip_end += 1;
        hero.velocity.y -= zip.end_gravity * TICK;
        if hero.zip_end >= zip.end_ticks {
            hero.no_zip_ticks = zip.no_catch_ticks;
            let velocity = hero.velocity;
            leave_rail(hero, velocity);
        }
        return;
    }
    let Some((rail, mut at)) = hero
        .rail
        .and_then(|at| world.rails().get(at.rail).map(|rail| (rail, at)))
    else {
        hero.no_zip_ticks = zip.no_catch_ticks;
        leave_rail(hero, Vec3::ZERO);
        return;
    };
    let along = rail.direction(at);
    hero.zip_speed = approach(hero.zip_speed, zip.speed, zip.accel * TICK);
    hero.speed = hero.zip_speed;
    turn(hero, yaw_of(flat(along)), zip.turn);
    let on = rail.advance(&mut at, hero.zip_speed * TICK);
    hero.rail = Some(at);
    if !on {
        hero.zip_end = 1;
        hero.velocity = along * hero.zip_speed;
        return;
    }
    // The hang: from where he caught the cable to under it, on the game's spring.
    let gap = hero.rail_offset.length();
    let closing = -game_spring(-gap, 0.0, zip.hang, zip.hang_cap, &mut hero.rail_pull);
    hero.rail_offset = hero.rail_offset.normalize_or_zero() * closing;
    let target = rail.position(at) - Vec3::Y * zip.hang_below + hero.rail_offset;
    hero.velocity = (target - hero.position) / TICK;
}

/// The water at `feet`, as the games' ground probe finds it (`hero-controller.md`,
/// "Swimming").
fn find_water(world: &dyn CollisionWorld, feet: Vec3) -> Option<WaterLevel> {
    let surface = water::surface_at(world.water(), feet)?;
    let depth = height_above_ground(world, Vec3::new(feet.x, surface, feet.z));
    Some(WaterLevel { surface, depth })
}

/// The games' spring (`hero-controller.md`, "Swimming"): the rate, kept within the error,
/// gains `gain × error` and loses `damping × rate`, is capped at `cap` when that is
/// positive, and is kept within the error again. Within 1% of the cap of its target, the
/// value snaps to it.
pub fn game_spring(value: f32, target: f32, spring: Spring, cap: f32, rate: &mut f32) -> f32 {
    let error = target - value;
    *rate = rate.clamp(-error.abs(), error.abs());
    *rate += spring.gain * error - spring.damping * *rate;
    if cap > 0.0 {
        *rate = rate.clamp(-cap, cap);
    }
    *rate = rate.clamp(-error.abs(), error.abs());
    let next = value + *rate;
    if (target - next).abs() < cap * 0.01 {
        *rate = 0.0;
        return target;
    }
    next
}

/// Swimming fast: R1 or R2 in the games, crouch or strafe here.
fn fast_swim_held(input: &PlayerInput) -> bool {
    input.held(Action::Crouch) || input.held(Action::Strafe)
}

/// Gets the hero into the water, or up to its surface from under it (`hero_enter_water`).
fn enter_water(hero: &mut HeroMotion, profile: &HeroProfile) {
    let swim = &profile.swim;
    let Some(water) = hero.water else {
        return;
    };
    let feet = hero.position.y;
    let state = hero.state;
    if state.is_underwater() {
        if hero.dive_left == 0 && hero.velocity.y > 0.0 && feet > water.surface - swim.surface_below
        {
            start_floating(hero, water.surface, profile);
        }
        return;
    }
    if state.is_on_surface()
        || state.is_on_ledge()
        || matches!(state, HeroState::Grind | HeroState::Dead)
    {
        return;
    }
    // A jump only counts on its way down.
    let past_apex = match state {
        HeroState::Jump(_) => hero.jump.past_apex,
        HeroState::DoubleJump | HeroState::Glide => hero.velocity.y <= 0.0,
        _ => true,
    };
    let mut gets_in = false;
    if past_apex {
        let slack = swim
            .entry_slack
            .max((hero.velocity.y * TICK).abs() + swim.entry_slack_extra);
        gets_in = (water.surface - (feet + swim.entry_below)).abs() < slack
            && water.depth > swim.entry_depth
            && hero.velocity.y < 0.0;
        gets_in |= feet < water.surface - swim.deep_below;
    }
    if state == HeroState::Jump(JumpKind::Water)
        && hero.jump.past_apex
        && feet < water.surface - swim.jump_back_below
    {
        gets_in = true;
    }
    if gets_in {
        start_floating(hero, water.surface, profile);
    }
}

/// Surface idle (state 54) from in the water or under it: the bob starts from how he
/// came in, and his horizontal velocity drifts on.
fn start_floating(hero: &mut HeroMotion, surface: f32, profile: &HeroProfile) {
    let swim = &profile.swim;
    hero.set_state(HeroState::SurfaceIdle);
    hero.ground = None;
    hero.rail = None;
    hero.ledge = None;
    hero.speed = 0.0;
    hero.stroke_left = 0;
    hero.bob = hero.position.y - (surface - swim.float_below);
    hero.bob_rate = (swim.bob_entry_scale * hero.velocity.y).max(-swim.bob_entry_max) * TICK;
    hero.float_height = surface;
    hero.float_rate = 0.0;
    hero.drift = hero.horizontal_velocity().clamp_length_max(swim.drift_max);
}

/// Stops swimming when the water runs out under him, which the games' levels don't let
/// happen.
fn out_of_water(hero: &mut HeroMotion) {
    hero.pitch = 0.0;
    hero.pitch_rate = 0.0;
    hero.take_off_height = hero.position.y;
    hero.double_jumped = true;
    hero.set_state(HeroState::Fall);
}

fn reset_stroke(hero: &mut HeroMotion) {
    hero.stroke_frame = 0.0;
    hero.stroke_step = 0.0;
}

/// How hard the stroke pushes this tick: the stroke table over the frames the animation
/// crossed last tick, as the game reckons it.
fn stroke_factor(hero: &HeroMotion, swim: &SwimProfile) -> f32 {
    let table = &swim.stroke;
    if table.is_empty() {
        return 0.0;
    }
    let count = table.len();
    let whole = (hero.stroke_frame as usize).min(count - 1);
    let fraction = hero.stroke_frame - whole as f32;
    let before = (whole + count - 1) % count;
    fraction * table[whole] + (hero.stroke_step - fraction) * table[before]
}

fn advance_stroke(hero: &mut HeroMotion, rate: f32, swim: &SwimProfile) {
    hero.stroke_step = swim.stroke_frames_per_rate * rate;
    let frames = swim.stroke.len().max(1) as f32;
    hero.stroke_frame = (hero.stroke_frame + hero.stroke_step).rem_euclid(frames);
}

fn start_dive(hero: &mut HeroMotion, fast: bool, profile: &HeroProfile) {
    hero.set_state(if fast {
        HeroState::UnderwaterFast
    } else {
        HeroState::UnderwaterSwim
    });
    hero.dive_left = profile.swim.dive_ticks;
    reset_stroke(hero);
}

/// One tick on the surface (states 53 and 54).
fn surface_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
) {
    let swim = &profile.swim;
    let Some(water) = hero.water else {
        out_of_water(hero);
        air_tick(hero, input, stick, wish, profile);
        return;
    };
    let idle = hero.state == HeroState::SurfaceIdle;

    if hero.jump_press.within(swim.jump_press_window)
        && (!idle || hero.ticks > swim.idle_jump_after_tick)
    {
        hero.jump_press.used = true;
        hero.pitch = 0.0;
        hero.pitch_rate = 0.0;
        start_jump(hero, JumpKind::Water, profile);
        air_tick(hero, input, stick, wish, profile);
        return;
    }
    // DL's surface states have no dive (`swim.dives`).
    let window = swim.dive_press_window;
    let fast_pressed = hero.crouch_press.within(window) || hero.strafe_press.within(window);
    if swim.dives && (hero.melee_press.within(window) || (idle && fast_pressed)) {
        hero.melee_press.used = true;
        hero.crouch_press.used = true;
        hero.strafe_press.used = true;
        start_dive(hero, fast_swim_held(input), profile);
        underwater_tick(hero, input, stick, wish, profile);
        return;
    }
    let stand_depth = if idle {
        swim.stand_depth_idle
    } else {
        swim.stand_depth_swimming
    };
    if water.depth < stand_depth {
        // The game wades in state 117, not read yet; the kit lands and walks.
        hero.velocity.y = 0.0;
        out_of_water(hero);
        air_tick(hero, input, stick, wish, profile);
        return;
    }
    if idle {
        if stick.magnitude > swim.start_stick && hero.ticks > swim.start_after_tick {
            hero.set_state(HeroState::SurfaceSwim);
            hero.stroke_left = swim.stroke_ticks;
            hero.bob_spring_rate = 0.0;
            reset_stroke(hero);
        }
    } else if stick.magnitude < swim.stop_stick && hero.speed == 0.0 && hero.stroke_left == 0 {
        start_floating(hero, water.surface, profile);
    }

    // The stick's way, relative to the camera, or on as he faces with it released.
    let target_yaw = if stick.is_active() {
        yaw_of(wish)
    } else {
        hero.yaw
    };
    turn(hero, target_yaw, swim.turn);
    let mut target = swim.surface_speed * stick.magnitude;
    if stick.is_active() {
        target = target.max(swim.surface_min_speed);
    }
    hero.stroke_left = hero.stroke_left.saturating_sub(1);
    let mut horizontal = Vec3::ZERO;
    if hero.state == HeroState::SurfaceSwim {
        if !stick.is_active() && hero.stroke_left > 0 {
            target = swim.surface_speed;
        }
        if wrap(target_yaw - hero.yaw).abs() > swim.surface_turn_limit_degrees.to_radians() {
            target = 0.0;
        }
        let rate = if hero.speed < target {
            swim.surface_accel
        } else {
            swim.surface_brake
        };
        hero.speed = approach(hero.speed, target, rate * TICK);
        horizontal = hero.facing() * (hero.speed * swim.surface_stroke * stroke_factor(hero, swim));
        advance_stroke(hero, swim.surface_stroke_rate, swim);
    } else {
        hero.speed = 0.0;
    }
    let drift = (hero.drift.length() - swim.drift_brake * TICK).max(0.0);
    hero.drift = hero.drift.normalize_or_zero() * drift;
    hero.set_horizontal_velocity(horizontal + hero.drift);
    // The games don't level him out on the surface in this code; the kit does.
    hero.pitch = game_spring(
        hero.pitch,
        0.0,
        swim.pitch_spring,
        swim.pitch_cap,
        &mut hero.pitch_rate,
    );
    float(hero, water.surface, profile);
}

/// Where his feet float this tick (`hero_surface_float`), as a vertical velocity.
fn float(hero: &mut HeroMotion, surface: f32, profile: &HeroProfile) {
    let swim = &profile.swim;
    let swimming = hero.state == HeroState::SurfaceSwim;
    if hero.ticks > swim.bob_free_ticks || hero.bob < 0.0 || hero.previous_state.is_underwater() {
        if swimming {
            hero.bob = game_spring(
                hero.bob,
                0.0,
                swim.bob_swim,
                swim.bob_swim_cap * TICK,
                &mut hero.bob_spring_rate,
            );
            hero.bob_rate = 0.0;
        } else {
            hero.bob_rate -= hero.bob * swim.bob_stiffness + hero.bob_rate * swim.bob_damping;
            if hero.bob.abs() < 0.001 && hero.bob_rate < 0.0001 {
                hero.bob_rate = 0.0;
            }
        }
    }
    hero.bob += hero.bob_rate;
    if swimming && hero.bob > 0.0 {
        hero.bob_rate = hero.bob_rate.min(0.0);
        hero.bob = approach(hero.bob, 0.0, swim.bob_sink * TICK);
    }
    hero.float_height = game_spring(
        hero.float_height,
        surface,
        swim.surface_follow,
        0.0,
        &mut hero.float_rate,
    );
    let feet = hero.float_height - swim.float_below + hero.bob;
    hero.velocity.y = (feet - hero.position.y) / TICK;
}

/// One tick under water (states 50, 51 and 52).
fn underwater_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
) {
    let swim = &profile.swim;
    if hero.water.is_none() {
        out_of_water(hero);
        air_tick(hero, input, stick, wish, profile);
        return;
    }
    let rise = input.held(Action::Jump);
    let sink = input.held(Action::Melee);
    let pitching = rise || sink;
    let fast = fast_swim_held(input);

    let diving = hero.dive_left > 0;
    match hero.state {
        HeroState::UnderwaterSwim => {
            let mid_stroke = hero.stroke_frame > swim.idle_stroke_from
                && hero.stroke_frame < swim.idle_stroke_to;
            if !pitching
                && !diving
                && mid_stroke
                && stick.magnitude < swim.stop_stick
                && hero.speed < swim.idle_speed
            {
                start_underwater_idle(hero);
            } else if fast && hero.ticks > swim.fast_after_tick {
                hero.set_state(HeroState::UnderwaterFast);
            }
        }
        HeroState::UnderwaterFast if !fast => {
            if stick.magnitude > swim.fast_to_swim_stick {
                hero.set_state(HeroState::UnderwaterSwim);
                reset_stroke(hero);
            } else if !pitching && !diving && stick.magnitude < swim.fast_to_idle_stick {
                start_underwater_idle(hero);
            }
        }
        HeroState::UnderwaterIdle if stick.magnitude > swim.start_stick || pitching || fast => {
            if fast {
                hero.set_state(HeroState::UnderwaterFast);
            } else {
                hero.set_state(HeroState::UnderwaterSwim);
                reset_stroke(hero);
            }
        }
        _ => {}
    }
    let state = hero.state;
    let diving = hero.dive_left > 0;

    // Pitch: down with melee (Square), up with jump (Cross), level otherwise. The games
    // read how hard the button is pressed; the kit's buttons are always fully pressed.
    let mut target = if (sink && !rise) || diving {
        let press: f32 = if sink { 1.0 } else { 0.0 };
        let press = if diving {
            press.max(swim.dive_min_pitch)
        } else {
            press
        };
        press * swim.pitch_max
    } else if rise {
        -swim.pitch_max
    } else {
        0.0
    };
    if state == HeroState::UnderwaterIdle {
        target = 0.0;
    }
    let cap = swim.pitch_cap
        + if pitching {
            swim.pitch_cap_pressed
        } else {
            0.0
        };
    hero.pitch = game_spring(
        hero.pitch,
        target,
        swim.pitch_spring,
        cap,
        &mut hero.pitch_rate,
    );

    let target_yaw = if stick.is_active() {
        yaw_of(wish)
    } else {
        hero.yaw
    };
    turn(hero, target_yaw, swim.turn);

    let fast_state = state == HeroState::UnderwaterFast;
    let mut target = if diving {
        swim.dive_speed
    } else if fast_state {
        if fast {
            swim.fast_speed
        } else {
            swim.fast_coast_speed
        }
    } else {
        swim.under_speed
    };
    let mut scale = stick.magnitude;
    if pitching || diving {
        scale = 1.0;
    }
    if stick.magnitude > swim.scale_from {
        let least = if fast_state {
            swim.fast_min_scale
        } else {
            swim.min_scale
        };
        scale = scale.max(least);
    }
    if !(fast_state && fast) {
        target *= scale;
    }
    let (accel, brake) = if fast_state {
        (swim.fast_accel, swim.fast_brake)
    } else {
        (swim.under_accel, swim.under_brake)
    };
    let rate = if hero.speed < target { accel } else { brake };
    hero.speed = approach(hero.speed, target, rate * TICK);

    // Along his facing, tipped by the pitch.
    let direction = hero.facing() * ops::cos(hero.pitch) + Vec3::NEG_Y * ops::sin(hero.pitch);
    let after_fast = hero.previous_state == HeroState::UnderwaterFast;
    hero.velocity = match state {
        HeroState::UnderwaterFast => direction * hero.speed,
        HeroState::UnderwaterSwim => {
            let mut push = hero.speed * swim.under_stroke * stroke_factor(hero, swim);
            if after_fast && hero.ticks < swim.after_fast_ticks {
                push = push.max(swim.after_fast_min_speed);
            }
            direction * push
        }
        _ => {
            let brake = if after_fast {
                swim.idle_brake_after_fast
            } else {
                swim.idle_brake
            };
            let drift = (hero.drift.length() - brake * TICK).max(0.0);
            hero.drift = hero.drift.normalize_or_zero() * drift;
            hero.drift
        }
    };
    let stroke_rate = (hero.speed * swim.under_stroke_rate_per_speed)
        .clamp(swim.under_stroke_rate_min, swim.under_stroke_rate_max);
    advance_stroke(hero, stroke_rate, swim);
    hero.dive_left = hero.dive_left.saturating_sub(1);
}

/// Underwater idle (state 51): he drifts on with the velocity he had.
fn start_underwater_idle(hero: &mut HeroMotion) {
    hero.set_state(HeroState::UnderwaterIdle);
    hero.drift = hero.velocity;
}

fn air_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
) {
    let air = &profile.air;
    let t = hero.ticks;
    match hero.state {
        HeroState::Jump(kind) => {
            if let Some(next) = jump_transition(hero, kind, input, wish, profile) {
                match next {
                    Transition::Flip(side) => {
                        start_flip(hero, wish.normalize_or_zero(), stick, side, profile);
                    }
                    Transition::Wall(wall) => start_wall_jump(hero, wall, profile),
                    Transition::Long => start_jump(hero, JumpKind::Long, profile),
                    Transition::Double => {
                        hero.double_jumped = true;
                        hero.set_state(HeroState::DoubleJump);
                        double_jump_tick(hero, input, stick, wish, profile);
                        return;
                    }
                }
                air_tick(hero, input, stick, wish, profile);
                return;
            }

            let params = jump_params(profile, kind).clone();
            let past_pushoff = t >= params.pushoff_ticks;
            let falling = hero.jump.past_apex;
            let to_land = ticks_to_land(hero);
            jump_vertical(hero, &params, input.held(Action::Jump), profile);
            if kind == JumpKind::Heli {
                let heli = &profile.heli_jump;
                if (heli.thrust_from_tick..=heli.thrust_to_tick).contains(&t) {
                    hero.velocity.y += heli.thrust * TICK;
                }
                let glide = &profile.glide;
                if hero.abilities.heli_pack
                    && input.held(Action::Jump)
                    && t > glide.after_heli_jump_tick
                    && hero.height_above_ground > glide.heli_jump_min_height
                {
                    start_glide(hero);
                    return;
                }
            }

            match kind {
                JumpKind::Long if !past_pushoff => {
                    // The base speed heads for the walk speed, and each push-off tick
                    // adds a push that grows with t; after push-off the velocity is kept
                    // for the whole flight (`hero-controller.md`, "Long jump").
                    hero.long_jump_base = approach(
                        hero.long_jump_base,
                        profile.walk.speed,
                        air.pushoff_towards_cap * TICK,
                    );
                    let push = hero.long_jump_base + t as f32 * profile.long_jump.forward_push;
                    let facing = hero.facing();
                    hero.set_horizontal_velocity(facing * push);
                }
                JumpKind::Long => {}
                // Following a rail, or flying on from its end (`follow_rail_in_air`).
                JumpKind::Grind | JumpKind::GrindSwitch => {}
                JumpKind::Flip => flip_tick(hero, stick, falling, to_land, profile),
                JumpKind::Wall => {
                    // Off the wall along its normal, turning to face that way.
                    let wall_jump = &profile.wall_jump;
                    if let Some(normal) = hero.last_wall {
                        if t >= wall_jump.turn_from_tick {
                            turn(hero, yaw_of(normal), wall_jump.turn);
                        }
                        if t >= wall_jump.away_from_tick {
                            hero.set_horizontal_velocity(normal * wall_jump.speed);
                        }
                    }
                }
                JumpKind::Ledge => {
                    // Forward onto the ledge, timed like the climbing animation.
                    let ledge = &profile.ledge;
                    let step = ledge.climb_forward_accel * TICK;
                    if (ledge.climb_forward_from_tick..=ledge.climb_forward_until_tick).contains(&t)
                    {
                        hero.climb_speed =
                            approach(hero.climb_speed, ledge.climb_forward_speed, step);
                    } else if t > ledge.climb_forward_until_tick
                        && t <= ledge.climb_forward_stop_tick
                    {
                        hero.climb_speed = approach(hero.climb_speed, 0.0, step);
                    }
                    let facing = hero.facing();
                    hero.set_horizontal_velocity(facing * hero.climb_speed);
                    if t > ledge.climb_turn_after_tick {
                        face_movement(hero, wish, stick, profile);
                    }
                }
                _ if !past_pushoff => {
                    // Push-off eases the ground speed towards the move's air speed.
                    let speed = hero.horizontal_velocity().length();
                    let target = params.air_speed * stick.vector.length().min(1.0);
                    let rate = if speed < target {
                        air.pushoff_towards_cap
                    } else {
                        air.pushoff_brake
                    };
                    let next = approach(speed, target, rate * TICK);
                    let direction = if hero.strafing {
                        hero.strafe_heading
                    } else {
                        hero.facing()
                    };
                    hero.set_horizontal_velocity(direction * next);
                }
                _ => {
                    air_control(
                        hero,
                        stick,
                        wish,
                        params.air_speed,
                        air.release_brake,
                        profile,
                    );
                }
            }
        }
        HeroState::DoubleJump => double_jump_tick(hero, input, stick, wish, profile),
        HeroState::Glide => {
            let glide = &profile.glide;
            if !input.held(Action::Jump) {
                hero.set_state(HeroState::Fall);
                air_tick(hero, input, stick, wish, profile);
                return;
            }
            let spring = glide.spring;
            hero.glide_rate +=
                spring.gain * (-glide.sink - hero.velocity.y) - spring.damping * hero.glide_rate;
            hero.glide_rate = hero.glide_rate.clamp(-spring.cap, spring.cap);
            hero.velocity.y += hero.glide_rate;
            let current = hero.horizontal_velocity();
            let (target, rate) = if stick.is_active() {
                (wish.normalize_or_zero() * glide.speed, glide.accel)
            } else {
                (Vec3::ZERO, glide.brake)
            };
            let next = current + (target - current).clamp_length_max(rate * TICK);
            hero.set_horizontal_velocity(next);
            face_movement(hero, wish, stick, profile);
        }
        HeroState::Fall => {
            let fall = &profile.fall;
            hero.velocity.y = (hero.velocity.y - fall.gravity * TICK).max(-air.fall_speed_cap);
            let landing = &fall.landing;
            hero.fell_far |= hero.ticks > landing.far_after_ticks
                || hero.height_above_ground > landing.far_above;
            air_control(
                hero,
                stick,
                wish,
                fall.air_speed,
                fall.release_brake,
                profile,
            );
        }
        _ => {}
    }
}

enum Transition {
    Flip(StrafeSide),
    Wall(Wall),
    Long,
    Double,
}

/// What a jump turns into this tick, in the order the games check (`hero-controller.md`,
/// "Wall jump", "Long jump", "Double jump").
fn jump_transition(
    hero: &mut HeroMotion,
    kind: JumpKind,
    input: &PlayerInput,
    wish: Vec3,
    profile: &HeroProfile,
) -> Option<Transition> {
    let t = hero.ticks;
    // A jump still on the ground turns into a flip with crouch held and the stick just
    // pushed to a side or back (UYA 0x503B68; DL ignores the crouch here, 0x6A3D28).
    let crouch = &profile.flip.crouch;
    if matches!(
        kind,
        JumpKind::Standing | JumpKind::Run | JumpKind::Long | JumpKind::Heli
    ) && t < crouch.early_before_tick
        && hero.ground.is_some()
        && crouch_held(input, profile)
        && hero.stick_press.within(crouch.early_window)
    {
        let side = flip_side(hero, wish.normalize_or_zero());
        if side != StrafeSide::Forward {
            return Some(Transition::Flip(side));
        }
    }
    // A crouch early in a jump makes it a long jump (UYA; DL ignores it, 0x6A3D88).
    let long = &profile.long_jump;
    if matches!(kind, JumpKind::Standing | JumpKind::Run)
        && hero.abilities.heli_pack
        && profile.walk.crouches
        && t < long.from_jump_before_tick
        && hero.crouch_press.within(long.crouch_window)
        && hero.horizontal_velocity().length() > long.min_speed
    {
        hero.crouch_press.used = true;
        // The run jump's speed carries into the long jump.
        hero.speed = hero.horizontal_velocity().length();
        return Some(Transition::Long);
    }
    // Nothing else can happen until the push-off is a tick behind.
    if !past_pushoff_gate(hero, profile) {
        return None;
    }

    let wall_jump = &profile.wall_jump;
    let can_wall_jump = match kind {
        JumpKind::Standing | JumpKind::Run => true,
        JumpKind::Wall => t > wall_jump.again_after_tick,
        _ => false,
    };
    if can_wall_jump
        && let Some(wall) = hero.wall
        && hero.jump_press.within(wall_jump.press_window)
        && hero.jump_press.ticks < t
    {
        hero.jump_press.used = true;
        return Some(Transition::Wall(wall));
    }

    if !matches!(kind, JumpKind::Standing | JumpKind::Run) {
        return None;
    }
    let double = &profile.double_jump;
    let window = t
        .saturating_sub(double.press_skip)
        .clamp(double.press_window_min, double.press_window_max);
    let height_ok = !hero.jump.past_apex
        || (hero.height_above_ground > double.past_apex_min_height
            && hero.velocity.y.abs() < double.past_apex_max_speed);
    // No double jump while a gun's firing animation plays (`weapons.md`).
    if !hero.double_jumped
        && !hero.gun.firing
        && t > double.after_tick
        && hero.jump_press.within(window)
        && height_ok
    {
        hero.jump_press.used = true;
        return Some(Transition::Double);
    }
    None
}

/// The vertical model every jump-type move shares (`hero-controller.md`, "Vertical
/// model").
fn jump_vertical(hero: &mut HeroMotion, params: &JumpParams, holding: bool, profile: &HeroProfile) {
    let air = &profile.air;
    let t = hero.ticks;
    let jump = &mut hero.jump;

    // Holding the button raises the target height; the first tick always counts.
    if jump.holds_left > 0 && (t == 0 || (jump.holding && holding)) {
        jump.height += jump.step;
        jump.holds_left -= 1;
    } else {
        jump.holding = false;
    }

    // The climb hangs still through its push-off and one tick more, where other jumps press
    // into the ground [verified on UYA NTSC and DL NTSC, 2026-10-20].
    if hero.state == HeroState::Jump(JumpKind::Ledge) && t <= params.pushoff_ticks {
        hero.velocity.y = 0.0;
        return;
    }
    if t < params.pushoff_ticks {
        hero.velocity.y = -air.pushoff_press * TICK;
        return;
    }
    hero.velocity.y = (hero.velocity.y - jump.gravity * TICK).max(-air.fall_speed_cap);
    if !jump.blocked {
        // Top the speed up to what a projectile needs to reach the target height.
        let needed = ops::sqrt(2.0 * params.gravity * jump.height) - jump.lift;
        if needed > 0.0 {
            hero.velocity.y += needed;
            jump.lift += needed;
        }
    }
    if !jump.past_apex && hero.velocity.y < air.apex_below {
        jump.past_apex = true;
        jump.gravity *= params.apex_gravity_factor;
    }
}

/// A jump-type state's transitions wait until its push-off is over and one more tick has
/// passed (`hero-controller.md`, "Ledges").
fn past_pushoff_gate(hero: &HeroMotion, profile: &HeroProfile) -> bool {
    match hero.state {
        HeroState::Jump(kind) => hero.ticks > jump_params(profile, kind).pushoff_ticks + 1,
        _ => true,
    }
}

fn start_glide(hero: &mut HeroMotion) {
    hero.glide_rate = 0.0;
    hero.set_state(HeroState::Glide);
}

fn double_jump_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
) {
    let double = &profile.double_jump;
    let glide = &profile.glide;
    let air = &profile.air;
    let t = hero.ticks;
    if hero.abilities.heli_pack
        && input.held(Action::Jump)
        && t > glide.after_double_jump_tick
        && hero.height_above_ground > glide.double_jump_min_height
    {
        start_glide(hero);
        air_tick(hero, input, stick, wish, profile);
        return;
    }
    if t < double.boost_ticks {
        hero.velocity.y -= double.gravity * TICK;
        // Less boost when already well above the first take-off.
        let above = hero.position.y - hero.take_off_height;
        let factor = if above > double.higher_above {
            double.higher_factor
        } else if above > double.high_above {
            double.high_factor
        } else {
            1.0
        };
        let cap = double.cap * factor;
        if hero.velocity.y < cap {
            hero.velocity.y = (hero.velocity.y + double.boost_accel * TICK).min(cap);
        }
    } else if t >= double.boost_ticks + double.hold_ticks {
        hero.velocity.y = (hero.velocity.y - double.gravity * TICK).max(-air.fall_speed_cap);
    }
    air_control(
        hero,
        stick,
        wish,
        double.air_speed,
        air.release_brake,
        profile,
    );
}

/// Steering in the air (`hero-controller.md`, "Air control").
fn air_control(
    hero: &mut HeroMotion,
    stick: &Stick,
    wish: Vec3,
    air_speed: f32,
    release_brake: f32,
    profile: &HeroProfile,
) {
    let air = &profile.air;
    face_in_air(hero, wish, stick, profile);
    let current = hero.horizontal_velocity();
    let stick_length = stick.vector.length().min(1.0);
    // The game compares the speed asked for with a share of the walk speed, so a slow move
    // such as the double jump lets go at a firmer stick (UYA 0x4EDE80, DL 0x680A88).
    if air_speed * stick_length < air.release_below * profile.walk.speed {
        let speed = approach(current.length(), 0.0, release_brake * TICK);
        hero.set_horizontal_velocity(current.normalize_or_zero() * speed);
        return;
    }
    let target = wish.normalize_or_zero() * air_speed * stick_length;
    // Steering against the current motion brakes up to three times as hard.
    let cos = if current.length_squared() > 1e-8 && target.length_squared() > 1e-8 {
        current.normalize().dot(target.normalize())
    } else {
        1.0
    };
    let factor = (1.5 * (1.0 - cos)).clamp(air.control_factor_min, air.control_factor_max);
    let next = current + (target - current).clamp_length_max(air.control_accel * factor * TICK);
    hero.set_horizontal_velocity(next);
}

/// Jumping and falling he turns to the way he steers, or to the camera while strafing.
fn face_in_air(hero: &mut HeroMotion, wish: Vec3, stick: &Stick, profile: &HeroProfile) {
    if hero.strafing {
        turn(hero, hero.camera_yaw, profile.air.turn);
    } else {
        face_movement(hero, wish, stick, profile);
    }
}

fn face_movement(hero: &mut HeroMotion, wish: Vec3, stick: &Stick, profile: &HeroProfile) {
    if stick.is_active() {
        turn(hero, yaw_of(wish), profile.air.turn);
    }
}
