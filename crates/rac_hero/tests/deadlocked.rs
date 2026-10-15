//! The controller with Deadlocked's profile against our own DL NTSC-U traces
//! `dlntsc-dreadzone-moves-{1..5}` (PLAN.md RE.6; `docs/research/hero-controller.md`,
//! "Differences between the games"), and beside UYA's where the games differ.
//!
//! The traces show the pad one frame before the hero acts on it, so a press on trace row
//! n is a press on the tick of row n + 1 here.

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, HeroMotion, HeroProfile, HeroState, JumpKind, ParryWorld,
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
