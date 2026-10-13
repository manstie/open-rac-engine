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
        let stored = image.bytes(at + 0x20, 0x400)?;
        let palette = (0..=255u8)
            .map(|index| {
                let slot = csm1(index) as usize * 4;
                [
                    stored[slot],
                    stored[slot + 1],
                    stored[slot + 2],
                    stored[slot + 3],
                ]
            })
            .collect();
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

/// Where the GS keeps palette entry `index` in a CSM1 palette: bits 3 and 4 swapped.
pub fn csm1(index: u8) -> u8 {
    (index & 0xE7) | ((index & 0x10) >> 1) | ((index & 0x08) << 1)
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
    fn other_formats_are_errors() {
        let mut bytes = pif();
        bytes[16] = 0x14;
        assert!(Texture::read_pif(&Image::new(&bytes, 0), 0).is_err());
        bytes[0] = b'X';
        assert!(Texture::read_pif(&Image::new(&bytes, 0), 0).is_err());
    }
}
