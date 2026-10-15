//! Which release a disc is (PLAN.md P2.2): `SYSTEM.CNF` names the boot executable, and
//! the boot executable's SHA-1 pins the build.
//!
//! The serial alone isn't enough: GC USA shipped as v1.01 and v2.00 under one serial, and
//! every address we know belongs to one build. Hashing the boot executable (3 MB at most)
//! is cheap where hashing a whole 4 GB image isn't. The eight releases and their hashes
//! are in `docs/research/disc-layout.md`.

use std::fmt;
use std::io::{Read, Seek};

use crate::{Disc, IsoError};

/// The four games, by their American titles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Game {
    /// Ratchet & Clank (2002).
    RatchetAndClank,
    /// Ratchet & Clank: Going Commando, Ratchet & Clank 2 in Europe.
    GoingCommando,
    /// Ratchet & Clank: Up Your Arsenal, Ratchet & Clank 3 in Europe.
    UpYourArsenal,
    /// Ratchet: Deadlocked, Ratchet: Gladiator in Europe.
    Deadlocked,
}

/// Where a release was sold, which sets its video mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// North America, 60 Hz.
    NtscU,
    /// Europe and Australia, 50 Hz.
    Pal,
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Region::NtscU => "NTSC-U",
            Region::Pal => "PAL",
        })
    }
}

/// A release we know: one build of one game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Release {
    /// Ratchet & Clank, NTSC-U SCUS-97199.
    RacNtsc,
    /// Ratchet & Clank, PAL SCES-50916 v2.00.
    RacPal,
    /// Going Commando, NTSC-U SCUS-97268 v2.00 (Greatest Hits).
    GcNtsc,
    /// Ratchet & Clank 2, PAL SCES-51607.
    GcPal,
    /// Ratchet & Clank 3, PAL SCES-52456.
    Rac3Pal,
    /// Up Your Arsenal, NTSC-U SCUS-97353.
    UyaNtsc,
    /// Deadlocked, NTSC-U SCUS-97465.
    DlNtsc,
    /// Gladiator, PAL SCES-53285.
    GladiatorPal,
}

/// What we know about a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub game: Game,
    pub region: Region,
    /// The title on the release's own box.
    pub title: &'static str,
    /// The serial on the box, `SCUS-97353`.
    pub serial: &'static str,
    /// `SYSTEM.CNF`'s `VER`.
    pub version: &'static str,
    /// The boot executable `SYSTEM.CNF` names, `SCUS_973.53`.
    pub boot: &'static str,
    /// Its SHA-1, in lowercase hex.
    pub boot_sha1: &'static str,
    /// The sector of the game's own table of contents, which finds the data the file system
    /// doesn't list.
    pub toc_sector: u64,
    /// Whether the boot executable is a stub carrying the real program, packed (R&C3
    /// onward; `rac_formats::elf::unpack`).
    pub packed: bool,
}

impl Release {
    pub const ALL: [Release; 8] = [
        Release::RacNtsc,
        Release::RacPal,
        Release::GcNtsc,
        Release::GcPal,
        Release::Rac3Pal,
        Release::UyaNtsc,
        Release::DlNtsc,
        Release::GladiatorPal,
    ];

    pub fn info(self) -> &'static ReleaseInfo {
        // `docs/research/disc-layout.md`, "The eight discs" and "Tables of contents".
        match self {
            Release::RacNtsc => &ReleaseInfo {
                game: Game::RatchetAndClank,
                region: Region::NtscU,
                title: "Ratchet & Clank",
                serial: "SCUS-97199",
                version: "1.00",
                boot: "SCUS_971.99",
                boot_sha1: "72fd1de379dafd8f243dceeb3be50980f3fd3cd5",
                toc_sector: 1500,
                packed: false,
            },
            Release::RacPal => &ReleaseInfo {
                game: Game::RatchetAndClank,
                region: Region::Pal,
                title: "Ratchet & Clank",
                serial: "SCES-50916",
                version: "2.00",
                boot: "SCES_509.16",
                boot_sha1: "79956931bd62fafd8d20fa2eae796dbaf2e15e83",
                toc_sector: 1500,
                packed: false,
            },
            Release::GcNtsc => &ReleaseInfo {
                game: Game::GoingCommando,
                region: Region::NtscU,
                title: "Ratchet & Clank: Going Commando",
                serial: "SCUS-97268",
                version: "2.00",
                boot: "SCUS_972.68",
                boot_sha1: "4124a79c1e5a1ceb56f514df0dbbc11f072af56f",
                toc_sector: 1001,
                packed: false,
            },
            Release::GcPal => &ReleaseInfo {
                game: Game::GoingCommando,
                region: Region::Pal,
                title: "Ratchet & Clank 2",
                serial: "SCES-51607",
                version: "1.00",
                boot: "SCES_516.07",
                boot_sha1: "cb59ce5396ddebb8aa539b0847f8bf66d6a47fe0",
                toc_sector: 1001,
                packed: false,
            },
            Release::Rac3Pal => &ReleaseInfo {
                game: Game::UpYourArsenal,
                region: Region::Pal,
                title: "Ratchet & Clank 3",
                serial: "SCES-52456",
                version: "1.00",
                boot: "SCES_524.56",
                boot_sha1: "e3a8f37d6033b007a80f63dfbfca9afe24f27a47",
                toc_sector: 1001,
                packed: true,
            },
            Release::UyaNtsc => &ReleaseInfo {
                game: Game::UpYourArsenal,
                region: Region::NtscU,
                title: "Ratchet & Clank: Up Your Arsenal",
                serial: "SCUS-97353",
                version: "1.00",
                boot: "SCUS_973.53",
                boot_sha1: "601012bb03ef1213ad1253baebae8a071b8a7df7",
                toc_sector: 1001,
                packed: true,
            },
            Release::DlNtsc => &ReleaseInfo {
                game: Game::Deadlocked,
                region: Region::NtscU,
                title: "Ratchet: Deadlocked",
                serial: "SCUS-97465",
                version: "1.00",
                boot: "SCUS_974.65",
                boot_sha1: "aa91b1c3b9b1a244320c47580b77342ef9856e95",
                toc_sector: 1001,
                packed: true,
            },
            Release::GladiatorPal => &ReleaseInfo {
                game: Game::Deadlocked,
                region: Region::Pal,
                title: "Ratchet: Gladiator",
                serial: "SCES-53285",
                version: "1.00",
                boot: "SCES_532.85",
                boot_sha1: "2ca8d367f625001a4da12fcf466b7d218431bfd0",
                toc_sector: 1001,
                packed: true,
            },
        }
    }

    /// Which release `disc` is.
    pub fn identify<R: Read + Seek>(disc: &mut Disc<R>) -> Result<Release, IdentifyError> {
        let cnf = SystemCnf::read(disc)?;
        let serial = known_serial(&cnf.boot)?;
        let boot = disc
            .find(&cnf.boot)?
            .ok_or_else(|| IdentifyError::MissingBoot {
                boot: cnf.boot.clone(),
            })?;
        let sha1 = sha1_smol::Sha1::from(disc.read_file(&boot)?)
            .digest()
            .to_string();
        Release::ALL
            .into_iter()
            .find(|release| {
                let info = release.info();
                info.boot.eq_ignore_ascii_case(&cnf.boot) && info.boot_sha1 == sha1
            })
            .ok_or(IdentifyError::UnknownBuild {
                serial,
                version: cnf.version,
                sha1,
            })
    }
}

/// The serial of the game whose boot executable is `boot`, if we know one.
fn known_serial(boot: &str) -> Result<&'static str, IdentifyError> {
    Release::ALL
        .into_iter()
        .map(Release::info)
        .find(|info| info.boot.eq_ignore_ascii_case(boot))
        .map(|info| info.serial)
        .ok_or_else(|| IdentifyError::UnknownGame {
            boot: boot.to_owned(),
        })
}

/// Why a disc isn't a release we know.
#[derive(Debug, thiserror::Error)]
pub enum IdentifyError {
    #[error(transparent)]
    Iso(#[from] IsoError),
    #[error("not a PlayStation 2 game: the disc has no SYSTEM.CNF")]
    NoSystemCnf,
    #[error("SYSTEM.CNF names no boot executable")]
    NoBoot,
    #[error("SYSTEM.CNF boots {boot}, which isn't the disc of a game we support")]
    UnknownGame { boot: String },
    #[error("SYSTEM.CNF boots {boot}, which the disc doesn't hold")]
    MissingBoot { boot: String },
    #[error(
        "{serial} {} isn't a build we know: its boot executable's SHA-1 is {sha1}",
        version.as_deref().map_or_else(|| "of an unknown version".to_owned(), |v| format!("v{v}"))
    )]
    UnknownBuild {
        serial: &'static str,
        version: Option<String>,
        sha1: String,
    },
}

/// What a disc's `SYSTEM.CNF` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemCnf {
    /// The boot executable's file name (`SCUS_973.53`), from `BOOT2`.
    pub boot: String,
    /// `VER`, such as `1.00`.
    pub version: Option<String>,
    /// `VMODE`: `NTSC` or `PAL`.
    pub video_mode: Option<String>,
}

impl SystemCnf {
    /// Reads `SYSTEM.CNF` from the disc.
    pub fn read<R: Read + Seek>(disc: &mut Disc<R>) -> Result<Self, IdentifyError> {
        let entry = disc.find("SYSTEM.CNF")?.ok_or(IdentifyError::NoSystemCnf)?;
        let text = disc.read_file(&entry)?;
        Self::parse(&String::from_utf8_lossy(&text)).ok_or(IdentifyError::NoBoot)
    }

    /// `key = value` lines; `BOOT2` is a path such as `cdrom0:\SCUS_973.53;1`. `None` if
    /// there's no `BOOT2`.
    pub fn parse(text: &str) -> Option<Self> {
        let value = |key: &str| {
            text.lines().find_map(|line| {
                let (k, v) = line.split_once('=')?;
                k.trim()
                    .eq_ignore_ascii_case(key)
                    .then(|| v.trim().to_owned())
            })
        };
        let path = value("BOOT2")?;
        let name = path.rsplit(['\\', '/', ':']).next()?;
        let boot = name.split(';').next()?.to_owned();
        (!boot.is_empty()).then(|| Self {
            boot,
            version: value("VER"),
            video_mode: value("VMODE"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use std::collections::HashSet;
    use std::io::Cursor;

    fn identify(files: &[(&str, &[u8])]) -> Result<Release, IdentifyError> {
        Release::identify(&mut Disc::open(Cursor::new(fixture::image(files))).unwrap())
    }

    #[test]
    fn reads_system_cnf() {
        // As the discs write it: CRLF, and spaces after the values on all but R&C1's.
        let text = "BOOT2 = cdrom0:\\SCES_524.56;1 \r\nVER = 1.00 \r\nVMODE = PAL \r\n";
        assert_eq!(
            SystemCnf::parse(text),
            Some(SystemCnf {
                boot: "SCES_524.56".into(),
                version: Some("1.00".into()),
                video_mode: Some("PAL".into()),
            })
        );
        let bare = SystemCnf::parse("BOOT2=cdrom0:\\SLES_123.45").unwrap();
        assert_eq!((bare.boot.as_str(), bare.version), ("SLES_123.45", None));
        assert_eq!(SystemCnf::parse("VER = 1.00\r\nVMODE = NTSC"), None);
        assert_eq!(SystemCnf::parse("BOOT2 = cdrom0:\\"), None);
    }

    #[test]
    fn the_table_is_consistent() {
        let mut boots = HashSet::new();
        for release in Release::ALL {
            let info = release.info();
            // SCUS-97353 boots SCUS_973.53.
            let (prefix, number) = info.serial.split_once('-').unwrap();
            let boot = format!("{prefix}_{}.{}", &number[..3], &number[3..]);
            assert_eq!(info.boot, boot, "{release:?}");
            assert!(boots.insert(info.boot), "{release:?}");
            assert_eq!(info.boot_sha1.len(), 40, "{release:?}");
            assert!(
                info.boot_sha1
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            );
            assert_eq!(
                info.toc_sector == 1500,
                info.game == Game::RatchetAndClank,
                "{release:?}"
            );
            assert_eq!(
                info.packed,
                matches!(info.game, Game::UpYourArsenal | Game::Deadlocked),
                "{release:?}"
            );
        }
    }

    #[test]
    fn hashes_the_boot_executable_it_names() {
        let cnf = b"BOOT2 = cdrom0:\\SCUS_972.68;1\r\nVER = 1.01\r\nVMODE = NTSC\r\n";
        let boot = b"not the v2.00 build";
        let error = identify(&[("SYSTEM.CNF", cnf), ("SCUS_972.68", boot)]).unwrap_err();
        let IdentifyError::UnknownBuild {
            serial,
            version,
            sha1,
        } = &error
        else {
            panic!("{error:?}");
        };
        assert_eq!((*serial, version.as_deref()), ("SCUS-97268", Some("1.01")));
        assert_eq!(*sha1, sha1_smol::Sha1::from(boot).digest().to_string());
        assert!(
            error
                .to_string()
                .starts_with("SCUS-97268 v1.01 isn't a build")
        );
    }

    #[test]
    fn tells_why_a_disc_is_unknown() {
        let error = identify(&[("README.TXT", b"hi")]).unwrap_err();
        assert!(matches!(error, IdentifyError::NoSystemCnf), "{error:?}");

        let error = identify(&[("SYSTEM.CNF", b"VMODE = PAL")]).unwrap_err();
        assert!(matches!(error, IdentifyError::NoBoot), "{error:?}");

        let cnf = b"BOOT2 = cdrom0:\\SLUS_200.00;1";
        let error = identify(&[("SYSTEM.CNF", cnf), ("SLUS_200.00", b"elf")]).unwrap_err();
        assert!(
            matches!(&error, IdentifyError::UnknownGame { boot } if boot == "SLUS_200.00"),
            "{error:?}"
        );

        let cnf = b"BOOT2 = cdrom0:\\SCUS_973.53;1";
        let error = identify(&[("SYSTEM.CNF", cnf)]).unwrap_err();
        assert!(
            matches!(&error, IdentifyError::MissingBoot { boot } if boot == "SCUS_973.53"),
            "{error:?}"
        );
    }
}

/// Reads the real discs: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;

    #[test]
    #[ignore = "needs game discs"]
    fn every_disc_identifies() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let images = crate::find_images(Path::new(&folder), 2);
        assert!(!images.is_empty(), "no disc images in {folder}");
        let mut found = Vec::new();
        for path in images {
            let file = File::open(&path).unwrap();
            let mut disc = Disc::open(BufReader::new(file)).unwrap();
            let release = Release::identify(&mut disc)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let info = release.info();
            let cnf = SystemCnf::read(&mut disc).unwrap();
            assert_eq!(cnf.version.as_deref(), Some(info.version), "{release:?}");
            let video = match info.region {
                Region::NtscU => "NTSC",
                Region::Pal => "PAL",
            };
            assert_eq!(cnf.video_mode.as_deref(), Some(video), "{release:?}");

            // The table of contents starts with its header's size: R&C1's whole header
            // after a version word, then the first global WAD header's
            // (`disc-layout.md`, "Tables of contents").
            let toc = disc.sectors(info.toc_sector, 1).unwrap();
            let word = |at: usize| u32::from_le_bytes(toc[at..at + 4].try_into().unwrap());
            let first = match info.game {
                Game::RatchetAndClank => [1, 0x2960],
                Game::GoingCommando => [0x328, word(4)],
                Game::UpYourArsenal | Game::Deadlocked => [0x648, word(4)],
            };
            assert_eq!([word(0), word(4)], first, "{release:?}");

            // Packed executables are stubs entered at 0x800008.
            let boot = disc.find(info.boot).unwrap().unwrap();
            let elf = disc.read_file(&boot).unwrap();
            let entry = u32::from_le_bytes(elf[0x18..0x1C].try_into().unwrap());
            assert_eq!(entry == 0x0080_0008, info.packed, "{release:?}");
            found.push(release);
        }
        println!("identified {found:?}");
    }
}
