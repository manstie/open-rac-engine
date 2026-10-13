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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn axes_turn_forward_left_up_into_gltf() {
        assert_eq!(to_gltf([1.0, 2.0, 3.0]), [2.0, 3.0, 1.0]);
        assert_eq!(quat_to_gltf([0.1, 0.2, 0.3, 0.9]), [0.2, 0.3, 0.1, 0.9]);
    }
}
