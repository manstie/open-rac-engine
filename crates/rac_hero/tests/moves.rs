//! P1.5's second chunk against `docs/research/hero-controller.md`: the wall jump, ledges,
//! getting hit, when each jump can start, the charge boots and the ability flags. These
//! rules are read in UYA's and DL's code but not yet traced, so the tests check the code's
//! numbers.

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, GrindRail, HeroMotion, HeroProfile, HeroState, JumpKind, Material,
    ParryWorld, Side,
    controller::{self, Abilities, StepEvents, TICK, step},
    motor,
    water::WaterVolume,
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
        look_stick: [0, 0],
        held: buttons(held) | buttons(pressed),
        pressed: buttons(pressed),
    }
}

fn idle() -> PlayerInput {
    input(None, &[], &[])
}

fn block(centre: Vec3, half_extents: Vec3, material: Material) -> BodyPlacement {
    BodyPlacement {
        material,
        ..BodyPlacement::fixed(Collider::Cuboid { half_extents }, centre, Quat::IDENTITY)
    }
}

fn floor() -> BodyPlacement {
    block(
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(100.0, 0.5, 100.0),
        Material::Solid,
    )
}

struct Rig {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
}

impl Rig {
    fn new(bodies: &[BodyPlacement], position: Vec3, yaw: f32) -> Self {
        let mut rig = Self {
            hero: HeroMotion::new(position, yaw),
            profile: HeroProfile::uya(),
            world: ParryWorld::new(bodies),
        };
        rig.tick(&idle());
        rig
    }

    fn tick(&mut self, input: &PlayerInput) -> StepEvents {
        step(&mut self.hero, input, 0.0, &self.profile, &self.world)
    }

    fn ticks(&mut self, count: usize, input: &PlayerInput) {
        for _ in 0..count {
            self.tick(input);
        }
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

// --- Wall jump -------------------------------------------------------------------------

/// Two walls facing each other across x = ±1.5, 15 u tall, with the hero between them
/// facing the right one (+x).
fn shaft(material: Material) -> Rig {
    let wall = |x: f32| block(Vec3::new(x, 7.5, 0.0), Vec3::new(0.5, 7.5, 5.0), material);
    Rig::new(
        &[floor(), wall(2.0), wall(-2.0)],
        Vec3::new(1.0, 0.0, 0.0),
        -FRAC_PI_2,
    )
}

#[test]
fn deadlocked_never_wall_jumps() {
    // "Deadlocked's moves": DL's code never enters state 17, even off wall-jump surfaces.
    for material in [Material::Solid, Material::WallJump] {
        let mut rig = shaft(material);
        rig.profile = HeroProfile::dl();
        let press = input(None, &[], &[Action::Jump]);
        for _ in 0..90 {
            rig.tick(&press);
            assert_ne!(
                rig.hero.state,
                HeroState::Jump(JumpKind::Wall),
                "{material:?}"
            );
        }
    }
}

#[test]
fn a_wall_jump_clings_then_leaves_along_the_wall_normal() {
    // "Wall jump": 9 push-off ticks on the wall, pressed down at the push-off rate, then
    // 4.9 u/s along the wall's normal; the target height off an ordinary wall is 1.8 u.
    let mut rig = shaft(Material::Solid);
    let press = input(None, &[], &[Action::Jump]);
    rig.tick(&press);
    rig.until(30, &press, |hero| {
        hero.state == HeroState::Jump(JumpKind::Wall)
    });
    for tick in 0..9 {
        assert!(
            (rig.hero.velocity.y + 48.0 / 60.0).abs() < 1e-3,
            "tick {tick}: {}",
            rig.hero.velocity.y
        );
        rig.tick(&idle());
    }
    rig.tick(&idle());
    assert!(
        (rig.hero.velocity.x + 4.9).abs() < 1e-3 && rig.hero.velocity.z.abs() < 1e-3,
        "{}",
        rig.hero.velocity
    );
    assert!(rig.hero.velocity.y > 0.0, "launched upwards");
}

/// Presses jump every tick in the shaft, counting wall jumps, until he lands or `stop`.
/// Returns the count and the highest the feet got.
fn press_up_the_shaft(rig: &mut Rig, stop: impl Fn(&HeroMotion) -> bool) -> (usize, f32) {
    let press = input(None, &[], &[Action::Jump]);
    rig.tick(&press);
    let mut jumps = 0;
    let mut highest: f32 = 0.0;
    for _ in 0..1200 {
        rig.tick(&press);
        // A wall jump's first tick leaves its tick count at 1, even straight after another.
        if rig.hero.state == HeroState::Jump(JumpKind::Wall) && rig.hero.ticks == 1 {
            jumps += 1;
        }
        highest = highest.max(rig.hero.position.y);
        if (rig.hero.ground.is_some() && jumps > 0) || stop(&rig.hero) {
            break;
        }
    }
    (jumps, highest)
}

#[test]
fn ordinary_walls_allow_two_wall_jumps_in_a_row() {
    let (jumps, _) = press_up_the_shaft(&mut shaft(Material::Solid), |_| false);
    assert_eq!(jumps, 2);
}

#[test]
fn wall_jump_surfaces_chain_and_climb() {
    // Material 15: unlimited, and 3.3–3.35 u each.
    let (jumps, highest) = press_up_the_shaft(&mut shaft(Material::WallJump), |hero| {
        hero.position.y > 12.0
    });
    assert!(
        highest > 12.0,
        "only reached {highest} in {jumps} wall jumps"
    );
    assert!(jumps >= 3, "{jumps} wall jumps");
}

// --- Ledges ----------------------------------------------------------------------------

/// A block 3 u tall whose face is at z = −2 (facing +z), with the hero 0.5 u in front of
/// it, facing it.
fn ledge_rig() -> Rig {
    Rig::new(
        &[
            floor(),
            block(
                Vec3::new(0.0, 1.5, -4.0),
                Vec3::new(2.0, 1.5, 2.0),
                Material::Solid,
            ),
        ],
        Vec3::new(0.0, 0.0, -1.5),
        0.0,
    )
}

/// Jumps at the ledge with X held and waits until he hangs.
fn hang(rig: &mut Rig) {
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.until(60, &input(None, &[Action::Jump], &[]), |hero| {
        hero.state == HeroState::LedgeGrab
    });
    rig.until(30, &idle(), |hero| hero.state == HeroState::LedgeHang);
    rig.ticks(10, &idle());
}

#[test]
fn a_jump_grabs_a_ledge_and_hangs_below_its_lip() {
    // "Ledges": the feet hang 1.43 u below the lip and 0.5 u out from the wall, facing it.
    let mut rig = ledge_rig();
    hang(&mut rig);
    let ledge = rig.hero.ledge.expect("holds a ledge");
    assert!((ledge.lip.y - 3.0).abs() < 1e-3, "{}", ledge.lip);
    assert!((ledge.lip.z + 2.0).abs() < 0.04, "{}", ledge.lip);
    assert!((ledge.normal - Vec3::Z).length() < 1e-3);
    let expected = ledge.lip + Vec3::Z * 0.5 - Vec3::Y * 1.43;
    assert!(
        (rig.hero.position - expected).length() < 0.01,
        "{} vs {expected}",
        rig.hero.position
    );
    assert!(
        rig.hero.yaw.abs() < 0.01,
        "faces the wall: {}",
        rig.hero.yaw
    );
}

#[test]
fn x_climbs_onto_the_ledge() {
    let mut rig = ledge_rig();
    hang(&mut rig);
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Ledge));
    rig.until(120, &idle(), |hero| hero.ground.is_some());
    assert!(rig.hero.position.y > 2.9, "{}", rig.hero.position);
    assert!(rig.hero.position.z < -1.7, "{}", rig.hero.position);
}

#[test]
fn crouch_lets_go_and_the_hero_falls() {
    let mut rig = ledge_rig();
    hang(&mut rig);
    rig.tick(&input(None, &[Action::Crouch], &[Action::Crouch]));
    assert_eq!(rig.hero.state, HeroState::Fall);
    rig.until(120, &idle(), |hero| hero.ground.is_some());
    assert!(rig.hero.position.y.abs() < 1e-3);
}

#[test]
fn a_shimmy_steps_like_the_trace_and_finishes_its_cycle() {
    // "Ledges", shimmy [verified on UYA NTSC]: still for nine ticks, then the steps the
    // trace shows, and a whole cycle of 0.623 u even when the stick lets go part-way.
    let mut rig = ledge_rig();
    hang(&mut rig);
    let start = rig.hero.position;
    // Facing −z with the camera behind, stick left is −x.
    let left = input(Some([-127, 0]), &[], &[]);
    rig.tick(&left);
    assert_eq!(rig.hero.state, HeroState::LedgeShimmy(Side::Left));
    let mut steps = Vec::new();
    for _ in 0..18 {
        let before = rig.hero.position;
        rig.tick(&left);
        steps.push(rig.hero.position.distance(before));
    }
    let mut expected = vec![0.0; 9];
    expected.extend([
        0.0125, 0.0125, 0.025, 0.025, 0.0375, 0.0375, 0.081, 0.081, 0.081,
    ]);
    for (tick, (step, want)) in steps.iter().zip(&expected).enumerate() {
        assert!(
            (step - want).abs() < 1e-4,
            "tick {}: {step} not {want}",
            tick + 1
        );
    }
    rig.until(30, &idle(), |hero| hero.state == HeroState::LedgeHang);
    let moved = start.x - rig.hero.position.x;
    assert!((moved - 0.623).abs() < 3e-3, "moved {moved}");
    assert!((rig.hero.position.y - start.y).abs() < 1e-3);
    // It settles a skin's width nearer the wall than the hang it started from.
    assert!(
        (rig.hero.position.z - start.z).abs() < 0.02,
        "{start} {}",
        rig.hero.position
    );
}

#[test]
fn the_climb_moves_forward_from_its_fifth_tick() {
    // "Ledges", ledge jump [verified on UYA NTSC]: still for four ticks, then towards
    // 2 u/s at 8 u/s², which it keeps until he lands.
    let mut rig = ledge_rig();
    hang(&mut rig);
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Ledge));
    // The speed, not the motion: the kit's capsule keeps a skin's width from the lip, so
    // it rubs it for a few ticks where the game's hero passes clear.
    for tick in 1..=30u32 {
        rig.tick(&idle());
        let speed = Vec3::new(rig.hero.velocity.x, 0.0, rig.hero.velocity.z).length();
        let want = (tick.saturating_sub(4) as f32 * 8.0 / 60.0).min(2.0);
        assert!(
            (speed - want).abs() < 1e-4,
            "tick {tick}: {speed} not {want}"
        );
    }
}

#[test]
fn ledges_too_low_to_hang_from_are_not_found() {
    // The top must be 0.5–1.5 u above the feet, and 1.8 u above the ground.
    let rig = ledge_rig();
    let profile = &rig.profile;
    let world = &rig.world;
    let found = |feet: Vec3, height: f32| {
        controller::detect_ledge(world, profile, feet, 0.0, height).is_some()
    };
    assert!(found(Vec3::new(0.0, 1.6, -1.5), 1.6));
    assert!(!found(Vec3::new(0.0, 1.4, -1.5), 1.4), "too high above");
    assert!(!found(Vec3::new(0.0, 2.6, -1.5), 2.6), "too little rise");
    // From the floor, the top is 3 u up: out of reach.
    assert!(!found(Vec3::new(0.0, 0.0, -1.5), 0.0));
}

#[test]
fn deadlocked_finds_ledges_further_out() {
    // "Deadlocked's ledges": DL casts for the top once, 1.05 u ahead of the feet, where
    // UYA steps from 0.525 to 0.75 u. With the face 1 u ahead only DL's cast is over the
    // top; with it 0.5 u ahead both are.
    let rig = ledge_rig();
    let found = |profile: &HeroProfile, z: f32| {
        controller::detect_ledge(&rig.world, profile, Vec3::new(0.0, 1.6, z), 0.0, 1.6).is_some()
    };
    let (uya, dl) = (HeroProfile::uya(), HeroProfile::dl());
    assert!(found(&dl, -1.0));
    assert!(!found(&uya, -1.0));
    assert!(found(&dl, -1.5) && found(&uya, -1.5));
}

fn dl_ledge_rig() -> Rig {
    let mut rig = ledge_rig();
    rig.profile = HeroProfile::dl();
    rig
}

#[test]
fn deadlocked_hangs_against_the_wall_short_of_its_hang_point() {
    // "Deadlocked's ledges" [verified on DL NTSC]: the lip stays where the cast hit the
    // top, 1.05 u ahead of the feet, so the hang point is 0.55 u ahead of his axis and the
    // wall holds him at its face. The grab hangs on its 24th tick.
    let mut rig = dl_ledge_rig();
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.until(60, &input(None, &[Action::Jump], &[]), |hero| {
        hero.state == HeroState::LedgeGrab
    });
    // The kit grabs at the end of the tick before the game's first grab frame.
    let grab = rig.until(40, &idle(), |hero| hero.state == HeroState::LedgeHang);
    assert_eq!(grab, 24);
    rig.ticks(60, &idle());
    let ledge = rig.hero.ledge.expect("holds a ledge");
    assert!(
        (ledge.lip - Vec3::new(0.0, 3.0, -2.55)).length() < 0.01,
        "{}",
        ledge.lip
    );
    assert!((ledge.hang - Vec3::new(0.0, 1.57, -2.05)).length() < 0.01);
    let position = rig.hero.position;
    assert!((position.y - 1.57).abs() < 0.01, "{position}");
    assert!(
        (position.z + 2.0 - 0.45).abs() < 0.02,
        "at the face: {position}"
    );
}

#[test]
fn deadlocked_shimmies_along_the_wall_at_about_uyas_pace() {
    // "Deadlocked's ledges" [verified on DL NTSC]: 1.5 × the table towards the ledge 0.2 u
    // to the side, whose hang point is 0.55 u ahead of his axis. The wall takes the rest,
    // so along it each step is 0.2 / |(0.2, 0.55)| of that, and the steps back pull him
    // 0.29 u off it. The animation runs from the first tick and a cycle is 32 ticks:
    // 0.60 u (the trace: 0.605 and 0.600).
    let mut rig = dl_ledge_rig();
    hang(&mut rig);
    rig.ticks(50, &idle());
    let left = input(Some([-127, 0]), &[], &[]);
    rig.tick(&left);
    assert_eq!(rig.hero.state, HeroState::LedgeShimmy(Side::Left));
    let start = rig.hero.position;
    let along = 0.2 / 0.2f32.hypot(0.55);
    let steps = rig.profile.ledge.shimmy_steps.clone();
    let mut furthest_out: f32 = 0.0;
    for tick in 1..=32 {
        let before = rig.hero.position.x;
        rig.tick(&left);
        let entry = steps.get(tick / 2).copied().unwrap_or(0.0);
        let want = -1.5 * entry * along;
        let moved = rig.hero.position.x - before;
        assert!(
            (moved - want).abs() < 1e-3,
            "tick {tick}: {moved} not {want}"
        );
        furthest_out = furthest_out.max(rig.hero.position.z - start.z);
    }
    assert_eq!(rig.hero.state, HeroState::LedgeShimmy(Side::Left));
    let cycle = start.x - rig.hero.position.x;
    assert!((cycle - 0.603).abs() < 0.005, "a cycle covers {cycle}");
    assert!((furthest_out - 0.293).abs() < 0.01, "{furthest_out}");
}

#[test]
fn deadlocked_climbs_forward_from_its_first_tick() {
    // "Deadlocked's ledges", ledge jump [verified on DL NTSC]: off on the 4th tick and up
    // 1.967 u from the hanging feet; the forward speed rises from the first tick at
    // 8 u/s², 2 u/s on the 15th.
    let mut rig = dl_ledge_rig();
    hang(&mut rig);
    rig.ticks(50, &idle());
    let feet = rig.hero.position.y;
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Ledge));
    let mut highest = feet;
    for tick in 1..=30u32 {
        let speed = Vec3::new(rig.hero.velocity.x, 0.0, rig.hero.velocity.z).length();
        let want = ((tick as f32) * 8.0 / 60.0).min(2.0);
        assert!(
            (speed - want).abs() < 1e-4,
            "tick {tick}: {speed} not {want}"
        );
        let off = rig.hero.position.y > feet + 1e-4;
        assert_eq!(off, tick > 3, "tick {tick}: {}", rig.hero.position.y - feet);
        rig.tick(&idle());
        highest = highest.max(rig.hero.position.y);
    }
    assert!(
        ((highest - feet) - 1.967).abs() < 0.01,
        "{}",
        highest - feet
    );
}

// --- Hits, hazards and death -----------------------------------------------------------

#[test]
fn a_hit_knocks_back_at_most_7_and_ends_after_28_ticks() {
    // "Getting hit and dying".
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    controller::hurt(
        &mut rig.hero,
        Vec3::new(0.0, 0.0, 12.0),
        false,
        &rig.profile,
    );
    assert_eq!(rig.hero.state, HeroState::Hit);
    assert!((rig.hero.velocity.length() - 7.0).abs() < 1e-4);
    assert!(!rig.hero.can_be_hurt());
    rig.tick(&idle());
    // 4 u/s² at first.
    assert!(
        (rig.hero.velocity.z - (7.0 - 4.0 / 60.0)).abs() < 1e-4,
        "{}",
        rig.hero.velocity
    );
    rig.until(40, &idle(), |hero| hero.state != HeroState::Hit);
    assert_eq!(rig.hero.state, HeroState::Idle);
    // Safe for 77 ticks from the hit.
    rig.ticks(60, &idle());
    assert!(rig.hero.can_be_hurt());
}

#[test]
fn a_fatal_hit_ends_in_death_and_a_respawn() {
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    controller::hurt(&mut rig.hero, Vec3::new(0.0, 4.0, 3.0), true, &rig.profile);
    rig.until(120, &idle(), |hero| hero.state == HeroState::Dead);
    assert!(!rig.hero.respawn_due(&rig.profile));
    rig.ticks(rig.profile.death.ticks as usize, &idle());
    assert!(rig.hero.respawn_due(&rig.profile));
}

#[test]
fn hazard_surfaces_report_and_deadly_ones_kill() {
    let pad = |material| {
        Rig::new(
            &[block(
                Vec3::new(0.0, -0.5, 0.0),
                Vec3::new(5.0, 0.5, 5.0),
                material,
            )],
            Vec3::ZERO,
            0.0,
        )
    };
    let mut rig = pad(Material::Hazard);
    assert!(rig.tick(&idle()).hazard);
    let mut rig = pad(Material::Deadly);
    rig.tick(&idle());
    assert_eq!(rig.hero.state, HeroState::Dead);
    let mut rig = pad(Material::Solid);
    assert!(!rig.tick(&idle()).hazard);
}

// --- When jumps can start -------------------------------------------------------------

#[test]
fn a_jump_from_walking_is_a_run_jump_only_past_0_85_stick() {
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.ticks(20, &input(Some([0, 80]), &[], &[]));
    rig.tick(&input(Some([0, 80]), &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Standing));

    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.ticks(20, &input(Some([0, 127]), &[], &[]));
    rig.tick(&input(Some([0, 127]), &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Run));
}

#[test]
fn a_jump_from_a_crouch_waits_for_its_seventh_tick() {
    // "Which jump a crouch gives": the press is kept until the crouch is 7 ticks old.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.tick(&input(None, &[Action::Crouch], &[Action::Crouch]));
    rig.tick(&input(None, &[Action::Crouch], &[]));
    rig.tick(&input(None, &[Action::Crouch], &[Action::Jump]));
    let crouched = input(None, &[Action::Crouch], &[]);
    let waited = rig.until(10, &crouched, |hero| hero.state != HeroState::Crouch);
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Heli));
    // The crouch began 3 ticks before the press; the jump goes once it is 7 ticks old.
    assert_eq!(waited, 5);
}

#[test]
fn a_long_jump_needs_the_stick_and_some_speed() {
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.ticks(20, &input(Some([0, 127]), &[], &[]));
    let forward = Some([0, 127]);
    rig.tick(&input(forward, &[Action::Crouch], &[Action::Crouch]));
    rig.ticks(6, &input(forward, &[Action::Crouch], &[]));
    // 6.17 u/s less 7 ticks at 27 u/s² is still well over 0.9.
    rig.tick(&input(forward, &[Action::Crouch], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Long));
}

#[test]
fn the_double_jump_waits_for_the_jumps_fifteenth_tick() {
    // "Double jump": t > 14, with a press from the last t − 4 ticks.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(4, &input(None, &[Action::Jump], &[]));
    // Pressed on the jump's 6th tick: kept, and used on the 16th (t = 15).
    rig.tick(&input(None, &[], &[Action::Jump]));
    let waited = rig.until(20, &idle(), |hero| hero.state == HeroState::DoubleJump);
    assert_eq!(rig.hero.ticks, 1);
    assert_eq!(waited, 10);

    // A press in the first 4 ticks never counts.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.tick(&input(None, &[], &[]));
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(40, &idle());
    assert_ne!(rig.hero.state, HeroState::DoubleJump);
}

#[test]
fn a_press_just_before_landing_jumps_on_landing() {
    // On the ground a press is kept for 7 ticks (9 walking).
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(5, &idle());
    rig.until(120, &idle(), |hero| {
        hero.position.y < 0.2 && hero.velocity.y < 0.0
    });
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.until(10, &idle(), |hero| !hero.state.is_airborne());
    rig.tick(&idle());
    assert!(rig.hero.state.is_airborne(), "{:?}", rig.hero.state);
}

#[test]
fn the_slope_limit_is_50_degrees() {
    // "Slope limit": standing ground is at most 0.8727 rad steep.
    let body = HeroProfile::uya().body;
    let normal = |degrees: f32| Quat::from_rotation_x(degrees.to_radians()) * Vec3::Y;
    assert!(motor::is_walkable(normal(49.9), &body));
    assert!(!motor::is_walkable(normal(50.1), &body));
}

// --- Walking: skids and slides --------------------------------------------------------

#[test]
fn reversing_the_stick_at_a_run_skids_at_17() {
    // "Ground movement", the walk's skid: a full stick swung more than 110° in 6 ticks,
    // more than 70° from the facing, brakes at 17 u/s² instead of 30 while he turns.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.ticks(30, &input(FORWARD, &[], &[]));
    assert!((rig.hero.speed - 6.17).abs() < 1e-4);
    let back = input(Some([0, -127]), &[], &[]);
    rig.tick(&back);
    assert!((rig.hero.speed - (6.17 - 17.0 * TICK)).abs() < 1e-4);
    // Turned round, he runs the other way.
    rig.ticks(90, &back);
    assert!(rig.hero.velocity.z > 6.0, "{}", rig.hero.velocity);

    // Pulling the stick back only halfway is no skid: the speed falls at 30 u/s².
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.ticks(30, &input(FORWARD, &[], &[]));
    rig.tick(&input(Some([0, 86]), &[], &[]));
    assert!((rig.hero.speed - (6.17 - 30.0 * TICK)).abs() < 1e-4);
}

#[test]
fn a_slow_hero_moves_the_way_the_stick_points() {
    // Under 1.7 u/s the walk goes the stick's way while he turns to face it.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.tick(&input(Some([127, 0]), &[], &[]));
    let velocity = rig.hero.velocity;
    assert!(velocity.x > 0.0 && velocity.z.abs() < 1e-6, "{velocity}");
    assert!(rig.hero.yaw.abs() < 0.2);
}

#[test]
fn a_part_pushed_stick_far_round_turns_before_speeding_up() {
    // Under 0.9 of the stick and more than 45° round, the speed doesn't rise.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    let half_right = input(Some([86, 0]), &[], &[]);
    rig.tick(&half_right);
    assert_eq!(rig.hero.speed, 0.0);
    let waited = rig.until(60, &half_right, |hero| hero.speed > 0.0);
    // By then he faces within 45° of the stick (yaw −π/2).
    assert!(
        (rig.hero.yaw + FRAC_PI_2).abs() <= 45f32.to_radians(),
        "{}",
        rig.hero.yaw
    );
    assert!(waited > 3, "{waited}");
}

// --- Charge boots ----------------------------------------------------------------------

const FORWARD: Option<[i8; 2]> = Some([0, 127]);

/// Walks the hero up to speed, then taps crouch twice, `gap` ticks apart.
fn double_tap(rig: &mut Rig, gap: usize) {
    rig.ticks(20, &input(FORWARD, &[], &[]));
    rig.tick(&input(FORWARD, &[], &[Action::Crouch]));
    rig.ticks(gap - 1, &input(FORWARD, &[], &[]));
    rig.tick(&input(FORWARD, &[], &[Action::Crouch]));
}

#[test]
fn a_second_crouch_tap_within_15_ticks_charges() {
    // "Charge boots": R1 pressed again while the 15-frame window from the last press runs.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    double_tap(&mut rig, 15);
    assert_eq!(rig.hero.state, HeroState::Charge);

    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    double_tap(&mut rig, 16);
    assert_ne!(rig.hero.state, HeroState::Charge);
}

#[test]
fn a_charge_dashes_at_30_u_s_for_a_second_then_skids() {
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    double_tap(&mut rig, 3);
    assert_eq!(rig.hero.state, HeroState::Charge);
    let start = rig.hero.speed;
    rig.tick(&idle());
    // Up at 100 u/s² towards 30 u/s, along the facing (-z).
    assert!((rig.hero.speed - (start + 100.0 * TICK)).abs() < 1e-4);
    rig.ticks(30, &idle());
    assert_eq!(rig.hero.speed, 30.0);
    // The step limit holds the motion to 0.43 u a tick, 25.8 u/s (DL's trace).
    let max = 0.43 / TICK;
    assert!(
        (rig.hero.velocity.z + max).abs() < 1e-3,
        "{}",
        rig.hero.velocity
    );

    // 61 ticks in all (frames 0-60); the 62nd skids.
    let rest = rig.until(40, &idle(), |hero| hero.state != HeroState::Charge);
    assert_eq!(1 + 1 + 30 + rest, 62);
    assert_eq!(rig.hero.state, HeroState::Skid);
    // The skid slides on the capped motion, braking at 27 u/s².
    assert!((rig.hero.speed - (max - 27.0 * TICK)).abs() < 1e-3);

    // It ignores the controls for 30 ticks and doesn't turn...
    let (before, yaw) = (rig.hero.speed, rig.hero.yaw);
    rig.tick(&input(Some([127, 0]), &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Skid);
    assert_eq!(rig.hero.yaw, yaw);
    assert!((before - rig.hero.speed - 27.0 * TICK).abs() < 1e-4);
    // ...and the slide stops when the controls come back, 30 ticks after the charge.
    let stopped = rig.until(40, &idle(), |hero| hero.state == HeroState::Idle);
    assert_eq!(2 + stopped, 31);
}

#[test]
fn a_charge_steers_like_a_car_with_the_stick() {
    // Full right turns right at 70°/s once the 50-tick ramp is over.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    double_tap(&mut rig, 3);
    let right = input(Some([127, 0]), &[], &[]);
    rig.ticks(50, &right);
    let yaw = rig.hero.yaw;
    rig.tick(&right);
    let turned = rig.hero.yaw - yaw;
    assert!((turned + 1.2217305 * TICK).abs() < 1e-5, "{turned}");

    // Early in the charge the same stick turns far less.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    double_tap(&mut rig, 3);
    let yaw = rig.hero.yaw;
    rig.ticks(10, &right);
    assert!((rig.hero.yaw - yaw).abs() < 0.03);
}

#[test]
fn a_charge_into_a_wall_bounces_back() {
    // A wall 12 u ahead (-z).
    let wall = block(
        Vec3::new(0.0, 3.0, -12.5),
        Vec3::new(10.0, 3.0, 0.5),
        Material::Solid,
    );
    let mut rig = Rig::new(&[floor(), wall], Vec3::ZERO, 0.0);
    double_tap(&mut rig, 3);
    rig.until(60, &idle(), |hero| hero.state == HeroState::Bounce);
    // Back at 9 u/s, still facing the wall, slowing at 24 u/s².
    rig.tick(&idle());
    assert!((rig.hero.velocity.z - (9.0 - 24.0 * TICK)).abs() < 1e-4);
    assert_eq!(rig.hero.yaw, 0.0);
    // It lasts its animation's 54 ticks; the 55th idles.
    let rest = rig.until(60, &idle(), |hero| hero.state == HeroState::Idle);
    assert_eq!(1 + rest, 55);
}

#[test]
fn a_charge_off_an_edge_sinks_and_carries_on() {
    // A 3 u high deck ending 10 u ahead, over the floor.
    let deck = block(
        Vec3::new(0.0, 1.5, 5.0),
        Vec3::new(10.0, 1.5, 15.0),
        Material::Solid,
    );
    let mut rig = Rig::new(&[floor(), deck], Vec3::new(0.0, 3.0, 0.0), 0.0);
    double_tap(&mut rig, 3);
    rig.until(30, &idle(), |hero| hero.ground.is_none());
    // Down to the floor at no more than 7.5 u/s, charging all the way.
    let mut fastest: f32 = 0.0;
    while rig.hero.ground.is_none() {
        assert_eq!(rig.hero.state, HeroState::Charge);
        rig.tick(&idle());
        fastest = fastest.min(rig.hero.velocity.y);
    }
    assert_eq!(rig.hero.state, HeroState::Charge);
    assert!(rig.hero.position.y < 0.01);
    assert!((-7.5 - 1e-3..-7.0).contains(&fastest), "{fastest}");
}

// --- Ability flags ---------------------------------------------------------------------

#[test]
fn without_the_boots_a_double_tap_only_crouches() {
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.hero = rig.hero.clone().with_abilities(Abilities {
        charge_boots: false,
        ..Abilities::ALL
    });
    double_tap(&mut rig, 3);
    assert_ne!(rig.hero.state, HeroState::Charge);
}

#[test]
fn without_the_heli_pack_crouch_jumps_are_plain_and_nothing_glides() {
    let no_pack = Abilities {
        heli_pack: false,
        ..Abilities::ALL
    };
    // From a standing crouch: a standing jump, not the heli jump.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.hero = rig.hero.clone().with_abilities(no_pack);
    rig.tick(&input(None, &[Action::Crouch], &[Action::Crouch]));
    rig.ticks(7, &input(None, &[Action::Crouch], &[]));
    rig.tick(&input(None, &[Action::Crouch], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Standing));

    // From a running crouch: no long jump either.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.hero = rig.hero.clone().with_abilities(no_pack);
    rig.ticks(20, &input(FORWARD, &[], &[]));
    rig.tick(&input(FORWARD, &[Action::Crouch], &[Action::Crouch]));
    rig.ticks(6, &input(FORWARD, &[Action::Crouch], &[]));
    rig.tick(&input(FORWARD, &[Action::Crouch], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Standing));

    // A double jump held all the way down never glides.
    let mut rig = Rig::new(&[floor()], Vec3::ZERO, 0.0);
    rig.hero = rig.hero.clone().with_abilities(no_pack);
    let hold = input(None, &[Action::Jump], &[]);
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(16, &hold);
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::DoubleJump);
    for _ in 0..80 {
        rig.tick(&hold);
        assert_ne!(rig.hero.state, HeroState::Glide);
    }
}

// --- Grinding --------------------------------------------------------------------------

/// The hero falling onto rails from 1 u above the first one's start.
fn rails(rails: Vec<GrindRail>) -> Rig {
    let above = rails[0].points[0] + Vec3::new(0.0, 1.0, -1.0);
    let mut rig = Rig {
        hero: HeroMotion::new(above, 0.0),
        profile: HeroProfile::uya(),
        world: ParryWorld::new(&[floor()]).with_rails(rails),
    };
    rig.tick(&idle());
    rig
}

fn straight(from: Vec3, to: Vec3) -> GrindRail {
    GrindRail::new(vec![from, to])
}

#[test]
fn a_hero_falling_onto_a_rail_rides_it_at_12_u_s() {
    // "Grinding": the base speed eases to 12 u/s at 15 u/s²; on a level rail that is all.
    let mut rig = rails(vec![straight(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, 2.0, -100.0),
    )]);
    rig.until(30, &idle(), |hero| hero.state == HeroState::Grind);
    rig.ticks(60, &idle());
    assert_eq!(rig.hero.state, HeroState::Grind);
    assert!((rig.hero.speed - 12.0).abs() < 1e-3, "{}", rig.hero.speed);
    assert!(
        (rig.hero.velocity.z + 12.0).abs() < 0.05,
        "{}",
        rig.hero.velocity
    );
    // Pulled onto the rail.
    assert!((rig.hero.position.y - 2.0).abs() < 0.01);
    assert!(rig.hero.position.x.abs() < 0.01);
}

#[test]
fn without_grind_boots_the_hero_falls_past_a_rail() {
    let mut rig = rails(vec![straight(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, 2.0, -100.0),
    )]);
    rig.hero = rig.hero.clone().with_abilities(Abilities {
        grind_boots: false,
        ..Abilities::ALL
    });
    rig.until(60, &idle(), |hero| hero.ground.is_some());
    assert_ne!(rig.hero.state, HeroState::Grind);
}

#[test]
fn slopes_speed_the_grind_up_downhill_and_slow_it_uphill() {
    // 30° down: the slope term heads for 12 u/s more (72/π per radian), up to 24 in all.
    let drop = 60.0 * 30f32.to_radians().tan();
    let mut rig = rails(vec![straight(
        Vec3::new(0.0, 50.0, 0.0),
        Vec3::new(0.0, 50.0 - drop, -60.0),
    )]);
    rig.until(30, &idle(), |hero| hero.state == HeroState::Grind);
    rig.ticks(150, &idle());
    assert_eq!(rig.hero.state, HeroState::Grind);
    assert!(
        rig.hero.speed > 23.5 && rig.hero.speed <= 24.0,
        "{}",
        rig.hero.speed
    );

    // 30° up: 15/π per radian less, 2.5 u/s at most.
    let mut rig = rails(vec![straight(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, 2.0 + drop, -60.0),
    )]);
    rig.until(30, &idle(), |hero| hero.state == HeroState::Grind);
    rig.ticks(150, &idle());
    assert_eq!(rig.hero.state, HeroState::Grind);
    assert!((rig.hero.speed - 9.5).abs() < 0.01, "{}", rig.hero.speed);
}

#[test]
fn a_jump_off_a_rail_follows_it_and_lands_back_on() {
    let mut rig = rails(vec![straight(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, 2.0, -200.0),
    )]);
    rig.until(30, &idle(), |hero| hero.state == HeroState::Grind);
    rig.ticks(60, &idle());
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Grind));
    let (start_z, mut highest, mut ticks) = (rig.hero.position.z, 0.0f32, 0);
    while rig.hero.state != HeroState::Grind {
        rig.tick(&input(None, &[Action::Jump], &[]));
        highest = highest.max(rig.hero.position.y);
        // Still over the rail all the way.
        assert!(rig.hero.position.x.abs() < 0.01, "{}", rig.hero.position);
        ticks += 1;
        assert!(ticks < 120, "never landed back on the rail");
    }
    // A held grind jump aims for 2.45 u, and it carried on along the rail at 12 u/s.
    assert!(highest > 3.5, "{highest}");
    assert!(rig.hero.position.z < start_z - 12.0 * TICK * (ticks as f32 - 2.0));
}

#[test]
fn a_rails_end_drops_the_hero_with_its_speed() {
    let mut rig = rails(vec![straight(
        Vec3::new(0.0, 4.0, 0.0),
        Vec3::new(0.0, 4.0, -20.0),
    )]);
    rig.until(30, &idle(), |hero| hero.state == HeroState::Grind);
    rig.until(200, &idle(), |hero| hero.state != HeroState::Grind);
    assert_eq!(rig.hero.state, HeroState::Fall);
    assert!(rig.hero.velocity.z < -5.0, "{}", rig.hero.velocity);
    // And it doesn't catch the rail's end again on the way down.
    rig.ticks(5, &idle());
    assert_eq!(rig.hero.state, HeroState::Fall);
}

#[test]
fn a_sideways_stick_switches_to_the_next_rail() {
    // Rails at x = 0 and x = 3, both along -z; facing -z, the stick's right is +x.
    let mut rig = rails(vec![
        straight(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 2.0, -200.0)),
        straight(Vec3::new(3.0, 2.0, 0.0), Vec3::new(3.0, 2.0, -200.0)),
    ]);
    rig.until(30, &idle(), |hero| hero.state == HeroState::Grind);
    rig.ticks(60, &idle());
    let right = Some([127, 0]);
    rig.tick(&input(right, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::GrindSwitch));
    rig.until(120, &idle(), |hero| hero.state == HeroState::Grind);
    assert_eq!(rig.hero.rail.map(|at| at.rail), Some(1));
    assert!(
        (rig.hero.position.x - 3.0).abs() < 0.05,
        "{}",
        rig.hero.position
    );
}

// --- Zip lines -------------------------------------------------------------------------
// Read in UYA's code and traced on Florana (`hero-controller.md`, "Zip lines").

/// The hero standing on the floor facing −z, under zip lines whose first starts 2.2 u up
/// just ahead of him.
fn zip_lines(cables: Vec<GrindRail>) -> Rig {
    let mut rig = Rig {
        hero: HeroMotion::new(Vec3::new(0.0, 0.0, -0.5), 0.0),
        profile: HeroProfile::uya(),
        world: ParryWorld::new(&[floor()]).with_rails(cables),
    };
    rig.tick(&idle());
    rig
}

fn cable(from: Vec3, to: Vec3) -> GrindRail {
    GrindRail::zip_line(vec![from, to])
}

/// Jumps and holds the jump until the hands catch the first cable.
fn catch(rig: &mut Rig) {
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.until(30, &input(None, &[Action::Jump], &[]), |hero| {
        hero.state == HeroState::ZipLine
    });
}

#[test]
fn deadlocked_has_no_zip_lines() {
    // "Deadlocked's moves": DL's code has no catch, and its state 118 is the death fall.
    let mut rig = zip_lines(vec![cable(
        Vec3::new(0.0, 2.2, 0.0),
        Vec3::new(0.0, 2.2, -90.0),
    )]);
    rig.profile = HeroProfile::dl();
    rig.tick(&input(None, &[], &[Action::Jump]));
    for _ in 0..90 {
        rig.tick(&input(None, &[Action::Jump], &[]));
        assert_ne!(rig.hero.state, HeroState::ZipLine);
    }
}

#[test]
fn a_jump_under_a_zip_line_catches_it_and_hangs_the_feet_below() {
    let mut rig = zip_lines(vec![cable(
        Vec3::new(0.0, 2.2, 0.0),
        Vec3::new(0.0, 2.2, -90.0),
    )]);
    catch(&mut rig);
    assert_eq!(rig.hero.state.game_number(), 118);
    // The hands, 1.34 u up, reach the cable's window (0.7 under to 0.4 over) early in
    // the jump.
    assert!(
        rig.hero.position.y < 2.2 - 1.34 + 0.4,
        "{}",
        rig.hero.position
    );
    rig.ticks(60, &idle());
    assert_eq!(rig.hero.state, HeroState::ZipLine);
    // Settled 1.63 u under the cable, as on Florana's, and right under it.
    assert!(
        (rig.hero.position.y - (2.2 - 1.63)).abs() < 0.01,
        "{}",
        rig.hero.position
    );
    assert!(rig.hero.position.x.abs() < 0.01, "{}", rig.hero.position);
    assert!(rig.hero.velocity.z < 0.0);
}

#[test]
fn a_zip_lines_speed_builds_to_14_u_s_whatever_its_slope() {
    // Florana's trace: from 0, +0.0025 u per frame each frame to 0.23333 (94 frames).
    for drop in [0.0, 30.0, -10.0] {
        let mut rig = zip_lines(vec![cable(
            Vec3::new(0.0, 2.2, 0.0),
            Vec3::new(0.0, 2.2 - drop, -95.0),
        )]);
        // It starts at half the jump's velocity along the cable, 0 unless it climbs.
        catch(&mut rig);
        let start = rig.hero.speed;
        rig.ticks(30, &idle());
        assert!(
            (rig.hero.speed - start - 30.0 * 0.15).abs() < 1e-3,
            "{drop}: {}",
            rig.hero.speed
        );
        rig.ticks(70, &idle());
        assert_eq!(rig.hero.state, HeroState::ZipLine);
        assert!((rig.hero.speed - 14.0).abs() < 1e-4, "{}", rig.hero.speed);
    }
}

#[test]
fn nothing_the_player_does_changes_the_ride() {
    let line = || {
        zip_lines(vec![cable(
            Vec3::new(0.0, 2.2, 0.0),
            Vec3::new(0.0, 2.2, -90.0),
        )])
    };
    let (mut still, mut busy) = (line(), line());
    catch(&mut still);
    catch(&mut busy);
    let presses = [
        input(None, &[], &[Action::Jump]),
        input(None, &[], &[Action::Melee]),
        input(Some([127, 0]), &[], &[]),
        input(Some([-127, 0]), &[Action::Crouch], &[Action::Crouch]),
    ];
    for tick in 0..120 {
        still.tick(&idle());
        busy.tick(&presses[tick % presses.len()]);
    }
    assert_eq!(busy.hero.state, HeroState::ZipLine);
    assert_eq!(busy.hero.position, still.hero.position);
    assert_eq!(busy.hero.speed, still.hero.speed);
}

#[test]
fn the_cables_end_lets_go_after_6_ticks_and_no_cable_catches_for_30() {
    // A short cable, then another right after it at the same height: flying on past the
    // first's end, the hands pass the second, but it can't be caught yet.
    let mut rig = zip_lines(vec![
        cable(Vec3::new(0.0, 2.2, 0.0), Vec3::new(0.0, 2.2, -6.0)),
        cable(Vec3::new(0.0, 2.2, -6.3), Vec3::new(0.0, 2.2, -60.0)),
    ]);
    catch(&mut rig);
    let mut riding = 0;
    while rig
        .hero
        .rail
        .is_some_and(|at| at.segment == 0 && at.t < 1.0)
    {
        rig.tick(&idle());
        riding += 1;
        assert!(riding < 300, "never reached the end");
    }
    // At the end: 5 more ticks hanging on, gaining 22 u/s² of fall from the second.
    let speed = rig.hero.speed;
    rig.ticks(4, &idle());
    assert_eq!(rig.hero.state, HeroState::ZipLine);
    rig.tick(&idle());
    assert_eq!(rig.hero.state, HeroState::Fall);
    assert!(
        (rig.hero.velocity.y + 5.0 * 22.0 * TICK).abs() < 1e-3,
        "{}",
        rig.hero.velocity
    );
    assert!((-rig.hero.velocity.z - speed).abs() < 1e-3);
    for _ in 0..40 {
        rig.tick(&idle());
        assert_ne!(rig.hero.state, HeroState::ZipLine);
    }
}

#[test]
fn zip_lines_and_grind_rails_each_keep_to_their_own_move() {
    // Jumping under a grind rail doesn't hang from it.
    let mut rig = zip_lines(vec![straight(
        Vec3::new(0.0, 2.2, 0.0),
        Vec3::new(0.0, 2.2, -90.0),
    )]);
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.until(120, &idle(), |hero| hero.ground.is_some());
    assert_ne!(rig.hero.state, HeroState::ZipLine);
    // Falling onto a zip line, with the grind boots, doesn't grind it: the cable is
    // passed, then caught by the hands from above.
    let mut rig = rails(vec![cable(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, 2.0, -100.0),
    )]);
    rig.until(60, &idle(), |hero| hero.state != HeroState::Fall);
    assert_eq!(rig.hero.state, HeroState::ZipLine);
}

// --- Swimming --------------------------------------------------------------------------

/// The water's surface in [`pool`].
const SURFACE: f32 = 3.0;
/// Where the feet float at rest: 0.12 u under the surface.
const AFLOAT: f32 = SURFACE - 0.12;

/// Water 3 u deep over the floor and 100 u across, with the hero at `position`, facing
/// −z, and `extra` bodies in it.
fn pool(position: Vec3, extra: &[BodyPlacement]) -> Rig {
    let water = WaterVolume::new(Vec3::new(0.0, 1.5, 0.0), Vec3::new(50.0, 1.5, 50.0));
    let mut bodies = vec![floor()];
    bodies.extend_from_slice(extra);
    let mut rig = Rig {
        hero: HeroMotion::new(position, 0.0),
        profile: HeroProfile::uya(),
        world: ParryWorld::new(&bodies).with_water(vec![water]),
    };
    rig.tick(&idle());
    rig
}

/// Dropped in from 2 u above the surface and left to settle.
fn floating() -> Rig {
    let mut rig = pool(Vec3::new(0.0, SURFACE + 2.0, 0.0), &[]);
    rig.until(120, &idle(), |hero| hero.state == HeroState::SurfaceIdle);
    rig.ticks(600, &idle());
    rig
}

#[test]
fn falling_into_deep_water_bobs_and_settles_afloat() {
    // "Swimming": a falling hero gets in with his feet about 0.45 u under; the bob then
    // carries him deeper and back up, and he settles 0.12 u under the surface.
    let mut rig = pool(Vec3::new(0.0, SURFACE + 2.0, 0.0), &[]);
    rig.until(120, &idle(), |hero| hero.state == HeroState::SurfaceIdle);
    assert!(
        (rig.hero.position.y - (SURFACE - 0.45)).abs() < 0.3,
        "{}",
        rig.hero.position
    );
    let mut lowest = f32::MAX;
    for _ in 0..90 {
        rig.tick(&idle());
        lowest = lowest.min(rig.hero.position.y);
    }
    assert!(lowest < AFLOAT - 0.4, "{lowest}");
    rig.ticks(600, &idle());
    assert_eq!(rig.hero.state, HeroState::SurfaceIdle);
    assert!(
        (rig.hero.position.y - AFLOAT).abs() < 0.01,
        "{}",
        rig.hero.position
    );
}

#[test]
fn the_surface_swim_averages_a_little_over_3_u_s() {
    // 3 u/s × 5.5 × the stroke, which averages about 1.06 × the speed.
    let mut rig = floating();
    rig.tick(&input(FORWARD, &[], &[]));
    assert_eq!(rig.hero.state, HeroState::SurfaceSwim);
    rig.ticks(120, &input(FORWARD, &[], &[]));
    assert!((rig.hero.speed - 3.0).abs() < 1e-4, "{}", rig.hero.speed);
    let start = rig.hero.position;
    // Two whole strokes: 30 frames at 0.3 a tick each.
    rig.ticks(200, &input(FORWARD, &[], &[]));
    let travelled = rig.hero.position - start;
    let average = -travelled.z / (200.0 * TICK);
    assert!((average - 3.17).abs() < 0.05, "{average}");
    assert!(travelled.x.abs() < 1e-3);
    assert!((rig.hero.position.y - AFLOAT).abs() < 0.01);

    // Let go and he finishes his stroke, slows at 5 u/s², and floats again.
    let stopped = rig.until(120, &idle(), |hero| hero.state == HeroState::SurfaceIdle);
    assert!(stopped > 30, "{stopped}");
}

#[test]
fn deadlocked_swims_on_the_surface_without_diving() {
    // "Deadlocked's swimming": DL's surface states have no dive.
    let mut rig = floating();
    rig.profile = HeroProfile::dl();
    for buttons in [&[Action::Melee][..], &[Action::Crouch], &[Action::Strafe]] {
        rig.tick(&input(None, &[], buttons));
        rig.ticks(5, &idle());
        assert_eq!(rig.hero.state, HeroState::SurfaceIdle, "{buttons:?}");
    }
}

#[test]
fn melee_dives_and_jump_rises_back_to_the_surface() {
    let mut rig = floating();
    rig.tick(&input(None, &[], &[Action::Melee]));
    assert_eq!(rig.hero.state, HeroState::UnderwaterSwim);
    // The 40-tick dive points him at least 0.35 × 78° down and heads for 6 u/s.
    rig.ticks(40, &idle());
    assert!(rig.hero.pitch > 0.3, "{}", rig.hero.pitch);
    assert!(rig.hero.position.y < AFLOAT - 0.5, "{}", rig.hero.position);
    let deepest = rig.hero.position.y;

    // Holding jump pitches him up to 78° and swims him up until he surfaces.
    let rise = input(None, &[Action::Jump], &[]);
    rig.until(300, &rise, |hero| hero.state == HeroState::SurfaceIdle);
    assert!(rig.hero.position.y > deepest + 0.5);
    assert!(
        rig.hero.position.y > SURFACE - 0.4 - 0.1,
        "{}",
        rig.hero.position
    );
    rig.ticks(600, &idle());
    assert!(
        (rig.hero.position.y - AFLOAT).abs() < 0.01,
        "{}",
        rig.hero.position
    );
    assert!(rig.hero.pitch.abs() < 0.01, "{}", rig.hero.pitch);
}

#[test]
fn melee_held_pitches_down_at_most_78_degrees() {
    let mut rig = floating();
    let sink = input(None, &[Action::Melee], &[]);
    rig.tick(&input(None, &[], &[Action::Melee]));
    let mut steepest = 0.0f32;
    for _ in 0..30 {
        rig.tick(&sink);
        steepest = steepest.max(rig.hero.pitch);
        // The pitch turns at most 0.0204 rad a tick with the button held.
        assert!(rig.hero.pitch <= 0.0204 * 31.0 + 1e-3);
    }
    assert!(steepest > 0.5);
    assert!(rig.hero.velocity.y < 0.0);
    rig.ticks(200, &sink);
    assert!(rig.hero.pitch <= 1.3613569 + 1e-6);
}

#[test]
fn crouch_held_under_water_swims_fast_at_7_u_s() {
    let mut rig = floating();
    let fast = input(None, &[Action::Crouch], &[]);
    rig.tick(&input(None, &[Action::Crouch], &[Action::Melee]));
    assert_eq!(rig.hero.state, HeroState::UnderwaterFast);
    rig.ticks(120, &fast);
    assert_eq!(rig.hero.state, HeroState::UnderwaterFast);
    assert!((rig.hero.speed - 7.0).abs() < 1e-4, "{}", rig.hero.speed);
    assert!(
        (rig.hero.velocity.length() - 7.0).abs() < 1e-3,
        "{}",
        rig.hero.velocity
    );

    // Let go with the stick still and he drifts to a stop at 8 u/s² after a fast swim.
    rig.tick(&idle());
    assert_eq!(rig.hero.state, HeroState::UnderwaterIdle);
    let stopped = rig.until(120, &idle(), |hero| hero.velocity.length() < 1e-4);
    assert!((50..=56).contains(&stopped), "{stopped}");
}

#[test]
fn the_swim_turns_fast_after_20_ticks_with_crouch_held() {
    let mut rig = floating();
    rig.tick(&input(FORWARD, &[], &[Action::Melee]));
    assert_eq!(rig.hero.state, HeroState::UnderwaterSwim);
    rig.until(60, &input(FORWARD, &[Action::Crouch], &[]), |hero| {
        hero.state == HeroState::UnderwaterFast
    });
    assert!(rig.hero.ticks <= 1);
}

#[test]
fn jump_leaves_the_surface_about_1_5_u_up_and_falls_back_in() {
    // State 18: target 1.95-1.98 u, hold 10, push-off 5, gravity 29.7. As with every jump
    // the target isn't the apex ("Jump heights"), and the push-off presses him down into
    // the water first.
    let mut rig = floating();
    let start = rig.hero.position.y;
    rig.tick(&input(None, &[], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Water));
    let held = input(None, &[Action::Jump], &[]);
    let (mut highest, mut ticks) = (start, 0);
    while rig.hero.state == HeroState::Jump(JumpKind::Water) {
        rig.tick(&held);
        highest = highest.max(rig.hero.position.y);
        ticks += 1;
        assert!(ticks < 120, "never fell back in");
    }
    assert_eq!(rig.hero.state, HeroState::SurfaceIdle);
    let rise = highest - start;
    assert!((rise - 1.55).abs() < 0.02, "{rise}");
}

#[test]
fn a_jump_from_the_water_lands_on_the_side() {
    // A side 0.5 u above the surface, 1 u ahead.
    let side = block(
        Vec3::new(0.0, (SURFACE + 0.5) / 2.0, -6.0),
        Vec3::new(10.0, (SURFACE + 0.5) / 2.0, 5.0),
        Material::Solid,
    );
    let mut rig = pool(Vec3::new(0.0, SURFACE + 2.0, 0.0), &[side]);
    rig.until(120, &idle(), |hero| hero.state == HeroState::SurfaceIdle);
    rig.ticks(600, &idle());
    rig.tick(&input(FORWARD, &[], &[Action::Jump]));
    rig.until(120, &input(FORWARD, &[Action::Jump], &[]), |hero| {
        hero.ground.is_some()
    });
    assert!((rig.hero.position.y - (SURFACE + 0.5)).abs() < 0.01);
    assert!(!rig.hero.state.is_swimming());
}

#[test]
fn shallow_water_is_walked_through() {
    let water = WaterVolume::new(Vec3::new(0.0, 0.3, 0.0), Vec3::new(50.0, 0.3, 50.0));
    let mut rig = Rig {
        hero: HeroMotion::new(Vec3::new(0.0, 2.0, 0.0), 0.0),
        profile: HeroProfile::uya(),
        world: ParryWorld::new(&[floor()]).with_water(vec![water]),
    };
    // Falling in lands on the bottom; walking stays walking.
    rig.until(120, &idle(), |hero| hero.ground.is_some());
    for _ in 0..120 {
        rig.tick(&input(FORWARD, &[], &[]));
        assert!(!rig.hero.state.is_swimming(), "{:?}", rig.hero.state);
    }
    assert_eq!(rig.hero.state, HeroState::Walk);
}

#[test]
fn swimming_onto_a_shallow_shelf_stands_up() {
    // The floor rises to 0.4 u under the surface beyond z = -6.
    let shelf = block(
        Vec3::new(0.0, (SURFACE - 0.4) / 2.0, -26.0),
        Vec3::new(50.0, (SURFACE - 0.4) / 2.0, 20.0),
        Material::Solid,
    );
    let mut rig = pool(Vec3::new(0.0, SURFACE + 2.0, 0.0), &[shelf]);
    rig.until(120, &idle(), |hero| hero.state == HeroState::SurfaceIdle);
    rig.ticks(600, &idle());
    rig.until(600, &input(FORWARD, &[], &[]), |hero| hero.ground.is_some());
    assert!(!rig.hero.state.is_swimming());
    assert!((rig.hero.position.y - (SURFACE - 0.4)).abs() < 0.01);
}
