//! Reads the user's disc images (PLAN.md P2.1): raw 2048-byte sectors, and the files the
//! ISO9660 file system lists.
//!
//! Most game data isn't in the file system at all: R&C3, UYA, Deadlocked and Gladiator
//! reach it by sector numbers from their own table of contents
//! (`docs/research/disc-layout.md`). So the sector reads are the main interface, and the
//! file system is only needed for `SYSTEM.CNF` and the boot executable.
//!
//! Nothing is read until it's asked for, so a 4 GB image is never loaded whole.

use std::io::{Read, Seek, SeekFrom};

/// Bytes per sector.
pub const SECTOR: u64 = 2048;
/// The primary volume descriptor's sector.
const DESCRIPTORS: u64 = 16;

/// Why the disc couldn't be read.
#[derive(Debug, thiserror::Error)]
pub enum IsoError {
    #[error("reading the disc image: {0}")]
    Io(#[from] std::io::Error),
    #[error("no ISO9660 primary volume descriptor")]
    NotIso,
    #[error("a directory record at sector {sector} is damaged")]
    BadRecord { sector: u64 },
}

/// Where a file lies on the disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub sector: u64,
    pub size: u64,
}

/// One entry of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// Without the `;1` version suffix.
    pub name: String,
    pub extent: Extent,
    pub is_dir: bool,
}

/// A disc image opened for reading.
#[derive(Debug)]
pub struct Disc<R> {
    reader: R,
    root: Extent,
    volume: String,
}

impl<R: Read + Seek> Disc<R> {
    /// Opens an ISO9660 image and reads its primary volume descriptor.
    pub fn open(mut reader: R) -> Result<Self, IsoError> {
        let mut pvd = vec![0; SECTOR as usize];
        reader.seek(SeekFrom::Start(DESCRIPTORS * SECTOR))?;
        reader.read_exact(&mut pvd)?;
        if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
            return Err(IsoError::NotIso);
        }
        let volume = String::from_utf8_lossy(&pvd[40..72]).trim_end().to_string();
        let root = record_extent(&pvd[156..190]);
        Ok(Self {
            reader,
            root,
            volume,
        })
    }

    /// The volume's name, as the disc gives it.
    pub fn volume(&self) -> &str {
        &self.volume
    }

    /// `count` sectors from `sector` on.
    pub fn sectors(&mut self, sector: u64, count: u64) -> Result<Vec<u8>, IsoError> {
        self.bytes(sector * SECTOR, count * SECTOR)
    }

    /// `len` bytes from byte `offset` of the image.
    pub fn bytes(&mut self, offset: u64, len: u64) -> Result<Vec<u8>, IsoError> {
        let mut out = vec![0; len as usize];
        self.reader.seek(SeekFrom::Start(offset))?;
        self.reader.read_exact(&mut out)?;
        Ok(out)
    }

    /// The entries of the directory at `extent`.
    pub fn list(&mut self, extent: Extent) -> Result<Vec<DirEntry>, IsoError> {
        let data = self.bytes(extent.sector * SECTOR, extent.size)?;
        let mut entries = Vec::new();
        let mut at = 0;
        while at < data.len() {
            let length = data[at] as usize;
            if length == 0 {
                // Records don't cross sectors; the rest of this one is empty.
                at = (at / SECTOR as usize + 1) * SECTOR as usize;
                continue;
            }
            let sector = extent.sector + (at as u64) / SECTOR;
            let record = data
                .get(at..at + length)
                .filter(|r| r.len() >= 34)
                .ok_or(IsoError::BadRecord { sector })?;
            let name_length = record[32] as usize;
            let name = record
                .get(33..33 + name_length)
                .ok_or(IsoError::BadRecord { sector })?;
            // 0 and 1 are the directory itself and its parent.
            if name != [0] && name != [1] {
                let name = String::from_utf8_lossy(name);
                let name = name.split(';').next().unwrap_or_default();
                entries.push(DirEntry {
                    name: name.trim_end_matches('.').to_string(),
                    extent: record_extent(record),
                    is_dir: record[25] & 2 != 0,
                });
            }
            at += length;
        }
        Ok(entries)
    }

    /// The file at `path` (`/`-separated, any case), if there is one.
    pub fn find(&mut self, path: &str) -> Result<Option<DirEntry>, IsoError> {
        let mut dir = self.root;
        let mut found = None;
        for part in path.split('/').filter(|p| !p.is_empty()) {
            let entries = self.list(dir)?;
            let Some(entry) = entries
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case(part))
            else {
                return Ok(None);
            };
            dir = entry.extent;
            found = Some(entry);
        }
        Ok(found)
    }

    /// The whole of a file.
    pub fn read_file(&mut self, entry: &DirEntry) -> Result<Vec<u8>, IsoError> {
        self.bytes(entry.extent.sector * SECTOR, entry.extent.size)
    }
}

/// A directory record's extent: little-endian sector at +2, size at +10.
fn record_extent(record: &[u8]) -> Extent {
    let word = |at: usize| {
        u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
    };
    Extent {
        sector: word(2) as u64,
        size: word(10) as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn record(name: &[u8], sector: u32, size: u32, dir: bool) -> Vec<u8> {
        let mut r = vec![0u8; 33];
        r[2..6].copy_from_slice(&sector.to_le_bytes());
        r[6..10].copy_from_slice(&sector.to_be_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[14..18].copy_from_slice(&size.to_be_bytes());
        r[25] = if dir { 2 } else { 0 };
        r[32] = name.len() as u8;
        r.extend_from_slice(name);
        if r.len() % 2 == 1 {
            r.push(0);
        }
        r[0] = r.len() as u8;
        r
    }

    /// A tiny image: the root holds `SYSTEM.CNF;1` and a directory `G` holding `A.WAD;1`.
    fn image() -> Vec<u8> {
        let mut image = vec![0u8; 24 * SECTOR as usize];
        let sector = |n: usize| n * SECTOR as usize;
        let pvd = sector(16);
        image[pvd] = 1;
        image[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        image[pvd + 40..pvd + 72].copy_from_slice(b"TEST_VOLUME                     ");
        let root = record(&[0], 18, SECTOR as u32, true);
        image[pvd + 156..pvd + 156 + root.len()].copy_from_slice(&root);
        let mut dir = Vec::new();
        dir.extend(record(&[0], 18, SECTOR as u32, true));
        dir.extend(record(&[1], 18, SECTOR as u32, true));
        dir.extend(record(b"SYSTEM.CNF;1", 20, 5, false));
        dir.extend(record(b"G", 19, SECTOR as u32, true));
        image[sector(18)..sector(18) + dir.len()].copy_from_slice(&dir);
        let sub = record(b"A.WAD;1", 21, 3, false);
        image[sector(19)..sector(19) + sub.len()].copy_from_slice(&sub);
        image[sector(20)..sector(20) + 5].copy_from_slice(b"BOOT2");
        image[sector(21)..sector(21) + 3].copy_from_slice(b"WAD");
        image
    }

    #[test]
    fn finds_files_by_path_in_any_case() {
        let mut disc = Disc::open(Cursor::new(image())).unwrap();
        assert_eq!(disc.volume(), "TEST_VOLUME");
        let cnf = disc.find("system.cnf").unwrap().unwrap();
        assert_eq!(disc.read_file(&cnf).unwrap(), b"BOOT2");
        let wad = disc.find("/G/A.WAD").unwrap().unwrap();
        assert_eq!(
            wad.extent,
            Extent {
                sector: 21,
                size: 3
            }
        );
        assert!(!wad.is_dir);
        assert_eq!(disc.find("G/MISSING").unwrap(), None);
    }

    #[test]
    fn reads_raw_sectors() {
        let mut disc = Disc::open(Cursor::new(image())).unwrap();
        assert_eq!(&disc.sectors(21, 1).unwrap()[..3], b"WAD");
        assert!(disc.sectors(100, 1).is_err());
    }

    #[test]
    fn other_files_are_not_iso_images() {
        let error = Disc::open(Cursor::new(vec![0u8; 20 * SECTOR as usize])).unwrap_err();
        assert!(matches!(error, IsoError::NotIso));
    }
}
