// Parts are ported from OpenGOAL's 989snd (`game/sound/989snd/vagvoice.cpp`, `util.cpp`,
// `player.cpp`):
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

//! The games' sound engine, 989snd, for the sound effects in the user's discs' banks
//! (PLAN.md P3.13; `docs/research/effects.md`, "Sounds").
//!
//! A port of the sound-effect half of OpenGOAL's 989snd (ISC, `docs/research/prior-art.md`)
//! with an SPU voice of its own: a [`Synth`] holds banks read by `rac_formats::sound`, plays
//! their sounds by number with a volume, pan and pitch, and renders 48 kHz stereo. Each
//! sound runs its grains 240 times a second (every 200 samples), as the library does on the
//! IOP; its tones play their ADPCM samples through SPU voices with the SPU's envelopes and
//! interpolation. Music banks, reverb, noise and instance limits aren't modelled.
//!
//! No Bevy here: the game feeds the output to its audio device, and moves each sound's
//! volume and pan as the listener moves (its rules are the game's, in the game).

mod handler;
mod spu;
mod tables;

use std::collections::HashMap;
use std::sync::Arc;

use rac_formats::sound::{SoundBank, Tone};

use handler::{Handler, PAN_KEEP, PAN_RESET, Params};
use spu::Voice;
use tables::{NOTE_PITCH, PAN};

/// The output's sample rate, the SPU's.
pub const SAMPLE_RATE: u32 = 48_000;
/// Samples between the sounds' ticks: they run 240 times a second.
const TICK_SAMPLES: u32 = SAMPLE_RATE / 240;
/// Volume groups the library keeps a master volume for.
const GROUPS: usize = 32;
/// A master volume of 1.
const FULL: i32 = 0x400;

/// A bank added to a [`Synth`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BankId(usize);

/// A sound playing, until it ends or is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundHandle(u32);

/// How to play a sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Play {
    /// Out of 0x400, full.
    pub volume: i32,
    /// Degrees round the listener, 0 ahead and 90 to the right; `None` for the sound's own.
    pub pan: Option<i32>,
    /// In 128ths of a semitone.
    pub pitch_mod: i32,
    /// Out of ±0x8000, which bends by the tone's own range.
    pub pitch_bend: i32,
}

impl Default for Play {
    fn default() -> Self {
        Self {
            volume: FULL,
            pan: None,
            pitch_mod: 0,
            pitch_bend: 0,
        }
    }
}

/// A small random number source, as the library's calls to `rand` (0 to 2³¹ − 1). Sounds
/// aren't part of the simulation, so this needn't be the level's.
#[derive(Debug, Clone)]
pub(crate) struct Rng(u32);

impl Rng {
    pub(crate) fn next(&mut self) -> u32 {
        // xorshift32.
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x >> 1
    }
}

/// A voice with the tone it plays and what its sound set it to.
#[derive(Debug, Clone)]
pub(crate) struct VoiceSlot {
    pub(crate) id: u64,
    bank: usize,
    pub(crate) voice: Voice,
    pub(crate) tone: Tone,
    /// The tone's volume and pan once its registers are read.
    pub(crate) volume: i32,
    pub(crate) pan: i32,
}

/// What the playing sounds share: the voices, the block's registers, and the SPU's
/// volume rules.
#[derive(Debug)]
pub(crate) struct Mixer {
    voices: Vec<VoiceSlot>,
    next_voice: u64,
    pub(crate) rng: Rng,
    pub(crate) block_registers: [i8; 32],
    /// What random picks and cycles chose last, kept with the sound as the library keeps it
    /// in the grain itself: by bank, sound and grain.
    pub(crate) grain_memory: HashMap<(usize, usize, usize), i32>,
    group_volume: [i32; GROUPS],
    samples: Vec<Option<Arc<[u8]>>>,
}

impl Mixer {
    pub(crate) fn alive(&self, id: u64) -> bool {
        self.voices.iter().any(|slot| slot.id == id)
    }

    pub(crate) fn voice(&mut self, id: u64) -> Option<&mut VoiceSlot> {
        self.voices.iter_mut().find(|slot| slot.id == id)
    }

    pub(crate) fn voice_ref(&self, id: u64) -> Option<&VoiceSlot> {
        self.voices.iter().find(|slot| slot.id == id)
    }

    /// A voice's left and right volumes from three volumes (out of 127) and pans
    /// (degrees), as `VoiceManager::MakeVolume` does in stereo.
    pub(crate) fn make_volume(
        &self,
        volume1: i32,
        pan1: i32,
        volume2: i32,
        pan2: i32,
        volume3: i32,
        pan3: i32,
    ) -> (i16, i16) {
        let volume = volume1 * 258 * volume2 / 0x7F * volume3 / 0x7F;
        if volume == 0 {
            return (0, 0);
        }
        let mut pan = (pan1 + pan2 + pan3).rem_euclid(360);
        pan = if pan >= 270 { pan - 270 } else { pan + 90 };
        let (left, right) = if pan < 180 {
            PAN[pan as usize]
        } else {
            let (left, right) = PAN[pan as usize - 180];
            (right, left)
        };
        (
            (left as i32 * volume / 0x3FFF) as i16,
            (right as i32 * volume / 0x3FFF) as i16,
        )
    }

    /// The group's master volume on a voice's volume, and the library's curve (the square).
    fn adjust_to_group(&self, volume: i16, group: i32) -> i16 {
        let mut volume = volume as i32;
        if group >= 15 {
            return volume as i16;
        }
        if volume >= 0x7FFF {
            volume = 0x7FFE;
        }
        let master = self.group_volume[group.clamp(0, GROUPS as i32 - 1) as usize];
        volume = volume * master / 0x400;
        let sign = if volume < 0 { -1 } else { 1 };
        (volume * volume / 0x7FFE * sign) as i16
    }

    pub(crate) fn set_voice_volume(&mut self, id: u64, base: (i16, i16), group: i32) {
        let left = self.adjust_to_group(base.0, group);
        let right = self.adjust_to_group(base.1, group);
        if let Some(slot) = self.voice(id) {
            slot.voice.set_volume(left >> 1, right >> 1);
        }
    }

    /// Starts a voice on a tone (`VoiceManager::StartTone`).
    pub(crate) fn start_tone(
        &mut self,
        bank_index: usize,
        tone: &Tone,
        pitch: u32,
        base: (i16, i16),
        group: i32,
        volume: i32,
        pan: i32,
    ) -> Option<u64> {
        let samples = self.samples.get(bank_index)?.clone()?;
        if tone.sample as usize >= samples.len() {
            return None;
        }
        let id = self.next_voice;
        self.next_voice += 1;
        let voice = Voice::key_on(
            samples,
            tone.sample as usize,
            pitch,
            (tone.adsr1, tone.adsr2),
        );
        self.voices.push(VoiceSlot {
            id,
            bank: bank_index,
            voice,
            tone: *tone,
            volume,
            pan,
        });
        self.set_voice_volume(id, base, group);
        Some(id)
    }
}

/// The pitch register of a tone played at `note` and `fine` with a pitch bend and
/// modulation (`PitchBend`, then `PS1Note2Pitch`).
pub(crate) fn pitch(tone: &Tone, bend: i32, modulation: i32, note: i32, fine: i32) -> u32 {
    let base = (note << 7) + fine + modulation;
    let bent = if bend >= 0 {
        tone.pitch_bend_high as i32 * (bend << 7) / 0x7FFF + base
    } else {
        tone.pitch_bend_low as i32 * (bend << 7) / 0x8000 + base
    };
    let (note, fine) = (bent / 128, bent % 128);
    // A positive centre note is a PlayStation sample at 44.1 kHz.
    let at_44 = tone.centre_note >= 0;
    let centre = (tone.centre_note as i32).abs();
    let pitch = note_to_pitch(centre, tone.centre_fine as u8 as i32, note, fine) as u32;
    if at_44 {
        44_100 * pitch / 48_000
    } else {
        pitch
    }
}

/// The SPU pitch of `note` and `fine` for a sample whose own rate is at `centre`
/// (`sceSdNote2Pitch`), 0x1000 being the sample's rate.
fn note_to_pitch(centre: i32, centre_fine: i32, note: i32, fine: i32) -> u16 {
    let fine_total = fine + centre_fine;
    let mut carry = fine_total;
    if fine_total < 0 {
        carry = fine_total + 127;
    }
    carry /= 128;
    let note = note + carry - centre;
    let mut sixths = note / 6;
    if note < 0 {
        sixths -= 1;
    }
    let mut offset2 = fine_total - carry * 128;
    let negative = if note < 0 { -1 } else { 0 };
    if sixths < 0 {
        sixths -= 1;
    }
    let octaves = sixths / 2 - negative;
    let mut shift = octaves - 2;
    let mut offset1 = note - octaves * 12;
    if offset1 < 0 || (offset1 == 0 && offset2 < 0) {
        offset1 += 12;
        shift = octaves - 3;
    }
    if offset2 < 0 {
        offset1 = offset1 - 1 + carry;
        offset2 += (carry + 1) * 128;
    }
    let (offset1, offset2) = (
        offset1.clamp(0, 11) as usize,
        offset2.clamp(0, 127) as usize,
    );
    let mut pitch = NOTE_PITCH[offset1] as i32 * NOTE_PITCH[offset2 + 12] as i32 / 0x10000;
    if shift < 0 {
        let shift = (-shift).min(31);
        pitch = (pitch + (1 << (shift - 1))) >> shift;
    }
    pitch.clamp(0, u16::MAX as i32) as u16
}

/// The sound engine: banks, the sounds playing from them, and their voices.
#[derive(Debug)]
pub struct Synth {
    banks: Vec<Option<Arc<SoundBank>>>,
    sounds: Vec<(SoundHandle, Handler)>,
    next_handle: u32,
    mixer: Mixer,
    /// Samples left until the sounds' next tick.
    countdown: u32,
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

impl Synth {
    pub fn new() -> Self {
        Self {
            banks: Vec::new(),
            sounds: Vec::new(),
            next_handle: 1,
            mixer: Mixer {
                voices: Vec::new(),
                next_voice: 1,
                rng: Rng(0x2545_F491),
                block_registers: [0; 32],
                grain_memory: HashMap::new(),
                group_volume: [FULL; GROUPS],
                samples: Vec::new(),
            },
            countdown: 0,
        }
    }

    /// Adds a bank to play from.
    pub fn add_bank(&mut self, bank: SoundBank) -> BankId {
        let samples: Arc<[u8]> = bank.samples.clone().into();
        self.banks.push(Some(Arc::new(bank)));
        self.mixer.samples.push(Some(samples));
        BankId(self.banks.len() - 1)
    }

    /// Stops a bank's sounds at once and drops it.
    pub fn remove_bank(&mut self, bank: BankId) {
        self.sounds
            .retain(|(_, handler)| handler.bank_index != bank.0);
        if let Some(slot) = self.banks.get_mut(bank.0) {
            *slot = None;
        }
        if let Some(slot) = self.mixer.samples.get_mut(bank.0) {
            *slot = None;
        }
        self.mixer.voices.retain(|slot| slot.bank != bank.0);
    }

    /// The sounds in a bank.
    pub fn sound_count(&self, bank: BankId) -> usize {
        self.bank(bank).map_or(0, |bank| bank.sounds.len())
    }

    fn bank(&self, bank: BankId) -> Option<&Arc<SoundBank>> {
        self.banks.get(bank.0).and_then(Option::as_ref)
    }

    /// Plays sound `sound` of `bank`; `None` if it has no grains or doesn't exist.
    pub fn play(&mut self, bank: BankId, sound: usize, play: Play) -> Option<SoundHandle> {
        let data = self.bank(bank)?.clone();
        let params = Params {
            volume: play.volume,
            pan: play.pan.unwrap_or(PAN_RESET),
            pitch_mod: play.pitch_mod,
            pitch_bend: play.pitch_bend,
            registers: [0; 4],
        };
        let handler = Handler::new(data, bank.0, sound, None, None, params, &mut self.mixer)?;
        let handle = SoundHandle(self.next_handle);
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        self.sounds.push((handle, handler));
        Some(handle)
    }

    /// Whether a sound is still playing (its grains or voices).
    pub fn is_playing(&self, handle: SoundHandle) -> bool {
        self.sounds.iter().any(|(h, _)| *h == handle)
    }

    fn handler(&mut self, handle: SoundHandle) -> Option<&mut Handler> {
        self.sounds
            .iter_mut()
            .find(|(h, _)| *h == handle)
            .map(|(_, handler)| handler)
    }

    /// Moves a sound's volume (out of 0x400) and pan (degrees; `None` keeps it).
    pub fn set_volume_pan(&mut self, handle: SoundHandle, volume: i32, pan: Option<i32>) {
        let Some(index) = self.sounds.iter().position(|(h, _)| *h == handle) else {
            return;
        };
        let handler = &mut self.sounds[index].1;
        handler.set_vol_pan(volume.max(0), pan.unwrap_or(PAN_KEEP), &mut self.mixer);
    }

    /// Sets a sound's pitch modulation, in 128ths of a semitone.
    pub fn set_pitch_mod(&mut self, handle: SoundHandle, pitch_mod: i32) {
        let Some(index) = self.sounds.iter().position(|(h, _)| *h == handle) else {
            return;
        };
        self.sounds[index]
            .1
            .set_pitch_mod(pitch_mod, &mut self.mixer);
    }

    /// Stops a sound: its grains end and its voices release.
    pub fn stop(&mut self, handle: SoundHandle) {
        let Some(index) = self.sounds.iter().position(|(h, _)| *h == handle) else {
            return;
        };
        self.sounds[index].1.stop(&mut self.mixer);
    }

    /// Stops every sound.
    pub fn stop_all(&mut self) {
        for (_, handler) in &mut self.sounds {
            handler.stop(&mut self.mixer);
        }
    }

    /// The sound a handle plays, by its number in its bank.
    pub fn sound_of(&mut self, handle: SoundHandle) -> Option<usize> {
        self.handler(handle).map(|handler| handler.started_as())
    }

    /// Sets a volume group's master volume (out of 0x400).
    pub fn set_group_volume(&mut self, group: usize, volume: i32) {
        if let Some(master) = self.mixer.group_volume.get_mut(group) {
            *master = volume.clamp(0, 0x7FFF);
        }
    }

    /// Voices sounding now.
    pub fn voices(&self) -> usize {
        self.mixer.voices.len()
    }

    /// Renders interleaved stereo at [`SAMPLE_RATE`] into `out`, from -1 to 1.
    pub fn render(&mut self, out: &mut [f32]) {
        for frame in out.as_chunks_mut::<2>().0 {
            if self.countdown == 0 {
                self.tick();
                self.countdown = TICK_SAMPLES;
            }
            self.countdown -= 1;
            let (left, right) = self.mix();
            frame[0] = left as f32 / 32768.0;
            frame[1] = right as f32 / 32768.0;
        }
    }

    /// Every sound's tick, dropping those that have finished.
    fn tick(&mut self) {
        let mixer = &mut self.mixer;
        self.sounds.retain_mut(|(_, handler)| !handler.tick(mixer));
    }

    /// One output sample: every live voice, saturating as the SPU's mix does.
    fn mix(&mut self) -> (i16, i16) {
        self.mixer.voices.retain(|slot| !slot.voice.dead());
        let (mut left, mut right) = (0i32, 0i32);
        for slot in &mut self.mixer.voices {
            let (l, r) = slot.voice.run();
            left = (left + l).clamp(i16::MIN as i32, i16::MAX as i32);
            right = (right + r).clamp(i16::MIN as i32, i16::MAX as i32);
        }
        (left as i16, right as i16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_formats::sound::{Grain, GrainOp, Sound};

    fn tone(sample: u32, centre: i8) -> Tone {
        Tone {
            priority: 0,
            volume: 127,
            centre_note: centre,
            centre_fine: 0,
            pan: 0,
            map_low: 0,
            map_high: 0,
            pitch_bend_low: 2,
            pitch_bend_high: 2,
            adsr1: 0x00FF,
            adsr2: 0x1FC0,
            flags: 0,
            sample,
        }
    }

    /// 50 loud blocks, the last ending the sample; and a looped one.
    fn samples() -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..50 {
            let mut block = [0x77u8; 16];
            block[0] = 0;
            block[1] = if i == 49 { 1 } else { 0 };
            out.extend(block);
        }
        let mut block = [0x77u8; 16];
        block[0] = 0;
        block[1] = 7;
        out.extend(block);
        out
    }

    fn bank(sounds: Vec<Vec<Grain>>) -> SoundBank {
        SoundBank {
            name: *b"test",
            version: 3,
            sounds: sounds
                .into_iter()
                .map(|grains| Sound {
                    volume: 127,
                    group: 0,
                    pan: 0,
                    instance_limit: 0,
                    flags: 0,
                    grains,
                })
                .collect(),
            samples: samples(),
        }
    }

    fn grain(delay: i32, op: GrainOp) -> Grain {
        Grain { delay, op }
    }

    fn loudness(synth: &mut Synth, frames: usize) -> (f32, f32) {
        let mut out = vec![0.0; frames * 2];
        synth.render(&mut out);
        let left = out.iter().step_by(2).map(|s| s.abs()).sum::<f32>();
        let right = out.iter().skip(1).step_by(2).map(|s| s.abs()).sum::<f32>();
        (left, right)
    }

    #[test]
    fn a_tone_sounds_then_the_sound_ends() {
        let mut synth = Synth::new();
        let bank = synth.add_bank(bank(vec![vec![grain(0, GrainOp::Tone(tone(0, -60)))]]));
        let handle = synth.play(bank, 0, Play::default()).unwrap();
        assert_eq!(synth.voices(), 1);
        let (left, right) = loudness(&mut synth, 1000);
        assert!(left > 1.0 && (left - right).abs() < left * 0.01);
        // 1400 samples at the sample's own rate, then the voice and the sound are gone.
        loudness(&mut synth, 2000);
        assert_eq!(synth.voices(), 0);
        assert!(!synth.is_playing(handle));
    }

    #[test]
    fn pan_and_volume_move_the_voice() {
        let mut synth = Synth::new();
        let bank = synth.add_bank(bank(vec![vec![grain(0, GrainOp::Tone(tone(800, -60)))]]));
        let right = Play {
            pan: Some(90),
            ..Play::default()
        };
        let handle = synth.play(bank, 0, right).unwrap();
        let (l, r) = loudness(&mut synth, 500);
        assert!(r > 10.0 * l.max(0.001), "{l} {r}");
        synth.set_volume_pan(handle, 0x100, Some(270));
        let (l2, r2) = loudness(&mut synth, 500);
        assert!(l2 > 10.0 * r2.max(0.001), "{l2} {r2}");
        // A quarter of the volume, squared by the library's curve.
        assert!(l2 < r * 0.2, "{l2} {r}");
        synth.stop(handle);
        loudness(&mut synth, 5000);
        assert!(!synth.is_playing(handle));
    }

    #[test]
    fn grains_wait_their_delay_and_registers_set_a_tones_volume() {
        let mut quiet = tone(0, -60);
        quiet.volume = -1;
        let mut synth = Synth::new();
        let bank = synth.add_bank(bank(vec![vec![
            grain(
                0,
                GrainOp::Control {
                    op: 30,
                    args: [0, 20, 0, 0],
                },
            ),
            // 24 ticks of 200 samples.
            grain(24, GrainOp::Tone(quiet)),
        ]]));
        synth.play(bank, 0, Play::default()).unwrap();
        // The tone starts on the 24th tick after the first.
        let (before, _) = loudness(&mut synth, 23 * 200);
        assert_eq!(before, 0.0);
        let (after, _) = loudness(&mut synth, 400);
        assert!(after > 0.0);
        // Register 0 is 20 of 127, squared: far quieter than a full tone.
        let mut full = Synth::new();
        let bank = full.add_bank(self::bank(vec![vec![grain(
            0,
            GrainOp::Tone(tone(0, -60)),
        )]]));
        full.play(bank, 0, Play::default()).unwrap();
        let (loud, _) = loudness(&mut full, 400);
        assert!(after < loud * 0.1, "{after} {loud}");
    }

    #[test]
    fn notes_set_the_pitch_by_semitones() {
        let base = tone(0, -60);
        assert_eq!(pitch(&base, 0, 0, 60, 0), 0x1000);
        // An octave up and down, and a 44.1 kHz sample.
        let mut low = base;
        low.centre_note = -72;
        assert_eq!(pitch(&low, 0, 0, 60, 0), 0x800);
        let mut high = base;
        high.centre_note = -48;
        assert_eq!(pitch(&high, 0, 0, 60, 0), 0x2000);
        let mut ps1 = base;
        ps1.centre_note = 60;
        assert_eq!(pitch(&ps1, 0, 0, 60, 0), 44_100 * 0x1000 / 48_000);
        // A pitch modulation of 128 is a semitone; a full bend, the tone's 2 semitones.
        let up = pitch(&base, 0, 128, 60, 0) as f32 / 0x1000 as f32;
        assert!((up - 2f32.powf(1.0 / 12.0)).abs() < 1e-3, "{up}");
        let bent = pitch(&base, 0x7FFF, 0, 60, 0) as f32 / 0x1000 as f32;
        assert!((bent - 2f32.powf(2.0 / 12.0)).abs() < 1e-3, "{bent}");
    }

    #[test]
    fn a_looping_sound_plays_until_stopped() {
        let mut synth = Synth::new();
        let bank = synth.add_bank(bank(vec![vec![grain(
            0,
            GrainOp::Tone(tone(50 * 16, -60)),
        )]]));
        let handle = synth.play(bank, 0, Play::default()).unwrap();
        loudness(&mut synth, 48_000);
        assert!(synth.is_playing(handle));
        synth.stop(handle);
        loudness(&mut synth, 5000);
        assert!(!synth.is_playing(handle));
        assert_eq!(synth.voices(), 0);
    }
}
