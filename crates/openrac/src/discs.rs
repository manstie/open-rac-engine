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
    /// Ratchet: Deadlocked, NTSC-U.
    DlNtsc,
    /// Ratchet: Gladiator, PAL (Deadlocked's European release).
    GladiatorPal,
}

impl Release {
    /// The boot executable `SYSTEM.CNF` names (`docs/research/disc-layout.md`).
    fn boot(self) -> &'static str {
        match self {
            Release::UyaNtsc => "SCUS_973.53",
            Release::DlNtsc => "SCUS_974.65",
            Release::GladiatorPal => "SCES_532.85",
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
    [Release::UyaNtsc, Release::DlNtsc, Release::GladiatorPal]
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

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use rac_formats::gameplay::Gameplay;

    #[test]
    #[ignore = "needs game discs"]
    fn every_uya_levels_rails_decode() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) = find(Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let levels = rac_formats::level::Toc::read_uya_disc(&mut disc)
            .unwrap()
            .levels;
        let mut counts = Vec::new();
        for (level, files) in levels.iter().enumerate() {
            if files.level.is_empty() {
                continue;
            }
            let rails = Gameplay::read_uya(&mut disc, level)
                .unwrap()
                .grind_paths()
                .unwrap();
            for rail in &rails {
                assert!(rail.points.len() >= 2, "level {level}");
                assert!(rail.points.iter().flatten().all(|v| v.is_finite()));
            }
            if !rails.is_empty() {
                counts.push((level, rails.len()));
            }
        }
        // `docs/research/gameplay-format.md`, "UYA's rails". Florana's (2) and Crash Site's
        // (17) are zip lines.
        assert_eq!(
            counts,
            [
                (2, 3),
                (11, 1),
                (17, 1),
                (19, 1),
                (30, 1),
                (32, 8),
                (33, 1),
                (34, 3),
                (35, 3)
            ]
        );
        let florana = Gameplay::read_uya(&mut disc, 2)
            .unwrap()
            .grind_paths()
            .unwrap();
        assert_eq!(florana[0].points.len(), 15);
        let first = florana[0].points[0];
        assert!((first[0] - 464.756).abs() < 1e-3 && (first[2] - 339.988).abs() < 1e-3);
    }
}
