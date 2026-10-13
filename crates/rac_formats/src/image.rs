//! Little-endian values read from game data at the addresses its pointers use.

use crate::FormatError;

/// A block of game data, read at the addresses its own pointers use.
///
/// An EE memory image starts at address 0, and the EE sees its 32 MB of RAM again through
/// the uncached segments, so [`Image::ee`] masks addresses to 25 bits. Data read from a
/// file is addressed from wherever its pointers count from ([`Image::new`]).
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
    bytes: &'a [u8],
    base: u32,
    mask: u32,
}

impl<'a> Image<'a> {
    /// `bytes` start at address `base`.
    pub fn new(bytes: &'a [u8], base: u32) -> Self {
        Self {
            bytes,
            base,
            mask: u32::MAX,
        }
    }

    /// A memory image of the EE's RAM.
    pub fn ee(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            base: 0,
            mask: 0x01FF_FFFF,
        }
    }

    /// `len` bytes at `address`.
    pub fn bytes(&self, address: u32, len: usize) -> Result<&'a [u8], FormatError> {
        let outside = FormatError::OutOfBounds { address, len };
        let start = (address & self.mask)
            .checked_sub(self.base)
            .ok_or(outside.clone())? as usize;
        let end = start.checked_add(len).ok_or(outside.clone())?;
        self.bytes.get(start..end).ok_or(outside)
    }

    fn array<const N: usize>(&self, address: u32) -> Result<[u8; N], FormatError> {
        let mut out = [0; N];
        out.copy_from_slice(self.bytes(address, N)?);
        Ok(out)
    }

    pub fn u8(&self, address: u32) -> Result<u8, FormatError> {
        Ok(self.array::<1>(address)?[0])
    }

    pub fn i8(&self, address: u32) -> Result<i8, FormatError> {
        Ok(self.u8(address)? as i8)
    }

    pub fn u16(&self, address: u32) -> Result<u16, FormatError> {
        Ok(u16::from_le_bytes(self.array(address)?))
    }

    pub fn i16(&self, address: u32) -> Result<i16, FormatError> {
        Ok(i16::from_le_bytes(self.array(address)?))
    }

    pub fn u32(&self, address: u32) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.array(address)?))
    }

    pub fn f32(&self, address: u32) -> Result<f32, FormatError> {
        Ok(f32::from_le_bytes(self.array(address)?))
    }

    pub fn f32s<const N: usize>(&self, address: u32) -> Result<[f32; N], FormatError> {
        let mut out = [0.0; N];
        for (i, value) in out.iter_mut().enumerate() {
            *value = self.f32(address + 4 * i as u32)?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_values_at_their_addresses() {
        let bytes = [0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x80, 0x3F];
        let image = Image::new(&bytes, 0x100);
        assert_eq!(image.u8(0x100), Ok(1));
        assert_eq!(image.u16(0x101), Ok(0x0302));
        assert_eq!(image.u32(0x100), Ok(0x0403_0201));
        assert_eq!(image.f32(0x104), Ok(1.0));
        assert_eq!(image.i8(0x107), Ok(0x3F));
    }

    #[test]
    fn reading_outside_the_data_is_an_error() {
        let bytes = [0; 4];
        let image = Image::new(&bytes, 0x100);
        assert!(image.u32(0x101).is_err());
        assert!(image.u8(0xFF).is_err());
        assert!(image.u8(0x104).is_err());
        assert!(image.bytes(0xFFFF_FFFF, 2).is_err());
    }

    #[test]
    fn ee_images_see_ram_through_the_uncached_segments() {
        let mut bytes = vec![0; 0x20];
        bytes[0x10] = 7;
        let image = Image::ee(&bytes);
        assert_eq!(image.u8(0x10), Ok(7));
        assert_eq!(image.u8(0x2000_0010), Ok(7));
        assert_eq!(image.u8(0x3000_0010), Ok(7));
    }
}
