//! Moby classes: the games' animated objects, with their skeleton, animation and mesh
//! (`docs/research/moby-format.md`, PLAN.md P3.7).
//!
//! The layout was read on UYA NTSC-U, in memory and on the disc. DL's classes differ in
//! their skeleton ([`MobyClass::read_dl`]); R&C3 and Gladiator aren't checked yet.

mod anim;
mod mesh;

pub use anim::{JointScale, JointTranslation, Keyframe, Sequence};
pub use mesh::{
    CHROME, GLASS, Mesh, MeshTriangle, MeshVertex, Packet, PacketKind, Skin, Vertex, normal,
};

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
    /// Where the joint sits relative to its parent at rest, along the parent's axes at
    /// rest.
    pub offset: [f32; 3],
    /// Where the joint sits in model space at rest.
    pub rest: [f32; 3],
    /// Takes a point in model space at rest into the joint's own space, as the game skins
    /// with it: three rows of a 3×4 matrix, a rotation then a translation. Most joints
    /// aren't turned at rest, but some are (Clank's face, the wrench), and their vertices
    /// only land right through this.
    pub inverse_bind: [[f32; 4]; 3],
}

impl Joint {
    /// The joint's orientation at rest, as a unit quaternion `[x, y, z, w]` in
    /// [`Keyframe::rotation`]'s convention: it turns the joint's axes into the model's.
    /// For the rare bind that also scales, it's the rotation part, normalised.
    pub fn rest_rotation(&self) -> [f32; 4] {
        // The orientation is the inverse bind's rotation transposed.
        let m = |row: usize, column: usize| self.inverse_bind[column][row];
        let trace = m(0, 0) + m(1, 1) + m(2, 2);
        let q = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            [
                (m(2, 1) - m(1, 2)) / s,
                (m(0, 2) - m(2, 0)) / s,
                (m(1, 0) - m(0, 1)) / s,
                s / 4.0,
            ]
        } else if m(0, 0) > m(1, 1) && m(0, 0) > m(2, 2) {
            let s = (1.0 + m(0, 0) - m(1, 1) - m(2, 2)).sqrt() * 2.0;
            [
                s / 4.0,
                (m(0, 1) + m(1, 0)) / s,
                (m(0, 2) + m(2, 0)) / s,
                (m(2, 1) - m(1, 2)) / s,
            ]
        } else if m(1, 1) > m(2, 2) {
            let s = (1.0 + m(1, 1) - m(0, 0) - m(2, 2)).sqrt() * 2.0;
            [
                (m(0, 1) + m(1, 0)) / s,
                s / 4.0,
                (m(1, 2) + m(2, 1)) / s,
                (m(0, 2) - m(2, 0)) / s,
            ]
        } else {
            let s = (1.0 + m(2, 2) - m(0, 0) - m(1, 1)).sqrt() * 2.0;
            [
                (m(0, 2) + m(2, 0)) / s,
                (m(1, 2) + m(2, 1)) / s,
                s / 4.0,
                (m(1, 0) - m(0, 1)) / s,
            ]
        };
        let length = q.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length.is_finite() && length > 0.0 {
            q.map(|v| v / length)
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    }
}

/// A moby class: the shared data of every moby of one kind.
#[derive(Debug, Clone, PartialEq)]
pub struct MobyClass {
    /// The class's scale; one class unit is `scale / 1024` world units.
    pub scale: f32,
    pub joints: Vec<Joint>,
    /// By sequence number; unused numbers are `None`.
    pub sequences: Vec<Option<Sequence>>,
    /// The mesh's packets: high detail, low detail, then metal.
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
        let metal = image.u8(address + 0x06)? as u32;
        let metal_begin = image.u8(address + 0x07)? as u32;
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
            // The record is the inverse bind as VU0 keeps a matrix: four quadwords, each a
            // column. The w lanes, which the game doesn't multiply by, hold the offset and
            // the parent's address again.
            let record = skeleton + index * JOINT_RECORD;
            let columns = [
                image.f32s::<3>(record)?,
                image.f32s::<3>(record + 0x10)?,
                image.f32s::<3>(record + 0x20)?,
                image.f32s::<3>(record + 0x30)?,
            ];
            let inverse_bind: [[f32; 4]; 3] =
                std::array::from_fn(|row| std::array::from_fn(|column| columns[column][row]));
            joints.push(Joint {
                parent,
                offset,
                rest: unbind(inverse_bind).ok_or_else(|| FormatError::Invalid {
                    what: "joint bind matrix",
                    address: record,
                    detail: format!("joint {index}'s doesn't invert"),
                })?,
                inverse_bind,
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
        let packets = if packet_table == 0 {
            Vec::new()
        } else {
            read_packets(image, packet_table, [high, low, metal, metal_begin], from)?
        };

        Ok(Self {
            scale,
            joints,
            sequences,
            packets,
        })
    }

    /// Reads a DL class as stored on the disc, with pointers from its start. Its header
    /// keeps UYA's packet table, counts and scale, but its skeleton differs (`moby-format.md`,
    /// "DL's classes"):
    /// - the skeleton (+0x14) is a 3×4 inverse bind matrix per joint (0x30 bytes, rows of
    ///   rotation then translation), not UYA's 0x40-byte records of VU0 columns;
    /// - the joint offsets (+0x18) end with a byte naming the parent by its number in the
    ///   low seven bits, 0x7F for the root's none; the high bit is a flag we don't use.
    ///
    /// Its sequences aren't read.
    pub fn read_dl(image: &Image, address: u32) -> Result<Self, FormatError> {
        let pointer = |at: u32| -> Result<u32, FormatError> { Ok(address + image.u32(at)?) };
        let packet_table = image.u32(address)?;
        let count = |at: u32| image.u8(address + at).map(u32::from);
        let counts = [count(4)?, count(5)?, count(6)?, count(7)?];
        let joint_count = image.u8(address + 0x08)? as u32;
        let skeleton = pointer(address + 0x14)?;
        let offsets = pointer(address + 0x18)?;
        let scale = image.f32(address + 0x24)?;

        let mut joints = Vec::with_capacity(joint_count as usize);
        for index in 0..joint_count {
            let record = offsets + index * 0x10;
            let parent = match (index, image.u32(record + 0x0C)?) {
                (0, code) if code & 0x7F == 0x7F => None,
                (_, code) if code < 0x100 && code & 0x7F < index => Some((code & 0x7F) as usize),
                (_, code) => {
                    return Err(FormatError::Invalid {
                        what: "joint parent",
                        address: record + 0x0C,
                        detail: format!("{code:#x} for joint {index}"),
                    });
                }
            };
            let bind = skeleton + index * 0x30;
            let rows = [
                image.f32s::<4>(bind)?,
                image.f32s::<4>(bind + 0x10)?,
                image.f32s::<4>(bind + 0x20)?,
            ];
            joints.push(Joint {
                parent,
                offset: image.f32s::<3>(record)?,
                rest: unbind(rows).ok_or_else(|| FormatError::Invalid {
                    what: "joint bind matrix",
                    address: skeleton + index * 0x30,
                    detail: format!("joint {index}'s doesn't invert"),
                })?,
                inverse_bind: rows,
            });
        }
        let packets = if packet_table == 0 {
            Vec::new()
        } else {
            read_packets(image, address + packet_table, counts, address)?
        };
        Ok(Self {
            scale,
            joints,
            sequences: Vec::new(),
            packets,
        })
    }

    /// World units per class unit.
    pub fn unit(&self) -> f32 {
        self.scale / 1024.0
    }

    /// The mesh at one level of detail, or of the metal packets.
    pub fn mesh(&self, kind: PacketKind) -> Result<Mesh, FormatError> {
        mesh_of(&self.packets, kind)
    }
}

/// Where an inverse bind matrix (rows of rotation and translation) puts the joint: the
/// point it takes to the origin.
fn unbind(rows: [[f32; 4]; 3]) -> Option<[f32; 3]> {
    let m = |r: usize, c: usize| rows[r][c] as f64;
    let minor =
        |r0: usize, r1: usize, c0: usize, c1: usize| m(r0, c0) * m(r1, c1) - m(r0, c1) * m(r1, c0);
    let det =
        m(0, 0) * minor(1, 2, 1, 2) - m(0, 1) * minor(1, 2, 0, 2) + m(0, 2) * minor(1, 2, 0, 1);
    if det.abs() < 1e-9 {
        return None;
    }
    // The inverse's rows, by cofactors.
    let inverse = [
        [minor(1, 2, 1, 2), -minor(0, 2, 1, 2), minor(0, 1, 1, 2)],
        [-minor(1, 2, 0, 2), minor(0, 2, 0, 2), -minor(0, 1, 0, 2)],
        [minor(1, 2, 0, 1), -minor(0, 2, 0, 1), minor(0, 1, 0, 1)],
    ];
    let t = [m(0, 3), m(1, 3), m(2, 3)];
    Some(inverse.map(|row| (-(row[0] * t[0] + row[1] * t[1] + row[2] * t[2]) / det) as f32))
}

/// Reads a packet table: `counts` are the high-detail, low-detail and metal packet counts
/// and the metal packets' first entry, as a class header and an armor's mesh file both
/// keep them.
pub(crate) fn read_packets(
    image: &Image,
    table: u32,
    [high, low, metal, metal_begin]: [u32; 4],
    from: u32,
) -> Result<Vec<Packet>, FormatError> {
    let kinds = (0..high)
        .map(|index| (index, PacketKind::High))
        .chain((high..high + low).map(|index| (index, PacketKind::Low)))
        .chain((metal_begin..metal_begin + metal).map(|index| (index, PacketKind::Metal)));
    kinds
        .map(|(index, kind)| Packet::read(image, table + index * 0x10, kind, from))
        .collect()
}

/// The mesh `packets` of one kind make.
pub(crate) fn mesh_of(packets: &[Packet], kind: PacketKind) -> Result<Mesh, FormatError> {
    let packets: Vec<Packet> = packets.iter().filter(|p| p.kind == kind).cloned().collect();
    Mesh::assemble(&packets)
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

    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    /// A UYA skeleton record at the writer's position: the inverse bind's rotation, given
    /// by its columns, and its translation, each a quadword as VU0 keeps a matrix. The w
    /// lanes are left 0; the game keeps the offset and parent there.
    fn record(w: &mut Writer, columns: [[f32; 3]; 3], translation: [f32; 3]) {
        for column in columns.into_iter().chain([translation]) {
            for value in column {
                w.f32(value);
            }
            w.u32(0);
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
            w.at(0x200 + joint as u32 * 0x40);
            record(&mut w, IDENTITY, rest.map(|value| -value));
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
    fn a_joint_turned_at_rest_keeps_its_whole_inverse_bind() {
        // Joint 2 is turned a quarter turn about z at rest, as Clank's eyes and the wrench
        // are. Its offset (5, 0, 2) is along joint 1's axes, which aren't turned, so it sits
        // at (5, 0, 12). Its inverse bind turns model axes into its own: x to -y and y to
        // x, so the columns are (0, -1, 0) and (1, 0, 0), and t = -R·(5, 0, 12).
        let mut turned = Writer::default();
        record(
            &mut turned,
            [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0, 5.0, -12.0],
        );
        let mut w = Writer {
            bytes: three_joints(),
        };
        for (index, word) in turned.bytes.chunks(4).enumerate() {
            let word = u32::from_le_bytes(word.try_into().unwrap());
            w.poke_u32(0x280 + index as u32 * 4, word);
        }
        let class = MobyClass::read(&Image::new(&w.bytes, 0), 0x100).unwrap();
        let joint = &class.joints[2];
        assert_eq!(joint.rest, [5.0, 0.0, 12.0], "not -t, which is (0, -5, 12)");
        assert_eq!(joint.inverse_bind[0], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(joint.inverse_bind[1], [-1.0, 0.0, 0.0, 5.0]);
        // Its orientation turns its x axis to the model's y: +90° about z.
        let [x, y, z, w] = joint.rest_rotation();
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!(x.abs() < 1e-6 && y.abs() < 1e-6, "{x} {y}");
        assert!(
            (z - half).abs() < 1e-6 && (w - half).abs() < 1e-6,
            "{z} {w}"
        );
        assert_eq!(class.joints[1].rest_rotation(), [0.0, 0.0, 0.0, 1.0]);
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
        w.at(class + 0x80);
        record(&mut w, IDENTITY, [0.0; 3]);
        record(&mut w, IDENTITY, [0.0, 0.0, -5.0]);
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

    /// A DL class at 0 (pointers from its start) with three joints: the root, a hip 5
    /// units up, and a joint turned a quarter turn about z at (2, 0, 5).
    fn dl_class() -> Vec<u8> {
        let mut w = Writer::default();
        w.u32(0).u8(0).u8(0).u8(0).u8(0).u8(3).put(&[0; 0xB]);
        w.u32(0x100).u32(0x200).put(&[0; 8]).f32(1024.0);
        w.at(0x100);
        // Inverse binds: identity, a translation, and a rotation then a translation. The
        // turned joint's inverse takes (2, 0, 5) to the origin: R⁻¹ = [[0,1,0],[-1,0,0],
        // [0,0,1]], so t = -R⁻¹·(2, 0, 5) = (0, 2, -5).
        let binds: [[[f32; 4]; 3]; 3] = [
            [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]],
            [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., -5.]],
            [[0., 1., 0., 0.], [-1., 0., 0., 2.], [0., 0., 1., -5.]],
        ];
        for row in binds.iter().flatten().flatten() {
            w.f32(*row);
        }
        w.at(0x200);
        // Parents as the skins name them (the flag set) and as other classes may (unset).
        for (offset, parent) in [
            ([0., 0., 0.], 0xFF),
            ([0., 0., 5.], 0x80),
            ([2., 0., 0.], 0x01),
        ] {
            for value in offset {
                w.f32(value);
            }
            w.u32(parent);
        }
        w.bytes
    }

    #[test]
    fn reads_a_dl_skeleton_from_its_inverse_binds() {
        let bytes = dl_class();
        let class = MobyClass::read_dl(&Image::new(&bytes, 0), 0).unwrap();
        assert_eq!(class.unit(), 1.0);
        let parents: Vec<Option<usize>> = class.joints.iter().map(|j| j.parent).collect();
        assert_eq!(parents, [None, Some(0), Some(1)]);
        assert_eq!(class.joints[1].rest, [0.0, 0.0, 5.0]);
        let turned = class.joints[2].rest;
        assert!(
            (turned[0] - 2.0).abs() < 1e-6 && turned[1].abs() < 1e-6,
            "{turned:?}"
        );
        assert!((turned[2] - 5.0).abs() < 1e-6);
        assert!(class.packets.is_empty() && class.sequences.is_empty());
    }

    #[test]
    fn a_dl_parent_after_its_child_is_an_error() {
        let mut w = Writer { bytes: dl_class() };
        // Joint 1's parent, made joint 1 itself.
        w.poke_u32(0x21C, 0x81);
        assert!(MobyClass::read_dl(&Image::new(&w.bytes, 0), 0).is_err());
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
