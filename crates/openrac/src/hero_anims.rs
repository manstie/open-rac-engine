//! The hero's animations from the user's own UYA disc, as clips for the hero kit's rig
//! (PLAN.md P1.17). Nothing here is stored: it's read from the disc at start-up.
//!
//! - The skeleton and every sequence come from Veldin's core, which has all 127
//!   (`docs/research/disc-layout.md`, "Level data").
//! - Which sequence each move plays, and where it starts, is UYA's own choice, read in its
//!   code (`docs/research/hero-controller.md`, "Animations per state"), and so are the
//!   fidgets' rules and the walk's change of stride.
//! - The rig's joints are UYA's, so [`SkeletonMap`] gives each rig joint its UYA joint's
//!   turn from rest relative to its parent's, its offset from where UYA has it at rest, so
//!   every model the rig wears keeps its own proportions, and its scale: UYA squashes the
//!   hero by scaling his root, and works his face by scaling as much as turning.

use bevy::math::{Affine3A, Mat3, Quat, Vec3};
use rac_formats::moby::{Joint, Keyframe, MobyClass};
use rac_rig::{
    FidgetRule, FlipSide, Landing, LineUpError, RigAnimations, RigClip, RigKeyframe, RigMove,
    SkeletonMap, SourceJoint, model,
};

/// Veldin: a new game starts here, and its core holds every one of the hero's sequences.
pub const LEVEL: usize = 1;

/// UYA's sequence for each move, whether it loops, and the keyframe the game starts it at.
/// A landing finishes its jump's own sequence from two keyframes past the one the game
/// lands it on (`hero_change_state` sets it per jump), and a fall's has a sequence of its
/// own, started deeper after a long fall.
const MOVES: &[(RigMove, usize, bool, usize)] = &[
    (RigMove::Idle, 0, true, 0),
    (RigMove::IdleHurt, 74, true, 0),
    (RigMove::Fidget(0), 1, false, 0),
    (RigMove::Fidget(1), 2, false, 0),
    (RigMove::Fidget(2), 73, false, 0),
    (RigMove::Fidget(3), 78, false, 0),
    (RigMove::SetOff, 3, true, 15),
    (RigMove::Run, 4, true, 16),
    (RigMove::StrafeBack, 20, true, 0),
    (RigMove::Skid, 6, false, 0),
    (RigMove::RunSkid, 5, false, 0),
    (RigMove::Crouch, 13, false, 2),
    (RigMove::CrouchMove, 14, true, 0),
    (RigMove::FallStart, 10, false, 0),
    (RigMove::Fall, 11, true, 0),
    (RigMove::Land(Landing::Fall), 12, false, 9),
    (RigMove::Land(Landing::LongFall), 12, false, 4),
    (RigMove::Land(Landing::Jump), 7, false, 14),
    (RigMove::Land(Landing::RunJump), 9, false, 13),
    (RigMove::Land(Landing::Flip(FlipSide::Left)), 28, false, 26),
    (RigMove::Land(Landing::Flip(FlipSide::Right)), 29, false, 26),
    (
        RigMove::Land(Landing::Flip(FlipSide::Forward)),
        30,
        false,
        26,
    ),
    (RigMove::Land(Landing::Flip(FlipSide::Back)), 31, false, 26),
    (RigMove::Land(Landing::HeliJump), 21, false, 32),
    (RigMove::Land(Landing::WallJump), 35, false, 12),
    (RigMove::Land(Landing::WaterJump), 89, false, 16),
    (RigMove::Land(Landing::LedgeJump), 34, false, 21),
    (RigMove::Land(Landing::DoubleJump), 22, false, 21),
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

/// The idle's fidgets, by [`RigMove::Fidget`] number: sequences 1 and 2 about every 10 s,
/// 73 every 8 s (where nothing is within 8 u overhead, which the rig can't see), and 78 at
/// low health (UYA's table at 0x2F4370).
const FIDGETS: [FidgetRule; 4] = [
    FidgetRule {
        interval: 10.0,
        cooldown: 5.5,
        low_health: false,
    },
    FidgetRule {
        interval: 10.0,
        cooldown: 5.5,
        low_health: false,
    },
    FidgetRule {
        interval: 8.0,
        cooldown: 5.0,
        low_health: false,
    },
    FidgetRule {
        interval: 4.0,
        cooldown: 2.5,
        low_health: true,
    },
];

/// Keyframes the walk moves on by when it changes to running, and to setting off (UYA
/// 0x1DBE60).
const WALK_PHASE: [usize; 2] = [1, 5];

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

/// A scale along a joint's own axes, from UYA's axes to Bevy's: `to_bevy` permutes the
/// axes, with signs a scale doesn't have.
fn scale_to_bevy([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(y, z, x)
}

/// Each UYA joint relative to its parent in one keyframe, in UYA's units and axes: its
/// rotation, translation and scale.
fn locals(class: &MobyClass, keyframe: &Keyframe) -> Vec<(Quat, Vec3, Vec3)> {
    let mut locals: Vec<(Quat, Vec3, Vec3)> = class
        .joints
        .iter()
        .enumerate()
        .map(|(index, joint)| {
            (
                Quat::from_array(keyframe.rotation(index)),
                Vec3::from_array(joint.offset),
                Vec3::ONE,
            )
        })
        .collect();
    for entry in &keyframe.translations {
        if let Some(local) = locals.get_mut(entry.joint as usize) {
            local.1 = Vec3::from_array(entry.offset.map(|v| v as f32));
        }
    }
    for entry in &keyframe.scales {
        if let Some(local) = locals.get_mut(entry.joint as usize) {
            local.2 = Vec3::from_array(entry.scale);
        }
    }
    locals
}

/// Each joint's transform in the model for one keyframe, in its units and axes: its parent's,
/// times its translation, rotation and scale, so a scale carries to its children
/// (`moby-format.md`, "Posing a skeleton").
fn world(class: &MobyClass, keyframe: &Keyframe) -> Vec<Affine3A> {
    let mut world: Vec<Affine3A> = Vec::with_capacity(class.joints.len());
    for ((rotation, translation, scale), joint) in
        locals(class, keyframe).into_iter().zip(&class.joints)
    {
        let local = Affine3A::from_scale_rotation_translation(scale, rotation, translation);
        let parent = joint.parent.and_then(|p| world.get(p)).copied();
        world.push(parent.map_or(local, |parent| parent * local));
    }
    world
}

/// Each joint's skinning transform for one keyframe, in the hero's space (metres, Bevy
/// axes): what takes a vertex bound at rest to where the pose puts it. It's the joint's
/// transform times its inverse bind, as the game skins (`moby-format.md`, "Skeleton").
pub(crate) fn skinning(class: &MobyClass, keyframe: &Keyframe) -> Vec<Affine3A> {
    let unit = class.unit();
    // Bevy's axes from UYA's, and metres from class units.
    let axes = Mat3::from_cols(to_bevy(Vec3::X), to_bevy(Vec3::Y), to_bevy(Vec3::Z));
    let into = Affine3A::from_mat3(axes) * Affine3A::from_scale(Vec3::splat(unit));
    world(class, keyframe)
        .into_iter()
        .zip(&class.joints)
        .map(|(posed, joint)| {
            let [r0, r1, r2] = joint.inverse_bind;
            let bind = Affine3A::from_mat3_translation(
                Mat3::from_cols(
                    Vec3::new(r0[0], r1[0], r2[0]),
                    Vec3::new(r0[1], r1[1], r2[1]),
                    Vec3::new(r0[2], r1[2], r2[2]),
                ),
                Vec3::new(r0[3], r1[3], r2[3]),
            );
            into * posed * bind * into.inverse()
        })
        .collect()
}

/// Where each joint sits in one keyframe, in the hero's space.
pub(crate) fn joint_positions(class: &MobyClass, keyframe: &Keyframe) -> Vec<Vec3> {
    let unit = class.unit();
    world(class, keyframe)
        .into_iter()
        .map(|posed| to_bevy(Vec3::from(posed.translation) * unit))
        .collect()
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
    let turns: Vec<Quat> = class.joints.iter().map(rest_orientation).collect();
    SkeletonMap::new(&rig, &sources, &parents, &rest, &turns)
}

/// One keyframe as the rig's source pose: each UYA joint relative to its parent, in the
/// hero's axes and metres. A joint with no children whose bind scales (UYA's brows) takes
/// that scale into its own, as it reaches only its vertices.
fn source_pose(class: &MobyClass, keyframe: &Keyframe, leaves: &[bool]) -> Vec<SourceJoint> {
    let unit = class.unit();
    locals(class, keyframe)
        .into_iter()
        .zip(&class.joints)
        .zip(leaves)
        .map(|(((rotation, translation, scale), joint), &leaf)| {
            let bound = if leaf {
                Vec3::from_array(joint.rest_scale())
            } else {
                Vec3::ONE
            };
            SourceJoint {
                rotation: quat_to_bevy(rotation),
                translation: to_bevy(translation * unit),
                scale: scale_to_bevy((scale * bound).to_array()),
            }
        })
        .collect()
}

/// The rig's clips for every move UYA animates, from the hero's class.
pub fn rig_animations(class: &MobyClass) -> Result<RigAnimations, LineUpError> {
    let map = skeleton_map(class)?;
    let mut animations = RigAnimations {
        driven: map.driven(),
        fidgets: FIDGETS.to_vec(),
        walk_phase: WALK_PHASE,
        ..Default::default()
    };
    let mut leaves = vec![true; class.joints.len()];
    for parent in class.joints.iter().filter_map(|j| j.parent) {
        leaves[parent] = false;
    }
    for &(movement, number, looping, start) in MOVES {
        let Some(Some(sequence)) = class.sequences.get(number) else {
            continue;
        };
        if start >= sequence.keyframes.len() {
            continue;
        }
        let keyframes: Vec<RigKeyframe> = sequence
            .keyframes
            .iter()
            .map(|keyframe| {
                let (rotations, offsets, scales) = map.pose(&source_pose(class, keyframe, &leaves));
                RigKeyframe {
                    ticks: keyframe.ticks(),
                    frame: keyframe.stamp as f32 / 16.0,
                    rotations,
                    offsets,
                    scales,
                }
            })
            .collect();
        animations.clips.insert(
            movement,
            RigClip {
                keyframes,
                looping,
                start,
            },
        );
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
        let keyframe = &class.sequences[0].as_ref().unwrap().keyframes[0];
        assert!(joint_positions(&class, keyframe)[1].distance(Vec3::Y) < 1e-5);
        let skin = skinning(&class, keyframe);
        let facing = skin[1].transform_vector3(Vec3::NEG_Z);
        assert!(
            facing.distance(Vec3::NEG_X) < 1e-3,
            "forward turned to the left: {facing}"
        );
        // A vertex on the hips turns about them, which stay 1 up.
        let ahead = skin[1].transform_point3(Vec3::new(0.0, 1.0, -0.5));
        assert!(ahead.distance(Vec3::new(-0.5, 1.0, 0.0)) < 1e-4, "{ahead}");
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
        let keyframe = &class.sequences[0].as_ref().unwrap().keyframes[0];
        let skin = skinning(&class, keyframe);
        let vertex = Vec3::new(0.2, 1.0, -0.5);
        assert!(skin[1].transform_point3(vertex).distance(vertex) < 1e-4);
        assert!(joint_positions(&class, keyframe)[1].distance(Vec3::Y) < 1e-5);
        let animations = rig_animations(&class).unwrap();
        let rotation = animations.clips[&RigMove::Idle].keyframes[0].rotations[1];
        assert!(rotation.angle_between(Quat::IDENTITY) < 1e-3);
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
        assert!(keyframe.scales.iter().all(|s| *s == Vec3::ONE));
        assert_eq!(animations.fidgets.len(), 4);
    }

    #[test]
    fn scales_carry_into_the_clips_and_a_leafs_bind_scale_with_them() {
        // The root squashed to 0.9 tall in UYA's axes (its z), and the last joint, a leaf,
        // bound stretched 2 along UYA's x and scaled back to 0.5.
        let mut class = class();
        let last = class.joints.len() - 1;
        class.joints[last].inverse_bind[0][0] = 2.0;
        let keyframe = &mut class.sequences[0].as_mut().unwrap().keyframes[0];
        keyframe.scales = vec![
            rac_formats::moby::JointScale {
                joint: 0,
                scale: [1.0, 1.0, 0.9],
            },
            rac_formats::moby::JointScale {
                joint: last as u8,
                scale: [0.5, 1.0, 1.0],
            },
        ];
        let animations = rig_animations(&class).unwrap();
        let scales = &animations.clips[&RigMove::Idle].keyframes[0].scales;
        assert!(
            scales[0].distance(Vec3::new(1.0, 0.9, 1.0)) < 1e-6,
            "up is Bevy's y"
        );
        assert!(
            scales[last].distance(Vec3::ONE) < 1e-6,
            "2 × 0.5 along Bevy's z"
        );
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
