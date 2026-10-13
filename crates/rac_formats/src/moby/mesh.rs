//! Mesh packets: the vertices and triangle strips one VU1 program run draws, and the mesh
//! a list of them makes (`docs/research/moby-format.md`, "Mesh").
//!
//! A packet's vertices are skinned through 64 VU0 matrix slots and fill a 512-entry
//! vertex cache, and both outlast the packet: a later packet's vertices load matrices an
//! earlier one blended, and its duplicate vertices copy cached ones. So a packet only
//! makes sense in its list, which [`Mesh::assemble`] replays in order.
//!
//! The rules follow ReRAC's moby notes (ISC, `docs/research/prior-art.md`), written from
//! Wrench's behaviour on R&C1; this code is our own, and each rule was checked on UYA's
//! data.

use crate::{FormatError, Image};

/// Which level of detail a packet draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    High,
    Low,
}

/// A VU0 address that means "don't store".
const NO_STORE: u8 = 0xF4;
/// VU0 matrix slots: four quadwords each.
const SLOTS: usize = 64;
/// Entries of the vertex cache, addressed by a vertex's 9-bit ID.
const CACHE: usize = 512;
/// The VU1 program has this many vertices in flight, so each record carries the ID of
/// the vertex this many before it.
const IN_FLIGHT: usize = 7;

/// One vertex record of a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vertex {
    /// As stored: the ID of the vertex [`IN_FLIGHT`] before (low 9 bits), then which VU0
    /// matrix slots and weights it uses, and which joint it moves into a slot.
    pub blend: [u8; 8],
    /// Azimuth and elevation, 256 steps a turn ([`normal`]).
    pub normal: [u8; 2],
    /// In model space at the rest pose, in class units.
    pub position: [i16; 3],
}

/// One packet of a moby's mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub kind: PacketKind,
    /// Where its packet table entry is, for errors.
    pub entry: u32,
    /// The packet's own vertices: two-joint blends, then three-joint blends, then
    /// vertices that use one matrix.
    pub vertices: Vec<Vertex>,
    pub two_way: usize,
    pub three_way: usize,
    /// Each own vertex's ID: the vertex-cache entry it fills.
    pub ids: Vec<u16>,
    /// Joints moved into VU0 matrix slots before the vertices: (joint, VU0 address).
    pub matrix_transfers: Vec<[u8; 2]>,
    /// The vertex-cache entries the packet sends again after its own vertices.
    pub duplicates: Vec<u16>,
    /// The texture coordinates of every vertex sent, own and duplicate (1.0 = 4096).
    pub sts: Vec<[i16; 2]>,
    /// The strip indices after their 4-byte header (see [`Mesh::assemble`]).
    pub strip: Vec<u8>,
    /// The texture each texture switch selects (the GS TEX0 register's low word): an
    /// index into the class's textures, or -1 for none.
    pub textures: Vec<i32>,
    /// The strip index each texture switch brings: the header's third byte, then byte
    /// 0xC of each quadword of the texture data.
    pub switch_indices: Vec<u8>,
}

impl Packet {
    /// Reads the packet whose table entry is at `entry`; its pointers count from `from`
    /// (0 where they are addresses).
    pub(crate) fn read(
        image: &Image,
        entry: u32,
        kind: PacketKind,
        from: u32,
    ) -> Result<Self, FormatError> {
        let invalid = |what: &'static str, detail: String| FormatError::Invalid {
            what,
            address: entry,
            detail,
        };
        let vif = from + image.u32(entry)?;
        let vif_quadwords = image.u16(entry + 4)? as u32;
        let table = from + image.u32(entry + 8)?;
        let table_quadwords = image.u8(entry + 0xC)? as usize;

        let field = |index: u32| -> Result<usize, FormatError> {
            Ok(image.u16(table + index * 2)? as usize)
        };
        let transfers = field(0)?;
        let (two_way, three_way, one) = (field(1)?, field(2)?, field(3)?);
        let own = two_way + three_way + one;
        let duplicates = field(4)?;
        let sent = field(5)?;
        let records_at = field(6)?;
        if own + duplicates != sent {
            return Err(invalid(
                "vertex table",
                format!("{own} vertices and {duplicates} duplicates, {sent} sent"),
            ));
        }
        // Every packet has at least one record after its vertices, for the last IDs.
        let records = table_quadwords.saturating_sub(records_at / 16);
        if records <= own {
            return Err(invalid(
                "vertex table",
                format!("{records} records for {own} vertices"),
            ));
        }

        let mut matrix_transfers = Vec::with_capacity(transfers);
        for index in 0..transfers as u32 {
            let at = table + 0x10 + index * 2;
            matrix_transfers.push([image.u8(at)?, image.u8(at + 1)?]);
        }
        // The duplicates start 8-byte aligned after the transfers.
        let duplicates_at = table + (0x10 + transfers as u32 * 2).next_multiple_of(8);
        let mut duplicate_slots = Vec::with_capacity(duplicates);
        for index in 0..duplicates as u32 {
            duplicate_slots.push(image.u16(duplicates_at + index * 2)? >> 7);
        }

        let record = |index: usize| table + records_at as u32 + index as u32 * 0x10;
        let mut vertices = Vec::with_capacity(own);
        for index in 0..own {
            let at = record(index);
            let mut blend = [0; 8];
            blend.copy_from_slice(image.bytes(at, 8)?);
            vertices.push(Vertex {
                blend,
                normal: [image.u8(at + 8)?, image.u8(at + 9)?],
                position: [
                    image.i16(at + 0xA)?,
                    image.i16(at + 0xC)?,
                    image.i16(at + 0xE)?,
                ],
            });
        }
        // Record i holds the ID of vertex i - 7; the IDs no record holds are up to six
        // more halfwords, in order, in the second half of the last record.
        let first_spare = records.saturating_sub(IN_FLIGHT);
        let mut ids = Vec::with_capacity(own);
        for index in 0..own {
            let holder = index + IN_FLIGHT;
            let id = if holder < records {
                image.u16(record(holder))?
            } else {
                let spare = index - first_spare;
                if spare >= 6 {
                    return Err(invalid(
                        "vertex table",
                        format!("vertex {index}'s ID is past the last record"),
                    ));
                }
                image.u16(record(records - 1) + 4 + spare as u32 * 2)?
            };
            ids.push(id & 0x1FF);
        }

        // Texture coordinates (V2-16, regular packets only), the strips (V4-8), and the
        // texture switches' GS registers (V4-32), if the packet changes texture.
        let unpacks = vif_unpacks(image, vif, vif_quadwords)?;
        let mut unpacks = unpacks.iter().peekable();
        let mut sts = Vec::new();
        if let Some(&&(at, len, format)) = unpacks.peek()
            && format == 0x05
        {
            unpacks.next();
            for index in 0..(len / 4).min(sent) as u32 {
                sts.push([image.i16(at + index * 4)?, image.i16(at + index * 4 + 2)?]);
            }
        }
        if sts.len() < sent {
            return Err(invalid(
                "VIF list",
                format!("{} texture coordinates for {sent} vertices", sts.len()),
            ));
        }
        let Some(&(at, len, _)) = unpacks.next() else {
            return Err(invalid("VIF list", "no strip unpack".into()));
        };
        let indices = image.bytes(at, len)?;
        if indices.len() < 4 {
            return Err(invalid("strip", "shorter than its header".into()));
        }
        let mut textures = Vec::new();
        let mut switch_indices = vec![indices[2]];
        if let Some(&(at, len, _)) = unpacks.next() {
            let blocks = len / 0x40;
            for block in 0..blocks as u32 {
                textures.push(image.u32(at + block * 0x40 + 0x20)? as i32);
            }
            for quadword in 0..blocks as u32 {
                switch_indices.push(image.u8(at + quadword * 0x10 + 0xC)?);
            }
        }

        Ok(Self {
            kind,
            entry,
            vertices,
            two_way,
            three_way,
            ids,
            matrix_transfers,
            duplicates: duplicate_slots,
            sts,
            strip: indices[4..].to_vec(),
            textures,
            switch_indices,
        })
    }

    /// How many vertices the strips can use: the packet's own, then its duplicates.
    pub fn sent(&self) -> usize {
        self.vertices.len() + self.duplicates.len()
    }
}

/// The joints a vertex follows: up to three, with weights out of 256 (unused ones 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Skin {
    pub joints: [u8; 3],
    pub weights: [u16; 3],
}

impl Skin {
    fn one(joint: u8) -> Self {
        Self {
            joints: [joint, 0, 0],
            weights: [256, 0, 0],
        }
    }

    fn single(&self) -> Option<u8> {
        (self.weights[1] == 0 && self.weights[2] == 0).then_some(self.joints[0])
    }
}

/// A vertex of an assembled mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshVertex {
    /// In model space at the rest pose, in class units.
    pub position: [i16; 3],
    /// Azimuth and elevation ([`normal`]).
    pub normal: [u8; 2],
    /// Texture coordinates, 1.0 = 4096.
    pub st: [i16; 2],
    pub skin: Skin,
}

/// A triangle of an assembled mesh. The games draw both sides, so the stored winding
/// says nothing about which way it faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshTriangle {
    pub corners: [u32; 3],
    /// The class texture it's drawn with, or -1 for none.
    pub texture: i32,
}

/// One level of detail of a moby's mesh, with every packet's vertices in order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    pub vertices: Vec<MeshVertex>,
    pub triangles: Vec<MeshTriangle>,
}

impl Mesh {
    /// Assembles one list of packets (a level of detail), replaying how VU0 skins the
    /// vertices and VU1 caches them:
    /// - Matrix transfers move joints into VU0 slots; a vertex may move one more in before
    ///   it loads. Blended vertices mix two or three slots and may store the result.
    /// - Each own vertex fills the vertex cache at its ID; each duplicate copies a cached
    ///   vertex and takes the next texture coordinate.
    /// - Strip indices count from 1. A set high bit means the vertex doesn't complete a
    ///   triangle (the GS's ADC bit). 0 switches texture and takes its index from the
    ///   switch, the last of which is 0 and ends the packet; the last three indices before
    ///   it are never drawn.
    pub fn assemble(packets: &[Packet]) -> Result<Self, FormatError> {
        let mut slots: [Option<Skin>; SLOTS] = [None; SLOTS];
        let mut cache: Vec<Option<MeshVertex>> = vec![None; CACHE];
        let mut mesh = Mesh::default();
        let mut texture = -1;
        for packet in packets {
            let invalid = |detail: String| FormatError::Invalid {
                what: "moby packet",
                address: packet.entry,
                detail,
            };
            let slot = |address: u8| -> Result<usize, FormatError> {
                if !address.is_multiple_of(4) || address >= NO_STORE {
                    return Err(invalid(format!("VU0 address {address:#x}")));
                }
                Ok(address as usize / 4)
            };
            let store = |slots: &mut [Option<Skin>; SLOTS], address: u8, skin: Skin| {
                if address != NO_STORE {
                    slots[slot(address)?] = Some(skin);
                }
                Ok::<(), FormatError>(())
            };
            let load = |slots: &[Option<Skin>; SLOTS], address: u8| -> Result<Skin, FormatError> {
                slots[slot(address)?]
                    .ok_or_else(|| invalid(format!("VU0 address {address:#x} loaded unset")))
            };
            let joint = |slots: &[Option<Skin>; SLOTS], address: u8| -> Result<u8, FormatError> {
                load(slots, address)?
                    .single()
                    .ok_or_else(|| invalid("a blend of blends".into()))
            };

            for &[joint, address] in &packet.matrix_transfers {
                store(&mut slots, address, Skin::one(joint))?;
            }
            let base = mesh.vertices.len() as u32;
            for (index, vertex) in packet.vertices.iter().enumerate() {
                let b = vertex.blend;
                let skin = if index < packet.two_way {
                    store(&mut slots, b[6], Skin::one(b[1] >> 1))?;
                    let skin = Skin {
                        joints: [joint(&slots, b[2])?, joint(&slots, b[3])?, 0],
                        weights: [b[4] as u16, b[5] as u16, 0],
                    };
                    store(&mut slots, b[7], skin)?;
                    skin
                } else if index < packet.two_way + packet.three_way {
                    let skin = Skin {
                        joints: [
                            joint(&slots, b[2])?,
                            joint(&slots, b[3])?,
                            joint(&slots, b[1] & 0xFE)?,
                        ],
                        weights: [b[4] as u16, b[5] as u16, b[6] as u16],
                    };
                    store(&mut slots, b[7], skin)?;
                    skin
                } else {
                    store(&mut slots, b[3], Skin::one(b[1] >> 1))?;
                    load(&slots, b[2])?
                };
                if skin.weights.iter().sum::<u16>() != 256 {
                    return Err(invalid(format!("weights {:?}", skin.weights)));
                }
                let made = MeshVertex {
                    position: vertex.position,
                    normal: vertex.normal,
                    st: packet.sts[index],
                    skin,
                };
                cache[packet.ids[index] as usize] = Some(made);
                mesh.vertices.push(made);
            }
            for (index, &entry) in packet.duplicates.iter().enumerate() {
                let cached = cache
                    .get(entry as usize)
                    .copied()
                    .flatten()
                    .ok_or_else(|| invalid(format!("duplicate of empty cache entry {entry}")))?;
                mesh.vertices.push(MeshVertex {
                    st: packet.sts[packet.vertices.len() + index],
                    ..cached
                });
            }

            // The strip, as (vertex, completes a triangle, texture) per index sent.
            let mut sent = Vec::with_capacity(packet.strip.len());
            let mut switches = 0;
            for &stored in &packet.strip {
                let index = if stored == 0 {
                    if let Some(&chosen) = packet.textures.get(switches) {
                        texture = chosen;
                    }
                    let index = packet.switch_indices.get(switches).copied().unwrap_or(0);
                    switches += 1;
                    if index == 0 {
                        break;
                    }
                    index | 0x80
                } else {
                    stored
                };
                sent.push(((index & 0x7F) as usize - 1, index & 0x80 == 0, texture));
            }
            sent.truncate(sent.len().saturating_sub(3));
            let mut window: [Option<usize>; 3] = [None; 3];
            for (vertex, kick, texture) in sent {
                window = [window[1], window[2], Some(vertex)];
                let [Some(a), Some(b), Some(c)] = window else {
                    continue;
                };
                if !kick || a == b || b == c || a == c {
                    continue;
                }
                if a.max(b).max(c) >= packet.sent() {
                    return Err(invalid(format!(
                        "strip index past the {} vertices sent",
                        packet.sent()
                    )));
                }
                mesh.triangles.push(MeshTriangle {
                    corners: [a, b, c].map(|v| base + v as u32),
                    texture,
                });
            }
        }
        Ok(mesh)
    }
}

/// A packed normal as a unit vector in model space: azimuth about z from x, then
/// elevation towards z, each 256 steps a turn.
pub fn normal(packed: [u8; 2]) -> [f32; 3] {
    let turn = std::f32::consts::TAU / 256.0;
    let (azimuth, elevation) = (packed[0] as f32 * turn, packed[1] as f32 * turn);
    [
        azimuth.cos() * elevation.cos(),
        azimuth.sin() * elevation.cos(),
        elevation.sin(),
    ]
}

/// Each UNPACK in a VIF list: (address of its data, length in bytes, format), where the
/// format is the command's low four bits (vn and vl).
fn vif_unpacks(
    image: &Image,
    start: u32,
    quadwords: u32,
) -> Result<Vec<(u32, usize, u32)>, FormatError> {
    let end = start + quadwords * 16;
    let mut at = start;
    let mut unpacks = Vec::new();
    while at < end {
        let code = image.u32(at)?;
        at += 4;
        let command = (code >> 24) & 0x7F;
        let num = (code >> 16) & 0xFF;
        let immediate = code & 0xFFFF;
        let skip = match command {
            0x60..=0x7F => {
                // vn + 1 components of 32, 16 or 8 bits each (V4-5 packs 16 bits).
                let components = ((command >> 2) & 3) + 1;
                let bits = match command & 3 {
                    0 => 32,
                    1 => 16,
                    2 => 8,
                    _ => 16 / components,
                };
                let count = if num == 0 { 256 } else { num };
                let len = (bits * components * count).div_ceil(32) * 4;
                unpacks.push((at, len as usize, command & 0x0F));
                len
            }
            0x20 => 4,
            0x30 | 0x31 => 16,
            0x4A => (if num == 0 { 256 } else { num }) * 8,
            0x50 | 0x51 => (if immediate == 0 { 65536 } else { immediate }) * 16,
            _ => 0,
        };
        at += skip;
    }
    Ok(unpacks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby::tests::Writer;

    fn vertex(blend: [u8; 8], x: i16) -> Vertex {
        Vertex {
            blend,
            normal: [0, 0],
            position: [x, 0, 0],
        }
    }

    /// A packet of the given vertices (all one-matrix unless `two_way` says otherwise),
    /// IDs 0, 1, … and the strip.
    fn packet(vertices: Vec<Vertex>, two_way: usize, strip: &[u8]) -> Packet {
        let count = vertices.len();
        Packet {
            kind: PacketKind::High,
            entry: 0,
            ids: (0..count as u16).collect(),
            vertices,
            two_way,
            three_way: 0,
            matrix_transfers: Vec::new(),
            duplicates: Vec::new(),
            sts: (0..count as i16).map(|i| [i, 0]).collect(),
            strip: strip.to_vec(),
            textures: vec![3],
            switch_indices: vec![0x81, 0],
        }
    }

    #[test]
    fn strips_skip_vertices_marked_not_to_draw() {
        // The texture switch brings the first index, 1; then 2, 3 and 4 draw triangles,
        // 0x81 restarts at 1, and the last three before the end aren't drawn.
        let vertices = (0..5)
            .map(|x| vertex([0, 0, 0, 0xF4, 0, 0, 0, 0], x))
            .collect();
        let mut packet = packet(vertices, 0, &[0, 2, 3, 4, 0x81, 0x82, 5, 1, 1, 1, 0]);
        packet.matrix_transfers = vec![[9, 0]];
        let mesh = Mesh::assemble(&[packet]).unwrap();
        let corners: Vec<[u32; 3]> = mesh.triangles.iter().map(|t| t.corners).collect();
        assert_eq!(corners, [[0, 1, 2], [1, 2, 3], [0, 1, 4]]);
        assert!(mesh.triangles.iter().all(|t| t.texture == 3));
        assert!(mesh.vertices.iter().all(|v| v.skin == Skin::one(9)));
    }

    #[test]
    fn vertices_blend_matrices_in_vu0_slots_that_outlast_the_packet() {
        // Vertex 0 moves joint 5 into slot 1 (address 4), blends slots 0 and 1 half and
        // half, and stores the blend in slot 2; vertex 1 loads joint 7 from slot 0.
        let first = packet(
            vec![
                vertex([0, 5 << 1, 0, 4, 128, 128, 4, 8], 0),
                vertex([0, 0, 0, 0xF4, 0, 0, 0, 0], 1),
            ],
            1,
            &[0, 0],
        );
        let mut first = first;
        first.matrix_transfers = vec![[7, 0]];
        first.switch_indices = vec![0];
        // The next packet's vertex loads the blend.
        let mut second = packet(vec![vertex([0, 0, 8, 0xF4, 0, 0, 0, 0], 2)], 0, &[0]);
        second.ids = vec![9];
        second.switch_indices = vec![0];
        let mesh = Mesh::assemble(&[first, second]).unwrap();
        let blend = Skin {
            joints: [7, 5, 0],
            weights: [128, 128, 0],
        };
        assert_eq!(mesh.vertices[0].skin, blend);
        assert_eq!(mesh.vertices[1].skin, Skin::one(7));
        assert_eq!(mesh.vertices[2].skin, blend);
    }

    #[test]
    fn duplicates_copy_cached_vertices_with_their_own_texture_coordinates() {
        let mut first = packet(vec![vertex([0, 0, 0, 0xF4, 0, 0, 0, 0], 42)], 0, &[0]);
        first.matrix_transfers = vec![[1, 0]];
        first.ids = vec![300];
        first.switch_indices = vec![0];
        let mut second = packet(Vec::new(), 0, &[0]);
        second.duplicates = vec![300];
        second.sts = vec![[77, 88]];
        second.switch_indices = vec![0];
        let mesh = Mesh::assemble(&[first, second]).unwrap();
        assert_eq!(mesh.vertices[1].position, [42, 0, 0]);
        assert_eq!(mesh.vertices[1].st, [77, 88]);

        let mut missing = packet(Vec::new(), 0, &[0]);
        missing.duplicates = vec![12];
        missing.sts = vec![[0, 0]];
        assert!(Mesh::assemble(&[missing]).is_err());
    }

    #[test]
    fn loading_an_empty_slot_is_an_error() {
        let bad = packet(vec![vertex([0, 0, 8, 0xF4, 0, 0, 0, 0], 0)], 0, &[0]);
        assert!(Mesh::assemble(&[bad]).is_err());
    }

    #[test]
    fn normals_turn_from_x_towards_y_then_up() {
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
        assert!(close(normal([0, 0]), [1.0, 0.0, 0.0]));
        assert!(close(normal([64, 0]), [0.0, 1.0, 0.0]));
        assert!(close(normal([0, 64]), [0.0, 0.0, 1.0]));
    }

    /// A packet table entry, a vertex table with one transfer, two vertices and one
    /// duplicate, and a VIF list of texture coordinates and strips around other commands.
    fn one_packet() -> (Vec<u8>, u32) {
        let mut w = Writer::default();
        let entry = 0x100;
        // VIF list at 0x200 (6 quadwords), vertex table at 0x300 (5 quadwords).
        w.at(entry)
            .u32(0x200)
            .u16(6)
            .u16(0)
            .u32(0x300)
            .u8(5)
            .u8(0)
            .u8(0)
            .u8(3);
        w.at(0x200);
        // NOP, STMASK and its word, then a V2-16 unpack of three coordinates.
        w.u32(0)
            .u32(0x2000_0000)
            .u32(0)
            .u32(0x6503_0000)
            .u16(1)
            .u16(2)
            .u16(3)
            .u16(4)
            .u16(5)
            .u16(6);
        // The strips: a V4-8 unpack of two words (eight bytes).
        w.u32(0x6E02_0000);
        w.put(&[0, 0, 0x81, 0, 0, 2, 3, 0]);
        w.at(0x260);
        // One transfer, two one-matrix vertices, one duplicate, records at 0x20.
        w.at(0x300)
            .u16(1)
            .u16(0)
            .u16(0)
            .u16(2)
            .u16(1)
            .u16(3)
            .u16(0x20)
            .u16(0);
        w.u8(56)
            .u8(0)
            .u16(0)
            .u16(0)
            .u16(0)
            .u16(5 << 7)
            .u16(0)
            .u16(0)
            .u16(0);
        // Two vertices and the one record after them, whose IDs are in its second half.
        w.put(&[0, 0, 0, 0xF4, 0, 0, 0, 0, 1, 2])
            .u16(10)
            .u16(-20i16 as u16)
            .u16(30);
        w.put(&[0, 0, 0, 0xF4, 0, 0, 0, 0, 3, 4])
            .u16(11)
            .u16(21)
            .u16(31);
        w.put(&[0, 0, 0, 0]).u16(5).u16(6).put(&[0; 8]);
        (w.bytes, entry)
    }

    #[test]
    fn reads_a_packet() {
        let (bytes, entry) = one_packet();
        let packet = Packet::read(&Image::new(&bytes, 0), entry, PacketKind::Low, 0).unwrap();
        assert_eq!(packet.vertices[0].position, [10, -20, 30]);
        assert_eq!(packet.vertices[1].normal, [3, 4]);
        assert_eq!(packet.ids, [5, 6]);
        assert_eq!(packet.matrix_transfers, [[56, 0]]);
        assert_eq!(packet.duplicates, [5]);
        assert_eq!(packet.sts, [[1, 2], [3, 4], [5, 6]]);
        assert_eq!(packet.switch_indices, [0x81]);
        assert_eq!(packet.strip, [0, 2, 3, 0]);

        let mesh = Mesh::assemble(&[packet]).unwrap();
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.vertices[2].position, [10, -20, 30], "the duplicate");
        assert_eq!(mesh.vertices[2].st, [5, 6]);
    }

    #[test]
    fn a_vertex_table_that_doesnt_add_up_is_an_error() {
        let (mut bytes, entry) = one_packet();
        // Four sent, but two own vertices and one duplicate.
        bytes[0x30A] = 4;
        assert!(Packet::read(&Image::new(&bytes, 0), entry, PacketKind::High, 0).is_err());
    }
}
