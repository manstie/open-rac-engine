//! The hero's animations from the user's own UYA disc, as clips for the hero kit's rig
//! (PLAN.md P1.17). Nothing here is stored: it's read from the disc at start-up.
//!
//! - The skeleton and every sequence come from Veldin's core, which has all 127
//!   (`docs/research/disc-layout.md`, "Level data").
//! - Which sequence each move plays is UYA's own choice, read in its code
//!   (`docs/research/hero-controller.md`, "Animations per state").
//! - Each keyframe is posed on UYA's skeleton, then [`SkeletonMap`] gives each rig joint the
//!   rotation of its UYA joint relative to its rig parent's, and its offset from where UYA
//!   has it at rest, so every model the rig wears keeps its own proportions.

use bevy::math::{Quat, Vec3};
use rac_formats::moby::{Joint, Keyframe, MobyClass};
use rac_rig::{
    FlipSide, LineUpError, RigAnimations, RigClip, RigKeyframe, RigMove, SkeletonMap, model,
};

/// Veldin: a new game starts here, and its core holds every one of the hero's sequences.
pub const LEVEL: usize = 1;

/// UYA's sequence for each move, whether it loops, and the keyframe the game starts it at.
const MOVES: &[(RigMove, usize, bool, usize)] = &[
    (RigMove::Idle, 0, true, 0),
    (RigMove::SetOff, 3, true, 0),
    (RigMove::Run, 4, true, 0),
    (RigMove::StrafeBack, 20, true, 0),
    (RigMove::Skid, 6, false, 0),
    (RigMove::Crouch, 13, false, 2),
    (RigMove::CrouchMove, 14, true, 0),
    (RigMove::Fall, 11, true, 0),
    (RigMove::Jump, 7, false, 0),
    (RigMove::RunJump, 9, false, 0),
    (RigMove::LongJump, 18, false, 0),
    (RigMove::Flip(FlipSide::Left), 28, false, 0),
    (RigMove::Flip(FlipSide::Right), 29, false, 0),
    (RigMove::Flip(FlipSide::Forward), 30, false, 0),
    (RigMove::Flip(FlipSide::Back), 31, false, 0),
    (RigMove::HeliJump, 21, false, 0),
    (RigMove::WallJump, 35, false, 0),
    (RigMove::WaterJump, 89, false, 0),
    (RigMove::LedgeJump, 34, false, 0),
    (RigMove::GrindJump, 72, false, 0),
    (RigMove::GrindSwitch, 30, false, 0),
    (RigMove::DoubleJump, 22, false, 3),
    (RigMove::Glide, 19, true, 0),
    (RigMove::Combo(0), 23, false, 0),
    (RigMove::Combo(1), 24, false, 0),
    (RigMove::Combo(2), 25, false, 0),
    (RigMove::JumpAttack, 40, false, 0),
    (RigMove::Throw, 26, false, 0),
    (RigMove::Hit, 16, false, 0),
    (RigMove::LedgeGrab, 32, false, 1),
    (RigMove::LedgeHang, 33, true, 0),
    (RigMove::ShimmyLeft, 36, true, 0),
    (RigMove::ShimmyRight, 37, true, 0),
    (RigMove::Grind, 44, true, 0),
    (RigMove::ZipLine, 90, true, 0),
    (RigMove::Swim, 54, true, 0),
    (RigMove::SwimIdle, 55, true, 0),
    (RigMove::SwimFast, 56, true, 0),
    (RigMove::SurfaceSwim, 82, true, 0),
    (RigMove::SurfaceIdle, 53, true, 0),
    (RigMove::Dead, 64, false, 0),
];

/// UYA's class axes (x forward, y left, z up) to the hero's local space in Bevy (x right,
/// y up, z back). It's a rotation, so quaternions' vector parts map the same way.
pub fn to_bevy(v: Vec3) -> Vec3 {
    Vec3::new(-v.y, v.z, -v.x)
}

fn quat_to_bevy(q: Quat) -> Quat {
    let v = to_bevy(Vec3::new(q.x, q.y, q.z));
    Quat::from_xyzw(v.x, v.y, v.z, q.w)
}

/// A joint's orientation at rest in the hero's space. Most joints aren't turned at rest,
/// but some are: the hero's legs by 0.7°, Clank's face, the whole wrench
/// (`moby-format.md`, "Skeleton").
pub(crate) fn rest_orientation(joint: &Joint) -> Quat {
    quat_to_bevy(Quat::from_array(joint.rest_rotation()))
}

/// Each UYA joint's turn from its rest orientation, and its position (world units), in the
/// hero's space, for one keyframe. Keyframes carry a joint's rest orientation in its
/// rotation; the rig and a mesh bound at rest need only the turn from rest.
pub(crate) fn pose(class: &MobyClass, keyframe: &Keyframe) -> (Vec<Quat>, Vec<Vec3>) {
    let count = class.joints.len();
    let mut translations: Vec<Vec3> = class
        .joints
        .iter()
        .map(|joint| Vec3::from_array(joint.offset))
        .collect();
    for entry in &keyframe.translations {
        if let Some(t) = translations.get_mut(entry.joint as usize) {
            *t = Vec3::from_array(entry.offset.map(|v| v as f32));
        }
    }
    let mut rotations = vec![Quat::IDENTITY; count];
    let mut positions = vec![Vec3::ZERO; count];
    for (index, joint) in class.joints.iter().enumerate() {
        let local = Quat::from_array(keyframe.rotation(index));
        match joint.parent {
            Some(parent) => {
                rotations[index] = rotations[parent] * local;
                positions[index] = positions[parent] + rotations[parent] * translations[index];
            }
            None => {
                rotations[index] = local;
                positions[index] = translations[index];
            }
        }
    }
    let unit = class.unit();
    (
        rotations
            .into_iter()
            .zip(&class.joints)
            .map(|(rotation, joint)| quat_to_bevy(rotation) * rest_orientation(joint).inverse())
            .collect(),
        positions.into_iter().map(|p| to_bevy(p * unit)).collect(),
    )
}

/// How UYA's hero skeleton lines up with the rig's, through the UYA joint each rig joint
/// names.
pub fn skeleton_map(class: &MobyClass) -> Result<SkeletonMap, LineUpError> {
    let rig = model::hero().joints;
    let sources: Vec<Option<usize>> = rig.iter().map(|j| j.uya.map(usize::from)).collect();
    let parents: Vec<Option<usize>> = class.joints.iter().map(|j| j.parent).collect();
    let unit = class.unit();
    let rest: Vec<Vec3> = class
        .joints
        .iter()
        .map(|j| to_bevy(Vec3::from_array(j.rest) * unit))
        .collect();
    SkeletonMap::new(&rig, &sources, &parents, &rest)
}

/// The rig's clips for every move UYA animates, from the hero's class.
pub fn rig_animations(class: &MobyClass) -> Result<RigAnimations, LineUpError> {
    let map = skeleton_map(class)?;
    let mut animations = RigAnimations {
        driven: map.driven(),
        ..Default::default()
    };
    for &(movement, number, looping, start) in MOVES {
        let Some(Some(sequence)) = class.sequences.get(number) else {
            continue;
        };
        let keyframes: Vec<RigKeyframe> = sequence
            .keyframes
            .iter()
            .skip(start)
            .map(|keyframe| {
                let (rotations, positions) = pose(class, keyframe);
                let (rotations, offsets) = map.pose(&rotations, &positions);
                RigKeyframe {
                    ticks: keyframe.ticks(),
                    frame: keyframe.stamp as f32 / 16.0,
                    rotations,
                    offsets,
                }
            })
            .collect();
        if !keyframes.is_empty() {
            animations
                .clips
                .insert(movement, RigClip { keyframes, looping });
        }
    }
    Ok(animations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_formats::moby::{Joint, Sequence};

    /// A class shaped like UYA's hero as far as the rig is concerned: each joint a rig joint
    /// names hangs from its rig parent's, and the rest from the root. Joint 1, the hips, is
    /// 1 unit up and turned a quarter turn about UYA's up axis in the one keyframe.
    fn class() -> MobyClass {
        let rig = model::hero().joints;
        let count = rig.iter().filter_map(|j| j.uya).max().unwrap() as usize + 1;
        let mut parents = vec![Some(0); count];
        parents[0] = None;
        for joint in &rig {
            if let (Some(own), Some(parent)) = (joint.uya, joint.parent) {
                parents[own as usize] = rig[parent].uya.map(usize::from);
            }
        }
        let mut joints: Vec<Joint> = Vec::new();
        for (index, parent) in parents.into_iter().enumerate() {
            let offset = if index == 1 {
                [0.0, 0.0, 1.0]
            } else {
                [0.0; 3]
            };
            let base = parent.map_or([0.0; 3], |p| joints[p].rest);
            let rest = [
                base[0] + offset[0],
                base[1] + offset[1],
                base[2] + offset[2],
            ];
            joints.push(Joint {
                parent,
                offset,
                rest,
                inverse_bind: [
                    [1.0, 0.0, 0.0, -rest[0]],
                    [0.0, 1.0, 0.0, -rest[1]],
                    [0.0, 0.0, 1.0, -rest[2]],
                ],
            });
        }
        // The game stores the inverse rotation: -90° about z for a +90° turn.
        let quarter = (0.5f32.sqrt() * 32768.0) as i16;
        let mut rotations = vec![[0, 0, 0, 0x7FFF]; count];
        rotations[1] = [0, 0, -quarter, quarter];
        let keyframe = Keyframe {
            rate: 0.5,
            stamp: 32,
            rotations,
            scales: Vec::new(),
            translations: Vec::new(),
        };
        MobyClass {
            scale: 1024.0,
            joints,
            sequences: vec![Some(Sequence {
                bounds: [0.0; 4],
                keyframes: vec![keyframe],
            })],
            packets: Vec::new(),
        }
    }

    #[test]
    fn uya_axes_become_the_heros_bevy_axes() {
        assert_eq!(to_bevy(Vec3::X), Vec3::NEG_Z, "forward");
        assert_eq!(to_bevy(Vec3::Y), Vec3::NEG_X, "left");
        assert_eq!(to_bevy(Vec3::Z), Vec3::Y, "up");
        // A turn to the left about UYA's up is a turn to the left about Bevy's up.
        let left = quat_to_bevy(Quat::from_rotation_z(0.5));
        assert!(left.angle_between(Quat::from_rotation_y(0.5)) < 1e-5);
    }

    #[test]
    fn keyframes_pose_the_uya_skeleton_in_bevy_space() {
        let class = class();
        let (rotations, positions) =
            pose(&class, &class.sequences[0].as_ref().unwrap().keyframes[0]);
        assert!(positions[1].distance(Vec3::Y) < 1e-5);
        let facing = rotations[1] * Vec3::NEG_Z;
        assert!(
            facing.distance(Vec3::NEG_X) < 1e-3,
            "forward turned to the left: {facing}"
        );
    }

    #[test]
    fn a_joint_turned_at_rest_poses_as_its_turn_from_rest() {
        // The hips turned a quarter turn about UYA's up at rest, as Clank's eyes are: the
        // keyframe's quarter turn is then only the rest orientation, so no turn at all.
        let mut class = class();
        class.joints[1].inverse_bind = [
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, -1.0],
        ];
        let (rotations, positions) =
            pose(&class, &class.sequences[0].as_ref().unwrap().keyframes[0]);
        assert!(rotations[1].angle_between(Quat::IDENTITY) < 1e-3);
        assert!(positions[1].distance(Vec3::Y) < 1e-5);
        assert!(
            rest_orientation(&class.joints[1])
                .angle_between(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
                < 1e-3,
            "a turn to the left about UYA's up is one about Bevy's"
        );
    }

    #[test]
    fn idle_becomes_a_looping_clip_on_the_rig() {
        let animations = rig_animations(&class()).unwrap();
        let idle = &animations.clips[&RigMove::Idle];
        assert!(idle.looping);
        let keyframe = &idle.keyframes[0];
        assert_eq!(keyframe.frame, 2.0);
        assert_eq!(keyframe.ticks, 2.0);
        assert!(animations.driven[model::joint::ROOT]);
        assert!(animations.driven[model::joint::HEAD]);
        assert!(
            !animations.driven[model::joint::CLANK],
            "Clank isn't UYA's hero"
        );
        let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        assert!(keyframe.rotations[model::joint::HIPS].angle_between(turn) < 1e-3);
        assert!(
            keyframe.rotations[model::joint::SPINE].angle_between(Quat::IDENTITY) < 1e-3,
            "the spine turns with the hips, not on them"
        );
        for offset in &keyframe.offsets {
            assert!(offset.length() < 1e-5, "nothing moves from rest: {offset}");
        }
    }

    #[test]
    fn a_skeleton_without_the_rigs_joints_doesnt_line_up() {
        let mut small = class();
        small.joints.truncate(2);
        assert!(matches!(
            rig_animations(&small),
            Err(LineUpError::Missing { .. })
        ));
    }
}

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use crate::discs::{Release, find};

    #[test]
    #[ignore = "needs game discs"]
    fn the_uya_disc_gives_the_hero_and_every_move() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let core = rac_formats::level::Core::read_uya(&mut disc, LEVEL).unwrap();
        let hero = core.hero().unwrap();
        assert_eq!(hero.joints.len(), 111);
        assert_eq!(hero.sequences.iter().flatten().count(), 127);
        let animations = rig_animations(&hero).unwrap();
        assert_eq!(animations.clips.len(), MOVES.len());
        // Swing 1 runs from its start frame, 16, as the melee code expects.
        let swing = &animations.clips[&RigMove::Combo(0)];
        assert_eq!(swing.keyframes[0].frame, 16.0);
    }
}
