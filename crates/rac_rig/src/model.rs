//! The placeholder hero: a blocky Ratchet with Clank on his back and the wrench in his
//! right hand, made of chamfered blocks.
//!
//! It is modelled by hand. UYA's hero was the reference for proportions only: the joints
//! sit where UYA's do, rounded to the centimetre, and the blocks were sized against the
//! hero's mesh measured from the user's own disc (`docs/research/moby-format.md`). No
//! geometry comes from the game. The joints follow UYA's layout (each names the UYA joint
//! it stands for), so animation read from a disc can drive the rig later.

use bevy::math::{Vec2, Vec3};

use crate::shapes::{at, beam, block_at};

/// The rig's joints, parents before children.
pub mod joint {
    pub const ROOT: usize = 0;
    pub const HIPS: usize = 1;
    pub const SPINE: usize = 2;
    pub const CHEST: usize = 3;
    pub const NECK: usize = 4;
    pub const HEAD: usize = 5;
    pub const JAW: usize = 6;
    pub const EAR_L: usize = 7;
    pub const EAR_L_TIP: usize = 8;
    pub const EAR_R: usize = 9;
    pub const EAR_R_TIP: usize = 10;
    pub const SHOULDER_R: usize = 11;
    pub const ELBOW_R: usize = 12;
    pub const HAND_R: usize = 13;
    pub const FINGERS_R: usize = 14;
    pub const THUMB_R: usize = 15;
    pub const SHOULDER_L: usize = 16;
    pub const ELBOW_L: usize = 17;
    pub const HAND_L: usize = 18;
    pub const FINGERS_L: usize = 19;
    pub const THUMB_L: usize = 20;
    pub const THIGH_R: usize = 21;
    pub const KNEE_R: usize = 22;
    pub const ANKLE_R: usize = 23;
    pub const TOE_R: usize = 24;
    pub const THIGH_L: usize = 25;
    pub const KNEE_L: usize = 26;
    pub const ANKLE_L: usize = 27;
    pub const TOE_L: usize = 28;
    /// The tail's seven joints, from the hips out.
    pub const TAIL: [usize; 7] = [29, 30, 31, 32, 33, 34, 35];
    pub const CLANK: usize = 36;
    pub const CLANK_HEAD: usize = 37;
    pub const CLANK_ANTENNA: usize = 38;
    pub const CLANK_ARM_L: usize = 39;
    pub const CLANK_ARM_R: usize = 40;
    pub const CLANK_LEG_L: usize = 41;
    pub const CLANK_LEG_R: usize = 42;
    pub const WRENCH: usize = 43;
    pub const COUNT: usize = 44;
}

/// One joint of the rig. At rest every joint is unrotated: the hero stands in a T-pose,
/// facing forward, as UYA's skeleton does.
#[derive(Debug, Clone, PartialEq)]
pub struct JointDef {
    pub name: &'static str,
    pub parent: Option<usize>,
    /// In the hero's local space, metres from his feet.
    pub rest: Vec3,
    /// The UYA hero joint it stands for, if any.
    pub uya: Option<u8>,
}

/// Which material a part is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Look {
    /// The body: gold.
    Main,
    /// Eyes, nose, Clank's panel and the tail's tip: a darker gold.
    Accent,
}

/// A block of the model: the convex hull of `points`, moving with `joint`.
#[derive(Debug, Clone, PartialEq)]
pub struct PartDef {
    pub joint: usize,
    pub look: Look,
    /// In the hero's local space at rest.
    pub points: Vec<Vec3>,
}

/// The whole model.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDef {
    pub joints: Vec<JointDef>,
    pub parts: Vec<PartDef>,
}

impl ModelDef {
    /// The part's points relative to its joint at rest.
    pub fn local_points(&self, part: &PartDef) -> Vec<Vec3> {
        let rest = self.joints[part.joint].rest;
        part.points.iter().map(|p| *p - rest).collect()
    }
}

fn j(name: &'static str, parent: usize, rest: Vec3, uya: u8) -> JointDef {
    JointDef {
        name,
        parent: Some(parent),
        rest,
        uya: Some(uya),
    }
}

fn own(name: &'static str, parent: usize, rest: Vec3) -> JointDef {
    JointDef {
        name,
        parent: Some(parent),
        rest,
        uya: None,
    }
}

/// `side` is +1 for the hero's left and -1 for his right.
fn sided(name: [&'static str; 2], side: f32) -> &'static str {
    if side > 0.0 { name[0] } else { name[1] }
}

fn joints() -> Vec<JointDef> {
    use joint::*;
    let mut joints = vec![
        JointDef {
            name: "root",
            parent: None,
            rest: Vec3::ZERO,
            uya: Some(0),
        },
        j("hips", ROOT, at(-0.04, 0.0, 0.49), 1),
        j("spine", HIPS, at(-0.03, 0.0, 0.59), 3),
        j("chest", SPINE, at(-0.05, 0.0, 0.80), 5),
        j("neck", CHEST, at(-0.02, 0.0, 0.95), 7),
        j("head", NECK, at(-0.01, 0.0, 1.08), 8),
        j("jaw", HEAD, at(0.0, 0.0, 1.04), 10),
        j("ear_l", HEAD, at(-0.10, 0.11, 1.19), 41),
        j("ear_l_tip", EAR_L, at(-0.21, 0.25, 1.27), 42),
        j("ear_r", HEAD, at(-0.10, -0.12, 1.19), 44),
        j("ear_r_tip", EAR_R, at(-0.21, -0.26, 1.27), 45),
    ];
    for (side, uya) in [(-1.0, 53), (1.0, 73)] {
        let base = joints.len();
        joints.extend([
            j(
                sided(["shoulder_l", "shoulder_r"], side),
                CHEST,
                at(-0.02, 0.15 * side, 0.92),
                uya,
            ),
            j(
                sided(["elbow_l", "elbow_r"], side),
                base,
                at(-0.03, 0.35 * side, 0.88),
                uya + 1,
            ),
            j(
                sided(["hand_l", "hand_r"], side),
                base + 1,
                at(-0.02, 0.52 * side, 0.88),
                uya + 3,
            ),
            j(
                sided(["fingers_l", "fingers_r"], side),
                base + 2,
                at(0.01, 0.69 * side, 0.92),
                uya + 7,
            ),
            j(
                sided(["thumb_l", "thumb_r"], side),
                base + 2,
                at(0.09, 0.59 * side, 0.90),
                uya + 16,
            ),
        ]);
    }
    for (side, uya) in [(-1.0, 92), (1.0, 98)] {
        let base = joints.len();
        joints.extend([
            j(
                sided(["thigh_l", "thigh_r"], side),
                HIPS,
                at(-0.04, 0.065 * side, 0.46),
                uya,
            ),
            j(
                sided(["knee_l", "knee_r"], side),
                base,
                at(0.01, 0.065 * side, 0.27),
                uya + 1,
            ),
            j(
                sided(["ankle_l", "ankle_r"], side),
                base + 1,
                at(-0.04, 0.065 * side, 0.08),
                uya + 2,
            ),
            j(
                sided(["toe_l", "toe_r"], side),
                base + 2,
                at(0.14, 0.065 * side, 0.0),
                uya + 4,
            ),
        ]);
    }
    let tail = [-0.13, -0.21, -0.32, -0.43, -0.52, -0.59, -0.69];
    for (i, forward) in tail.into_iter().enumerate() {
        let parent = if i == 0 { HIPS } else { TAIL[i - 1] };
        let names = [
            "tail_0", "tail_1", "tail_2", "tail_3", "tail_4", "tail_5", "tail_6",
        ];
        joints.push(j(names[i], parent, at(forward, 0.0, 0.46), 104 + i as u8));
    }
    // Clank rides on UYA's joint 5, facing backwards; his joints are our own.
    joints.extend([
        own("clank", CHEST, at(-0.05, 0.0, 0.80)),
        own("clank_head", CLANK, at(-0.29, 0.0, 1.0)),
        own("clank_antenna", CLANK_HEAD, at(-0.30, 0.0, 1.17)),
        own("clank_arm_l", CLANK, at(-0.26, 0.115, 0.95)),
        own("clank_arm_r", CLANK, at(-0.26, -0.115, 0.95)),
        own("clank_leg_l", CLANK, at(-0.19, 0.06, 0.68)),
        own("clank_leg_r", CLANK, at(-0.19, -0.06, 0.68)),
        // The wrench sits on the hand joint, pointing forward at rest, as UYA's does.
        own("wrench", HAND_R, at(-0.02, -0.52, 0.88)),
    ]);
    debug_assert_eq!(joints.len(), COUNT);
    joints
}

fn part(joint: usize, look: Look, points: Vec<Vec3>) -> PartDef {
    PartDef {
        joint,
        look,
        points,
    }
}

/// Up and forward in the hero's space, as hints for which way a beam's width runs.
const UP: Vec3 = Vec3::Y;
const FORWARD: Vec3 = Vec3::NEG_Z;
const LEFT: Vec3 = Vec3::NEG_X;

fn ratchet_parts() -> Vec<PartDef> {
    use Look::*;
    use joint::*;
    let v = Vec2::new;
    let mut parts = vec![
        // Body.
        part(
            HIPS,
            Main,
            block_at(at(-0.01, 0.0, 0.45), 0.10, 0.12, 0.07, 0.025),
        ),
        part(
            SPINE,
            Main,
            beam(
                at(-0.01, 0.0, 0.51),
                at(-0.02, 0.0, 0.73),
                LEFT,
                v(0.105, 0.095),
                v(0.12, 0.095),
                0.03,
            ),
        ),
        part(
            CHEST,
            Main,
            beam(
                at(-0.03, 0.0, 0.72),
                at(-0.04, 0.0, 0.95),
                LEFT,
                v(0.125, 0.10),
                v(0.15, 0.09),
                0.035,
            ),
        ),
        part(
            NECK,
            Main,
            beam(
                at(-0.02, 0.0, 0.92),
                at(-0.01, 0.0, 1.02),
                LEFT,
                v(0.05, 0.05),
                v(0.05, 0.05),
                0.015,
            ),
        ),
        // Head: a rounded cranium over wide cheeks, a muzzle, nose and eyes.
        part(
            HEAD,
            Main,
            block_at(at(-0.01, 0.0, 1.16), 0.15, 0.17, 0.14, 0.06),
        ),
        part(
            HEAD,
            Main,
            block_at(at(0.02, 0.0, 1.03), 0.12, 0.19, 0.06, 0.04),
        ),
        part(
            HEAD,
            Main,
            beam(
                at(0.08, 0.0, 1.06),
                at(0.22, 0.0, 1.05),
                LEFT,
                v(0.09, 0.06),
                v(0.06, 0.04),
                0.02,
            ),
        ),
        part(
            HEAD,
            Accent,
            block_at(at(0.225, 0.0, 1.075), 0.02, 0.035, 0.022, 0.008),
        ),
        part(
            JAW,
            Main,
            block_at(at(0.11, 0.0, 0.98), 0.07, 0.08, 0.025, 0.015),
        ),
        // Tail: thin at the hips, bushy at the end, with a dark tip.
    ];
    for side in [1.0, -1.0] {
        let (ear, tip) = if side > 0.0 {
            (EAR_L, EAR_L_TIP)
        } else {
            (EAR_R, EAR_R_TIP)
        };
        let width = at(0.3, 0.0, 1.0);
        parts.extend([
            part(
                HEAD,
                Accent,
                block_at(at(0.135, 0.075 * side, 1.15), 0.025, 0.045, 0.05, 0.012),
            ),
            part(
                ear,
                Main,
                beam(
                    at(-0.08, 0.10 * side, 1.17),
                    at(-0.21, 0.25 * side, 1.27),
                    width,
                    v(0.10, 0.025),
                    v(0.08, 0.02),
                    0.012,
                ),
            ),
            part(
                tip,
                Main,
                beam(
                    at(-0.21, 0.25 * side, 1.27),
                    at(-0.29, 0.41 * side, 1.41),
                    width,
                    v(0.08, 0.02),
                    v(0.005, 0.005),
                    0.0,
                ),
            ),
        ]);
    }
    for side in [1.0, -1.0] {
        let [shoulder, elbow, hand, fingers, thumb] = if side > 0.0 {
            [SHOULDER_L, ELBOW_L, HAND_L, FINGERS_L, THUMB_L]
        } else {
            [SHOULDER_R, ELBOW_R, HAND_R, FINGERS_R, THUMB_R]
        };
        let l = |forward: f32, left: f32, up: f32| at(forward, left * side, up);
        parts.extend([
            part(
                shoulder,
                Main,
                block_at(l(-0.02, 0.17, 0.95), 0.075, 0.06, 0.055, 0.025),
            ),
            part(
                shoulder,
                Main,
                beam(
                    l(-0.02, 0.15, 0.92),
                    l(-0.03, 0.35, 0.88),
                    FORWARD,
                    v(0.05, 0.045),
                    v(0.045, 0.042),
                    0.015,
                ),
            ),
            part(
                elbow,
                Main,
                beam(
                    l(-0.03, 0.35, 0.88),
                    l(-0.02, 0.46, 0.88),
                    FORWARD,
                    v(0.045, 0.042),
                    v(0.05, 0.046),
                    0.015,
                ),
            ),
            part(
                elbow,
                Main,
                beam(
                    l(-0.02, 0.44, 0.88),
                    l(-0.02, 0.52, 0.88),
                    FORWARD,
                    v(0.065, 0.06),
                    v(0.07, 0.064),
                    0.02,
                ),
            ),
            part(
                hand,
                Main,
                block_at(l(0.0, 0.58, 0.885), 0.075, 0.06, 0.04, 0.02),
            ),
            part(
                fingers,
                Main,
                beam(
                    l(0.0, 0.64, 0.895),
                    l(0.0, 0.80, 0.895),
                    FORWARD,
                    v(0.085, 0.032),
                    v(0.075, 0.028),
                    0.015,
                ),
            ),
            part(
                thumb,
                Main,
                beam(
                    l(0.07, 0.58, 0.895),
                    l(0.17, 0.61, 0.895),
                    UP,
                    v(0.025, 0.03),
                    v(0.02, 0.025),
                    0.01,
                ),
            ),
        ]);
    }
    for side in [1.0, -1.0] {
        let [thigh, knee, ankle, toe] = if side > 0.0 {
            [THIGH_L, KNEE_L, ANKLE_L, TOE_L]
        } else {
            [THIGH_R, KNEE_R, ANKLE_R, TOE_R]
        };
        let l = |forward: f32, left: f32, up: f32| at(forward, left * side, up);
        parts.extend([
            part(
                thigh,
                Main,
                beam(
                    l(-0.04, 0.07, 0.47),
                    l(0.0, 0.07, 0.27),
                    LEFT,
                    v(0.055, 0.06),
                    v(0.045, 0.05),
                    0.015,
                ),
            ),
            part(
                knee,
                Main,
                beam(
                    l(0.0, 0.07, 0.27),
                    l(-0.03, 0.07, 0.12),
                    LEFT,
                    v(0.045, 0.05),
                    v(0.05, 0.055),
                    0.015,
                ),
            ),
            part(
                knee,
                Main,
                beam(
                    l(-0.03, 0.07, 0.17),
                    l(-0.035, 0.07, 0.08),
                    LEFT,
                    v(0.065, 0.07),
                    v(0.07, 0.075),
                    0.02,
                ),
            ),
            part(
                ankle,
                Main,
                block_at(l(0.03, 0.075, 0.045), 0.12, 0.075, 0.045, 0.02),
            ),
            part(
                toe,
                Main,
                beam(
                    l(0.13, 0.075, 0.042),
                    l(0.29, 0.075, 0.035),
                    LEFT,
                    v(0.075, 0.042),
                    v(0.06, 0.03),
                    0.02,
                ),
            ),
        ]);
    }
    let radii = [0.03, 0.035, 0.04, 0.045, 0.05, 0.06, 0.065];
    let tail = [-0.13, -0.21, -0.32, -0.43, -0.52, -0.59, -0.69, -0.80];
    for (i, &joint) in joint::TAIL.iter().enumerate() {
        let end = if i + 1 < radii.len() {
            radii[i + 1]
        } else {
            0.01
        };
        parts.push(part(
            joint,
            if i + 1 < radii.len() { Main } else { Accent },
            beam(
                at(tail[i], 0.0, 0.46),
                at(tail[i + 1], 0.0, 0.465),
                LEFT,
                Vec2::splat(radii[i]),
                Vec2::splat(end),
                0.012,
            ),
        ));
    }
    parts
}

fn clank_parts() -> Vec<PartDef> {
    use Look::*;
    use joint::*;
    let v = Vec2::new;
    let mut parts = vec![
        // Body, facing backwards with the panel on his chest.
        part(
            CLANK,
            Main,
            beam(
                at(-0.265, 0.0, 0.68),
                at(-0.265, 0.0, 1.0),
                LEFT,
                v(0.10, 0.08),
                v(0.115, 0.085),
                0.025,
            ),
        ),
        part(
            CLANK,
            Accent,
            block_at(at(-0.352, 0.0, 0.85), 0.01, 0.07, 0.08, 0.005),
        ),
        part(
            CLANK_HEAD,
            Main,
            block_at(at(-0.305, 0.0, 1.085), 0.105, 0.14, 0.085, 0.035),
        ),
        part(
            CLANK_ANTENNA,
            Main,
            beam(
                at(-0.30, 0.0, 1.16),
                at(-0.31, 0.0, 1.23),
                LEFT,
                v(0.008, 0.008),
                v(0.008, 0.008),
                0.0,
            ),
        ),
        part(
            CLANK_ANTENNA,
            Accent,
            block_at(at(-0.31, 0.0, 1.245), 0.02, 0.02, 0.02, 0.006),
        ),
    ];
    for side in [1.0, -1.0] {
        let (arm, leg) = if side > 0.0 {
            (CLANK_ARM_L, CLANK_LEG_L)
        } else {
            (CLANK_ARM_R, CLANK_LEG_R)
        };
        let l = |forward: f32, left: f32, up: f32| at(forward, left * side, up);
        parts.extend([
            part(
                CLANK_HEAD,
                Accent,
                block_at(l(-0.41, 0.06, 1.09), 0.012, 0.035, 0.03, 0.006),
            ),
            part(
                arm,
                Main,
                beam(
                    l(-0.26, 0.125, 0.95),
                    l(-0.29, 0.13, 0.81),
                    FORWARD,
                    v(0.022, 0.022),
                    v(0.02, 0.02),
                    0.006,
                ),
            ),
            part(
                arm,
                Main,
                block_at(l(-0.29, 0.13, 0.79), 0.025, 0.022, 0.025, 0.008),
            ),
            part(
                leg,
                Main,
                beam(
                    l(-0.19, 0.06, 0.69),
                    l(-0.19, 0.06, 0.62),
                    LEFT,
                    v(0.03, 0.03),
                    v(0.028, 0.028),
                    0.008,
                ),
            ),
            part(
                leg,
                Main,
                block_at(l(-0.21, 0.065, 0.60), 0.05, 0.035, 0.02, 0.01),
            ),
        ]);
    }
    parts
}

/// The wrench, in the right hand at rest: the grip through the fist, the shaft and the
/// claw ahead of it, 0.83 m in all (UYA's is 0.82).
fn wrench_parts() -> Vec<PartDef> {
    use Look::*;
    let v = Vec2::new;
    let w = |forward: f32, up: f32| at(-0.02 + forward, -0.58, 0.88 + up);
    let mut parts = vec![
        part(
            joint::WRENCH,
            Accent,
            block_at(w(-0.225, 0.0), 0.02, 0.035, 0.035, 0.01),
        ),
        part(
            joint::WRENCH,
            Main,
            beam(
                w(-0.21, 0.0),
                w(0.06, 0.0),
                UP,
                v(0.028, 0.028),
                v(0.028, 0.028),
                0.01,
            ),
        ),
        part(
            joint::WRENCH,
            Main,
            beam(
                w(0.06, 0.0),
                w(0.43, 0.0),
                UP,
                v(0.02, 0.02),
                v(0.02, 0.02),
                0.006,
            ),
        ),
        part(
            joint::WRENCH,
            Main,
            block_at(w(0.46, 0.0), 0.04, 0.03, 0.075, 0.012),
        ),
    ];
    for up in [0.055, -0.055] {
        parts.push(part(
            joint::WRENCH,
            Main,
            beam(
                w(0.48, up),
                w(0.61, up * 1.25),
                UP,
                v(0.025, 0.028),
                v(0.02, 0.024),
                0.008,
            ),
        ));
    }
    parts
}

/// The hero, Clank and the wrench.
pub fn hero() -> ModelDef {
    let mut parts = ratchet_parts();
    parts.extend(clank_parts());
    parts.extend(wrench_parts());
    ModelDef {
        joints: joints(),
        parts,
    }
}

/// Just the wrench, for the copy that flies when it is thrown.
pub fn wrench_parts_only() -> ModelDef {
    let mut model = hero();
    model.parts.retain(|part| part.joint == joint::WRENCH);
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hull::convex_hull;

    #[test]
    fn joints_come_after_their_parents_and_are_named_once() {
        let model = hero();
        assert_eq!(model.joints.len(), joint::COUNT);
        for (index, joint) in model.joints.iter().enumerate() {
            if let Some(parent) = joint.parent {
                assert!(parent < index, "{} before its parent", joint.name);
            }
            let same = model.joints.iter().filter(|j| j.name == joint.name).count();
            assert_eq!(same, 1, "{} named twice", joint.name);
        }
        assert_eq!(model.joints[joint::HAND_R].name, "hand_r");
        assert_eq!(model.joints[joint::HAND_R].uya, Some(56));
        assert_eq!(model.joints[joint::HAND_L].uya, Some(76));
        assert_eq!(model.joints[joint::TOE_L].uya, Some(102));
        assert_eq!(model.joints[joint::TAIL[6]].uya, Some(110));
    }

    #[test]
    fn the_right_hand_is_on_the_heros_right() {
        let model = hero();
        assert!(model.joints[joint::HAND_R].rest.x > 0.4);
        assert!(model.joints[joint::HAND_L].rest.x < -0.4);
        assert!(
            model.joints[joint::TOE_R].rest.z < 0.0,
            "toes point forward"
        );
        assert!(
            model.joints[joint::TAIL[6]].rest.z > 0.5,
            "the tail points back"
        );
    }

    #[test]
    fn every_part_is_a_solid_of_medium_detail() {
        let model = hero();
        let mut triangles = 0;
        for part in &model.parts {
            let hull = convex_hull(&part.points);
            assert!(
                hull.len() >= 8,
                "a part of {} is flat",
                model.joints[part.joint].name
            );
            triangles += hull.len();
        }
        assert!((1000..6000).contains(&triangles), "{triangles} triangles");
    }

    #[test]
    fn the_model_is_about_as_tall_as_the_hero() {
        let model = hero();
        let top = model
            .parts
            .iter()
            .flat_map(|part| &part.points)
            .map(|p| p.y)
            .fold(f32::MIN, f32::max);
        assert!((1.3..1.5).contains(&top), "{top}");
    }
}
