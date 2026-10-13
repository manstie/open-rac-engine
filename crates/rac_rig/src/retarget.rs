//! Lining another skeleton up with the rig's (PLAN.md §5, 2026-10-13): a game's hero
//! skeleton, for its animations and its mesh.
//!
//! Each rig joint names the joint of the other, "source", skeleton that it stands for. The
//! bones line up when each of those descends, in the source, from the joint its rig parent
//! stands for. Then:
//! - a source pose becomes a rig pose: each rig joint takes its source joint's rotation
//!   relative to its rig parent's source joint, and its offset from where the source has it
//!   at rest, so a model of other proportions keeps its own;
//! - a source mesh's skin weights move to the rig's joints: a source joint the rig leaves
//!   out (a finger, an eyelid) gives its weight to its nearest ancestor the rig has.

use bevy::math::{Quat, Vec3};

use crate::model::JointDef;

/// Why a skeleton doesn't line up with the rig's.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LineUpError {
    #[error("{found} rig joints were mapped, but the rig has {expected}")]
    Count { expected: usize, found: usize },
    #[error("the source skeleton has {parents} parents but {rest} rest positions")]
    Source { parents: usize, rest: usize },
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
    #[error("rig joint {joint} has a source joint but its parent {parent} has none")]
    Unmapped {
        joint: &'static str,
        parent: &'static str,
    },
    #[error(
        "rig joint {joint}'s source joint {source_joint} doesn't descend from {parent}'s, {parent_source}"
    )]
    Crossed {
        joint: &'static str,
        source_joint: usize,
        parent: &'static str,
        parent_source: usize,
    },
}

/// How a source skeleton lines up with the rig's.
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonMap {
    rig: Vec<JointDef>,
    /// Per rig joint, the source joint it stands for.
    sources: Vec<Option<usize>>,
    /// Per source joint, the rig joint its vertices follow.
    follows: Vec<Option<usize>>,
    /// Per source joint, its rest position in the hero's space.
    source_rest: Vec<Vec3>,
}

impl SkeletonMap {
    /// Lines the source skeleton up with `rig`. `sources` gives each rig joint's source
    /// joint; `parents` and `rest` describe the source skeleton, parents first, with rest
    /// positions in the hero's space (metres, Bevy axes) and no rotation at rest.
    pub fn new(
        rig: &[JointDef],
        sources: &[Option<usize>],
        parents: &[Option<usize>],
        rest: &[Vec3],
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

        let mut follows: Vec<Option<usize>> = vec![None; parents.len()];
        for (index, joint) in rig.iter().enumerate() {
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
            if let Some(parent) = joint.parent {
                let Some(parent_source) = sources[parent] else {
                    return Err(LineUpError::Unmapped {
                        joint: joint.name,
                        parent: rig[parent].name,
                    });
                };
                if !descends(source, parent_source) {
                    return Err(LineUpError::Crossed {
                        joint: joint.name,
                        source_joint: source,
                        parent: rig[parent].name,
                        parent_source,
                    });
                }
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
            follows,
            source_rest: rest.to_vec(),
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

    /// One source pose, given as each source joint's rotation and position in the hero's
    /// space, as rig joint rotations relative to their parents and offsets from rest (the
    /// form of [`crate::RigKeyframe`]). Joints the source doesn't drive stay at rest.
    pub fn pose(&self, rotations: &[Quat], positions: &[Vec3]) -> (Vec<Quat>, Vec<Vec3>) {
        let mut local = vec![Quat::IDENTITY; self.rig.len()];
        let mut offsets = vec![Vec3::ZERO; self.rig.len()];
        for (index, joint) in self.rig.iter().enumerate() {
            let Some(own) = self.sources[index] else {
                continue;
            };
            let (Some(&rotation), Some(&position)) = (rotations.get(own), positions.get(own))
            else {
                continue;
            };
            let parent = joint.parent.and_then(|p| self.sources[p]);
            let (parent_rotation, parent_position, parent_rest) = match parent {
                Some(p) => (
                    rotations.get(p).copied().unwrap_or_default(),
                    positions.get(p).copied().unwrap_or_default(),
                    self.source_rest[p],
                ),
                None => (Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO),
            };
            let inverse = parent_rotation.inverse();
            local[index] = (inverse * rotation).normalize();
            offsets[index] =
                inverse * (position - parent_position) - (self.source_rest[own] - parent_rest);
        }
        (local, offsets)
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

    #[test]
    fn left_out_joints_follow_their_nearest_mapped_ancestor() {
        let (parents, rest) = source();
        let map =
            SkeletonMap::new(&rig(), &[Some(0), Some(1), Some(3), None], &parents, &rest).unwrap();
        assert_eq!(map.follows(2), Some(1), "the elbow follows the hips");
        assert_eq!(map.follows(4), Some(2), "the finger follows the hand");
        assert_eq!(map.driven(), vec![true, true, true, false]);
        let rest = map.rest();
        assert_eq!(rest[2], Vec3::new(2.0, 1.0, 0.0), "the source's long arm");
        assert!(
            rest[3].distance(Vec3::new(2.2, 1.0, 0.0)) < 1e-6,
            "the prop keeps its offset"
        );
    }

    #[test]
    fn crossed_or_shared_bones_dont_line_up() {
        let (parents, rest) = source();
        let crossed = SkeletonMap::new(&rig(), &[Some(0), Some(3), Some(1), None], &parents, &rest);
        assert!(matches!(crossed, Err(LineUpError::Crossed { .. })));
        let shared = SkeletonMap::new(&rig(), &[Some(0), Some(1), Some(1), None], &parents, &rest);
        assert!(matches!(shared, Err(LineUpError::Shared { .. })));
        let missing = SkeletonMap::new(&rig(), &[Some(0), Some(1), Some(9), None], &parents, &rest);
        assert!(matches!(missing, Err(LineUpError::Missing { .. })));
        let unmapped = SkeletonMap::new(&rig(), &[Some(0), None, Some(3), None], &parents, &rest);
        assert!(matches!(unmapped, Err(LineUpError::Unmapped { .. })));
    }

    #[test]
    fn poses_are_relative_to_the_rig_parents_source_joint() {
        let (parents, rest) = source();
        let map =
            SkeletonMap::new(&rig(), &[Some(0), Some(1), Some(3), None], &parents, &rest).unwrap();
        // The hips turn a quarter about up; the arm swings round with them.
        let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let rotations = vec![Quat::IDENTITY, turn, turn, turn, turn];
        let positions: Vec<Vec3> = rest
            .iter()
            .map(|p| Vec3::Y + turn * (*p - Vec3::Y))
            .collect();
        let (local, offsets) = map.pose(&rotations, &positions);
        assert!(local[1].angle_between(turn) < 1e-5);
        assert!(
            local[2].angle_between(Quat::IDENTITY) < 1e-5,
            "the hand doesn't turn on the hips"
        );
        for offset in &offsets {
            assert!(offset.length() < 1e-5, "nothing moves from rest: {offset}");
        }
        assert_eq!(local[3], Quat::IDENTITY, "the prop isn't driven");
    }
}
