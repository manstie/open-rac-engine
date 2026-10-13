//! Animation sequences and their keyframes (`docs/research/moby-format.md`, "Sequences
//! and keyframes").

use crate::{FormatError, Image};

/// A sequence whose keyframes are stored whole says so with this in its header's +0x13.
const UNCOMPRESSED: u8 = 0xFF;
/// Compressed keyframes' pointers carry flags in their top four bits.
const POINTER: u32 = 0x0FFF_FFFF;
/// Rotations are fixed point with 1.0 = 0x8000; scales have 1.0 = 0x1000.
const ROTATION_ONE: f32 = 32768.0;
const SCALE_ONE: f32 = 4096.0;

/// One animation: keyframes the game interpolates between, wrapping at the end.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    /// A bounding sphere: centre and radius, in class units.
    pub bounds: [f32; 4],
    pub keyframes: Vec<Keyframe>,
}

/// A joint's scale for one keyframe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointScale {
    pub joint: u8,
    /// Along the joint's own axes, 1.0 = unscaled.
    pub scale: [f32; 3],
}

/// A joint's position relative to its parent for one keyframe, in place of its rest
/// offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointTranslation {
    pub joint: u8,
    /// In class units.
    pub offset: [i16; 3],
}

/// A pose of the whole skeleton.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    /// How far through the move to the next keyframe one tick goes: 1 / its ticks, or 0 if
    /// the animation stops here.
    pub rate: f32,
    /// The animation frame the game reports here, in sixteenths.
    pub stamp: u16,
    /// Each joint's rotation as stored, x, y, z, w with 1.0 = 0x8000. The game stores the
    /// inverse rotation; [`Keyframe::rotation`] gives the usual one.
    pub rotations: Vec<[i16; 4]>,
    pub scales: Vec<JointScale>,
    pub translations: Vec<JointTranslation>,
}

impl Keyframe {
    /// How many ticks the move to the next keyframe takes, or 0 if it never does.
    pub fn ticks(&self) -> f32 {
        if self.rate > 0.0 {
            1.0 / self.rate
        } else {
            0.0
        }
    }

    /// The joint's rotation as a unit quaternion `[x, y, z, w]` in the usual convention,
    /// rotating class axes.
    pub fn rotation(&self, joint: usize) -> [f32; 4] {
        let [x, y, z, w] = self.rotations[joint].map(|value| value as f32 / ROTATION_ONE);
        let length = (x * x + y * y + z * z + w * w).sqrt();
        if length == 0.0 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        [-x / length, -y / length, -z / length, w / length]
    }
}

impl Sequence {
    /// Reads the sequence at `address`. Its keyframe pointers count from `frames_from`:
    /// 0 where they are addresses (in memory), else the class's or sequence's start (on
    /// the disc).
    pub(super) fn read(
        image: &Image,
        address: u32,
        joints: usize,
        frames_from: u32,
    ) -> Result<Self, FormatError> {
        let bounds = image.f32s::<4>(address)?;
        let count = image.u8(address + 0x10)? as u32;
        let extra = image.u8(address + 0x13)?;
        let frames = address + 0x1C;
        let mut keyframes = Vec::with_capacity(count as usize);
        if extra == UNCOMPRESSED {
            for index in 0..count {
                let at = frames_from + (image.u32(frames + index * 4)? & POINTER);
                keyframes.push(read_whole(image, at, joints)?);
            }
        } else {
            if joints == 0 {
                return Err(FormatError::Invalid {
                    what: "compressed sequence",
                    address,
                    detail: "the class has no joints".into(),
                });
            }
            let base = frames + (count + extra as u32) * 4;
            for index in 0..count {
                let at = frames_from + (image.u32(frames + index * 4)? & POINTER);
                keyframes.push(unpack(image, base, at, joints)?);
            }
        }
        Ok(Self { bounds, keyframes })
    }
}

/// A keyframe stored whole, laid out as the game unpacks compressed ones.
fn read_whole(image: &Image, at: u32, joints: usize) -> Result<Keyframe, FormatError> {
    let rate = image.f32(at)?;
    let stamp = image.u16(at + 4)?;
    let stored = image.u16(at + 8)? as usize / 8;
    if stored != joints {
        return Err(FormatError::Invalid {
            what: "keyframe",
            address: at,
            detail: format!("{stored} joints, the class has {joints}"),
        });
    }
    let scales = image.u16(at + 0xA)? as u32;
    let translations = image.u16(at + 0xE)? as u32;
    let mut rotations = Vec::with_capacity(joints);
    let mut entry = at + 0x10;
    for _ in 0..joints {
        rotations.push([0, 2, 4, 6].map(|c| image.i16(entry + c)).try_map_ok()?);
        entry += 8;
    }
    let mut keyframe = Keyframe {
        rate,
        stamp,
        rotations,
        scales: Vec::new(),
        translations: Vec::new(),
    };
    for _ in 0..scales {
        let value = [0, 2, 4].map(|c| image.u16(entry + c)).try_map_ok()?;
        keyframe.scales.push(scale(value, image.u16(entry + 6)?));
        entry += 8;
    }
    for _ in 0..translations {
        keyframe.translations.push(JointTranslation {
            joint: image.u8(entry + 6)?,
            offset: [0, 2, 4].map(|c| image.i16(entry + c)).try_map_ok()?,
        });
        entry += 8;
    }
    Ok(keyframe)
}

fn scale(value: [u16; 3], joint: u16) -> JointScale {
    JointScale {
        // The top bits are flags for the pose builder.
        joint: joint as u8,
        scale: value.map(|v| v as f32 / SCALE_ONE),
    }
}

/// Reads ten-bit signed values, three to each little-endian word, low bits first.
struct TenBits {
    at: u32,
    shift: u32,
}

impl TenBits {
    fn next(&mut self, image: &Image) -> Result<i16, FormatError> {
        let word = image.u32(self.at)?;
        let value = ((word << (22 - self.shift)) as i32) >> 22;
        self.shift += 10;
        if self.shift == 30 {
            self.shift = 0;
            self.at += 4;
        }
        Ok(value as i16)
    }
}

/// Unpacks a compressed keyframe at `at` against its sequence's base block at `base`.
fn unpack(image: &Image, base: u32, at: u32, joints: usize) -> Result<Keyframe, FormatError> {
    let ticks = image.u16(at)?;
    let stamp = image.u16(at + 2)?;
    let header = image.u32(base)?;
    let data = at + 4;
    let mut ten_bits = TenBits { at: data, shift: 0 };
    let mut bytes = data + ((header >> 10) & 0x7FF);
    let mut halves = data + (header & 0x3FF);
    let signs = data + (header >> 21);

    let mut rotations = Vec::with_capacity(joints);
    for joint in 0..joints as u32 {
        let mut rotation = [0i16; 4];
        for (c, value) in rotation.iter_mut().take(3).enumerate() {
            let stored = image.u16(base + 4 + joint * 6 + c as u32 * 2)?;
            // The base value's low two bits say how this keyframe changes it, and stay
            // in the result.
            let changed = match stored & 3 {
                0 => stored,
                1 => {
                    let delta = image.i8(bytes)?;
                    bytes += 1;
                    stored.wrapping_add((delta as i16 * 4) as u16)
                }
                2 => stored.wrapping_add((ten_bits.next(image)? * 4) as u16),
                _ => {
                    let value = image.u16(halves)?;
                    halves += 2;
                    value
                }
            };
            *value = changed as i16;
        }
        let [x, y, z] = [rotation[0], rotation[1], rotation[2]].map(|v| v as f32 / ROTATION_ONE);
        let w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
        let mut w = ((w * ROTATION_ONE) as i32).min(0x7FFF) as i16;
        if (image.u8(signs + joint / 8)? >> (joint % 8)) & 1 != 0 {
            w = -w;
        }
        rotation[3] = w;
        rotations.push(rotation);
    }

    let counts = base + 4 + joints as u32 * 6;
    let scales = image.u8(counts)? as u32;
    let translations = image.u8(counts + 1)? as u32;
    let mut stream = signs + (joints as u32).div_ceil(8);
    let mut keyframe = Keyframe {
        rate: if ticks == 0 { 0.0 } else { 1.0 / ticks as f32 },
        stamp,
        rotations,
        scales: Vec::new(),
        translations: Vec::new(),
    };
    for index in 0..scales + translations {
        let entry = counts + 2 + index * 8;
        let modes = image.u8(stream)?;
        stream += 1;
        let mut value = [0i16; 3];
        for (c, value) in value.iter_mut().enumerate() {
            let stored = image.i16(entry + c as u32 * 2)?;
            *value = match (modes >> (2 * c)) & 3 {
                1 => {
                    let delta = image.i8(stream)?;
                    stream += 1;
                    stored.wrapping_add(delta as i16 * 4)
                }
                2 => {
                    let value = image.i16(stream)?;
                    stream += 2;
                    value
                }
                _ => stored,
            };
        }
        let joint = image.u16(entry + 6)?;
        if index < scales {
            keyframe.scales.push(scale(value.map(|v| v as u16), joint));
        } else {
            keyframe.translations.push(JointTranslation {
                joint: joint as u8,
                offset: value,
            });
        }
    }
    Ok(keyframe)
}

/// `[Result<T, E>; N]` → `Result<[T; N], E>`.
trait TryMapOk<T, E, const N: usize> {
    fn try_map_ok(self) -> Result<[T; N], E>;
}

impl<T: Copy + Default, E, const N: usize> TryMapOk<T, E, N> for [Result<T, E>; N] {
    fn try_map_ok(self) -> Result<[T; N], E> {
        let mut out = [T::default(); N];
        for (slot, value) in out.iter_mut().zip(self) {
            *slot = value?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby::tests::Writer;

    /// Packs ten-bit values three to a word, the way the game reads them.
    fn ten_bit_words(values: &[i16]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in values.chunks(3) {
            let mut word = 0u32;
            for (i, &value) in chunk.iter().enumerate() {
                word |= ((value as u32) & 0x3FF) << (10 * i);
            }
            out.extend_from_slice(&word.to_le_bytes());
        }
        out
    }

    /// A compressed sequence for two joints with one keyframe that uses every way a value
    /// can change, one scale and one translation.
    fn compressed() -> (Vec<u8>, u32) {
        let mut w = Writer::default();
        let sequence = 0x100;
        // Bounds, one keyframe, no extra words.
        w.at(sequence).f32(1.0).f32(2.0).f32(3.0).f32(4.0);
        w.u8(1).u8(0xFF).u8(0).u8(0).u32(0).u32(0);
        let frame_pointer = w.here();
        w.u32(0);
        // The base block: the header word, then each joint's base rotation.
        // The frame's data: two ten-bit values, then two bytes of deltas, one 16-bit
        // value, then the sign bits and the scale and translation modes.
        let ten = ten_bit_words(&[-3, 100]);
        let bytes_at = ten.len() as u32;
        let halves_at = bytes_at + 2;
        let signs_at = halves_at + 2;
        w.u32(halves_at | bytes_at << 10 | signs_at << 21);
        // Joint 0: x keeps, y adds a byte, z adds ten bits.
        w.u16(0x0100).u16(0x0201).u16(0x0302);
        // Joint 1: x replaced by a 16-bit value, y adds ten bits, z adds a byte.
        w.u16(0x0003).u16(0x0042).u16(0x0011);
        // One scale, one translation.
        w.u8(1).u8(1);
        w.u16(0x1000).u16(0x1000).u16(0x1000).u16(0x8001);
        w.u16(10).u16(20).u16(30).u16(1);
        let frame = w.here();
        w.u16(4).u16(48);
        w.put(&ten);
        w.u8(5).u8(-2i8 as u8);
        w.u16(0x1234);
        // Joint 1's w is negative.
        w.u8(0b10);
        // Scale: x adds a byte; translation: y is a 16-bit value, z adds a byte.
        w.u8(0b01).u8(3);
        w.u8(0b01_10_00).u16(-500i16 as u16).u8(-1i8 as u8);
        w.poke_u32(frame_pointer, frame | 0xF000_0000);
        (w.bytes, sequence)
    }

    #[test]
    fn unpacks_every_kind_of_change() {
        let (bytes, at) = compressed();
        let sequence = Sequence::read(&Image::new(&bytes, 0), at, 2, 0).unwrap();
        assert_eq!(sequence.bounds, [1.0, 2.0, 3.0, 4.0]);
        let keyframe = &sequence.keyframes[0];
        assert_eq!(keyframe.rate, 0.25);
        assert_eq!(keyframe.ticks(), 4.0);
        assert_eq!(keyframe.stamp, 48);
        let [a, b] = [keyframe.rotations[0], keyframe.rotations[1]];
        assert_eq!(a[..3], [0x0100, 0x0201 + 5 * 4, 0x0302 - 3 * 4]);
        assert_eq!(b[..3], [0x1234, 0x0042 + 100 * 4, 0x0011 - 2 * 4]);
        assert!(a[3] > 0x7F00 && b[3] < -0x7E00);
        assert_eq!(
            keyframe.scales,
            [JointScale {
                joint: 1,
                scale: [(0x1000 + 12) as f32 / 4096.0, 1.0, 1.0],
            }]
        );
        assert_eq!(
            keyframe.translations,
            [JointTranslation {
                joint: 1,
                offset: [10, -500, 26],
            }]
        );
    }

    #[test]
    fn rotations_come_out_as_the_inverse_of_what_is_stored() {
        let keyframe = Keyframe {
            rate: 0.5,
            stamp: 0,
            // A quarter turn, stored as the game does.
            rotations: vec![[0, 0, 23170, 23170]],
            scales: Vec::new(),
            translations: Vec::new(),
        };
        let [x, y, z, w] = keyframe.rotation(0);
        assert_eq!([x, y], [0.0, 0.0]);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!((z + half).abs() < 1e-3 && (w - half).abs() < 1e-3);
    }

    #[test]
    fn reads_whole_keyframes() {
        let mut w = Writer::default();
        w.at(0x100).put(&[0; 16]).u8(1).u8(0).u8(0).u8(UNCOMPRESSED);
        w.u32(0).u32(0);
        let pointer = w.here();
        w.u32(0);
        let frame = w.here();
        w.f32(0.5).u16(32).u16(2).u16(8).u16(1).u16(0).u16(1);
        w.u16(1).u16(2).u16(3).u16(0x7000);
        w.u16(0x2000).u16(0x1000).u16(0x1000).u16(0x8000);
        w.u16(-4i16 as u16).u16(5).u16(6).u16(0);
        w.poke_u32(pointer, frame);
        let sequence = Sequence::read(&Image::new(&w.bytes, 0), 0x100, 1, 0).unwrap();
        let keyframe = &sequence.keyframes[0];
        assert_eq!((keyframe.ticks(), keyframe.stamp), (2.0, 32));
        assert_eq!(keyframe.rotations, [[1, 2, 3, 0x7000]]);
        assert_eq!(keyframe.scales[0].scale, [2.0, 1.0, 1.0]);
        assert_eq!(keyframe.translations[0].offset, [-4, 5, 6]);
    }

    #[test]
    fn a_whole_keyframe_for_another_skeleton_is_an_error() {
        let mut w = Writer::default();
        w.at(0x100).put(&[0; 16]).u8(1).u8(0).u8(0).u8(UNCOMPRESSED);
        w.u32(0).u32(0).u32(0x200);
        w.at(0x200).f32(0.5).u16(0).u16(0).u16(16);
        w.put(&[0; 32]);
        assert!(Sequence::read(&Image::new(&w.bytes, 0), 0x100, 1, 0).is_err());
    }
}
