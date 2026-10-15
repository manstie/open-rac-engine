//! UYA's armor: the hero's mesh and textures, one set for each armor and multiplayer skin
//! (PLAN.md P3.7; `docs/research/moby-format.md`, "Armor").
//!
//! The hero's class has no mesh of its own. The game reads the armor he wears from the
//! armor WAD, the fifth global header in the table of contents (`armor_load`, 0x422838):
//! its textures first, which go straight to the GS, then its mesh, which it relocates
//! (`armor_relocate`, 0x423F28) and hands to the hero's class (`armor_attach`, 0x424008).
//! The same WAD holds the wrench (entry [`WRENCH`], `wrench_load`, 0x422D70) and Clank's
//! textures (`clank_texture_load`, 0x423238); Clank's mesh is his class's own.
//!
//! DL keeps its skins in the same place, the fifth global header, but each is a whole
//! moby class with its skeleton ([`SkinTable`], [`Skin`]).

use crate::level::{SECTOR, SectorRange, Toc};
use crate::moby::{Mesh, MobyClass, Packet, PacketKind, mesh_of, read_packets};
use crate::texture::Texture;
use crate::{DiscReader, FormatError, Image};

/// The armor WAD's header among UYA's globals, and the skins' among DL's.
const UYA_ARMOR_GLOBAL: usize = 4;
const DL_SKIN_GLOBAL: usize = 4;
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
    /// High detail, low detail, then metal.
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
    /// low, metal) and the metal packets' first entry, and the packet table's offset; its
    /// pointers count from its start. The textures start with their count and each one's offset.
    pub fn parse(mesh: &[u8], textures: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(mesh, 0);
        let count = |at: u32| image.u8(at).map(u32::from);
        let counts = [count(0)?, count(1)?, count(2)?, count(3)?];
        let packets = read_packets(&image, image.u32(4)?, counts, 0)?;

        let textures = read_textures(textures)?;
        Ok(Self { packets, textures })
    }

    /// The mesh at one level of detail, or of the metal packets.
    pub fn mesh(&self, kind: PacketKind) -> Result<Mesh, FormatError> {
        mesh_of(&self.packets, kind)
    }
}

/// A texture file: a count, an offset per texture, then each as a PIF2 image.
fn read_textures(bytes: &[u8]) -> Result<Vec<Texture>, FormatError> {
    let image = Image::new(bytes, 0);
    let count = image.u32(0)?;
    if count > 64 {
        return Err(FormatError::Invalid {
            what: "armor textures",
            address: 0,
            detail: format!("{count} textures"),
        });
    }
    (0..count)
        .map(|index| Texture::read_pif(&image, image.u32(4 + index * 4)?))
        .collect()
}

/// Where one of DL's skins is: its class, then its textures, in sectors from the skin
/// WAD's first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinEntry {
    pub class: SectorRange,
    pub textures: SectorRange,
}

/// DL's skin table: the WAD's first sector, then one entry per skin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinTable {
    pub sector: u32,
    pub entries: Vec<SkinEntry>,
}

impl SkinTable {
    /// The skins in DL's table of contents: after the header's size and sector, four
    /// words per skin (class sector and count, textures sector and count), up to the
    /// first empty entry. What follows, texture files of their own, isn't read yet.
    pub fn read_dl(toc: &Toc) -> Result<Self, FormatError> {
        let global = toc
            .globals
            .get(DL_SKIN_GLOBAL)
            .ok_or(FormatError::Invalid {
                what: "table of contents",
                address: 0,
                detail: "no skin WAD".into(),
            })?;
        let image = Image::new(&global.bytes, 0);
        let mut entries = Vec::new();
        let mut at = 8;
        while at as usize + 16 <= global.bytes.len() {
            let entry = SkinEntry {
                class: SectorRange {
                    sector: image.u32(at)?,
                    count: image.u32(at + 4)?,
                },
                textures: SectorRange {
                    sector: image.u32(at + 8)?,
                    count: image.u32(at + 12)?,
                },
            };
            if entry.class.is_empty() {
                break;
            }
            entries.push(entry);
            at += 16;
        }
        Ok(Self {
            sector: global.sector,
            entries,
        })
    }
}

/// One of DL's skins: a moby class with its own skeleton and mesh, and its textures.
#[derive(Debug, Clone, PartialEq)]
pub struct Skin {
    pub class: MobyClass,
    /// By the texture numbers the mesh's texture switches use.
    pub textures: Vec<Texture>,
}

impl Skin {
    /// Reads skin `index` of `table` from the disc.
    pub fn read_from(
        disc: &mut impl DiscReader,
        table: &SkinTable,
        index: usize,
    ) -> Result<Self, FormatError> {
        let entry = table
            .entries
            .get(index)
            .ok_or_else(|| FormatError::Invalid {
                what: "skin",
                address: index as u32,
                detail: format!("the skin WAD lists {}", table.entries.len()),
            })?;
        let mut read = |range: SectorRange| {
            disc.read(
                (table.sector as u64 + range.sector as u64) * SECTOR,
                range.count as usize * SECTOR as usize,
            )
        };
        let class = read(entry.class)?;
        let textures = read(entry.textures)?;
        Ok(Self {
            class: MobyClass::read_dl(&Image::new(&class, 0), 0)?,
            textures: read_textures(&textures)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::GlobalHeader;

    #[test]
    fn dls_skin_table_ends_at_its_first_empty_entry() {
        let mut bytes = Vec::new();
        for word in [64u32, 2000, 1, 9, 10, 3, 13, 9, 22, 3, 0, 0, 0, 0, 40, 3] {
            bytes.extend(word.to_le_bytes());
        }
        let mut globals = vec![
            GlobalHeader {
                sector: 0,
                bytes: vec![8, 0, 0, 0, 0, 0, 0, 0],
            };
            DL_SKIN_GLOBAL
        ];
        globals.push(GlobalHeader {
            sector: 2000,
            bytes,
        });
        let toc = Toc {
            globals,
            levels: Vec::new(),
        };
        let table = SkinTable::read_dl(&toc).unwrap();
        assert_eq!(table.sector, 2000);
        assert_eq!(table.entries.len(), 2);
        assert_eq!(
            table.entries[1].textures,
            SectorRange {
                sector: 22,
                count: 3
            }
        );
    }

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
