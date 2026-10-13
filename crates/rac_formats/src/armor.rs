//! UYA's armor: the hero's mesh and textures, one set for each armor and multiplayer skin
//! (PLAN.md P3.7; `docs/research/moby-format.md`, "Armor").
//!
//! The hero's class has no mesh of its own. The game reads the armor he wears from the
//! armor WAD, the fifth global header in the table of contents (`armor_load`, 0x422838):
//! its textures first, which go straight to the GS, then its mesh, which it relocates
//! (`armor_relocate`, 0x423F28) and hands to the hero's class (`armor_attach`, 0x424008).
//! The same WAD holds the wrench (entry [`WRENCH`], `wrench_load`, 0x422D70) and Clank's
//! textures (`clank_texture_load`, 0x423238); Clank's mesh is his class's own.

use crate::level::{SECTOR, SectorRange, Toc};
use crate::moby::{Mesh, Packet, PacketKind};
use crate::texture::Texture;
use crate::{DiscReader, FormatError, Image};

/// The armor WAD's header among UYA's globals.
const UYA_ARMOR_GLOBAL: usize = 4;
/// Entries in UYA's armor table. Entries 35 on are whole moby classes rather than meshes,
/// and aren't read yet.
const UYA_ARMORS: usize = 56;
/// The entry the game loads for the wrench.
pub const WRENCH: usize = 31;

/// Where one armor's files are, in sectors from the armor WAD's first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmorEntry {
    pub mesh: SectorRange,
    pub textures: SectorRange,
}

/// The armor WAD's table: its first sector, then one entry per armor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmorTable {
    pub sector: u32,
    pub entries: Vec<ArmorEntry>,
    /// Clank's textures, each a single image: 0 is the one a level starts with.
    pub clank_textures: Vec<SectorRange>,
}

impl ArmorTable {
    /// The table in UYA's table of contents: after the header's size and sector, four
    /// words per armor (mesh sector and count, textures sector and count), then a sector
    /// and count per Clank texture.
    pub fn read_uya(toc: &Toc) -> Result<Self, FormatError> {
        let global = toc
            .globals
            .get(UYA_ARMOR_GLOBAL)
            .ok_or(FormatError::Invalid {
                what: "table of contents",
                address: 0,
                detail: "no armor WAD".into(),
            })?;
        let image = Image::new(&global.bytes, 0);
        let mut entries = Vec::with_capacity(UYA_ARMORS);
        for index in 0..UYA_ARMORS as u32 {
            let at = 8 + index * 16;
            entries.push(ArmorEntry {
                mesh: SectorRange {
                    sector: image.u32(at)?,
                    count: image.u32(at + 4)?,
                },
                textures: SectorRange {
                    sector: image.u32(at + 8)?,
                    count: image.u32(at + 12)?,
                },
            });
        }
        let mut clank_textures = Vec::new();
        let mut at = 8 + UYA_ARMORS as u32 * 16;
        while at as usize + 8 <= global.bytes.len() {
            clank_textures.push(SectorRange {
                sector: image.u32(at)?,
                count: image.u32(at + 4)?,
            });
            at += 8;
        }
        Ok(Self {
            sector: global.sector,
            entries,
            clank_textures,
        })
    }

    /// Clank's texture `index` from the disc.
    pub fn read_clank_texture(
        &self,
        disc: &mut impl DiscReader,
        index: usize,
    ) -> Result<Texture, FormatError> {
        let range = self
            .clank_textures
            .get(index)
            .ok_or_else(|| FormatError::Invalid {
                what: "Clank texture",
                address: index as u32,
                detail: format!("the armor WAD lists {}", self.clank_textures.len()),
            })?;
        let bytes = disc.read(
            (self.sector as u64 + range.sector as u64) * SECTOR,
            range.count as usize * SECTOR as usize,
        )?;
        Texture::read_pif(&Image::new(&bytes, 0), 0)
    }
}

/// One armor: the hero's mesh in it, and its textures.
#[derive(Debug, Clone, PartialEq)]
pub struct Armor {
    /// High detail first, then low.
    pub packets: Vec<Packet>,
    /// By the texture numbers the mesh's texture switches use.
    pub textures: Vec<Texture>,
}

impl Armor {
    /// Reads armor `index` (0 is the one a new game starts in, [`WRENCH`] the wrench) from
    /// a UYA disc.
    pub fn read_uya(disc: &mut impl DiscReader, index: usize) -> Result<Self, FormatError> {
        let table = ArmorTable::read_uya(&Toc::read_uya_disc(disc)?)?;
        Self::read_from(disc, &table, index)
    }

    /// Reads armor `index` of `table` from the disc.
    pub fn read_from(
        disc: &mut impl DiscReader,
        table: &ArmorTable,
        index: usize,
    ) -> Result<Self, FormatError> {
        let entry = table
            .entries
            .get(index)
            .ok_or_else(|| FormatError::Invalid {
                what: "armor",
                address: index as u32,
                detail: format!("the armor WAD lists {}", table.entries.len()),
            })?;
        let mut read = |range: SectorRange| {
            disc.read(
                (table.sector as u64 + range.sector as u64) * SECTOR,
                range.count as usize * SECTOR as usize,
            )
        };
        let mesh = read(entry.mesh)?;
        let textures = read(entry.textures)?;
        Self::parse(&mesh, &textures)
    }

    /// Reads an armor from its two files. The mesh starts with the packet counts (high,
    /// low, metal, first metal) and the packet table's offset; its pointers count from
    /// its start. The textures start with their count and each one's offset.
    pub fn parse(mesh: &[u8], textures: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(mesh, 0);
        let (high, low) = (image.u8(0)? as u32, image.u8(1)? as u32);
        let table = image.u32(4)?;
        let mut packets = Vec::with_capacity((high + low) as usize);
        for index in 0..high + low {
            let kind = if index < high {
                PacketKind::High
            } else {
                PacketKind::Low
            };
            packets.push(Packet::read(&image, table + index * 0x10, kind, 0)?);
        }

        let image = Image::new(textures, 0);
        let count = image.u32(0)?;
        if count > 64 {
            return Err(FormatError::Invalid {
                what: "armor textures",
                address: 0,
                detail: format!("{count} textures"),
            });
        }
        let textures = (0..count)
            .map(|index| Texture::read_pif(&image, image.u32(4 + index * 4)?))
            .collect::<Result<_, _>>()?;
        Ok(Self { packets, textures })
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
mod tests {
    use super::*;
    use crate::level::GlobalHeader;

    #[test]
    fn the_table_lists_56_armors_then_clanks_textures() {
        let mut bytes = Vec::new();
        let size = 8 + UYA_ARMORS * 16 + 2 * 8;
        bytes.extend((size as u32).to_le_bytes());
        bytes.extend(1000u32.to_le_bytes());
        for index in 0..UYA_ARMORS as u32 {
            for word in [index * 10, 5, index * 10 + 5, 3] {
                bytes.extend(word.to_le_bytes());
            }
        }
        for word in [900u32, 4, 904, 4] {
            bytes.extend(word.to_le_bytes());
        }
        let mut globals = vec![
            GlobalHeader {
                sector: 0,
                bytes: vec![8, 0, 0, 0, 0, 0, 0, 0],
            };
            UYA_ARMOR_GLOBAL
        ];
        globals.push(GlobalHeader {
            sector: 1000,
            bytes,
        });
        let toc = Toc {
            globals,
            levels: Vec::new(),
        };
        let table = ArmorTable::read_uya(&toc).unwrap();
        assert_eq!(table.sector, 1000);
        assert_eq!(table.entries.len(), UYA_ARMORS);
        assert_eq!(
            table.entries[WRENCH].mesh,
            SectorRange {
                sector: 310,
                count: 5
            }
        );
        assert_eq!(
            table.clank_textures,
            [
                SectorRange {
                    sector: 900,
                    count: 4
                },
                SectorRange {
                    sector: 904,
                    count: 4
                }
            ]
        );
    }
}
