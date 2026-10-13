//! Mesh packets: the vertices and triangle strips one VU1 program run draws
//! (`docs/research/moby-format.md`, "Mesh").
//!
//! Partly understood: the positions and strips read correctly, but how a packet's
//! duplicate vertices map to earlier ones, and which joints each vertex follows, are still
//! open. Triangles that use a duplicate can't be placed yet.

use crate::{FormatError, Image};

/// Which level of detail a packet draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    High,
    Low,
}

/// One packet of a moby's mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub kind: PacketKind,
    /// The packet's own vertices in model space at the rest pose, in class units. Vertices
    /// sent to VU1 beyond these are duplicates of earlier ones.
    pub positions: Vec<[i16; 3]>,
    /// Each vertex's blending data, as stored (VU0 matrix slots and weights).
    pub blending: Vec<[u8; 8]>,
    /// The joints loaded into VU0 matrix slots first: (joint, slot) as stored.
    pub matrix_transfers: Vec<[u8; 2]>,
    /// One entry per duplicate vertex, as stored.
    pub duplicates: Vec<u16>,
    /// How many vertices the strips can use: the packet's own, then its duplicates.
    pub sent: usize,
    /// The strip indices as stored after the header: from 1, with the high bit set on a
    /// vertex that doesn't complete a triangle, and 0 at the end. The first slot holds 0;
    /// the real first index is in the header.
    pub strip: Vec<u8>,
    /// The strip header's four bytes.
    pub strip_header: [u8; 4],
}

impl Packet {
    /// Reads the packet whose table entry is at `entry`; its pointers count from `from`
    /// (0 where they are addresses).
    pub(super) fn read(
        image: &Image,
        entry: u32,
        kind: PacketKind,
        from: u32,
    ) -> Result<Self, FormatError> {
        let vif = from + image.u32(entry)?;
        let vif_quadwords = image.u16(entry + 4)? as u32;
        let table = from + image.u32(entry + 8)?;

        let field = |index: u32| -> Result<usize, FormatError> {
            Ok(image.u16(table + index * 2)? as usize)
        };
        let transfers = field(0)?;
        let own = field(1)? + field(2)? + field(3)?;
        let duplicates = field(4)?;
        let sent = field(5)?;
        let records = field(6)? as u32;
        if own + duplicates != sent {
            return Err(FormatError::Invalid {
                what: "vertex table",
                address: table,
                detail: format!("{own} vertices and {duplicates} duplicates, {sent} sent"),
            });
        }

        let mut matrix_transfers = Vec::with_capacity(transfers);
        for index in 0..transfers as u32 {
            let at = table + 0x10 + index * 2;
            matrix_transfers.push([image.u8(at)?, image.u8(at + 1)?]);
        }
        let mut duplicate_entries = Vec::with_capacity(duplicates);
        for index in 0..duplicates as u32 {
            duplicate_entries.push(image.u16(table + 0x10 + transfers as u32 * 2 + index * 2)?);
        }
        let mut positions = Vec::with_capacity(own);
        let mut blending = Vec::with_capacity(own);
        for index in 0..own as u32 {
            let at = table + records + index * 0x10;
            let mut blend = [0; 8];
            blend.copy_from_slice(image.bytes(at, 8)?);
            blending.push(blend);
            positions.push([
                image.i16(at + 0xA)?,
                image.i16(at + 0xC)?,
                image.i16(at + 0xE)?,
            ]);
        }

        let unpacks = vif_unpacks(image, vif, vif_quadwords)?;
        let Some(&(at, len)) = unpacks.get(1) else {
            return Err(FormatError::Invalid {
                what: "VIF list",
                address: vif,
                detail: format!("{} unpacks, the strips are the second", unpacks.len()),
            });
        };
        let indices = image.bytes(at, len)?;
        if indices.len() < 4 {
            return Err(FormatError::Invalid {
                what: "strip",
                address: at,
                detail: "shorter than its header".into(),
            });
        }
        let mut strip_header = [0; 4];
        strip_header.copy_from_slice(&indices[..4]);

        Ok(Self {
            kind,
            positions,
            blending,
            matrix_transfers,
            duplicates: duplicate_entries,
            sent,
            strip: indices[4..].to_vec(),
            strip_header,
        })
    }

    /// The strips' triangles, as indices into the vertices sent (the packet's own first,
    /// then its duplicates). The GS draws both sides, so their winding isn't meaningful.
    pub fn triangles(&self) -> Vec<[usize; 3]> {
        let mut triangles = Vec::new();
        let mut window: [Option<usize>; 3] = [None; 3];
        for (slot, &stored) in self.strip.iter().enumerate() {
            let index = if slot == 0 && stored == 0 {
                self.strip_header[2]
            } else {
                stored
            };
            if index == 0 {
                break;
            }
            let vertex = (index & 0x7F) as usize - 1;
            window = [window[1], window[2], Some(vertex)];
            if index & 0x80 != 0 {
                continue;
            }
            if let [Some(a), Some(b), Some(c)] = window
                && a != b
                && b != c
                && a != c
                && a.max(b).max(c) < self.sent
            {
                triangles.push([a, b, c]);
            }
        }
        triangles
    }
}

/// The data of each UNPACK in a VIF list: (address, length in bytes).
fn vif_unpacks(
    image: &Image,
    start: u32,
    quadwords: u32,
) -> Result<Vec<(u32, usize)>, FormatError> {
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
                unpacks.push((at, len as usize));
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

    fn packet_with(strip: &[u8], header: [u8; 4], sent: u16) -> Packet {
        Packet {
            kind: PacketKind::High,
            positions: vec![[0; 3]; sent as usize],
            blending: vec![[0; 8]; sent as usize],
            matrix_transfers: Vec::new(),
            duplicates: Vec::new(),
            sent: sent as usize,
            strip: strip.to_vec(),
            strip_header: header,
        }
    }

    #[test]
    fn strips_skip_vertices_marked_not_to_draw() {
        // The first index (3) is in the header. 0x80 + n restarts the strip at n.
        let packet = packet_with(&[0, 0x82, 0x81, 4, 5, 0x82, 0x83, 1, 0], [0, 0, 3, 0], 5);
        assert_eq!(
            packet.triangles(),
            [[1, 0, 3], [0, 3, 4], [1, 2, 0]],
            "3, 2, 1 isn't drawn; 1, 4 and 4, 5 close triangles; 2, 3 restart"
        );
    }

    #[test]
    fn strips_drop_triangles_beyond_the_vertices_sent() {
        let packet = packet_with(&[0, 0x82, 9, 0], [0, 0, 1, 0], 3);
        assert!(packet.triangles().is_empty());
    }

    /// A packet table entry, a vertex table with one transfer, one vertex and one
    /// duplicate, and a VIF list of two unpacks around other commands.
    fn one_packet() -> (Vec<u8>, u32) {
        let mut w = Writer::default();
        let entry = 0x100;
        w.at(entry).u32(0x200).u16(6).u16(0).u32(0x300).u32(0);
        w.at(0x200);
        // NOP, STMASK and its word, then a V2-16 unpack of two values.
        w.u32(0)
            .u32(0x2000_0000)
            .u32(0)
            .u32(0x7502_0000)
            .u16(1)
            .u16(2)
            .u16(3)
            .u16(4);
        // STROW and four words, then the strips: a V4-8 unpack of two (eight bytes).
        w.u32(0x3000_0000).put(&[0; 16]).u32(0x6E02_0000);
        w.put(&[0xFE, 0, 2, 0, 0, 0x81, 0x83, 0]);
        w.at(0x260);
        w.at(0x300)
            .u16(1)
            .u16(0)
            .u16(0)
            .u16(1)
            .u16(1)
            .u16(2)
            .u16(0x20)
            .u16(0);
        w.u8(56).u8(8).u16(0x0200).put(&[0; 12]);
        w.put(&[0, 0, 8, 0xF4, 0, 0, 0, 0, 1, 2])
            .u16(10)
            .u16(-20i16 as u16)
            .u16(30);
        (w.bytes, entry)
    }

    #[test]
    fn reads_a_packet() {
        let (bytes, entry) = one_packet();
        let packet = Packet::read(&Image::new(&bytes, 0), entry, PacketKind::Low, 0).unwrap();
        assert_eq!(packet.positions, [[10, -20, 30]]);
        assert_eq!(packet.blending[0][2..4], [8, 0xF4]);
        assert_eq!(packet.matrix_transfers, [[56, 8]]);
        assert_eq!(packet.duplicates, [0x0200]);
        assert_eq!(packet.sent, 2);
        assert_eq!(packet.strip_header, [0xFE, 0, 2, 0]);
        assert_eq!(packet.strip, [0, 0x81, 0x83, 0]);
    }

    #[test]
    fn a_vertex_table_that_doesnt_add_up_is_an_error() {
        let (mut bytes, entry) = one_packet();
        // Three sent, but one own vertex and one duplicate.
        bytes[0x30A] = 3;
        assert!(Packet::read(&Image::new(&bytes, 0), entry, PacketKind::High, 0).is_err());
    }
}
