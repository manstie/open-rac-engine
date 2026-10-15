//! UYA's table of contents and level files, as far as the hero's model needs them
//! (PLAN.md P2.3, P3.3; `docs/research/disc-layout.md`, "Level data").
//!
//! Read on UYA NTSC-U, from the disc and the game's level loader (`level_load_core`,
//! 0x428750). GC and DL share the outline but not every size, and aren't checked yet.
//!
//! Everything here works on bytes the caller read from the disc: sector numbers come out,
//! and the caller fetches them (`rac_iso`).

use crate::moby::MobyClass;
use crate::{DiscReader, FormatError, Image, wad};

/// Bytes per sector.
pub(crate) const SECTOR: u64 = 2048;
/// Sectors of the table of contents to read: the globals and every level entry.
const TOC_SECTORS: u64 = 16;
/// Sectors of DL's table of contents to read for its globals, 0xC430 bytes on our discs.
const DL_TOC_SECTORS: u64 = 32;
/// The hero's moby class number.
pub const HERO_CLASS: u32 = 0;

/// The table of contents' first sector on GC, UYA and DL.
pub const TOC_SECTOR: u64 = 1001;
/// UYA's global WAD headers before the level table: armor, audio, bonus, gadget, hud,
/// misc, mpeg, scene and space, 0x7400 bytes in all on our disc. DL has eight too.
const UYA_GLOBALS: usize = 8;
/// Bytes per level table entry: three sector ranges.
const LEVEL_ENTRY: usize = 24;

/// Sectors on the disc: where they start and how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SectorRange {
    pub sector: u32,
    pub count: u32,
}

impl SectorRange {
    fn read(image: &Image, at: u32) -> Result<Self, FormatError> {
        Ok(Self {
            sector: image.u32(at)?,
            count: image.u32(at + 4)?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// One level's files. Each range points at the file's header sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevelFiles {
    pub audio: SectorRange,
    pub level: SectorRange,
    pub scene: SectorRange,
}

/// A global WAD's header in the table of contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalHeader {
    /// The WAD's first sector.
    pub sector: u32,
    /// The whole header, its size and sector first.
    pub bytes: Vec<u8>,
}

/// The table of contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toc {
    pub globals: Vec<GlobalHeader>,
    /// By level number; levels the game doesn't have are empty.
    pub levels: Vec<LevelFiles>,
}

impl Toc {
    /// Reads UYA's table of contents from `bytes`, the sectors from [`TOC_SECTOR`] on.
    pub fn read_uya(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        let (globals, mut at) = read_globals(&image)?;
        let mut levels = Vec::new();
        while (at as usize) + LEVEL_ENTRY <= bytes.len() {
            levels.push(LevelFiles {
                audio: SectorRange::read(&image, at)?,
                level: SectorRange::read(&image, at + 8)?,
                scene: SectorRange::read(&image, at + 16)?,
            });
            at += LEVEL_ENTRY as u32;
        }
        while levels.last().is_some_and(|l| *l == LevelFiles::default()) {
            levels.pop();
        }
        Ok(Self { globals, levels })
    }

    /// Reads UYA's table of contents from the disc.
    pub fn read_uya_disc(disc: &mut impl DiscReader) -> Result<Self, FormatError> {
        Self::read_uya(&disc.read(TOC_SECTOR * SECTOR, (TOC_SECTORS * SECTOR) as usize)?)
    }

    /// Reads DL's (and Gladiator's) table of contents from the disc: eight global WAD
    /// headers as on UYA, the fifth of them the skins'. What follows them isn't UYA's level
    /// table and isn't read yet, so `levels` is empty.
    pub fn read_dl_disc(disc: &mut impl DiscReader) -> Result<Self, FormatError> {
        let bytes = disc.read(TOC_SECTOR * SECTOR, (DL_TOC_SECTORS * SECTOR) as usize)?;
        let (globals, _) = read_globals(&Image::new(&bytes, 0))?;
        Ok(Self {
            globals,
            levels: Vec::new(),
        })
    }
}

/// The global WAD headers at the start of a table of contents, each starting with its
/// size and first sector, and where the next thing starts.
fn read_globals(image: &Image) -> Result<(Vec<GlobalHeader>, u32), FormatError> {
    let mut globals = Vec::with_capacity(UYA_GLOBALS);
    let mut at = 0u32;
    for _ in 0..UYA_GLOBALS {
        let size = image.u32(at)?;
        if size < 8 || size % 4 != 0 {
            return Err(FormatError::Invalid {
                what: "global WAD header",
                address: at,
                detail: format!("size {size:#x}"),
            });
        }
        globals.push(GlobalHeader {
            sector: image.u32(at + 4)?,
            bytes: image.bytes(at, size as usize)?.to_vec(),
        });
        at += size;
    }
    Ok((globals, at))
}

/// A level WAD's header (0x60 bytes, its first sector). Ranges are in sectors from the
/// WAD's own first sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelHeader {
    /// The WAD's first sector on the disc.
    pub sector: u32,
    pub id: u32,
    /// The level's data: textures, geometry and the core with every moby class.
    pub data: SectorRange,
    pub sound_bank: SectorRange,
    pub gameplay: SectorRange,
    pub occlusion: SectorRange,
}

impl LevelHeader {
    pub fn read(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        let size = image.u32(0)?;
        if size != 0x60 {
            return Err(FormatError::Invalid {
                what: "level header",
                address: 0,
                detail: format!("size {size:#x}, UYA's is 0x60"),
            });
        }
        Ok(Self {
            sector: image.u32(4)?,
            id: image.u32(8)?,
            data: SectorRange::read(&image, 0x10)?,
            sound_bank: SectorRange::read(&image, 0x18)?,
            gameplay: SectorRange::read(&image, 0x20)?,
            occlusion: SectorRange::read(&image, 0x28)?,
        })
    }

    /// Reads UYA level `level`'s header from the disc.
    pub fn read_uya_disc(disc: &mut impl DiscReader, level: usize) -> Result<Self, FormatError> {
        let toc = Toc::read_uya_disc(disc)?;
        let files = toc
            .levels
            .get(level)
            .filter(|files| !files.level.is_empty())
            .ok_or_else(|| FormatError::Invalid {
                what: "level",
                address: level as u32,
                detail: "not on this disc".into(),
            })?;
        Self::read(&disc.read(files.level.sector as u64 * SECTOR, 0x60)?)
    }
}

/// Reads and unpacks the WAD-compressed file at byte `at` of the disc.
pub(crate) fn read_wad(
    disc: &mut impl DiscReader,
    at: u64,
    what: &'static str,
) -> Result<Vec<u8>, FormatError> {
    let packed_size = wad::wad_size(&disc.read(at, 16)?).ok_or(FormatError::Invalid {
        what,
        address: (at / SECTOR) as u32,
        detail: "not a WAD".into(),
    })?;
    wad::decompress(&disc.read(at, packed_size)?)
}

/// The start of a level's data: where its parts are, in bytes from the data's start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataHeader {
    /// The core's header, stored as is: (offset, size).
    pub core_header: (u32, u32),
    /// The core itself, a WAD that the game unpacks whole.
    pub core: u32,
}

impl DataHeader {
    /// Bytes to read for [`DataHeader::read`].
    pub const SIZE: usize = 0x80;

    pub fn read(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        Ok(Self {
            core_header: (image.u32(0x08)?, image.u32(0x0C)?),
            core: image.u32(0x48)?,
        })
    }
}

/// A moby class in the core: where it is and its class number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassEntry {
    /// Bytes from the core's start.
    pub offset: u32,
    pub class: u32,
}

/// The core's header: tables of what's in the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreHeader {
    pub moby_classes: Vec<ClassEntry>,
    /// Where the hero's sequence table is, in bytes from the header's start.
    hero_sequences: u32,
    bytes: Vec<u8>,
}

impl CoreHeader {
    pub fn read(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        let count = image.u32(0x18)?;
        let table = image.u32(0x1C)?;
        let mut moby_classes = Vec::with_capacity(count.min(1024) as usize);
        for index in 0..count {
            let at = table + index * 0x20;
            moby_classes.push(ClassEntry {
                offset: image.u32(at)?,
                class: image.u32(at + 4)?,
            });
        }
        Ok(Self {
            moby_classes,
            hero_sequences: image.u32(0x78)?,
            bytes: bytes.to_vec(),
        })
    }

    /// The class numbered `class`, if the level has it.
    pub fn moby_class(&self, class: u32) -> Option<ClassEntry> {
        self.moby_classes.iter().copied().find(|c| c.class == class)
    }

    /// The hero's sequences (class 0's), which the core keeps outside his class: one
    /// offset from the core's start per sequence, 0 where there is none. `count` is his
    /// class's sequence count.
    pub fn hero_sequences(&self, count: usize) -> Result<Vec<u32>, FormatError> {
        let image = Image::new(&self.bytes, 0);
        (0..count as u32)
            .map(|i| image.u32(self.hero_sequences + i * 4))
            .collect()
    }
}

/// A level's core, unpacked, and its header.
pub struct Core {
    pub header: CoreHeader,
    /// The core as the game unpacks it; class and sequence offsets count from its start.
    pub bytes: Vec<u8>,
}

impl Core {
    /// Reads and unpacks the core of UYA level `level` from the disc.
    pub fn read_uya(disc: &mut impl DiscReader, level: usize) -> Result<Self, FormatError> {
        let header = LevelHeader::read_uya_disc(disc, level)?;
        let data = (header.sector as u64 + header.data.sector as u64) * SECTOR;
        let parts = DataHeader::read(&disc.read(data, DataHeader::SIZE)?)?;
        let (offset, size) = parts.core_header;
        let core_header = CoreHeader::read(&disc.read(data + offset as u64, size as usize)?)?;
        let bytes = read_wad(disc, data + parts.core as u64, "level core")?;
        Ok(Self {
            header: core_header,
            bytes,
        })
    }

    /// Moby class `class` from the core's class table, with its own mesh and sequences.
    pub fn moby_class(&self, class: u32) -> Result<MobyClass, FormatError> {
        let entry = self
            .header
            .moby_class(class)
            .ok_or_else(|| FormatError::Invalid {
                what: "level core",
                address: class,
                detail: "no such moby class".into(),
            })?;
        MobyClass::read_placed(
            &Image::new(&self.bytes, 0),
            entry.offset,
            crate::moby::Placement::Packed,
        )
    }

    /// The hero's class (class 0) with his skeleton and every sequence.
    pub fn hero(&self) -> Result<MobyClass, FormatError> {
        let entry = self
            .header
            .moby_class(HERO_CLASS)
            .ok_or(FormatError::Invalid {
                what: "level core",
                address: 0,
                detail: "no hero class".into(),
            })?;
        let image = Image::new(&self.bytes, 0);
        let count = image.u8(entry.offset + 0x0C)? as usize;
        let sequences = self.header.hero_sequences(count)?;
        MobyClass::read_hero(&image, entry.offset, &sequences)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby::tests::Writer;

    #[test]
    fn reads_the_level_table_after_eight_globals() {
        let mut w = Writer::default();
        for (i, size) in [0x10u32, 8, 8, 0x0C, 8, 8, 8, 8].into_iter().enumerate() {
            w.u32(size).u32(100 + i as u32);
            w.put(&vec![0; size as usize - 8]);
        }
        // Level 0 empty, level 1 present, then padding.
        w.put(&[0; 24]);
        for value in [1017, 4, 1021, 9439, 1022, 5] {
            w.u32(value);
        }
        w.put(&[0; 48]);
        let toc = Toc::read_uya(&w.bytes).unwrap();
        assert_eq!(toc.globals[3].sector, 103);
        assert_eq!(toc.globals[3].bytes.len(), 0x0C);
        assert_eq!(toc.levels.len(), 2);
        assert_eq!(
            toc.levels[1].level,
            SectorRange {
                sector: 1021,
                count: 9439
            }
        );
        assert!(toc.levels[0].level.is_empty());
    }

    #[test]
    fn a_global_header_of_no_size_is_an_error() {
        assert!(Toc::read_uya(&[0; 64]).is_err());
    }

    #[test]
    fn reads_a_level_header() {
        let mut w = Writer::default();
        for value in [
            0x60, 0x11BC43, 1, 0, 0x314, 0x1F28, 1, 0x313, 0x223C, 0x297, 0x24D3, 0xC,
        ] {
            w.u32(value);
        }
        w.put(&[0; 0x30]);
        let header = LevelHeader::read(&w.bytes).unwrap();
        assert_eq!(header.sector, 0x11BC43);
        assert_eq!(header.id, 1);
        assert_eq!(
            header.data,
            SectorRange {
                sector: 0x314,
                count: 0x1F28
            }
        );
        assert_eq!(header.gameplay.sector, 0x223C);
        w.bytes[0] = 0x68;
        assert!(LevelHeader::read(&w.bytes).is_err());
    }

    #[test]
    fn finds_the_hero_class_and_his_sequences() {
        let mut w = Writer::default();
        w.at(0x18).u32(2).u32(0xC0);
        w.at(0x78).u32(0x100);
        w.at(0xC0).u32(0x5000).u32(0).put(&[0; 24]);
        w.u32(0x6000).u32(7).put(&[0; 24]);
        w.at(0x100).u32(0x9000).u32(0).u32(0x9400);
        let core = CoreHeader::read(&w.bytes).unwrap();
        assert_eq!(
            core.moby_class(0),
            Some(ClassEntry {
                offset: 0x5000,
                class: 0
            })
        );
        assert_eq!(core.moby_class(7).unwrap().offset, 0x6000);
        assert_eq!(core.moby_class(8), None);
        assert_eq!(core.hero_sequences(3).unwrap(), [0x9000, 0, 0x9400]);
    }
}
