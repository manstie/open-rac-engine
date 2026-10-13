//! The hero's animations from the user's own UYA disc, as clips for the hero kit's rig
//! (PLAN.md P1.17). Nothing here is stored: it's read from the disc at start-up.
//!
//! - The skeleton and every sequence come from Veldin's core, which has all 127
//!   (`docs/research/disc-layout.md`, "Level data").
//! - Which sequence each move plays is UYA's own choice, read in its code
//!   (`docs/research/hero-controller.md`, "Animations per state").
//! - Each keyframe is posed on UYA's skeleton, then each rig joint takes the rotation and
//!   offset of its UYA joint relative to its rig parent's, so the rig's joints sit where
//!   UYA's do even where the rig leaves joints out.

use bevy::math::{Quat, Vec3};
use rac_formats::moby::{Keyframe, MobyClass};
use rac_rig::{FlipSide, RigAnimations, RigClip, RigKeyframe, RigMove, model};

/// Veldin: a new game starts here, and its core holds every one of the hero's sequences.
pub const LEVEL: usize = 1;

/// UYA's sequence for each move, whether it loops, and the keyframe the game starts it at.
const MOVES: &[(RigMove, usize, bool, usize)] = &[
    (RigMove::Idle, 0, true, 0),
    (RigMove::SetOff, 3, true, 0),
    (RigMove::Run, 4, true, 0),
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
    (RigMove::Swim, 54, true, 0),
    (RigMove::SwimIdle, 55, true, 0),
    (RigMove::SwimFast, 56, true, 0),
    (RigMove::SurfaceSwim, 82, true, 0),
    (RigMove::SurfaceIdle, 53, true, 0),
    (RigMove::Dead, 64, false, 0),
];

/// UYA's class axes (x forward, y left, z up) to the hero's local space in Bevy (x right,
/// y up, z back). It's a rotation, so quaternions' vector parts map the same way.
fn to_bevy(v: Vec3) -> Vec3 {
    Vec3::new(-v.y, v.z, -v.x)
}

fn quat_to_bevy(q: Quat) -> Quat {
    let v = to_bevy(Vec3::new(q.x, q.y, q.z));
    Quat::from_xyzw(v.x, v.y, v.z, q.w)
}

/// Each UYA joint's rotation and position (world units) in the hero's space, for one
/// keyframe.
fn pose(class: &MobyClass, keyframe: &Keyframe) -> (Vec<Quat>, Vec<Vec3>) {
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
        rotations.into_iter().map(quat_to_bevy).collect(),
        positions.into_iter().map(|p| to_bevy(p * unit)).collect(),
    )
}

/// The rig's clips for every move UYA animates, from the hero's class.
pub fn rig_animations(class: &MobyClass) -> RigAnimations {
    let rig = model::hero().joints;
    let uya = |joint: usize| rig[joint].uya.map(|u| u as usize);
    let driven: Vec<bool> = rig
        .iter()
        .map(|joint| joint.uya.is_some_and(|u| (u as usize) < class.joints.len()))
        .collect();

    let mut animations = RigAnimations {
        driven: driven.clone(),
        ..Default::default()
    };
    for &(movement, number, looping, start) in MOVES {
        let Some(Some(sequence)) = class.sequences.get(number) else {
            continue;
        };
        let mut keyframes = Vec::new();
        for keyframe in sequence.keyframes.iter().skip(start) {
            let (world, positions) = pose(class, keyframe);
            let mut rotations = vec![Quat::IDENTITY; rig.len()];
            let mut offsets = vec![Vec3::ZERO; rig.len()];
            for (index, joint) in rig.iter().enumerate() {
                let (true, Some(own)) = (driven[index], uya(index)) else {
                    continue;
                };
                let parent = joint.parent.and_then(uya);
                let (parent_rotation, parent_position) = match parent {
                    Some(p) => (world[p], positions[p]),
                    None => (Quat::IDENTITY, Vec3::ZERO),
                };
                let inverse = parent_rotation.inverse();
                rotations[index] = (inverse * world[own]).normalize();
                let rest = joint.rest - joint.parent.map_or(Vec3::ZERO, |p| rig[p].rest);
                offsets[index] = inverse * (positions[own] - parent_position) - rest;
            }
            keyframes.push(RigKeyframe {
                ticks: keyframe.ticks(),
                frame: keyframe.stamp as f32 / 16.0,
                rotations,
                offsets,
            });
        }
        if !keyframes.is_empty() {
            animations
                .clips
                .insert(movement, RigClip { keyframes, looping });
        }
    }
    animations
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_formats::moby::{Joint, Sequence};

    /// A two-joint class: a root and a joint 1 unit up, with joint 1 turned a quarter
    /// turn about UYA's up axis in its one keyframe.
    fn class() -> MobyClass {
        let joint = |parent, rest: [f32; 3]| Joint {
            parent,
            offset: rest,
            rest,
        };
        // The game stores the inverse rotation: -90° about z for a +90° turn.
        let quarter = (0.5f32.sqrt() * 32768.0) as i16;
        let keyframe = Keyframe {
            rate: 0.5,
            stamp: 32,
            rotations: vec![[0, 0, 0, 0x7FFF], [0, 0, -quarter, quarter]],
            scales: Vec::new(),
            translations: Vec::new(),
        };
        MobyClass {
            scale: 1024.0,
            joints: vec![joint(None, [0.0; 3]), joint(Some(0), [0.0, 0.0, 1.0])],
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
    fn idle_becomes_a_looping_clip_on_the_rig() {
        let animations = rig_animations(&class());
        let idle = &animations.clips[&RigMove::Idle];
        assert!(idle.looping);
        assert_eq!(idle.keyframes[0].frame, 2.0);
        assert_eq!(idle.keyframes[0].ticks, 2.0);
        assert!(animations.driven[model::joint::ROOT]);
        assert!(animations.driven[model::joint::HIPS]);
        // The class has two joints, so the rig's other UYA joints aren't driven.
        assert!(!animations.driven[model::joint::HEAD]);
        assert!(!animations.driven[model::joint::CLANK]);
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
        let animations = rig_animations(&hero);
        assert_eq!(animations.clips.len(), MOVES.len());
        // Swing 1 runs from its start frame, 16, as the melee code expects.
        let swing = &animations.clips[&RigMove::Combo(0)];
        assert_eq!(swing.keyframes[0].frame, 16.0);
    }
}
