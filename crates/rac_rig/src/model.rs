//! The placeholder hero: a blocky Ratchet with Clank on his back and the wrench in his
//! right hand, made of chamfered blocks.
//!
//! It is modelled by hand. UYA's hero was the reference for proportions only: the joints
//! sit where UYA's do, rounded to the centimetre, and the blocks were sized against the
//! hero's mesh measured from the user's own disc (`docs/research/moby-format.md`). No
//! geometry comes from the game. The skeleton is laid out as UYA's hero's (joint *n* of
//! the rig is UYA's joint *n*, up to [`joint::UYA_JOINTS`]), so animation read from a disc
//! drives the rig joint for joint, face and fingers included.

use bevy::math::{Mat3, Quat, Vec2, Vec3};

use crate::shapes::{at, beam, block_at};

/// The rig's joints, parents before children. Joints 0 to 110 are UYA's hero's, in its
/// order; Clank's and the wrench's are the rig's own and come after.
pub mod joint {
    pub const ROOT: usize = 0;
    pub const HIPS: usize = 1;
    pub const WAIST: usize = 2;
    pub const SPINE: usize = 3;
    pub const SPINE_UPPER: usize = 4;
    pub const CHEST: usize = 5;
    pub const CHEST_UPPER: usize = 6;
    pub const NECK: usize = 7;
    pub const HEAD: usize = 8;
    pub const JAW: usize = 10;
    /// The face's other joints: brows, lids, cheeks, mouth and nose. UYA animates them
    /// by moving and scaling them as much as by turning them; which is which isn't read,
    /// so they go by UYA's numbers.
    pub const FACE: [usize; 36] = [
        9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
        33, 34, 35, 36, 37, 38, 39, 40, 47, 48, 49, 50, 51,
    ];
    pub const EAR_L: usize = 41;
    pub const EAR_L_TIP: usize = 42;
    pub const EAR_L_END: usize = 43;
    pub const EAR_R: usize = 44;
    pub const EAR_R_TIP: usize = 45;
    pub const EAR_R_END: usize = 46;
    pub const CLAVICLE_R: usize = 52;
    pub const SHOULDER_R: usize = 53;
    pub const ELBOW_R: usize = 54;
    pub const FOREARM_R: usize = 55;
    pub const HAND_R: usize = 56;
    /// Each finger's three joints, from the knuckle out.
    pub const INDEX_R: [usize; 3] = [57, 58, 59];
    pub const MIDDLE_R: [usize; 3] = [60, 61, 62];
    pub const RING_R: [usize; 3] = [63, 64, 65];
    pub const PINKY_R: [usize; 3] = [66, 67, 68];
    /// The middle finger's knuckle, which carries the open model's fingers.
    pub const FINGERS_R: usize = 60;
    pub const THUMB_R: usize = 69;
    pub const THUMB_R_TIP: usize = 70;
    /// A joint at the elbow that UYA scales to keep the elbow's shape as it bends.
    pub const ELBOW_R_HELPER: usize = 71;
    pub const CLAVICLE_L: usize = 72;
    pub const SHOULDER_L: usize = 73;
    pub const ELBOW_L: usize = 74;
    pub const FOREARM_L: usize = 75;
    pub const HAND_L: usize = 76;
    pub const INDEX_L: [usize; 3] = [77, 78, 79];
    pub const MIDDLE_L: [usize; 3] = [80, 81, 82];
    pub const RING_L: [usize; 3] = [83, 84, 85];
    pub const PINKY_L: [usize; 3] = [86, 87, 88];
    pub const FINGERS_L: usize = 80;
    pub const THUMB_L: usize = 89;
    pub const THUMB_L_TIP: usize = 90;
    pub const ELBOW_L_HELPER: usize = 91;
    /// Each arm from the clavicle to the elbow's helper, fingers included: what the
    /// games' arm layers move.
    pub const ARM_R: std::ops::RangeInclusive<usize> = CLAVICLE_R..=ELBOW_R_HELPER;
    pub const ARM_L: std::ops::RangeInclusive<usize> = CLAVICLE_L..=ELBOW_L_HELPER;
    pub const THIGH_R: usize = 92;
    pub const KNEE_R: usize = 93;
    pub const ANKLE_R: usize = 94;
    pub const HEEL_R: usize = 95;
    /// The ball of the foot.
    pub const TOE_R: usize = 96;
    pub const TOE_R_TIP: usize = 97;
    pub const THIGH_L: usize = 98;
    pub const KNEE_L: usize = 99;
    pub const ANKLE_L: usize = 100;
    pub const HEEL_L: usize = 101;
    pub const TOE_L: usize = 102;
    pub const TOE_L_TIP: usize = 103;
    /// The tail's seven joints, from the hips out.
    pub const TAIL: [usize; 7] = [104, 105, 106, 107, 108, 109, 110];
    /// How many of the rig's joints are UYA's hero's.
    pub const UYA_JOINTS: usize = 111;
    pub const CLANK: usize = 111;
    pub const CLANK_HEAD: usize = 112;
    pub const CLANK_ANTENNA: usize = 113;
    pub const CLANK_ARM_L: usize = 114;
    pub const CLANK_ARM_R: usize = 115;
    pub const CLANK_LEG_L: usize = 116;
    pub const CLANK_LEG_R: usize = 117;
    pub const WRENCH: usize = 118;
    pub const COUNT: usize = 119;
}

/// One joint of the rig. At rest every joint is unrotated: the hero stands in a T-pose,
/// facing forward, as UYA's skeleton does. A model whose own joints are turned at rest
/// says so in [`crate::ModelData::turns`].
#[derive(Debug, Clone, PartialEq)]
pub struct JointDef {
    pub name: &'static str,
    pub parent: Option<usize>,
    /// In the hero's local space, metres from his feet.
    pub rest: Vec3,
    /// The UYA hero joint it stands for, if any.
    pub uya: Option<u8>,
}

/// Which way the wrench points from the right hand at rest, in the hero's space: forward
/// from the fist, a little out and down, as UYA's held wrench does (its pose while
/// Ratchet stands, measured on the user's disc). Every model's wrench points this way, so
/// clips from a disc swing any of them as the game swings its own, and the procedural
/// poses aim the hand by it.
pub fn wrench_points() -> Vec3 {
    Vec3::new(0.47, -0.12, -0.87).normalize()
}

/// How a gun sits in the right fist, in the hand's frame at rest as for
/// [`wrench_points`]: its barrel out along the forearm, and its top on the thumb's side,
/// where the wrench's head sticks out. Guns are modelled with the barrel along −Z and the
/// top along +Y (`crate::guns`); this turns them into the fist.
pub fn gun_grip() -> Quat {
    // At rest the right arm reaches out along +x, so its forearm runs that way.
    let barrel = Vec3::X;
    let top = (wrench_points() - barrel * wrench_points().dot(barrel)).normalize();
    Quat::from_mat3(&Mat3::from_cols(top.cross(-barrel), top, -barrel))
}

/// How far the fist's middle, where a gun's grip sits, is from the hand joint, in the
/// hand's frame at rest.
pub const GRIP_FROM_HAND: Vec3 = Vec3::new(0.05, 0.0, 0.0);

/// The open wrench is modelled pointing forward, then turned about the hand by this.
pub(crate) fn wrench_turn() -> Quat {
    Quat::from_rotation_arc(Vec3::NEG_Z, wrench_points())
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

fn own(name: &'static str, parent: usize, rest: Vec3) -> JointDef {
    JointDef {
        name,
        parent: Some(parent),
        rest,
        uya: None,
    }
}

/// UYA's hero's joints in its order: name, parent, and where it sits at rest (forward,
/// left, up, metres), rounded to the centimetre. The sides' joints mirror each other;
/// UYA's own are a millimetre or so apart.
#[rustfmt::skip]
const UYA: [(&str, usize, [f32; 3]); joint::UYA_JOINTS] = [
    ("root", 0, [0.0, 0.0, 0.0]),
    ("hips", 0, [-0.04, 0.0, 0.49]),
    ("waist", 1, [-0.04, 0.0, 0.52]),
    ("spine", 2, [-0.03, 0.0, 0.59]),
    ("spine_upper", 3, [-0.03, 0.0, 0.69]),
    ("chest", 4, [-0.05, 0.0, 0.80]),
    ("chest_upper", 5, [-0.02, 0.0, 0.90]),
    ("neck", 6, [-0.02, 0.0, 0.95]),
    ("head", 7, [-0.01, 0.0, 1.08]),
    ("face_09", 8, [0.15, 0.0, 1.07]),
    ("jaw", 8, [0.0, 0.0, 1.04]),
    ("face_11", 10, [0.13, 0.0, 0.96]),
    ("face_12", 10, [0.16, 0.0, 1.01]),
    ("face_13", 10, [0.14, 0.0, 0.97]),
    ("face_14", 8, [-0.04, 0.0, 1.00]),
    ("face_15", 14, [0.05, 0.0, 0.99]),
    ("face_16", 8, [0.06, -0.05, 1.04]),
    ("face_17", 8, [0.06, 0.05, 1.04]),
    ("face_18", 8, [0.03, 0.0, 1.15]),
    ("face_19", 18, [0.14, 0.09, 1.14]),
    ("face_20", 18, [0.14, -0.09, 1.14]),
    ("face_21", 8, [0.02, -0.01, 1.13]),
    ("face_22", 21, [0.10, -0.06, 1.25]),
    ("face_23", 8, [0.02, 0.01, 1.13]),
    ("face_24", 23, [0.10, 0.06, 1.25]),
    ("face_25", 8, [-0.05, 0.01, 1.17]),
    ("face_26", 25, [0.09, -0.08, 1.24]),
    ("face_27", 26, [0.06, -0.15, 1.19]),
    ("face_28", 26, [0.15, -0.02, 1.19]),
    ("face_29", 8, [-0.05, -0.01, 1.17]),
    ("face_30", 29, [0.09, 0.08, 1.24]),
    ("face_31", 30, [0.06, 0.15, 1.19]),
    ("face_32", 30, [0.15, 0.02, 1.19]),
    ("face_33", 8, [-0.03, -0.18, 1.09]),
    ("face_34", 33, [-0.03, -0.20, 1.04]),
    ("face_35", 8, [-0.03, 0.18, 1.09]),
    ("face_36", 35, [-0.03, 0.20, 1.04]),
    ("face_37", 8, [0.15, 0.0, 1.05]),
    ("face_38", 8, [0.19, 0.0, 1.02]),
    ("face_39", 8, [0.14, 0.09, 1.09]),
    ("face_40", 8, [0.14, -0.09, 1.09]),
    ("ear_l", 8, [-0.10, 0.11, 1.19]),
    ("ear_l_tip", 41, [-0.21, 0.25, 1.27]),
    ("ear_l_end", 42, [-0.27, 0.36, 1.35]),
    ("ear_r", 8, [-0.10, -0.12, 1.19]),
    ("ear_r_tip", 44, [-0.21, -0.26, 1.27]),
    ("ear_r_end", 45, [-0.27, -0.36, 1.35]),
    ("face_47", 8, [0.11, -0.13, 1.02]),
    ("face_48", 8, [0.11, 0.13, 1.02]),
    ("face_49", 8, [0.13, -0.08, 1.20]),
    ("face_50", 8, [0.13, 0.08, 1.20]),
    ("face_51", 8, [0.15, 0.0, 1.02]),
    ("clavicle_r", 6, [-0.02, -0.08, 0.91]),
    ("shoulder_r", 52, [-0.02, -0.15, 0.92]),
    ("elbow_r", 53, [-0.03, -0.35, 0.88]),
    ("forearm_r", 54, [-0.03, -0.39, 0.88]),
    ("hand_r", 55, [-0.02, -0.52, 0.88]),
    ("index_r_1", 56, [0.07, -0.68, 0.92]),
    ("index_r_2", 57, [0.07, -0.74, 0.92]),
    ("index_r_3", 58, [0.07, -0.80, 0.92]),
    ("middle_r_1", 56, [0.01, -0.69, 0.92]),
    ("middle_r_2", 60, [0.01, -0.76, 0.92]),
    ("middle_r_3", 61, [0.01, -0.83, 0.92]),
    ("ring_r_1", 56, [-0.05, -0.68, 0.92]),
    ("ring_r_2", 63, [-0.05, -0.75, 0.92]),
    ("ring_r_3", 64, [-0.05, -0.81, 0.92]),
    ("pinky_r_1", 56, [-0.11, -0.68, 0.92]),
    ("pinky_r_2", 66, [-0.11, -0.72, 0.92]),
    ("pinky_r_3", 67, [-0.11, -0.77, 0.92]),
    ("thumb_r_1", 56, [0.09, -0.59, 0.90]),
    ("thumb_r_2", 69, [0.15, -0.61, 0.90]),
    ("elbow_r_helper", 54, [-0.02, -0.38, 0.88]),
    ("clavicle_l", 6, [-0.02, 0.08, 0.91]),
    ("shoulder_l", 72, [-0.02, 0.15, 0.92]),
    ("elbow_l", 73, [-0.03, 0.35, 0.88]),
    ("forearm_l", 74, [-0.03, 0.39, 0.88]),
    ("hand_l", 75, [-0.02, 0.52, 0.88]),
    ("index_l_1", 76, [0.07, 0.68, 0.92]),
    ("index_l_2", 77, [0.07, 0.74, 0.92]),
    ("index_l_3", 78, [0.07, 0.80, 0.92]),
    ("middle_l_1", 76, [0.01, 0.69, 0.92]),
    ("middle_l_2", 80, [0.01, 0.76, 0.92]),
    ("middle_l_3", 81, [0.01, 0.83, 0.92]),
    ("ring_l_1", 76, [-0.05, 0.68, 0.92]),
    ("ring_l_2", 83, [-0.05, 0.75, 0.92]),
    ("ring_l_3", 84, [-0.05, 0.81, 0.92]),
    ("pinky_l_1", 76, [-0.11, 0.68, 0.92]),
    ("pinky_l_2", 86, [-0.11, 0.72, 0.92]),
    ("pinky_l_3", 87, [-0.11, 0.77, 0.92]),
    ("thumb_l_1", 76, [0.09, 0.59, 0.90]),
    ("thumb_l_2", 89, [0.15, 0.61, 0.90]),
    ("elbow_l_helper", 74, [-0.02, 0.38, 0.88]),
    ("thigh_r", 1, [-0.04, -0.065, 0.46]),
    ("knee_r", 92, [0.01, -0.065, 0.27]),
    ("ankle_r", 93, [-0.04, -0.065, 0.08]),
    ("heel_r", 94, [0.03, -0.065, 0.0]),
    ("toe_r", 95, [0.14, -0.065, 0.0]),
    ("toe_r_tip", 96, [0.21, -0.065, 0.0]),
    ("thigh_l", 1, [-0.04, 0.065, 0.46]),
    ("knee_l", 98, [0.01, 0.065, 0.27]),
    ("ankle_l", 99, [-0.04, 0.065, 0.08]),
    ("heel_l", 100, [0.03, 0.065, 0.0]),
    ("toe_l", 101, [0.14, 0.065, 0.0]),
    ("toe_l_tip", 102, [0.21, 0.065, 0.0]),
    ("tail_0", 1, [-0.13, 0.0, 0.46]),
    ("tail_1", 104, [-0.21, 0.0, 0.46]),
    ("tail_2", 105, [-0.32, 0.0, 0.46]),
    ("tail_3", 106, [-0.43, 0.0, 0.46]),
    ("tail_4", 107, [-0.52, 0.0, 0.46]),
    ("tail_5", 108, [-0.59, 0.0, 0.46]),
    ("tail_6", 109, [-0.69, 0.0, 0.46]),
];

fn joints() -> Vec<JointDef> {
    use joint::*;
    let mut joints: Vec<JointDef> = UYA
        .iter()
        .enumerate()
        .map(|(index, &(name, parent, [forward, left, up]))| JointDef {
            name,
            parent: (index > 0).then_some(parent),
            rest: at(forward, left, up),
            uya: Some(index as u8),
        })
        .collect();
    // Clank rides on UYA's joint 5, facing backwards; his joints are our own.
    joints.extend([
        own("clank", CHEST, at(-0.05, 0.0, 0.80)),
        own("clank_head", CLANK, at(-0.29, 0.0, 1.0)),
        own("clank_antenna", CLANK_HEAD, at(-0.30, 0.0, 1.17)),
        own("clank_arm_l", CLANK, at(-0.26, 0.115, 0.95)),
        own("clank_arm_r", CLANK, at(-0.26, -0.115, 0.95)),
        own("clank_leg_l", CLANK, at(-0.19, 0.06, 0.68)),
        own("clank_leg_r", CLANK, at(-0.19, -0.06, 0.68)),
        // The wrench sits on the hand joint; it points as UYA's held wrench does
        // ([`wrench_points`]).
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
/// claw ahead of it, 0.83 m in all (UYA's is 0.82), modelled pointing forward and turned
/// to [`wrench_points`].
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
    let hand = at(-0.02, -0.52, 0.88);
    let turn = wrench_turn();
    for part in &mut parts {
        for point in &mut part.points {
            *point = hand + turn * (*point - hand);
        }
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
    fn the_rigs_joints_are_uyas_then_clanks_and_the_wrench() {
        let model = hero();
        let joints = &model.joints;
        for (index, joint) in joints.iter().enumerate() {
            let expected = (index < joint::UYA_JOINTS).then_some(index as u8);
            assert_eq!(joint.uya, expected, "{}", joint.name);
        }
        let descends = |mut index: usize, ancestor: usize| {
            while let Some(parent) = joints[index].parent {
                if parent == ancestor {
                    return true;
                }
                index = parent;
            }
            false
        };
        for face in joint::FACE {
            assert!(descends(face, joint::HEAD), "{}", joints[face].name);
        }
        let fingers = [
            (joint::INDEX_R, joint::INDEX_L),
            (joint::MIDDLE_R, joint::MIDDLE_L),
            (joint::RING_R, joint::RING_L),
            (joint::PINKY_R, joint::PINKY_L),
        ];
        for (right, left) in fingers {
            assert_eq!(joints[right[0]].parent, Some(joint::HAND_R));
            assert_eq!(joints[left[0]].parent, Some(joint::HAND_L));
            for (r, l) in right.into_iter().zip(left) {
                let (r, l) = (joints[r].rest, joints[l].rest);
                assert!(
                    (r * Vec3::new(-1.0, 1.0, 1.0) - l).length() < 1e-6,
                    "{r} {l}"
                );
            }
            assert_eq!(joints[right[2]].parent, Some(right[1]));
        }
        assert_eq!(joint::FINGERS_R, joint::MIDDLE_R[0]);
        assert_eq!(joints[joint::THUMB_R_TIP].parent, Some(joint::THUMB_R));
        assert_eq!(joints[joint::SHOULDER_R].parent, Some(joint::CLAVICLE_R));
        assert_eq!(joints[joint::TOE_L_TIP].parent, Some(joint::TOE_L));
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
