//! UYA's armor: the hero's mesh and textures, one set for each armor and multiplayer skin
//! (PLAN.md P3.7; `docs/research/moby-format.md`, "Armor").
//!
//! The hero's class has no mesh of its own. The game reads the armor he wears from the
//! armor WAD, the fifth global header in the table of contents (`armor_load`, 0x422838):
//! its textures first, which go straight to the GS, then its mesh, which it relocates
//! (`armor_relocate`, 0x423F28) and hands to the hero's class (`armor_attach`, 0x424008).
//! The same WAD holds the wrench (entry [`WRENCH`], `wrench_load`, 0x422D70) and Clank's
//! textures (`clank_texture_load`, 0x423238); Clank's mesh is his class's own. Its last
//! entries, [`UYA_SKINS`], are the multiplayer skins: whole moby classes on a skeleton
//! that is the hero's less some of his face ([`Skin::read_uya`], [`UYA_SKIN_JOINTS`]).
//!
//! DL keeps its skins in the same place, the fifth global header, but each is a whole
//! moby class with its skeleton ([`SkinTable`], [`Skin`]).

use std::ops::Range;

use crate::level::{SECTOR, SectorRange, Toc};
use crate::moby::{Bangle, Mesh, MobyClass, Packet, PacketKind, Placement, mesh_of, read_packets};
use crate::texture::Texture;
use crate::{DiscReader, FormatError, Image};

/// The armor WAD's header among UYA's globals, and the skins' among DL's.
const UYA_ARMOR_GLOBAL: usize = 4;
const DL_SKIN_GLOBAL: usize = 4;
/// Entries in UYA's armor table: meshes for the hero up to 34, then [`UYA_SKINS`].
const UYA_ARMORS: usize = 56;
/// The entry the game loads for the wrench.
pub const WRENCH: usize = 31;
/// UYA's multiplayer skins in the armor table, each a whole moby class with its own
/// skeleton rather than a mesh for the hero's: libuya's skin numbers 1 (Robo) to 21 (Dan)
/// in order, `moby-format.md`, "Multiplayer skins". Skin 0, Ratchet, has no entry here.
pub const UYA_SKINS: Range<usize> = 35..56;
/// For each joint of a multiplayer skin's skeleton, the hero's joint it is. The skins' 86
/// joints are the hero's 111 less 25 of his face's, at the same rest positions with the
/// same parents and binds (but Brainius' left eye, turned half a degree more), so his
/// animations can drive them through this.
pub const UYA_SKIN_JOINTS: [u8; 86] = {
    let mut map = [0; 86];
    let mut joint = 0;
    while joint < map.len() {
        map[joint] = match joint {
            // The root, spine, neck, head and jaw.
            0..=10 => joint,
            11 => 18,
            // The eyes and what hangs from them.
            12..=15 => joint + 9,
            16 => 29,
            17 => 30,
            18 => 32,
            // The ears.
            19..=24 => joint + 22,
            25 | 26 => joint + 24,
            // Both arms with their fingers, both legs and the tail.
            _ => joint + 25,
        } as u8;
        joint += 1;
    }
    map
};

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

/// One of DL's skins or UYA's multiplayer skins: a moby class with its own skeleton and
/// mesh, and its textures.
#[derive(Debug, Clone, PartialEq)]
pub struct Skin {
    pub class: MobyClass,
    /// By the texture numbers the mesh's texture switches use.
    pub textures: Vec<Texture>,
    /// The parts of its mesh a moby shows or hides one by one: DL's armors keep their
    /// head, helmet and face there (`moby-format.md`, "Bangles").
    pub bangles: Vec<Bangle>,
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
        let image = Image::new(&class, 0);
        Ok(Self {
            class: MobyClass::read_dl(&image, 0)?,
            textures: read_textures(&textures)?,
            bangles: MobyClass::read_bangles(&image, 0, Placement::Packed)?,
        })
    }

    /// Reads UYA's multiplayer skin at entry `index` of the armor table, one of
    /// [`UYA_SKINS`]. The entries before them are meshes for the hero's class ([`Armor`]).
    pub fn read_uya(
        disc: &mut impl DiscReader,
        table: &ArmorTable,
        index: usize,
    ) -> Result<Self, FormatError> {
        // Read as a class, an armor's mesh file would give nonsense rather than an error.
        if !UYA_SKINS.contains(&index) {
            return Err(FormatError::Invalid {
                what: "multiplayer skin",
                address: index as u32,
                detail: format!(
                    "armor entry {index} isn't one; they are {} to {}",
                    UYA_SKINS.start,
                    UYA_SKINS.end - 1
                ),
            });
        }
        let entry = table
            .entries
            .get(index)
            .ok_or_else(|| FormatError::Invalid {
                what: "multiplayer skin",
                address: index as u32,
                detail: format!("the armor WAD lists {}", table.entries.len()),
            })?;
        let mut read = |range: SectorRange| {
            disc.read(
                (table.sector as u64 + range.sector as u64) * SECTOR,
                range.count as usize * SECTOR as usize,
            )
        };
        let class = read(entry.mesh)?;
        let textures = read(entry.textures)?;
        Self::parse_uya(&class, &textures)
    }

    /// Reads a UYA multiplayer skin from its two files: a class as the disc stores one,
    /// with UYA's skeleton and pointers counted from its start, and a texture file as an
    /// armor's.
    pub fn parse_uya(class: &[u8], textures: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(class, 0);
        Ok(Self {
            class: MobyClass::read_placed(&image, 0, Placement::Packed)?,
            textures: read_textures(textures)?,
            bangles: MobyClass::read_bangles(&image, 0, Placement::Packed)?,
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

    /// A UYA class as the disc stores one, pointers from its start: two joints, the second
    /// 10 units above the first, no sequences and no packets.
    fn packed_class() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x120];
        let mut put = |at: usize, word: u32| {
            bytes[at..at + 4].copy_from_slice(&word.to_le_bytes());
        };
        put(0x08, 2); // The joint count's byte.
        put(0x14, 0x80); // The skeleton.
        put(0x18, 0x100); // The joint offsets.
        put(0x24, 0.1458f32.to_bits());
        for (joint, height) in [0.0f32, 10.0].into_iter().enumerate() {
            // Identity columns, then the translation taking the joint to the origin.
            let record = 0x80 + joint * 0x40;
            put(record, 1.0f32.to_bits());
            put(record + 0x14, 1.0f32.to_bits());
            put(record + 0x28, 1.0f32.to_bits());
            put(record + 0x38, (-height).to_bits());
            let offset = 0x100 + joint * 0x10;
            put(offset + 8, height.to_bits());
            // The parent, as the scratchpad address of its record.
            put(offset + 0xC, if joint == 0 { 0 } else { 0x7000_0000 });
        }
        bytes
    }

    /// A texture file holding one 2 by 1 PIF2 image.
    fn texture_file() -> Vec<u8> {
        let mut bytes = vec![0u8; 8 + 0x420 + 2];
        bytes[0..4].copy_from_slice(&1u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&8u32.to_le_bytes());
        bytes[8..12].copy_from_slice(b"2FIP");
        bytes[16..20].copy_from_slice(&2u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&0x13u32.to_le_bytes());
        bytes
    }

    #[test]
    fn a_uya_skin_is_a_whole_class_with_its_textures() {
        let skin = Skin::parse_uya(&packed_class(), &texture_file()).unwrap();
        assert_eq!(skin.class.joints.len(), 2);
        assert_eq!(skin.class.joints[1].parent, Some(0));
        assert_eq!(skin.class.joints[1].rest, [0.0, 0.0, 10.0]);
        assert_eq!(skin.class.scale, 0.1458);
        assert!(skin.class.packets.is_empty());
        assert_eq!((skin.textures[0].width, skin.textures[0].height), (2, 1));
    }

    /// A disc that is never read.
    struct NoDisc;

    impl DiscReader for NoDisc {
        fn read(&mut self, offset: u64, _len: usize) -> Result<Vec<u8>, FormatError> {
            Err(FormatError::Read(format!("nothing at {offset}")))
        }
    }

    #[test]
    fn only_the_last_entries_are_read_as_skins() {
        let range = SectorRange {
            sector: 1,
            count: 1,
        };
        let table = ArmorTable {
            sector: 0,
            entries: vec![
                ArmorEntry {
                    mesh: range,
                    textures: range,
                };
                UYA_ARMORS
            ],
            clank_textures: Vec::new(),
        };
        for index in [0, WRENCH, 34, 56] {
            let error = Skin::read_uya(&mut NoDisc, &table, index).unwrap_err();
            assert!(
                matches!(
                    error,
                    FormatError::Invalid {
                        what: "multiplayer skin",
                        ..
                    }
                ),
                "{index}: {error}"
            );
        }
        // A skin's entry goes on to the disc.
        let error = Skin::read_uya(&mut NoDisc, &table, 35).unwrap_err();
        assert!(matches!(error, FormatError::Read(_)), "{error}");
    }

    #[test]
    fn skin_joints_are_hero_joints_in_order() {
        // Parents come before their children in both skeletons, so the map must keep order.
        assert!(UYA_SKIN_JOINTS.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(UYA_SKIN_JOINTS[..11], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(UYA_SKIN_JOINTS[12..16], [21, 22, 23, 24]);
        assert_eq!(UYA_SKIN_JOINTS[85], 110);
    }
}

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;

    use rac_iso::{Disc, Release, SystemCnf};

    use super::*;
    use crate::level::Core;

    /// Veldin, whose core holds the hero's class.
    const VELDIN: usize = 1;

    struct DiscImage(Disc<BufReader<File>>);

    impl DiscReader for DiscImage {
        fn read(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
            self.0
                .bytes(offset, len as u64)
                .map_err(|error| FormatError::Read(error.to_string()))
        }
    }

    /// The UYA NTSC-U disc image in `OPENRAC_SOURCE_DIR`, known by its boot executable.
    fn uya() -> DiscImage {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        rac_iso::find_images(Path::new(&folder), 2)
            .into_iter()
            .find_map(|path| {
                let mut disc = Disc::open(BufReader::new(File::open(path).ok()?)).ok()?;
                let boot = SystemCnf::read(&mut disc).ok()?.boot;
                if !boot.eq_ignore_ascii_case(Release::UyaNtsc.info().boot) {
                    return None;
                }
                (Release::identify(&mut disc).ok()? == Release::UyaNtsc).then_some(DiscImage(disc))
            })
            .expect("a UYA NTSC-U disc image")
    }

    #[test]
    #[ignore = "needs game discs"]
    fn uya_skins_are_the_heros_skeleton_less_some_of_his_face() {
        let mut disc = uya();
        let table = ArmorTable::read_uya(&Toc::read_uya_disc(&mut disc).unwrap()).unwrap();
        let hero = Core::read_uya(&mut disc, VELDIN).unwrap().hero().unwrap();
        // Positions agree to 2 mm at the hero's scale; one of Brainius' is 1.3 mm off.
        let near = 0.002 / hero.unit();
        for index in UYA_SKINS {
            let skin = Skin::read_uya(&mut disc, &table, index).unwrap();
            let class = &skin.class;
            assert_eq!(class.joints.len(), UYA_SKIN_JOINTS.len(), "skin {index}");
            assert!(!skin.textures.is_empty(), "skin {index}");
            let triangles = class.mesh(PacketKind::High).unwrap().triangles.len();
            assert!(triangles > 1000, "skin {index}: {triangles} triangles");
            for (joint, own) in class.joints.iter().enumerate() {
                let theirs = &hero.joints[UYA_SKIN_JOINTS[joint] as usize];
                let parent = own.parent.map(|p| UYA_SKIN_JOINTS[p] as usize);
                assert_eq!(theirs.parent, parent, "skin {index} joint {joint}'s parent");
                // In class units: Bruiser's class is scaled up, not his skeleton.
                let apart = (0..3)
                    .map(|axis| (own.rest[axis] - theirs.rest[axis]).abs())
                    .fold(0.0, f32::max);
                assert!(
                    apart < near,
                    "skin {index} joint {joint}: {apart} units off"
                );
                // The same turn and scale at rest; Brainius' left eye is turned half a
                // degree more.
                let dot: f32 = own
                    .rest_rotation()
                    .iter()
                    .zip(theirs.rest_rotation())
                    .map(|(a, b)| a * b)
                    .sum();
                let degrees = 2.0 * dot.abs().min(1.0).acos().to_degrees();
                assert!(
                    degrees < 1.0,
                    "skin {index} joint {joint}: turned {degrees}°"
                );
                // How much the bind scales each of the joint's axes.
                let scale = |bind: &[[f32; 4]; 3]| {
                    bind.map(|row| row[..3].iter().map(|v| v * v).sum::<f32>().sqrt())
                };
                for (a, b) in scale(&own.inverse_bind)
                    .iter()
                    .zip(scale(&theirs.inverse_bind))
                {
                    assert!(
                        (a - b).abs() < 0.01,
                        "skin {index} joint {joint}: scaled {a} for {b}"
                    );
                }
            }
        }
    }
}
