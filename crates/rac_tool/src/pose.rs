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

/// glTF's axis i is the class's axis `GLTF_AXES[i]` (as [`to_gltf`] maps them).
const GLTF_AXES: [usize; 3] = [1, 2, 0];

/// A joint's rest rotation relative to its parent's, in glTF axes: its node's rotation
/// when no animation plays. Most joints aren't turned at rest, but some are.
pub fn rest_local(class: &MobyClass, joint: usize) -> [f32; 4] {
    let own = class.joints[joint].rest_rotation();
    let local = match class.joints[joint].parent {
        Some(parent) => {
            let [x, y, z, w] = class.joints[parent].rest_rotation();
            multiply([-x, -y, -z, w], own)
        }
        None => own,
    };
    quat_to_gltf(local)
}

/// A joint's inverse bind in glTF axes and world units, as glTF stores a matrix: column
/// by column.
pub fn inverse_bind(class: &MobyClass, joint: usize) -> [f32; 16] {
    let bind = class.joints[joint].inverse_bind;
    let mut out = [0.0; 16];
    for (column, &from_column) in GLTF_AXES.iter().enumerate() {
        for (row, &from_row) in GLTF_AXES.iter().enumerate() {
            out[column * 4 + row] = bind[from_row][from_column];
        }
    }
    for (row, &from_row) in GLTF_AXES.iter().enumerate() {
        out[12 + row] = bind[from_row][3] * class.unit();
    }
    out[15] = 1.0;
    out
}

/// The Hamilton product `a b`: `b`'s rotation, then `a`'s.
fn multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
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

    #[test]
    fn a_joint_turned_at_rest_keeps_its_turn_in_gltf() {
        use rac_formats::moby::Joint;
        // Joint 1 sits 1 up, turned a quarter turn about the class's up (z) at rest; its
        // inverse bind turns x to -y and y to x, and takes (0, 0, 1) home.
        let class = MobyClass {
            scale: 1024.0,
            joints: vec![
                Joint {
                    parent: None,
                    offset: [0.0; 3],
                    rest: [0.0; 3],
                    inverse_bind: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                    ],
                },
                Joint {
                    parent: Some(0),
                    offset: [0.0, 0.0, 1.0],
                    rest: [0.0, 0.0, 1.0],
                    inverse_bind: [
                        [0.0, 1.0, 0.0, 0.0],
                        [-1.0, 0.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, -1.0],
                    ],
                },
            ],
            points: Vec::new(),
            sequences: Vec::new(),
            packets: Vec::new(),
        };
        // The class's up is glTF's y.
        let [x, y, z, w] = rest_local(&class, 1);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!(x.abs() < 1e-6 && z.abs() < 1e-6);
        assert!((y - half).abs() < 1e-6 && (w - half).abs() < 1e-6);
        // glTF x (class y) goes to the class's x, which is glTF z; glTF z goes to -x.
        let bind = inverse_bind(&class, 1);
        assert_eq!(bind[0..4], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(bind[8..12], [-1.0, 0.0, 0.0, 0.0]);
        assert_eq!(bind[12..16], [0.0, -1.0, 0.0, 1.0]);
    }
}
