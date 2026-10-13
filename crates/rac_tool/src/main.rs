//! `openrac-tool`: extracts and inspects the games' data (PLAN.md P2.7).
//!
//! Everything it writes is derived from the user's own disc, so it goes in `cache/`, never
//! the repository. `hero` reads the disc itself; `moby` reads EE memory images made from
//! savestates (`tools/ghidra/import-savestate.sh` writes them), for reverse engineering.

mod glb;
mod pose;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use rac_formats::level::Core;
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
    }
    Ok(())
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
