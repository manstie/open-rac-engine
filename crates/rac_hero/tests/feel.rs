//! The controller against the figures in `docs/research/hero-controller.md` (PLAN.md RE.6).
//!
//! Each test drives `controller::step` on a flat floor with scripted input and checks a
//! number the research verified on UYA NTSC-U, converted to u/s (per-frame values × 60).

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, HeroMotion, HeroProfile, HeroState, JumpKind, ParryWorld,
    controller::step,
};
use rac_input::{Action, Buttons, PlayerInput};

/// Velocities in u/s are per-frame values × 60, so per-frame figures compare to 1e-3 u/s.
const TOLERANCE: f32 = 2e-3;

struct Rig {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
}

impl Rig {
    fn new() -> Self {
        let world = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(200.0, 0.5, 200.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        )]);
        let mut rig = Self {
            hero: HeroMotion::new(Vec3::ZERO, 0.0),
            profile: HeroProfile::uya(),
            world,
        };
        // Settle onto the floor.
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

    /// Runs forward at full speed.
    fn run_up(&mut self) {
        self.ticks(20, &input(Some([0, 127]), &[], &[]));
        assert_eq!(self.hero.speed, 6.17);
    }

    fn horizontal_speed(&self) -> f32 {
        Vec2::new(self.hero.velocity.x, self.hero.velocity.z).length()
    }

    /// Crouches long enough for a jump from the crouch to go (7 ticks, "Which jump a
    /// crouch gives").
    fn crouch(&mut self, stick: Option<[i8; 2]>) {
        self.tick(&input(stick, &[Action::Crouch], &[Action::Crouch]));
        self.ticks(6, &input(stick, &[Action::Crouch], &[]));
        assert_eq!(self.hero.state, HeroState::Crouch);
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
fn walking_reaches_6_17_at_37_per_second_squared() {
    // "Speeding up": 37 u/s², so top speed takes about 10 frames.
    let mut rig = Rig::new();
    let forward = input(Some([0, 127]), &[], &[]);
    for tick in 1..=10 {
        rig.tick(&forward);
        let expected = (37.0 / 60.0 * tick as f32).min(6.17);
        assert!(
            (rig.hero.speed - expected).abs() < TOLERANCE,
            "tick {tick}: {}",
            rig.hero.speed
        );
    }
    rig.tick(&forward);
    assert_eq!(rig.hero.speed, 6.17);
    assert_eq!(rig.hero.state, HeroState::Walk);
    // Facing −z, so he moved down −z.
    assert!(rig.hero.position.z < -0.5 && rig.hero.position.x.abs() < 1e-4);
    assert!(rig.hero.position.y.abs() < 1e-3, "stays on the floor");
}

#[test]
fn letting_go_skids_two_ticks_then_idles_from_1_5() {
    // "Letting go of the stick": two frames at 17 u/s², then idle at no more than 1.5 u/s,
    // which shrinks by 17.7 u/s² and stops about 6 frames later.
    let mut rig = Rig::new();
    rig.run_up();
    let release = input(None, &[], &[]);
    rig.tick(&release);
    assert!((rig.hero.speed - (6.17 - 17.0 / 60.0)).abs() < TOLERANCE);
    rig.tick(&release);
    assert!((rig.hero.speed - (6.17 - 2.0 * 17.0 / 60.0)).abs() < TOLERANCE);
    rig.tick(&release);
    assert_eq!(rig.hero.state, HeroState::Idle);
    assert_eq!(rig.hero.speed, 1.5);
    let mut ticks_to_stop = 0;
    while rig.hero.speed > 0.0 {
        rig.tick(&release);
        ticks_to_stop += 1;
    }
    assert_eq!(ticks_to_stop, 6);
}

/// Presses jump, holds it for `hold_ticks` in all (the press counts), then lets go, and
/// returns the highest the feet got above the start.
fn jump_apex(rig: &mut Rig, held: &[Action], hold_ticks: usize) -> f32 {
    jump_apex_with(rig, None, held, hold_ticks)
}

fn jump_apex_with(
    rig: &mut Rig,
    stick: Option<[i8; 2]>,
    held: &[Action],
    hold_ticks: usize,
) -> f32 {
    let start = rig.hero.position.y;
    let mut apex: f32 = 0.0;
    rig.tick(&input(stick, held, &[Action::Jump]));
    for tick in 1..240 {
        let mut keys = held.to_vec();
        if tick < hold_ticks {
            keys.push(Action::Jump);
        }
        rig.tick(&input(stick, &keys, &[]));
        apex = apex.max(rig.hero.position.y - start);
        if !rig.hero.state.is_airborne() {
            break;
        }
    }
    apex
}

#[test]
fn a_held_jump_takes_off_at_the_documented_speed() {
    // "Vertical model": with X held, h climbs from 1.42 by 0.075294 per frame, and the
    // take-off speed is −0.01333 − 0.00825 + 0.16098 = 0.13940 u per frame.
    let mut rig = Rig::new();
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Standing));
    assert!(
        (rig.hero.velocity.y - (-0.013_333 * 60.0)).abs() < TOLERANCE,
        "push-off"
    );
    rig.tick(&input(None, &[Action::Jump], &[]));
    assert!(
        (rig.hero.velocity.y - 0.139_40 * 60.0).abs() < 0.01,
        "{}",
        rig.hero.velocity.y
    );
}

// Apex heights measured in our own UYA NTSC trace `uyantsc-veldin-moves-1` (frames 165,
// 82, 619 and 1158; `docs/research/hero-controller.md`, "Jump heights"). The hold length
// is read from the trace's jump block: how many steps the target height took.

#[test]
fn a_fully_held_jump_peaks_where_the_game_does() {
    let apex = jump_apex(&mut Rig::new(), &[], 17);
    assert!((apex - 1.8867).abs() < 0.002, "apex {apex}");
}

#[test]
fn a_three_frame_tap_peaks_where_the_game_does() {
    let apex = jump_apex(&mut Rig::new(), &[], 3);
    assert!((apex - 1.3115).abs() < 0.002, "apex {apex}");
}

#[test]
fn a_heli_jump_peaks_where_the_game_does() {
    let mut rig = Rig::new();
    rig.crouch(None);
    let apex = jump_apex(&mut rig, &[Action::Crouch], 3);
    assert!((apex - 3.4327).abs() < 0.005, "apex {apex}");
}

#[test]
fn a_long_jump_peaks_where_the_game_does() {
    // The trace's long jump rose 1.7037 u, but part of that is the capsule riding up
    // uneven ground mid-flight (its actual vertical speed exceeds the intended one on
    // frames 7–12). The game's intended vertical velocity, summed over the flight, peaks
    // at 1.42717 u, which is what the jump model alone gives.
    let mut rig = Rig::new();
    rig.run_up();
    rig.crouch(Some([0, 127]));
    let apex = jump_apex_with(&mut rig, Some([0, 127]), &[Action::Crouch], 5);
    assert!((apex - 1.42717).abs() < 0.002, "apex {apex}");
}

#[test]
fn landing_returns_to_the_ground() {
    let mut rig = Rig::new();
    rig.tick(&input(None, &[], &[Action::Jump]));
    for _ in 0..120 {
        rig.tick(&input(None, &[], &[]));
    }
    assert!(!rig.hero.state.is_airborne(), "{:?}", rig.hero.state);
    assert!(rig.hero.position.y.abs() < 1e-3);
}

#[test]
fn the_double_jump_climbs_by_0_0334_per_frame_to_8_75() {
    // "Double jump": gravity, then +150 u/s² towards 8.75 u/s, for 8 frames.
    let mut rig = Rig::new();
    rig.tick(&input(None, &[], &[Action::Jump]));
    // Rise to the apex of a tapped jump, where the vertical speed is about zero.
    rig.ticks(2, &input(None, &[], &[]));
    while rig.hero.velocity.y > 0.0 {
        rig.tick(&input(None, &[], &[]));
    }
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::DoubleJump);
    let mut previous = rig.hero.velocity.y;
    let step = (150.0 - 29.7) / 60.0;
    for _ in 1..8 {
        rig.tick(&input(None, &[Action::Jump], &[]));
        let rise = rig.hero.velocity.y - previous;
        if rig.hero.velocity.y < 8.75 {
            assert!((rise - step).abs() < TOLERANCE, "rise {rise}");
        }
        previous = rig.hero.velocity.y;
    }
    assert!(
        (rig.hero.velocity.y - 8.75).abs() < TOLERANCE,
        "{}",
        rig.hero.velocity.y
    );
}

#[test]
fn holding_after_a_double_jump_glides_down_at_2_16() {
    // "Glide": entered holding X past the double jump's 30th tick; the sink then eases
    // towards 3 × 0.72 = 2.16 u/s and holds it.
    let mut rig = Rig::new();
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(16, &input(None, &[Action::Jump], &[]));
    while rig.hero.velocity.y > 0.5 {
        rig.tick(&input(None, &[], &[]));
    }
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::DoubleJump);
    let hold = input(None, &[Action::Jump], &[]);
    while rig.hero.state == HeroState::DoubleJump {
        rig.tick(&hold);
    }
    assert_eq!(rig.hero.state, HeroState::Glide);
    for _ in 0..200 {
        rig.tick(&hold);
        if rig.hero.state == HeroState::Glide && rig.hero.ticks > 60 {
            break;
        }
    }
    assert_eq!(rig.hero.state, HeroState::Glide);
    assert!(
        (rig.hero.velocity.y + 2.16).abs() < 0.01,
        "{}",
        rig.hero.velocity.y
    );
}

#[test]
fn a_long_jump_from_a_full_run_leaves_at_9_7() {
    // "Long jump": from a full run the push-off ends at 0.1617 u per frame (9.7 u/s), and
    // the flight keeps that velocity with no air control.
    // Jumping and crouching together keeps the run's speed: the run jump turns into a
    // long jump within its first 3 ticks ("Which jump a crouch gives").
    let mut rig = Rig::new();
    rig.run_up();
    rig.tick(&input(
        Some([0, 127]),
        &[Action::Crouch],
        &[Action::Jump, Action::Crouch],
    ));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Long));
    rig.ticks(4, &input(Some([0, 127]), &[Action::Jump], &[]));
    assert!(
        (rig.horizontal_speed() - 0.1617 * 60.0).abs() < 0.02,
        "{}",
        rig.horizontal_speed()
    );
    let take_off = rig.hero.velocity;

    // Steering sideways changes nothing in the air.
    rig.ticks(20, &input(Some([127, 0]), &[], &[]));
    assert!((rig.hero.velocity.x - take_off.x).abs() < 1e-5);
    assert!((rig.hero.velocity.z - take_off.z).abs() < 1e-5);
}

#[test]
fn a_strafe_flip_moves_sideways_at_5_5() {
    // "Flips": in the stick's direction at max(2, 5.5 × stick) u/s, at speed at once.
    let mut rig = Rig::new();
    rig.tick(&input(Some([-127, 0]), &[Action::Strafe], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Flip));
    assert!(
        (rig.hero.velocity.x + 5.5).abs() < TOLERANCE,
        "{}",
        rig.hero.velocity
    );
    assert!(rig.hero.velocity.z.abs() < 1e-4);
}

#[test]
fn the_heli_jump_rises_by_0_00536_per_frame_under_thrust() {
    // "Heli jump thrust": on frames 35–70 the vertical speed rises by 49/3600 − 29.7/3600.
    let mut rig = Rig::new();
    rig.crouch(None);
    rig.tick(&input(None, &[Action::Crouch], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Heli));
    let hold = input(None, &[Action::Jump], &[]);
    while rig.hero.ticks < 36 {
        rig.tick(&hold);
    }
    let before = rig.hero.velocity.y;
    rig.tick(&hold);
    let rise = rig.hero.velocity.y - before;
    assert!((rise - 0.005_361 * 60.0).abs() < TOLERANCE, "rise {rise}");
}

#[test]
fn air_control_steers_at_20_per_second_squared() {
    // "Air control": 20 u/s² towards the stick at the air speed cap.
    let mut rig = Rig::new();
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.tick(&input(None, &[], &[]));
    let steer = input(Some([0, 127]), &[], &[]);
    rig.tick(&steer);
    let first = rig.horizontal_speed();
    rig.tick(&steer);
    let second = rig.horizontal_speed();
    assert!(
        (second - first - 20.0 / 60.0).abs() < TOLERANCE,
        "{first} → {second}"
    );
}

#[test]
fn walking_off_an_edge_falls_at_22_per_second_squared() {
    // "Falling": gravity 22 u/s².
    let world = ParryWorld::new(&[BodyPlacement::fixed(
        Collider::Cuboid {
            half_extents: Vec3::new(1.0, 5.0, 1.0),
        },
        Vec3::new(0.0, -5.0, 0.0),
        Quat::IDENTITY,
    )]);
    let profile = HeroProfile::uya();
    let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
    let forward = input(Some([0, 127]), &[], &[]);
    while hero.state != HeroState::Fall {
        step(&mut hero, &forward, 0.0, &profile, &world);
        assert!(hero.position.z > -5.0, "never left the block");
    }
    let before = hero.velocity.y;
    step(&mut hero, &forward, 0.0, &profile, &world);
    assert!((hero.velocity.y - before + 22.0 / 60.0).abs() < TOLERANCE);
}

#[test]
fn the_same_inputs_give_the_same_motion() {
    // Determinism: two runs of one script end bit-identical.
    let script: Vec<PlayerInput> = (0..300)
        .map(|tick| {
            let stick = [((tick * 7) % 255) as i8, ((tick * 13) % 255) as i8];
            let pressed = if tick % 37 == 0 {
                &[Action::Jump][..]
            } else {
                &[]
            };
            input(Some(stick), &[Action::Jump], pressed)
        })
        .collect();
    let run = || {
        let mut rig = Rig::new();
        for input in &script {
            rig.tick(input);
        }
        (
            rig.hero.position,
            rig.hero.velocity,
            rig.hero.yaw,
            rig.hero.state,
        )
    };
    assert_eq!(run(), run());
}
