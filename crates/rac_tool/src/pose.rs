//! Joint transforms in world units and glTF's axes.

use rac_formats::moby::{Keyframe, MobyClass};

/// Class axes are x forward, y left, z up (`docs/research/moby-format.md`); glTF's are x
/// left, y up, z forward. The mapping is a rotation, so quaternions map the same way.
pub fn to_gltf(v: [f32; 3]) -> [f32; 3] {
    [v[1], v[2], v[0]]
}

pub fn quat_to_gltf(q: [f32; 4]) -> [f32; 4] {
    [q[1], q[2], q[0], q[3]]
}

/// A joint's transform relative to its parent in one keyframe, in glTF axes and world
/// units. Translation and scale are `None` where the keyframe keeps the rest values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Local {
    pub translation: Option<[f32; 3]>,
    pub rotation: [f32; 4],
    pub scale: Option<[f32; 3]>,
}

pub fn local_pose(class: &MobyClass, keyframe: &Keyframe) -> Vec<Local> {
    let unit = class.unit();
    let mut pose: Vec<Local> = (0..class.joints.len())
        .map(|joint| Local {
            translation: None,
            rotation: quat_to_gltf(keyframe.rotation(joint)),
            scale: None,
        })
        .collect();
    for entry in &keyframe.translations {
        if let Some(local) = pose.get_mut(entry.joint as usize) {
            local.translation = Some(to_gltf(entry.offset.map(|v| v as f32 * unit)));
        }
    }
    for entry in &keyframe.scales {
        if let Some(local) = pose.get_mut(entry.joint as usize) {
            local.scale = Some(to_gltf(entry.scale));
        }
    }
    pose
}

fn distance_to_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let length = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if length > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    let d = [p[0] - closest[0], p[1] - closest[1], p[2] - closest[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The joint a vertex at the rest pose (class units) most likely follows: the bone it
/// lies nearest, where a bone runs from a joint to its child and moves with the joint.
/// A stand-in until the mesh's own blending is decoded.
pub fn nearest_bone(class: &MobyClass, vertex: [f32; 3]) -> usize {
    let joints = &class.joints;
    let mut has_child = vec![false; joints.len()];
    let mut best = (f32::INFINITY, 0);
    for joint in joints {
        if let Some(parent) = joint.parent {
            has_child[parent] = true;
            let d = distance_to_segment(vertex, joints[parent].rest, joint.rest);
            if d < best.0 {
                best = (d, parent);
            }
        }
    }
    for (index, joint) in joints.iter().enumerate() {
        if !has_child[index] {
            // Past the end of a chain belongs to its last joint.
            let d = distance_to_segment(vertex, joint.rest, joint.rest);
            if d <= best.0 {
                best = (d, index);
            }
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_formats::moby::Joint;

    fn joint(parent: Option<usize>, rest: [f32; 3]) -> Joint {
        Joint {
            parent,
            offset: rest,
            rest,
        }
    }

    #[test]
    fn axes_turn_forward_left_up_into_gltf() {
        assert_eq!(to_gltf([1.0, 2.0, 3.0]), [2.0, 3.0, 1.0]);
        assert_eq!(quat_to_gltf([0.1, 0.2, 0.3, 0.9]), [0.2, 0.3, 0.1, 0.9]);
    }

    #[test]
    fn vertices_follow_the_bone_they_lie_along() {
        let class = MobyClass {
            scale: 1024.0,
            joints: vec![
                joint(None, [0.0, 0.0, 0.0]),
                joint(Some(0), [0.0, 0.0, 10.0]),
                joint(Some(1), [0.0, 10.0, 10.0]),
            ],
            sequences: Vec::new(),
            packets: Vec::new(),
        };
        assert_eq!(nearest_bone(&class, [1.0, 0.0, 5.0]), 0);
        assert_eq!(nearest_bone(&class, [0.0, 5.0, 11.0]), 1);
        assert_eq!(nearest_bone(&class, [0.0, 12.0, 10.0]), 2);
    }
}
