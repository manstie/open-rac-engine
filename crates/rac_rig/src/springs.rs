//! UYA's joint springs over the hero's animation (`weapons.md`, "The hero's joint
//! springs"): after his animation poses him, a few of his joints turn by angles of their
//! own, each eased on a spring towards a target the game sets afresh every frame
//! (`hero_joint_springs`, 0x2F4580; `hero_joint_spring_step`, 0x4DC480). The kit plays
//! the two a gun drives, in the game's order:
//! - the raised gun arm (`hero_gun_arm_raise`, 0x4E26F8; [`rac_hero::GunArm`]): the
//!   clavicles turn the arm up, both of them for a gun held in both hands, and past 35°
//!   the waist bends back for the rest;
//! - the turn to the gun's target (`hero_head_look`, 0x4E2F50; [`rac_hero::GunHand::target`]):
//!   the upper chest turns to its yaw and the waist to its pitch.
//!
//! Each spring holds three angles about the joint's own x, y and z in UYA's axes (x
//! forward, y left, z up), turned in that order after the animation's turn. Its gain and
//! damping stay as last set; its targets go back to 0 every frame, so a joint no rule
//! drives springs home.

use bevy::math::ops;
use bevy::prelude::*;
use rac_hero::{HeroMotion, HeroState, Hold};
use std::f32::consts::{PI, TAU};

use crate::model::joint;

/// One joint's spring: its angles, their rates a tick, this tick's targets, and the
/// spring's gain and damping a tick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Spring {
    joint: usize,
    angles: Vec3,
    rates: Vec3,
    target: Vec3,
    gain: f32,
    damping: f32,
}

impl Spring {
    const fn new(joint: usize, gain: f32, damping: f32) -> Self {
        Self {
            joint,
            angles: Vec3::ZERO,
            rates: Vec3::ZERO,
            target: Vec3::ZERO,
            gain,
            damping,
        }
    }

    fn set(&mut self, gain: f32, damping: f32) -> &mut Self {
        self.gain = gain;
        self.damping = damping;
        self
    }

    /// A tick of each angle's spring (0x445010): the rate, kept within the gap, gains
    /// `gain` of the gap and loses `damping` of itself, is kept within the gap again, and
    /// moves the angle.
    fn step(&mut self) {
        for axis in 0..3 {
            let gap = wrap(self.target[axis] - self.angles[axis]);
            let most = gap.abs();
            let rate = self.rates[axis].clamp(-most, most);
            let rate = (rate + self.gain * gap - self.damping * rate).clamp(-most, most);
            self.rates[axis] = rate;
            self.angles[axis] = wrap(self.angles[axis] + rate);
        }
    }

    /// The turn in the rig's axes: about UYA's x (the rig's −z), then y (−x), then z (y).
    fn turn(&self) -> Quat {
        Quat::from_axis_angle(Vec3::NEG_Z, self.angles.x)
            * Quat::from_axis_angle(Vec3::NEG_X, self.angles.y)
            * Quat::from_axis_angle(Vec3::Y, self.angles.z)
    }
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// The springs, with the gains Veldin's image holds before anything sets them [data]:
/// the waist (point 10) 0.008, the upper chest (point 11) 0.014, the clavicles (points
/// 12 and 13) none, all damped 0.3.
const WAIST: usize = 0;
const CHEST: usize = 1;
const CLAVICLE_R: usize = 2;
const CLAVICLE_L: usize = 3;

/// The arm raised past this, the waist bends back for the rest (−35° in UYA's sign).
const ARM_MOST: f32 = 35.0;
/// The raise's springs, and the Annihilator's and RY3NO's roll of the arm as it rises,
/// up to 40°.
const RAISE_SPRING: (f32, f32) = (0.05, 0.3);
const RAISE_ROLL: f32 = 0.009_973_31;
const RAISE_ROLL_MOST: f32 = 40.0;
const ROLLED: [u16; 2] = [0x3F, 0x97];
/// The turn to the gun's target: from 0.5 u ahead of his feet and 0.55 u up, its yaw
/// within 59° each way and its pitch from 25° up to 35° down, on springs of 0.037 and 0.3.
const LOOK_FROM: Vec2 = Vec2::new(0.5, 0.55);
const LOOK_YAW_MOST: f32 = 59.0;
const LOOK_UP_MOST: f32 = 25.0;
const LOOK_DOWN_MOST: f32 = 35.0;
const LOOK_SPRING: (f32, f32) = (0.037, 0.3);

/// What a hero's joint springs remember between frames.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JointSprings {
    springs: [Spring; 4],
    /// The fraction of a tick not yet stepped.
    pending: f32,
}

impl Default for JointSprings {
    fn default() -> Self {
        Self {
            springs: [
                Spring::new(joint::WAIST, 0.008, 0.3),
                Spring::new(joint::CHEST_UPPER, 0.014, 0.3),
                Spring::new(joint::CLAVICLE_R, 0.0, 0.0),
                Spring::new(joint::CLAVICLE_L, 0.0, 0.0),
            ],
            pending: 0.0,
        }
    }
}

/// The states his head and upper body turn to a target in: UYA's state types 0, 1, 2
/// and 4 (standing, walking, falling and jumping; its type 7 isn't read).
fn turns_to_target_in(state: HeroState) -> bool {
    matches!(
        state,
        HeroState::Idle
            | HeroState::Look
            | HeroState::Walk
            | HeroState::Skid
            | HeroState::Fall
            | HeroState::Jump(_)
    )
}

impl JointSprings {
    /// Moves the springs on by `seconds` of `hero`, a tick at a time.
    pub fn step(&mut self, hero: &HeroMotion, seconds: f32) {
        self.pending += seconds * 60.0;
        while self.pending >= 1.0 {
            self.pending -= 1.0;
            self.tick(hero);
        }
    }

    fn tick(&mut self, hero: &HeroMotion) {
        for spring in &mut self.springs {
            spring.target = Vec3::ZERO;
        }
        self.raise(hero);
        self.look(hero);
        for spring in &mut self.springs {
            spring.step();
        }
    }

    /// The raised gun arm, in UYA's sign: negative turns it up.
    fn raise(&mut self, hero: &HeroMotion) {
        let mut arm = -hero.gun_arm.pitch;
        if arm == 0.0 {
            return;
        }
        let most = -ARM_MOST.to_radians();
        if arm < most {
            let (gain, damping) = RAISE_SPRING;
            self.springs[WAIST].set(gain, damping).target.y = arm - most;
            arm = most;
        }
        let roll = if hero.gun.item.is_some_and(|item| ROLLED.contains(&item)) {
            (arm * RAISE_ROLL).min(RAISE_ROLL_MOST.to_radians())
        } else {
            0.0
        };
        // The left clavicle too for a gun held in both hands.
        let last = if hero.gun.pose.hold == Hold::TwoHands {
            CLAVICLE_L
        } else {
            CLAVICLE_R
        };
        let (gain, damping) = RAISE_SPRING;
        for spring in &mut self.springs[CLAVICLE_R..=last] {
            spring.set(gain, damping).target = Vec3::new(roll, arm, 0.0);
        }
    }

    /// The turn to the gun's target: yaw on the upper chest, pitch (down positive) on the
    /// waist.
    fn look(&mut self, hero: &HeroMotion) {
        let Some(target) = hero.gun.target.filter(|_| turns_to_target_in(hero.state)) else {
            return;
        };
        let from = hero.position + hero.facing() * LOOK_FROM.x + Vec3::Y * LOOK_FROM.y;
        let to = target - from;
        let yaw = wrap(ops::atan2(-to.x, -to.z) - hero.yaw)
            .clamp(-LOOK_YAW_MOST.to_radians(), LOOK_YAW_MOST.to_radians());
        let across = Vec2::new(to.x, to.z).length();
        let pitch = ops::atan2(-to.y, across)
            .clamp(-LOOK_UP_MOST.to_radians(), LOOK_DOWN_MOST.to_radians());
        let (gain, damping) = LOOK_SPRING;
        self.springs[CHEST].set(gain, damping).target.z = yaw;
        self.springs[WAIST].set(gain, damping).target.y = pitch;
    }

    /// The turn joint `index` takes after its animation's, in the rig's axes.
    pub fn turn(&self, index: usize) -> Option<Quat> {
        self.springs
            .iter()
            .find(|spring| spring.joint == index && spring.angles != Vec3::ZERO)
            .map(Spring::turn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::{GunArm, GunHand, GunPose};

    fn hero_with(gun: GunHand, arm: GunArm) -> HeroMotion {
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.gun = gun;
        hero.gun_arm = arm;
        hero
    }

    fn held(item: u16, hold: Hold) -> GunHand {
        GunHand {
            out: true,
            item: Some(item),
            pose: GunPose { hold, ..default() },
            ..default()
        }
    }

    fn settle(springs: &mut JointSprings, hero: &HeroMotion) {
        for _ in 0..600 {
            springs.tick(hero);
        }
    }

    #[test]
    fn the_spring_closes_the_gap_without_passing_it() {
        let mut spring = Spring::new(0, 0.037, 0.3);
        spring.target.z = 1.0;
        let mut last = 0.0;
        for _ in 0..400 {
            spring.step();
            assert!(spring.angles.z >= last && spring.angles.z <= 1.0);
            last = spring.angles.z;
        }
        assert!((spring.angles.z - 1.0).abs() < 1e-3);
        // Its first tick moves the angle by the gain's share of the gap.
        let mut spring = Spring::new(0, 0.037, 0.3);
        spring.target.z = 1.0;
        spring.step();
        assert!((spring.angles.z - 0.037).abs() < 1e-6);
    }

    #[test]
    fn a_raised_arm_turns_the_clavicle_up_and_past_35_degrees_the_waist_back() {
        let raised = |degrees: f32, hold| {
            hero_with(
                held(0x2F, hold),
                GunArm {
                    raised: true,
                    pitch: degrees.to_radians(),
                },
            )
        };
        let mut springs = JointSprings::default();
        settle(&mut springs, &raised(30.0, Hold::OneHand));
        let up = springs.springs[CLAVICLE_R].angles.y.to_degrees();
        assert!((up + 30.0).abs() < 0.1, "{up}");
        assert_eq!(springs.springs[CLAVICLE_L].angles, Vec3::ZERO);
        assert!(springs.springs[WAIST].angles.y.abs() < 1e-3);

        let mut springs = JointSprings::default();
        settle(&mut springs, &raised(60.0, Hold::TwoHands));
        for side in [CLAVICLE_R, CLAVICLE_L] {
            let up = springs.springs[side].angles.y.to_degrees();
            assert!((up + 35.0).abs() < 0.1, "{up}");
        }
        let back = springs.springs[WAIST].angles.y.to_degrees();
        assert!((back + 25.0).abs() < 0.1, "{back}");

        // The turn lifts the barrel: a gun ahead along the rig's −z ends up pointing up.
        let turn = springs.turn(joint::CLAVICLE_R).unwrap();
        assert!((turn * Vec3::NEG_Z).y > 0.5);
        // Lowered again, the springs go home.
        settle(
            &mut springs,
            &hero_with(held(0x2F, Hold::TwoHands), GunArm::default()),
        );
        assert!(
            springs
                .turn(joint::CLAVICLE_R)
                .is_none_or(|t| t.to_axis_angle().1 < 1e-3)
        );
    }

    #[test]
    fn the_upper_body_turns_to_the_guns_target_within_its_limits() {
        let mut gun = held(0x2F, Hold::OneHand);
        // Ahead is −z; a target off to his left and below the chest.
        gun.target = Some(Vec3::new(-5.0, 0.0, -5.0));
        let hero = hero_with(gun, GunArm::default());
        let mut springs = JointSprings::default();
        settle(&mut springs, &hero);
        let yaw = springs.springs[CHEST].angles.z.to_degrees();
        let pitch = springs.springs[WAIST].angles.y.to_degrees();
        assert!(yaw > 40.0 && yaw < 50.0, "turns left: {yaw}");
        assert!(pitch > 0.0 && pitch < 10.0, "looks down a little: {pitch}");
        // The chest's turn takes his front towards the target.
        let front = springs.turn(joint::CHEST_UPPER).unwrap() * Vec3::NEG_Z;
        assert!(front.x < -0.6);

        // Behind him, the yaw stops at 59°; far above, the pitch at 25° up.
        let mut gun = held(0x2F, Hold::OneHand);
        gun.target = Some(Vec3::new(-1.0, 40.0, 5.0));
        let mut springs = JointSprings::default();
        settle(&mut springs, &hero_with(gun, GunArm::default()));
        assert!((springs.springs[CHEST].angles.z.to_degrees() - 59.0).abs() < 0.1);
        assert!((springs.springs[WAIST].angles.y.to_degrees() + 25.0).abs() < 0.1);

        // Not while attacking.
        let mut attacking = hero_with(gun, GunArm::default());
        attacking.state = HeroState::Hit;
        let mut springs = JointSprings::default();
        settle(&mut springs, &attacking);
        assert!(springs.turn(joint::CHEST_UPPER).is_none());
    }
}
