use super::follow::{self, FollowInput, Rig};
use super::vertical::Vertical;
use super::volume::VolumeSettings;
use super::*;
use crate::{
    HeroProfile,
    collision::{BodyPlacement, Collider, ParryWorld},
    controller::{JumpKind, TICK},
};
use rac_input::{Action, Buttons};

fn uya() -> HeroProfile {
    HeroProfile::uya()
}

fn empty() -> ParryWorld {
    ParryWorld::default()
}

fn boxes(placements: &[(Vec3, Vec3)]) -> ParryWorld {
    let bodies: Vec<_> = placements
        .iter()
        .map(|&(centre, half_extents)| {
            BodyPlacement::fixed(Collider::Cuboid { half_extents }, centre, Quat::IDENTITY)
        })
        .collect();
    ParryWorld::new(&bodies)
}

/// A floor whose top is at y = 0.
fn floor() -> (Vec3, Vec3) {
    (Vec3::new(0.0, -0.5, 0.0), Vec3::new(50.0, 0.5, 50.0))
}

/// A rig at rest behind a hero at the origin facing −z.
fn rest(profile: &HeroProfile) -> (Rig, HeroMotion) {
    let hero = HeroMotion::new(Vec3::ZERO, 0.0);
    (Rig::behind(&hero, 0.0, &profile.camera), hero)
}

fn still() -> FollowInput {
    FollowInput {
        stick: Vec2::ZERO,
        mouse: Vec2::ZERO,
        speed: DEFAULT_SPEED,
        lock: None,
        steer: None,
        volume: VolumeSettings::default(),
    }
}

fn mouse(turn: Vec2) -> FollowInput {
    FollowInput {
        mouse: turn,
        ..still()
    }
}

fn step(rig: &mut Rig, hero: &HeroMotion, input: &FollowInput, world: &ParryWorld) -> (Vec3, Vec3) {
    follow::step(rig, hero, input, &uya(), world)
}

#[test]
fn rests_behind_and_above_the_hero() {
    let camera = FollowCamera::for_player(0, 0.0);
    let profile = uya().camera;
    let (position, rotation) = camera.placement(Vec3::new(10.0, 5.0, 0.0), &profile);
    assert!(
        (position - Vec3::new(10.0, 7.0, 4.64)).length() < 1e-4,
        "{position}"
    );
    // It aims 1.5 u above the feet.
    let towards_aim = (Vec3::new(10.0, 6.5, 0.0) - position).normalize();
    assert!((rotation * Vec3::NEG_Z - towards_aim).length() < 1e-4);
    // Deadlocked's lock-strafe camera rests 6 u behind the eye, 2 u above his feet.
    let profile = HeroProfile::dl().camera;
    let (position, rotation) = camera.placement(Vec3::ZERO, &profile);
    assert!(
        (position - Vec3::new(0.0, 2.0, 6.0)).length() < 1e-4,
        "{position}"
    );
    assert!((rotation * Vec3::NEG_Z - Vec3::NEG_Z).length() < 1e-5);
}

#[test]
fn a_still_hero_keeps_the_camera_at_rest() {
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    for _ in 0..60 {
        let (position, aim) = step(&mut rig, &hero, &still(), &empty());
        assert!(
            (position - Vec3::new(0.0, 2.0, 4.64)).length() < 1e-4,
            "{position}"
        );
        assert_eq!(aim, Vec3::new(0.0, 1.5, 0.0));
    }
}

#[test]
fn running_sideways_drags_the_arm_round() {
    // camera.md, step 3: at 0.1028 u per tick across the arm, it turns
    // 0.85 × 0.1028 / 4.64 = 0.01883 rad a tick (measured in uyantsc-veldin-cam-1). The
    // hero runs to the camera's right, so the camera turns right: its yaw falls.
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    let speed = 6.17;
    hero.velocity = Vec3::X * speed;
    hero.state = HeroState::Walk;
    let mut yaw = rig.arm_yaw;
    for tick in 0..20 {
        hero.position += hero.velocity * TICK;
        step(&mut rig, &hero, &still(), &empty());
        let turned = wrap(rig.arm_yaw - yaw);
        yaw = rig.arm_yaw;
        if tick < 5 {
            assert!(
                (turned + 0.85 * speed * TICK / 4.64).abs() < 1e-4,
                "{turned}"
            );
        }
    }
}

#[test]
fn strafing_stops_the_drag() {
    // Strafing with the buttons held, the arm takes up none of his movement
    // (`camera.md`, "Per-state parameters"), so the camera doesn't swing round.
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.velocity = Vec3::X * 6.17;
    hero.state = HeroState::Walk;
    hero.strafing = true;
    for tick in 1..=120 {
        hero.position += hero.velocity * TICK;
        let (position, aim) = step(&mut rig, &hero, &still(), &empty());
        assert!(rig.arm_yaw.abs() < 1e-6, "{}", rig.arm_yaw);
        // Once the aim has moved over, it is on the lagging look point.
        if tick >= 90 {
            assert_eq!(aim, rig.look + Vec3::Y * 1.5);
            let facing = (aim - position).normalize();
            assert!(facing.x.abs() < 1e-3, "{facing}");
        }
    }
}

#[test]
fn strafing_moves_the_aim_to_the_look_point_over_90_ticks() {
    // `camera_follow_motion` (0x3E39E0) aims between the follow point and the look point
    // by a count that rises a tick at a time to 90 while he strafes and falls the same way
    // once he stops (`camera.md`, step 8).
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.velocity = Vec3::X * 6.17;
    hero.state = HeroState::Walk;
    let mut count = 0;
    for tick in 0..200 {
        // Held for 30, let go for 10, then held.
        hero.strafing = !(30..40).contains(&tick);
        count = if hero.strafing {
            (count + 1).min(90)
        } else {
            count - 1
        };
        hero.position += hero.velocity * TICK;
        let (_, aim) = step(&mut rig, &hero, &still(), &empty());
        let wanted = rig.follow.lerp(rig.look, count as f32 / 90.0) + Vec3::Y * 1.5;
        assert!(
            (aim - wanted).length() < 1e-5,
            "tick {tick}: {aim} {wanted}"
        );
    }
    assert_eq!(count, 90);
}

#[test]
fn toggling_strafe_doesnt_jerk_the_view() {
    // Strafing sideways, then tapping strafe, the view used to jump between the look point
    // and the follow point beside it, 0.14 rad in a tick. Now it turns 0.011 at most.
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.velocity = Vec3::X * 6.17;
    hero.state = HeroState::Walk;
    let mut last = None;
    for tick in 0..300 {
        hero.strafing = tick < 200 || tick % 8 < 4;
        hero.position += hero.velocity * TICK;
        let (position, aim) = step(&mut rig, &hero, &still(), &empty());
        let view = (aim - position).normalize();
        if let Some(last) = last
            && tick >= 200
        {
            let turned = view.angle_between(last);
            assert!(turned < 0.02, "tick {tick}: {turned}");
        }
        last = Some(view);
    }
}

#[test]
fn locked_on_the_look_point_stiffens_and_the_aim_moves_to_it() {
    // Lock-on sets the look point's spring to 0.03 and moves the aim as strafing does
    // (`camera_follow_params`, 0x3E42C0). Running away from the camera at a target ahead,
    // the look point trails by (0.2 / 0.03 − 1) × v per tick.
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.velocity = Vec3::NEG_Z * 6.17;
    hero.state = HeroState::Walk;
    let input = FollowInput {
        lock: Some(Vec3::new(0.0, 1.0, -1000.0)),
        ..still()
    };
    let mut aim = Vec3::ZERO;
    for _ in 0..300 {
        hero.position += hero.velocity * TICK;
        (_, aim) = step(&mut rig, &hero, &input, &empty());
    }
    let lag = rig.follow.z - rig.look.z;
    assert!(
        (lag + (0.2 / 0.03 - 1.0) * 6.17 * TICK).abs() < 0.01,
        "{lag}"
    );
    assert_eq!(aim, rig.look + Vec3::Y * 1.5);
}

#[test]
fn the_look_point_trails_a_running_hero_by_1_27() {
    // camera.md, step 2: after each tick it is (0.2 / 0.015 − 1) × v per tick behind.
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.velocity = Vec3::NEG_Z * 6.17;
    hero.state = HeroState::Walk;
    for _ in 0..300 {
        hero.position += hero.velocity * TICK;
        step(&mut rig, &hero, &still(), &empty());
    }
    let lag = rig.follow.z - rig.look.z;
    assert!(
        (lag + (0.2 / 0.015 - 1.0) * 6.17 * TICK).abs() < 0.01,
        "{lag}"
    );
}

#[test]
fn the_distance_springs_back_as_in_the_trace() {
    // uyantsc-veldin-cam-1, frames 178299 and 178300: 5.0949955 then 5.087694; 300
    // frames later, 4.6434946 (camera.md, step 5).
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    let before = 5.094_995_5;
    rig.distance = before;
    step(&mut rig, &hero, &still(), &empty());
    let after = rig.distance;
    let mut expected = (after, after - before);
    for _ in 0..300 {
        step(&mut rig, &hero, &still(), &empty());
        let rate = expected.1 + 0.003 * (4.64 - expected.0) - 0.2 * expected.1;
        expected = (expected.0 + rate, rate);
    }
    assert!((rig.distance - expected.0).abs() < 1e-5, "{}", rig.distance);
}

#[test]
fn running_at_the_camera_pushes_it_back() {
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.velocity = Vec3::Z * 6.17;
    hero.state = HeroState::Walk;
    for _ in 0..60 {
        hero.position += hero.velocity * TICK;
        step(&mut rig, &hero, &still(), &empty());
    }
    assert!(rig.distance > 5.0, "{}", rig.distance);
}

#[test]
fn deadlockeds_camera_stays_6_u_back() {
    // DL's distance is the push-back's far end, so running at it changes nothing.
    let profile = HeroProfile::dl();
    let (mut rig, mut hero) = rest(&profile);
    hero.velocity = Vec3::Z * 7.25;
    hero.state = HeroState::Walk;
    for _ in 0..120 {
        hero.position += hero.velocity * TICK;
        follow::step(&mut rig, &hero, &still(), &profile, &empty());
        assert!((rig.distance - 6.0).abs() < 1e-6, "{}", rig.distance);
    }
}

#[test]
fn a_jump_does_not_lift_the_camera() {
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.state = HeroState::Jump(JumpKind::Standing);
    for tick in 0..30 {
        hero.position.y = 1.5 * ops::sin(tick as f32 / 30.0 * PI);
        hero.height_above_ground = hero.position.y;
        let (position, _) = step(&mut rig, &hero, &still(), &empty());
        assert!((position.y - 2.0).abs() < 1e-4, "{position}");
    }
    assert_eq!(rig.vertical, Vertical::Hold(0.0));
}

#[test]
fn a_heli_jump_raises_the_follow_point_3_u_over_85_ticks() {
    // camera.md, "The follow point's height" [verified on uyantsc-veldin-camvert-1].
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.state = HeroState::Jump(JumpKind::Heli);
    for tick in 1..=85 {
        step(&mut rig, &hero, &still(), &empty());
        let expected = 3.0 * tick as f32 / 85.0;
        assert!(
            (rig.follow.y - expected).abs() < 1e-5,
            "{tick}: {}",
            rig.follow.y
        );
    }
    assert_eq!(rig.vertical, Vertical::AfterRise);
    // Afterwards it follows him again.
    step(&mut rig, &hero, &still(), &empty());
    assert_eq!(rig.follow.y, 0.0);
}

#[test]
fn a_flip_rises_1_u_and_comes_back() {
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.state = HeroState::Jump(JumpKind::Flip);
    for _ in 0..35 {
        step(&mut rig, &hero, &still(), &empty());
    }
    assert!((rig.follow.y - 1.0).abs() < 1e-5, "{}", rig.follow.y);
    // Past the apex it comes back down over the same 35 ticks.
    hero.pass_apex();
    for tick in 1..=35 {
        step(&mut rig, &hero, &still(), &empty());
        let expected = 1.0 - tick as f32 / 35.0;
        assert!(
            (rig.follow.y - expected).abs() < 1e-5,
            "{tick}: {}",
            rig.follow.y
        );
    }
}

#[test]
fn the_look_point_barely_rises_in_the_air_for_small_gaps() {
    // camera.md: off the ground, the vertical gain is 0.002 + 0.0105 × min(gap, 2) / 2.
    let profile = uya();
    let (mut rig, mut hero) = rest(&profile);
    hero.state = HeroState::Fall;
    hero.position.y = 0.5;
    step(&mut rig, &hero, &still(), &empty());
    // Last tick's gap was 0, so the gain is 0.002 and the first step is 0.002 × 0.5.
    assert!((rig.look.y - 0.001).abs() < 1e-6, "{}", rig.look.y);
}

#[test]
fn a_low_ceiling_brings_the_pivot_and_height_down() {
    // camera.md, "Obstacles and collision" [verified on uyantsc-veldin-camceil-1]: under
    // a ceiling 2.6 u up the pivot sits 1.05 under it and the height springs there.
    let profile = uya();
    let ceiling = (Vec3::new(0.0, 3.1, 0.0), Vec3::new(1.0, 0.5, 1.0));
    let world = boxes(&[floor(), ceiling]);
    let (mut rig, mut hero) = rest(&profile);
    hero.height_above_ground = 0.0;
    for _ in 0..600 {
        step(&mut rig, &hero, &still(), &world);
    }
    let clearance =
        super::obstacles::clearance(&hero, rig.follow, rig.distance, &profile.camera, &world);
    assert!(
        (clearance.point.y - 1.55).abs() < 1e-4,
        "{}",
        clearance.point
    );
    let (position, _) = step(&mut rig, &hero, &still(), &world);
    assert!((position.y - 1.55).abs() < 0.01, "{position}");
    // A gap under 2.1 u puts it halfway.
    let low = (Vec3::new(0.0, 2.4, 0.0), Vec3::new(1.0, 0.5, 1.0));
    let world = boxes(&[floor(), low]);
    let clearance = super::obstacles::clearance(&hero, rig.follow, 2.0, &profile.camera, &world);
    assert!(
        (clearance.point.y - 0.95).abs() < 1e-4,
        "{}",
        clearance.point
    );
    assert!(
        clearance
            .height
            .is_some_and(|height| (height - 0.95).abs() < 1e-4)
    );
}

#[test]
fn the_player_turns_and_tilts_the_arm() {
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    step(&mut rig, &hero, &mouse(Vec2::new(0.1, 0.0)), &empty());
    assert!((rig.arm_yaw + 0.1).abs() < 1e-5);
    // Tilting is limited to 40° and to 0.0305 rad a tick.
    for _ in 0..200 {
        step(&mut rig, &hero, &mouse(Vec2::new(0.0, -0.1)), &empty());
    }
    assert!(
        (rig.arm_tilt - 40f32.to_radians()).abs() < 1e-4,
        "{}",
        rig.arm_tilt
    );
}

#[test]
fn the_stick_turns_the_arm_in_proportion_and_tilts_it_past_0_3() {
    // camera.md, step 4 [verified at half tilt]: 0.05015 × x rad a tick at speed 50; a y
    // of 0.2 doesn't tilt it.
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    let input = FollowInput {
        stick: Vec2::new(0.2105, 0.2),
        ..still()
    };
    step(&mut rig, &hero, &input, &empty());
    assert!(
        (rig.arm_yaw + 0.05015 * 0.2105).abs() < 1e-5,
        "{}",
        rig.arm_yaw
    );
    assert!(rig.arm_tilt.abs() < 1e-6);
    // Full up looks up: the arm lowers by 0.02 of 40° a tick, then comes back when let go.
    let up = FollowInput {
        stick: Vec2::new(0.0, 1.0),
        ..still()
    };
    step(&mut rig, &hero, &up, &empty());
    assert!(
        (rig.arm_tilt + 0.02 * 40f32.to_radians()).abs() < 1e-5,
        "{}",
        rig.arm_tilt
    );
    for _ in 0..100 {
        step(&mut rig, &hero, &up, &empty());
    }
    assert!((rig.arm_tilt + 40f32.to_radians()).abs() < 1e-4);
    for _ in 0..300 {
        step(&mut rig, &hero, &still(), &empty());
    }
    assert!(rig.arm_tilt.abs() < 1e-4, "{}", rig.arm_tilt);
}

#[test]
fn a_locked_target_turns_the_camera_to_look_past_the_hero_at_it() {
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    // A target off to the right: the arm has a right angle to turn, so the push is
    // full, at 1/400 of π/2 on the first tick.
    let input = FollowInput {
        lock: Some(Vec3::new(10.0, 1.0, 0.0)),
        ..still()
    };
    step(&mut rig, &hero, &input, &empty());
    assert!(
        (rig.arm_yaw + FRAC_PI_2 / 400.0).abs() < 1e-6,
        "{}",
        rig.arm_yaw
    );
    // It ramps up and settles with the camera looking at the target.
    let mut camera = (Vec3::ZERO, Vec3::ZERO);
    for _ in 0..600 {
        camera = step(&mut rig, &hero, &input, &empty());
    }
    let (position, _) = camera;
    let view = Vec3::new(-position.x, 0.0, -position.z).normalize();
    assert!(view.dot(Vec3::X) > 0.99, "{view}");
}

#[test]
fn turning_the_camera_overrides_the_lock() {
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    let input = FollowInput {
        lock: Some(Vec3::new(10.0, 1.0, 0.0)),
        mouse: Vec2::new(0.05, 0.0),
        ..still()
    };
    step(&mut rig, &hero, &input, &empty());
    assert!((rig.arm_yaw + 0.05).abs() < 1e-6, "{}", rig.arm_yaw);
}

#[test]
fn walls_keep_the_camera_in_front_of_them() {
    // A wall 2 u behind the hero.
    let profile = uya();
    let world = boxes(&[(Vec3::new(0.0, 0.0, 2.5), Vec3::new(10.0, 10.0, 0.5))]);
    let (mut rig, hero) = rest(&profile);
    for _ in 0..10 {
        let (position, _) = step(&mut rig, &hero, &still(), &world);
        assert!(position.z < 2.0 - 0.24, "{position}");
    }
    // The wall pulls the arm in, never under 1.5 u.
    assert!(rig.pull_in > 0.0);
    assert!(rig.arm_length() >= 1.5 - 1e-5);
}

#[test]
fn turning_into_a_wall_pulls_the_camera_in_faster() {
    let profile = uya();
    // A wall to the camera's left: turning right swings the camera into it.
    let world = boxes(&[(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(0.5, 10.0, 10.0))]);
    let (mut rig, hero) = rest(&profile);
    let turn = FollowInput {
        stick: Vec2::new(1.0, 0.0),
        ..still()
    };
    let mut bumped = false;
    for _ in 0..60 {
        step(&mut rig, &hero, &turn, &world);
        bumped |= rig.pull_in_timer > profile.camera.obstacles.pull_in_rest;
        assert!(rig.arm_length() >= 1.5 - 1e-5);
    }
    assert!(bumped);
}

#[test]
fn the_pull_in_eases_out_by_a_cosine() {
    // camera.md [verified on uyantsc-phoenix-camwall-1]: clear of obstacles with the stick
    // resting, the timer counts down and the pull-in shrinks by (1 − cos(π t / 2000)) / 2.
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    rig.pull_in = 2.0;
    rig.pull_in_timer = 2000;
    step(&mut rig, &hero, &still(), &empty());
    let factor = (1.0 - ops::cos(PI * 1999.0 / 2000.0)) / 2.0;
    assert!((rig.pull_in - 2.0 * factor).abs() < 1e-6, "{}", rig.pull_in);
    for _ in 0..400 {
        step(&mut rig, &hero, &still(), &empty());
    }
    assert!(rig.pull_in < 0.01, "{}", rig.pull_in);
}

#[test]
fn a_view_blocked_for_30_ticks_starts_again_behind_him() {
    // camera.md [verified on uyantsc-veldin-camblock-1]: the hero 1.5 u under the floor.
    let profile = uya();
    let world = boxes(&[(Vec3::new(0.0, -2.5, 0.0), Vec3::new(50.0, 2.5, 50.0))]);
    let (mut rig, mut hero) = rest(&profile);
    hero.position.y = -1.5;
    hero.yaw = 1.0;
    rig.follow.y = -1.5;
    rig.look.y = -1.5;
    for tick in 1..30 {
        step(&mut rig, &hero, &still(), &world);
        assert_eq!(rig.blocked_ticks, tick);
    }
    step(&mut rig, &hero, &still(), &world);
    assert_eq!(rig.blocked_ticks, 0);
    // Behind him along his facing.
    assert!((rig.arm_yaw - 1.0).abs() < 1e-6, "{}", rig.arm_yaw);
}

#[test]
fn a_volume_retunes_the_distance_and_its_gain_stays() {
    // camera.md, "Camera volumes" [verified on uyantsc-florana-camvol-4]: distance 8 at
    // gain 0.001, and back at 0.001 afterwards.
    let profile = uya();
    let (mut rig, hero) = rest(&profile);
    let inside = FollowInput {
        volume: VolumeSettings {
            distance: Some(8.0),
            height: Some(5.0),
            ..default()
        },
        ..still()
    };
    step(&mut rig, &hero, &inside, &empty());
    // The first step: 0.001 × (8 − 4.64).
    assert!(
        (rig.distance - (4.64 + 0.001 * 3.36)).abs() < 1e-6,
        "{}",
        rig.distance
    );
    for _ in 0..2000 {
        step(&mut rig, &hero, &inside, &empty());
    }
    assert!((rig.distance - 8.0).abs() < 0.01, "{}", rig.distance);
    let distance = rig.distance;
    step(&mut rig, &hero, &still(), &empty());
    assert!(
        (rig.distance - (distance + 0.001 * (4.64 - distance))).abs() < 0.001,
        "{}",
        rig.distance
    );
}

/// A hero, a camera and the input for driving [`FollowCamera::tick`].
struct Scene {
    profile: HeroProfile,
    hero: HeroMotion,
    camera: FollowCamera,
    world: ParryWorld,
}

impl Scene {
    fn new(profile: HeroProfile, setting: Option<CameraSetting>) -> Self {
        let mut camera = FollowCamera::for_player(0, 0.0);
        camera.setting = setting;
        Self {
            profile,
            hero: HeroMotion::new(Vec3::ZERO, 0.0),
            camera,
            world: boxes(&[floor()]),
        }
    }

    fn tick(&mut self, input: &PlayerInput) -> (Vec3, Quat) {
        self.camera.tick(
            &self.hero,
            input,
            None,
            &[],
            &[],
            &self.profile,
            &self.world,
        )
    }

    fn blend(&self) -> Option<Blend> {
        self.camera.state.as_ref().unwrap().blend
    }

    fn view(&self) -> View {
        self.camera.state.as_ref().unwrap().view
    }
}

fn holding(action: Action) -> PlayerInput {
    let mut held = Buttons::NONE;
    held.insert(action);
    PlayerInput { held, ..default() }
}

#[test]
fn the_lock_strafe_camera_doesnt_turn_as_he_walks() {
    // Deadlocked's default: rigid behind his eye, turned only by the player.
    let mut scene = Scene::new(HeroProfile::dl(), None);
    scene.hero.state = HeroState::Walk;
    let idle = PlayerInput::default();
    for _ in 0..30 {
        scene.tick(&idle);
    }
    for _ in 0..120 {
        scene.hero.position.x += 0.12;
        scene.hero.yaw = -FRAC_PI_2;
        let (position, rotation) = scene.tick(&idle);
        assert_eq!(scene.camera.yaw, 0.0);
        assert!((rotation * Vec3::NEG_Z - Vec3::NEG_Z).length() < 1e-5);
        // 6 u behind last tick's eye.
        assert!(
            (position.x - (scene.hero.position.x - 0.12)).abs() < 1e-4,
            "{position}"
        );
        assert!((position.z - 6.0).abs() < 1e-4, "{position}");
    }
    assert!(!scene.camera.is_first_person());
}

#[test]
fn the_look_button_gives_first_person_in_lock_strafe_and_blends_back() {
    let mut scene = Scene::new(uya(), Some(CameraSetting::LockStrafe));
    let idle = PlayerInput::default();
    for _ in 0..30 {
        scene.tick(&idle);
    }
    scene.hero.state = HeroState::Look;
    let look = holding(Action::FirstPerson);
    scene.tick(&look);
    // It cuts in.
    assert!(scene.camera.is_first_person());
    assert_eq!(scene.blend(), None);
    let (position, _) = scene.tick(&look);
    assert!((position.y - 1.75).abs() < 1e-4, "{position}");
    scene.hero.state = HeroState::Idle;
    scene.tick(&idle);
    // Back out, from 0.25 at 0.075 a tick: 10 ticks.
    let blend = scene.blend().unwrap();
    assert!((blend.position - 0.325).abs() < 1e-6, "{}", blend.position);
    // Nine more steps of 0.075 reach 1 (but for rounding, one more), and the next shows
    // the lock-strafe camera as it is.
    let mut ticks = 0;
    while scene.blend().is_some() {
        scene.tick(&idle);
        ticks += 1;
    }
    assert!((10..=11).contains(&ticks), "{ticks}");
}

#[test]
fn the_third_person_look_waits_7_ticks_then_blends_in() {
    let mut scene = Scene::new(uya(), None);
    let idle = PlayerInput::default();
    for _ in 0..10 {
        scene.tick(&idle);
    }
    scene.hero.state = HeroState::Look;
    let look = holding(Action::FirstPerson);
    // Meanwhile he keeps his facing, and the follow camera turns to it.
    for _ in 0..6 {
        scene.tick(&look);
        assert_eq!(scene.view(), View::Follow);
        assert!(!scene.camera.look_ready());
    }
    scene.tick(&look);
    assert_eq!(scene.view(), View::Look { ready: false });
    assert!(!scene.camera.look_ready());
    // Ready once the position blend is half done: 7 steps of 0.075.
    for _ in 0..5 {
        scene.tick(&look);
        assert!(!scene.camera.look_ready());
    }
    scene.tick(&look);
    assert!(scene.camera.look_ready());
    assert!(scene.camera.is_first_person());
    // Then the camera jumps to the eye while the rotation blends on. It sits at last
    // tick's eye, which takes the first-person height a tick later.
    scene.tick(&look);
    let (position, _) = scene.tick(&look);
    assert!((position.y - 1.75).abs() < 1e-4, "{position}");
    assert!(
        Vec2::new(position.x, position.z).length() < 0.05,
        "{position}"
    );
    // Leaving blends back from 0.25 at 0.018 a tick, once its own blend is over.
    for _ in 0..20 {
        scene.tick(&look);
    }
    scene.hero.state = HeroState::Idle;
    scene.tick(&idle);
    assert_eq!(scene.view(), View::Follow);
    let blend = scene.blend().unwrap();
    assert!((blend.position - 0.268).abs() < 1e-6, "{}", blend.position);
}

#[test]
fn the_look_turns_only_with_a_stick_past_half() {
    let mut scene = Scene::new(uya(), None);
    scene.hero.state = HeroState::Look;
    let look = holding(Action::FirstPerson);
    for _ in 0..40 {
        scene.tick(&look);
    }
    let yaw = scene.camera.yaw;
    // 64 counts is 0.21 after the pad's dead zone: under the look's threshold.
    let mut half = holding(Action::FirstPerson);
    half.look_stick = [64, 0];
    for _ in 0..30 {
        scene.tick(&half);
    }
    assert_eq!(scene.camera.yaw, yaw);
    half.look_stick = [127, 0];
    scene.tick(&half);
    assert!(scene.camera.yaw < yaw);
}

#[test]
fn the_first_person_setting_sits_at_the_eye() {
    let mut scene = Scene::new(HeroProfile::dl(), Some(CameraSetting::FirstPerson));
    let idle = PlayerInput::default();
    scene.tick(&idle);
    let (position, _) = scene.tick(&idle);
    assert!(
        (position - Vec3::new(0.0, 1.15, 0.0)).length() < 1e-4,
        "{position}"
    );
    assert!(scene.camera.is_first_person());
}

#[test]
fn changing_the_setting_blends_to_the_new_camera() {
    let mut scene = Scene::new(uya(), None);
    let idle = PlayerInput::default();
    for _ in 0..10 {
        scene.tick(&idle);
    }
    scene.camera.setting = Some(CameraSetting::LockStrafe);
    let (position, _) = scene.tick(&idle);
    assert_eq!(scene.view(), View::Hero);
    // The first tick still shows where it was.
    assert!(
        (position - Vec3::new(0.0, 2.0, 4.64)).length() < 1e-3,
        "{position}"
    );
    for _ in 0..14 {
        scene.tick(&idle);
    }
    assert_eq!(scene.blend(), None);
}

/// The real hero on the floor and his camera, ticked together; returns the largest change
/// of the view's pitch in one tick, and of the follow point's height.
fn largest_steps(scene: &mut Scene, inputs: &[PlayerInput]) -> (f32, f32) {
    let mut last: Option<(f32, f32)> = None;
    let mut steps = (0.0_f32, 0.0_f32);
    for input in inputs {
        let yaw = scene.camera.yaw;
        crate::controller::step(&mut scene.hero, input, yaw, &scene.profile, &scene.world);
        scene.tick(input);
        let now = (
            scene.camera.pitch,
            scene.camera.state.as_ref().unwrap().rig.follow.y,
        );
        if let Some(before) = last {
            steps.0 = steps.0.max((now.0 - before.0).abs());
            steps.1 = steps.1.max((now.1 - before.1).abs());
        }
        last = Some(now);
    }
    steps
}

fn pressing(held: &[Action], pressed: &[Action]) -> PlayerInput {
    let mut input = PlayerInput::default();
    for &action in held.iter().chain(pressed) {
        input.held.insert(action);
    }
    for &action in pressed {
        input.pressed.insert(action);
    }
    input
}

#[test]
fn gliding_after_a_double_or_heli_jump_doesnt_jolt_the_view() {
    // The glide isn't a jump-type state, so the jump's hold or rise ends and the follow
    // point snaps to his height (`camera.md`, "The follow point's height"). The view's tilt
    // comes from the arm and the heights, not the aim (0x3E7B50), so it doesn't follow:
    // pitching at the aim, it jumped 0.62 rad in a tick.
    let jump = pressing(&[], &[Action::Jump]);
    let hold = pressing(&[Action::Jump], &[]);
    let crouch = pressing(&[Action::Crouch], &[]);
    let mut double = vec![PlayerInput::default(); 10];
    double.push(jump);
    double.extend(vec![hold; 16]);
    double.extend(vec![PlayerInput::default(); 8]);
    double.push(jump);
    double.extend(vec![hold; 60]);
    let mut heli = vec![PlayerInput::default(); 10];
    heli.push(pressing(&[Action::Crouch], &[Action::Crouch]));
    heli.extend(vec![crouch; 7]);
    heli.push(pressing(&[Action::Crouch], &[Action::Jump]));
    heli.extend(vec![hold; 110]);
    for (take, inputs) in [("double", double), ("heli", heli)] {
        let mut scene = Scene::new(uya(), None);
        let (pitch, follow) = largest_steps(&mut scene, &inputs[..inputs.len() - 1]);
        assert_eq!(scene.hero.state, HeroState::Glide, "{take}");
        assert!(
            follow > 0.5,
            "{take}: the follow point moved {follow} at most"
        );
        assert!(pitch < 0.03, "{take}: the view pitched {pitch} in a tick");
    }
}

#[test]
fn the_view_tilts_up_with_the_arm_below_level_on_the_ground() {
    // 0x3E7B50: on good ground, with the dragged arm x of the pitch limit below level
    // (x past 0.1), the view tilts up towards 2 × min(x, 1 − x) × 15°. At −20° the arm's
    // angle reads atan(sin 20°) = 18.88°, so x = 0.472 and the view tilts up 14.16° more
    // than in the air [verified on uyantsc-veldin-camhalf-0.75-1 to 0.00002 rad].
    let profile = uya();
    let mut pitches = Vec::new();
    for on_ground in [false, true] {
        let (mut rig, mut hero) = rest(&profile);
        if on_ground {
            hero.ground = Some(crate::motor::Ground {
                normal: Vec3::Y,
                body: crate::collision::BodyId(0),
            });
        }
        // Looking up lowers the camera.
        step(
            &mut rig,
            &hero,
            &mouse(Vec2::new(0.0, 20f32.to_radians())),
            &empty(),
        );
        for _ in 0..600 {
            step(&mut rig, &hero, &still(), &empty());
        }
        assert!(
            (rig.arm_tilt + 20f32.to_radians()).abs() < 1e-4,
            "{}",
            rig.arm_tilt
        );
        pitches.push(rig.view.1);
    }
    let look_up = pitches[1] - pitches[0];
    let expected = 2.0 * (ops::atan(ops::sin(20f32.to_radians())) / 40f32.to_radians()) * 15.0;
    assert!(
        (look_up - expected.to_radians()).abs() < 1e-4,
        "{} {}",
        look_up.to_degrees(),
        expected
    );
}

#[test]
fn a_guns_zoom_springs_the_field_of_view_and_the_camera_keeps_it() {
    let mut scene = Scene::new(uya(), Some(CameraSetting::FirstPerson));
    let idle = PlayerInput::default();
    scene.tick(&idle);
    assert_eq!(scene.camera.zoom(), 1.0);
    let fov = |scene: &Scene| scene.camera.state.as_ref().unwrap().fov;
    let narrow = 20f32.to_radians();
    scene.hero.gun.zoom = Some(crate::Zoom {
        fov: narrow,
        spring: Spring {
            gain: 0.02,
            damping: 0.2,
        },
    });
    let mut last = fov(&scene);
    for _ in 0..300 {
        scene.tick(&idle);
        // In without passing it.
        assert!(fov(&scene) <= last && fov(&scene) >= narrow);
        last = fov(&scene);
    }
    assert!((last - narrow).abs() < 1e-3, "{last}");
    // It stays there once the gun stops asking, and the free look turns slower by it.
    scene.hero.gun.zoom = None;
    scene.tick(&idle);
    assert!((fov(&scene) - narrow).abs() < 1e-3);
    let free = &scene.camera.state.as_ref().unwrap().free;
    assert!((free.zoom - narrow / 1.1100293).abs() < 1e-3);
    let tangents = ops::tan(narrow / 2.0) / ops::tan(1.1100293 / 2.0);
    assert!((scene.camera.zoom() - tangents).abs() < 1e-3);
}
