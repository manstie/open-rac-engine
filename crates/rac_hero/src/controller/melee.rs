//! The wrench (`hero-controller.md`, "Melee"): the three-swing combo (state 19), the jump
//! attack (20), the throw (21), the thrown wrench, the grind attack (42) and the clank off
//! walls.
//!
//! The games time their attacks by the hero's animation, so each attack here keeps the
//! animation's frame as the game would play it, and the profile's windows are in those
//! frames. What the wrench can hit each tick is left in [`HeroMotion::strikes`] for the
//! game's combat code to test against its targets.

use bevy::{math::ops, prelude::*};
use rac_input::{Action, PlayerInput};
use std::f32::consts::FRAC_PI_2;

use super::{
    HeroMotion, HeroState, JumpKind, StrafeSide, TICK, air_tick, approach, facing, flat_length,
    ground_tick, start_jump, turn, wrap, yaw_of,
};
use crate::{
    collision::CollisionWorld,
    gun::{GunHand, Hold, WhipHand},
    profile::{AimProfile, AttackProfile, HeroProfile, Push, TurnSpring},
    stick::Stick,
};

/// One way the wrench can hit something this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    /// Each attack and each throw has its own number, from 1 up, so a target is hit at most
    /// once by it.
    pub serial: u32,
    pub damage: f32,
    /// The push the game hands the target: a direction scaled by the attack's strength.
    pub push: Vec3,
    pub shape: StrikeShape,
    /// The gun in his hand struck, not the wrench: the Plasma Whip's rope.
    pub gun: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrikeShape {
    /// A swing: the wrench's line, hand to tip, swept from where it was last tick to where
    /// it is now, and a ball round its head. The game tests the sweep as five lines in
    /// between, ending with this tick's.
    Sweep {
        from: (Vec3, Vec3),
        to: (Vec3, Vec3),
        head_radius: f32,
    },
    Ball {
        centre: Vec3,
        radius: f32,
    },
}

impl StrikeShape {
    /// How far `point` is inside the shape; positive when a ball of that `radius` round it
    /// touches.
    pub fn touches(&self, point: Vec3, radius: f32) -> bool {
        match *self {
            StrikeShape::Sweep {
                from,
                to,
                head_radius,
            } => {
                let swept = (1..=SWEEP_LINES).any(|i| {
                    let s = i as f32 / SWEEP_LINES as f32;
                    let hand = from.0.lerp(to.0, s);
                    let tip = from.1.lerp(to.1, s);
                    distance_to_segment(point, hand, tip) <= radius
                });
                let head = to.1 - (to.1 - to.0).normalize_or_zero() * HEAD_INSET;
                swept || point.distance(head) <= radius + head_radius
            }
            StrikeShape::Ball { centre, radius: r } => point.distance(centre) <= radius + r,
        }
    }
}

/// The game tests a swing's sweep as this many lines.
const SWEEP_LINES: u32 = 5;
/// Its head ball sits this far in from the tip.
const HEAD_INSET: f32 = 0.085;

fn distance_to_segment(point: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_squared();
    let s = if length_squared > 0.0 {
        ((point - a).dot(ab) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    point.distance(a + ab * s)
}

/// The wrench clanking off a wall or the ground: where, and the surface's normal there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clank {
    pub point: Vec3,
    pub normal: Vec3,
}

/// The wrench while it is out of the hero's hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrownWrench {
    pub position: Vec3,
    /// On its way back to the hand.
    pub returning: bool,
    /// Which way it was thrown.
    pub direction: Vec3,
    /// It is the Plasma Whip's end, which the rope follows.
    pub whip: bool,
    /// u/s, and how fast that changes, u/s².
    speed: f32,
    accel: f32,
    serial: u32,
}

/// The attack under way, or the last one: the next combo swing depends on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Attack {
    /// The combo swing, 0 to 2, or `JUMP_ATTACK` or `THROW` (the game's table rows).
    row: usize,
    /// The animation's frame, its speed, and how many ticks it blends in.
    frame: f32,
    rate: f32,
    blend: u32,
    serial: u32,
    /// The direction the attack goes: set by the stick (`locked`) or towards `target`.
    aim_yaw: f32,
    locked: bool,
    target: Option<Vec3>,
    /// A combo swing lunges harder to reach a target.
    lunge_scale: f32,
    /// The jump attack is on its way down.
    descending: bool,
    /// The thrown wrench is nearly back: the catch animation starts next tick.
    catch_due: bool,
    /// The wrench's line last tick, hand to tip.
    last_line: Option<(Vec3, Vec3)>,
    /// It has clanked off a wall, which it does once.
    clanked: bool,
    /// The Plasma Whip started it.
    whip: bool,
}

const JUMP_ATTACK: usize = 4;
const THROW: usize = 3;
/// The grind attack's two swings follow.
const GRIND: usize = 5;

impl Default for Attack {
    fn default() -> Self {
        Self {
            row: 0,
            frame: 0.0,
            rate: 1.0,
            blend: 1,
            serial: 0,
            aim_yaw: 0.0,
            locked: false,
            target: None,
            lunge_scale: 1.0,
            descending: false,
            catch_due: false,
            last_line: None,
            clanked: false,
            whip: false,
        }
    }
}

impl Attack {
    fn blending(&self, ticks: u32) -> bool {
        ticks < self.blend
    }
}

impl HeroMotion {
    /// The animation frame of the attack under way, or of the last one.
    pub fn attack_frame(&self) -> f32 {
        self.attack.frame
    }

    /// Which way the attack under way is going, if it has an aim.
    pub fn attack_aim(&self) -> Option<f32> {
        (self.attack.locked || self.attack.target.is_some()).then_some(self.attack.aim_yaw)
    }

    /// The attack under way is the Plasma Whip's.
    pub fn whip_attack(&self) -> bool {
        self.attack.whip
            && matches!(
                self.state,
                HeroState::Combo(_) | HeroState::JumpAttack | HeroState::Throw
            )
    }

    /// Where the wrench is during a swing or jump attack, hand to tip, in the world.
    pub fn wrench_line(&self) -> Option<(Vec3, Vec3)> {
        swings(self.state)
            .then_some(self.attack.last_line)
            .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    Combo,
    JumpAttack,
    Throw,
}

fn row(profile: &HeroProfile, row: usize) -> &AttackProfile {
    let melee = &profile.melee;
    match row {
        JUMP_ATTACK => &melee.jump_attack,
        THROW => &melee.throw,
        GRIND.. => &melee.grind.swings[row - GRIND],
        step => &melee.combo[step],
    }
}

/// The states whose wrench swings through the air.
fn swings(state: HeroState) -> bool {
    matches!(
        state,
        HeroState::Combo(_) | HeroState::JumpAttack | HeroState::GrindAttack(_)
    )
}

/// Starts an attack when melee was pressed in time, from the states that allow it
/// (UYA's attack check at the top of the transitions, and the crouch's throw).
pub(super) fn try_attack(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let melee = &profile.melee;
    hero.wrench_coming = hero.wrench_coming.saturating_sub(1);
    if hero.thrown.is_some() || hero.input_lock > 0 {
        return;
    }
    let crouch = input.held(Action::Crouch);
    // With the Plasma Whip in hand and a round for it, the fire button starts the same
    // attacks (`weapons.md`, "Plasma Whip"); the newer of the two presses counts.
    let whip = hero.gun.whip.is_some_and(|whip| whip.loaded)
        && !hero.fire_press.used
        && (hero.melee_press.used || hero.fire_press.ticks <= hero.melee_press.ticks);
    let press = if whip {
        hero.fire_press
    } else {
        hero.melee_press
    };
    // Square brings out the wrench (`weapons.md`, "Switching and firing") whether or not
    // an attack can start yet, and the attack waits until it is in his hand.
    if !whip && hero.gun.out && input.pressed(Action::Melee) {
        hero.gun = GunHand::default();
        hero.wrench_coming = melee.wrench_draw_ticks;
    }
    if hero.wrench_coming > 0 {
        return;
    }
    let t = hero.ticks;
    let next = match hero.state {
        HeroState::Idle | HeroState::Walk | HeroState::Skid => press
            .within(melee.ground_press_window)
            .then_some(if crouch { Next::Throw } else { Next::Combo }),
        HeroState::Crouch => press
            .within(melee.crouch_press_window)
            .then_some(Next::Throw),
        HeroState::Jump(
            JumpKind::Standing
            | JumpKind::Run
            | JumpKind::Long
            | JumpKind::Heli
            | JumpKind::Flip
            | JumpKind::Wall,
        )
        | HeroState::DoubleJump
        | HeroState::Glide
        | HeroState::Fall => {
            let late = matches!(
                hero.state,
                HeroState::DoubleJump | HeroState::Jump(JumpKind::Flip)
            );
            let after = match hero.state {
                HeroState::DoubleJump => melee.double_jump_after_tick,
                HeroState::Jump(JumpKind::Flip) if hero.flip_side == StrafeSide::Back => {
                    melee.back_flip_after_tick
                }
                HeroState::Jump(JumpKind::Flip) => melee.flip_after_tick,
                _ => melee.air_after_tick,
            };
            let window = if late {
                melee.air_press_window_late
            } else {
                melee.air_press_window
            };
            (t > after.max(melee.air_after_tick) && press.within(window)).then_some(
                if hero.height_above_ground <= melee.combo_below_height {
                    Next::Combo
                } else {
                    Next::JumpAttack
                },
            )
        }
        HeroState::Combo(_) | HeroState::JumpAttack | HeroState::Throw => {
            let attack = hero.attack;
            let current = row(profile, attack.row);
            let blending = attack.blending(t);
            let thrown = profile.melee.thrown.again_after_tick;
            if melee.chain_throw
                && hero.state == HeroState::Throw
                && t > thrown
                && !blending
                && attack.frame >= current.chain_from
                && crouch
                && press.within(melee.crouch_press_window)
            {
                Some(Next::Throw)
            } else if blending || attack.frame < current.chain_from {
                None
            } else {
                let window =
                    press_window(attack.frame, current.buffer_from).clamp(1, melee.chain_press_max);
                press
                    .within(window)
                    .then_some(if crouch && melee.chain_throw {
                        Next::Throw
                    } else {
                        Next::Combo
                    })
            }
        }
        _ => None,
    };
    if let Some(next) = next {
        start_attack(hero, next, whip, stick, wish, profile, world);
    }
}

/// Ticks of presses an attack counts, from its `buffer_from` frame to `frame`: two a
/// frame, from the frame's whole part as the game does.
fn press_window(frame: f32, buffer_from: f32) -> u32 {
    ((frame.trunc() - buffer_from) * 2.0).max(0.0) as u32
}

fn start_attack(
    hero: &mut HeroMotion,
    next: Next,
    whip: bool,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let melee = &profile.melee;
    if whip {
        hero.fire_press.used = true;
        hero.whip_attacks = hero.whip_attacks.wrapping_add(1);
    } else {
        hero.melee_press.used = true;
    }
    let leaving = hero.state;
    let last = hero.attack;
    let (row_index, state) = match next {
        Next::Combo => {
            // The next swing when this one comes straight from the last, early enough, or
            // when the combo stopped only a moment ago; otherwise the first.
            let previous_step = match leaving {
                HeroState::Combo(step) => {
                    (last.frame < melee.combo[usize::from(step)].chain_before).then_some(step)
                }
                _ => match hero.previous_state {
                    HeroState::Combo(step) if hero.ticks < melee.chain_memory_ticks => Some(step),
                    _ => None,
                },
            };
            let step = previous_step.map_or(0, |step| (usize::from(step) + 1) % 3);
            (step, HeroState::Combo(step as u8))
        }
        Next::JumpAttack => (JUMP_ATTACK, HeroState::JumpAttack),
        Next::Throw => (THROW, HeroState::Throw),
    };
    let current = row(profile, row_index);
    // Square brings out the wrench (`weapons.md`, "Switching and firing"); the whip's
    // attacks keep it in hand.
    if !whip {
        hero.gun = GunHand::default();
    }
    hero.attack_serial += 1;
    hero.attack = Attack {
        row: row_index,
        frame: current.start_frame,
        rate: 1.0,
        blend: current.blend_ticks,
        serial: hero.attack_serial,
        aim_yaw: hero.yaw,
        whip,
        ..default()
    };
    let aim_profile = match next {
        Next::Combo => melee.combo_aim,
        Next::JumpAttack => melee.jump_aim,
        Next::Throw => melee.throw_aim,
    };
    aim(hero, stick, wish, &aim_profile, melee.lock_stick, world);
    match next {
        Next::Combo => {
            // Turning to the stick's direction first takes longer to blend in.
            if hero.attack.locked && row_index < 2 {
                let angle = wrap(hero.attack.aim_yaw - hero.yaw).abs();
                hero.attack.blend += (angle * 1.5) as u32;
            }
        }
        Next::JumpAttack => {
            let jump = &melee.jump;
            if leaving == HeroState::Jump(JumpKind::Flip) && hero.ticks < jump.after_flip_ticks {
                hero.attack.blend = jump.blend_after_flip;
            }
        }
        Next::Throw => {
            if leaving == HeroState::Throw {
                hero.attack.blend = melee.thrown.blend_again;
            }
        }
    }
    hero.speed = flat_length(hero.velocity);
    if !melee.blend_creeps {
        straight_from_start(hero);
    }
    hero.restart_state(state);
}

/// DL's attacks don't wait for their animation to blend in: the frame runs from the start
/// frame. (Our DL traces show it half a frame on, as their rows are read part way through
/// the hero's update: `hero-controller.md`, "Deadlocked's melee".)
fn straight_from_start(hero: &mut HeroMotion) {
    hero.attack.blend = 0;
}

/// A glove that has just fired takes him into its throw (0x500F98).
pub(super) fn try_glove_throw(
    hero: &mut HeroMotion,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let throw = &profile.gun.glove_throw;
    let gun = &hero.gun;
    let fired = gun.firing
        && gun.item.is_some()
        && gun.pose.hold == Hold::None
        // The game's guns fire after its hero moves, so he sees it on the next tick.
        && (1..throw.within_ticks).contains(&gun.firing_ticks);
    if !fired
        || stick.magnitude >= throw.stick_below
        || !matches!(
            hero.state,
            HeroState::Idle | HeroState::Walk | HeroState::Skid
        )
    {
        return;
    }
    hero.attack = Attack {
        aim_yaw: hero.yaw,
        ..default()
    };
    aim(
        hero,
        stick,
        wish,
        &throw.aim,
        profile.melee.lock_stick,
        world,
    );
    hero.speed = flat_length(hero.velocity);
    hero.restart_state(HeroState::GloveThrow);
}

/// One tick of a glove's throw (0x4FAA68's state 34): he brakes to a stop, turning to its
/// aim; it ends on its tick count, and may jump out just before.
pub(super) fn glove_throw_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let throw = &profile.gun.glove_throw;
    if hero.ticks >= throw.jump_from && hero.jump_press.within(throw.jump_window) {
        hero.jump_press.used = true;
        start_jump(hero, JumpKind::Standing, profile);
        air_tick(hero, input, stick, wish, profile);
        return;
    }
    if hero.ticks >= throw.ticks {
        if hero.ground.is_none() {
            hero.take_off_height = hero.position.y;
            hero.double_jumped = true;
            hero.set_state(HeroState::Fall);
            air_tick(hero, input, stick, wish, profile);
        } else {
            hero.setting_off = hero.speed < profile.walk.setting_off_below;
            hero.set_state(HeroState::Idle);
            ground_tick(hero, input, stick, wish, profile);
        }
        return;
    }
    aim_or_turn(
        hero, true, stick, wish, &throw.aim, throw.turn, profile, world,
    );
    let rate = if hero.speed < 0.0 {
        throw.rise
    } else {
        throw.brake
    };
    hero.speed = approach(hero.speed, 0.0, rate * TICK);
    let forward = hero.facing();
    hero.set_horizontal_velocity(forward * hero.speed);
    fall_off_ground(hero, throw.gravity, profile);
}

/// Whether the grind attack under way may jump off the rail: from its swing's `jump_from`
/// frame.
pub(super) fn grind_attack_may_jump(hero: &HeroMotion, profile: &HeroProfile) -> bool {
    hero.attack.frame >= row(profile, hero.attack.row).jump_from
}

/// On a rail a melee press swings the wrench, and the swing ends back in the grind (UYA's
/// transitions for states 39 and 42). A press past a swing's `chain_from` frame starts the
/// other swing; one before it waits, and holds the swing from ending meanwhile.
pub(super) fn grind_attack_transitions(hero: &mut HeroMotion, profile: &HeroProfile) {
    let pressed = hero.melee_press.within(profile.melee.grind.press_window);
    match hero.state {
        HeroState::Grind if pressed => start_grind_attack(hero, 0, profile),
        HeroState::GrindAttack(step) => {
            let current = row(profile, hero.attack.row);
            if pressed {
                if hero.attack.frame > current.chain_from {
                    start_grind_attack(hero, (step + 1) % 2, profile);
                }
            } else if hero.attack.frame > current.end {
                hero.set_state(HeroState::Grind);
            }
        }
        _ => {}
    }
}

/// Starts grind swing `step`: the first from the grind, the other straight after a swing.
/// It has no aim; the rail takes him on.
fn start_grind_attack(hero: &mut HeroMotion, step: u8, profile: &HeroProfile) {
    hero.melee_press.used = true;
    let row_index = GRIND + usize::from(step);
    let current = row(profile, row_index);
    hero.gun = GunHand::default();
    hero.attack_serial += 1;
    hero.attack = Attack {
        row: row_index,
        frame: current.start_frame,
        rate: 1.0,
        blend: current.blend_ticks,
        serial: hero.attack_serial,
        aim_yaw: hero.yaw,
        ..default()
    };
    if !profile.melee.blend_creeps {
        straight_from_start(hero);
    }
    hero.restart_state(HeroState::GrindAttack(step));
}

/// Picks a target for the attack under way, and takes the stick's direction when it is
/// pushed past `lock_stick` (UYA's 0x4EEA78): once either is set, the attack keeps it.
fn aim(
    hero: &mut HeroMotion,
    stick: &Stick,
    wish: Vec3,
    aim: &AimProfile,
    lock_stick: f32,
    world: &dyn CollisionWorld,
) {
    if hero.attack.locked {
        return;
    }
    let mut search_yaw = hero.yaw;
    if stick.magnitude > lock_stick && wish != Vec3::ZERO {
        search_yaw = yaw_of(wish);
        hero.attack.locked = true;
        hero.attack.aim_yaw = search_yaw;
        hero.attack.target = None;
    }
    let mut origin = hero.position;
    if hero.state == HeroState::JumpAttack {
        // The jump attack measures from the ground under him.
        origin.y -= hero.height_above_ground;
    }
    if let Some(target) = best_target(world.targets(), origin, search_yaw, aim) {
        hero.attack.target = Some(target);
        hero.attack.aim_yaw = yaw_of(target - hero.position);
    }
}

/// The target with the least distance × (1 + angle off `yaw`) within the aim's range and
/// cone, as UYA scores them (0x4E56A8). Ties go to the first.
pub fn best_target(targets: &[Vec3], from: Vec3, yaw: f32, aim: &AimProfile) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for &target in targets {
        if let Some(score) = target_score(target - from, yaw, aim)
            && best.is_none_or(|(least, _)| score < least)
        {
            best = Some((score, target));
        }
    }
    best.map(|(_, target)| target)
}

/// UYA's score for a target `to` away, lower being better, or `None` outside the aim's
/// range, cone and rise.
pub fn target_score(to: Vec3, yaw: f32, aim: &AimProfile) -> Option<f32> {
    let cone = aim.cone_degrees.to_radians();
    let rise = aim.rise_degrees.to_radians();
    let distance = to.length();
    if distance > aim.range {
        return None;
    }
    let off = wrap(yaw_of(to) - yaw).abs();
    if cone > 0.0 && off > cone {
        return None;
    }
    let up = ops::atan2(to.y, flat_length(to)).abs();
    if rise > 0.0 && up > rise {
        return None;
    }
    Some(distance + off * distance)
}

/// One tick of an attack: its animation, how it ends, and how it moves.
pub(super) fn attack_tick(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    if hero.ticks == 0 {
        // The game enters the state at the end of a frame, after the last state has moved
        // him, so the attack's own motion starts on its next tick.
        return;
    }
    if leave_attack(hero, input, stick, wish, profile) {
        return;
    }
    match hero.state {
        HeroState::Combo(step) => {
            combo_motion(hero, usize::from(step), stick, wish, profile, world)
        }
        HeroState::JumpAttack => jump_attack_motion(hero, stick, wish, profile, world),
        HeroState::Throw => throw_motion(hero, stick, wish, profile, world),
        _ => {}
    }
}

/// The animation frame for this tick: it blends in from `start_frame`, then plays at
/// its rate from there. It runs before anything else in an attack's tick, as the game's
/// transitions see the frame its animation reaches that frame.
pub(super) fn advance_frame(hero: &mut HeroMotion, profile: &HeroProfile) {
    let melee = &profile.melee;
    let t = hero.ticks;
    let start = row(profile, hero.attack.row).start_frame;
    let attack = &mut hero.attack;
    if attack.catch_due {
        attack.catch_due = false;
        attack.frame = attack.frame.max(melee.thrown.catch_frame);
        return;
    }
    attack.frame = if t == 0 || t == attack.blend {
        start
    } else if t < attack.blend {
        start + t as f32 / attack.blend as f32
    } else {
        attack.frame + melee.frames_per_tick * attack.rate
    };
    if hero.state == HeroState::Throw {
        let thrown = &melee.thrown;
        if let Some(hold) = thrown.hold_frame
            && hero.thrown.is_some()
            && attack.frame < thrown.catch_frame
        {
            attack.frame = attack.frame.min(hold);
        }
        let late = thrown.catch_after_tick.is_some_and(|after| t >= after);
        if late && attack.frame < thrown.catch_frame {
            attack.frame = thrown.catch_frame;
        }
        if let Some(back) = thrown.catch_back
            && hero.thrown.is_none()
            && t >= back.after_tick
            && attack.frame < back.before_frame
        {
            attack.frame = thrown.catch_frame;
        }
    }
}

/// Jumps out, walks off or ends the attack, as the transitions allow this tick. Returns
/// whether it did, having moved him in the new state.
fn leave_attack(
    hero: &mut HeroMotion,
    input: &PlayerInput,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
) -> bool {
    let melee = &profile.melee;
    let attack = hero.attack;
    let current = row(profile, attack.row);
    let t = hero.ticks;
    let blending = attack.blending(t);
    let frame = attack.frame;

    let jump_window = if hero.state == HeroState::Throw {
        melee.thrown.jump_out_window
    } else {
        press_window(frame, current.buffer_from)
    };
    if !blending && frame > current.jump_from && hero.jump_press.within(jump_window) {
        hero.jump_press.used = true;
        let kind = if melee.crouch_jump_out && input.held(Action::Crouch) {
            // A jump from a crouch.
            let long = &profile.long_jump;
            if !hero.abilities.heli_pack {
                JumpKind::Standing
            } else if stick.magnitude > long.min_stick && hero.speed > long.min_speed {
                JumpKind::Long
            } else {
                JumpKind::Heli
            }
        } else {
            JumpKind::Standing
        };
        start_jump(hero, kind, profile);
        air_tick(hero, input, stick, wish, profile);
        return true;
    }
    if blending {
        return false;
    }
    let next = match hero.state {
        HeroState::JumpAttack => (frame > current.end).then_some(HeroState::Idle),
        _ if frame > melee.walk_out_frame && stick.magnitude > melee.walk_out_stick => {
            Some(HeroState::Walk)
        }
        _ if frame > current.end && stick.magnitude > melee.end_walk_stick => Some(HeroState::Walk),
        _ if frame > current.end + melee.end_idle_frames => Some(HeroState::Idle),
        _ => None,
    };
    let Some(next) = next else {
        return false;
    };
    if hero.ground.is_none() {
        // Over in the air: falling.
        hero.take_off_height = hero.position.y;
        hero.double_jumped = true;
        hero.set_state(HeroState::Fall);
        air_tick(hero, input, stick, wish, profile);
        return true;
    }
    hero.setting_off = hero.speed < profile.walk.setting_off_below;
    hero.set_state(next);
    ground_tick(hero, input, stick, wish, profile);
    true
}

/// Turns towards the attack's aim, or looks for one while it has none and `looking`.
fn aim_or_turn(
    hero: &mut HeroMotion,
    looking: bool,
    stick: &Stick,
    wish: Vec3,
    aim_profile: &AimProfile,
    spring: TurnSpring,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let attack = hero.attack;
    if !attack.locked && attack.target.is_none() {
        if looking {
            aim(
                hero,
                stick,
                wish,
                aim_profile,
                profile.melee.lock_stick,
                world,
            );
        }
    } else {
        turn(hero, attack.aim_yaw, spring);
    }
}

/// A combo swing's motion (UYA's state 19 handler): a lunge timed by the animation,
/// turning to the aim, and stopping short of edges.
fn combo_motion(
    hero: &mut HeroMotion,
    step: usize,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let melee = &profile.melee;
    let current = &melee.combo[step];
    let t = hero.ticks;
    let attack = hero.attack;
    let blending = attack.blending(t);

    // Reaching for a target: a harder lunge the further it is, set in the first ticks.
    if let (Some(target), Some(reach)) = (attack.target, current.reach)
        && t <= 3
    {
        let distance = (flat_length(target - hero.position) - 0.5).max(0.0);
        if (distance - reach.offset).abs() < reach.range + 3.0 {
            let scale = ((distance - reach.offset) / 0.9).min(reach.max_scale);
            if scale > 1.0 {
                hero.attack.lunge_scale = scale;
            }
        }
    }
    let lunging = blending
        || current
            .lunge
            .last()
            .is_some_and(|&(end, _)| attack.frame < end);
    let target_speed = if lunging {
        current
            .lunge
            .iter()
            .find(|&&(upto, _)| attack.frame <= upto)
            .or(current.lunge.last())
            .map_or(0.0, |&(_, speed)| speed * hero.attack.lunge_scale)
    } else {
        0.0
    };

    // The second swing stops looking for a target once it is under way.
    let looking = !(step == 1 && !blending && attack.frame > 7.0);
    aim_or_turn(
        hero,
        looking,
        stick,
        wish,
        &melee.combo_aim,
        melee.turn,
        profile,
        world,
    );

    let rate = if hero.speed < target_speed {
        melee.lunge_accel
    } else {
        melee.lunge_brake
    };
    hero.speed = approach(hero.speed, target_speed, rate * TICK);
    let direction = if hero.attack.locked {
        facing(hero.attack.aim_yaw)
    } else {
        hero.facing()
    };
    hero.set_horizontal_velocity(direction * hero.speed);
    fall_off_ground(hero, melee.air_gravity, profile);

    // Stop rather than lunge off an edge.
    if hero.ground.is_some() && hero.speed > 0.0 {
        let ahead = hero.position
            + hero.horizontal_velocity() * TICK * melee.edge_lookahead_ticks
            + Vec3::Y * melee.edge_probe_above;
        let span = melee.edge_probe_above + melee.edge_probe_below;
        let steepest = ops::cos(profile.body.slope_limit_degrees.to_radians());
        let ground_ahead = world
            .cast_ray(ahead, Vec3::NEG_Y * span)
            .is_some_and(|hit| hit.normal.y >= steepest);
        if !ground_ahead {
            hero.speed = 0.0;
            hero.set_horizontal_velocity(Vec3::ZERO);
        }
    }
}

/// The jump attack's motion (UYA's state 20 handler): up, a hold, then down hard, braking
/// all the while unless high up or closing on a target.
fn jump_attack_motion(
    hero: &mut HeroMotion,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let melee = &profile.melee;
    let jump = &melee.jump;
    let t = hero.ticks;
    let high = hero.height_above_ground > jump.carry_above;
    let horizontal = hero.horizontal_velocity();
    let current = horizontal.length();
    let attack = hero.attack;
    if let Some(target) = attack.target {
        let distance = (target - hero.position).length();
        let target_speed = if (distance > jump.target_near && distance < jump.target_far) || high {
            jump.carry_speed
        } else {
            0.0
        };
        let speed = approach(current, target_speed, jump.target_accel * TICK);
        let facing = hero.facing();
        hero.set_horizontal_velocity(facing * speed);
    } else {
        let target_speed = if high && t < jump.carry_until_tick {
            jump.carry_speed
        } else {
            0.0
        };
        let speed = approach(current, target_speed, jump.brake * TICK);
        let direction = if current > 0.0 {
            horizontal / current
        } else {
            hero.facing()
        };
        hero.set_horizontal_velocity(direction * speed);
    }
    hero.speed = flat_length(hero.velocity);

    let looking = attack.frame < jump.aim_until_frame;
    aim_or_turn(
        hero,
        looking,
        stick,
        wish,
        &melee.jump_aim,
        melee.turn,
        profile,
        world,
    );

    let height = hero.height_above_ground;
    let rise = if height > jump.higher_above {
        jump.rise_higher
    } else if height > jump.high_above {
        jump.rise_high
    } else {
        jump.rise
    };
    if t <= jump.rise_until_tick {
        hero.velocity.y = approach(hero.velocity.y, rise, jump.rise_accel * TICK);
    } else if t > jump.hold_until_tick {
        hero.velocity.y = (hero.velocity.y - jump.gravity * TICK).max(-jump.fall_cap);
        if hero.velocity.y < 0.0 {
            hero.attack.descending = true;
        }
    }

    // The animation: slow going up, then timed so the strike lands with him.
    let frame = hero.attack.frame;
    let far_up = hero.ground.is_none()
        && jump.hover_ticks > 0
        && ticks_to_land(height, hero.velocity.y, jump.gravity) >= jump.hover_ticks;
    hero.attack.rate = if far_up {
        jump.hover_rate
    } else if !hero.attack.descending {
        jump.rise_rate
    } else if hero.ground.is_some() {
        if frame < jump.strike_frame {
            jump.landed_rate
        } else {
            1.0
        }
    } else {
        let ticks = ticks_to_land(height, hero.velocity.y, jump.gravity);
        if ticks == 0 || jump.strike_frame - frame <= 1.0 {
            1.0
        } else {
            let rate = (jump.strike_frame - frame) / (ticks as f32 * melee.frames_per_tick);
            if height < jump.low_rate_below {
                rate.max(jump.low_rate_min)
            } else {
                rate
            }
        }
    };
}

/// Whole ticks until a fall from `height` at `vertical` u/s under `gravity` reaches the
/// ground, at most 60 (UYA's 0x50C4A0).
fn ticks_to_land(height: f32, vertical: f32, gravity: f32) -> u32 {
    let v = vertical * TICK;
    let g = gravity * TICK * TICK;
    let ticks = (v + ops::sqrt(v * v + 2.0 * g * height.max(0.0))) / g;
    ticks.clamp(0.0, 60.0) as u32
}

/// The throw's motion (UYA's state 21 handler): stopping, turning to the aim, and letting
/// go of the wrench at its frame.
fn throw_motion(
    hero: &mut HeroMotion,
    stick: &Stick,
    wish: Vec3,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let melee = &profile.melee;
    let t = hero.ticks;
    // It looks for a target only while the animation blends in.
    let blending = hero.attack.blending(t);
    aim_or_turn(
        hero,
        blending,
        stick,
        wish,
        &melee.throw_aim,
        melee.throw_turn,
        profile,
        world,
    );
    let attack = hero.attack;
    let rate = if hero.speed > 0.0 {
        melee.lunge_brake
    } else {
        melee.lunge_accel
    };
    hero.speed = approach(hero.speed, 0.0, rate * TICK);
    let facing = hero.facing();
    hero.set_horizontal_velocity(facing * hero.speed);
    fall_off_ground(hero, melee.air_gravity, profile);

    let thrown = &melee.thrown;
    let step = melee.frames_per_tick * attack.rate;
    let crossed =
        attack.frame > thrown.release_frame && attack.frame - thrown.release_frame <= step;
    if !blending && crossed && hero.thrown.is_none() {
        hero.attack_serial += 1;
        hero.thrown = Some(ThrownWrench {
            position: hero.position + relative(hero, thrown.hand),
            returning: false,
            direction: facing,
            speed: thrown.speed,
            accel: 0.0,
            serial: hero.attack_serial,
            whip: attack.whip,
        });
    }
}

/// Off the ground an attack falls at `gravity`; on it, it stays down.
fn fall_off_ground(hero: &mut HeroMotion, gravity: f32, profile: &HeroProfile) {
    if hero.ground.is_none() {
        hero.velocity.y = (hero.velocity.y - gravity * TICK).max(-profile.air.fall_speed_cap);
    } else {
        hero.velocity.y = 0.0;
    }
}

/// A point given as (forward, left, up) from the hero's feet, relative to them in the
/// world.
fn relative(hero: &HeroMotion, offset: [f32; 3]) -> Vec3 {
    let forward = hero.facing();
    let left = facing(hero.yaw + FRAC_PI_2);
    forward * offset[0] + left * offset[1] + Vec3::Y * offset[2]
}

/// The thrown wrench's flight (UYA's wrench object, 0x338088): out, slowing ever harder,
/// until it stops or hits something; then back to the hand, speeding up ever harder.
pub(super) fn thrown_tick(
    hero: &mut HeroMotion,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) {
    let thrown = &profile.melee.thrown;
    let Some(mut wrench) = hero.thrown else {
        return;
    };
    let hand = hero.position + relative(hero, thrown.hand);
    if wrench.returning {
        wrench.accel += thrown.return_jerk * TICK;
        wrench.speed += wrench.accel * TICK;
        let to_hand = hand - wrench.position;
        let distance = to_hand.length();
        let step = wrench.speed * TICK;
        if distance <= step || distance > thrown.lost_beyond {
            hero.thrown = None;
            return;
        }
        wrench.position += to_hand / distance * step;
        if hero.state == HeroState::Throw
            && hero.attack.frame < thrown.catch_frame
            && distance / step < thrown.catch_ahead_ticks
        {
            hero.attack.catch_due = true;
        }
    } else {
        // It moves, then slows: the first tick out is at full speed, and it turns back in
        // the tick it stops.
        let from = wrench.position;
        let mut to = from + wrench.direction * wrench.speed * TICK;
        // Low over the ground it keeps to a height above it.
        let lift = 0.5;
        if let Some(hit) = world.cast_ray(
            to + Vec3::Y * lift,
            Vec3::NEG_Y * (lift + thrown.height_within),
        ) {
            let ground = to.y + lift - (lift + thrown.height_within) * hit.fraction;
            let wanted = ground + thrown.height;
            let rate = if to.y < wanted {
                thrown.rise
            } else {
                thrown.sink
            };
            to.y = approach(to.y, wanted, rate * TICK);
        }
        if let Some(hit) = world.cast_ray(from, to - from) {
            to = from + (to - from) * hit.fraction;
            wrench.returning = true;
            wrench.speed *= thrown.wall_slow;
        }
        wrench.position = to;
        wrench.accel += thrown.jerk * TICK;
        wrench.speed = approach(wrench.speed, 0.0, wrench.accel * TICK);
        if wrench.speed <= 0.0 {
            wrench.returning = true;
        }
    }
    hero.thrown = Some(wrench);
}

/// Works out what the wrench can hit this tick, after the hero has moved.
pub(super) fn update_strikes(hero: &mut HeroMotion, profile: &HeroProfile) {
    hero.strikes.clear();
    let melee = &profile.melee;
    let whip = hero.gun.whip.filter(|_| hero.attack.whip);
    if swings(hero.state) {
        let current = row(profile, hero.attack.row);
        if let Some((hand, tip)) = swing_line(hero, current) {
            let attack = hero.attack;
            let last = hero.attack.last_line.replace((hand, tip));
            if let Some(whip) = whip {
                whip_strike(hero, &whip, last.unwrap_or((hand, tip)), (hand, tip));
                return;
            }
            let frame = attack.frame;
            let live = !attack.blending(hero.ticks)
                && frame >= current.hit_from
                && frame <= current.hit_to;
            // A combo swing only hits in front of him.
            let out_to = yaw_of(tip - hero.position);
            let in_front = !matches!(hero.state, HeroState::Combo(_))
                || wrap(out_to - hero.yaw).abs() <= FRAC_PI_2;
            if live && in_front {
                let push = match current.push {
                    Push::Left => facing(out_to + FRAC_PI_2),
                    Push::Right => facing(out_to - FRAC_PI_2),
                    Push::Forward => hero.facing(),
                } * current.knockback;
                hero.strikes.push(Strike {
                    serial: attack.serial,
                    damage: current.damage,
                    push,
                    shape: StrikeShape::Sweep {
                        from: last.unwrap_or((hand, tip)),
                        to: (hand, tip),
                        head_radius: current.head_radius,
                    },
                    gun: false,
                });
            }
        }
        // The grind attack's first ticks hit all round him too.
        let grind = &melee.grind;
        if matches!(hero.state, HeroState::GrindAttack(_)) && hero.ticks < grind.ball_ticks {
            hero.strikes.push(Strike {
                serial: hero.attack.serial,
                damage: grind.ball_damage,
                push: facing(hero.rail_yaw + grind.ball_push_degrees.to_radians())
                    * grind.ball_knockback,
                shape: StrikeShape::Ball {
                    centre: hero.position + Vec3::Y * grind.ball_above,
                    radius: grind.ball_radius,
                },
                gun: false,
            });
        }
        let jump = &melee.jump;
        if hero.state == HeroState::JumpAttack
            && !hero.attack.blending(hero.ticks)
            && hero.attack.frame > jump.slam_from_frame
            && hero.velocity.y < 0.0
            && hero.ground.is_none()
        {
            hero.strikes.push(Strike {
                serial: hero.attack.serial,
                damage: jump.slam_damage,
                push: hero.facing() * jump.slam_knockback,
                shape: StrikeShape::Ball {
                    centre: hero.position - Vec3::Y * jump.slam_below,
                    radius: jump.slam_radius,
                },
                gun: false,
            });
        }
    }
    if let Some(wrench) = hero.thrown {
        let thrown = &melee.thrown;
        if wrench.whip {
            // The rope runs from his hand to its end, hitting along its length.
            if let Some(whip) = hero.gun.whip {
                let hand = hero.position + relative(hero, thrown.hand);
                hero.strikes.push(Strike {
                    serial: wrench.serial,
                    damage: whip.damage,
                    push: wrench.direction * whip.push + Vec3::Y * whip.lift,
                    shape: StrikeShape::Sweep {
                        from: (hand, wrench.position),
                        to: (hand, wrench.position),
                        head_radius: 0.0,
                    },
                    gun: true,
                });
            }
            return;
        }
        let push = if wrench.returning {
            -wrench.direction * thrown.knockback_back
        } else {
            wrench.direction * thrown.knockback_out
        };
        hero.strikes.push(Strike {
            serial: wrench.serial,
            damage: thrown.damage,
            push,
            shape: StrikeShape::Ball {
                centre: wrench.position,
                radius: thrown.radius,
            },
            gun: false,
        });
    }
}

/// The Plasma Whip's rope in a swing or the jump attack (0x357F10): from the hand along
/// the wrench's line, its reach long, swept from last tick's; it hits once the attack is
/// a few ticks old, at every frame of its animation.
fn whip_strike(hero: &mut HeroMotion, whip: &WhipHand, last: (Vec3, Vec3), now: (Vec3, Vec3)) {
    let (reach, from) = match hero.state {
        HeroState::JumpAttack => (whip.reach[2], whip.jump_from),
        _ if hero.attack.row == 2 => (whip.reach[1], whip.combo_from),
        _ => (whip.reach[0], whip.combo_from),
    };
    if hero.ticks < from {
        return;
    }
    let rope = |(hand, tip): (Vec3, Vec3)| {
        let along = (tip - hand).normalize_or(hero.facing());
        (hand, hand + along * reach)
    };
    let to = rope(now);
    hero.strikes.push(Strike {
        serial: hero.attack.serial,
        damage: whip.damage,
        push: hero.facing() * whip.push + Vec3::Y * whip.lift,
        shape: StrikeShape::Sweep {
            from: rope(last),
            to,
            head_radius: 0.0,
        },
        gun: true,
    });
}

/// The wrench clanking off a wall it swings into (UYA's wrench object, 0x338088), and the
/// jump attack's off the ground it lands on. Neither stops the swing. Runs after
/// [`update_strikes`], which has placed this tick's wrench.
pub(super) fn clank(hero: &mut HeroMotion, profile: &HeroProfile, world: &dyn CollisionWorld) {
    hero.clank = None;
    let jump_attack = hero.state == HeroState::JumpAttack;
    // The clank is the wrench object's: the whip has none.
    if (!matches!(hero.state, HeroState::Combo(_)) && !jump_attack) || hero.attack.whip {
        return;
    }
    let Some((_, tip)) = hero.attack.last_line else {
        return;
    };
    let clank = &profile.melee.clank;
    // A ray from low on his body towards the tip, shortening as the wrench does when it
    // comes round (0x337FD0).
    let out_yaw = yaw_of(tip - hero.position);
    let off = wrap(out_yaw - hero.yaw).abs();
    let shorten_at = clank.shorten_degrees.to_radians();
    if !hero.attack.clanked
        && hero.ticks > clank.after_tick
        && (jump_attack || off < clank.within_degrees.to_radians())
    {
        let length = clank.reach * (1.0 - clank.shorten * off.min(shorten_at) / shorten_at);
        let from = hero.position + Vec3::Y * clank.above;
        let direction = facing(out_yaw);
        if let Some(hit) = world.cast_ray(from, direction * length) {
            let point = from + direction * length * hit.fraction;
            hero.attack.clanked = true;
            hero.clank = Some(Clank {
                point,
                normal: hit.normal,
            });
            // Whatever is there takes the knock.
            if clank.damage > 0.0 {
                hero.strikes.push(Strike {
                    serial: hero.attack.serial,
                    damage: clank.damage,
                    push: direction * clank.knockback,
                    shape: StrikeShape::Ball {
                        centre: point,
                        radius: 0.0,
                    },
                    gun: false,
                });
            }
        }
    }
    let step = profile.melee.frames_per_tick * hero.attack.rate;
    let frame = hero.attack.frame;
    if jump_attack && frame > clank.ground_frame && frame - step <= clank.ground_frame {
        let from = tip + Vec3::Y * clank.ground_above;
        let span = clank.ground_above + clank.ground_below;
        if let Some(hit) = world.cast_ray(from, Vec3::NEG_Y * span) {
            hero.clank = Some(Clank {
                point: from - Vec3::Y * span * hit.fraction,
                normal: hit.normal,
            });
        }
    }
}

/// Where the wrench's line is at the attack's frame, in the world, from the profile's
/// keys.
fn swing_line(hero: &HeroMotion, current: &AttackProfile) -> Option<(Vec3, Vec3)> {
    let keys = &current.swing;
    let first = keys.first()?;
    let frame = hero.attack.frame;
    let (hand, tip) = match keys.iter().position(|key| key.frame >= frame) {
        None => {
            let last = keys.last()?;
            (last.hand, last.tip)
        }
        Some(0) => (first.hand, first.tip),
        Some(i) => {
            let (a, b) = (keys[i - 1], keys[i]);
            let s = (frame - a.frame) / (b.frame - a.frame);
            let mix = |p: [f32; 3], q: [f32; 3]| {
                [
                    p[0] + (q[0] - p[0]) * s,
                    p[1] + (q[1] - p[1]) * s,
                    p[2] + (q[2] - p[2]) * s,
                ]
            };
            (mix(a.hand, b.hand), mix(a.tip, b.tip))
        }
    };
    Some((
        hero.position + relative(hero, hand),
        hero.position + relative(hero, tip),
    ))
}
