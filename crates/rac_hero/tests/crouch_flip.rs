//! The flip from a crouch (crouch held, the stick pushed to a side or back, and X) against
//! `docs/research/hero-controller.md`, "Flips". The figures come from the trace
//! `uyantsc-veldin-crouchflip-1` on UYA NTSC-U, converted to u/s (per-frame values × 60).
//! The kit's tick n is the game's frame n of the flip, as for the other jumps.

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, HeroMotion, HeroProfile, HeroState, JumpKind, ParryWorld, StrafeSide,
    controller::{TICK, step},
};
use rac_input::{Action, Buttons, PlayerInput};

/// Per-frame figures × 60 compare to about 1e-3 u/s.
const TOLERANCE: f32 = 2e-3;

const LEFT: [i8; 2] = [-127, 0];
const RIGHT: [i8; 2] = [127, 0];
const BACK: [i8; 2] = [0, -127];
const FORWARD: [i8; 2] = [0, 127];

fn buttons(actions: &[Action]) -> Buttons {
    let mut buttons = Buttons::NONE;
    for &action in actions {
        buttons.insert(action);
    }
    buttons
}

fn input(stick: Option<[i8; 2]>, held: &[Action], pressed: &[Action]) -> PlayerInput {
    PlayerInput {
        move_stick: stick.unwrap_or([0, 0]),
        look: [0, 0],
        held: buttons(held) | buttons(pressed),
        pressed: buttons(pressed),
    }
}

fn crouch(stick: Option<[i8; 2]>) -> PlayerInput {
    input(stick, &[Action::Crouch], &[])
}

fn crouch_jump(stick: Option<[i8; 2]>) -> PlayerInput {
    input(stick, &[Action::Crouch], &[Action::Jump])
}

fn block(centre: Vec3, half_extents: Vec3) -> BodyPlacement {
    BodyPlacement::fixed(Collider::Cuboid { half_extents }, centre, Quat::IDENTITY)
}

struct Rig {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
}

impl Rig {
    /// A hero facing −z on an open floor, with the camera behind him.
    fn new() -> Self {
        Self::on(&[block(
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(100.0, 0.5, 100.0),
        )])
    }

    fn on(bodies: &[BodyPlacement]) -> Self {
        let mut rig = Self {
            hero: HeroMotion::new(Vec3::ZERO, 0.0),
            profile: HeroProfile::uya(),
            world: ParryWorld::new(bodies),
        };
        rig.tick(&input(None, &[], &[]));
        rig
    }

    fn tick(&mut self, input: &PlayerInput) {
        step(&mut self.hero, input, 0.0, &self.profile, &self.world);
    }

    fn ticks(&mut self, count: usize, input: &PlayerInput) {
        for _ in 0..count {
            self.tick(input);
        }
    }

    /// Crouches from standing for `ticks`, as long as the trace's crouches.
    fn crouch(&mut self, ticks: usize) {
        self.tick(&input(None, &[Action::Crouch], &[Action::Crouch]));
        self.ticks(ticks - 1, &crouch(None));
        assert_eq!(self.hero.state, HeroState::Crouch);
    }

    /// The trace's take: the stick pushed, then X two ticks later.
    fn flip(&mut self, stick: [i8; 2]) {
        self.ticks(2, &crouch(Some(stick)));
        self.tick(&crouch_jump(Some(stick)));
    }

    fn flipping(&self) -> bool {
        self.hero.state == HeroState::Jump(JumpKind::Flip)
    }

    fn horizontal(&self) -> Vec2 {
        Vec2::new(self.hero.velocity.x, self.hero.velocity.z)
    }
}

#[test]
fn a_crouch_then_the_stick_and_x_flips_to_that_side() {
    for (stick, side) in [
        (LEFT, StrafeSide::Left),
        (RIGHT, StrafeSide::Right),
        (BACK, StrafeSide::Back),
    ] {
        let mut rig = Rig::new();
        rig.crouch(16);
        rig.flip(stick);
        assert!(rig.flipping(), "{stick:?}: {:?}", rig.hero.state);
        assert_eq!(rig.hero.flip_side, side);
    }
    // Forward is no flip: from a crouch that old, X is the heli jump.
    let mut rig = Rig::new();
    rig.crouch(16);
    rig.flip(FORWARD);
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Heli));
}

#[test]
fn a_crouch_flip_needs_no_wait_for_the_crouch() {
    // Take 6 flipped on its crouch's 5th frame; the heli jump would wait for the 7th.
    let mut rig = Rig::new();
    rig.crouch(2);
    rig.flip(LEFT);
    assert!(rig.flipping(), "{:?}", rig.hero.state);
}

#[test]
fn the_stick_held_through_a_crouch_only_turns_him() {
    // Take 5: left held for a second of crouch, then X, gave the heli jump. The stick's
    // press was too long before.
    let mut rig = Rig::new();
    rig.crouch(16);
    rig.ticks(30, &crouch(Some(LEFT)));
    assert!(
        rig.hero.yaw > 0.1,
        "the crouch turns him left: {}",
        rig.hero.yaw
    );
    rig.tick(&crouch_jump(Some(LEFT)));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Heli));
}

#[test]
fn the_jump_and_the_stick_must_come_within_12_ticks() {
    // Pushed from rest, the stick counts as flicked for 4 ticks, as long as it points
    // more than 55° from where it rested in one of the last 4 (take 6's frames 736 to
    // 739), so the gap runs from the last of them.
    for (after_push, flips) in [(14, true), (15, false)] {
        let mut rig = Rig::new();
        rig.crouch(2);
        rig.ticks(after_push, &crouch(Some(LEFT)));
        rig.tick(&crouch_jump(Some(LEFT)));
        assert_eq!(rig.flipping(), flips, "{after_push}: {:?}", rig.hero.state);
    }
}

#[test]
fn x_first_then_the_stick_still_flips() {
    // From a young crouch: X, then the stick two ticks later.
    let mut rig = Rig::new();
    rig.crouch(2);
    rig.tick(&crouch_jump(None));
    rig.tick(&crouch(None));
    rig.tick(&crouch(Some(LEFT)));
    assert!(rig.flipping(), "{:?}", rig.hero.state);

    // Take 4: from an old crouch X is the heli jump at once, and the stick pushed six
    // frames later, while it is still pushing off the ground, turns it into the flip.
    let mut rig = Rig::new();
    rig.crouch(16);
    rig.tick(&crouch_jump(None));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Heli));
    rig.ticks(5, &crouch(None));
    rig.tick(&crouch(Some(LEFT)));
    assert!(rig.flipping(), "{:?}", rig.hero.state);
    assert_eq!(rig.hero.flip_side, StrafeSide::Left);
}

#[test]
fn crouch_the_stick_and_x_together_flip_from_standing() {
    // Take 7: R1, left and X on the same frame, from idle.
    let mut rig = Rig::new();
    rig.tick(&input(
        Some(LEFT),
        &[Action::Crouch],
        &[Action::Crouch, Action::Jump],
    ));
    assert!(rig.flipping(), "{:?}", rig.hero.state);
}

#[test]
fn a_standing_crouch_flip_speeds_up_to_4_2_and_slows_for_the_landing() {
    // Take 1: the flip's speed grows by 0.0022222 u a frame (8 u/s²) from 0 to 0.07 (4.2
    // u/s), and eases back by as much over the last frames before the landing.
    let mut rig = Rig::new();
    rig.crouch(16);
    rig.flip(LEFT);
    let mut speeds = vec![rig.horizontal().length()];
    while rig.flipping() {
        // Moving left, along −x.
        assert!(rig.hero.velocity.x <= 0.0 && rig.hero.velocity.z.abs() < 1e-4);
        rig.tick(&crouch(None));
        if rig.flipping() {
            speeds.push(rig.horizontal().length());
        }
    }
    for (tick, speed) in speeds.iter().enumerate().take(31) {
        let expected = 8.0 * (tick + 1) as f32 * TICK;
        assert!((speed - expected).abs() < TOLERANCE, "tick {tick}: {speed}");
    }
    let top: Vec<usize> = (0..speeds.len())
        .filter(|&tick| (speeds[tick] - 4.2).abs() < 1e-4)
        .collect();
    assert_eq!(top.first(), Some(&31), "{speeds:?}");
    // The trace held 4.2 for 11 frames and then slowed for 10 before landing on its
    // 52nd; on flat ground the kit lands a frame later.
    let slowing = speeds.len() - 1 - top.last().unwrap();
    assert!((10..=11).contains(&slowing), "{slowing}: {speeds:?}");
    for pair in speeds[top.last().unwrap() + 1..].windows(2) {
        assert!(
            (pair[0] - pair[1] - 8.0 * TICK).abs() < TOLERANCE,
            "{pair:?}"
        );
    }
    assert!((52..=53).contains(&speeds.len()), "{}", speeds.len());
}

#[test]
fn a_crouch_flip_turns_back_to_where_he_faced_ten_ticks_before() {
    // The crouch turned him a little towards the stick before X; the flip turns him back.
    let mut rig = Rig::new();
    rig.crouch(16);
    rig.ticks(2, &crouch(Some(LEFT)));
    rig.tick(&crouch_jump(Some(LEFT)));
    assert!(rig.flipping());
    rig.ticks(20, &crouch(None));
    assert!(rig.hero.yaw.abs() < 0.01, "{}", rig.hero.yaw);
}

#[test]
fn a_crouch_flip_from_a_run_carries_the_run_on_along_his_facing() {
    // Take 6: running, crouch, then the stick from forward to left and X on the crouch's
    // 5th frame. The run's speed carries on forward while the flip moves left.
    let mut rig = Rig::new();
    rig.ticks(30, &input(Some(FORWARD), &[], &[]));
    rig.tick(&input(Some(FORWARD), &[Action::Crouch], &[Action::Crouch]));
    rig.ticks(2, &crouch(Some(FORWARD)));
    rig.ticks(2, &crouch(Some(LEFT)));
    let carried = rig.horizontal().length();
    assert!(carried > 1.5, "{carried}");
    rig.tick(&crouch_jump(Some(LEFT)));
    assert!(rig.flipping());
    assert_eq!(rig.hero.flip_side, StrafeSide::Left);
    rig.ticks(9, &crouch(None));
    // Ten ticks in: forward (−z) at the carried speed, left (−x) at 10 × 8/60.
    let velocity = rig.horizontal();
    assert!(
        (velocity.y + carried).abs() < 0.02,
        "{velocity} vs {carried}"
    );
    assert!((velocity.x + 80.0 * TICK).abs() < TOLERANCE, "{velocity}");
}

#[test]
fn landings_keep_the_carried_speed_or_none() {
    // A back flip lands with no speed.
    let mut rig = Rig::new();
    rig.ticks(30, &input(Some(FORWARD), &[], &[]));
    rig.tick(&input(
        Some(BACK),
        &[Action::Crouch],
        &[Action::Crouch, Action::Jump],
    ));
    assert_eq!(rig.hero.flip_side, StrafeSide::Back);
    while rig.flipping() {
        rig.tick(&input(Some(BACK), &[], &[]));
    }
    assert!(rig.hero.speed < 1e-3, "{}", rig.hero.speed);

    // A side flip from a run lands with the run's speed while the stick is held...
    for (held, expected) in [(Some(LEFT), None), (None, Some(2.5))] {
        let mut rig = Rig::new();
        rig.ticks(30, &input(Some(FORWARD), &[], &[]));
        rig.tick(&input(
            Some(LEFT),
            &[Action::Crouch],
            &[Action::Crouch, Action::Jump],
        ));
        assert_eq!(rig.hero.flip_side, StrafeSide::Left);
        let carried = 6.17;
        while rig.flipping() {
            rig.tick(&input(held, &[], &[]));
        }
        // ...and at most 2.5 u/s with it let go (take 6 landed at 0.041667 u a frame).
        let expected = expected.unwrap_or(carried);
        assert!(
            (rig.hero.speed - expected).abs() < 0.2,
            "{held:?}: {}",
            rig.hero.speed
        );
    }
}

#[test]
fn a_crouch_flip_stops_short_of_a_drop() {
    // A ledge 0.3 u to his left with a deep drop beyond: low over the ledge late in the
    // flip, with no ground ahead at the take-off height, the flip's own speed stops.
    let ledge = block(Vec3::new(49.7, -0.5, 0.0), Vec3::new(50.0, 0.5, 50.0));
    let pit = block(Vec3::new(0.0, -20.5, 0.0), Vec3::new(100.0, 0.5, 100.0));
    let mut rig = Rig::on(&[ledge, pit]);
    rig.crouch(16);
    rig.flip(LEFT);
    while rig.hero.ticks < 14 {
        rig.tick(&crouch(None));
    }
    assert!(rig.flipping());
    assert_eq!(rig.hero.velocity.x, 0.0, "{}", rig.hero.velocity);

    // The same flip on open ground keeps going.
    let mut rig = Rig::new();
    rig.crouch(16);
    rig.flip(LEFT);
    while rig.hero.ticks < 14 {
        rig.tick(&crouch(None));
    }
    assert!(rig.hero.velocity.x < -1.0, "{}", rig.hero.velocity);
}

#[test]
fn the_crouch_turns_on_a_slow_spring_of_its_own() {
    // Take 5: with the stick held to his left, the crouch turned him on gain 0.002,
    // damping 0.07 and a cap of 0.1658 rad a frame (`hero-controller.md`, "Crouching"),
    // which reproduces all 34 traced frames to 3e-6 rad. So a stick pushed to a side stays
    // to his side long enough to flip.
    let mut rig = Rig::new();
    rig.crouch(2);
    let (mut yaw, mut rate) = (0.0_f32, 0.0_f32);
    for tick in 0..34 {
        rig.tick(&crouch(Some(LEFT)));
        let error = std::f32::consts::FRAC_PI_2 - yaw;
        rate += 0.002 * error - 0.07 * rate;
        yaw += rate;
        assert!(
            (rig.hero.yaw - yaw).abs() < 1e-5,
            "tick {tick}: {} vs {yaw}",
            rig.hero.yaw
        );
    }
    // The trace turned 0.76 rad in those 34 frames, with the stick 1.44 rad round.
    assert!(rig.hero.yaw > 0.5 && rig.hero.yaw < 1.0, "{}", rig.hero.yaw);
}
