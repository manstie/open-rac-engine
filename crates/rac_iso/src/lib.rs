//! Reads the user's disc images (PLAN.md P2.1): raw 2048-byte sectors, and the files the
//! ISO9660 file system lists; and tells which release each image is ([`Release`], P2.2).
//!
//! Most game data isn't in the file system at all: R&C3, UYA, Deadlocked and Gladiator
//! reach it by sector numbers from their own table of contents
//! (`docs/research/disc-layout.md`). So the sector reads are the main interface, and the
//! file system is only needed for `SYSTEM.CNF` and the boot executable.
//!
//! Nothing is read until it's asked for, so a 4 GB image is never loaded whole.

mod release;

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub use release::{Game, IdentifyError, Region, Release, ReleaseInfo, SystemCnf};

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

/// The disc images (`.iso` files) in `folder` and in the folders inside it, down to `depth`
/// levels, sorted by path.
pub fn find_images(folder: &Path, depth: u32) -> Vec<PathBuf> {
    fn collect(folder: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth > 0 {
                    collect(&path, depth - 1, out);
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("iso"))
            {
                out.push(path);
            }
        }
    }
    let mut images = Vec::new();
    collect(folder, depth, &mut images);
    images.sort();
    images
}

/// Tiny disc images for tests.
#[cfg(test)]
mod fixture {
    use super::SECTOR;

    /// A directory record.
    pub fn record(name: &[u8], sector: u32, size: u32, dir: bool) -> Vec<u8> {
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

    /// An image of `sectors` sectors whose primary volume descriptor names a root
    /// directory at sector 18.
    pub fn blank(sectors: usize) -> Vec<u8> {
        let mut image = vec![0u8; sectors * SECTOR as usize];
        let pvd = 16 * SECTOR as usize;
        image[pvd] = 1;
        image[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        image[pvd + 40..pvd + 72].copy_from_slice(b"TEST_VOLUME                     ");
        let root = record(&[0], 18, SECTOR as u32, true);
        image[pvd + 156..pvd + 156 + root.len()].copy_from_slice(&root);
        image
    }

    /// An image whose root directory holds `files`, one sector each from sector 19.
    pub fn image(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut image = blank(19 + files.len());
        let mut dir = Vec::new();
        dir.extend(record(&[0], 18, SECTOR as u32, true));
        dir.extend(record(&[1], 18, SECTOR as u32, true));
        for (n, (name, data)) in files.iter().enumerate() {
            let sector = 19 + n;
            let name = format!("{name};1");
            dir.extend(record(
                name.as_bytes(),
                sector as u32,
                data.len() as u32,
                false,
            ));
            let at = sector * SECTOR as usize;
            image[at..at + data.len()].copy_from_slice(data);
        }
        let root = 18 * SECTOR as usize;
        image[root..root + dir.len()].copy_from_slice(&dir);
        image
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{blank, record};
    use super::*;
    use std::io::Cursor;

    /// A tiny image: the root holds `SYSTEM.CNF;1` and a directory `G` holding `A.WAD;1`.
    fn image() -> Vec<u8> {
        let mut image = blank(24);
        let sector = |n: usize| n * SECTOR as usize;
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
    fn finds_images_down_to_a_depth() {
        let root = std::env::temp_dir().join(format!("rac_iso_images_{}", std::process::id()));
        let deeper = root.join("game").join("deeper");
        std::fs::create_dir_all(&deeper).unwrap();
        for file in [
            "a.iso",
            "B.ISO",
            "notes.txt",
            "game/c.iso",
            "game/deeper/d.iso",
        ] {
            std::fs::write(root.join(file), b"").unwrap();
        }
        let found = find_images(&root, 1);
        std::fs::remove_dir_all(&root).unwrap();
        let names: Vec<_> = found
            .iter()
            .map(|path| path.strip_prefix(&root).unwrap().to_path_buf())
            .collect();
        let expected: Vec<PathBuf> = ["B.ISO", "a.iso", "game/c.iso"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn other_files_are_not_iso_images() {
        let error = Disc::open(Cursor::new(vec![0u8; 20 * SECTOR as usize])).unwrap_err();
        assert!(matches!(error, IsoError::NotIso));
    }
}
