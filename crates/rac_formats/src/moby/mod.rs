//! Moby classes: the games' animated objects, with their skeleton, animation and mesh
//! (`docs/research/moby-format.md`, PLAN.md P3.7).
//!
//! The layout was read on UYA NTSC-U, in memory and on the disc. R&C3, DL and Gladiator
//! are expected to match, which isn't checked yet.

mod anim;
mod mesh;

pub use anim::{JointScale, JointTranslation, Keyframe, Sequence};
pub use mesh::{Mesh, MeshTriangle, MeshVertex, Packet, PacketKind, Skin, Vertex, normal};

use crate::{FormatError, Image};

/// The scratchpad address of joint 0's record while a pose is built; the joint offsets
/// name a joint's parent by its record's address.
const JOINT_RECORDS: u32 = 0x7000_0000;
/// Bytes per joint record in the scratchpad and in the skeleton.
const JOINT_RECORD: u32 = 0x40;

/// One joint of a class's skeleton, in class units (see [`MobyClass::unit`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    /// Joint 0 is the root and has none; every other joint's parent comes before it.
    pub parent: Option<usize>,
    /// Where the joint sits relative to its parent at rest. The rest pose has no
    /// rotations, so this is also the offset in model space.
    pub offset: [f32; 3],
    /// Where the joint sits in model space at rest.
    pub rest: [f32; 3],
}

/// A moby class: the shared data of every moby of one kind.
#[derive(Debug, Clone, PartialEq)]
pub struct MobyClass {
    /// The class's scale; one class unit is `scale / 1024` world units.
    pub scale: f32,
    pub joints: Vec<Joint>,
    /// By sequence number; unused numbers are `None`.
    pub sequences: Vec<Option<Sequence>>,
    /// The mesh's packets, high detail first and then low.
    pub packets: Vec<Packet>,
}

/// How a class's pointers are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// As the game leaves it in memory: pointers are addresses.
    Loaded,
    /// As stored on the disc: pointers count from the class's start, and so do its
    /// sequences' keyframe pointers (`moby_class_relocate`, 0x424048).
    Packed,
}

impl MobyClass {
    /// Reads the class whose header is at `address`, as loaded in memory.
    pub fn read(image: &Image, address: u32) -> Result<Self, FormatError> {
        Self::read_placed(image, address, Placement::Loaded)
    }

    /// Reads the class whose header is at `address`.
    pub fn read_placed(
        image: &Image,
        address: u32,
        placement: Placement,
    ) -> Result<Self, FormatError> {
        Self::read_with(image, address, placement, None)
    }

    /// Reads the hero's class as stored on the disc. The core keeps his sequences outside
    /// his class (its sequence table is empty there), and lists them in its header
    /// (`level::CoreHeader::hero_sequences`): `sequences` are their addresses in `image`,
    /// 0 for none. Their keyframe pointers count from each sequence's own start
    /// (`hero_sequence_relocate`, 0x423DB8).
    pub fn read_hero(image: &Image, address: u32, sequences: &[u32]) -> Result<Self, FormatError> {
        Self::read_with(image, address, Placement::Packed, Some(sequences))
    }

    fn read_with(
        image: &Image,
        address: u32,
        placement: Placement,
        external: Option<&[u32]>,
    ) -> Result<Self, FormatError> {
        let from = match placement {
            Placement::Loaded => 0,
            Placement::Packed => address,
        };
        let pointer = |at: u32| -> Result<u32, FormatError> {
            Ok(match image.u32(at)? {
                0 => 0,
                value => from + value,
            })
        };
        let packet_table = pointer(address)?;
        let high = image.u8(address + 0x04)? as u32;
        let low = image.u8(address + 0x05)? as u32;
        let joint_count = image.u8(address + 0x08)? as usize;
        let sequence_count = image.u8(address + 0x0C)? as u32;
        let skeleton = pointer(address + 0x14)?;
        let offsets = pointer(address + 0x18)?;
        let scale = image.f32(address + 0x24)?;

        let mut joints = Vec::with_capacity(joint_count);
        for index in 0..joint_count as u32 {
            let record = offsets + index * 0x10;
            let offset = image.f32s::<3>(record)?;
            let parent = match (index, image.u32(record + 0x0C)?) {
                (0, _) => None,
                (_, at) => {
                    let parent = at.wrapping_sub(JOINT_RECORDS) / JOINT_RECORD;
                    if at < JOINT_RECORDS || at % JOINT_RECORD != 0 || parent >= index {
                        return Err(FormatError::Invalid {
                            what: "joint parent",
                            address: record + 0x0C,
                            detail: format!("{at:#x} for joint {index}"),
                        });
                    }
                    Some(parent as usize)
                }
            };
            let inverse = image.f32s::<3>(skeleton + index * JOINT_RECORD + 0x30)?;
            joints.push(Joint {
                parent,
                offset,
                rest: inverse.map(|value| -value),
            });
        }

        let mut sequences = Vec::with_capacity(sequence_count as usize);
        for index in 0..sequence_count {
            let (at, frames_from) = match external {
                Some(list) => {
                    let at = list.get(index as usize).copied().unwrap_or(0);
                    (at, at)
                }
                None => (pointer(address + 0x48 + index * 4)?, from),
            };
            sequences.push(if at == 0 {
                None
            } else {
                Some(Sequence::read(image, at, joint_count, frames_from)?)
            });
        }

        // On the disc the hero has no mesh of his own: it comes from the armor WAD.
        let packet_count = if packet_table == 0 { 0 } else { high + low };
        let mut packets = Vec::with_capacity(packet_count as usize);
        for index in 0..packet_count {
            let kind = if index < high {
                PacketKind::High
            } else {
                PacketKind::Low
            };
            packets.push(Packet::read(
                image,
                packet_table + index * 0x10,
                kind,
                from,
            )?);
        }

        Ok(Self {
            scale,
            joints,
            sequences,
            packets,
        })
    }

    /// World units per class unit.
    pub fn unit(&self) -> f32 {
        self.scale / 1024.0
    }

    /// The mesh at one level of detail.
    pub fn mesh(&self, kind: PacketKind) -> Result<Mesh, FormatError> {
        let packets: Vec<Packet> = self
            .packets
            .iter()
            .filter(|p| p.kind == kind)
            .cloned()
            .collect();
        Mesh::assemble(&packets)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds data at an address, the way a class sits in memory.
    #[derive(Default)]
    pub(crate) struct Writer {
        pub bytes: Vec<u8>,
    }

    impl Writer {
        /// Moves forwards to `address`, padding with zeros.
        pub fn at(&mut self, address: u32) -> &mut Self {
            let address = address as usize;
            assert!(
                address >= self.bytes.len(),
                "the writer only moves forwards"
            );
            self.bytes.resize(address, 0);
            self
        }

        pub fn here(&self) -> u32 {
            self.bytes.len() as u32
        }

        pub fn put(&mut self, bytes: &[u8]) -> &mut Self {
            self.bytes.extend_from_slice(bytes);
            self
        }

        pub fn u8(&mut self, value: u8) -> &mut Self {
            self.put(&[value])
        }

        pub fn u16(&mut self, value: u16) -> &mut Self {
            self.put(&value.to_le_bytes())
        }

        pub fn u32(&mut self, value: u32) -> &mut Self {
            self.put(&value.to_le_bytes())
        }

        pub fn f32(&mut self, value: f32) -> &mut Self {
            self.put(&value.to_le_bytes())
        }

        pub fn poke_u32(&mut self, address: u32, value: u32) {
            let at = address as usize;
            self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    /// A class with three joints in a chain and no sequences or packets.
    fn three_joints() -> Vec<u8> {
        let mut w = Writer::default();
        w.at(0x100).u32(0).u8(0).u8(0).u16(0).u8(3).put(&[0; 3]);
        w.u8(0)
            .put(&[0; 3])
            .u32(0)
            .u32(0x200)
            .u32(0x400)
            .put(&[0; 8]);
        w.f32(0.25);
        let offsets: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [0.0, 0.0, 10.0], [5.0, 0.0, 2.0]];
        let rests: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [0.0, 0.0, 10.0], [5.0, 0.0, 12.0]];
        for (joint, rest) in rests.iter().enumerate() {
            w.at(0x200 + joint as u32 * 0x40 + 0x30);
            for &value in rest {
                w.f32(-value);
            }
            w.u32(0);
        }
        w.at(0x400);
        for (joint, offset) in offsets.iter().enumerate() {
            for &value in offset {
                w.f32(value);
            }
            let parent = joint.saturating_sub(1) as u32;
            w.u32(if joint == 0 {
                0
            } else {
                0x7000_0000 + parent * 0x40
            });
        }
        w.bytes
    }

    #[test]
    fn reads_the_skeleton() {
        let bytes = three_joints();
        let class = MobyClass::read(&Image::new(&bytes, 0), 0x100).unwrap();
        assert_eq!(class.unit(), 0.25 / 1024.0);
        assert_eq!(class.joints.len(), 3);
        assert_eq!(class.joints[0].parent, None);
        assert_eq!(class.joints[2].parent, Some(1));
        assert_eq!(class.joints[2].offset, [5.0, 0.0, 2.0]);
        assert_eq!(class.joints[2].rest, [5.0, 0.0, 12.0]);
        assert!(class.sequences.is_empty() && class.packets.is_empty());
    }

    #[test]
    fn a_parent_after_its_child_is_an_error() {
        let mut w = Writer {
            bytes: three_joints(),
        };
        // Joint 1's parent, made joint 2.
        w.poke_u32(0x41C, 0x7000_0080);
        let error = MobyClass::read(&Image::new(&w.bytes, 0), 0x100).unwrap_err();
        assert!(matches!(
            error,
            FormatError::Invalid {
                what: "joint parent",
                ..
            }
        ));
    }

    /// A packed class (pointers from its start) with two joints, and one compressed
    /// sequence kept outside it, as the hero's are on the disc.
    fn packed_hero() -> (Vec<u8>, u32, u32) {
        let mut w = Writer::default();
        let class = 0x100;
        w.at(class).u32(0).u8(0).u8(0).u16(0).u8(2).put(&[0; 3]);
        w.u8(1)
            .put(&[0; 3])
            .u32(0)
            .u32(0x80)
            .u32(0x100)
            .put(&[0; 8]);
        w.f32(1024.0);
        w.at(class + 0x80 + 0x40 + 0x30).f32(0.0).f32(0.0).f32(-5.0);
        w.at(class + 0x100)
            .put(&[0; 16])
            .f32(0.0)
            .f32(0.0)
            .f32(5.0)
            .u32(0x7000_0000);
        // The sequence: one keyframe, its pointer relative to the sequence.
        let sequence = 0x400;
        w.at(sequence)
            .put(&[0; 16])
            .u8(1)
            .u8(0xFF)
            .u8(0)
            .u8(0)
            .u32(0)
            .u32(0);
        w.u32(0xF000_0000 | 0x40);
        // Base block: header (all data at the frame's start), two joints' bases, no
        // scales or translations.
        w.u32(0).put(&[0; 12]).u8(0).u8(0);
        w.at(sequence + 0x40).u16(6).u16(16).u8(0);
        (w.bytes, class, sequence)
    }

    #[test]
    fn reads_the_heros_class_as_packed_on_the_disc() {
        let (bytes, class, sequence) = packed_hero();
        let image = Image::new(&bytes, 0);
        let hero = MobyClass::read_hero(&image, class, &[sequence]).unwrap();
        assert_eq!(hero.joints[1].rest, [0.0, 0.0, 5.0]);
        assert_eq!(hero.joints[1].parent, Some(0));
        assert!(hero.packets.is_empty());
        let keyframe = &hero.sequences[0].as_ref().unwrap().keyframes[0];
        assert_eq!((keyframe.ticks(), keyframe.stamp), (6.0, 16));
        // Read as if loaded, the relative pointers point nowhere sensible.
        assert!(MobyClass::read(&image, class).is_err());
    }

    #[test]
    fn truncated_data_is_an_error() {
        let bytes = three_joints();
        let image = Image::new(&bytes[..0x410], 0);
        assert!(matches!(
            MobyClass::read(&image, 0x100),
            Err(FormatError::OutOfBounds { .. })
        ));
    }
}
