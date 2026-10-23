//! UYA's gadget WAD: the moby classes of what the hero holds in his hand other than the
//! wrench (the guns, gloves and hand gadgets), each with its texture and sound bank
//! (PLAN.md P1.9, P3.7; `docs/research/moby-format.md`, "The gadget WAD").
//!
//! No level's core has a class for a gun. The game streams the class of the item in hand
//! (its weapon row's +0x10) from this WAD, the seventh global header in the table of
//! contents. Read in UYA NTSC-U's code: `gadget_read` (0x429C30) reads an entry's sound
//! bank and file into one of three buffers, and when the item comes out `gadget_select`
//! (0x42A300) has `gadget_install` (0x42A058) unpack its texture, which goes to a GS slot of
//! its own, and then its class, which it registers with that texture as the class's
//! texture 0.
//!
//! Each entry names its class, so a gun's class is found by its number as the game finds
//! it ([`GadgetTable::find`]). Its file starts with where the class and its textures are,
//! each a WAD ([`Gadget::parse`]). The sound bank is a 989snd bank the game keeps in IOP
//! memory ([`GadgetTable::read_sounds`]), whose sounds the class's sound table plays.

use crate::armor::read_textures;
use crate::level::{SECTOR, SectorRange, Toc};
use crate::moby::{Bangle, Mesh, MobyClass, PacketKind, Placement};
use crate::sound::{ClassSound, SoundBank};
use crate::texture::Texture;
use crate::{DiscReader, FormatError, Image, wad};

/// The gadget WAD's header among UYA's globals.
const UYA_GADGET_GLOBAL: usize = 6;
/// Entries in the gadget table; the game's searches stop after this many.
pub const UYA_GADGETS: usize = 48;
/// Bytes per entry: the class number, then the file's and the sound bank's sectors.
const ENTRY: u32 = 20;
/// Bytes of a gadget file's header: where its class and textures are.
const FILE_HEADER: usize = 16;

/// One entry of the gadget table. Its sectors count from the gadget WAD's first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GadgetEntry {
    /// The moby class number, as weapon rows name it. Entries 11, 12, 28, 40, 41 and
    /// 45–47 have 0 and the same file on UYA NTSC-U, which no gun names.
    pub class: u32,
    /// The class and its textures ([`Gadget::parse`]).
    pub file: SectorRange,
    /// A 989snd sound bank, which the game sends to IOP memory; empty for some entries.
    pub sounds: SectorRange,
}

/// The gadget WAD's table: its first sector, then one entry per gadget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GadgetTable {
    pub sector: u32,
    pub entries: Vec<GadgetEntry>,
}

impl GadgetTable {
    /// The table in UYA's table of contents: after the header's size and sector,
    /// [`UYA_GADGETS`] entries of five words (class number, file sector and count, sound
    /// bank sector and count), which fill the header.
    pub fn read_uya(toc: &Toc) -> Result<Self, FormatError> {
        let global = toc
            .globals
            .get(UYA_GADGET_GLOBAL)
            .ok_or(FormatError::Invalid {
                what: "table of contents",
                address: 0,
                detail: "no gadget WAD".into(),
            })?;
        let image = Image::new(&global.bytes, 0);
        let entries = (0..UYA_GADGETS as u32)
            .map(|index| {
                let at = 8 + index * ENTRY;
                Ok(GadgetEntry {
                    class: image.u32(at)?,
                    file: SectorRange {
                        sector: image.u32(at + 4)?,
                        count: image.u32(at + 8)?,
                    },
                    sounds: SectorRange {
                        sector: image.u32(at + 12)?,
                        count: image.u32(at + 16)?,
                    },
                })
            })
            .collect::<Result<_, FormatError>>()?;
        Ok(Self {
            sector: global.sector,
            entries,
        })
    }

    /// The entry of moby class `class`: the first that names it, as the game searches
    /// (`gadget_select`, `gadget_preload` 0x429D28).
    pub fn find(&self, class: u32) -> Option<usize> {
        self.entries.iter().position(|entry| entry.class == class)
    }

    /// Reads entry `index`'s sound bank from the disc; `None` for an entry with none.
    pub fn read_sounds(
        &self,
        disc: &mut impl DiscReader,
        index: usize,
    ) -> Result<Option<SoundBank>, FormatError> {
        let Some(entry) = self.entries.get(index) else {
            return Ok(None);
        };
        if entry.sounds.is_empty() {
            return Ok(None);
        }
        let bytes = disc.read(
            (self.sector as u64 + entry.sounds.sector as u64) * SECTOR,
            entry.sounds.count as usize * SECTOR as usize,
        )?;
        SoundBank::parse(&bytes).map(Some)
    }
}

/// A gadget's class and its textures, as the game installs them for the item in hand.
#[derive(Debug, Clone, PartialEq)]
pub struct Gadget {
    pub class: MobyClass,
    /// By the texture numbers the mesh's texture switches use. The guns have one, which
    /// the game draws all of the class with.
    pub textures: Vec<Texture>,
    /// The parts of its mesh a moby shows or hides one by one (`moby-format.md`,
    /// "Bangles").
    pub bangles: Vec<Bangle>,
    /// What it plays, from its own sound bank (`gadget_install` points every entry at it).
    pub sounds: Vec<ClassSound>,
}

impl Gadget {
    /// Reads the gadget of moby class `class` (a weapon row's +0x10) from a UYA disc.
    pub fn read_uya(disc: &mut impl DiscReader, class: u32) -> Result<Self, FormatError> {
        let table = GadgetTable::read_uya(&Toc::read_uya_disc(disc)?)?;
        let index = table.find(class).ok_or_else(|| FormatError::Invalid {
            what: "gadget",
            address: class,
            detail: format!("the gadget WAD has no class {class:#x}"),
        })?;
        Self::read_from(disc, &table, index)
    }

    /// Reads entry `index` of `table` from the disc.
    pub fn read_from(
        disc: &mut impl DiscReader,
        table: &GadgetTable,
        index: usize,
    ) -> Result<Self, FormatError> {
        let entry = table
            .entries
            .get(index)
            .ok_or_else(|| FormatError::Invalid {
                what: "gadget",
                address: index as u32,
                detail: format!("the gadget WAD lists {}", table.entries.len()),
            })?;
        let file = disc.read(
            (table.sector as u64 + entry.file.sector as u64) * SECTOR,
            entry.file.count as usize * SECTOR as usize,
        )?;
        Self::parse(&file, entry.file.sector)
    }

    /// Reads a gadget from its file, which starts at `sector` of the gadget WAD. The file
    /// starts with four words: the class's offset and size, then the textures' (a size of 0
    /// for none), with offsets in bytes from the WAD's first sector, so less `sector`'s
    /// start within the file (`gadget_install`). Each is a WAD: the class unpacks to a
    /// class as the disc stores one, pointers from its start; the textures to a texture
    /// file as an armor's.
    pub fn parse(file: &[u8], sector: u32) -> Result<Self, FormatError> {
        let header = Image::new(file, 0);
        let start = sector as u64 * SECTOR;
        let part = |what: &'static str, at: u32| -> Result<Option<Vec<u8>>, FormatError> {
            let (offset, size) = (header.u32(at)?, header.u32(at + 4)? as usize);
            if size == 0 {
                return Ok(None);
            }
            let invalid = |detail: String| FormatError::Invalid {
                what,
                address: offset,
                detail,
            };
            let from = (offset as u64)
                .checked_sub(start)
                .filter(|&from| from >= FILE_HEADER as u64)
                .ok_or_else(|| invalid(format!("outside the file at sector {sector:#x}")))?
                as usize;
            let bytes =
                file.get(from..from.saturating_add(size))
                    .ok_or(FormatError::OutOfBounds {
                        address: offset,
                        len: size,
                    })?;
            if wad::wad_size(bytes) != Some(size) {
                return Err(invalid(format!("not a WAD of {size:#x} bytes")));
            }
            wad::decompress(bytes).map(Some)
        };
        let class = part("gadget class", 0)?.ok_or(FormatError::Invalid {
            what: "gadget class",
            address: 0,
            detail: "none in the file".into(),
        })?;
        let textures = match part("gadget textures", 8)? {
            Some(bytes) => read_textures(&bytes)?,
            None => Vec::new(),
        };
        let image = Image::new(&class, 0);
        Ok(Self {
            class: MobyClass::read_placed(&image, 0, Placement::Packed)?,
            textures,
            bangles: MobyClass::read_bangles(&image, 0, Placement::Packed)?,
            sounds: ClassSound::read_table(&image, 0, true)?,
        })
    }

    /// The mesh at one level of detail, or of the metal packets.
    pub fn mesh(&self, kind: PacketKind) -> Result<Mesh, FormatError> {
        self.class.mesh(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::GlobalHeader;

    /// A table of contents whose seventh global header is a gadget table of `classes`,
    /// each with a file and a sound bank of its own.
    fn toc(classes: &[u32]) -> Toc {
        let mut bytes = Vec::new();
        bytes.extend((8 + UYA_GADGETS as u32 * ENTRY).to_le_bytes());
        bytes.extend(3000u32.to_le_bytes());
        for index in 0..UYA_GADGETS as u32 {
            let class = classes.get(index as usize).copied().unwrap_or(0);
            for word in [class, 1 + index * 40, 20, 21 + index * 40, 19] {
                bytes.extend(word.to_le_bytes());
            }
        }
        let mut globals = vec![
            GlobalHeader {
                sector: 0,
                bytes: vec![8, 0, 0, 0, 0, 0, 0, 0],
            };
            UYA_GADGET_GLOBAL
        ];
        globals.push(GlobalHeader {
            sector: 3000,
            bytes,
        });
        Toc {
            globals,
            levels: Vec::new(),
        }
    }

    #[test]
    fn the_table_lists_48_gadgets_by_class() {
        let table = GadgetTable::read_uya(&toc(&[0xA5D, 0xC4A, 0x5C9, 0x1863])).unwrap();
        assert_eq!(table.sector, 3000);
        assert_eq!(table.entries.len(), UYA_GADGETS);
        assert_eq!(
            table.entries[3],
            GadgetEntry {
                class: 0x1863,
                file: SectorRange {
                    sector: 121,
                    count: 20
                },
                sounds: SectorRange {
                    sector: 141,
                    count: 19
                },
            }
        );
        assert_eq!(table.find(0x5C9), Some(2));
        assert_eq!(table.find(0x1864), None);
    }

    #[test]
    fn the_first_entry_of_a_class_is_the_one_found() {
        let table = GadgetTable::read_uya(&toc(&[0x10, 0x20, 0x10])).unwrap();
        assert_eq!(table.find(0x10), Some(0));
        // The rest of the synthetic table is class 0.
        assert_eq!(table.find(0), Some(3));
    }

    #[test]
    fn a_short_header_is_an_error() {
        let mut toc = toc(&[]);
        toc.globals[UYA_GADGET_GLOBAL]
            .bytes
            .truncate(8 + 10 * ENTRY as usize);
        assert!(GadgetTable::read_uya(&toc).is_err());
        toc.globals.truncate(UYA_GADGET_GLOBAL);
        assert!(GadgetTable::read_uya(&toc).is_err());
    }

    use crate::wad::build::stored;

    /// A class as the disc stores one: one joint at the origin, no sequences or packets.
    fn class_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0xA0];
        let mut put = |at: usize, word: u32| {
            bytes[at..at + 4].copy_from_slice(&word.to_le_bytes());
        };
        put(0x08, 1);
        put(0x14, 0x50); // The skeleton: identity columns, no translation.
        put(0x18, 0x90); // The joint's offset.
        put(0x24, 0.0569f32.to_bits());
        for column in 0..3 {
            put(0x50 + column * 0x14, 1.0f32.to_bits());
        }
        bytes
    }

    /// A texture file as the guns' are: a count of one, its offset (padded to 16 bytes),
    /// then a 2 by 1 PIF2 image, cut to `len` bytes.
    fn texture_bytes(len: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x10 + 0x420 + 2];
        bytes[0..4].copy_from_slice(&1u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&0x10u32.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(b"2FIP");
        bytes[0x18..0x1C].copy_from_slice(&2u32.to_le_bytes());
        bytes[0x1C..0x20].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x20..0x24].copy_from_slice(&0x13u32.to_le_bytes());
        // Palette entry 1, then the second pixel, which uses it.
        bytes[0x34..0x38].copy_from_slice(&[0xFF, 0x80, 0x00, 0x80]);
        bytes[0x10 + 0x421] = 1;
        bytes.truncate(len);
        bytes
    }

    /// A gadget file at `sector` of the WAD: the header, the class's WAD and, 16-aligned,
    /// the textures' WAD (when given), with offsets from the WAD's first sector.
    fn file(sector: u32, textures: Option<&[u8]>) -> Vec<u8> {
        let start = sector * SECTOR as u32;
        let class = stored(&class_bytes());
        let mut bytes = vec![0u8; FILE_HEADER];
        bytes.extend(&class);
        let mut header = [start + FILE_HEADER as u32, class.len() as u32, 0, 0];
        if let Some(textures) = textures {
            while !bytes.len().is_multiple_of(16) {
                bytes.push(0);
            }
            let packed = stored(textures);
            header[2] = start + bytes.len() as u32;
            header[3] = packed.len() as u32;
            bytes.extend(&packed);
        }
        for (index, word) in header.iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn a_gadget_file_unpacks_to_its_class_and_texture() {
        let textures = texture_bytes(usize::MAX);
        let gadget = Gadget::parse(&file(0x2FF, Some(&textures)), 0x2FF).unwrap();
        assert_eq!(gadget.class.joints.len(), 1);
        assert_eq!(gadget.class.joints[0].parent, None);
        assert_eq!(gadget.class.joints[0].rest, [0.0; 3]);
        assert_eq!(gadget.class.scale, 0.0569);
        assert!(gadget.class.packets.is_empty());
        assert!(gadget.bangles.is_empty());
        assert_eq!(gadget.textures.len(), 1);
        let texture = &gadget.textures[0];
        assert_eq!((texture.width, texture.height), (2, 1));
        assert_eq!(texture.pixels, [0, 1]);
        assert_eq!(texture.palette[1], [0xFF, 0x80, 0x00, 0x80]);
    }

    #[test]
    fn a_gadget_may_have_no_textures() {
        let gadget = Gadget::parse(&file(1, None), 1).unwrap();
        assert_eq!(gadget.class.joints.len(), 1);
        assert!(gadget.textures.is_empty());
    }

    #[test]
    fn a_texture_file_too_short_for_its_image_is_an_error() {
        // The PIF2 image's header says 2 by 1, but its pixels aren't all there.
        let error = Gadget::parse(&file(5, Some(&texture_bytes(0x431))), 5).unwrap_err();
        assert!(matches!(error, FormatError::OutOfBounds { .. }), "{error}");
    }

    #[test]
    fn offsets_outside_the_file_are_errors() {
        let bytes = file(5, None);
        // Read as if it started a sector later, the class lies before it.
        let error = Gadget::parse(&bytes, 6).unwrap_err();
        assert!(
            matches!(
                error,
                FormatError::Invalid {
                    what: "gadget class",
                    ..
                }
            ),
            "{error}"
        );
        // Cut short, the class runs past its end.
        let error = Gadget::parse(&bytes[..bytes.len() - 4], 5).unwrap_err();
        assert!(matches!(error, FormatError::OutOfBounds { .. }), "{error}");
        // A file with no class.
        let mut empty = bytes.clone();
        empty[4..8].fill(0);
        assert!(Gadget::parse(&empty, 5).is_err());
        // Too short for its header.
        assert!(Gadget::parse(&bytes[..12], 5).is_err());
    }

    #[test]
    fn a_size_that_isnt_the_wads_is_an_error() {
        let mut bytes = file(5, None);
        let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        bytes[4..8].copy_from_slice(&(size - 1).to_le_bytes());
        let error = Gadget::parse(&bytes, 5).unwrap_err();
        assert!(
            matches!(
                error,
                FormatError::Invalid {
                    what: "gadget class",
                    ..
                }
            ),
            "{error}"
        );
    }
}

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use crate::level::disc_tests::disc;

    /// A gun the kit has: its V1 row's class (`weapons.md`), its entry in the table, and
    /// its class header's joint count and high, low and metal packet counts, as read from
    /// the disc's bytes (2026-10-21).
    struct Gun {
        name: &'static str,
        class: u32,
        entry: usize,
        joints: usize,
        packets: [usize; 3],
    }

    const GUNS: [Gun; 6] = [
        Gun {
            name: "Shock Blaster",
            class: 0x1863,
            entry: 13,
            joints: 14,
            packets: [15, 0, 4],
        },
        Gun {
            name: "N60 Storm",
            class: 0x18BE,
            entry: 17,
            joints: 8,
            packets: [17, 0, 6],
        },
        Gun {
            name: "Annihilator",
            class: 0x1864,
            entry: 29,
            joints: 9,
            packets: [15, 0, 0],
        },
        Gun {
            name: "Flux Rifle",
            class: 0xC04,
            entry: 27,
            joints: 14,
            packets: [13, 0, 2],
        },
        Gun {
            name: "Nitro Launcher",
            class: 0x1865,
            entry: 15,
            joints: 20,
            packets: [16, 0, 0],
        },
        Gun {
            name: "Lava Gun",
            class: 0x5C9,
            entry: 2,
            joints: 21,
            packets: [7, 4, 0],
        },
    ];

    #[test]
    #[ignore = "needs game discs"]
    fn uya_guns_read_from_the_gadget_wad() {
        let mut disc = disc(rac_iso::Release::UyaNtsc);
        let table = GadgetTable::read_uya(&Toc::read_uya_disc(&mut disc).unwrap()).unwrap();
        assert_eq!(table.sector, 0xE51DE);
        for gun in GUNS {
            let name = gun.name;
            assert_eq!(table.find(gun.class), Some(gun.entry), "{name}");
            let gadget = Gadget::read_from(&mut disc, &table, gun.entry).unwrap();
            let class = &gadget.class;
            assert_eq!(class.joints.len(), gun.joints, "{name}");
            assert_eq!(class.sequences.len(), 3, "{name}");
            for (kind, count) in [PacketKind::High, PacketKind::Low, PacketKind::Metal]
                .into_iter()
                .zip(gun.packets)
            {
                let packets = class.packets.iter().filter(|p| p.kind == kind).count();
                assert_eq!(packets, count, "{name}: {kind:?} packets");
            }
            // The guns' classes are scaled 0.044 to 0.061.
            assert!(
                (0.04..0.07).contains(&class.scale),
                "{name}: {}",
                class.scale
            );
            // One 128² texture, which the game maps the class's texture 0 to.
            let sizes: Vec<_> = gadget
                .textures
                .iter()
                .map(|t| (t.width, t.height))
                .collect();
            assert_eq!(sizes, [(128, 128)], "{name}");
            let high = gadget.mesh(PacketKind::High).unwrap();
            assert!(
                high.triangles.len() > 100,
                "{name}: {}",
                high.triangles.len()
            );
            let used: std::collections::BTreeSet<i32> =
                high.triangles.iter().map(|t| t.texture).collect();
            assert!(used.iter().all(|&t| t == 0), "{name}: textures {used:?}");
        }
        // The class sound tables of two (`effects.md`, "Sounds"): the Shock Blaster's
        // charge (4) and the Lava Gun's stream (0) loop.
        let mut looped = |class: u32| -> Vec<bool> {
            let index = table.find(class).unwrap();
            let gadget = Gadget::read_from(&mut disc, &table, index).unwrap();
            assert!(
                gadget
                    .sounds
                    .iter()
                    .enumerate()
                    .all(|(i, s)| s.sound == Some(i as u16))
            );
            gadget.sounds.iter().map(|sound| sound.looped).collect()
        };
        assert_eq!(looped(0x1863), [false, false, false, false, true]);
        assert_eq!(looped(0x5C9), [true, false, false, false, true]);
        // Every gun's bank reads: version 3 but for entries 0, 3 and 10 (version 1), and
        // every tone's sample within it.
        for index in 0..UYA_GADGETS {
            let Some(bank) = table.read_sounds(&mut disc, index).unwrap() else {
                assert_eq!(index, 44, "entry {index} has no bank");
                continue;
            };
            let version = if [0, 3, 10].contains(&index) { 1 } else { 3 };
            assert_eq!(bank.version, version, "entry {index}");
            for grain in bank.sounds.iter().flat_map(|sound| &sound.grains) {
                if let crate::sound::GrainOp::Tone(tone) = &grain.op {
                    assert!((tone.sample as usize) < bank.samples.len(), "entry {index}");
                }
            }
        }
        // The same through the class number alone; the wrench isn't in it.
        let lava = Gadget::read_uya(&mut disc, 0x5C9).unwrap();
        assert_eq!(lava.class.joints.len(), 21);
        assert!(Gadget::read_uya(&mut disc, 0x47).is_err());
    }
}
