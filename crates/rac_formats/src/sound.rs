//! Sony's 989snd sound banks and the moby classes' sound tables (PLAN.md P3.13;
//! `docs/research/effects.md`, "Sounds").
//!
//! UYA keeps a bank of sound effects for each gun in the gadget WAD
//! ([`crate::gadget::GadgetEntry::sounds`]) and one for each level in its level WAD
//! ([`crate::level::LevelHeader::sound_bank`]). A moby class names the sounds it plays in
//! its sound table ([`ClassSound`]): each entry picks a sound of a bank, and says how far
//! it carries and how loud it is.
//!
//! A bank file is a short list of chunks, `{type, count}` then `{offset, size}` each: the
//! bank (`SBlk`), then its samples in the SPU's ADPCM ([`AdpcmDecoder`]). The bank lists
//! sounds; a sound is a short program of grains run 240 times a second, which start tones
//! (a sample at a pitch, with an envelope), wait, set registers and pick at random. The
//! layout is OpenGOAL's reading of 989snd (`game/sound/989snd/loader.cpp`, ISC,
//! `docs/research/prior-art.md`), checked on every gun's bank and Veldin's on UYA NTSC-U:
//! the guns' are version 3 (three are version 1), Veldin's version 3 with 321 sounds.

use crate::{FormatError, Image};

/// A bank's magic, `SBlk` read as a little-endian word.
const SFX_BLOCK: u32 = u32::from_le_bytes(*b"SBlk");
/// A music bank's, which this reader doesn't take.
const MUSIC_BANK: u32 = u32::from_le_bytes(*b"SBv2");
/// Bytes of a version 1 grain: its type and delay, then its parameters.
const GRAIN_V1: u32 = 0x28;
/// Bytes of a later grain: an opcode word and a delay.
const GRAIN_V2: u32 = 8;
/// Bytes of a sound's entry in the bank.
const SOUND: u32 = 12;
/// Bytes of a class's sound table entry.
const CLASS_SOUND: u32 = 0x20;

/// A bank of sound effects and the samples its tones play.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundBank {
    /// Its name: `mn00` for every gun's, `DAW` for Veldin's.
    pub name: [u8; 4],
    pub version: u32,
    /// By the numbers a class's sound table uses ([`ClassSound::sound`]).
    pub sounds: Vec<Sound>,
    /// SPU ADPCM; each tone's sample starts at its [`Tone::sample`].
    pub samples: Vec<u8>,
}

/// One sound: its volume and pan, and the grains that play it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    /// 0 to 127.
    pub volume: i8,
    /// The volume group (effects, music, speech, …) whose master volume it follows.
    pub group: i8,
    /// Degrees, 0 ahead and 90 to the right.
    pub pan: i16,
    pub instance_limit: i8,
    /// 1 loops, 2 plays solo, 8 limits instances (0x10 by volume, 0x20 by age).
    pub flags: u16,
    pub grains: Vec<Grain>,
}

/// One step of a sound's program.
#[derive(Debug, Clone, PartialEq)]
pub struct Grain {
    /// Ticks, of 240 a second, from the grain before.
    pub delay: i32,
    pub op: GrainOp,
}

/// What a grain does. The control grains' numbers are [`GrainOp::Control`]'s `op`.
#[derive(Debug, Clone, PartialEq)]
pub enum GrainOp {
    Null,
    /// Starts a voice playing a sample (grain types 1 and 9).
    Tone(Tone),
    /// Sets one of the sound's four LFOs (type 4).
    Lfo(Lfo),
    /// Starts another sound of the bank alongside (type 5).
    StartChild(PlaySound),
    /// Stops that child (type 6).
    StopChild(PlaySound),
    /// A message to a plugin of the sound library (type 7), with no sound of its own.
    Plugin {
        id: u32,
        index: u32,
    },
    /// Goes on as another sound of the bank (type 8).
    Branch(PlaySound),
    /// Waits a random number of ticks below this (type 26).
    RandDelay(i32),
    /// The rest: loops, stops, random picks, pitch bends, registers, markers.
    Control {
        op: u8,
        args: [i16; 4],
    },
}

/// A tone: which sample a voice plays, at what pitch, and its envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tone {
    pub priority: i8,
    /// 0 to 127; −1 to −4 take a register of the sound's, −5 a random volume.
    pub volume: i8,
    /// The note that plays the sample at its own rate. Negative for a sample at 48 kHz,
    /// whose note is the value's magnitude; positive for one at 44.1 kHz.
    pub centre_note: i8,
    /// 128ths of a semitone above `centre_note`.
    pub centre_fine: i8,
    /// Degrees; negative values take a register, as `volume` does.
    pub pan: i16,
    pub map_low: i8,
    pub map_high: i8,
    /// Semitones a full pitch bend takes it down and up.
    pub pitch_bend_low: i8,
    pub pitch_bend_high: i8,
    /// The SPU's envelope registers.
    pub adsr1: u16,
    pub adsr2: u16,
    /// 8 is noise, 0x10 reverb only.
    pub flags: u16,
    /// Where its sample starts in [`SoundBank::samples`].
    pub sample: u32,
}

/// An LFO's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lfo {
    pub which: u8,
    /// 1 volume, 2 pan, 3 pitch modulation, 4 pitch bend.
    pub target: u8,
    pub target_extra: u8,
    /// 1 sine, 2 square, 3 triangle, 4 saw, 5 random.
    pub shape: u8,
    pub duty_cycle: u16,
    pub depth: u16,
    pub flags: u16,
    pub start_offset: u16,
    pub step_size: u32,
}

/// Another sound a grain plays, with its volume, pan and registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaySound {
    pub volume: i32,
    pub pan: i32,
    pub registers: [i8; 4],
    /// Its number in the same bank; negative when it is named instead.
    pub sound: i32,
}

impl SoundBank {
    /// Reads a bank file: its chunks, the bank and its samples. `bytes` may run on past
    /// the file (a disc range is whole sectors).
    pub fn parse(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        let kind = image.u32(0)?;
        let chunks = image.u32(4)?;
        if !(kind == 1 || kind == 3) || !(2..=3).contains(&chunks) {
            return Err(FormatError::Invalid {
                what: "sound bank",
                address: 0,
                detail: format!("file type {kind} with {chunks} chunks"),
            });
        }
        let chunk = |index: u32| -> Result<&[u8], FormatError> {
            let offset = image.u32(8 + index * 8)?;
            let size = image.u32(12 + index * 8)?;
            image.bytes(offset, size as usize)
        };
        let block = chunk(0)?;
        let samples = chunk(1)?.to_vec();
        let bank = Image::new(block, 0);
        match bank.u32(0)? {
            SFX_BLOCK => {}
            MUSIC_BANK => {
                return Err(FormatError::Invalid {
                    what: "sound bank",
                    address: 0,
                    detail: "a music bank (SBv2)".into(),
                });
            }
            magic => {
                return Err(FormatError::Invalid {
                    what: "sound bank",
                    address: 0,
                    detail: format!("magic {magic:#010x}"),
                });
            }
        }
        let version = bank.u32(4)?;
        let mut name = [0; 4];
        name.copy_from_slice(bank.bytes(0xC, 4)?);
        let count = bank.i16(0x16)?;
        let first_sound = bank.u32(0x1C)?;
        let first_grain = bank.u32(0x20)?;
        // From version 2 a grain is an opcode whose argument points into this block.
        let grain_data = if version >= 2 { bank.u32(0x34)? } else { 0 };
        let sounds = (0..count.max(0) as u32)
            .map(|index| {
                let at = first_sound + index * SOUND;
                let grains = bank.u8(at + 4)? as i8;
                let first = bank.u32(at + 8)?;
                let grains = (0..grains.max(0) as u32)
                    .map(|g| {
                        if version < 2 {
                            read_grain_v1(&bank, first_grain + first + g * GRAIN_V1)
                        } else {
                            read_grain(&bank, first_grain + first + g * GRAIN_V2, grain_data)
                        }
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Sound {
                    volume: bank.i8(at)?,
                    group: bank.i8(at + 1)?,
                    pan: bank.i16(at + 2)?,
                    instance_limit: bank.i8(at + 5)?,
                    flags: bank.u16(at + 6)?,
                    grains,
                })
            })
            .collect::<Result<_, FormatError>>()?;
        Ok(Self {
            name,
            version,
            sounds,
            samples,
        })
    }
}

/// A version 1 grain: type, delay, then its parameters in place.
fn read_grain_v1(bank: &Image, at: u32) -> Result<Grain, FormatError> {
    let kind = bank.u32(at)?;
    let delay = bank.u32(at + 4)? as i32;
    let params = at + 8;
    let control = || {
        let mut args = [0; 4];
        for (i, arg) in args.iter_mut().enumerate() {
            *arg = bank.i16(params + 2 * i as u32)?;
        }
        Ok(args)
    };
    Ok(Grain {
        delay,
        op: grain_op(bank, kind.min(0xFF) as u8, params, control, || {
            bank.u32(params).map(|v| v as i32)
        })?,
    })
}

/// A later grain: the type in the opcode's top byte and a 24-bit argument, which points
/// into the grain data for the grains with parameters, gives a random delay its range, and
/// gives the control grains three signed bytes.
fn read_grain(bank: &Image, at: u32, grain_data: u32) -> Result<Grain, FormatError> {
    let opcode = bank.u32(at)?;
    let delay = bank.u32(at + 4)? as i32;
    let kind = (opcode >> 24) as u8;
    let value = opcode & 0xFF_FFFF;
    let args = [0, 1, 2].map(|i| ((opcode >> (8 * i)) & 0xFF) as u8 as i8 as i16);
    Ok(Grain {
        delay,
        op: grain_op(
            bank,
            kind,
            grain_data + value,
            || Ok([args[0], args[1], args[2], 0]),
            || Ok(value as i32 + 1),
        )?,
    })
}

fn grain_op(
    bank: &Image,
    kind: u8,
    params: u32,
    control: impl Fn() -> Result<[i16; 4], FormatError>,
    rand_delay: impl Fn() -> Result<i32, FormatError>,
) -> Result<GrainOp, FormatError> {
    Ok(match kind {
        0 => GrainOp::Null,
        1 | 9 => GrainOp::Tone(read_tone(bank, params)?),
        4 => GrainOp::Lfo(Lfo {
            which: bank.u8(params)?,
            target: bank.u8(params + 1)?,
            target_extra: bank.u8(params + 2)?,
            shape: bank.u8(params + 3)?,
            duty_cycle: bank.u16(params + 4)?,
            depth: bank.u16(params + 6)?,
            flags: bank.u16(params + 8)?,
            start_offset: bank.u16(params + 0xA)?,
            step_size: bank.u32(params + 0xC)?,
        }),
        5 => GrainOp::StartChild(read_play_sound(bank, params)?),
        6 => GrainOp::StopChild(read_play_sound(bank, params)?),
        7 => GrainOp::Plugin {
            id: bank.u32(params)?,
            index: bank.u32(params + 4)?,
        },
        8 => GrainOp::Branch(read_play_sound(bank, params)?),
        26 => GrainOp::RandDelay(rand_delay()?),
        op => GrainOp::Control {
            op,
            args: control()?,
        },
    })
}

fn read_tone(bank: &Image, at: u32) -> Result<Tone, FormatError> {
    Ok(Tone {
        priority: bank.i8(at)?,
        volume: bank.i8(at + 1)?,
        centre_note: bank.i8(at + 2)?,
        centre_fine: bank.i8(at + 3)?,
        pan: bank.i16(at + 4)?,
        map_low: bank.i8(at + 6)?,
        map_high: bank.i8(at + 7)?,
        pitch_bend_low: bank.i8(at + 8)?,
        pitch_bend_high: bank.i8(at + 9)?,
        adsr1: bank.u16(at + 0xA)?,
        adsr2: bank.u16(at + 0xC)?,
        flags: bank.u16(at + 0xE)?,
        sample: bank.u32(at + 0x10)?,
    })
}

fn read_play_sound(bank: &Image, at: u32) -> Result<PlaySound, FormatError> {
    let mut registers = [0; 4];
    for (i, register) in registers.iter_mut().enumerate() {
        *register = bank.i8(at + 8 + i as u32)?;
    }
    Ok(PlaySound {
        volume: bank.u32(at)? as i32,
        pan: bank.u32(at + 4)? as i32,
        registers,
        sound: bank.u32(at + 0xC)? as i32,
    })
}

/// One entry of a moby class's sound table (UYA: class + 0x28, its count at + 0xD;
/// `moby_play_sound`, 0x4A7200, and the volume it gets, 0x4A53D0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassSound {
    /// Within `near` of the listener it plays at `max_volume`; from there to `far` it falls
    /// to `min_volume` (by the square of the distance left when [`ClassSound::squared`]).
    pub near: f32,
    pub far: f32,
    /// Out of 0x400, which is full volume; some are louder, and the game caps them there.
    pub min_volume: i32,
    pub max_volume: i32,
    /// A pitch modulation picked at random from `pitch_low` up to `pitch_high`, in 128ths
    /// of a semitone.
    pub pitch_low: i32,
    pub pitch_high: i32,
    /// It loops until stopped, and only plays when asked to loop.
    pub looped: bool,
    /// Bit 0: the volume falls with the square of the distance. The rest aren't read.
    pub flags: u8,
    /// Its number in the bank; `None` for an entry with no sound (0xFFFF).
    pub sound: Option<u16>,
    /// Which loaded bank: 0 the level's; a gun's are set to its own (4) when it comes out.
    pub bank: u32,
}

impl ClassSound {
    /// The volume at `distance` from the listener, out of 0x400.
    pub fn volume_at(&self, distance: f32) -> i32 {
        if distance <= self.near {
            return self.max_volume;
        }
        if distance >= self.far {
            return self.min_volume;
        }
        let left = (self.far - distance) / (self.far - self.near);
        let share = if self.squared() { left * left } else { left };
        self.min_volume + (share * (self.max_volume - self.min_volume) as f32) as i32
    }

    pub fn squared(&self) -> bool {
        self.flags & 1 != 0
    }

    /// Reads the sound table of the class whose header is at `address`. Its pointer counts
    /// from the class's start when `packed` (as stored on the disc).
    pub fn read_table(image: &Image, address: u32, packed: bool) -> Result<Vec<Self>, FormatError> {
        let count = image.u8(address + 0xD)? as u32;
        let table = image.u32(address + 0x28)?;
        if count == 0 || table == 0 {
            return Ok(Vec::new());
        }
        let table = if packed { address + table } else { table };
        (0..count)
            .map(|index| Self::read(image, table + index * CLASS_SOUND))
            .collect()
    }

    /// Reads one entry at `at`.
    pub fn read(image: &Image, at: u32) -> Result<Self, FormatError> {
        let sound = image.u16(at + 0x1A)?;
        Ok(Self {
            near: image.f32(at)?,
            far: image.f32(at + 4)?,
            min_volume: image.u32(at + 8)? as i32,
            max_volume: image.u32(at + 0xC)? as i32,
            pitch_low: image.u32(at + 0x10)? as i32,
            pitch_high: image.u32(at + 0x14)? as i32,
            looped: image.u8(at + 0x18)? != 0,
            flags: image.u8(at + 0x19)?,
            sound: (sound != 0xFFFF).then_some(sound),
            bank: image.u32(at + 0x1C)?,
        })
    }
}

/// The SPU's ADPCM: 16-byte blocks of 28 samples, each block a shift and a filter, flags,
/// then 4-bit deltas. A block's flags end the sample (bit 0), and with bit 1 loop it back to
/// the last block that set bit 2.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AdpcmDecoder {
    history: [i32; 2],
}

/// What a block's flags say.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BlockFlags {
    pub loop_start: bool,
    pub loop_repeat: bool,
    pub end: bool,
}

/// Samples in a block.
pub const BLOCK_SAMPLES: usize = 28;
/// Bytes in a block.
pub const BLOCK_BYTES: usize = 16;

/// The predictors, in 64ths, by the block's filter.
const FILTERS: [[i32; 2]; 5] = [[0, 0], [60, 0], [115, -52], [98, -55], [122, -60]];

impl AdpcmDecoder {
    /// Decodes one block into `out`, carrying the prediction on to the next.
    pub fn decode(
        &mut self,
        block: &[u8; BLOCK_BYTES],
        out: &mut [i16; BLOCK_SAMPLES],
    ) -> BlockFlags {
        let shift = (block[0] & 0xF) as u32;
        // Filters past the last decode as none, and shifts past 12 as 9 (the SPU's).
        let filter = FILTERS
            .get((block[0] >> 4 & 7) as usize)
            .copied()
            .unwrap_or([0, 0]);
        let shift = if shift > 12 { 9 } else { shift };
        for (i, sample) in out.iter_mut().enumerate() {
            let byte = block[2 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0xF } else { byte >> 4 };
            let mut value = (((nibble as i16) << 12) as i32) >> shift;
            value += (filter[0] * self.history[0]) >> 6;
            value += (filter[1] * self.history[1]) >> 6;
            let value = value.clamp(i16::MIN as i32, i16::MAX as i32);
            self.history = [value, self.history[0]];
            *sample = value as i16;
        }
        BlockFlags {
            end: block[1] & 1 != 0,
            loop_repeat: block[1] & 2 != 0,
            loop_start: block[1] & 4 != 0,
        }
    }

    /// Decodes a sample from `at` in `data` to its end flag (or the data's end), without
    /// looping.
    pub fn decode_all(data: &[u8], at: usize) -> Vec<i16> {
        let mut decoder = Self::default();
        let mut out = Vec::new();
        let mut block = [0; BLOCK_SAMPLES];
        for chunk in data.get(at..).unwrap_or(&[]).as_chunks::<BLOCK_BYTES>().0 {
            let flags = decoder.decode(chunk, &mut block);
            out.extend_from_slice(&block);
            if flags.end {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby::tests::Writer;

    /// A version 3 bank of two sounds: a register set at random then a tone, and a tone
    /// alone; then 32 bytes of samples.
    fn bank_file() -> Vec<u8> {
        let mut block = Writer::default();
        block.put(b"SBlk").u32(3).u32(4).put(b"mn00");
        block.at(0x16).u16(2).u16(3).u16(1);
        // Sounds at 0x40, grains at 0x58, grain data at 0x70.
        block.at(0x1C).u32(0x40).u32(0x58);
        block.at(0x34).u32(0x70);
        block.at(0x40);
        block.put(&[0x7E, 0]).u16(0).put(&[2, 0]).u16(1).u32(0);
        block.put(&[0x7F, 0]).u16(0).put(&[1, 0]).u16(0).u32(0x10);
        // Sound 0: set register 0 to 100–115, then the tone at 0; sound 1: the tone at
        // 0x18 after 10 ticks.
        block.u32(0x1F73_6400).u32(0);
        block.u32(0x0100_0000).u32(0);
        block.u32(0x0100_0018).u32(10);
        // Tone 0 takes register 0 for its volume and plays a 48 kHz sample at note 83;
        // tone 1 plays from 0x10 at 44.1 kHz.
        block.at(0x70);
        block.put(&[0, 0xFF, 0xAD, 0x31]).u16(0).put(&[0, 0, 0, 0]);
        block.u16(0x80FF).u16(0x9FCD).u16(1).u32(0).u32(0);
        block
            .put(&[0x5A, 0x7F, 0x48, 0])
            .u16(90)
            .put(&[0, 0x5A, 2, 3]);
        block.u16(0x1EFF).u16(0xDFCC).u16(1).u32(0x10).u32(0);
        let block = block.bytes;
        let mut file = Writer::default();
        file.u32(3).u32(2);
        file.u32(0x18).u32(block.len() as u32);
        file.u32(0x18 + block.len() as u32).u32(32);
        file.put(&block).put(&[0x11; 32]);
        file.bytes
    }

    #[test]
    fn reads_a_version_3_bank_with_its_grains_and_tones() {
        let bank = SoundBank::parse(&bank_file()).unwrap();
        assert_eq!(&bank.name, b"mn00");
        assert_eq!(bank.version, 3);
        assert_eq!(bank.samples.len(), 32);
        assert_eq!(bank.sounds.len(), 2);
        let first = &bank.sounds[0];
        assert_eq!((first.volume, first.flags), (0x7E, 1));
        assert_eq!(
            first.grains[0].op,
            GrainOp::Control {
                op: 31,
                args: [0, 100, 115, 0]
            }
        );
        let GrainOp::Tone(tone) = first.grains[1].op else {
            panic!("a tone");
        };
        assert_eq!(
            (tone.volume, tone.centre_note, tone.centre_fine),
            (-1, -83, 0x31)
        );
        assert_eq!((tone.adsr1, tone.adsr2, tone.sample), (0x80FF, 0x9FCD, 0));
        let second = &bank.sounds[1];
        assert_eq!(second.grains[0].delay, 10);
        let GrainOp::Tone(tone) = second.grains[0].op else {
            panic!("a tone");
        };
        assert_eq!(
            (tone.pan, tone.pitch_bend_low, tone.pitch_bend_high),
            (90, 2, 3)
        );
        assert_eq!(tone.sample, 0x10);
    }

    #[test]
    fn refuses_music_banks_and_other_files() {
        let mut file = bank_file();
        file[0x18..0x1C].copy_from_slice(b"SBv2");
        assert!(SoundBank::parse(&file).is_err());
        assert!(SoundBank::parse(&[0; 64]).is_err());
        assert!(SoundBank::parse(&file[..0x30]).is_err());
    }

    #[test]
    fn reads_a_class_sound_table_and_falls_off_with_distance() {
        let mut class = Writer::default();
        class.at(0xD).put(&[2]);
        class.at(0x28).u32(0x40);
        class.at(0x40);
        class
            .f32(0.0)
            .f32(40.0)
            .u32(0)
            .u32(0x400)
            .u32(-127i32 as u32)
            .u32(127);
        class.put(&[1, 0]).u16(3).u32(4);
        class
            .f32(10.0)
            .f32(20.0)
            .u32(0x100)
            .u32(0x300)
            .u32(0)
            .u32(0);
        class.put(&[0, 1]).u16(0xFFFF).u32(0);
        let image = Image::new(&class.bytes, 0);
        let table = ClassSound::read_table(&image, 0, true).unwrap();
        assert_eq!(table.len(), 2);
        assert_eq!(table[0].sound, Some(3));
        assert!(table[0].looped);
        assert_eq!(
            (table[0].pitch_low, table[0].pitch_high, table[0].bank),
            (-127, 127, 4)
        );
        assert_eq!(table[0].volume_at(0.0), 0x400);
        assert_eq!(table[0].volume_at(20.0), 0x200);
        assert_eq!(table[0].volume_at(50.0), 0);
        assert_eq!(table[1].sound, None);
        // Halfway between near and far, the square of a half of the range above min.
        assert_eq!(table[1].volume_at(15.0), 0x100 + 0x80);
    }

    #[test]
    fn decodes_adpcm_blocks_with_their_filters_and_flags() {
        // Shift 12 keeps each nibble as is; filter 1 adds 60/64 of the last sample.
        let mut block = [0u8; BLOCK_BYTES];
        block[0] = 0x1C;
        block[1] = 0x01;
        block[2] = 0x21;
        let mut out = [0; BLOCK_SAMPLES];
        let mut decoder = AdpcmDecoder::default();
        let flags = decoder.decode(&block, &mut out);
        assert!(flags.end && !flags.loop_repeat && !flags.loop_start);
        assert_eq!(out[0], 1);
        // 2 + (60 × 1) / 64.
        assert_eq!(out[1], 2);
        assert_eq!(out[2], (60 * 2) >> 6);
        // A nibble of 8 is −8 before the shift.
        let mut low = [0u8; BLOCK_BYTES];
        low[0] = 0x00;
        low[2] = 0x08;
        let mut decoder = AdpcmDecoder::default();
        decoder.decode(&low, &mut out);
        assert_eq!(out[0], -0x8000);
        assert_eq!(
            AdpcmDecoder::decode_all(&[block, block].concat(), 0).len(),
            28
        );
    }
}
