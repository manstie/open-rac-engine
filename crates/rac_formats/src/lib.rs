//! Parsers for the games' data formats (PLAN.md §4.3, Phase 3).
//!
//! Nothing here depends on Bevy, so tools and tests can use it directly. Everything reads
//! from an [`Image`]: a block of game data addressed the way its own pointers are, either
//! an EE memory image (for reverse engineering, before the disc layer exists) or data
//! read from the disc.
//!
//! - [`moby`]: moby classes, with their skeleton, animation and mesh
//!   (`docs/research/moby-format.md`).
//! - [`armor`]: UYA's armor, the hero's mesh and textures.
//! - [`gadget`]: UYA's gadget WAD, the classes of the guns and other items held in hand.
//! - [`elf`]: the boot executables, unpacked from their stub and written as plain ELFs.
//! - [`texture`]: indexed images as the GS draws them.
//! - [`wad`]: the games' LZ compression.
//! - [`level`]: UYA's table of contents and level files, down to the moby classes.
//! - [`gameplay`]: a level's gameplay file: its grind paths so far.
//!
//! Bad data gives a [`FormatError`], never a panic: users' discs can be damaged or of a
//! release we haven't seen.

pub mod armor;
pub mod elf;
pub mod gadget;
pub mod gameplay;
pub mod image;
pub mod level;
pub mod moby;
pub mod texture;
pub mod wad;

pub use image::Image;

/// Why some data couldn't be read.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FormatError {
    /// The data points outside the bytes we have.
    #[error("{len} bytes at {address:#x} are outside the data")]
    OutOfBounds { address: u32, len: usize },
    /// The data is there but doesn't make sense.
    #[error("{what} at {address:#x}: {detail}")]
    Invalid {
        what: &'static str,
        address: u32,
        detail: String,
    },
    /// The disc couldn't be read.
    #[error("reading the disc: {0}")]
    Read(String),
}

/// Reads byte ranges of a disc image; `rac_iso::Disc` does it for real images.
pub trait DiscReader {
    fn read(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError>;
}
