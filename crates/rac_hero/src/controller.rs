//! The hero's moves, one 60 Hz tick at a time, after `docs/research/hero-controller.md`.
//!
//! [`step`] is a plain function of the hero's state, one tick's input, the camera's yaw,
//! the profile and the collision world. It reads no clock and no global state, so the same
//! inputs always give the same motion, and tests can drive it directly.
//!
//! Directions: y is up, and a yaw of 0 faces −z. The stick is read relative to the
//! camera's yaw, as in the games.

use bevy::{math::ops, prelude::*};
use rac_input::{Action, PlayerInput};
use std::f32::consts::{PI, TAU};

use crate::{
    collision::CollisionWorld,
    motor::{self, Ground},
    profile::{HeroProfile, JumpParams, TurnSpring},
    stick::{Stick, read_stick},
};

/// Seconds per tick.
pub const TICK: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JumpKind {
    /// From standing.
    Standing,
    /// From walking.
    Run,
    /// From a running crouch.
    Long,
    /// While strafing, to the side or back.
    Flip,
    /// From a standing crouch: UYA's jump with the heli-pack's thrust.
    Heli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeroState {
    Idle,
    Walk,
    /// Braking hard: after a sharp turn, or landing from a long jump.
    Skid,
    Crouch,
    Fall,
    Jump(JumpKind),
    DoubleJump,
    Glide,
}

impl HeroState {
    /// The games' number for the state (libuya's `PlayerState`), for comparing with
    /// traces (`hero-controller.md`, "State numbers").
    pub fn game_number(self) -> u32 {
        match self {
            HeroState::Idle => 0,
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
        }
    }

    pub fn is_airborne(self) -> bool {
        matches!(
            self,
            HeroState::Fall | HeroState::Jump(_) | HeroState::DoubleJump | HeroState::Glide
        )
    }
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
    /// Walking, but not yet past the setting-off speed.
    setting_off: bool,
    /// Ticks of braking done since the stick was released.
    stop_ticks: u32,
    jump: JumpBlock,
    take_off_height: f32,
    double_jumped: bool,
    flip_direction: Vec3,
    flip_speed: f32,
    flip_target: f32,
    /// The long jump's base speed, before its per-tick push.
    long_jump_base: f32,
    /// The glide's vertical spring rate, u/s per tick.
    glide_rate: f32,
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
            setting_off: true,
            stop_ticks: 0,
            jump: JumpBlock::default(),
            take_off_height: position.y,
            double_jumped: false,
            flip_direction: Vec3::ZERO,
            flip_speed: 0.0,
            flip_target: 0.0,
            long_jump_base: 0.0,
            glide_rate: 0.0,
        }
    }

    pub fn facing(&self) -> Vec3 {
        facing(self.yaw)
    }

    fn set_state(&mut self, state: HeroState) {
        if self.state != state {
            self.state = state;
            self.ticks = 0;
        }
    }

    fn horizontal_velocity(&self) -> Vec3 {
        Vec3::new(self.velocity.x, 0.0, self.velocity.z)
    }

    fn set_horizontal_velocity(&mut self, horizontal: Vec3) {
        self.velocity.x = horizontal.x;
        self.velocity.z = horizontal.z;
    }

    fn in_pushoff(&self, profile: &HeroProfile) -> bool {
        match self.state {
            HeroState::Jump(kind) => self.ticks < jump_params(profile, kind).pushoff_ticks,
            _ => false,
        }
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
    }
}

/// Advances `hero` by one tick.
pub fn step(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    camera_yaw: f32,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let stick = read_stick(input.move_stick, &profile.stick);
    let wish = stick_on_ground(stick.vector, camera_yaw);

    // Ride whatever he stands on.
    if let Some(ground) = hero.ground {
        hero.position += world.body_motion(ground.body);
    }

    if hero.state.is_airborne() {
        air_tick(hero, input, &stick, wish, profile);
    } else {
        ground_tick(hero, input, &stick, wish, camera_yaw, profile);
    }

    // During a jump's push-off he is still pressed to the ground.
    let pushing_off = hero.in_pushoff(profile);
    let grounded = !hero.state.is_airborne() || pushing_off;
    let outcome = motor::move_body(
        world,
        &profile.body,
        hero.position,
        hero.velocity * TICK,
        grounded,
    );
    hero.position = outcome.position;
    hero.ticks += 1;

    if !hero.state.is_airborne() {
        hero.ground = outcome.ground;
        if outcome.ground.is_none() {
            // Walked off an edge.
            hero.set_horizontal_velocity(hero.horizontal_velocity());
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
    match outcome.ground {
        Some(ground) if hero.velocity.y <= 0.0 => land(hero, ground, &stick),
        _ => hero.ground = None,
    }
}

fn land(hero: &mut HeroMotion, ground: Ground, stick: &Stick) {
    let horizontal = hero.horizontal_velocity();
    let landing_from_long_jump = hero.state == HeroState::Jump(JumpKind::Long);
    hero.ground = Some(ground);
    hero.velocity.y = 0.0;
    hero.speed = horizontal.length();
    hero.stop_ticks = 0;
    if landing_from_long_jump {
        hero.set_state(HeroState::Skid);
    } else {
        // A landing walks on at once, without the setting-off phase.
        hero.setting_off = !stick.is_active();
        hero.set_state(HeroState::Walk);
    }
}

fn ground_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    camera_yaw: f32,
    profile: &HeroProfile,
) {
    let walk = &profile.walk;
    let wish_direction = wish.normalize_or_zero();

    if input.pressed(Action::Jump) {
        // While strafing, a jump to the side or back is a flip; forwards it is a jump.
        if input.held(Action::Strafe)
            && stick.is_active()
            && stick.vector.y < 0.7 * stick.vector.length()
        {
            start_flip(hero, wish_direction, stick, profile);
        } else if hero.state == HeroState::Crouch {
            let kind = if hero.speed >= profile.long_jump.min_speed {
                JumpKind::Long
            } else {
                JumpKind::Heli
            };
            start_jump(hero, kind, profile);
        } else {
            let kind = if hero.speed > 0.5 {
                JumpKind::Run
            } else {
                JumpKind::Standing
            };
            start_jump(hero, kind, profile);
        }
        air_tick(hero, input, stick, wish, profile);
        return;
    }

    if input.held(Action::Crouch) {
        hero.set_state(HeroState::Crouch);
        hero.speed = approach(hero.speed, 0.0, walk.crouch_brake * TICK);
        if stick.is_active() {
            turn(
                hero,
                yaw_of(wish_direction),
                walk.turn_setting_off.at(stick.magnitude),
            );
        }
        let facing = hero.facing();
        hero.set_horizontal_velocity(facing * hero.speed);
        return;
    }

    if input.held(Action::Strafe) && stick.is_active() {
        // Strafing faces the camera and moves with the stick. The games' strafe speeds
        // aren't read yet, so this reuses the walk's [placeholder].
        hero.set_state(HeroState::Walk);
        hero.stop_ticks = 0;
        turn(hero, camera_yaw, walk.turn_moving);
        let target = wish_direction * walk.speed * stick.magnitude;
        let current = hero.horizontal_velocity();
        let rate = if target.length() >= current.length() {
            walk.accel
        } else {
            walk.brake
        };
        let next = current + (target - current).clamp_length_max(rate * TICK);
        hero.set_horizontal_velocity(next);
        hero.speed = next.length();
        return;
    }

    if stick.is_active() {
        let target_yaw = yaw_of(wish_direction);
        let error = wrap(target_yaw - hero.yaw).abs();
        hero.stop_ticks = 0;
        if hero.state == HeroState::Walk
            && !hero.setting_off
            && error > walk.skid_angle
            && hero.speed > walk.skid_above
        {
            hero.set_state(HeroState::Skid);
        }
        if hero.state == HeroState::Skid {
            hero.speed = approach(hero.speed, 0.0, walk.skid_brake * TICK);
            turn(hero, target_yaw, walk.turn_skidding);
            if error < 0.5 || hero.speed < 1.0 {
                hero.setting_off = hero.speed < walk.setting_off_below;
                hero.set_state(HeroState::Walk);
            }
        } else {
            if hero.state != HeroState::Walk {
                hero.setting_off = hero.speed < walk.setting_off_below;
                hero.set_state(HeroState::Walk);
            }
            if hero.setting_off && hero.speed >= walk.setting_off_below {
                hero.setting_off = false;
            }
            let spring = if hero.setting_off {
                walk.turn_setting_off.at(stick.magnitude)
            } else {
                walk.turn_moving
            };
            turn(hero, target_yaw, spring);
            let target = walk.speed * stick.magnitude;
            let rate = if hero.speed < target {
                walk.accel
            } else {
                walk.brake
            };
            hero.speed = approach(hero.speed, target, rate * TICK);
        }
    } else {
        match hero.state {
            HeroState::Walk if hero.stop_ticks < walk.stop_skid_ticks => {
                // Letting go: a couple of ticks of skid braking...
                hero.speed = approach(hero.speed, 0.0, walk.skid_brake * TICK);
                hero.stop_ticks += 1;
            }
            HeroState::Walk => {
                // ...then idle, with what little speed is left.
                hero.speed = hero.speed.min(walk.stop_residual);
                hero.set_state(HeroState::Idle);
            }
            HeroState::Skid => {
                hero.speed = approach(hero.speed, 0.0, walk.skid_brake * TICK);
                if hero.speed <= 0.0 {
                    hero.set_state(HeroState::Idle);
                }
            }
            _ => {
                hero.set_state(HeroState::Idle);
                hero.speed = approach(hero.speed, 0.0, walk.stop_brake * TICK);
            }
        }
    }
    let facing = hero.facing();
    hero.set_horizontal_velocity(facing * hero.speed);
    hero.velocity.y = 0.0;
}

fn start_jump(hero: &mut HeroMotion, kind: JumpKind, profile: &HeroProfile) {
    let params = jump_params(profile, kind);
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
    if hero.state != HeroState::Jump(JumpKind::Flip) {
        let facing = hero.facing();
        hero.set_horizontal_velocity(facing * hero.speed);
    }
    hero.velocity.y = 0.0;
    hero.long_jump_base = hero.speed;
    hero.take_off_height = hero.position.y;
    hero.double_jumped = false;
}

fn start_flip(hero: &mut HeroMotion, direction: Vec3, stick: &Stick, profile: &HeroProfile) {
    start_jump(hero, JumpKind::Flip, profile);
    let flip = &profile.flip;
    hero.flip_direction = direction;
    hero.flip_speed = flip.start_speed;
    hero.flip_target = (flip.speed * stick.vector.length().min(1.0)).max(flip.min_speed);
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
            let params = jump_params(profile, kind).clone();
            let past_pushoff = t >= params.pushoff_ticks;
            if matches!(kind, JumpKind::Standing | JumpKind::Run)
                && past_pushoff
                && !hero.double_jumped
                && input.pressed(Action::Jump)
            {
                hero.double_jumped = true;
                hero.set_state(HeroState::DoubleJump);
                double_jump_tick(hero, input, stick, wish, profile);
                return;
            }

            jump_vertical(hero, &params, input.held(Action::Jump), profile);
            if kind == JumpKind::Heli {
                let heli = &profile.heli_jump;
                if (heli.thrust_from_tick..=heli.thrust_to_tick).contains(&t) {
                    hero.velocity.y += heli.thrust * TICK;
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
                JumpKind::Flip => {
                    hero.flip_speed =
                        approach(hero.flip_speed, hero.flip_target, profile.flip.accel * TICK);
                    hero.set_horizontal_velocity(hero.flip_direction * hero.flip_speed);
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
                    let facing = hero.facing();
                    hero.set_horizontal_velocity(facing * next);
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

fn double_jump_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
) {
    let double = &profile.double_jump;
    let air = &profile.air;
    let t = hero.ticks;
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
        if hero.velocity.y < -profile.glide.enter_below && input.held(Action::Jump) {
            hero.glide_rate = 0.0;
            hero.set_state(HeroState::Glide);
        }
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
    let current = hero.horizontal_velocity();
    let stick_length = stick.vector.length().min(1.0);
    if stick_length < air.release_below {
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
    face_movement(hero, wish, stick, profile);
}

fn face_movement(hero: &mut HeroMotion, wish: Vec3, stick: &Stick, profile: &HeroProfile) {
    if stick.is_active() {
        turn(hero, yaw_of(wish), profile.air.turn);
    }
}
