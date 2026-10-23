//! The controller with Deadlocked's profile against our own DL NTSC-U traces
//! `dlntsc-dreadzone-moves-{1..5}`, `dlntsc-dreadzone-melee-{1..5}` and
//! `dlntsc-dreadzone-flip-{1,2}` (PLAN.md RE.6;
//! `docs/research/hero-controller.md`, "Differences between the games" and "Deadlocked's
//! melee"), and beside UYA's where the games differ.
//!
//! The traces show the pad one frame before the hero acts on it, so a press on trace row
//! n is a press on the tick of row n + 1 here.

use std::f32::consts::PI;

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, GrindRail, HeroMotion, HeroProfile, HeroState, JumpKind, ParryWorld,
    controller::step,
};
use rac_input::{Action, Buttons, PlayerInput};

const TOLERANCE: f32 = 2e-3;

struct Rig {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
}

impl Rig {
    fn new(profile: HeroProfile) -> Self {
        let world = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(200.0, 0.5, 200.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        )]);
        let mut rig = Self {
            hero: HeroMotion::new(Vec3::ZERO, 0.0),
            profile,
            world,
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

    /// A tick with the camera looking along `camera_yaw` (0 is along −z).
    fn tick_seen_from(&mut self, camera_yaw: f32, input: &PlayerInput) {
        step(
            &mut self.hero,
            input,
            camera_yaw,
            &self.profile,
            &self.world,
        );
    }

    fn horizontal_speed(&self) -> f32 {
        Vec2::new(self.hero.velocity.x, self.hero.velocity.z).length()
    }

    /// The highest the feet get above the start for a jump held `hold_ticks` in all.
    fn jump_apex(&mut self, hold_ticks: usize) -> f32 {
        let start = self.hero.position.y;
        let mut apex: f32 = 0.0;
        self.tick(&input(None, &[], &[Action::Jump]));
        for tick in 1..240 {
            let held: &[Action] = if tick < hold_ticks {
                &[Action::Jump]
            } else {
                &[]
            };
            self.tick(&input(None, held, &[]));
            apex = apex.max(self.hero.position.y - start);
            if !self.hero.state.is_airborne() {
                break;
            }
        }
        apex
    }

    /// Jumps, presses jump again `second_press` ticks later, and returns the tick of the
    /// first jump on which the double jump began, if it did.
    fn double_jump_tick(&mut self, second_press: usize) -> Option<usize> {
        self.tick(&input(None, &[], &[Action::Jump]));
        for tick in 1..120 {
            let pressed: &[Action] = if tick == second_press {
                &[Action::Jump]
            } else {
                &[]
            };
            self.tick(&input(None, &[], pressed));
            if self.hero.state == HeroState::DoubleJump {
                return Some(tick);
            }
            if !self.hero.state.is_airborne() {
                return None;
            }
        }
        None
    }
}

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
        look_stick: [0, 0],
        held: buttons(held) | buttons(pressed),
        pressed: buttons(pressed),
    }
}

#[test]
fn deadlocked_walks_at_7_25() {
    // moves-1: the speed steps by 0.6167 u/s a tick (37 u/s²) to 7.25.
    let mut rig = Rig::new(HeroProfile::dl());
    let forward = input(Some([0, 127]), &[], &[]);
    rig.tick(&forward);
    assert!((rig.hero.speed - 37.0 / 60.0).abs() < TOLERANCE);
    rig.ticks(15, &forward);
    assert_eq!(rig.hero.speed, 7.25);
}

/// DL's hero code has no cut-off: the idle's 0.22 and the walk's 0.17 decide (both games'
/// transitions; `hero-controller.md`, "Input").
#[test]
fn deadlocked_walks_on_a_stick_over_0_22_and_keeps_on_down_to_0_17() {
    let stick = |counts: i8| input(Some([0, counts]), &[], &[]);
    // 0.237 of the stick: under UYA's cut-off.
    let mut uya = Rig::new(HeroProfile::uya());
    uya.ticks(10, &stick(66));
    assert_eq!(uya.hero.state, HeroState::Idle);

    let mut dl = Rig::new(HeroProfile::dl());
    // 0.197, as in dlntsc-dreadzone-camhalf-0.5-1, stands still.
    dl.ticks(10, &stick(63));
    assert_eq!(dl.hero.state, HeroState::Idle);
    dl.ticks(10, &stick(66));
    assert_eq!(dl.hero.state, HeroState::Walk);
    assert!((dl.hero.speed - 7.25 * 18.0 / 76.0).abs() < TOLERANCE);
    // 0.184 keeps him walking; 0.145 stops him.
    dl.ticks(10, &stick(62));
    assert_eq!(dl.hero.state, HeroState::Walk);
    dl.ticks(10, &stick(59));
    assert_eq!(dl.hero.state, HeroState::Idle);
}

#[test]
fn a_fully_held_jump_peaks_where_deadlocked_does() {
    // moves-1, row 304: X held through the jump, 1.8865 u above the take-off. UYA's
    // jump is the same.
    let apex = Rig::new(HeroProfile::dl()).jump_apex(17);
    assert!((apex - 1.8865).abs() < 0.002, "apex {apex}");
}

#[test]
fn a_run_jump_flies_at_7_25() {
    // moves-1, row 591: a run jump keeps the walk's 7.25 u/s in the air.
    let mut rig = Rig::new(HeroProfile::dl());
    let forward = input(Some([0, 127]), &[], &[]);
    rig.ticks(20, &forward);
    rig.tick(&input(Some([0, 127]), &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Run));
    rig.ticks(20, &input(Some([0, 127]), &[Action::Jump], &[]));
    assert!(rig.hero.state.is_airborne());
    assert!(
        (rig.horizontal_speed() - 7.25).abs() < TOLERANCE,
        "{}",
        rig.horizontal_speed()
    );
}

#[test]
fn the_double_jump_tops_out_at_7() {
    // moves-4 and -5: the double jump's vertical speed reaches 0.116667 u a frame.
    let mut rig = Rig::new(HeroProfile::dl());
    assert_eq!(rig.double_jump_tick(20), Some(20));
    let mut top: f32 = 0.0;
    for _ in 0..12 {
        rig.tick(&input(None, &[Action::Jump], &[]));
        top = top.max(rig.hero.velocity.y);
    }
    assert!((top - 7.0).abs() < TOLERANCE, "{top}");
}

#[test]
fn an_early_second_press_double_jumps_on_the_11th_tick_in_deadlocked_and_the_15th_in_uya() {
    // moves-5, rows 121-133: the take-off on row 122, a second press on row 131, which the
    // hero sees on row 132 (the jump's 10th tick), and the double jump on row 133.
    assert_eq!(
        Rig::new(HeroProfile::dl()).double_jump_tick(10),
        Some(11),
        "Deadlocked"
    );
    // UYA keeps the press until t > 14 ("Double jump").
    assert_eq!(
        Rig::new(HeroProfile::uya()).double_jump_tick(10),
        Some(15),
        "UYA"
    );
}

#[test]
fn strafing_sideways_jumps_instead_of_flipping_in_deadlocked() {
    // moves-2, rows 387 and 717: L2 with the stick to the left or back, then X, is a
    // standing jump that keeps moving the stick's way at the walk's 7.25 u/s.
    let mut rig = Rig::new(HeroProfile::dl());
    let left = input(Some([-127, 0]), &[Action::Strafe], &[]);
    rig.ticks(30, &left);
    rig.tick(&input(Some([-127, 0]), &[Action::Strafe], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Standing));
    rig.ticks(10, &left);
    assert!(
        (rig.hero.velocity.x + 7.25).abs() < 0.01,
        "{}",
        rig.hero.velocity
    );

    // UYA flips.
    let mut rig = Rig::new(HeroProfile::uya());
    rig.tick(&input(Some([-127, 0]), &[Action::Strafe], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Flip));
}

/// A fully held standing jump while strafing with `stick` held, then a second jump press
/// `early` ticks before the first tick on the ground; the state that tick leaves him in.
/// Held fully, the jump comes down too fast for a late press to double jump.
fn jump_again_on_landing(profile: HeroProfile, stick: [i8; 2], early: usize) -> HeroState {
    let jump = input(Some(stick), &[Action::Strafe], &[Action::Jump]);
    let hold = input(Some(stick), &[Action::Strafe, Action::Jump], &[]);
    let strafe = input(Some(stick), &[Action::Strafe], &[]);
    let between = |tick: usize| if tick < 17 { &hold } else { &strafe };
    let mut rig = Rig::new(profile.clone());
    rig.tick(&jump);
    let landing = (1..)
        .find(|&tick| {
            rig.tick(between(tick));
            !rig.hero.state.is_airborne()
        })
        .expect("a landing");
    // The first ground tick follows the tick that lands.
    let first_ground = landing + 1;
    let mut rig = Rig::new(profile);
    rig.tick(&jump);
    for tick in 1..=first_ground {
        rig.tick(if tick == first_ground - early {
            &jump
        } else {
            between(tick)
        });
    }
    rig.hero.state
}

#[test]
fn deadlocked_flips_only_as_a_strafing_jump_lands() {
    // flip-1: strafing double jumps landed into flips on presses up to 7 frames old; one 9
    // frames old walked, and the next press jumped. The walk's own jump never flips.
    for early in [0, 3, 7] {
        assert_eq!(
            jump_again_on_landing(HeroProfile::dl(), [-127, 0], early),
            HeroState::Jump(JumpKind::Flip),
            "{early} ticks early"
        );
    }
    assert_eq!(
        jump_again_on_landing(HeroProfile::dl(), [-127, 0], 8),
        HeroState::Jump(JumpKind::Standing)
    );
    // UYA's walk flips anyway.
    assert_eq!(
        jump_again_on_landing(HeroProfile::uya(), [-127, 0], 8),
        HeroState::Jump(JumpKind::Flip)
    );
    // flip-2: strafing forward lands into a jump.
    assert_eq!(
        jump_again_on_landing(HeroProfile::dl(), [0, 127], 3),
        HeroState::Jump(JumpKind::Standing)
    );
}

#[test]
fn deadlocked_has_no_crouch() {
    // Its code never enters the crouch: crouch held stands, and crouch, the stick and a
    // jump together are a run jump, where UYA flips straight from idle (crouchflip-1,
    // take 7).
    let crouch = input(None, &[Action::Crouch], &[]);
    let crouch_flip = input(Some([-127, 0]), &[Action::Crouch], &[Action::Jump]);
    let mut rig = Rig::new(HeroProfile::dl());
    rig.ticks(20, &crouch);
    assert_eq!(rig.hero.state, HeroState::Idle);
    let mut rig = Rig::new(HeroProfile::dl());
    rig.tick(&crouch_flip);
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Run));

    let mut rig = Rig::new(HeroProfile::uya());
    rig.ticks(20, &crouch);
    assert_eq!(rig.hero.state, HeroState::Crouch);
    let mut rig = Rig::new(HeroProfile::uya());
    rig.tick(&crouch_flip);
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Flip));
}

// --- Melee ------------------------------------------------------------------------------

fn melee() -> PlayerInput {
    input(None, &[], &[Action::Melee])
}

#[test]
fn deadlocked_swings_play_from_their_start_and_lunge_at_4_75() {
    // melee-1, the first standing swing, rows t = 0..=19: the frame runs from 15.5 with no
    // blend, and the lunge rises at 37 u/s² to 4.75, holding while the frame was under 20.
    let trace = [
        0.0, 0.62, 1.23, 1.85, 2.47, 3.08, 3.70, 4.31, 4.75, 4.75, 4.28, 3.81, 3.35, 2.88, 2.41,
        1.95, 1.48, 1.01, 0.55, 0.08,
    ];
    let mut rig = Rig::new(HeroProfile::dl());
    rig.tick(&melee());
    for (t, &speed) in trace.iter().enumerate() {
        if t > 0 {
            rig.tick(&input(None, &[], &[]));
        }
        assert_eq!(rig.hero.state, HeroState::Combo(0));
        assert_eq!(rig.hero.attack_frame(), 15.5 + 0.5 * t as f32, "t = {t}");
        assert!(
            (rig.horizontal_speed() - speed).abs() < 0.01,
            "t = {t}: {} against {speed}",
            rig.horizontal_speed()
        );
    }
}

/// A swing, then `then` once its frame reaches 25.
fn into_a_swing(profile: HeroProfile, then: &PlayerInput) -> HeroState {
    let mut rig = Rig::new(profile);
    rig.tick(&melee());
    while rig.hero.attack_frame() < 25.0 {
        rig.tick(&input(None, &[], &[]));
    }
    rig.tick(then);
    rig.hero.state
}

#[test]
fn deadlocked_swings_on_with_crouch_held_and_jumps_out_standing() {
    // melee-5: R1 and Square at frame 28 chained swing 2, and R1 and X jumped as from
    // standing; UYA throws, and jumps from a crouch.
    let crouch_melee = input(None, &[Action::Crouch], &[Action::Melee]);
    let crouch_jump = input(None, &[Action::Crouch], &[Action::Jump]);
    assert_eq!(
        into_a_swing(HeroProfile::dl(), &crouch_melee),
        HeroState::Combo(1)
    );
    assert_eq!(
        into_a_swing(HeroProfile::uya(), &crouch_melee),
        HeroState::Throw
    );
    assert_eq!(
        into_a_swing(HeroProfile::dl(), &crouch_jump),
        HeroState::Jump(JumpKind::Standing)
    );
    assert_ne!(
        into_a_swing(HeroProfile::uya(), &crouch_jump),
        HeroState::Jump(JumpKind::Standing)
    );
}

#[test]
fn deadlocked_attacks_from_a_double_jump_far_sooner() {
    // Past its animation's frame 16, on its 12th tick (melee-3); UYA waits for frame 32.
    let first_tick = |profile: HeroProfile| {
        (1..40).find(|&delay| {
            let mut rig = Rig::new(profile.clone());
            rig.double_jump_tick(8).expect("a double jump");
            rig.ticks(delay - 1, &input(None, &[], &[]));
            rig.tick(&melee());
            rig.hero.state == HeroState::JumpAttack
        })
    };
    assert_eq!(first_tick(HeroProfile::dl()), Some(12));
    assert_eq!(first_tick(HeroProfile::uya()), Some(20));
}

#[test]
fn deadlocked_has_no_grind_attack() {
    // Its code is there, but the grind never starts it.
    let rail = GrindRail::new(vec![Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 2.0, -500.0)]);
    let mut rig = Rig::new(HeroProfile::dl());
    rig.world = ParryWorld::new(&[]).with_rails(vec![rail]);
    rig.hero = HeroMotion::new(Vec3::new(0.0, 3.0, -1.0), 0.0);
    for _ in 0..60 {
        rig.tick(&input(None, &[], &[]));
    }
    assert_eq!(rig.hero.state, HeroState::Grind);
    rig.tick(&melee());
    rig.ticks(5, &input(None, &[], &[]));
    assert_eq!(rig.hero.state, HeroState::Grind);
}

// --- Grinding ("Deadlocked's grinding") ---------------------------------------------------

/// The hero riding the first of `rails` with the camera along `camera_yaw`, after falling
/// onto it from 1 u above, just past its start.
fn grinding(profile: HeroProfile, rails: Vec<GrindRail>, camera_yaw: f32) -> Rig {
    let above = rails[0].points[0] + Vec3::new(0.0, 1.0, -1.0);
    let mut rig = Rig::new(profile);
    rig.world = ParryWorld::new(&[]).with_rails(rails);
    rig.hero = HeroMotion::new(above, 0.0);
    for _ in 0..30 {
        rig.tick_seen_from(camera_yaw, &input(None, &[], &[]));
    }
    assert_eq!(rig.hero.state, HeroState::Grind);
    rig
}

/// A straight rail 2 u up along −z (yaw 0).
fn rail_along_minus_z(length: f32) -> GrindRail {
    GrindRail::new(vec![Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 2.0, -length)])
}

#[test]
fn deadlocked_rides_facing_along_the_rail_where_uya_rides_side_on() {
    for (profile, yaw) in [
        (HeroProfile::dl(), 0.0),
        (HeroProfile::uya(), -std::f32::consts::FRAC_PI_2),
    ] {
        let mut rig = grinding(profile, vec![rail_along_minus_z(500.0)], 0.0);
        rig.ticks(60, &input(None, &[], &[]));
        assert_eq!(rig.hero.state, HeroState::Grind);
        assert!((rig.hero.yaw - yaw).abs() < 0.01, "yaw {}", rig.hero.yaw);
    }
}

#[test]
fn deadlocked_rides_a_rail_towards_the_camera_backwards() {
    // The rail runs 180° from the camera's yaw, past the entry's 90°: stance 180°.
    let mut rig = grinding(HeroProfile::dl(), vec![rail_along_minus_z(500.0)], PI);
    for _ in 0..60 {
        rig.tick_seen_from(PI, &input(None, &[], &[]));
        assert_eq!(rig.hero.state, HeroState::Grind);
    }
    assert!(
        (rig.hero.yaw.abs() - PI).abs() < 0.01,
        "yaw {}",
        rig.hero.yaw
    );
    assert!(rig.hero.velocity.z < -11.0, "{}", rig.hero.velocity);
}

#[test]
fn deadlocked_hops_round_on_the_rail_when_the_camera_turns_past_120_degrees() {
    let mut rig = grinding(HeroProfile::dl(), vec![rail_along_minus_z(500.0)], 0.0);
    rig.ticks(60, &input(None, &[], &[]));
    // 110° round is within the 120°: he rides on.
    let turned = 110f32.to_radians();
    rig.ticks(1, &input(None, &[], &[]));
    rig.tick_seen_from(turned, &input(None, &[], &[]));
    assert_eq!(rig.hero.state, HeroState::Grind);

    // 130° hops him round (state 63), over the rail all the way. With nothing held its
    // target height is 1.0 u and a step, and it peaks lower ("Vertical model").
    let turned = 130f32.to_radians();
    rig.tick_seen_from(turned, &input(None, &[], &[]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::GrindTurn));
    assert_eq!(rig.hero.state.game_number(), 63);
    let mut highest: f32 = 0.0;
    for _ in 0..120 {
        rig.tick_seen_from(turned, &input(None, &[], &[]));
        highest = highest.max(rig.hero.position.y - 2.0);
        assert!(rig.hero.position.x.abs() < 0.01, "{}", rig.hero.position);
        if rig.hero.state == HeroState::Grind {
            break;
        }
    }
    assert_eq!(rig.hero.state, HeroState::Grind);
    assert!(highest > 0.6 && highest < 0.8, "{highest}");
    // He lands facing back up the rail, within 60° of the camera, at the grind's speed.
    for _ in 0..60 {
        rig.tick_seen_from(turned, &input(None, &[], &[]));
        assert_eq!(rig.hero.state, HeroState::Grind);
    }
    assert!(
        (rig.hero.yaw.abs() - PI).abs() < 0.01,
        "yaw {}",
        rig.hero.yaw
    );
    assert!(rig.hero.speed > 11.0, "{}", rig.hero.speed);

    // UYA's hero never turns round.
    let mut rig = grinding(HeroProfile::uya(), vec![rail_along_minus_z(500.0)], 0.0);
    for _ in 0..60 {
        rig.tick_seen_from(PI, &input(None, &[], &[]));
        assert_eq!(rig.hero.state, HeroState::Grind);
    }
}

#[test]
fn deadlocked_rides_on_along_a_rail_that_starts_where_one_ends() {
    // The second rail starts 0.37 u from the first's end, inside DL's 0.5.
    let chained = || {
        vec![
            rail_along_minus_z(20.0),
            GrindRail::new(vec![
                Vec3::new(0.3, 2.1, -20.2),
                Vec3::new(0.3, 2.1, -200.0),
            ]),
        ]
    };
    let mut rig = grinding(HeroProfile::dl(), chained(), 0.0);
    for _ in 0..200 {
        rig.tick(&input(None, &[], &[]));
        assert_eq!(rig.hero.state, HeroState::Grind);
    }
    assert_eq!(rig.hero.rail.map(|at| at.rail), Some(1));
    assert!(rig.hero.speed > 11.0, "{}", rig.hero.speed);
    assert!(
        (rig.hero.position.x - 0.3).abs() < 0.01,
        "{}",
        rig.hero.position
    );

    // UYA's hero falls off the end.
    let mut rig = grinding(HeroProfile::uya(), chained(), 0.0);
    for _ in 0..200 {
        rig.tick(&input(None, &[], &[]));
        if rig.hero.state != HeroState::Grind {
            break;
        }
    }
    assert_eq!(rig.hero.state, HeroState::Fall);

    // So does DL's when the next rail starts 0.6 u away.
    let mut rig = grinding(
        HeroProfile::dl(),
        vec![
            rail_along_minus_z(20.0),
            GrindRail::new(vec![
                Vec3::new(0.6, 2.0, -20.0),
                Vec3::new(0.6, 2.0, -200.0),
            ]),
        ],
        0.0,
    );
    for _ in 0..200 {
        rig.tick(&input(None, &[], &[]));
        if rig.hero.state != HeroState::Grind {
            break;
        }
    }
    assert_eq!(rig.hero.state, HeroState::Fall);
}
