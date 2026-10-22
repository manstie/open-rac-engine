//! The games' textures as the GS draws them: 8-bit indexed images with a 256-colour
//! palette (PLAN.md P3.2).
//!
//! Read on UYA's armor textures (`docs/research/moby-format.md`, "Armor").

use crate::{FormatError, Image};

/// "PIF2", the header of the armor's images.
const PIF2: u32 = u32::from_le_bytes(*b"2FIP");
/// The GS's 8-bit indexed pixel format.
const PSMT8: u32 = 0x13;

/// An indexed image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    /// One palette index per pixel, row by row from the top.
    pub pixels: Vec<u8>,
    /// RGBA by index, alpha 0x80 opaque as the GS has it.
    pub palette: Vec<[u8; 4]>,
}

impl Texture {
    /// Reads a PIF2 image at `at`: the magic, a word we don't use, the width and height,
    /// the pixel format (+0x10, 8-bit only so far) and the mipmap count (+0x1C), then the
    /// palette at +0x20 and the pixels after it. The palette is stored in the GS's CSM1
    /// order, which swaps entries 8–15 and 16–23 of every 32.
    pub fn read_pif(image: &Image, at: u32) -> Result<Self, FormatError> {
        let invalid = |detail: String| FormatError::Invalid {
            what: "PIF2 image",
            address: at,
            detail,
        };
        if image.u32(at)? != PIF2 {
            return Err(invalid("no PIF2 magic".into()));
        }
        let (width, height) = (image.u32(at + 8)?, image.u32(at + 0xC)?);
        let format = image.u32(at + 0x10)?;
        if format != PSMT8 {
            return Err(invalid(format!("pixel format {format:#x}")));
        }
        if !(1..=1024).contains(&width) || !(1..=1024).contains(&height) {
            return Err(invalid(format!("{width} by {height}")));
        }
        let palette = csm1_palette(image.bytes(at + 0x20, 0x400)?);
        let pixels = image.bytes(at + 0x420, (width * height) as usize)?.to_vec();
        Ok(Self {
            width,
            height,
            pixels,
            palette,
        })
    }

    /// The image as RGBA8, with alpha doubled so the GS's 0x80 is opaque.
    pub fn rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &index in &self.pixels {
            let [r, g, b, a] = self.palette[index as usize];
            out.extend([r, g, b, (a as u16 * 2).min(255) as u8]);
        }
        out
    }
}

/// A 256-colour palette stored in the GS's CSM1 order (0x400 bytes), by index.
pub(crate) fn csm1_palette(stored: &[u8]) -> Vec<[u8; 4]> {
    (0..=255u8)
        .map(|index| {
            let slot = csm1(index) as usize * 4;
            [
                stored[slot],
                stored[slot + 1],
                stored[slot + 2],
                stored[slot + 3],
            ]
        })
        .collect()
}

/// Where the GS keeps palette entry `index` in a CSM1 palette: bits 3 and 4 swapped.
pub fn csm1(index: u8) -> u8 {
    (index & 0xE7) | ((index & 0x10) >> 1) | ((index & 0x08) << 1)
}

/// Puts the pixels of an 8-bit indexed image back in rows from the top when they are
/// stored as the GS lays them out, ready to be sent as a 32-bit image of half the width
/// and half the height. DL stores the level textures it keeps in EE memory this way.
///
/// The GS keeps both formats in blocks of 256 bytes, in the same order: 16×16 pixels of
/// the 8-bit image are 8×8 of the 32-bit one. So a pixel stays in its block, and only its
/// place in the block changes. Within a block, 8-bit pixel (x, y) is in 32-bit row
/// (y / 4) × 2 + y mod 2, in word (x + s) mod 8 of that row, where s is 4 when y mod 8 is
/// 2 to 5 and 0 otherwise, and in byte (y / 2) mod 2 + 2 × ((x / 8) mod 2) of the word.
/// The width must be a multiple of 16 and the height of 4.
pub fn unswizzle_psmt8(stored: &[u8], width: u32, height: u32) -> Result<Vec<u8>, FormatError> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || !w.is_multiple_of(16) || !h.is_multiple_of(4) || stored.len() < w * h {
        return Err(FormatError::Invalid {
            what: "GS-ordered texture",
            address: 0,
            detail: format!("{width} by {height} from {} bytes", stored.len()),
        });
    }
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            // 32-bit rows are 2w bytes, and a block's top-left corner is 8 of them per 16
            // 8-bit rows, and 32 bytes per 16 8-bit columns.
            let block = (y & !0xF) * w + (x & !0xF) * 2;
            let row = (y & 0xF) / 4 * 2 + y % 2;
            let shift = if (2..6).contains(&(y % 8)) { 4 } else { 0 };
            let word = (x + shift) % 8;
            let byte = (y / 2) % 2 + 2 * ((x / 8) % 2);
            pixels.push(stored[block + row * w * 2 + word * 4 + byte]);
        }
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2 by 1 PIF2 image whose pixels use entries 8 and 16.
    fn pif() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x420 + 2];
        bytes[0..4].copy_from_slice(b"2FIP");
        bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&PSMT8.to_le_bytes());
        // Entry 8 is stored in slot 16 and entry 16 in slot 8.
        bytes[0x20 + 16 * 4..0x20 + 16 * 4 + 4].copy_from_slice(&[255, 0, 0, 0x80]);
        bytes[0x20 + 8 * 4..0x20 + 8 * 4 + 4].copy_from_slice(&[0, 0, 255, 0x40]);
        bytes[0x420] = 8;
        bytes[0x421] = 16;
        bytes
    }

    #[test]
    fn palettes_are_read_in_csm1_order() {
        assert_eq!(csm1(8), 16);
        assert_eq!(csm1(16), 8);
        assert_eq!(csm1(7), 7);
        assert_eq!(csm1(0x29), 0x31);
        let bytes = pif();
        let texture = Texture::read_pif(&Image::new(&bytes, 0), 0).unwrap();
        assert_eq!(texture.rgba(), [255, 0, 0, 255, 0, 0, 255, 128]);
    }

    #[test]
    fn gs_ordered_pixels_come_back_in_rows() {
        // Each stored byte names where it sits, so the result shows which byte each pixel
        // came from: a 32 by 4 image is two blocks side by side, four 8-bit rows each.
        let (w, h) = (32usize, 4usize);
        let stored: Vec<u8> = (0..(w * h) as u32).map(|i| i as u8).collect();
        let pixels = unswizzle_psmt8(&stored, w as u32, h as u32).unwrap();
        let at = |x: usize, y: usize| pixels[y * w + x] as usize;
        // Pixels 0 and 8 of row 0 are bytes 0 and 2 of the first word, and row 2 shares
        // the words of row 0 four words along, in bytes 1 and 3.
        assert_eq!([at(0, 0), at(8, 0), at(0, 2), at(8, 2)], [0, 2, 17, 19]);
        // Pixel 1 of row 0 is the next word, pixel 4 the fifth; row 1 is the next 32-bit
        // row, 2w = 64 bytes on.
        assert_eq!([at(1, 0), at(4, 0), at(0, 1)], [4, 16, 64]);
        // The second block starts 32 bytes in.
        assert_eq!(at(16, 0), 32);
        // Every stored byte lands somewhere, once.
        let mut sorted = pixels.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, stored);
        // A larger image is a permutation too.
        let stored: Vec<u8> = (0..64 * 32).map(|i: u32| (i % 251) as u8).collect();
        let mut sorted = unswizzle_psmt8(&stored, 64, 32).unwrap();
        let mut expected = stored.clone();
        sorted.sort_unstable();
        expected.sort_unstable();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn gs_ordered_sizes_must_fill_blocks() {
        assert!(unswizzle_psmt8(&[0; 64], 8, 8).is_err());
        assert!(unswizzle_psmt8(&[0; 64], 16, 4).is_ok());
        assert!(unswizzle_psmt8(&[0; 63], 16, 4).is_err());
    }

    #[test]
    fn other_formats_are_errors() {
        let mut bytes = pif();
        bytes[16] = 0x14;
        assert!(Texture::read_pif(&Image::new(&bytes, 0), 0).is_err());
        bytes[0] = b'X';
        assert!(Texture::read_pif(&Image::new(&bytes, 0), 0).is_err());
    }
}
