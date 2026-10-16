//! The table of contents and level files of UYA and DL, as far as the hero's model needs
//! them (PLAN.md P2.3, P3.3; `docs/research/disc-layout.md`, "Level data" and "DL's level
//! data").
//!
//! Read on UYA NTSC-U, from the disc and the game's level loader (`level_load_core`,
//! 0x428750), and on DL NTSC-U from its loader in our DreadZone Station image
//! (`dl_level_read_headers`, 0x159AB0, and `dl_level_load_core`, 0x578940;
//! `re/SCUS-97465/symbols.toml`). Gladiator PAL reads the same way. GC isn't checked yet.
//!
//! Everything here works on bytes the caller read from the disc: sector numbers come out,
//! and the caller fetches them (`rac_iso`).

use crate::moby::{Bangle, MobyClass, Placement};
use crate::texture::{Texture, csm1, unswizzle_psmt8};
use crate::{DiscReader, FormatError, Image, wad};

/// Bytes per sector.
pub(crate) const SECTOR: u64 = 2048;
/// Sectors of the table of contents to read: the globals and every level entry.
const TOC_SECTORS: u64 = 16;
/// Sectors of DL's table of contents, as many as its loader reads: the globals (0xC430
/// bytes on our discs), then the level table.
const DL_TOC_SECTORS: u64 = 0x1A;
/// Bytes of a DL level WAD's header.
const DL_LEVEL_HEADER: usize = 0xC68;
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
        read_toc(bytes)
    }

    /// Reads UYA's table of contents from the disc.
    pub fn read_uya_disc(disc: &mut impl DiscReader) -> Result<Self, FormatError> {
        Self::read_uya(&disc.read(TOC_SECTOR * SECTOR, (TOC_SECTORS * SECTOR) as usize)?)
    }

    /// Reads DL's (and Gladiator's) table of contents from `bytes`: eight global WAD
    /// headers as on UYA, the fifth of them the skins', then UYA's level table. Levels
    /// 20–39 have no audio or scene of their own: the game uses level − 20's
    /// (`dl_level_read_headers`).
    pub fn read_dl(bytes: &[u8]) -> Result<Self, FormatError> {
        read_toc(bytes)
    }

    /// Reads DL's (and Gladiator's) table of contents from the disc.
    pub fn read_dl_disc(disc: &mut impl DiscReader) -> Result<Self, FormatError> {
        Self::read_dl(&disc.read(TOC_SECTOR * SECTOR, (DL_TOC_SECTORS * SECTOR) as usize)?)
    }

    /// Level `level`'s files, if the disc has its level file.
    fn level(&self, level: usize) -> Result<LevelFiles, FormatError> {
        self.levels
            .get(level)
            .copied()
            .filter(|files| !files.level.is_empty())
            .ok_or_else(|| FormatError::Invalid {
                what: "level",
                address: level as u32,
                detail: "not on this disc".into(),
            })
    }
}

/// The global WAD headers, then 24 bytes a level (audio, level, scene) to the end of
/// `bytes`, without the empty entries after the last level.
fn read_toc(bytes: &[u8]) -> Result<Toc, FormatError> {
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
    Ok(Toc { globals, levels })
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
        let files = Toc::read_uya_disc(disc)?.level(level)?;
        Self::read(&disc.read(files.level.sector as u64 * SECTOR, 0x60)?)
    }

    /// Reads a DL level WAD's header (0xC68 bytes): its size, its first sector, the level
    /// number, then from +0x18 the data and the sound bank. DL's loader reads the data at
    /// +0x18 (`dl_level_load_steps`, 0x578530), and when it reloads a level's gameplay
    /// without the rest, unpacks +0x58 into what the data part's gameplay WAD (+0x60)
    /// gives on a full load (`dl_level_load_core`), so `gameplay` is +0x58. The ranges
    /// between and after (the level's chunks, and 128 missions) aren't read, and DL has no
    /// occlusion range in UYA's place, so `occlusion` is empty.
    pub fn read_dl(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        let size = image.u32(0)?;
        if size as usize != DL_LEVEL_HEADER {
            return Err(FormatError::Invalid {
                what: "level header",
                address: 0,
                detail: format!("size {size:#x}, DL's is {DL_LEVEL_HEADER:#x}"),
            });
        }
        Ok(Self {
            sector: image.u32(4)?,
            id: image.u32(8)?,
            data: SectorRange::read(&image, 0x18)?,
            sound_bank: SectorRange::read(&image, 0x20)?,
            gameplay: SectorRange::read(&image, 0x58)?,
            occlusion: SectorRange::default(),
        })
    }

    /// Reads DL (or Gladiator) level `level`'s header from the disc.
    pub fn read_dl_disc(disc: &mut impl DiscReader, level: usize) -> Result<Self, FormatError> {
        let files = Toc::read_dl_disc(disc)?.level(level)?;
        Self::read_dl(&disc.read(files.level.sector as u64 * SECTOR, DL_LEVEL_HEADER)?)
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
    /// DL: the GS RAM image, stored as is: (offset, size). The game sends it to the GS
    /// as the level loads: palettes, small mipmaps, and the textures the GS keeps for the
    /// whole level. Not read on UYA yet.
    pub gs_ram: Option<(u32, u32)>,
}

impl DataHeader {
    /// Bytes to read for [`DataHeader::read`] and [`DataHeader::read_dl`].
    pub const SIZE: usize = 0x80;

    /// UYA's: the core header at +0x08, the core at +0x48.
    pub fn read(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        Ok(Self {
            core_header: (image.u32(0x08)?, image.u32(0x0C)?),
            core: image.u32(0x48)?,
            gs_ram: None,
        })
    }

    /// DL's: UYA's table with one more pair at +0x08 (the level's code), so the core
    /// header is at +0x10, the GS RAM image at +0x18 and the core at +0x50
    /// (`dl_level_load_core`, which copies the table's first 0x70 bytes).
    pub fn read_dl(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        Ok(Self {
            core_header: (image.u32(0x10)?, image.u32(0x14)?),
            core: image.u32(0x50)?,
            gs_ram: Some((image.u32(0x18)?, image.u32(0x1C)?)),
        })
    }
}

/// A moby class in the core: where it is, its class number and its textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassEntry {
    /// Bytes from the core's start.
    pub offset: u32,
    pub class: u32,
    /// DL: 0 when the class's textures are the level's own ([`ClassEntry::textures`]);
    /// 1, 2 or 3 when the game gives it the textures of the skin WAD's first, second or
    /// third texture group instead (`dl_level_load_core`). 0 for every class we've seen
    /// on UYA, where it isn't read in code.
    pub texture_group: u32,
    /// The class's textures, by the numbers its mesh uses: indices into the level's moby
    /// texture table ([`CoreHeader::moby_textures`]), 0xFF after the last.
    pub textures: [u8; 16],
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
            let mut textures = [0; 16];
            textures.copy_from_slice(image.bytes(at + 0x10, 16)?);
            moby_classes.push(ClassEntry {
                offset: image.u32(at)?,
                class: image.u32(at + 4)?,
                texture_group: image.u32(at + 8)?,
                textures,
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

    /// DL: the level's moby texture table (+0x38 count, +0x3C offset), 16 bytes each.
    pub fn moby_textures(&self) -> Result<Vec<TextureEntry>, FormatError> {
        let image = Image::new(&self.bytes, 0);
        let count = image.u32(0x38)?;
        let table = image.u32(0x3C)?;
        if count > 0x100 {
            return Err(FormatError::Invalid {
                what: "moby texture table",
                address: 0x38,
                detail: format!("{count} textures"),
            });
        }
        (0..count)
            .map(|index| TextureEntry::read(&image, table + index * 16))
            .collect()
    }

    /// DL: where in the core the pixels of the textures kept in EE memory start (+0x60).
    pub fn texture_base(&self) -> Result<u32, FormatError> {
        Image::new(&self.bytes, 0).u32(0x60)
    }

    /// DL: where in the GS RAM image the textures the GS keeps start. The GS RAM table
    /// (+0x00 count, +0x04 offset; 16 bytes an entry, its address at +0x8) lists the
    /// palettes and mipmaps the game sends first, then +0x84 entries for those textures,
    /// the first of which starts them (`dl_level_load_core`, `dl_gs_ram_send`).
    pub fn gs_kept_textures(&self) -> Result<u32, FormatError> {
        let image = Image::new(&self.bytes, 0);
        let (count, table) = (image.u32(0x00)?, image.u32(0x04)?);
        if image.u32(0x84)? == 0 {
            return Err(FormatError::Invalid {
                what: "GS RAM table",
                address: 0x84,
                detail: "no textures kept in the GS".into(),
            });
        }
        image.u32(table + count * 16 + 8)
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

/// One of DL's level textures: an 8-bit indexed image and where its pixels and palette
/// are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureEntry {
    /// Where its pixels are: from the core's texture base ([`CoreHeader::texture_base`]),
    /// or for a texture the GS keeps ([`TextureEntry::kept_in_gs`]), from the first of
    /// those in the GS RAM image ([`CoreHeader::gs_kept_textures`]).
    pub data: u32,
    pub width: u16,
    pub height: u16,
    /// 0 for a texture the GS keeps for the whole level; 1 or 3 for one kept in EE memory,
    /// 3 with a small mipmap in the GS RAM image.
    pub kind: u16,
    /// Its palette in the GS RAM image, in 0x100-byte blocks: 256 colours in the GS's
    /// CSM1 order.
    pub palette: u16,
    /// Its small mipmap in the GS RAM image, in 0x100-byte blocks, or -1 for none.
    pub mipmap: i16,
}

impl TextureEntry {
    fn read(image: &Image, at: u32) -> Result<Self, FormatError> {
        Ok(Self {
            data: image.u32(at)?,
            width: image.u16(at + 4)?,
            height: image.u16(at + 6)?,
            kind: image.u16(at + 8)?,
            palette: image.u16(at + 0xA)?,
            mipmap: image.u16(at + 0xC)? as i16,
        })
    }

    /// Whether the GS keeps it for the whole level, its pixels in the GS RAM image.
    pub fn kept_in_gs(&self) -> bool {
        self.kind == 0
    }
}

/// A level's core, unpacked, and its header.
pub struct Core {
    pub header: CoreHeader,
    /// The core as the game unpacks it; class and sequence offsets count from its start.
    pub bytes: Vec<u8>,
    /// DL: the level's GS RAM image ([`DataHeader::gs_ram`]). Empty from
    /// [`Core::read_uya`].
    pub gs_ram: Vec<u8>,
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
            gs_ram: Vec::new(),
        })
    }

    /// Reads and unpacks the core of DL (or Gladiator) level `level` from the disc, with
    /// its GS RAM image. Levels 1–15 are the story's planets, 21–35 the same for two
    /// players, 41–74 the multiplayer maps and 0 the main menu.
    pub fn read_dl(disc: &mut impl DiscReader, level: usize) -> Result<Self, FormatError> {
        let header = LevelHeader::read_dl_disc(disc, level)?;
        let data = (header.sector as u64 + header.data.sector as u64) * SECTOR;
        let parts = DataHeader::read_dl(&disc.read(data, DataHeader::SIZE)?)?;
        let (offset, size) = parts.core_header;
        let core_header = CoreHeader::read(&disc.read(data + offset as u64, size as usize)?)?;
        let gs_ram = match parts.gs_ram {
            Some((offset, size)) => disc.read(data + offset as u64, size as usize)?,
            None => Vec::new(),
        };
        let bytes = read_wad(disc, data + parts.core as u64, "level core")?;
        Ok(Self {
            header: core_header,
            bytes,
            gs_ram,
        })
    }

    fn entry(&self, class: u32) -> Result<ClassEntry, FormatError> {
        self.header
            .moby_class(class)
            .filter(|entry| entry.offset != 0)
            .ok_or_else(|| FormatError::Invalid {
                what: "level core",
                address: class,
                detail: "no such moby class".into(),
            })
    }

    /// DL's moby class `class` from the core's class table, with its mesh: DL's classes
    /// keep a 3×4 inverse bind per joint ([`MobyClass::read_dl`]).
    pub fn dl_moby_class(&self, class: u32) -> Result<MobyClass, FormatError> {
        MobyClass::read_dl(&Image::new(&self.bytes, 0), self.entry(class)?.offset)
    }

    /// The bangles of moby class `class` ([`MobyClass::read_bangles`]).
    pub fn moby_bangles(&self, class: u32) -> Result<Vec<Bangle>, FormatError> {
        let offset = self.entry(class)?.offset;
        MobyClass::read_bangles(&Image::new(&self.bytes, 0), offset, Placement::Packed)
    }

    /// DL: the textures of moby class `class` whose textures are the level's own, by
    /// the numbers its mesh uses. Each texture's pixels are where its table entry says
    /// ([`TextureEntry::data`]); its palette is in the GS RAM image.
    pub fn moby_textures(&self, class: u32) -> Result<Vec<Texture>, FormatError> {
        let entry = self.entry(class)?;
        if entry.texture_group != 0 {
            return Err(FormatError::Invalid {
                what: "moby class textures",
                address: class,
                detail: format!("from the skin WAD's texture group {}", entry.texture_group),
            });
        }
        let table = self.header.moby_textures()?;
        let core = Image::new(&self.bytes, 0);
        let gs_ram = Image::new(&self.gs_ram, 0);
        entry
            .textures
            .iter()
            .take_while(|&&index| index != 0xFF)
            .map(|&index| {
                let texture = table.get(index as usize).ok_or(FormatError::Invalid {
                    what: "moby class textures",
                    address: class,
                    detail: format!("texture {index} past the table"),
                })?;
                let (width, height) = (texture.width as u32, texture.height as u32);
                if !(1..=1024).contains(&width) || !(1..=1024).contains(&height) {
                    return Err(FormatError::Invalid {
                        what: "level texture",
                        address: index as u32,
                        detail: format!("{width} by {height}"),
                    });
                }
                let len = (width * height) as usize;
                // The game sends the GS RAM image to the GS as 8-bit images, but the
                // textures in EE memory as 32-bit ones, so those are stored in the GS's
                // order.
                let pixels = if texture.kept_in_gs() {
                    gs_ram
                        .bytes(self.header.gs_kept_textures()? + texture.data, len)?
                        .to_vec()
                } else {
                    let stored = core.bytes(self.header.texture_base()? + texture.data, len)?;
                    unswizzle_psmt8(stored, width, height)?
                };
                let stored = gs_ram.bytes(texture.palette as u32 * 0x100, 0x400)?;
                let palette = (0..=255u8)
                    .map(|index| {
                        let slot = csm1(index) as usize * 4;
                        [
                            stored[slot],
                            stored[slot + 1],
                            stored[slot + 2],
                            stored[slot + 3],
                        ]
                    })
                    .collect();
                Ok(Texture {
                    width,
                    height,
                    pixels,
                    palette,
                })
            })
            .collect()
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
        assert_eq!(core.moby_class(0).unwrap().offset, 0x5000);
        assert_eq!(core.moby_class(7).unwrap().offset, 0x6000);
        assert_eq!(core.moby_class(8), None);
        assert_eq!(core.hero_sequences(3).unwrap(), [0x9000, 0, 0x9400]);
    }
}

/// Reads the real discs: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
pub(crate) mod disc_tests {
    use super::*;
    use crate::moby::PacketKind;
    use std::fs::File;
    use std::io::BufReader;

    pub(crate) struct Disc(rac_iso::Disc<BufReader<File>>);

    impl DiscReader for Disc {
        fn read(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
            self.0
                .bytes(offset, len as u64)
                .map_err(|error| FormatError::Read(error.to_string()))
        }
    }

    /// The disc image of `release` in `OPENRAC_SOURCE_DIR`, known by its boot executable.
    /// A relative folder counts from the workspace's root, since tests run in the crate's.
    pub(crate) fn disc(release: rac_iso::Release) -> Disc {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        rac_iso::find_images(&root.join(folder), 2)
            .into_iter()
            .find_map(|path| {
                let mut disc = rac_iso::Disc::open(BufReader::new(File::open(path).ok()?)).ok()?;
                let cnf = rac_iso::SystemCnf::read(&mut disc).ok()?;
                (cnf.boot.eq_ignore_ascii_case(release.info().boot)
                    && rac_iso::Release::identify(&mut disc).ok()? == release)
                    .then_some(Disc(disc))
            })
            .unwrap_or_else(|| panic!("no {release:?} disc"))
    }

    /// The levels with a level file in DL's and Gladiator's level tables.
    const DL_LEVELS: [usize; 47] = [
        0, 1, 2, 4, 5, 6, 7, 8, 10, 11, 13, 14, 15, 21, 22, 24, 25, 26, 27, 28, 30, 31, 33, 34, 35,
        41, 42, 44, 45, 46, 47, 48, 50, 51, 53, 54, 61, 62, 64, 65, 66, 67, 68, 70, 71, 73, 74,
    ];
    const DL_RELEASES: [rac_iso::Release; 2] =
        [rac_iso::Release::DlNtsc, rac_iso::Release::GladiatorPal];
    /// DL's wrench, and the class the player's skin stands in for (libdl's names).
    const DL_WRENCH: u32 = 0x23FA;
    const DL_HERO: u32 = 0x23F7;

    #[test]
    #[ignore = "needs game discs"]
    fn every_dl_level_header_reads() {
        for release in DL_RELEASES {
            let mut disc = disc(release);
            let toc = Toc::read_dl_disc(&mut disc).unwrap();
            let levels: Vec<usize> = (0..toc.levels.len())
                .filter(|&level| !toc.levels[level].level.is_empty())
                .collect();
            assert_eq!(levels, DL_LEVELS, "{release:?}");
            for level in DL_LEVELS {
                let header = LevelHeader::read_dl_disc(&mut disc, level).unwrap();
                assert_eq!(header.id as usize, level, "{release:?}");
                assert!(!header.data.is_empty() && !header.gameplay.is_empty());
                // Only the story's planets and the menu have audio and scenes of their own.
                assert_eq!(!toc.levels[level].audio.is_empty(), level < 20, "{level}");
            }
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn dls_wrench_reads_from_a_level_core_with_its_texture() {
        for release in DL_RELEASES {
            let mut disc = disc(release);
            let core = Core::read_dl(&mut disc, 1).unwrap();
            let entry = core.header.moby_class(DL_WRENCH).unwrap();
            assert_eq!(entry.texture_group, 0);
            let wrench = core.dl_moby_class(DL_WRENCH).unwrap();
            assert_eq!(wrench.joints.len(), 17);
            assert!((wrench.scale - 0.2083333).abs() < 1e-6);
            // Every joint has the same turn at rest, as UYA's wrench does.
            for joint in &wrench.joints {
                let turn = joint.rest_rotation();
                let half = [-0.5, 0.5, -0.5, 0.5];
                assert!(turn.iter().zip(half).all(|(a, b)| (a - b).abs() < 1e-3));
            }
            let high = wrench.mesh(PacketKind::High).unwrap();
            assert_eq!((high.triangles.len(), high.vertices.len()), (834, 1126));
            assert_eq!(wrench.mesh(PacketKind::Low).unwrap().triangles.len(), 498);
            assert!(high.triangles.iter().all(|t| t.texture == 0));
            assert!(core.moby_bangles(DL_WRENCH).unwrap().is_empty());
            // One 128² texture, kept in EE memory, with a palette that isn't blank.
            let textures = core.moby_textures(DL_WRENCH).unwrap();
            assert_eq!(textures.len(), 1);
            let texture = &textures[0];
            assert_eq!((texture.width, texture.height), (128, 128));
            assert_eq!(core.header.moby_textures().unwrap()[88].kind, 1);
            let colours: std::collections::BTreeSet<[u8; 4]> = texture
                .pixels
                .iter()
                .map(|&i| texture.palette[i as usize])
                .collect();
            assert!(colours.len() > 64, "{release:?}: {} colours", colours.len());
            // The player's class has no mesh of its own: the skin brings it.
            let hero = core.dl_moby_class(DL_HERO).unwrap();
            assert_eq!(hero.joints.len(), 107);
            assert!(hero.packets.is_empty());
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn every_dl_level_but_the_menu_has_the_wrench() {
        let mut disc = disc(rac_iso::Release::DlNtsc);
        for level in DL_LEVELS {
            let core = Core::read_dl(&mut disc, level).unwrap();
            let wrench = core.header.moby_class(DL_WRENCH);
            assert_eq!(wrench.is_some(), level != 0, "level {level}");
            if level != 0 {
                let mesh = core
                    .dl_moby_class(DL_WRENCH)
                    .unwrap()
                    .mesh(PacketKind::High);
                assert_eq!(mesh.unwrap().triangles.len(), 834, "level {level}");
                assert_eq!(core.moby_textures(DL_WRENCH).unwrap().len(), 1);
            }
            // The player's class is only in the story's levels; the maps use other skins.
            let hero = core.header.moby_class(DL_HERO).is_some();
            assert_eq!(hero, (1..40).contains(&level), "level {level}");
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn the_wrench_hangs_from_the_right_hand() {
        // A class's joint lists (+0x1C) each name a joint, as the first of the joints that
        // follow its ancestors: the player's list 0, the point the wrench's code asks his
        // moby for, is joint 78, the right hand. The skin takes the lists from this class.
        let mut disc = disc(rac_iso::Release::DlNtsc);
        let core = Core::read_dl(&mut disc, 1).unwrap();
        let class = core.header.moby_class(DL_HERO).unwrap().offset;
        let image = Image::new(&core.bytes, 0);
        let lists = class + image.u32(class + 0x1C).unwrap();
        let first = class + image.u32(lists + 4).unwrap();
        let ancestors = image.u16(first).unwrap() as u32;
        assert_eq!(image.u8(first + 4 + ancestors).unwrap(), 78);
        let hand = core.dl_moby_class(DL_HERO).unwrap().joints[78].rest;
        assert!(hand[1] < 0.0, "on the right: {hand:?}");
    }

    #[test]
    #[ignore = "needs game discs"]
    fn dls_armors_carry_the_head_as_bangles() {
        use crate::armor::SkinTable;
        for release in DL_RELEASES {
            let mut disc = disc(release);
            let table = SkinTable::read_dl(&Toc::read_dl_disc(&mut disc).unwrap()).unwrap();
            for (index, entry) in table.entries.iter().enumerate() {
                let bytes = disc
                    .read(
                        (table.sector as u64 + entry.class.sector as u64) * SECTOR,
                        entry.class.count as usize * SECTOR as usize,
                    )
                    .unwrap();
                let image = Image::new(&bytes, 0);
                let class = MobyClass::read_dl(&image, 0).unwrap();
                let bangles = MobyClass::read_bangles(&image, 0, Placement::Packed).unwrap();
                if index >= 5 {
                    // The other skins are whole.
                    assert!(bangles.is_empty(), "{release:?} skin {index}");
                    continue;
                }
                let slots: Vec<usize> = bangles.iter().map(|b| b.slot).collect();
                assert_eq!(slots, (0..13).collect::<Vec<_>>(), "skin {index}");
                // 12 is the helmet, on the head joint alone, in the armor's texture; it
                // reaches 1.29 m, where the armor's own mesh stops at the neck.
                let helmet = bangles[12].mesh(PacketKind::High).unwrap();
                assert_eq!(helmet.triangles.len(), 486, "skin {index}");
                assert!(
                    helmet
                        .vertices
                        .iter()
                        .all(|v| v.skin.joints[0] == 12 && v.skin.weights[0] == 256)
                );
                assert!(helmet.triangles.iter().all(|t| t.texture == 0));
                let top = |mesh: &crate::moby::Mesh| {
                    mesh.vertices
                        .iter()
                        .map(|v| v.position[2] as f32 * class.unit())
                        .fold(f32::MIN, f32::max)
                };
                assert!((top(&helmet) - 1.292).abs() < 0.01, "{}", top(&helmet));
                let body = class.mesh(PacketKind::High).unwrap();
                assert!(top(&body) < 1.06, "{}", top(&body));
                // 10 is the bare head, to the ears' tips, on the face's joints, in the
                // second texture.
                let head = bangles[10].mesh(PacketKind::High).unwrap();
                assert!((top(&head) - 1.363).abs() < 0.01);
                assert!(head.triangles.iter().all(|t| t.texture == 1));
                assert!(head.vertices.iter().any(|v| v.skin.joints[0] > 12));
            }
        }
    }
}
