//! Finding the user's discs (PLAN.md P2.2): disc images in a folder, identified by
//! `rac_iso`'s table of releases.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use bevy::prelude::warn;
use rac_formats::{DiscReader, FormatError};
use rac_iso::{Disc, SystemCnf};

pub use rac_iso::Release;

/// An opened disc image, for `rac_formats` to read.
pub struct DiscImage(Disc<BufReader<File>>);

impl DiscReader for DiscImage {
    fn read(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        self.0
            .bytes(offset, len as u64)
            .map_err(|error| FormatError::Read(error.to_string()))
    }
}

/// The image at `path`, if it's `release`. A disc of the same game in a build we don't
/// know is refused with a warning: every address we use belongs to one build.
fn open_as(path: &Path, release: Release) -> Option<DiscImage> {
    let file = File::open(path).ok()?;
    let mut disc = Disc::open(BufReader::new(file)).ok()?;
    // Only images that boot the release's executable are worth hashing.
    let cnf = SystemCnf::read(&mut disc).ok()?;
    if !cnf.boot.eq_ignore_ascii_case(release.info().boot) {
        return None;
    }
    match Release::identify(&mut disc) {
        Ok(found) => (found == release).then_some(DiscImage(disc)),
        Err(error) => {
            warn!("{}: {error}", path.display());
            None
        }
    }
}

/// The first disc image of `release` in `folder` or the folders inside it.
pub fn find(folder: &Path, release: Release) -> Option<(PathBuf, DiscImage)> {
    rac_iso::find_images(folder, 2)
        .into_iter()
        .find_map(|path| open_as(&path, release).map(|disc| (path, disc)))
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
