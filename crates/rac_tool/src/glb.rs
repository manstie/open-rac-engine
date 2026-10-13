//! A moby class as a binary glTF: its skeleton, its mesh skinned to it, and every
//! sequence as an animation. For looking at in Blender or any glTF viewer; the file is
//! derived from the disc, so it belongs in `cache/`, never the repository.

use rac_formats::moby::{MobyClass, PacketKind};
use serde_json::{Value, json};

use crate::pose::{Local, local_pose, nearest_bone, to_gltf};

/// Ticks per second: keyframe lengths are in 60 Hz ticks.
const TICK_HZ: f32 = 60.0;

const FLOAT: u32 = 5126;
const UNSIGNED_BYTE: u32 = 5121;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// The glTF's binary chunk and the views and accessors into it.
#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Buffer {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        let mut view = json!({
            "buffer": 0,
            "byteOffset": self.bytes.len(),
            "byteLength": bytes.len(),
        });
        if let Some(target) = target {
            view["target"] = json!(target);
        }
        self.bytes.extend_from_slice(bytes);
        self.views.push(view);
        self.views.len() - 1
    }

    fn accessor(
        &mut self,
        bytes: &[u8],
        target: Option<u32>,
        component: u32,
        count: usize,
        kind: &str,
        bounds: Option<(Vec<f32>, Vec<f32>)>,
    ) -> usize {
        let view = self.view(bytes, target);
        let mut accessor = json!({
            "bufferView": view,
            "componentType": component,
            "count": count,
            "type": kind,
        });
        if let Some((min, max)) = bounds {
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn floats(&mut self, values: &[f32], width: usize, bounded: bool) -> usize {
        let kind = match width {
            1 => "SCALAR",
            3 => "VEC3",
            4 => "VEC4",
            16 => "MAT4",
            _ => unreachable!("no accessor type for {width} floats"),
        };
        let bounds = bounded.then(|| {
            let mut min = vec![f32::INFINITY; width];
            let mut max = vec![f32::NEG_INFINITY; width];
            for item in values.chunks(width) {
                for (c, &value) in item.iter().enumerate() {
                    min[c] = min[c].min(value);
                    max[c] = max[c].max(value);
                }
            }
            (min, max)
        });
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.accessor(&bytes, None, FLOAT, values.len() / width, kind, bounds)
    }
}

/// What [`write`] put in the file.
pub struct Written {
    pub bytes: Vec<u8>,
    pub triangles: usize,
    pub animations: usize,
}

/// Builds the glTF: joint nodes first (named `j000`…), then the mesh's node.
pub fn write(class: &MobyClass, name: &str) -> anyhow::Result<Written> {
    let unit = class.unit();
    let mut buffer = Buffer::default();

    let mut nodes: Vec<Value> = class
        .joints
        .iter()
        .enumerate()
        .map(|(index, joint)| {
            json!({
                "name": format!("j{index:03}"),
                "translation": to_gltf(joint.offset.map(|v| v * unit)),
            })
        })
        .collect();
    for (index, joint) in class.joints.iter().enumerate() {
        if let Some(parent) = joint.parent {
            let children = nodes[parent]
                .as_object_mut()
                .expect("nodes are objects")
                .entry("children")
                .or_insert(json!([]));
            children
                .as_array_mut()
                .expect("children are arrays")
                .push(json!(index));
        }
    }

    // The mesh: the high-detail packets' own vertices, each following the bone it lies
    // nearest.
    let mut positions = Vec::new();
    let mut joints = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for packet in class.packets.iter().filter(|p| p.kind == PacketKind::High) {
        let first = (positions.len() / 3) as u32;
        for vertex in &packet.positions {
            let model = vertex.map(|v| v as f32);
            positions.extend(to_gltf(model.map(|v| v * unit)));
            joints.extend([nearest_bone(class, model) as u8, 0, 0, 0]);
        }
        for triangle in packet.triangles() {
            if triangle.iter().all(|&i| i < packet.positions.len()) {
                indices.extend(triangle.map(|i| first + i as u32));
            }
        }
    }
    let vertex_count = positions.len() / 3;
    let weights: Vec<f32> = (0..vertex_count)
        .flat_map(|_| [1.0, 0.0, 0.0, 0.0])
        .collect();
    let position = buffer.floats(&positions, 3, true);
    let joint_bytes = buffer.accessor(
        &joints,
        Some(ARRAY_BUFFER),
        UNSIGNED_BYTE,
        vertex_count,
        "VEC4",
        None,
    );
    let weight = buffer.floats(&weights, 4, false);
    let index_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let index = buffer.accessor(
        &index_bytes,
        Some(ELEMENT_ARRAY_BUFFER),
        UNSIGNED_INT,
        indices.len(),
        "SCALAR",
        None,
    );

    // The rest pose has no rotations, so each inverse bind only moves the joint home.
    let mut inverse_binds = Vec::with_capacity(class.joints.len() * 16);
    for joint in &class.joints {
        let [x, y, z] = to_gltf(joint.rest.map(|v| -v * unit));
        inverse_binds.extend([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
        ]);
    }
    let inverse_bind = buffer.floats(&inverse_binds, 16, false);

    let animations = animations(class, &mut buffer);
    let animation_count = animations.len();

    let mesh_node = nodes.len();
    nodes.push(json!({ "name": name, "mesh": 0, "skin": 0 }));
    let mut document = json!({
        "asset": { "version": "2.0", "generator": "openrac-tool" },
        "scene": 0,
        "scenes": [{ "nodes": [0, mesh_node] }],
        "nodes": nodes,
        "meshes": [{
            "name": name,
            "primitives": [{
                "attributes": { "POSITION": position, "JOINTS_0": joint_bytes, "WEIGHTS_0": weight },
                "indices": index,
                "material": 0,
            }],
        }],
        // Gold, and no normals: viewers then shade every face flat.
        "materials": [{
            "name": "gold",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 0.766, 0.336, 1.0],
                "metallicFactor": 1.0,
                "roughnessFactor": 0.35,
            },
            "doubleSided": true,
        }],
        "skins": [{
            "joints": (0..class.joints.len()).collect::<Vec<_>>(),
            "inverseBindMatrices": inverse_bind,
            "skeleton": 0,
        }],
        "animations": animations,
        "buffers": [{ "byteLength": buffer.bytes.len() }],
        "bufferViews": buffer.views,
        "accessors": buffer.accessors,
    });
    if indices.is_empty() {
        // glTF has no empty meshes: keep just the skeleton and its animations.
        let object = document.as_object_mut().expect("the document is an object");
        object.remove("meshes");
        object.remove("skins");
        object.remove("materials");
        object["scenes"] = json!([{ "nodes": [0] }]);
        object["nodes"].as_array_mut().expect("nodes").pop();
    }

    Ok(Written {
        bytes: container(&serde_json::to_vec(&document)?, &buffer.bytes),
        triangles: indices.len() / 3,
        animations: animation_count,
    })
}

/// One animation per sequence, each keyframe at the time the game reaches it, and the
/// first again at the end so a looping player wraps smoothly.
fn animations(class: &MobyClass, buffer: &mut Buffer) -> Vec<Value> {
    let mut out = Vec::new();
    for (number, sequence) in class.sequences.iter().enumerate() {
        let Some(sequence) = sequence else { continue };
        if sequence.keyframes.is_empty() {
            continue;
        }
        let mut times = Vec::new();
        let mut poses: Vec<Vec<Local>> = Vec::new();
        let mut time = 0.0;
        for keyframe in &sequence.keyframes {
            times.push(time / TICK_HZ);
            poses.push(local_pose(class, keyframe));
            time += keyframe.ticks();
        }
        let last = sequence.keyframes.last().expect("not empty");
        if last.ticks() > 0.0 && sequence.keyframes.len() > 1 {
            times.push(time / TICK_HZ);
            poses.push(poses[0].clone());
        }
        let input = buffer.floats(&times, 1, true);

        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        let mut channel = |node: usize, path: &str, values: Vec<f32>, width: usize| {
            let output = buffer.floats(&values, width, false);
            samplers.push(json!({ "input": input, "output": output, "interpolation": "LINEAR" }));
            channels.push(json!({
                "sampler": samplers.len() - 1,
                "target": { "node": node, "path": path },
            }));
        };
        for joint in 0..class.joints.len() {
            let rest = to_gltf(class.joints[joint].offset.map(|v| v * class.unit()));
            channel(
                joint,
                "rotation",
                poses.iter().flat_map(|pose| pose[joint].rotation).collect(),
                4,
            );
            if poses.iter().any(|pose| pose[joint].translation.is_some()) {
                channel(
                    joint,
                    "translation",
                    poses
                        .iter()
                        .flat_map(|pose| pose[joint].translation.unwrap_or(rest))
                        .collect(),
                    3,
                );
            }
            if poses.iter().any(|pose| pose[joint].scale.is_some()) {
                channel(
                    joint,
                    "scale",
                    poses
                        .iter()
                        .flat_map(|pose| pose[joint].scale.unwrap_or([1.0; 3]))
                        .collect(),
                    3,
                );
            }
        }
        out.push(json!({
            "name": format!("seq{number:03}"),
            "samplers": samplers,
            "channels": channels,
        }));
    }
    out
}

/// The GLB container: a header, the JSON chunk and the binary chunk, each padded to four
/// bytes.
fn container(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let pad = |len: usize| len.next_multiple_of(4);
    let json_len = pad(json.len());
    let bin_len = pad(bin.len());
    let total = 12 + 8 + json_len + 8 + bin_len;
    let mut out = Vec::with_capacity(total);
    out.extend(b"glTF");
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((json_len as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(json);
    out.resize(20 + json_len, b' ');
    out.extend((bin_len as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    out.resize(total, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_formats::moby::{Joint, Keyframe, Sequence};

    fn class() -> MobyClass {
        let joint = |parent, rest: [f32; 3]| Joint {
            parent,
            offset: rest,
            rest,
        };
        let keyframe = |ticks: f32| Keyframe {
            rate: 1.0 / ticks,
            stamp: 0,
            rotations: vec![[0, 0, 0, 0x7FFF]; 2],
            scales: Vec::new(),
            translations: Vec::new(),
        };
        MobyClass {
            scale: 1024.0,
            joints: vec![joint(None, [0.0; 3]), joint(Some(0), [0.0, 0.0, 1.0])],
            sequences: vec![
                None,
                Some(Sequence {
                    bounds: [0.0; 4],
                    keyframes: vec![keyframe(2.0), keyframe(4.0)],
                }),
            ],
            packets: Vec::new(),
        }
    }

    fn chunks(glb: &[u8]) -> (Value, usize) {
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize,
            glb.len()
        );
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let bin_len = u32::from_le_bytes(glb[20 + json_len..24 + json_len].try_into().unwrap());
        (json, bin_len as usize)
    }

    #[test]
    fn writes_a_skeleton_and_one_animation_per_sequence() {
        let written = write(&class(), "test").unwrap();
        let (json, bin_len) = chunks(&written.bytes);
        assert!(json["buffers"][0]["byteLength"].as_u64().unwrap() as usize <= bin_len);
        assert!(json.get("meshes").is_none(), "no packets, no mesh");
        assert_eq!(json["nodes"][0]["children"], json!([1]));
        assert_eq!(json["nodes"][1]["translation"], json!([0.0, 1.0, 0.0]));
        assert_eq!(json["animations"].as_array().unwrap().len(), 1);
        assert_eq!(json["animations"][0]["name"], "seq001");
        // Two keyframes and the wrap: at 0, 2 and 6 ticks.
        let input = json["animations"][0]["samplers"][0]["input"]
            .as_u64()
            .unwrap() as usize;
        assert_eq!(json["accessors"][input]["count"], 3);
        let end = json["accessors"][input]["max"][0].as_f64().unwrap();
        assert!((end - 0.1).abs() < 1e-6, "{end}");
        assert_eq!(written.animations, 1);
    }
}
