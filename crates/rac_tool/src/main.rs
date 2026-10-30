//! `openrac-tool`: extracts and inspects the games' data (PLAN.md P2.7).
//!
//! Everything it writes is derived from the user's own disc, so it goes in `cache/`, never
//! the repository. `identify`, `hero`, `rails`, `cameras`, `mobys` and `unpack-elf` read
//! the disc itself, as does `sounds`, which can also render a bank's sounds to WAV files; `moby`
//! reads EE memory images made from savestates
//! (`tools/ghidra/import-savestate.sh` writes them), for reverse engineering.

mod cameras;
mod glb;
mod identify;
mod mobys;
mod pose;
mod sounds;
mod unpack;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use rac_formats::gameplay::{Gameplay, GrindPath};
use rac_formats::level::{Core, Toc};
use rac_formats::moby::{MobyClass, PacketKind};
use rac_formats::{DiscReader, FormatError, Image};
use rac_iso::Disc;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Say which release each disc image is: game, region, serial and build, from its
    /// SYSTEM.CNF and its boot executable's SHA-1.
    Identify {
        /// Disc images (.iso), or folders to search for them two levels deep.
        #[arg(default_value = "source")]
        paths: Vec<PathBuf>,
    },
    /// Describe a moby class in an EE memory image, and optionally write it as a binary
    /// glTF with its skeleton, mesh and animations.
    Moby {
        /// The EE memory image (32 MB, from a savestate).
        #[arg(long)]
        ee: PathBuf,
        /// A moby's address, in hex; its class is read.
        #[arg(long, value_parser = parse_hex, conflicts_with = "class")]
        moby: Option<u32>,
        /// A class's address, in hex.
        #[arg(long, value_parser = parse_hex)]
        class: Option<u32>,
        /// Write the class here as a binary glTF (.glb).
        #[arg(long)]
        glb: Option<PathBuf>,
        /// List every sequence.
        #[arg(long)]
        sequences: bool,
    },
    /// Read the hero's skeleton and animations from a UYA disc image, and optionally
    /// write them as a binary glTF.
    Hero {
        /// The disc image (.iso).
        #[arg(long)]
        iso: PathBuf,
        /// The level whose core to read them from (1 is Veldin).
        #[arg(long, default_value_t = 1)]
        level: usize,
        /// Write the skeleton and animations here as a binary glTF (.glb).
        #[arg(long)]
        glb: Option<PathBuf>,
        /// List every sequence.
        #[arg(long)]
        sequences: bool,
        /// Check them against the hero the game loaded, in this EE memory image of the same
        /// level.
        #[arg(long)]
        check_ee: Option<PathBuf>,
    },
    /// List the rails the hero can grind in each level of a UYA disc image, from the
    /// levels' gameplay files.
    Rails {
        /// The disc image (.iso).
        #[arg(long)]
        iso: PathBuf,
        /// Only this level, with every point of its rails.
        #[arg(long)]
        level: Option<usize>,
    },
    /// List the cameras, camera volumes, shapes and paths of each level of a UYA disc
    /// image, from the levels' gameplay files.
    Cameras {
        /// The disc image (.iso).
        #[arg(long)]
        iso: PathBuf,
        /// Only this level, with every camera's and volume's fields.
        #[arg(long)]
        level: Option<usize>,
        /// Check them against what the game loaded, in this EE memory image of the same
        /// level.
        #[arg(long, requires = "level")]
        check_ee: Option<PathBuf>,
    },
    /// List the mobys placed in each level of a UYA or DL disc image, from the levels'
    /// gameplay files, with their pvars.
    Mobys {
        /// The disc image (.iso).
        #[arg(long)]
        iso: PathBuf,
        /// Only this level.
        #[arg(long)]
        level: Option<usize>,
        /// Only mobys of this class, in hex; without --level, only the levels that have one.
        #[arg(long, value_parser = parse_hex)]
        class: Option<u32>,
    },
    /// List a UYA sound bank's sounds and the class sound tables that play them: a gun's
    /// bank from the gadget WAD, or a level's; optionally render each sound to a WAV file.
    Sounds {
        /// The disc image (.iso).
        #[arg(long)]
        iso: PathBuf,
        /// A gun's class number, in hex (a weapon row's +0x10).
        #[arg(long, value_parser = parse_hex, conflicts_with = "level")]
        gadget: Option<u32>,
        /// A level's number (1 is Veldin).
        #[arg(long)]
        level: Option<usize>,
        /// Write each sound here as `sound-NNN.wav` (under cache/).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// List a UYA level core's effect textures, and optionally write each as an RGBA PAM
    /// image (`fx-NNN.pam`, which ffmpeg reads) to look at.
    FxTextures {
        /// The disc image (.iso).
        #[arg(long)]
        iso: PathBuf,
        /// The level whose core to read them from (1 is Veldin).
        #[arg(long, default_value_t = 1)]
        level: usize,
        /// Write them here (under cache/).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Unpack a packed executable (R&C3, UYA, Deadlocked, Gladiator) from a disc image,
    /// list its sections, and optionally write it as a plain ELF for Ghidra.
    UnpackElf {
        /// The disc image (.iso).
        #[arg(long)]
        iso: PathBuf,
        /// The executable to unpack, such as I5BOOTN.ELF; by default the one the disc boots.
        #[arg(long)]
        file: Option<String>,
        /// Write the ELF here (under cache/).
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// A disc image for `rac_formats` to read.
struct IsoReader(Disc<BufReader<File>>);

impl DiscReader for IsoReader {
    fn read(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        self.0
            .bytes(offset, len as u64)
            .map_err(|error| FormatError::Read(error.to_string()))
    }
}

fn open_disc(path: &Path) -> Result<IsoReader> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    Ok(IsoReader(Disc::open(BufReader::new(file))?))
}

fn write_glb(class: &MobyClass, name: &str, path: &Path) -> Result<()> {
    let written = glb::write(class, name)?;
    std::fs::write(path, &written.bytes).with_context(|| format!("writing {}", path.display()))?;
    println!(
        "wrote {} ({} triangles, {} animations, {} bytes)",
        path.display(),
        written.triangles,
        written.animations,
        written.bytes.len()
    );
    Ok(())
}

/// Compares the hero read from the disc with the one the game loaded: every joint and
/// every keyframe must agree.
fn check_against_memory(disc: &MobyClass, ee: &Path) -> Result<()> {
    /// Where UYA NTSC keeps its hero struct; his moby is at + 0x161C.
    const HERO_MOBY: u32 = 0x001A_4BC0 + 0x161C;
    let bytes = std::fs::read(ee).with_context(|| format!("reading {}", ee.display()))?;
    let image = Image::ee(&bytes);
    let moby = image.u32(HERO_MOBY)?;
    let loaded = MobyClass::read(&image, image.u32(moby + 0x24)?)?;
    if loaded.joints != disc.joints {
        bail!("the skeletons differ");
    }
    if loaded.sequences != disc.sequences {
        let first = loaded
            .sequences
            .iter()
            .zip(&disc.sequences)
            .position(|(a, b)| a != b);
        bail!("the sequences differ, first at {first:?}");
    }
    println!(
        "matches the hero in {}: {} joints, {} sequences",
        ee.display(),
        disc.joints.len(),
        disc.sequences.iter().flatten().count()
    );
    Ok(())
}

fn parse_hex(text: &str) -> Result<u32, String> {
    let digits = text.trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(digits, 16).map_err(|error| format!("{text}: {error}"))
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Identify { paths } => identify::run(&paths)?,
        Command::Moby {
            ee,
            moby,
            class,
            glb,
            sequences,
        } => {
            let bytes = std::fs::read(&ee).with_context(|| format!("reading {}", ee.display()))?;
            let image = Image::ee(&bytes);
            let address = match (moby, class) {
                (Some(moby), _) => image.u32(moby + 0x24)?,
                (None, Some(class)) => class,
                (None, None) => bail!("give --moby or --class"),
            };
            let class = MobyClass::read(&image, address)
                .with_context(|| format!("reading the class at {address:#x}"))?;
            describe(address, &class, sequences);
            if let Some(path) = glb {
                write_glb(&class, &format!("class_{address:x}"), &path)?;
            }
        }
        Command::Hero {
            iso,
            level,
            glb,
            sequences,
            check_ee,
        } => {
            let mut disc = open_disc(&iso)?;
            let core = Core::read_uya(&mut disc, level)
                .with_context(|| format!("reading level {level}'s core"))?;
            let hero = core.hero().context("reading the hero's class")?;
            let entry = core.header.moby_class(0).expect("hero() found it");
            println!(
                "level {level}: core of {} bytes, {} moby classes",
                core.bytes.len(),
                core.header.moby_classes.len()
            );
            describe(entry.offset, &hero, sequences);
            if let Some(ee) = check_ee {
                check_against_memory(&hero, &ee)?;
            }
            if let Some(path) = glb {
                write_glb(&hero, "hero", &path)?;
            }
        }
        Command::FxTextures { iso, level, out } => {
            let mut disc = open_disc(&iso)?;
            let core = Core::read_uya(&mut disc, level)
                .with_context(|| format!("reading level {level}'s core"))?;
            let textures = core.fx_textures().context("reading the effect textures")?;
            if let Some(out) = &out {
                std::fs::create_dir_all(out)
                    .with_context(|| format!("creating {}", out.display()))?;
            }
            for (index, texture) in textures.iter().enumerate() {
                println!("{index:3}: {} by {}", texture.width, texture.height);
                if let Some(out) = &out {
                    let path = out.join(format!("fx-{index:03}.pam"));
                    let mut bytes = format!(
                        "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
                        texture.width, texture.height
                    )
                    .into_bytes();
                    bytes.extend(texture.rgba());
                    std::fs::write(&path, bytes)
                        .with_context(|| format!("writing {}", path.display()))?;
                }
            }
        }
        Command::Rails { iso, level } => {
            let mut disc = open_disc(&iso)?;
            let levels = match level {
                Some(level) => vec![level],
                None => (0..Toc::read_uya_disc(&mut disc)?.levels.len()).collect(),
            };
            for number in levels {
                let gameplay = match Gameplay::read_uya(&mut disc, number) {
                    Ok(gameplay) => gameplay,
                    // Gaps in the level table.
                    Err(_) if level.is_none() => continue,
                    Err(error) => return Err(error.into()),
                };
                let paths = gameplay
                    .grind_paths()
                    .with_context(|| format!("reading level {number}'s grind paths"))?;
                describe_rails(number, &paths, level.is_some());
            }
        }
        Command::Cameras {
            iso,
            level,
            check_ee,
        } => cameras::run(&iso, level, check_ee.as_deref())?,
        Command::Mobys { iso, level, class } => mobys::run(&iso, level, class)?,
        Command::Sounds {
            iso,
            gadget,
            level,
            out,
        } => sounds::run(&iso, gadget, level, out.as_deref())?,
        Command::UnpackElf { iso, file, out } => {
            unpack::run(&iso, file.as_deref(), out.as_deref())?;
        }
    }
    Ok(())
}

fn describe_rails(level: usize, paths: &[GrindPath], every_point: bool) {
    println!("level {level}: {} rails", paths.len());
    for (number, path) in paths.iter().enumerate() {
        let length: f32 = path
            .points
            .windows(2)
            .map(|pair| {
                let [a, b] = [pair[0], pair[1]];
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
            })
            .sum();
        let ends = path
            .points
            .first()
            .zip(path.points.last())
            .map(|(a, b)| format!("from {a:.2?} to {b:.2?}"))
            .unwrap_or_default();
        println!(
            "  rail {number}: {} points, {length:.2} u {ends}, flags {:#x}",
            path.points.len(),
            path.flags
        );
        if every_point {
            for point in &path.points {
                println!("    {point:.3?}");
            }
        }
    }
}

fn describe(address: u32, class: &MobyClass, list_sequences: bool) {
    let high = class
        .packets
        .iter()
        .filter(|p| p.kind == PacketKind::High)
        .count();
    let used = class.sequences.iter().flatten().count();
    println!(
        "class {address:#x}: scale {}, {} joints, {used} of {} sequences, {high} + {} packets",
        class.scale,
        class.joints.len(),
        class.sequences.len(),
        class.packets.len() - high,
    );
    if !list_sequences {
        return;
    }
    for (number, sequence) in class.sequences.iter().enumerate() {
        let Some(sequence) = sequence else { continue };
        let ticks: f32 = sequence.keyframes.iter().map(|k| k.ticks()).sum();
        let stamps = sequence
            .keyframes
            .first()
            .zip(sequence.keyframes.last())
            .map(|(a, b)| format!("{}..{}", a.stamp as f32 / 16.0, b.stamp as f32 / 16.0))
            .unwrap_or_default();
        let scaled: Vec<u8> = sequence
            .keyframes
            .first()
            .map(|k| k.scales.iter().map(|s| s.joint).collect())
            .unwrap_or_default();
        println!(
            "  seq {number:3}: {:3} keyframes, {:5.1} ticks, frames {stamps}, scales joints {scaled:?}",
            sequence.keyframes.len(),
            ticks,
        );
    }
}
