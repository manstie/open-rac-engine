// A voice of the PS2's SPU, after OpenGOAL's (`game/sound/common/voice.cpp`,
// `envelope.cpp`).
//
// Copyright (c) 2020-2026 OpenGOAL Team; 2021-2024 Ziemas.
//
// Permission to use, copy, modify, and/or distribute this software for any purpose with or
// without fee is hereby granted, provided that the above copyright notice and this
// permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO
// THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO
// EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL
// DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
// AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! One SPU voice: it streams a sample's ADPCM blocks, following their loop flags, steps
//! through them at its pitch with the SPU's Gaussian interpolation, and shapes them with
//! its envelope and volume, one output sample at 48 kHz at a time.

use std::collections::VecDeque;
use std::sync::Arc;

use rac_formats::sound::{AdpcmDecoder, BLOCK_BYTES, BLOCK_SAMPLES};

use crate::tables::GAUSS;

/// The highest pitch the SPU takes: just under four times the sample's own rate.
const MAX_PITCH: u32 = 0x3FFF;

/// A voice's envelope's stage.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Attack,
    Decay,
    Sustain,
    Release,
    #[default]
    Stopped,
}

/// The SPU's ADSR envelope, its two registers held as one word (ADSR1 low, ADSR2 high).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Adsr {
    register: u32,
    phase: Phase,
    target: i32,
    shift: u8,
    step: i8,
    exponential: bool,
    decrease: bool,
    counter: u32,
    level: i32,
}

impl Adsr {
    pub(crate) fn set(&mut self, adsr1: u16, adsr2: u16) {
        self.register = adsr1 as u32 | (adsr2 as u32) << 16;
    }

    fn bits(&self, from: u32, width: u32) -> u32 {
        self.register >> from & ((1 << width) - 1)
    }

    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    pub(crate) fn level(&self) -> i32 {
        self.level
    }

    pub(crate) fn attack(&mut self) {
        self.phase = Phase::Attack;
        self.level = 0;
        self.counter = 0;
        self.settings();
    }

    pub(crate) fn release(&mut self) {
        if self.phase == Phase::Stopped {
            return;
        }
        self.phase = Phase::Release;
        self.counter = 0;
        self.settings();
    }

    pub(crate) fn stop(&mut self) {
        self.phase = Phase::Stopped;
        self.level = 0;
    }

    fn settings(&mut self) {
        match self.phase {
            Phase::Attack => {
                self.exponential = self.bits(15, 1) != 0;
                self.decrease = false;
                self.shift = self.bits(10, 5) as u8;
                self.step = 7 - self.bits(8, 2) as i8;
                self.target = 0x7FFF;
            }
            Phase::Decay => {
                self.exponential = true;
                self.decrease = true;
                self.shift = self.bits(4, 4) as u8;
                self.step = -8;
                self.target = (self.bits(0, 4) as i32 + 1) << 11;
            }
            Phase::Sustain => {
                self.exponential = self.bits(31, 1) != 0;
                self.decrease = self.bits(30, 1) != 0;
                self.shift = self.bits(24, 5) as u8;
                let step = self.bits(22, 2) as i8;
                self.step = if self.decrease { -8 + step } else { 7 - step };
                self.target = 0;
            }
            Phase::Release => {
                self.exponential = self.bits(21, 1) != 0;
                self.decrease = true;
                self.shift = self.bits(16, 5) as u8;
                self.step = -8;
                self.target = 0;
            }
            Phase::Stopped => {}
        }
    }

    /// One step of the level: the counter runs at a rate set by the shift, and each time
    /// it fills the level moves by the step (scaled by the level itself when exponential).
    fn step(&mut self) {
        let mut rate = 0x80_0000u32;
        let shift = self.shift as i32 - 11;
        if shift > 0 {
            rate >>= shift;
        }
        let mut step = ((self.step as i32) << (11 - self.shift as i32).max(0)) as i16 as i32;
        if self.exponential {
            if !self.decrease && self.level > 0x6000 {
                rate >>= 2;
            }
            if self.decrease {
                step = ((step * self.level) >> 15) as i16 as i32;
            }
        }
        self.counter += rate;
        if self.counter >= 0x80_0000 {
            self.counter = 0;
            self.level = (self.level + step).clamp(0, i16::MAX as i32);
        }
    }

    fn run(&mut self) {
        if self.phase == Phase::Stopped {
            return;
        }
        self.step();
        if self.phase == Phase::Sustain {
            return;
        }
        let reached = if self.decrease {
            self.level <= self.target
        } else {
            self.level >= self.target
        };
        if reached {
            self.phase = match self.phase {
                Phase::Attack => Phase::Decay,
                Phase::Decay => Phase::Sustain,
                Phase::Release => Phase::Stopped,
                phase => phase,
            };
            self.settings();
        }
    }
}

/// One voice playing a sample.
#[derive(Debug, Clone)]
pub(crate) struct Voice {
    samples: Arc<[u8]>,
    /// The block to decode next, and where the sample loops back to.
    block: usize,
    loop_start: usize,
    decoder: AdpcmDecoder,
    decoded: VecDeque<i16>,
    /// The phase between decoded samples, 12 bits.
    counter: u32,
    /// 0x1000 plays the sample at 48 kHz.
    pub(crate) pitch: u32,
    pub(crate) adsr: Adsr,
    /// The SPU's volume registers' levels, out of 0x7FFF.
    left: i32,
    right: i32,
}

/// Applies a volume out of 0x8000.
fn scale(sample: i32, volume: i32) -> i32 {
    (sample * volume) >> 15
}

impl Voice {
    /// A voice keyed on: the sample from byte `start` of `samples`, its envelope attacking.
    pub(crate) fn key_on(samples: Arc<[u8]>, start: usize, pitch: u32, adsr: (u16, u16)) -> Self {
        let mut envelope = Adsr::default();
        envelope.set(adsr.0, adsr.1);
        envelope.attack();
        Self {
            samples,
            block: start,
            loop_start: start,
            decoder: AdpcmDecoder::default(),
            decoded: VecDeque::with_capacity(64),
            counter: 0,
            pitch,
            adsr: envelope,
            left: 0,
            right: 0,
        }
    }

    pub(crate) fn key_off(&mut self) {
        self.adsr.release();
    }

    /// Sets its volume registers, as the library writes them (out of 0x3FFF, doubled as
    /// the SPU does for a volume that doesn't sweep).
    pub(crate) fn set_volume(&mut self, left: i16, right: i16) {
        self.left = (left as i32) << 1;
        self.right = (right as i32) << 1;
    }

    pub(crate) fn dead(&self) -> bool {
        self.adsr.phase() == Phase::Stopped
    }

    /// Decodes the next block, then follows its flags: an end loops back to the last
    /// block that started a loop, and stops the voice unless the block repeats.
    fn decode_block(&mut self) {
        let Some(bytes) = self.samples.get(self.block..self.block + BLOCK_BYTES) else {
            self.adsr.stop();
            return;
        };
        let bytes: &[u8; BLOCK_BYTES] = bytes.try_into().expect("16 bytes");
        let mut out = [0; BLOCK_SAMPLES];
        let flags = self.decoder.decode(bytes, &mut out);
        if flags.loop_start {
            self.loop_start = self.block;
        }
        self.decoded.extend(out);
        if flags.end {
            self.block = self.loop_start;
            if !flags.loop_repeat {
                self.adsr.stop();
            }
        } else {
            self.block += BLOCK_BYTES;
        }
    }

    /// One output sample, left and right.
    pub(crate) fn run(&mut self) -> (i32, i32) {
        while self.decoded.len() < 16 {
            if self.dead() {
                self.decoded.extend([0; 4]);
            } else {
                self.decode_block();
            }
        }
        let taps = &GAUSS[(self.counter as usize & 0xFF0) >> 4];
        let mut sample = 0i32;
        for (i, &tap) in taps.iter().enumerate() {
            sample = (sample + ((self.decoded[i] as i32 * tap as i32) >> 15)) as i16 as i32;
        }
        self.counter += self.pitch.min(MAX_PITCH);
        let steps = self.counter >> 12;
        self.counter &= 0xFFF;
        for _ in 0..steps {
            self.decoded.pop_front();
        }
        let sample = scale(sample, self.adsr.level());
        let out = (scale(sample, self.left), scale(sample, self.right));
        self.adsr.run();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block of 28 loud samples (nibbles of 7, no shift or filter).
    fn block(flags: u8) -> [u8; 16] {
        let mut block = [0x77; 16];
        block[0] = 0;
        block[1] = flags;
        block
    }

    #[test]
    fn an_envelope_attacks_decays_to_its_sustain_and_releases() {
        let mut adsr = Adsr::default();
        // The fastest linear attack, a decay to level 7 of 15, sustain held.
        adsr.set(0x00F7, 0x1FC0);
        adsr.attack();
        let mut ticks = 0;
        while adsr.phase() == Phase::Attack {
            adsr.run();
            ticks += 1;
        }
        assert!(ticks < 10, "{ticks}");
        while adsr.phase() == Phase::Decay {
            adsr.run();
        }
        assert_eq!(adsr.phase(), Phase::Sustain);
        assert!(adsr.level() <= 8 << 11);
        adsr.release();
        for _ in 0..1_000_000 {
            if adsr.phase() == Phase::Stopped {
                break;
            }
            adsr.run();
        }
        assert_eq!(adsr.phase(), Phase::Stopped);
    }

    #[test]
    fn a_voice_stops_at_its_samples_end_unless_it_loops() {
        let one: Arc<[u8]> = [block(0), block(1)].concat().into();
        let mut voice = Voice::key_on(one, 0, 0x1000, (0x00FF, 0x1FC0));
        voice.set_volume(0x3FFF, 0x3FFF);
        let mut heard = false;
        for _ in 0..200 {
            heard |= voice.run().0 != 0;
        }
        assert!(heard);
        assert!(voice.dead(), "the end flag without a repeat stops it");

        // Block 1 starts the loop, block 2 ends it and repeats.
        let looped: Arc<[u8]> = [block(0), block(4), block(3)].concat().into();
        let mut voice = Voice::key_on(looped, 0, 0x1000, (0x00FF, 0x1FC0));
        for _ in 0..10_000 {
            voice.run();
        }
        assert!(!voice.dead());
        assert_eq!(voice.loop_start, 16);
        voice.key_off();
        for _ in 0..1_000_000 {
            if voice.dead() {
                break;
            }
            voice.run();
        }
        assert!(voice.dead());
    }

    #[test]
    fn pitch_sets_how_fast_the_sample_goes_by() {
        let long: Arc<[u8]> = [vec![block(0); 99], vec![block(1)]]
            .concat()
            .concat()
            .into();
        let length = |pitch: u32| {
            let mut voice = Voice::key_on(long.clone(), 0, pitch, (0x00FF, 0x1FC0));
            let mut n = 0;
            while !voice.dead() {
                voice.run();
                n += 1;
            }
            n
        };
        // 2800 samples last about 2800 outputs at 0x1000 and half as many at twice the pitch.
        let normal = length(0x1000);
        let fast = length(0x2000);
        assert!((2700..2900).contains(&normal), "{normal}");
        assert!((fast as f32 / normal as f32 - 0.5).abs() < 0.02, "{fast}");
    }
}
