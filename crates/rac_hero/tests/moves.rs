//! P1.5's second chunk against `docs/research/hero-controller.md`: the wall jump, ledges,
//! getting hit, and when each jump can start. These rules are read in UYA's code but not
//! yet traced, so the tests check the code's numbers.

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, HeroMotion, HeroProfile, HeroState, JumpKind, Material, ParryWorld,
    Side,
    controller::{self, StepEvents, step},
    motor,
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
fn the_stick_to_one_side_shimmies_along_the_ledge() {
    let mut rig = ledge_rig();
    hang(&mut rig);
    let start = rig.hero.position;
    // Facing −z with the camera behind, stick left is −x.
    let left = input(Some([-127, 0]), &[], &[]);
    rig.tick(&left);
    assert_eq!(rig.hero.state, HeroState::LedgeShimmy(Side::Left));
    rig.ticks(30, &left);
    let moved = start.x - rig.hero.position.x;
    let expected = rig.profile.ledge.shimmy_speed * 30.0 / 60.0;
    assert!((moved - expected).abs() < 0.05, "moved {moved}");
    assert!((rig.hero.position.y - start.y).abs() < 1e-3);
    rig.tick(&idle());
    assert_eq!(rig.hero.state, HeroState::LedgeHang);
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
