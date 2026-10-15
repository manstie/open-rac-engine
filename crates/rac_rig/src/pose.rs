//! The rig's animation, made procedurally from the hero's state each frame (P1.17's
//! "animations authored in code").
//!
//! Each state has a target pose: a rotation for each joint relative to its parent, and an
//! offset for the hips. The rig eases towards it, so states blend into each other. While
//! the wrench swings, the right arm reaches for the controller's own hand position for the
//! swing (`HeroMotion::wrench_line`) with two-bone IK, and the wrench points at its tip, so
//! what the hero hits matches what is drawn.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::{math::ops, prelude::*};
use rac_hero::{HeroMotion, HeroState, JumpKind};

use crate::model::{JointDef, joint, wrench_points};

/// Metres the hero covers in one stride (two steps) at a run.
const STRIDE: f32 = 1.5;
/// Run speed (u/s) at which the stride is full; UYA's walk tops out at 10.
const FULL_RUN: f32 = 9.0;

/// A pose: each joint's rotation relative to its parent, and how far it is moved from
/// where it sits at rest (in its parent's space).
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    pub rotations: Vec<Quat>,
    pub offsets: Vec<Vec3>,
}

impl Pose {
    pub fn rest() -> Self {
        Self {
            rotations: vec![Quat::IDENTITY; joint::COUNT],
            offsets: vec![Vec3::ZERO; joint::COUNT],
        }
    }

    /// Moves `rate` (0 to 1) of the way to `target`.
    pub fn ease(&mut self, target: &Pose, rate: f32) {
        for (rotation, goal) in self.rotations.iter_mut().zip(&target.rotations) {
            *rotation = rotation.slerp(*goal, rate);
        }
        for (offset, goal) in self.offsets.iter_mut().zip(&target.offsets) {
            *offset = offset.lerp(*goal, rate);
        }
    }
}

/// What the animation keeps between frames.
#[derive(Debug, Clone, PartialEq)]
pub struct Animator {
    pub pose: Pose,
    /// The stride's phase, in radians.
    pub phase: f32,
    /// Seconds since the rig appeared, for idle motion.
    pub time: f32,
    /// The move whose clip plays, and how far into it (ticks).
    pub clip_move: Option<crate::RigMove>,
    pub clip_ticks: f32,
}

impl Default for Animator {
    fn default() -> Self {
        Self {
            pose: Pose::rest(),
            phase: 0.0,
            time: 0.0,
            clip_move: None,
            clip_ticks: 0.0,
        }
    }
}

fn rx(angle: f32) -> Quat {
    Quat::from_rotation_x(angle)
}
fn ry(angle: f32) -> Quat {
    Quat::from_rotation_y(angle)
}
fn rz(angle: f32) -> Quat {
    Quat::from_rotation_z(angle)
}

/// Builds a pose from rotations relative to parents, with some joints given a rotation in
/// the hero's space instead.
struct Builder<'a> {
    joints: &'a [JointDef],
    pose: Pose,
    world: Vec<Option<Quat>>,
}

impl<'a> Builder<'a> {
    fn new(joints: &'a [JointDef]) -> Self {
        Self {
            joints,
            pose: Pose::rest(),
            world: vec![None; joints.len()],
        }
    }

    fn set(&mut self, joint: usize, rotation: Quat) -> &mut Self {
        self.pose.rotations[joint] = rotation;
        self
    }

    /// Mirrored for the hero's other side: `right` is the right side's rotation.
    fn sides(&mut self, right: usize, left: usize, rotation: Quat) -> &mut Self {
        let (axis, angle) = rotation.to_axis_angle();
        self.set(right, rotation);
        self.set(
            left,
            Quat::from_axis_angle(Vec3::new(axis.x, -axis.y, -axis.z), angle),
        )
    }

    fn world(&mut self, joint: usize, rotation: Quat) -> &mut Self {
        self.world[joint] = Some(rotation);
        self
    }

    /// Turns the joints given in the hero's space into rotations relative to their
    /// parents.
    fn finish(mut self) -> Pose {
        let mut world = vec![Quat::IDENTITY; self.joints.len()];
        for (index, joint) in self.joints.iter().enumerate() {
            let parent = joint.parent.map_or(Quat::IDENTITY, |p| world[p]);
            if let Some(target) = self.world[index] {
                self.pose.rotations[index] = parent.inverse() * target;
            }
            world[index] = parent * self.pose.rotations[index];
        }
        self.pose
    }
}

/// Arms hanging at the sides, elbows a little bent.
fn arms_down(b: &mut Builder, swing: f32, spread: f32) {
    b.sides(
        joint::SHOULDER_R,
        joint::SHOULDER_L,
        rx(-swing) * rz(-1.25 + spread),
    );
    b.sides(
        joint::ELBOW_R,
        joint::ELBOW_L,
        ry(0.35 + swing.max(0.0) * 0.8),
    );
}

/// The wrench held down along the right leg, head back.
fn wrench_at_rest(b: &mut Builder) {
    let down_back = Vec3::new(0.15, -0.9, 0.35).normalize();
    b.world(
        joint::HAND_R,
        Quat::from_rotation_arc(wrench_points(), down_back),
    );
}

fn tail(b: &mut Builder, sway: f32, lift: f32, time: f32) {
    for (i, &joint) in joint::TAIL.iter().enumerate() {
        let k = i as f32;
        let wave = (time * 2.2 - k * 0.6).sin() * sway;
        b.set(joint, ry(wave) * rx(-lift / 7.0 - k * 0.02));
    }
}

fn clank(b: &mut Builder, speed: f32, time: f32) {
    let bob = (time * 3.0).sin();
    b.set(joint::CLANK_HEAD, rx(0.05 * bob));
    b.set(
        joint::CLANK_ANTENNA,
        rz((time * 7.0).sin() * 0.12 * (0.3 + speed)),
    );
    let dangle = 0.25 * speed + 0.05 * bob;
    b.set(joint::CLANK_LEG_L, rx(dangle));
    b.set(joint::CLANK_LEG_R, rx(dangle * 0.8));
    b.set(joint::CLANK_ARM_L, rx(-0.1 * bob));
    b.set(joint::CLANK_ARM_R, rx(0.1 * bob));
}

/// Legs bent to sit the hips `drop` metres lower with the feet where they were.
fn legs_bent(b: &mut Builder, drop: f32) {
    let thigh: f32 = 0.19;
    let shin: f32 = 0.19;
    let reach = (thigh + shin - drop.max(0.0)).clamp(0.1, thigh + shin - 1e-3);
    let knee = PI - ((thigh * thigh + shin * shin - reach * reach) / (2.0 * thigh * shin)).acos();
    let hip = ((thigh * thigh + reach * reach - shin * shin) / (2.0 * thigh * reach)).acos();
    b.sides(joint::THIGH_R, joint::THIGH_L, rx(hip));
    b.sides(joint::KNEE_R, joint::KNEE_L, rx(-knee));
    b.sides(joint::ANKLE_R, joint::ANKLE_L, rx(knee - hip));
}

/// Two-bone IK for the right arm: the hand at `hand`, bending the elbow down and back.
/// Positions are in the hero's space; the chest is taken to be upright.
fn reach_right(b: &mut Builder, joints: &[JointDef], hand: Vec3, tip: Vec3) {
    let shoulder = joints[joint::SHOULDER_R].rest;
    let elbow_rest = joints[joint::ELBOW_R].rest;
    let hand_rest = joints[joint::HAND_R].rest;
    let upper = elbow_rest.distance(shoulder);
    let lower = hand_rest.distance(elbow_rest);
    let to_hand = hand - shoulder;
    let along = to_hand.normalize_or(Vec3::X);
    let reach = to_hand
        .length()
        .clamp((upper - lower).abs() + 1e-3, upper + lower - 1e-3);
    let pole = Vec3::new(0.3, -1.0, 0.6);
    let bend = (pole - along * pole.dot(along)).normalize_or(Vec3::NEG_Y);
    let a = (upper * upper - lower * lower + reach * reach) / (2.0 * reach);
    let h = (upper * upper - a * a).max(0.0).sqrt();
    let elbow = shoulder + along * a + bend * h;
    let wrist = shoulder + along * reach;
    let upper_rest = (elbow_rest - shoulder).normalize();
    let lower_rest = (hand_rest - elbow_rest).normalize();
    b.world(
        joint::SHOULDER_R,
        Quat::from_rotation_arc(upper_rest, (elbow - shoulder).normalize()),
    );
    b.world(
        joint::ELBOW_R,
        Quat::from_rotation_arc(lower_rest, (wrist - elbow).normalize()),
    );
    let wrench = (tip - hand).normalize_or(Vec3::NEG_Z);
    b.world(
        joint::HAND_R,
        Quat::from_rotation_arc(wrench_points(), wrench),
    );
}

/// The pose for the hero's state now. `phase` is the stride's phase and `time` the rig's
/// clock, both in radians or seconds as named.
pub fn target(joints: &[JointDef], hero: &HeroMotion, phase: f32, time: f32) -> Pose {
    let mut b = Builder::new(joints);
    let horizontal = Vec2::new(hero.velocity.x, hero.velocity.z).length();
    let run = (horizontal / FULL_RUN).min(1.0);
    let state_time = hero.ticks as f32 / 60.0;
    let breathe = (time * 2.0).sin();

    // What every state starts from: standing, arms down, wrench held low.
    arms_down(&mut b, 0.0, 0.0);
    wrench_at_rest(&mut b);
    b.set(joint::CHEST, rx(0.02 * breathe));
    b.set(joint::HEAD, rx(-0.02 * breathe));
    b.sides(joint::EAR_R, joint::EAR_L, rz(0.05 * (time * 0.7).sin()));
    tail(&mut b, 0.25, 0.0, time);
    clank(&mut b, run, time);

    match hero.state {
        HeroState::Idle | HeroState::SurfaceIdle | HeroState::UnderwaterIdle => {}
        HeroState::Walk | HeroState::Charge => {
            let charge = matches!(hero.state, HeroState::Charge);
            let stride = phase.sin();
            let legs = 0.75 * run;
            b.set(joint::THIGH_R, rx(legs * stride + 0.1 * run));
            b.set(joint::THIGH_L, rx(-legs * stride + 0.1 * run));
            let knee = |offset: f32| -1.1 * run * (0.5 - 0.5 * (phase + offset).cos()).powf(1.5);
            b.set(joint::KNEE_R, rx(knee(0.0)));
            b.set(joint::KNEE_L, rx(knee(PI)));
            b.set(joint::SHOULDER_R, rx(legs * 0.8 * stride) * rz(-1.25));
            b.set(joint::SHOULDER_L, rx(-legs * 0.8 * stride) * rz(1.25));
            let lean = if charge { 0.6 } else { 0.18 * run };
            b.set(joint::SPINE, rx(-lean) * ry(0.08 * run * stride));
            b.set(joint::CHEST, rx(-0.05 * run) * ry(-0.15 * run * stride));
            b.set(joint::HEAD, rx(lean * 0.6));
            b.pose.offsets[joint::HIPS] = Vec3::Y * (-0.03 * run * (2.0 * phase).cos().abs());
            b.sides(joint::EAR_R, joint::EAR_L, rz(-0.25 * run) * rx(0.2 * run));
            tail(
                &mut b,
                0.35 * run + 0.1,
                0.6 * run,
                time * (1.0 + 2.0 * run),
            );
        }
        HeroState::Skid | HeroState::Bounce => {
            b.set(joint::SPINE, rx(0.3));
            legs_bent(&mut b, 0.08);
            b.pose.offsets[joint::HIPS] = Vec3::new(0.0, -0.08, 0.0);
            arms_down(&mut b, -0.4, 0.5);
        }
        HeroState::Crouch => {
            legs_bent(&mut b, 0.18);
            b.pose.offsets[joint::HIPS] = Vec3::new(0.0, -0.18, 0.0);
            b.set(joint::SPINE, rx(-0.35));
            b.set(joint::HEAD, rx(0.3));
            tail(&mut b, 0.1, -0.5, time);
        }
        HeroState::Fall => {
            arms_down(&mut b, 0.2, 0.9);
            b.sides(joint::THIGH_R, joint::THIGH_L, rx(0.3));
            b.sides(joint::KNEE_R, joint::KNEE_L, rx(-0.5));
            tail(&mut b, 0.4, 1.2, time * 2.0);
            b.sides(joint::EAR_R, joint::EAR_L, rz(0.3));
        }
        HeroState::Jump(kind) => {
            let tuck = (state_time * 3.0).min(1.0);
            b.sides(joint::THIGH_R, joint::THIGH_L, rx(0.9 * tuck));
            b.sides(joint::KNEE_R, joint::KNEE_L, rx(-1.4 * tuck));
            arms_down(&mut b, 0.3, 0.7);
            tail(&mut b, 0.3, 1.0, time * 2.0);
            match kind {
                JumpKind::Flip => {
                    // A back flip over the jump's first second.
                    let turn = (state_time / 0.9).min(1.0);
                    b.set(joint::HIPS, rx(TAU * turn));
                }
                JumpKind::Long => {
                    b.set(joint::HIPS, rx(-0.7));
                    b.sides(joint::THIGH_R, joint::THIGH_L, rx(-0.2));
                    arms_down(&mut b, -0.6, 0.3);
                }
                JumpKind::Heli => arms_down(&mut b, 0.0, 2.6),
                _ => {}
            }
        }
        HeroState::DoubleJump => {
            let spin = (state_time / 0.45).min(1.0);
            b.set(joint::HIPS, rx(-TAU * spin));
            b.sides(joint::THIGH_R, joint::THIGH_L, rx(1.3));
            b.sides(joint::KNEE_R, joint::KNEE_L, rx(-2.0));
            arms_down(&mut b, 0.6, 0.4);
        }
        HeroState::Glide => {
            arms_down(&mut b, 0.0, 2.7);
            b.set(joint::HIPS, rx(-0.3));
            b.sides(joint::THIGH_R, joint::THIGH_L, rx(-0.2));
            tail(&mut b, 0.2, 0.8, time);
        }
        HeroState::LedgeGrab | HeroState::LedgeHang | HeroState::LedgeShimmy(_) => {
            arms_down(&mut b, 0.4, 2.8);
            b.sides(joint::ELBOW_R, joint::ELBOW_L, ry(0.2));
            b.sides(joint::THIGH_R, joint::THIGH_L, rx(0.1 * (time * 1.5).sin()));
            tail(&mut b, 0.3, -1.0, time);
        }
        HeroState::Grind => {
            legs_bent(&mut b, 0.12);
            b.pose.offsets[joint::HIPS] = Vec3::new(0.0, -0.12, 0.0);
            b.set(joint::HIPS, ry(0.5) * rx(-0.2));
            arms_down(&mut b, 0.0, 0.9);
            tail(&mut b, 0.4, 0.8, time * 2.0);
        }
        HeroState::ZipLine => {
            // Hanging by both hands from the cable above, legs trailing.
            arms_down(&mut b, 0.0, 3.0);
            b.sides(joint::ELBOW_R, joint::ELBOW_L, ry(0.1));
            b.sides(
                joint::THIGH_R,
                joint::THIGH_L,
                rx(0.3 + 0.1 * (time * 2.0).sin()),
            );
            b.sides(joint::KNEE_R, joint::KNEE_L, rx(-0.4));
            tail(&mut b, 0.4, -0.6, time * 2.0);
        }
        HeroState::SurfaceSwim | HeroState::UnderwaterSwim | HeroState::UnderwaterFast => {
            let stroke = phase.sin();
            b.set(joint::HIPS, rx(-1.3));
            b.set(joint::HEAD, rx(0.9));
            b.set(joint::SHOULDER_R, rx(1.2 + stroke) * rz(-0.4));
            b.set(joint::SHOULDER_L, rx(1.2 - stroke) * rz(0.4));
            b.set(joint::THIGH_R, rx(0.3 * stroke));
            b.set(joint::THIGH_L, rx(-0.3 * stroke));
            tail(&mut b, 0.4, 0.0, time * 2.0);
        }
        HeroState::Combo(_) | HeroState::JumpAttack => {
            let swing = match hero.state {
                HeroState::Combo(1) => -1.0,
                HeroState::Combo(_) => 1.0,
                _ => 0.0,
            };
            b.set(joint::SPINE, ry(0.25 * swing) * rx(-0.15));
            legs_bent(&mut b, 0.05);
            b.pose.offsets[joint::HIPS] = Vec3::new(0.0, -0.05, 0.0);
            if let Some((hand, tip)) = hero.wrench_line() {
                let to_local = Quat::from_rotation_y(hero.yaw).inverse();
                let hand = to_local * (hand - hero.position);
                let tip = to_local * (tip - hero.position);
                reach_right(&mut b, joints, hand, tip);
            } else {
                // Wind-up and follow-through: the wrench held out to the side.
                b.set(joint::SHOULDER_R, ry(0.6 * swing) * rz(-0.4));
                b.world(
                    joint::HAND_R,
                    Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(0.7, 0.3, -0.6).normalize()),
                );
            }
        }
        HeroState::Throw => {
            let forward = Vec3::new(0.1, 0.05, -1.0).normalize();
            let rest = (joints[joint::ELBOW_R].rest - joints[joint::SHOULDER_R].rest).normalize();
            b.world(joint::SHOULDER_R, Quat::from_rotation_arc(rest, forward));
            b.set(joint::ELBOW_R, Quat::IDENTITY);
            b.set(joint::SPINE, ry(-0.3));
        }
        HeroState::Hit => {
            b.set(joint::SPINE, rx(0.5));
            arms_down(&mut b, -0.5, 1.2);
            b.set(joint::HEAD, rx(0.3));
            tail(&mut b, 0.5, 0.5, time * 3.0);
        }
        HeroState::Dead => {
            let fall = (state_time * 2.0).min(1.0);
            b.set(joint::HIPS, rx(FRAC_PI_2 * fall));
            b.pose.offsets[joint::HIPS] = Vec3::new(0.0, -0.3 * fall, 0.0);
            arms_down(&mut b, -0.3, 1.5);
        }
    }
    b.finish()
}

/// Strafing sideways UYA runs with the hips turned to the way he moves and the body turned
/// back to the camera (0x4EF060). Not backwards, which has its own walk.
pub fn twist_for_strafe(pose: &mut Pose, hero: &HeroMotion) {
    let moving = Vec2::new(hero.velocity.x, hero.velocity.z);
    if !hero.strafing
        || hero.strafing_back
        || hero.state != HeroState::Walk
        || moving.length() < 0.1
    {
        return;
    }
    // The movement's yaw against his, as the controller measures yaw.
    let twist = wrap_angle(ops::atan2(-moving.x, -moving.y) - hero.yaw);
    pose.rotations[joint::HIPS] = ry(twist) * pose.rotations[joint::HIPS];
    pose.rotations[joint::CHEST] = ry(-twist) * pose.rotations[joint::CHEST];
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Advances the stride's phase by the distance the hero ran this frame.
pub fn advance_phase(phase: f32, hero: &HeroMotion, seconds: f32) -> f32 {
    let horizontal = Vec2::new(hero.velocity.x, hero.velocity.z).length();
    let swimming = matches!(
        hero.state,
        HeroState::SurfaceSwim | HeroState::UnderwaterSwim | HeroState::UnderwaterFast
    );
    let distance = if swimming {
        // Strokes keep going at a steady rate.
        seconds * 1.6
    } else if hero.strafing_back {
        // Walking backwards runs the stride the other way.
        -horizontal * seconds
    } else {
        horizontal * seconds
    };
    (phase + distance / STRIDE * TAU).rem_euclid(TAU)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::hero;
    use rac_hero::HeroState;

    fn world_rotation(joints: &[JointDef], pose: &Pose, index: usize) -> Quat {
        let mut chain = vec![index];
        while let Some(parent) = joints[*chain.last().unwrap()].parent {
            chain.push(parent);
        }
        chain
            .iter()
            .rev()
            .fold(Quat::IDENTITY, |acc, &j| acc * pose.rotations[j])
    }

    #[test]
    fn standing_still_the_arms_hang_and_the_wrench_points_down() {
        let model = hero();
        let motion = HeroMotion::new(Vec3::ZERO, 0.0);
        let pose = target(&model.joints, &motion, 0.0, 0.0);
        let arm = world_rotation(&model.joints, &pose, joint::SHOULDER_R) * Vec3::X;
        assert!(arm.y < -0.8, "{arm}");
        let wrench = world_rotation(&model.joints, &pose, joint::HAND_R) * wrench_points();
        assert!(wrench.y < -0.8, "{wrench}");
    }

    #[test]
    fn the_sides_mirror() {
        let model = hero();
        let mut motion = HeroMotion::new(Vec3::ZERO, 0.0);
        motion.state = HeroState::Crouch;
        let pose = target(&model.joints, &motion, 0.0, 0.0);
        let right = world_rotation(&model.joints, &pose, joint::KNEE_R) * Vec3::NEG_Y;
        let left = world_rotation(&model.joints, &pose, joint::KNEE_L) * Vec3::NEG_Y;
        assert!((right - left).length() < 1e-4, "{right} {left}");
        assert!(right.z > 0.3, "shins slope back when crouching: {right}");
    }

    #[test]
    fn the_ik_puts_the_hand_where_asked() {
        let model = hero();
        let joints = &model.joints;
        let mut b = Builder::new(joints);
        let hand = Vec3::new(0.3, 0.85, -0.25);
        reach_right(&mut b, joints, hand, hand + Vec3::new(0.0, 0.0, -0.8));
        let pose = b.finish();
        let shoulder = joints[joint::SHOULDER_R].rest;
        let upper = world_rotation(joints, &pose, joint::SHOULDER_R)
            * (joints[joint::ELBOW_R].rest - shoulder);
        let lower = world_rotation(joints, &pose, joint::ELBOW_R)
            * (joints[joint::HAND_R].rest - joints[joint::ELBOW_R].rest);
        assert!((shoulder + upper + lower).distance(hand) < 1e-3);
        let wrench = world_rotation(joints, &pose, joint::HAND_R) * wrench_points();
        assert!(wrench.distance(Vec3::NEG_Z) < 1e-3);
    }

    #[test]
    fn the_stride_advances_with_distance() {
        let mut motion = HeroMotion::new(Vec3::ZERO, 0.0);
        motion.velocity = Vec3::new(STRIDE, 0.0, 0.0);
        let phase = advance_phase(0.0, &motion, 0.25);
        assert!((phase - TAU / 4.0).abs() < 1e-4);
        // Walking backwards runs it the other way.
        motion.strafing_back = true;
        let phase = advance_phase(0.0, &motion, 0.25);
        assert!((phase - 3.0 * TAU / 4.0).abs() < 1e-4);
    }

    #[test]
    fn strafing_sideways_turns_the_hips_to_the_movement_and_the_chest_back() {
        let model = hero();
        let mut motion = HeroMotion::new(Vec3::ZERO, 0.0);
        motion.state = HeroState::Walk;
        motion.strafing = true;
        // Facing −z, moving +x: the legs turn a right angle to the right.
        motion.velocity = Vec3::new(6.0, 0.0, 0.0);
        let mut pose = target(&model.joints, &motion, 0.0, 0.0);
        twist_for_strafe(&mut pose, &motion);
        let hips = world_rotation(&model.joints, &pose, joint::HIPS);
        assert!(
            (hips * Vec3::NEG_Z - Vec3::X).length() < 1e-3,
            "{}",
            hips * Vec3::NEG_Z
        );
        // The chest still faces the camera's way, give or take the run's own lean.
        let chest = world_rotation(&model.joints, &pose, joint::CHEST) * Vec3::NEG_Z;
        assert!(chest.dot(Vec3::NEG_Z) > 0.95, "{chest}");
    }
}
