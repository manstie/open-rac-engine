//! P1.8's wrench against `docs/research/hero-controller.md`, "Melee": the combo, the jump
//! attack, the throw, the grind attack and the clank. Speeds and frames are the ones in
//! UYA NTSC's traces
//! `uyantsc-veldin-moves-2` (a standing combo and jump attack) and
//! `uyantsc-veldin-melee-1` (throws, a combo with the stick held, a run-jump attack); the
//! rest are the code's numbers. UYA has no rail to grind on, so the grind attack is the
//! code's alone.

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, GrindRail, HeroMotion, HeroProfile, HeroState, JumpKind, ParryWorld,
    controller::{StrikeShape, TICK, best_target, step},
    profile::AimProfile,
};
use rac_input::{Action, Buttons, PlayerInput};
use std::f32::consts::FRAC_PI_2;

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

fn idle() -> PlayerInput {
    input(None, &[], &[])
}

fn melee() -> PlayerInput {
    input(None, &[], &[Action::Melee])
}

fn block(centre: Vec3, half_extents: Vec3) -> BodyPlacement {
    BodyPlacement::fixed(Collider::Cuboid { half_extents }, centre, Quat::IDENTITY)
}

fn floor() -> BodyPlacement {
    block(Vec3::new(0.0, -0.5, 0.0), Vec3::new(100.0, 0.5, 100.0))
}

struct Rig {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
}

impl Rig {
    fn new(bodies: &[BodyPlacement], targets: Vec<Vec3>) -> Self {
        let mut rig = Self {
            hero: HeroMotion::new(Vec3::ZERO, 0.0),
            profile: HeroProfile::uya(),
            world: ParryWorld::new(bodies).with_targets(targets),
        };
        rig.tick(&idle());
        rig
    }

    fn open() -> Self {
        Self::new(&[floor()], Vec::new())
    }

    fn tick(&mut self, input: &PlayerInput) {
        step(&mut self.hero, input, 0.0, &self.profile, &self.world);
    }

    fn ticks(&mut self, count: usize, input: &PlayerInput) {
        for _ in 0..count {
            self.tick(input);
        }
    }

    fn speed(&self) -> f32 {
        Vec2::new(self.hero.velocity.x, self.hero.velocity.z).length()
    }

    /// Ticks with `input` until `done`, at most `limit` times; returns how many it took.
    fn until(
        &mut self,
        limit: usize,
        input: &PlayerInput,
        done: impl Fn(&HeroMotion) -> bool,
    ) -> usize {
        for count in 0..limit {
            if done(&self.hero) {
                return count;
            }
            self.tick(input);
        }
        panic!(
            "gave up after {limit} ticks in {:?} at {}",
            self.hero.state, self.hero.position
        );
    }
}

/// The speed and animation frame after each tick of an attack, starting with the tick
/// that started it, as in the traces' rows t = 0, 1, 2 ...
fn record(rig: &mut Rig, first: &PlayerInput, then: &PlayerInput, ticks: usize) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    rig.tick(first);
    out.push((rig.speed(), rig.hero.attack_frame()));
    for _ in 1..ticks {
        rig.tick(then);
        out.push((rig.speed(), rig.hero.attack_frame()));
    }
    out
}

fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

// --- The combo --------------------------------------------------------------------------

#[test]
fn the_first_swing_lunges_at_4_4_u_s_timed_by_its_animation() {
    // moves-2, the standing combo's first swing: rows t = 1..=10.
    let mut rig = Rig::open();
    let rows = record(&mut rig, &melee(), &idle(), 11);
    assert_eq!(rig.hero.state, HeroState::Combo(0));
    let expected = [
        (0.0, 16.0),
        (0.62, 16.2),
        (1.23, 16.4),
        (1.85, 16.6),
        (2.47, 16.8),
        (3.08, 16.0),
        (3.70, 16.5),
        (4.32, 17.0),
        (4.40, 17.5),
        (3.93, 18.0),
        (3.47, 18.5),
    ];
    for (t, (&(speed, frame), &(want_speed, want_frame))) in
        rows.iter().zip(expected.iter()).enumerate()
    {
        assert!(
            close(speed, want_speed, 0.01) && close(frame, want_frame, 0.01),
            "t = {t}: speed {speed}, frame {frame}"
        );
    }
}

#[test]
fn the_second_swing_lunges_at_5_7_then_4_5_then_3_u_s() {
    // moves-2, the second swing: frames 7.0, 7.5, 8.0, 8.5, 9.0 after its 7-tick blend.
    let mut rig = Rig::open();
    rig.tick(&melee());
    rig.until(40, &melee(), |hero| hero.state == HeroState::Combo(1));
    let rows = record(&mut rig, &idle(), &idle(), 12);
    let speeds: Vec<f32> = rows[7..12].iter().map(|&(speed, _)| speed).collect();
    let frames: Vec<f32> = rows[7..12].iter().map(|&(_, frame)| frame).collect();
    // The first of these is the tick after the swing started, so one later than the
    // trace's rows.
    assert!(
        close(frames[0], 7.5, 0.01) && close(speeds[0], 4.5, 0.01),
        "{frames:?} {speeds:?}"
    );
    assert!(close(speeds[2], 4.03, 0.01), "{speeds:?}");
    assert!(close(speeds[3], 3.57, 0.01), "{speeds:?}");
}

#[test]
fn melee_pressed_in_time_chains_the_three_swings_and_starts_over() {
    let mut rig = Rig::open();
    rig.tick(&melee());
    let mut seen = vec![rig.hero.state];
    for _ in 0..200 {
        rig.tick(&melee());
        if seen.last() != Some(&rig.hero.state) {
            seen.push(rig.hero.state);
        }
        if seen.len() == 4 {
            break;
        }
    }
    assert_eq!(
        seen,
        [
            HeroState::Combo(0),
            HeroState::Combo(1),
            HeroState::Combo(2),
            HeroState::Combo(0),
        ]
    );
}

#[test]
fn the_next_swing_waits_for_its_frame_and_forgets_early_presses() {
    // The first swing takes a new press from frame 23 (tick 19), counting presses from
    // frame 19; one at frame 18.5 is too early and is lost.
    let mut rig = Rig::open();
    rig.tick(&melee());
    rig.ticks(9, &idle());
    rig.tick(&melee()); // t = 10, frame 18.5
    rig.ticks(40, &idle());
    assert_eq!(rig.hero.state, HeroState::Idle);

    let mut rig = Rig::open();
    rig.tick(&melee());
    rig.ticks(14, &idle());
    rig.tick(&melee()); // t = 15, frame 21.0: held over until frame 23
    let waited = rig.until(10, &idle(), |hero| hero.state != HeroState::Combo(0));
    assert_eq!(rig.hero.state, HeroState::Combo(1));
    assert_eq!(15 + 1 + waited, 20, "the second swing starts at frame 23");
}

#[test]
fn a_swing_left_for_a_moment_carries_on_the_combo() {
    // The first swing ends (frame 31, idle at tick 36); a press less than 27 ticks later
    // is the second swing, a later one the first again.
    let mut rig = Rig::open();
    rig.tick(&melee());
    rig.until(60, &idle(), |hero| hero.state == HeroState::Idle);
    rig.ticks(20, &idle());
    rig.tick(&melee());
    assert_eq!(rig.hero.state, HeroState::Combo(1));

    let mut rig = Rig::open();
    rig.tick(&melee());
    rig.until(60, &idle(), |hero| hero.state == HeroState::Idle);
    rig.ticks(30, &idle());
    rig.tick(&melee());
    assert_eq!(rig.hero.state, HeroState::Combo(0));
}

#[test]
fn a_swing_with_the_stick_held_goes_the_sticks_way_and_walks_off() {
    // melee-1: with the stick held the swing turns to it and moves that way at once, and
    // the third swing walks on when it ends.
    let mut rig = Rig::open();
    let left = input(Some([-127, 0]), &[], &[]);
    rig.tick(&input(Some([-127, 0]), &[], &[Action::Melee]));
    rig.tick(&left);
    let moving = rig.hero.velocity.normalize();
    assert!(moving.x < -0.99, "moving left at once: {moving}");
    rig.ticks(10, &left);
    assert!(
        close(rig.hero.yaw, FRAC_PI_2, 0.01),
        "turned to the stick: {}",
        rig.hero.yaw
    );
    // Over (frame 26.5) with the stick held, a swing walks on.
    let mut rig = Rig::open();
    let ahead = input(Some([0, 127]), &[], &[]);
    rig.tick(&input(Some([0, 127]), &[], &[Action::Melee]));
    let walked = rig.until(60, &ahead, |hero| hero.state == HeroState::Walk);
    assert_eq!(walked, 26);
}

#[test]
fn the_combo_turns_to_a_target_and_lunges_further_to_reach_it() {
    let yaw_to = |to: Vec3| (-to.x).atan2(-to.z);
    let target = Vec3::new(-2.0, 0.5, -3.0);
    let mut rig = Rig::new(&[floor()], vec![target]);
    rig.tick(&melee());
    assert_eq!(rig.hero.attack_aim(), Some(yaw_to(target)));
    rig.ticks(20, &idle());
    assert!(
        close(rig.hero.yaw, yaw_to(target), 0.02),
        "{} vs {}",
        rig.hero.yaw,
        yaw_to(target)
    );
    let reach = rig.hero.position.distance(Vec3::ZERO);

    let mut alone = Rig::open();
    alone.tick(&melee());
    alone.ticks(20, &idle());
    let plain = alone.hero.position.distance(Vec3::ZERO);
    // The scale raises the lunge's speed, not its acceleration, so the swing goes only a
    // little further.
    assert!(reach > plain * 1.1, "{reach} vs {plain}");
}

#[test]
fn a_swing_stops_short_of_an_edge() {
    // On a 2 u block facing its edge 0.6 u ahead, the lunge doesn't take him off.
    let platform = block(Vec3::new(0.0, 1.0, 0.0), Vec3::new(5.0, 1.0, 0.6));
    let mut rig = Rig::new(&[floor(), platform], Vec::new());
    rig.hero = HeroMotion::new(Vec3::new(0.0, 2.0, 0.0), 0.0);
    rig.tick(&idle());
    rig.tick(&melee());
    rig.ticks(30, &idle());
    assert!(rig.hero.position.y > 1.99, "{}", rig.hero.position);
    assert!(rig.hero.ground.is_some());
}

// --- What the wrench hits ---------------------------------------------------------------

#[test]
fn a_swing_hits_in_front_during_its_window_once_per_swing() {
    // The first swing hits from frame 17 to 23: ticks 7 to 19.
    let mut rig = Rig::open();
    rig.tick(&melee());
    let ahead = Vec3::new(0.0, 0.5, -1.5);
    let mut hit_ticks = Vec::new();
    let mut serials = Vec::new();
    for t in 1..30 {
        rig.tick(&idle());
        for strike in &rig.hero.strikes {
            serials.push(strike.serial);
            if strike.shape.touches(ahead, 0.3) {
                hit_ticks.push(t);
            }
            assert_eq!(strike.damage, 1.0);
        }
    }
    let first = *hit_ticks.first().expect("the swing hits ahead");
    assert!((7..=19).contains(&first), "{hit_ticks:?}");
    assert!(
        hit_ticks.iter().all(|t| (7..=19).contains(t)),
        "{hit_ticks:?}"
    );
    serials.dedup();
    assert_eq!(serials.len(), 1, "one serial for the swing");
}

#[test]
fn the_swings_push_targets_across_their_line() {
    // The first swing goes right to left and pushes to the hero's left (−x when he faces
    // −z); the second the other way.
    let mut rig = Rig::open();
    rig.tick(&melee());
    let push = (0..20)
        .find_map(|_| {
            rig.tick(&idle());
            rig.hero.strikes.first().map(|strike| strike.push)
        })
        .unwrap();
    assert!(push.length() > 0.99 && push.length() < 1.01, "{push}");
    let mut sideways = Vec::new();
    for _ in 0..20 {
        rig.tick(&melee());
        if rig.hero.state == HeroState::Combo(1)
            && let Some(strike) = rig.hero.strikes.first()
        {
            sideways.push(strike.push.x);
        }
    }
    assert!(sideways.iter().all(|&x| x >= 0.0), "{sideways:?}");
}

#[test]
fn best_target_weighs_distance_by_angle_and_keeps_to_range_and_cone() {
    let aim = AimProfile {
        range: 5.0,
        cone_degrees: 45.0,
        rise_degrees: 45.0,
    };
    // Facing −z (yaw 0): straight ahead at 3 u beats 2 u off at 40°.
    let ahead = Vec3::new(0.0, 0.0, -3.0);
    let aside = Vec3::new(-2.0 * 0.643, 0.0, -2.0 * 0.766);
    assert_eq!(
        best_target(&[aside, ahead], Vec3::ZERO, 0.0, &aim),
        Some(ahead)
    );
    // Out of the cone, out of range, or too far above.
    let behind = Vec3::new(0.0, 0.0, 2.0);
    let far = Vec3::new(0.0, 0.0, -6.0);
    let above = Vec3::new(0.0, 3.0, -1.0);
    assert_eq!(
        best_target(&[behind, far, above], Vec3::ZERO, 0.0, &aim),
        None
    );
}

// --- The jump attack --------------------------------------------------------------------

/// A standing jump, then melee once the jump is 12 ticks old.
fn jump_attack(rig: &mut Rig) {
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(11, &idle());
    rig.tick(&melee());
    assert_eq!(rig.hero.state, HeroState::JumpAttack);
}

#[test]
fn melee_in_the_air_needs_the_jump_11_ticks_old() {
    let mut rig = Rig::open();
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(5, &idle());
    rig.tick(&melee());
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Standing));
    rig.ticks(4, &idle());
    // The press is remembered for 14 ticks.
    rig.tick(&idle());
    assert_eq!(rig.hero.state, HeroState::JumpAttack);
}

#[test]
fn the_jump_attack_rises_holds_then_drives_down_at_130_u_s2() {
    // moves-2 and melee-1: up at 150 u/s² towards 7.7 u/s (less when high), held until
    // tick 14, then 130 u/s² down; the strike lands at frame 26 and it ends at 30.
    let mut rig = Rig::open();
    jump_attack(&mut rig);
    let mut vertical = Vec::new();
    for _ in 0..16 {
        rig.tick(&idle());
        vertical.push(rig.hero.velocity.y);
    }
    let step = 150.0 * TICK;
    assert!(vertical[1] - vertical[0] <= step + 1e-3, "{vertical:?}");
    // Up to 7.7 u/s, then 5.39 once more than 1.8 u up.
    assert!(close(vertical[1], 7.7, 1e-3), "{vertical:?}");
    assert!(close(vertical[7], 5.39, 1e-3), "{vertical:?}");
    assert!(close(vertical[13], vertical[7], 1e-3), "held: {vertical:?}");
    assert!(
        close(vertical[14] - vertical[13], -130.0 * TICK, 1e-3),
        "{vertical:?}"
    );
    rig.until(60, &idle(), |hero| hero.ground.is_some());
    let landed = rig.hero.attack_frame();
    assert!(close(landed, 26.0, 1.0), "landed at frame {landed}");
    rig.until(30, &idle(), |hero| hero.state == HeroState::Idle);
    assert!(rig.hero.attack_frame() > 30.0);
}

#[test]
fn the_jump_attack_brakes_at_15_u_s2_and_hits_for_2() {
    // melee-1: from a run jump at 6.17 u/s the speed falls 0.25 u/s a tick.
    let mut rig = Rig::open();
    let run = input(Some([0, 127]), &[], &[]);
    rig.ticks(40, &run);
    rig.tick(&input(Some([0, 127]), &[], &[Action::Jump]));
    rig.ticks(11, &run);
    rig.tick(&input(Some([0, 127]), &[], &[Action::Melee]));
    assert_eq!(rig.hero.state, HeroState::JumpAttack);
    rig.tick(&run);
    let before = rig.speed();
    rig.tick(&run);
    assert!(
        close(before - rig.speed(), 0.25, 1e-3),
        "{before} {}",
        rig.speed()
    );
    let damage = (0..40)
        .find_map(|_| {
            rig.tick(&run);
            rig.hero.strikes.first().map(|strike| strike.damage)
        })
        .unwrap();
    assert_eq!(damage, 2.0);
}

// --- The throw ----------------------------------------------------------------------------

fn throw() -> PlayerInput {
    input(None, &[Action::Crouch], &[Action::Melee])
}

#[test]
fn crouch_and_melee_throw_the_wrench_out_8_6_u_and_back() {
    // melee-1: released at frame 33.5 (t = 12), out 8.65 u from the hero in 30 ticks,
    // back in the hand 25 ticks later; standing still the throw lasts 95 ticks.
    let mut rig = Rig::open();
    rig.tick(&throw());
    assert_eq!(rig.hero.state, HeroState::Throw);
    let mut all = 0;
    let released = rig.until(20, &idle(), |hero| hero.thrown.is_some());
    assert_eq!(released, 12);
    assert!(close(rig.hero.attack_frame(), 33.5, 1e-3));
    let out = rig.until(40, &idle(), |hero| {
        hero.thrown.is_some_and(|wrench| wrench.returning)
    });
    let back = rig.until(40, &idle(), |hero| hero.thrown.is_none());
    assert!((29..=31).contains(&out), "{out}");
    assert!((24..=26).contains(&back), "{back}");
    all += released + out + back;
    all += rig.until(40, &idle(), |hero| hero.state == HeroState::Idle);
    // The trace's catch animation starts a tick later than the kit's, so its throw lasts
    // 95 ticks to the kit's 94.
    assert!((94..=95).contains(&all), "{all}");
}

#[test]
fn the_thrown_wrench_reaches_8_65_u_from_the_hero() {
    let mut rig = Rig::open();
    rig.tick(&throw());
    let mut furthest = 0.0f32;
    for _ in 0..80 {
        rig.tick(&idle());
        if let Some(wrench) = rig.hero.thrown {
            let away = Vec2::new(
                wrench.position.x - rig.hero.position.x,
                wrench.position.z - rig.hero.position.z,
            )
            .length();
            furthest = furthest.max(away);
        }
    }
    assert!(close(furthest, 8.66, 0.1), "{furthest}");
}

#[test]
fn the_throw_from_a_crouch_and_walking_off_after_the_release() {
    let mut rig = Rig::open();
    rig.ticks(5, &input(None, &[Action::Crouch], &[]));
    assert_eq!(rig.hero.state, HeroState::Crouch);
    rig.tick(&throw());
    assert_eq!(rig.hero.state, HeroState::Throw);
    // The stick past 0.9 walks him off once the wrench is gone (frame 34).
    let walk = input(Some([0, 127]), &[], &[]);
    let walked = rig.until(30, &walk, |hero| hero.state == HeroState::Walk);
    assert_eq!(walked, 14, "walks off at frame 34.5");
    assert!(rig.hero.thrown.is_some());
    rig.until(120, &walk, |hero| hero.thrown.is_none());
}

#[test]
fn no_attack_while_the_wrench_is_out() {
    let mut rig = Rig::open();
    rig.tick(&throw());
    rig.until(20, &idle(), |hero| hero.thrown.is_some());
    let walk = input(Some([0, 127]), &[], &[]);
    rig.until(30, &walk, |hero| hero.state == HeroState::Walk);
    rig.tick(&input(Some([0, 127]), &[], &[Action::Melee]));
    assert_eq!(rig.hero.state, HeroState::Walk);
}

#[test]
fn the_thrown_wrench_hits_as_a_ball_and_comes_back_off_a_wall() {
    let wall = block(Vec3::new(0.0, 2.0, -4.0), Vec3::new(5.0, 2.0, 0.5));
    let mut rig = Rig::new(&[floor(), wall], Vec::new());
    rig.tick(&throw());
    rig.until(20, &idle(), |hero| hero.thrown.is_some());
    let back = rig.until(20, &idle(), |hero| {
        hero.thrown.is_some_and(|wrench| wrench.returning)
    });
    assert!(back < 12, "turned back at the wall after {back}");
    let wrench = rig.hero.thrown.unwrap();
    assert!(wrench.position.z > -3.5 - 1e-3, "{}", wrench.position);
    let strike = rig.hero.strikes.last().unwrap();
    assert!(matches!(strike.shape, StrikeShape::Ball { radius, .. } if radius == 0.4));
    assert!(
        strike.push.z > 0.0,
        "back towards the hero: {}",
        strike.push
    );
}

#[test]
fn attacks_have_the_games_state_numbers() {
    assert_eq!(HeroState::Combo(2).game_number(), 19);
    assert_eq!(HeroState::JumpAttack.game_number(), 20);
    assert_eq!(HeroState::Throw.game_number(), 21);
}

#[test]
fn the_grind_attack_is_state_42() {
    assert_eq!(HeroState::GrindAttack(1).game_number(), 42);
    assert!(HeroState::GrindAttack(0).is_attacking());
    assert!(HeroState::GrindAttack(0).is_grinding());
}

// --- The grind attack -------------------------------------------------------------------

/// The hero riding a long straight rail 2 u up, along −z (yaw 0), at full speed.
fn grinding() -> Rig {
    let rail = GrindRail::new(vec![Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 2.0, -500.0)]);
    let mut rig = Rig {
        hero: HeroMotion::new(Vec3::new(0.0, 3.0, -1.0), 0.0),
        profile: HeroProfile::uya(),
        world: ParryWorld::new(&[floor()]).with_rails(vec![rail]),
    };
    rig.until(30, &idle(), |hero| hero.state == HeroState::Grind);
    rig.ticks(240, &idle());
    rig
}

fn jump() -> PlayerInput {
    input(None, &[], &[Action::Jump])
}

#[test]
fn he_rides_a_rail_side_on() {
    // "Grinding": he turns to the rail's way less 90°, so he faces to its right.
    let rig = grinding();
    assert!(
        (rig.hero.yaw + FRAC_PI_2).abs() < 0.01,
        "yaw {}",
        rig.hero.yaw
    );
}

#[test]
fn melee_on_a_rail_swings_and_he_rides_on() {
    // "The grind attack": sequence 70 from frame 14, blending in over 4 ticks, then half a
    // frame a tick; past frame 26 it is back to the grind, at the grind's own speed.
    let mut rig = grinding();
    let speed = rig.hero.speed;
    rig.tick(&melee());
    assert_eq!(rig.hero.state, HeroState::GrindAttack(0));
    assert_eq!(rig.hero.attack_frame(), 14.0);
    let ticks = rig.until(60, &idle(), |hero| hero.state == HeroState::Grind);
    assert_eq!(ticks, 4 + 25, "frame 26.5 is 25 half frames on from 14");
    assert!((rig.hero.speed - speed).abs() < 1e-3);
    assert!(rig.hero.velocity.z < -11.9, "{}", rig.hero.velocity);
}

#[test]
fn the_grind_attack_hits_all_round_him_then_with_the_wrench() {
    // A ball of 1 u round a point 0.6 u up for its first ticks, pushing forward and to the
    // left of the rail; then the wrench's line on frames 17 to 23, its head 0.7 u.
    let mut rig = grinding();
    let mut balls = 0;
    let mut sweeps = Vec::new();
    rig.tick(&melee());
    while matches!(rig.hero.state, HeroState::GrindAttack(_)) {
        for strike in &rig.hero.strikes {
            match strike.shape {
                StrikeShape::Ball { centre, radius } => {
                    balls += 1;
                    assert_eq!(radius, 1.0);
                    assert!(close(centre.y - rig.hero.position.y, 0.6, 1e-4));
                    assert!(
                        strike.push.z < 0.0 && strike.push.x < 0.0,
                        "{}",
                        strike.push
                    );
                }
                StrikeShape::Sweep { head_radius, .. } => {
                    assert_eq!(head_radius, 0.7);
                    sweeps.push(rig.hero.attack_frame());
                }
            }
        }
        rig.tick(&idle());
    }
    assert_eq!(balls, 19);
    assert_eq!(sweeps.first(), Some(&17.0));
    assert_eq!(sweeps.last(), Some(&23.0));
}

#[test]
fn a_press_late_in_a_grind_swing_swings_back_the_other_way() {
    // A press waits for frame 23 to start the other swing, sequence 71 from frame 6; a
    // press straight after that one swings the first again.
    let mut rig = grinding();
    rig.tick(&melee());
    rig.until(60, &idle(), |hero| hero.attack_frame() >= 20.0);
    rig.tick(&melee());
    assert_eq!(rig.hero.state, HeroState::GrindAttack(0));
    rig.until(20, &idle(), |hero| hero.state != HeroState::GrindAttack(0));
    assert_eq!(rig.hero.state, HeroState::GrindAttack(1));
    assert_eq!(rig.hero.attack_frame(), 6.0);
    rig.until(60, &idle(), |hero| hero.attack_frame() > 14.0);
    rig.tick(&melee());
    assert_eq!(rig.hero.state, HeroState::GrindAttack(0));
}

#[test]
fn x_leaves_a_grind_swing_only_from_its_jump_frame() {
    // X counts for 5 ticks, and only from frame 24 of the first swing.
    let mut rig = grinding();
    rig.tick(&melee());
    rig.until(60, &idle(), |hero| hero.attack_frame() >= 17.0);
    rig.tick(&jump());
    rig.until(60, &idle(), |hero| hero.attack_frame() >= 24.0);
    assert_eq!(rig.hero.state, HeroState::GrindAttack(0));
    rig.tick(&jump());
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Grind));
}

// --- The clank --------------------------------------------------------------------------

/// The clanks during the attack melee starts, with the tick of each.
fn clanks(rig: &mut Rig) -> Vec<(u32, rac_hero::Clank)> {
    let mut out = Vec::new();
    rig.tick(&melee());
    let state = rig.hero.state;
    while rig.hero.state == state {
        rig.tick(&idle());
        if let Some(clank) = rig.hero.clank {
            out.push((rig.hero.ticks, clank));
        }
    }
    out
}

#[test]
fn a_swing_into_a_wall_clanks_once_and_swings_on() {
    // "What the wrench hits": a 1.4 u ray from 0.5 u up towards the tip, past the swing's
    // tenth tick and within 70° of his facing. The wall's face is 1.2 u ahead.
    let wall = block(Vec3::new(0.0, 1.0, -1.7), Vec3::new(5.0, 1.0, 0.5));
    let mut rig = Rig::new(&[floor(), wall], Vec::new());
    let found = clanks(&mut rig);
    assert_eq!(found.len(), 1, "{found:?}");
    let (tick, clank) = found[0];
    assert!(tick > 10, "{tick}");
    assert!(close(clank.point.z, -1.2, 1e-3), "{}", clank.point);
    assert!(close(clank.point.y, 0.5, 1e-3), "{}", clank.point);
    assert!(clank.normal.z > 0.99, "{}", clank.normal);
    // In the open there is nothing to clank on.
    assert!(clanks(&mut Rig::open()).is_empty());
}

#[test]
fn the_jump_attack_clanks_on_the_ground_it_comes_down_on() {
    // As its frame passes 27, the ground under the wrench's tip.
    let mut rig = Rig::open();
    jump_attack(&mut rig);
    let mut found = Vec::new();
    while rig.hero.state == HeroState::JumpAttack {
        rig.tick(&idle());
        if let Some(clank) = rig.hero.clank {
            found.push((rig.hero.attack_frame(), clank));
        }
    }
    assert_eq!(found.len(), 1, "{found:?}");
    let (frame, clank) = found[0];
    assert!(frame > 27.0 && frame <= 28.0, "{frame}");
    assert!(clank.point.y.abs() < 1e-3, "{}", clank.point);
}
