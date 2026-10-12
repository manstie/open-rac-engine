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

use crate::{
    collision::{BodyId, CollisionWorld, Material},
    motor::{self, Ground},
    profile::{HeroProfile, JumpParams, TurnSpring},
    stick::{Stick, read_stick},
};

/// Seconds per tick.
pub const TICK: f32 = 1.0 / 60.0;

/// How far down [`step`] looks for the ground under an airborne hero, as the games do.
const GROUND_SEARCH: f32 = 32.0;

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
    /// Off a wall, after clinging to it.
    Wall,
    /// Up from hanging on a ledge.
    Ledge,
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
    Walk,
    /// Braking hard: after a sharp turn, or landing from a long jump.
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
            HeroState::Hit => 22,
            HeroState::LedgeGrab => 23,
            HeroState::LedgeHang => 24,
            HeroState::LedgeShimmy(Side::Left) => 25,
            HeroState::LedgeShimmy(Side::Right) => 26,
            HeroState::Jump(JumpKind::Ledge) => 27,
            HeroState::Dead => 57,
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
    jump_press: PressAge,
    crouch_press: PressAge,
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
            jump_press: PressAge::NEVER,
            crouch_press: PressAge::NEVER,
            wall: None,
            wall_ticks: 0,
            last_wall: None,
            ordinary_wall_jumps: 0,
            no_grab_ticks: 0,
            climb_speed: 0.0,
            hit_fatal: false,
        }
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

    /// Pressed to the ground at the start of a jump. A wall jump's push-off is spent on
    /// the wall and a climb's on the ledge, so they don't count.
    fn pushing_off_ground(&self, profile: &HeroProfile) -> bool {
        match self.state {
            HeroState::Jump(JumpKind::Wall | JumpKind::Ledge) => false,
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
    let front = feet + forward * profile.body.radius;
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

    // 3. Nothing in the way just above the lip, back towards the hero.
    let above_lip = top + Vec3::Y * 0.005;
    if let Some(hit) = world.cast_ray(
        above_lip - forward * ledge.clear_back,
        forward * ledge.clear_back,
    ) && hit.body != body
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

    // 5. The lip: walk from the top's hit back out over the wall until the top runs out.
    let mut lip = top;
    let mut searched = 0.0;
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

    hero.jump_press.update(input.pressed(Action::Jump));
    hero.crouch_press.update(input.pressed(Action::Crouch));
    hero.wall_ticks = hero.wall_ticks.saturating_sub(1);
    if hero.wall_ticks == 0 {
        hero.wall = None;
    }
    hero.no_grab_ticks = hero.no_grab_ticks.saturating_sub(1);
    hero.safe_ticks = hero.safe_ticks.saturating_sub(1);

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

    match hero.state {
        HeroState::Dead => dead_tick(hero, profile),
        HeroState::Hit => hit_tick(hero, profile),
        state if state.is_on_ledge() => ledge_tick(hero, input, &stick, wish, profile, world),
        state if state.is_airborne() => air_tick(hero, input, &stick, wish, profile),
        _ => ground_tick(hero, input, &stick, wish, camera_yaw, profile),
    }

    move_hero(hero, &stick, profile, world);

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
    let on_ground = !state.is_airborne() && !state.is_on_ledge();
    let pushing_off = hero.pushing_off_ground(profile);
    // Hits and deaths keep to the ground while they are on it.
    let grounded = match state {
        HeroState::Hit | HeroState::Dead => hero.ground.is_some() && hero.velocity.y <= 0.0,
        _ => on_ground || pushing_off,
    };
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
    if matches!(state, HeroState::Hit | HeroState::Dead) {
        if outcome.ground.is_some() && hero.velocity.y <= 0.0 {
            hero.velocity.y = 0.0;
            hero.ground = outcome.ground;
        } else {
            hero.ground = None;
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
        land(hero, ground, stick);
        return;
    }
    hero.ground = None;
    hero.height_above_ground = height_above_ground(world, hero.position);

    if matches!(
        state,
        HeroState::Jump(JumpKind::Standing | JumpKind::Run | JumpKind::Wall)
    ) && let Some(wall) = detect_wall(hero, profile, world)
    {
        hero.wall = Some(wall);
        hero.wall_ticks =
            profile.wall_jump.window_ticks + u32::from(state == HeroState::Jump(JumpKind::Wall));
    }
    try_grab(hero, profile, world);
}

fn land(hero: &mut HeroMotion, ground: Ground, stick: &Stick) {
    let horizontal = hero.horizontal_velocity();
    let landing_from_long_jump = hero.state == HeroState::Jump(JumpKind::Long);
    hero.ground = Some(ground);
    hero.velocity.y = 0.0;
    hero.speed = horizontal.length();
    hero.stop_ticks = 0;
    hero.wall = None;
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

    let jump_buffer = if hero.state == HeroState::Walk {
        walk.jump_buffer_walk
    } else {
        walk.jump_buffer_idle
    };
    if hero.state != HeroState::Crouch && hero.jump_press.within(jump_buffer) {
        hero.jump_press.used = true;
        // While strafing, a jump to the side or back is a flip; forwards it is a jump.
        if input.held(Action::Strafe)
            && stick.is_active()
            && stick.vector.y < 0.7 * stick.vector.length()
        {
            start_flip(hero, wish_direction, stick, profile);
        } else {
            let kind = if stick.magnitude > walk.run_jump_stick {
                JumpKind::Run
            } else {
                JumpKind::Standing
            };
            start_jump(hero, kind, profile);
        }
        air_tick(hero, input, stick, wish, profile);
        return;
    }

    if input.held(Action::Crouch) || hero.state == HeroState::Crouch {
        let long = &profile.long_jump;
        // A jump from a crouch waits until the crouch is old enough; the press is kept.
        if hero.state == HeroState::Crouch
            && hero.ticks >= long.crouch_ticks
            && hero.jump_press.within(long.press_window)
        {
            hero.jump_press.used = true;
            let kind = if stick.magnitude > long.min_stick && hero.speed > long.min_speed {
                JumpKind::Long
            } else {
                JumpKind::Heli
            };
            start_jump(hero, kind, profile);
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
    if !matches!(kind, JumpKind::Flip | JumpKind::Wall | JumpKind::Ledge) {
        let facing = hero.facing();
        hero.set_horizontal_velocity(facing * hero.speed);
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

fn start_flip(hero: &mut HeroMotion, direction: Vec3, stick: &Stick, profile: &HeroProfile) {
    start_jump(hero, JumpKind::Flip, profile);
    let flip = &profile.flip;
    hero.flip_direction = direction;
    hero.flip_speed = flip.start_speed;
    hero.flip_target = (flip.speed * stick.vector.length().min(1.0)).max(flip.min_speed);
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
                    hero.let_go(ledge.no_grab_ticks);
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
            let next = if towards(side) && ledge_to(side, ledge.shimmy_probe).is_some() {
                ledge_to(side, ledge.shimmy_speed * TICK)
            } else {
                None
            };
            match next {
                Some(next) => {
                    hero.ledge = Some(next);
                    hero.velocity = (next.hang - hero.position) / TICK;
                }
                None => {
                    hero.velocity = Vec3::ZERO;
                    hero.set_state(HeroState::LedgeHang);
                }
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
    if !on_ground && hero.ticks > 12 && hero.height_above_ground > 1.0 {
        // Thrown off something high: an ordinary fall from here.
        hero.take_off_height = hero.position.y;
        hero.double_jumped = true;
        hero.set_state(HeroState::Fall);
        return;
    }
    if hero.ticks >= hit.ticks && (on_ground || hero.height_above_ground <= 1.0) {
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
            if let Some(next) = jump_transition(hero, kind, profile) {
                match next {
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
            jump_vertical(hero, &params, input.held(Action::Jump), profile);
            if kind == JumpKind::Heli {
                let heli = &profile.heli_jump;
                if (heli.thrust_from_tick..=heli.thrust_to_tick).contains(&t) {
                    hero.velocity.y += heli.thrust * TICK;
                }
                let glide = &profile.glide;
                if input.held(Action::Jump)
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
                JumpKind::Flip => {
                    hero.flip_speed =
                        approach(hero.flip_speed, hero.flip_target, profile.flip.accel * TICK);
                    hero.set_horizontal_velocity(hero.flip_direction * hero.flip_speed);
                }
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
                    hero.climb_speed =
                        if t < ledge.climb_forward_from_tick || t > ledge.climb_forward_stop_tick {
                            0.0
                        } else if t <= ledge.climb_forward_until_tick {
                            approach(hero.climb_speed, ledge.climb_forward_speed, step)
                        } else {
                            approach(hero.climb_speed, 0.0, step)
                        };
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

enum Transition {
    Wall(Wall),
    Long,
    Double,
}

/// What a jump turns into this tick, in the order the games check (`hero-controller.md`,
/// "Wall jump", "Long jump", "Double jump").
fn jump_transition(
    hero: &mut HeroMotion,
    kind: JumpKind,
    profile: &HeroProfile,
) -> Option<Transition> {
    let t = hero.ticks;
    let long = &profile.long_jump;
    if matches!(kind, JumpKind::Standing | JumpKind::Run)
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
    if !hero.double_jumped && t > double.after_tick && hero.jump_press.within(window) && height_ok {
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
    if input.held(Action::Jump)
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
