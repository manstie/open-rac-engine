//! Strafing and lock-on against `docs/research/hero-controller.md`, "Strafing" and
//! "Lock-on". Both are read in UYA NTSC's code but not traced, so the tests check the
//! code's numbers.

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, Collider, HeroMotion, HeroProfile, HeroState, JumpKind, LockOn, ParryWorld,
    StrafeSide,
    controller::{TICK, step},
    lock_on,
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

fn strafe(stick: Option<[i8; 2]>) -> PlayerInput {
    input(stick, &[Action::Strafe], &[])
}

fn block(centre: Vec3, half_extents: Vec3) -> BodyPlacement {
    BodyPlacement::fixed(Collider::Cuboid { half_extents }, centre, Quat::IDENTITY)
}

fn floor() -> BodyPlacement {
    block(Vec3::new(0.0, -0.5, 0.0), Vec3::new(100.0, 0.5, 100.0))
}

const RIGHT: [i8; 2] = [127, 0];
const LEFT: [i8; 2] = [-127, 0];
const BACK: [i8; 2] = [0, -127];
const FORWARD: [i8; 2] = [0, 127];
/// The walk's 37 u/s² for one tick.
const STEP: f32 = 37.0 * TICK;

struct Rig {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
    camera_yaw: f32,
}

impl Rig {
    /// A hero facing `yaw` on an open floor, with the camera looking along −z.
    fn new(yaw: f32) -> Self {
        let mut rig = Self {
            hero: HeroMotion::new(Vec3::ZERO, yaw),
            profile: HeroProfile::uya(),
            world: ParryWorld::new(&[floor()]),
            camera_yaw: 0.0,
        };
        rig.tick(&input(None, &[], &[]));
        rig
    }

    fn tick(&mut self, input: &PlayerInput) {
        step(
            &mut self.hero,
            input,
            self.camera_yaw,
            &self.profile,
            &self.world,
        );
    }

    fn ticks(&mut self, count: usize, input: &PlayerInput) {
        for _ in 0..count {
            self.tick(input);
        }
    }

    fn horizontal(&self) -> Vec3 {
        Vec3::new(self.hero.velocity.x, 0.0, self.hero.velocity.z)
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn strafing_moves_with_the_stick_at_the_walks_speeds_and_keeps_facing_the_camera() {
    let mut rig = Rig::new(0.0);
    rig.tick(&strafe(Some(RIGHT)));
    assert_eq!(rig.hero.state, HeroState::Walk);
    assert!(close(rig.hero.speed, STEP), "{}", rig.hero.speed);
    rig.ticks(20, &strafe(Some(RIGHT)));
    // Sideways at the walk's 6.17 u/s, still facing the camera's way.
    assert!(
        (rig.horizontal() - Vec3::new(6.17, 0.0, 0.0)).length() < 1e-4,
        "{}",
        rig.hero.velocity
    );
    assert_eq!(rig.hero.yaw, 0.0);
    assert_eq!(rig.hero.strafe_side, StrafeSide::Right);
}

#[test]
fn strafing_turns_him_to_the_camera_on_its_own_spring() {
    let mut rig = Rig::new(0.0);
    rig.camera_yaw = FRAC_PI_2;
    rig.tick(&strafe(Some(FORWARD)));
    // The strafe's gain 0.042, damping 0.25 and cap 0.32 rad a tick (0x4EA5B0).
    let rate = 0.042 * FRAC_PI_2;
    assert!(close(rig.hero.yaw, rate), "{}", rig.hero.yaw);
    // The walk's own turning rate is left at 0.
    assert_eq!(rig.hero.yaw_rate, 0.0);
    rig.tick(&strafe(Some(FORWARD)));
    let next = rate + 0.042 * (FRAC_PI_2 - rate) - 0.25 * rate;
    assert!(close(rig.hero.yaw, rate + next), "{}", rig.hero.yaw);
    rig.ticks(120, &strafe(Some(FORWARD)));
    assert!(close(rig.hero.yaw, FRAC_PI_2), "{}", rig.hero.yaw);
    // Forward for the camera is −x now.
    assert!(rig.hero.velocity.x < -6.0, "{}", rig.hero.velocity);
}

#[test]
fn going_from_right_to_left_stops_him_for_two_ticks() {
    let mut rig = Rig::new(0.0);
    rig.ticks(20, &strafe(Some(RIGHT)));
    rig.tick(&strafe(Some(LEFT)));
    assert!(close(rig.hero.speed, STEP), "{}", rig.hero.speed);
    assert!(rig.hero.velocity.x < 0.0);
    rig.tick(&strafe(Some(LEFT)));
    assert!(close(rig.hero.speed, STEP), "{}", rig.hero.speed);
    rig.tick(&strafe(Some(LEFT)));
    assert!(close(rig.hero.speed, 2.0 * STEP), "{}", rig.hero.speed);
}

#[test]
fn going_from_left_to_right_eases_in_by_thirds() {
    let mut rig = Rig::new(0.0);
    rig.ticks(20, &strafe(Some(LEFT)));
    rig.tick(&strafe(Some(RIGHT)));
    let first = 6.17 / 3.0 + STEP;
    assert!(close(rig.hero.speed, first), "{}", rig.hero.speed);
    rig.tick(&strafe(Some(RIGHT)));
    let second = first * 2.0 / 3.0 + STEP;
    assert!(close(rig.hero.speed, second), "{}", rig.hero.speed);
    rig.tick(&strafe(Some(RIGHT)));
    assert!(close(rig.hero.speed, second + STEP), "{}", rig.hero.speed);
}

#[test]
fn a_strafe_jump_flips_only_with_the_stick_well_over_to_a_side_or_back() {
    for (stick, flips) in [
        (RIGHT, true),
        (LEFT, true),
        (BACK, true),
        (FORWARD, false),
        // An exact diagonal counts as forward.
        ([127, -127], false),
        // Under 0.9 of the stick.
        ([100, 0], false),
    ] {
        let mut rig = Rig::new(0.0);
        rig.tick(&input(Some(stick), &[Action::Strafe], &[Action::Jump]));
        let flipped = rig.hero.state == HeroState::Jump(JumpKind::Flip);
        assert_eq!(flipped, flips, "{stick:?}: {:?}", rig.hero.state);
    }
}

#[test]
fn strafing_backwards_walks_backwards() {
    let mut rig = Rig::new(0.0);
    rig.tick(&strafe(Some(BACK)));
    assert!(rig.hero.strafing_back);
    rig.tick(&strafe(Some(LEFT)));
    assert!(!rig.hero.strafing_back);
    rig.tick(&strafe(Some(BACK)));
    rig.tick(&input(Some(BACK), &[], &[]));
    assert!(!rig.hero.strafing && !rig.hero.strafing_back);
}

#[test]
fn a_strafe_jump_goes_the_strafes_way_and_turns_to_the_camera_in_the_air() {
    let mut rig = Rig::new(FRAC_PI_2);
    rig.ticks(3, &strafe(Some(FORWARD)));
    rig.tick(&input(Some(FORWARD), &[Action::Strafe], &[Action::Jump]));
    // Strafing never run-jumps, as traced on UYA NTSC.
    assert_eq!(rig.hero.state, HeroState::Jump(JumpKind::Standing));
    // Along the stick (−z), not the way he faced when the strafe began.
    assert!(rig.hero.velocity.z < 0.0 && rig.hero.velocity.x.abs() < 1e-4);
    // With the stick let go he still turns to the camera...
    rig.ticks(30, &strafe(None));
    assert!(rig.hero.state.is_airborne());
    assert!(rig.hero.yaw.abs() < 0.2, "{}", rig.hero.yaw);

    // ...which he doesn't without strafing.
    let mut rig = Rig::new(FRAC_PI_2);
    rig.tick(&input(None, &[], &[Action::Jump]));
    rig.ticks(30, &input(None, &[], &[]));
    assert!(close(rig.hero.yaw, FRAC_PI_2), "{}", rig.hero.yaw);
}

#[test]
fn letting_go_of_the_stick_slides_the_strafe_to_a_stop_the_way_it_went() {
    let mut rig = Rig::new(0.0);
    rig.ticks(20, &strafe(Some(RIGHT)));
    for _ in 0..12 {
        rig.tick(&strafe(None));
        assert!(rig.hero.velocity.z.abs() < 1e-5, "{}", rig.hero.velocity);
        assert!(rig.hero.velocity.x >= 0.0);
    }
    assert_eq!(rig.hero.state, HeroState::Idle);
    assert_eq!(rig.hero.yaw, 0.0);
}

#[test]
fn a_strafe_brakes_in_the_walk_until_he_faces_the_camera() {
    let mut rig = Rig::new(0.0);
    rig.ticks(20, &strafe(Some(RIGHT)));
    rig.camera_yaw = 1.0;
    rig.tick(&strafe(None));
    // Braking at the skid's 17 u/s², still walking, turning to the camera.
    assert_eq!(rig.hero.state, HeroState::Walk);
    assert!(
        close(rig.hero.speed, 6.17 - 17.0 * TICK),
        "{}",
        rig.hero.speed
    );
    assert!(rig.hero.yaw > 0.0);
    let mut walked = 1;
    while rig.hero.state == HeroState::Walk {
        rig.tick(&strafe(None));
        walked += 1;
        assert!(walked < 200);
    }
    assert!(
        (rig.hero.yaw - 1.0).abs() < 2f32.to_radians(),
        "{}",
        rig.hero.yaw
    );
    // Longer than the plain walk's two ticks of braking.
    assert!(walked > 3, "{walked}");
}

#[test]
fn locking_on_strafes_too() {
    let mut rig = Rig::new(0.0);
    rig.tick(&input(Some(RIGHT), &[Action::LockOn], &[]));
    assert!(rig.hero.strafing);
    assert!(rig.hero.velocity.x > 0.0 && rig.hero.yaw == 0.0);
}

// Lock-on.

fn entity(index: u32) -> Entity {
    Entity::from_raw_u32(index).unwrap()
}

struct Lock {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
    lock: LockOn,
    targets: Vec<(Entity, Vec3)>,
}

impl Lock {
    /// A hero at the origin facing −z, with targets: one ahead and a little left, one 45°
    /// round to the right, one behind.
    fn new(bodies: &[BodyPlacement]) -> Self {
        Self {
            hero: HeroMotion::new(Vec3::ZERO, 0.0),
            profile: HeroProfile::uya(),
            world: ParryWorld::new(bodies),
            lock: LockOn::default(),
            targets: vec![
                (entity(1), Vec3::new(-3.0, 1.0, -10.0)),
                (entity(2), Vec3::new(8.0, 1.0, -8.0)),
                (entity(3), Vec3::new(0.0, 1.0, 10.0)),
            ],
        }
    }

    fn update(&mut self, camera_yaw: f32, input: &PlayerInput) {
        lock_on::update(
            &mut self.lock,
            &self.hero,
            input,
            camera_yaw,
            &self.targets,
            &self.profile.lock_on,
            &self.world,
        );
    }
}

fn press_lock() -> PlayerInput {
    input(None, &[Action::LockOn], &[Action::LockOn])
}

fn hold_lock() -> PlayerInput {
    input(None, &[Action::LockOn], &[])
}

#[test]
fn pressing_lock_on_picks_the_best_target_near_the_middle_of_the_view() {
    let mut lock = Lock::new(&[floor()]);
    // Held without a fresh press, nothing is picked.
    lock.update(0.0, &hold_lock());
    assert_eq!(lock.lock.target, None);
    lock.update(0.0, &press_lock());
    // The one 45° round is outside the 35° cone, and the one behind is out of view.
    assert_eq!(lock.lock.target, Some(entity(1)));
    assert_eq!(lock.lock.target_point(), Some(Vec3::new(-3.0, 1.0, -10.0)));

    // Looking right, it is the other.
    let mut lock = Lock::new(&[floor()]);
    lock.update(-FRAC_PI_2 / 2.0, &press_lock());
    assert_eq!(lock.lock.target, Some(entity(2)));
}

#[test]
fn with_nothing_in_view_he_locks_on_to_what_he_faces() {
    let mut lock = Lock::new(&[floor()]);
    lock.targets.truncate(1);
    lock.update(std::f32::consts::PI, &press_lock());
    assert_eq!(lock.lock.target, Some(entity(1)));
}

#[test]
fn a_target_out_of_sight_or_beyond_25_u_is_not_picked() {
    // A wall between the hero and the first target.
    let mut lock = Lock::new(&[
        floor(),
        block(Vec3::new(-1.5, 1.0, -5.0), Vec3::new(1.0, 2.0, 0.2)),
    ]);
    lock.update(0.0, &press_lock());
    assert_eq!(lock.lock.target, None);

    let mut lock = Lock::new(&[floor()]);
    lock.targets = vec![(entity(1), Vec3::new(0.0, 1.0, -26.0))];
    lock.update(0.0, &press_lock());
    assert_eq!(lock.lock.target, None);
}

#[test]
fn the_lock_holds_until_a_button_is_let_go_or_the_target_is_beyond_30_u() {
    let mut lock = Lock::new(&[floor()]);
    lock.update(0.0, &press_lock());
    // It follows the target, and turning away doesn't lose it.
    lock.targets[0].1 = Vec3::new(-3.0, 1.0, -20.0);
    lock.update(std::f32::consts::PI, &hold_lock());
    assert_eq!(lock.lock.target_point(), Some(Vec3::new(-3.0, 1.0, -20.0)));
    lock.update(0.0, &input(None, &[Action::Strafe], &[]));
    assert_eq!(lock.lock.target, None);

    let mut lock = Lock::new(&[floor()]);
    lock.update(0.0, &press_lock());
    lock.targets[0].1 = Vec3::new(-3.0, 1.0, -31.0);
    lock.update(0.0, &hold_lock());
    assert_eq!(lock.lock.target, None);
    // A lost target isn't replaced while the pair stays held.
    lock.targets[0].1 = Vec3::new(-3.0, 1.0, -10.0);
    lock.update(0.0, &hold_lock());
    assert_eq!(lock.lock.target, None);

    // A target that goes away (destroyed) is let go.
    let mut lock = Lock::new(&[floor()]);
    lock.update(0.0, &press_lock());
    lock.targets.remove(0);
    lock.update(0.0, &hold_lock());
    assert_eq!(lock.lock.target, None);
}
