//! A level's gameplay file, as far as the hero needs it so far: the rails he grinds
//! (PLAN.md P3.10; `docs/research/gameplay-format.md`).
//!
//! Read on UYA NTSC-U, from the disc and the game's level loader (the function that fills
//! the rail list, 0x4252F0 in Veldin's build).

use crate::level::{LevelHeader, SECTOR, read_wad};
use crate::{DiscReader, FormatError, Image};

/// Where the gameplay file's header keeps the offset of its grind paths.
const GRIND_PATHS: u32 = 0x7C;
/// Bytes per grind path record: the bounding sphere, then four words.
const PATH_RECORD: u32 = 0x20;
/// Bytes per spline point: x, y, z, then the length of the segment it starts.
const POINT: u32 = 0x10;

/// A level's gameplay file, unpacked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gameplay {
    pub bytes: Vec<u8>,
}

/// A rail the hero can grind. He always moves along it in the order of its points.
#[derive(Debug, Clone, PartialEq)]
pub struct GrindPath {
    /// The centre of a sphere holding the whole rail; the game only looks for the
    /// nearest point of rails whose sphere holds the hero.
    pub centre: [f32; 3],
    pub radius: f32,
    /// The record's word that the game passes to its spline search with the spline.
    /// It is 0 on every rail on the UYA disc.
    pub flags: u32,
    /// The spline's points, in world units.
    pub points: Vec<[f32; 3]>,
}

impl Gameplay {
    /// Reads and unpacks UYA level `level`'s gameplay file from the disc.
    pub fn read_uya(disc: &mut impl DiscReader, level: usize) -> Result<Self, FormatError> {
        let header = LevelHeader::read_uya_disc(disc, level)?;
        let at = (header.sector as u64 + header.gameplay.sector as u64) * SECTOR;
        Ok(Self {
            bytes: read_wad(disc, at, "gameplay file")?,
        })
    }

    /// The level's rails, in the game's order. A level without rails has none.
    ///
    /// The block is a count, the offset of the spline data (from the block's start) and
    /// its size, then one 0x20-byte record per rail, then one offset per rail into the
    /// spline data. A spline is its point count, three zero words, then the points.
    pub fn grind_paths(&self) -> Result<Vec<GrindPath>, FormatError> {
        let image = Image::new(&self.bytes, 0);
        let block = image.u32(GRIND_PATHS)?;
        if block == 0 {
            return Ok(Vec::new());
        }
        let count = image.u32(block)?;
        let data = block.checked_add(image.u32(block + 4)?);
        let records = block + 0x10;
        let table = count
            .checked_mul(PATH_RECORD)
            .and_then(|size| records.checked_add(size));
        let (Some(data), Some(table)) = (data, table) else {
            return Err(FormatError::Invalid {
                what: "grind paths",
                address: block,
                detail: format!("{count} paths"),
            });
        };
        // Bounds the count before anything is allocated for it.
        image.bytes(table, count as usize * 4)?;
        (0..count)
            .map(|path| {
                let record = records + path * PATH_RECORD;
                let [x, y, z, radius] = image.f32s::<4>(record)?;
                let spline = data.checked_add(image.u32(table + path * 4)?).ok_or(
                    FormatError::OutOfBounds {
                        address: table + path * 4,
                        len: 4,
                    },
                )?;
                Ok(GrindPath {
                    centre: [x, y, z],
                    radius,
                    flags: image.u32(record + 0x10)?,
                    points: read_spline(&image, spline)?,
                })
            })
            .collect()
    }
}

fn read_spline(image: &Image, at: u32) -> Result<Vec<[f32; 3]>, FormatError> {
    let count = image.u32(at)?;
    let first = at + 0x10;
    image.bytes(first, count as usize * POINT as usize)?;
    (0..count)
        .map(|point| {
            let [x, y, z, _length] = image.f32s::<4>(first + point * POINT)?;
            Ok([x, y, z])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby::tests::Writer;

    /// A gameplay file whose grind path block at 0x100 holds `paths`.
    fn gameplay(paths: &[&[[f32; 3]]]) -> Gameplay {
        let mut w = Writer::default();
        w.at(GRIND_PATHS).u32(0x100);
        let records = 0x110;
        let table = records + paths.len() as u32 * PATH_RECORD;
        let data = table + paths.len() as u32 * 4;
        w.at(0x100)
            .u32(paths.len() as u32)
            .u32(data - 0x100)
            .u32(0)
            .u32(0);
        for (i, _) in paths.iter().enumerate() {
            w.f32(10.0 * i as f32).f32(2.0).f32(3.0).f32(4.0);
            w.u32(i as u32).u32(0).u32(0).u32(0);
        }
        let mut offset = 0;
        for points in paths {
            w.u32(offset);
            offset += 0x10 + points.len() as u32 * POINT;
        }
        for points in paths {
            w.u32(points.len() as u32).u32(0).u32(0).u32(0);
            for [x, y, z] in *points {
                w.f32(*x).f32(*y).f32(*z).f32(1.0);
            }
        }
        Gameplay { bytes: w.bytes }
    }

    #[test]
    fn reads_each_rails_sphere_and_points() {
        let a = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let b = [[7.0, 8.0, 9.0], [10.0, 11.0, 12.0], [13.0, 14.0, 15.0]];
        let paths = gameplay(&[&a, &b]).grind_paths().unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].points, a);
        assert_eq!(paths[1].points, b);
        assert_eq!(paths[1].centre, [10.0, 2.0, 3.0]);
        assert_eq!(paths[1].radius, 4.0);
        assert_eq!(paths[1].flags, 1);
    }

    #[test]
    fn a_level_without_rails_has_none() {
        let mut w = Writer::default();
        w.at(0x100);
        assert!(
            Gameplay { bytes: w.bytes }
                .grind_paths()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_rail_count_past_the_file_is_an_error() {
        let mut file = gameplay(&[&[[1.0, 2.0, 3.0]]]);
        file.bytes[0x100..0x104].copy_from_slice(&0x1000_0000u32.to_le_bytes());
        assert!(file.grind_paths().is_err());
        let mut file = gameplay(&[&[[1.0, 2.0, 3.0]]]);
        let spline = file.bytes.len() - 0x20;
        file.bytes[spline..spline + 4].copy_from_slice(&0x1000_0000u32.to_le_bytes());
        assert!(file.grind_paths().is_err());
    }
}
