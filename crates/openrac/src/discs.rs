//! Finding the user's discs (a first part of PLAN.md P2.2): disc images in a folder,
//! recognised by the boot executable their `SYSTEM.CNF` names.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use rac_formats::{DiscReader, FormatError};
use rac_iso::Disc;

/// The releases the app can use so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    /// Ratchet & Clank: Up Your Arsenal, NTSC-U.
    UyaNtsc,
}

impl Release {
    /// The boot executable `SYSTEM.CNF` names (`docs/research/disc-layout.md`).
    fn boot(self) -> &'static str {
        match self {
            Release::UyaNtsc => "SCUS_973.53",
        }
    }
}

/// An opened disc image, for `rac_formats` to read.
pub struct DiscImage(Disc<BufReader<File>>);

impl DiscReader for DiscImage {
    fn read(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        self.0
            .bytes(offset, len as u64)
            .map_err(|error| FormatError::Read(error.to_string()))
    }
}

/// Which release the image at `path` is, if it's one the app knows.
fn identify(path: &Path) -> Option<(Release, DiscImage)> {
    let file = File::open(path).ok()?;
    let mut disc = Disc::open(BufReader::new(file)).ok()?;
    let cnf = disc.find("SYSTEM.CNF").ok()??;
    let text = disc.read_file(&cnf).ok()?;
    let text = String::from_utf8_lossy(&text);
    [Release::UyaNtsc]
        .into_iter()
        .find(|release| text.contains(release.boot()))
        .map(|release| (release, DiscImage(disc)))
}

/// The first disc image of `release` in `folder` or the folders inside it.
pub fn find(folder: &Path, release: Release) -> Option<(PathBuf, DiscImage)> {
    let mut images = Vec::new();
    collect_images(folder, 2, &mut images);
    images.sort();
    images.into_iter().find_map(|path| {
        let (found, disc) = identify(&path)?;
        (found == release).then_some((path, disc))
    })
}

fn collect_images(folder: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth > 0 {
            collect_images(&path, depth - 1, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("iso"))
        {
            out.push(path);
        }
    }
}
