//! A gun in hand against `docs/research/weapons.md`, "The hero's side of firing": the
//! flags the weapons code sets in `HeroMotion::gun`, and what they change in his moves.
//! The walk's turn while the N60 fires and the release with a gun are checked against
//! UYA NTSC traces there; the rest against the code's numbers.

use bevy::prelude::*;
use rac_hero::{
    BodyPlacement, CameraSetting, Collider, GunArm, GunHand, GunPose, HeroMotion, HeroProfile,
    HeroState, Hold, LockOn, ParryWorld,
    controller::{TICK, step},
    lock_on,
};
use rac_input::{Action, Buttons, PlayerInput};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

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

const FORWARD: [i8; 2] = [0, 127];
const RIGHT: [i8; 2] = [127, 0];

/// A gun in hand, firing or not.
fn gun(firing: bool, held: bool) -> GunHand {
    GunHand {
        out: true,
        firing,
        firing_held: firing && held,
        steady: true,
        ..Default::default()
    }
}

struct Rig {
    hero: HeroMotion,
    profile: HeroProfile,
    world: ParryWorld,
    camera_yaw: f32,
    /// Kept in his hand every tick, as the weapons code would.
    hand: GunHand,
}

impl Rig {
    fn new(hand: GunHand) -> Self {
        let floor = BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(100.0, 0.5, 100.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        );
        let mut rig = Self {
            hero: HeroMotion::new(Vec3::ZERO, 0.0),
            profile: HeroProfile::uya(),
            world: ParryWorld::new(&[floor]),
            camera_yaw: 0.0,
            hand,
        };
        rig.tick(&idle());
        rig
    }

    fn tick(&mut self, input: &PlayerInput) {
        self.hero.gun = self.hand;
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

    /// Walks forward to full speed.
    fn run_up(&mut self) {
        self.ticks(20, &input(Some(FORWARD), &[], &[]));
        assert_eq!(self.hero.speed, 6.17);
    }
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

#[test]
fn a_held_gun_firing_turns_the_walk_on_the_steady_spring() {
    // "Checked in traces": with the N60 firing and the stick full and turned 90°, each
    // frame's rate is rate + 0.017 × (move angle − last yaw) − 0.18 × rate, capped at
    // 0.1454.
    let mut rig = Rig::new(gun(true, true));
    rig.run_up();
    let target = -FRAC_PI_2;
    let mut rate = rig.hero.yaw_rate;
    let mut yaw = rig.hero.yaw;
    for tick in 0..20 {
        rig.tick(&input(Some(RIGHT), &[], &[]));
        let error = wrap(target - yaw);
        rate = (rate + 0.017 * error - 0.18 * rate).clamp(-0.1454, 0.1454);
        yaw = wrap(yaw + rate);
        assert!(
            (rig.hero.yaw - yaw).abs() < 1e-5,
            "tick {tick}: {} against {yaw}",
            rig.hero.yaw
        );
    }
    // The firing turn is far slower than the moving walk's 0.037 and 0.13.
    let mut free = Rig::new(GunHand::default());
    free.run_up();
    let mut firing = Rig::new(gun(true, true));
    firing.run_up();
    for _ in 0..6 {
        free.tick(&input(Some(RIGHT), &[], &[]));
        firing.tick(&input(Some(RIGHT), &[], &[]));
    }
    assert!(
        free.hero.yaw < firing.hero.yaw - 0.2,
        "{} against {}",
        free.hero.yaw,
        firing.hero.yaw
    );
}

#[test]
fn a_held_gun_firing_doesnt_speed_up_while_the_stick_is_well_round() {
    // As for a part-pushed stick: more than 45° round, the speed doesn't rise.
    let mut rig = Rig::new(gun(true, true));
    rig.tick(&input(Some([0, -127]), &[], &[]));
    rig.ticks(5, &input(Some([0, -127]), &[], &[]));
    assert_eq!(rig.hero.state, HeroState::Walk);
    assert_eq!(rig.hero.speed, 0.0);
    let mut free = Rig::new(GunHand::default());
    free.ticks(6, &input(Some([0, -127]), &[], &[]));
    assert!(free.hero.speed > 0.0);
}

#[test]
fn with_a_gun_letting_go_brakes_a_tick_and_idles_without_the_skid() {
    // "No release skid with a gun": a tick of the walk's 30 u/s², then idle at 1.5 u/s;
    // the wrench's two ticks of skid at 17 u/s² are in feel.rs.
    let mut rig = Rig::new(gun(false, false));
    rig.run_up();
    rig.tick(&idle());
    assert!(
        (rig.hero.speed - (6.17 - 30.0 * TICK)).abs() < 1e-5,
        "{}",
        rig.hero.speed
    );
    assert_eq!(rig.hero.state, HeroState::Walk);
    rig.tick(&idle());
    assert_eq!(rig.hero.state, HeroState::Idle);
    assert_eq!(rig.hero.speed, 1.5);
}

#[test]
fn no_double_jump_while_a_gun_fires() {
    // "No double jump while firing": a press inside the double jump's window does
    // nothing while `firing` is set.
    for (firing, doubles) in [(true, false), (false, true)] {
        let mut rig = Rig::new(gun(firing, false));
        rig.tick(&input(None, &[], &[Action::Jump]));
        rig.ticks(9, &idle());
        rig.tick(&input(None, &[], &[Action::Jump]));
        let mut doubled = false;
        for _ in 0..20 {
            rig.tick(&idle());
            doubled |= rig.hero.state == HeroState::DoubleJump;
        }
        assert_eq!(doubled, doubles, "firing {firing}");
    }
}

#[test]
fn in_the_lock_strafe_camera_a_gun_makes_him_strafe_and_the_wrench_doesnt() {
    // "The hero in these modes": with anything but the wrench in hand he strafes.
    for (hand, strafes) in [(gun(false, false), true), (GunHand::default(), false)] {
        let mut rig = Rig::new(hand);
        rig.hero.camera_setting = CameraSetting::LockStrafe;
        rig.ticks(20, &input(Some(RIGHT), &[], &[]));
        assert_eq!(rig.hero.strafing, strafes);
        if strafes {
            // Sideways, still facing the camera's way.
            assert_eq!(rig.hero.yaw, 0.0);
            assert!(rig.hero.velocity.x > 6.0, "{}", rig.hero.velocity);
        } else {
            assert!(
                wrap(rig.hero.yaw + FRAC_PI_2).abs() < 0.01,
                "{}",
                rig.hero.yaw
            );
        }
    }
    // The follow camera never forces it.
    let mut rig = Rig::new(gun(false, false));
    rig.ticks(5, &input(Some(RIGHT), &[], &[]));
    assert!(!rig.hero.strafing);
}

#[test]
fn firing_in_the_lock_strafe_camera_triples_the_strafe_turn() {
    // The strafe's gain 0.042 and cap 0.32, tripled while firing or just after a press.
    for (firing, gain) in [(false, 0.042), (true, 0.126)] {
        let mut rig = Rig::new(gun(firing, false));
        rig.hero.camera_setting = CameraSetting::LockStrafe;
        rig.camera_yaw = FRAC_PI_2;
        rig.tick(&input(Some(FORWARD), &[], &[]));
        let rate = gain * FRAC_PI_2;
        assert!(
            (rig.hero.yaw - rate).abs() < 1e-5,
            "{} against {rate}",
            rig.hero.yaw
        );
    }
    // A fire press counts for two ticks.
    let mut rig = Rig::new(gun(false, false));
    rig.hero.camera_setting = CameraSetting::LockStrafe;
    rig.camera_yaw = FRAC_PI_2;
    rig.tick(&input(Some(FORWARD), &[], &[Action::Fire]));
    assert!((rig.hero.yaw - 0.126 * FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn melee_puts_the_gun_away_and_swings_once_the_wrench_is_out() {
    // uyantsc-veldin-fire-2, and DL's melee traces: Square with a gun out emptied his hand
    // at once, and he swung 5 ticks later, as the wrench came into it.
    let mut rig = Rig::new(gun(false, false));
    rig.tick(&input(None, &[], &[Action::Melee]));
    assert_eq!(rig.hero.gun, GunHand::default());
    // The weapons code puts the gun away in turn.
    rig.hand = GunHand::default();
    for tick in 0..5 {
        assert!(!rig.hero.state.is_attacking(), "tick {tick}");
        rig.tick(&idle());
    }
    assert_eq!(rig.hero.state, HeroState::Combo(0));
}

#[test]
fn while_the_quick_select_ring_holds_his_input_he_takes_none() {
    // The kit's ring doesn't pause the game, so the stick and buttons that work it
    // mustn't move him, as UYA blocks the pad's gameplay input once its ring is up.
    let selecting = GunHand {
        selecting: true,
        ..default()
    };
    let mut rig = Rig::new(selecting);
    let start = rig.hero.position;
    rig.ticks(30, &input(Some(RIGHT), &[], &[]));
    assert_eq!(rig.hero.state, HeroState::Idle);
    assert_eq!(rig.hero.speed, 0.0);
    assert_eq!(rig.hero.position, start);
    assert_eq!(rig.hero.yaw, 0.0);
    rig.tick(&input(Some(RIGHT), &[Action::Jump], &[Action::Jump]));
    assert_eq!(rig.hero.state, HeroState::Idle);

    // Running when it opens, he stops as if the stick were let go.
    let mut rig = Rig::new(gun(false, false));
    rig.run_up();
    rig.hand = GunHand {
        selecting: true,
        ..rig.hand
    };
    rig.ticks(10, &input(Some(FORWARD), &[], &[]));
    assert_eq!(rig.hero.state, HeroState::Idle);
    assert_eq!(rig.hero.speed, 0.0);
    // Once it closes, the same stick walks him again.
    rig.hand.selecting = false;
    rig.ticks(6, &input(Some(FORWARD), &[], &[]));
    assert!(rig.hero.speed > 0.0);
}

#[test]
fn lock_on_waits_for_the_weapons_code_to_allow_it() {
    let rig = Rig::new(GunHand::default());
    let target = Entity::from_raw_u32(1).unwrap();
    let targets = [(target, Vec3::new(0.0, 1.0, -8.0))];
    let press = input(None, &[], &[Action::LockOn]);
    for (allowed, locks) in [(true, true), (false, false)] {
        let mut lock = LockOn {
            allowed,
            ..default()
        };
        lock_on::update(
            &mut lock,
            &rig.hero,
            &press,
            0.0,
            &targets,
            &rig.profile.lock_on,
            &rig.world,
        );
        assert_eq!(lock.target.is_some(), locks, "allowed {allowed}");
    }
}

/// A gun held in one hand, out and not firing.
fn held_gun() -> GunHand {
    GunHand {
        out: true,
        item: Some(0x2F),
        pose: GunPose {
            hold: Hold::OneHand,
            ..default()
        },
        ..default()
    }
}

#[test]
fn firing_in_deadlocked_s_gun_cameras_turns_at_least_at_0_2() {
    // "Deadlocked's side of firing": the gain raised to 0.2 in lock-strafe and first
    // person, where UYA's first person doesn't change it.
    for setting in [CameraSetting::LockStrafe, CameraSetting::FirstPerson] {
        for (profile, firing, gain) in [
            (HeroProfile::dl(), false, 0.042),
            (HeroProfile::dl(), true, 0.2),
            (HeroProfile::uya(), true, 0.126),
        ] {
            let mut rig = Rig::new(gun(firing, false));
            rig.profile = profile;
            rig.hero.camera_setting = setting;
            rig.camera_yaw = FRAC_PI_2;
            rig.tick(&input(Some(FORWARD), &[], &[]));
            let gain = if setting == CameraSetting::FirstPerson && gain == 0.126 {
                0.042
            } else {
                gain
            };
            let rate = gain * FRAC_PI_2;
            assert!(
                (rig.hero.yaw - rate).abs() < 1e-5,
                "{setting:?}: {} against {rate}",
                rig.hero.yaw
            );
        }
    }
}

/// The gun arm with a wall across his path, its near face `gap` ahead of his feet.
fn arm_with_wall_at(profile: HeroProfile, gap: f32) -> GunArm {
    let mut rig = Rig::new(held_gun());
    rig.profile = profile;
    // Ahead is −z.
    let wall = BodyPlacement::fixed(
        Collider::Cuboid {
            half_extents: Vec3::new(5.0, 3.0, 0.5),
        },
        Vec3::new(0.0, 3.0, -gap - 0.5),
        Quat::IDENTITY,
    );
    let floor = BodyPlacement::fixed(
        Collider::Cuboid {
            half_extents: Vec3::new(100.0, 0.5, 100.0),
        },
        Vec3::new(0.0, -0.5, 0.0),
        Quat::IDENTITY,
    );
    rig.world = ParryWorld::new(&[floor, wall]);
    rig.tick(&idle());
    rig.hero.gun_arm
}

#[test]
fn deadlocked_raises_the_gun_arm_for_walls_further_out_and_less() {
    // Its ray counts 1.3 u from its start, 0.43 u ahead, and the guns' arm turns 30°–40°.
    assert!(!arm_with_wall_at(HeroProfile::uya(), 1.5).raised);
    let far = arm_with_wall_at(HeroProfile::dl(), 1.5);
    assert!(far.raised);
    assert!((far.pitch.to_degrees() - 30.0).abs() < 0.01, "{far:?}");
    assert!(!arm_with_wall_at(HeroProfile::dl(), 1.8).raised);
    let d = Vec2::new(0.6, 0.57).length() - 0.45;
    let expected = (40.0 * 1.25 * (1.3 - d) / 1.3).clamp(30.0, 40.0);
    let near = arm_with_wall_at(HeroProfile::dl(), 0.6);
    assert!(
        (near.pitch.to_degrees() - expected).abs() < 0.01,
        "{near:?}"
    );
}

#[test]
fn in_deadlocked_s_first_person_the_gun_arm_follows_the_pitch_10_lower() {
    let mut rig = Rig::new(held_gun());
    rig.profile = HeroProfile::dl();
    rig.hero.camera_setting = CameraSetting::FirstPerson;
    for (pitch, expected) in [
        (30.0_f32, 20.0),
        (-5.0, -15.0),
        (-15.0, -20.0),
        (95.0, 80.0),
    ] {
        rig.hero.camera_pitch = pitch.to_radians();
        rig.tick(&idle());
        assert!(!rig.hero.gun_arm.raised);
        assert!(
            (rig.hero.gun_arm.pitch.to_degrees() - expected).abs() < 1e-3,
            "{pitch}: {}",
            rig.hero.gun_arm.pitch.to_degrees()
        );
    }
    // UYA's arm doesn't follow the first-person camera.
    let mut rig = Rig::new(held_gun());
    rig.hero.camera_setting = CameraSetting::FirstPerson;
    rig.hero.camera_pitch = 0.5;
    rig.tick(&idle());
    assert_eq!(rig.hero.gun_arm, GunArm::default());
}

#[test]
fn a_wall_close_ahead_raises_the_gun_arm_the_nearer_the_more() {
    // "The raised gun arm": a ray 0.57 u up from just inside his radius to 1.3 u ahead;
    // a hit at d from his feet raises the arm by 70° × 1.25 × (1.3 − (d − 0.45)) / 1.3,
    // kept within 30° and 70°.
    let raised_by = |gap: f32| {
        let mut rig = Rig::new(held_gun());
        // A wall across his path, its near face `gap` ahead of his feet (ahead is −z).
        let wall = BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(5.0, 3.0, 0.5),
            },
            Vec3::new(0.0, 3.0, -gap - 0.5),
            Quat::IDENTITY,
        );
        let floor = BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(100.0, 0.5, 100.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        );
        rig.world = ParryWorld::new(&[floor, wall]);
        rig.tick(&idle());
        rig.hero.gun_arm
    };
    let far = raised_by(2.0);
    assert!(!far.raised && far.pitch == 0.0, "{far:?}");
    for gap in [0.6_f32, 0.9, 1.2] {
        let arm = raised_by(gap);
        let d = Vec2::new(gap, 0.57).length() - 0.45;
        let expected = (70.0 * 1.25 * (1.3 - d) / 1.3).clamp(30.0, 70.0);
        assert!(arm.raised, "{gap}");
        assert!(
            (arm.pitch.to_degrees() - expected).abs() < 0.01,
            "{gap}: {} against {expected}",
            arm.pitch.to_degrees()
        );
    }
    assert!(raised_by(0.6).pitch > raised_by(1.2).pitch);

    // The wrench, or a glove held by no grip, raises nothing.
    let mut rig = Rig::new(GunHand::default());
    rig.tick(&idle());
    assert_eq!(rig.hero.gun_arm, GunArm::default());
}

#[test]
fn in_the_lock_strafe_camera_the_gun_arm_follows_its_pitch() {
    let mut rig = Rig::new(held_gun());
    rig.hero.camera_setting = CameraSetting::LockStrafe;
    for (pitch, expected) in [
        (20.0_f32, 20.0),
        (-30.0, -30.0),
        (85.0, 80.0),
        (-60.0, -45.0),
    ] {
        rig.hero.camera_pitch = pitch.to_radians();
        rig.tick(&idle());
        assert!(!rig.hero.gun_arm.raised);
        assert!((rig.hero.gun_arm.pitch.to_degrees() - expected).abs() < 1e-3);
    }
}
