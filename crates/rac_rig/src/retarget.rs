//! Lining another skeleton up with the rig's (PLAN.md §5, 2026-10-13): a game's hero
//! skeleton, for its animations and its mesh.
//!
//! Each rig joint names the joint of the other, "source", skeleton that it stands for, or
//! none. The bones line up when each of those descends, in the source, from the joint its
//! nearest named rig ancestor stands for. Then:
//! - a source pose becomes a rig pose: each rig joint takes its source joint's turn from
//!   rest relative to that ancestor's, its offset from where the source has it at rest, so
//!   a model of other proportions keeps its own, and its scale;
//! - a source mesh's skin weights move to the rig's joints: a source joint the rig leaves
//!   out (a pad on DL's palm, an eyelid) gives its weight to its nearest ancestor the rig
//!   has.
//!
//! A rig joint the source lacks stays as the procedural pose leaves it, at rest in the
//! clips, between joints it has.

use bevy::math::{Affine3A, Quat, Vec3};

use crate::model::JointDef;

/// Why a skeleton doesn't line up with the rig's.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LineUpError {
    #[error("{found} rig joints were mapped, but the rig has {expected}")]
    Count { expected: usize, found: usize },
    #[error("the source skeleton has {parents} parents but {rest} rest positions")]
    Source { parents: usize, rest: usize },
    #[error("the source skeleton has {parents} joints but {turns} rest turns")]
    Turns { parents: usize, turns: usize },
    #[error("source joint {joint}'s parent {parent} doesn't come before it")]
    Order { joint: usize, parent: usize },
    #[error("rig joint {joint} stands for source joint {source_joint}, past the source's {count}")]
    Missing {
        joint: &'static str,
        source_joint: usize,
        count: usize,
    },
    #[error("rig joints {first} and {second} both stand for source joint {source_joint}")]
    Shared {
        first: &'static str,
        second: &'static str,
        source_joint: usize,
    },
    #[error(
        "rig joint {joint}'s source joint {source_joint} doesn't descend from {ancestor}'s, {ancestor_source}"
    )]
    Crossed {
        joint: &'static str,
        source_joint: usize,
        ancestor: &'static str,
        ancestor_source: usize,
    },
}

/// One source joint in a pose, relative to its parent, as a keyframe gives it, in the
/// hero's axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceJoint {
    /// Its rotation relative to its parent, its turn at rest included.
    pub rotation: Quat,
    /// Where it sits from its parent, along the parent's axes and before the parent's
    /// scale, in metres.
    pub translation: Vec3,
    /// Its scale along its own axes, which its children take on too.
    pub scale: Vec3,
}

/// How a source skeleton lines up with the rig's.
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonMap {
    rig: Vec<JointDef>,
    /// Per rig joint, the source joint it stands for.
    sources: Vec<Option<usize>>,
    /// Per rig joint, its nearest ancestor that stands for a source joint.
    anchors: Vec<Option<usize>>,
    /// Per source joint, the rig joint its vertices follow.
    follows: Vec<Option<usize>>,
    /// Per source joint, its parent.
    source_parents: Vec<Option<usize>>,
    /// Per source joint, its rest position in the hero's space.
    source_rest: Vec<Vec3>,
    /// Per source joint, its orientation at rest in the hero's space.
    source_turns: Vec<Quat>,
}

impl SkeletonMap {
    /// Lines the source skeleton up with `rig`. `sources` gives each rig joint's source
    /// joint; `parents`, `rest` and `turns` describe the source skeleton, parents first,
    /// with rest positions in the hero's space (metres, Bevy axes) and each joint's
    /// orientation at rest (empty if none is turned).
    pub fn new(
        rig: &[JointDef],
        sources: &[Option<usize>],
        parents: &[Option<usize>],
        rest: &[Vec3],
        turns: &[Quat],
    ) -> Result<Self, LineUpError> {
        if sources.len() != rig.len() {
            return Err(LineUpError::Count {
                expected: rig.len(),
                found: sources.len(),
            });
        }
        if parents.len() != rest.len() {
            return Err(LineUpError::Source {
                parents: parents.len(),
                rest: rest.len(),
            });
        }
        if !turns.is_empty() && turns.len() != parents.len() {
            return Err(LineUpError::Turns {
                parents: parents.len(),
                turns: turns.len(),
            });
        }
        for (joint, parent) in parents.iter().enumerate() {
            if let Some(parent) = *parent
                && parent >= joint
            {
                return Err(LineUpError::Order { joint, parent });
            }
        }
        let descends = |mut joint: usize, ancestor: usize| {
            while let Some(parent) = parents[joint] {
                if parent == ancestor {
                    return true;
                }
                joint = parent;
            }
            false
        };

        let mut anchors: Vec<Option<usize>> = Vec::with_capacity(rig.len());
        let mut follows: Vec<Option<usize>> = vec![None; parents.len()];
        for (index, joint) in rig.iter().enumerate() {
            // Rig parents come first, so theirs is settled.
            let anchor = joint
                .parent
                .and_then(|parent| sources[parent].map(|_| parent).or(anchors[parent]));
            anchors.push(anchor);
            let Some(source) = sources[index] else {
                continue;
            };
            if source >= parents.len() {
                return Err(LineUpError::Missing {
                    joint: joint.name,
                    source_joint: source,
                    count: parents.len(),
                });
            }
            if let Some(other) = follows[source] {
                return Err(LineUpError::Shared {
                    first: rig[other].name,
                    second: joint.name,
                    source_joint: source,
                });
            }
            if let Some(anchor) = anchor
                && let Some(anchor_source) = sources[anchor]
                && !descends(source, anchor_source)
            {
                return Err(LineUpError::Crossed {
                    joint: joint.name,
                    source_joint: source,
                    ancestor: rig[anchor].name,
                    ancestor_source: anchor_source,
                });
            }
            follows[source] = Some(index);
        }
        // The joints the rig leaves out follow their nearest ancestor it has. Parents come
        // first, so one pass settles each from its parent.
        for joint in 0..parents.len() {
            if follows[joint].is_none() {
                follows[joint] = parents[joint].and_then(|parent| follows[parent]);
            }
        }

        Ok(Self {
            rig: rig.to_vec(),
            sources: sources.to_vec(),
            anchors,
            follows,
            source_parents: parents.to_vec(),
            source_rest: rest.to_vec(),
            source_turns: if turns.is_empty() {
                vec![Quat::IDENTITY; parents.len()]
            } else {
                turns.to_vec()
            },
        })
    }

    /// Which rig joints a source pose drives.
    pub fn driven(&self) -> Vec<bool> {
        self.sources.iter().map(Option::is_some).collect()
    }

    /// The rig joint whose movement source joint `joint`'s vertices follow, if any.
    pub fn follows(&self, joint: usize) -> Option<usize> {
        self.follows.get(joint).copied().flatten()
    }

    /// Where each rig joint sits at rest in the source's proportions: on its source joint,
    /// or, for a joint the source lacks, as far from its parent as in the rig.
    pub fn rest(&self) -> Vec<Vec3> {
        let mut rest: Vec<Vec3> = Vec::with_capacity(self.rig.len());
        for (index, joint) in self.rig.iter().enumerate() {
            let position = match (self.sources[index], joint.parent) {
                (Some(source), _) => self.source_rest[source],
                (None, Some(parent)) => rest[parent] + joint.rest - self.rig[parent].rest,
                (None, None) => joint.rest,
            };
            rest.push(position);
        }
        rest
    }

    /// Each rig joint's orientation at rest in the source: its source joint's, or none for
    /// a joint the source lacks. A model skinned to the source at rest wears these
    /// ([`crate::ModelData::turns`]).
    pub fn turns(&self) -> Vec<Quat> {
        self.sources
            .iter()
            .map(|source| source.map_or(Quat::IDENTITY, |s| self.source_turns[s]))
            .collect()
    }

    /// One source pose, each source joint relative to its parent, as rig joint rotations
    /// relative to their parents' (turns from rest), offsets from rest and scales: the
    /// form of [`crate::RigKeyframe`]. Joints the source doesn't drive stay at rest. The
    /// scales of source joints the rig leaves out are dropped; their turns and moves
    /// carry to their children.
    pub fn pose(&self, joints: &[SourceJoint]) -> (Vec<Quat>, Vec<Vec3>, Vec<Vec3>) {
        let count = self.rig.len();
        let mut local = vec![Quat::IDENTITY; count];
        let mut offsets = vec![Vec3::ZERO; count];
        let mut scales = vec![Vec3::ONE; count];
        for index in 0..count {
            let Some(own) = self.sources[index] else {
                continue;
            };
            let anchor = self.anchors[index].and_then(|a| self.sources[a]);
            // The source joints from just under the anchor's down to this one.
            let mut chain = vec![own];
            let mut joint = own;
            while let Some(parent) = self.source_parents[joint]
                && Some(parent) != anchor
            {
                chain.push(parent);
                joint = parent;
            }
            let (mut moved, mut turned) = (Affine3A::IDENTITY, Quat::IDENTITY);
            let mut complete = true;
            for &link in chain.iter().rev() {
                let Some(pose) = joints.get(link) else {
                    complete = false;
                    break;
                };
                let scale = if link == own { Vec3::ONE } else { pose.scale };
                moved *= Affine3A::from_scale_rotation_translation(
                    scale,
                    pose.rotation,
                    pose.translation,
                );
                turned *= pose.rotation;
            }
            if !complete {
                continue;
            }
            let (anchor_turn, anchor_rest) = anchor.map_or((Quat::IDENTITY, Vec3::ZERO), |a| {
                (self.source_turns[a], self.source_rest[a])
            });
            local[index] = (anchor_turn * turned * self.source_turns[own].inverse()).normalize();
            offsets[index] =
                anchor_turn * Vec3::from(moved.translation) - (self.source_rest[own] - anchor_rest);
            scales[index] = joints[own].scale;
        }
        (local, offsets, scales)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joint(name: &'static str, parent: Option<usize>, rest: Vec3) -> JointDef {
        JointDef {
            name,
            parent,
            rest,
            uya: None,
        }
    }

    /// A rig of a root, a hip and a hand on a stick.
    fn rig() -> Vec<JointDef> {
        vec![
            joint("root", None, Vec3::ZERO),
            joint("hips", Some(0), Vec3::Y),
            joint("hand", Some(1), Vec3::new(1.0, 1.0, 0.0)),
            joint("prop", Some(2), Vec3::new(1.2, 1.0, 0.0)),
        ]
    }

    /// A source of root, hips, elbow, hand and finger, longer in the arm than the rig.
    fn source() -> (Vec<Option<usize>>, Vec<Vec3>) {
        (
            vec![None, Some(0), Some(1), Some(2), Some(3)],
            vec![
                Vec3::ZERO,
                Vec3::Y,
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(2.0, 1.0, 0.0),
                Vec3::new(2.3, 1.0, 0.0),
            ],
        )
    }

    /// The source at rest, each joint relative to its parent.
    fn at_rest(rest: &[Vec3], parents: &[Option<usize>]) -> Vec<SourceJoint> {
        rest.iter()
            .zip(parents)
            .map(|(position, parent)| SourceJoint {
                rotation: Quat::IDENTITY,
                translation: *position - parent.map_or(Vec3::ZERO, |p| rest[p]),
                scale: Vec3::ONE,
            })
            .collect()
    }

    #[test]
    fn left_out_joints_follow_their_nearest_mapped_ancestor() {
        let (parents, rest) = source();
        let map = SkeletonMap::new(
            &rig(),
            &[Some(0), Some(1), Some(3), None],
            &parents,
            &rest,
            &[],
        )
        .unwrap();
        assert_eq!(map.follows(2), Some(1), "the elbow follows the hips");
        assert_eq!(map.follows(4), Some(2), "the finger follows the hand");
        assert_eq!(map.driven(), vec![true, true, true, false]);
        let rest = map.rest();
        assert_eq!(rest[2], Vec3::new(2.0, 1.0, 0.0), "the source's long arm");
        assert!(
            rest[3].distance(Vec3::new(2.2, 1.0, 0.0)) < 1e-6,
            "the prop keeps its offset"
        );
        assert_eq!(map.turns(), vec![Quat::IDENTITY; 4]);
    }

    #[test]
    fn crossed_or_shared_bones_dont_line_up() {
        let (parents, rest) = source();
        let map =
            |sources: &[Option<usize>]| SkeletonMap::new(&rig(), sources, &parents, &rest, &[]);
        let crossed = map(&[Some(0), Some(3), Some(1), None]);
        assert!(matches!(crossed, Err(LineUpError::Crossed { .. })));
        let shared = map(&[Some(0), Some(1), Some(1), None]);
        assert!(matches!(shared, Err(LineUpError::Shared { .. })));
        let missing = map(&[Some(0), Some(1), Some(9), None]);
        assert!(matches!(missing, Err(LineUpError::Missing { .. })));
        let turns = SkeletonMap::new(&rig(), &[None; 4], &parents, &rest, &[Quat::IDENTITY]);
        assert!(matches!(turns, Err(LineUpError::Turns { .. })));
    }

    #[test]
    fn a_rig_joint_the_source_lacks_can_sit_between_two_it_has() {
        // The hips have no source joint: the hand lines up with the root's, from which the
        // source's hand descends.
        let (parents, rest) = source();
        let map = SkeletonMap::new(
            &rig(),
            &[Some(0), None, Some(3), None],
            &parents,
            &rest,
            &[],
        )
        .unwrap();
        assert_eq!(map.follows(1), Some(0), "the source's hips follow the root");
        let (local, offsets, _) = map.pose(&at_rest(&rest, &parents));
        for (rotation, offset) in local.iter().zip(&offsets) {
            assert!(rotation.angle_between(Quat::IDENTITY) < 1e-5);
            assert!(offset.length() < 1e-5, "nothing moves from rest: {offset}");
        }
    }

    #[test]
    fn poses_are_relative_to_the_rig_parents_source_joint() {
        let (parents, rest) = source();
        let map = SkeletonMap::new(
            &rig(),
            &[Some(0), Some(1), Some(3), None],
            &parents,
            &rest,
            &[],
        )
        .unwrap();
        // The hips turn a quarter about up; the arm swings round with them.
        let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut joints = at_rest(&rest, &parents);
        joints[1].rotation = turn;
        let (local, offsets, scales) = map.pose(&joints);
        assert!(local[1].angle_between(turn) < 1e-5);
        assert!(
            local[2].angle_between(Quat::IDENTITY) < 1e-5,
            "the hand doesn't turn on the hips"
        );
        for offset in &offsets {
            assert!(offset.length() < 1e-5, "nothing moves from rest: {offset}");
        }
        assert_eq!(local[3], Quat::IDENTITY, "the prop isn't driven");
        assert_eq!(scales, vec![Vec3::ONE; 4]);
    }

    #[test]
    fn a_left_out_joints_bend_and_scale_carry_to_its_children() {
        let (parents, rest) = source();
        let map = SkeletonMap::new(
            &rig(),
            &[Some(0), Some(1), Some(3), None],
            &parents,
            &rest,
            &[],
        )
        .unwrap();
        // The elbow, which the rig leaves out, bends a quarter turn back about up and
        // doubles in length along x: the hand ends up 2 back from it instead of 1 out.
        let bend = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut joints = at_rest(&rest, &parents);
        joints[2].rotation = bend;
        joints[2].scale = Vec3::new(2.0, 1.0, 1.0);
        joints[3].scale = Vec3::splat(0.5);
        let (local, offsets, scales) = map.pose(&joints);
        assert!(local[2].angle_between(bend) < 1e-5);
        // The hand sits at (1, 1, -2): 2 back from the elbow. At rest it's at (2, 1, 0).
        let hand = Vec3::new(1.0, 0.0, -2.0);
        assert!(
            (offsets[2] - (hand - Vec3::new(2.0, 0.0, 0.0))).length() < 1e-5,
            "{}",
            offsets[2]
        );
        assert_eq!(scales[2], Vec3::splat(0.5), "the hand's own scale");
    }

    #[test]
    fn joints_turned_at_rest_pose_as_turns_from_rest() {
        // The source's hand is turned a quarter about up at rest, as UYA's eyes are. A
        // keyframe that holds it at that turn moves it nothing from rest.
        let (parents, rest) = source();
        let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut turns = vec![Quat::IDENTITY; 5];
        turns[3] = quarter;
        turns[4] = quarter;
        let map = SkeletonMap::new(
            &rig(),
            &[Some(0), Some(1), Some(3), None],
            &parents,
            &rest,
            &turns,
        )
        .unwrap();
        assert_eq!(map.turns()[2], quarter);
        let mut joints = at_rest(&rest, &parents);
        joints[3].rotation = quarter;
        let (local, _, _) = map.pose(&joints);
        assert!(local[2].angle_between(Quat::IDENTITY) < 1e-5);
    }
}
