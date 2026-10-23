// A playing sound effect, after OpenGOAL's 989snd (`game/sound/989snd/blocksound_handler.cpp`,
// `sfxgrain.cpp`, `lfo.cpp`).
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

//! A sound effect being played: it runs its sound's grains 240 times a second, starting
//! tones on voices, and keeps their volume, pan and pitch as the game and its LFOs move
//! them.

use std::sync::Arc;

use rac_formats::sound::{Grain, GrainOp, Lfo, PlaySound, Sound, SoundBank, Tone};

use crate::{Mixer, Rng};

/// A pan that asks for the sound's own.
pub(crate) const PAN_RESET: i32 = -1;
/// A pan, or a volume, that keeps what there is.
pub(crate) const PAN_KEEP: i32 = -2;
pub(crate) const VOLUME_KEEP: i32 = 0x7FFF_FFFF;

// The control grains (`rac_formats::sound::GrainOp::Control`'s op).
const LOOP_START: u8 = 21;
const LOOP_END: u8 = 22;
const LOOP_CONTINUE: u8 = 23;
const STOP: u8 = 24;
const RAND_PLAY: u8 = 25;
const RAND_PB: u8 = 27;
const PB: u8 = 28;
const ADD_PB: u8 = 29;
const SET_REGISTER: u8 = 30;
const SET_REGISTER_RAND: u8 = 31;
const INC_REGISTER: u8 = 32;
const DEC_REGISTER: u8 = 33;
const TEST_REGISTER: u8 = 34;
const MARKER: u8 = 35;
const GOTO_MARKER: u8 = 36;
const GOTO_RANDOM_MARKER: u8 = 37;
const WAIT_FOR_ALL_VOICES: u8 = 38;
const PLAY_CYCLE: u8 = 39;
const ADD_REGISTER: u8 = 40;
const KEY_OFF_VOICES: u8 = 41;
const KILL_VOICES: u8 = 42;
const ON_STOP_MARKER: u8 = 43;
const COPY_REGISTER: u8 = 44;

/// What a sound is played with: its volume (0x400 is full), pan in degrees (or
/// [`PAN_RESET`]), pitch modulation in 128ths of a semitone and pitch bend (±0x8000 bends
/// by the tone's range), and its registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Params {
    pub(crate) volume: i32,
    pub(crate) pan: i32,
    pub(crate) pitch_mod: i32,
    pub(crate) pitch_bend: i32,
    pub(crate) registers: [i8; 4],
}

/// What an LFO moves.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Target {
    #[default]
    None,
    Volume,
    Pan,
    PitchMod,
    PitchBend,
    Other,
}

/// The shape of an LFO's wave.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Shape {
    #[default]
    Off,
    Sine,
    Square,
    Triangle,
    Saw,
    Random,
}

#[derive(Debug, Default, Clone, Copy)]
struct LfoTracker {
    shape: Shape,
    target: Target,
    setup_flags: u16,
    depth: i32,
    next_step: i32,
    step_size: u32,
    hold1: i32,
    hold2: i32,
    range: i32,
    tick: u32,
}

/// 2048 steps of a cosine, out of 32767.
fn sine(step: i32) -> i32 {
    let turn = step.rem_euclid(2048) as f64 / 2048.0;
    (32767.0 * (turn * std::f64::consts::TAU).cos()) as i32
}

impl LfoTracker {
    fn value(&mut self, multiple: u32, rng: &mut Rng) -> i32 {
        let step = self.next_step >> 16;
        self.next_step = self
            .next_step
            .wrapping_add(multiple.wrapping_mul(self.step_size) as i32);
        if self.next_step > 0x7FF_FFFF {
            self.next_step -= 0x800_0000;
        }
        let value = match self.shape {
            Shape::Off => 0,
            Shape::Sine => sine(step),
            Shape::Square => {
                if step >= self.hold1 {
                    -32767
                } else {
                    32767
                }
            }
            Shape::Triangle => {
                if step < 512 {
                    0x7FFF * step / 512
                } else if step >= 1536 {
                    0x7FFF * (step - 1536) / 512 - 0x7FFF
                } else {
                    0x7FFF - 65534 * (step - 512) / 1024
                }
            }
            Shape::Saw => {
                if step >= 1024 {
                    0x7FFF * (step - 1024) / 1024 - 0x7FFF
                } else {
                    0x7FFF * step / 1023
                }
            }
            Shape::Random => {
                if step >= 1024 && self.hold2 == 1 {
                    self.hold2 = 0;
                    self.hold1 = 2 * ((rng.next() as i32 & 0x7FFF) - 0x3FFF);
                } else if step < 1024 && self.hold2 == 0 {
                    self.hold2 = 1;
                    self.hold1 = -(rng.next() as i32 & 0x7FFF) * (rng.next() as i32 & 1);
                }
                self.hold1
            }
        };
        if self.setup_flags & 1 != 0 {
            -value
        } else {
            value
        }
    }
}

/// A sound being played, and the child sounds it started.
#[derive(Debug, Clone)]
pub(crate) struct Handler {
    bank: Arc<SoundBank>,
    pub(crate) bank_index: usize,
    /// The sound it plays now (a branch moves it on), and the one it started as.
    sound: usize,
    started_as: usize,
    group: i32,
    done: bool,
    grains_to_play: u32,
    grains_to_skip: u32,
    skip_grains: bool,
    /// Its voices, by id; a voice that has died is gone from the mixer.
    voices: Vec<u64>,
    children: Vec<Handler>,
    orig_volume: i32,
    cur_volume: i32,
    cur_pan: i32,
    cur_pm: i32,
    cur_pb: i32,
    app_volume: i32,
    app_pan: i32,
    app_pm: i32,
    app_pb: i32,
    lfo_volume: i32,
    lfo_pan: i32,
    lfo_pm: i32,
    lfo_pb: i32,
    note: i32,
    fine: i32,
    registers: [i8; 4],
    lfos: [LfoTracker; 4],
    countdown: i32,
    next_grain: isize,
}

/// A register's value for a tone's or child's volume or pan, from −1 to −4 the sound's
/// own, −5 a random one, and below the block's.
fn register(value: i32, registers: &[i8; 4], mixer: &mut Mixer, random: i32) -> i32 {
    if value >= -4 {
        registers[(-value - 1) as usize] as i32
    } else if value == -5 {
        random
    } else {
        mixer
            .block_registers
            .get((-value - 6) as usize)
            .copied()
            .unwrap_or(0) as i32
    }
}

fn wrap_pan(mut pan: i32) -> i32 {
    pan = pan.rem_euclid(360);
    pan
}

impl Handler {
    /// Starts sound `sound` of `bank` with its own volume and pan (`None` takes the
    /// sound's), and runs its first grains. `None` for a sound with no grains.
    pub(crate) fn new(
        bank: Arc<SoundBank>,
        bank_index: usize,
        sound: usize,
        volume: Option<i32>,
        pan: Option<i32>,
        params: Params,
        mixer: &mut Mixer,
    ) -> Option<Self> {
        let sfx = bank.sounds.get(sound)?;
        let first_delay = sfx.grains.first()?.delay;
        let sfx_volume = volume.unwrap_or(sfx.volume as i32);
        let sfx_pan = pan.unwrap_or(sfx.pan as i32);
        let app_volume = if params.volume == VOLUME_KEEP {
            0x400
        } else {
            params.volume
        };
        let pan = if params.pan == PAN_RESET || params.pan == PAN_KEEP {
            sfx_pan
        } else {
            params.pan
        };
        let mut handler = Self {
            group: sfx.group as i32,
            bank,
            bank_index,
            sound,
            started_as: sound,
            done: false,
            grains_to_play: 0,
            grains_to_skip: 0,
            skip_grains: false,
            voices: Vec::new(),
            children: Vec::new(),
            orig_volume: sfx_volume,
            cur_volume: ((sfx_volume * app_volume) >> 10).min(127),
            cur_pan: pan,
            cur_pm: params.pitch_mod,
            cur_pb: params.pitch_bend,
            app_volume,
            app_pan: pan,
            app_pm: params.pitch_mod,
            app_pb: params.pitch_bend,
            lfo_volume: 0,
            lfo_pan: 0,
            lfo_pm: 0,
            lfo_pb: 0,
            note: 60,
            fine: 0,
            registers: params.registers,
            lfos: [LfoTracker::default(); 4],
            countdown: first_delay,
            next_grain: 0,
        };
        while handler.countdown <= 0 && !handler.done {
            handler.do_grain(mixer);
        }
        Some(handler)
    }

    fn sfx(&self) -> &Sound {
        &self.bank.sounds[self.sound]
    }

    /// The sound it was started as; a branch grain may have moved it on since.
    pub(crate) fn started_as(&self) -> usize {
        self.started_as
    }

    /// One tick of 240 a second; true once it has finished and its voices have died.
    pub(crate) fn tick(&mut self, mixer: &mut Mixer) -> bool {
        self.voices.retain(|&id| mixer.alive(id));
        for index in 0..self.lfos.len() {
            self.tick_lfo(index, mixer);
        }
        self.children.retain_mut(|child| !child.tick(mixer));
        if self.done && self.children.is_empty() {
            return self.voices.is_empty();
        }
        self.countdown -= 1;
        while self.countdown <= 0 && !self.done {
            self.do_grain(mixer);
        }
        false
    }

    pub(crate) fn stop(&mut self, mixer: &mut Mixer) {
        self.done = true;
        for child in &mut self.children {
            child.stop(mixer);
        }
        for &id in &self.voices {
            if let Some(voice) = mixer.voice(id) {
                voice.voice.key_off();
            }
        }
    }

    pub(crate) fn set_vol_pan(&mut self, volume: i32, pan: i32, mixer: &mut Mixer) {
        if volume >= 0 {
            if volume != VOLUME_KEEP {
                self.app_volume = volume;
            }
        } else {
            self.app_volume = -1024 * volume / 127;
        }
        if pan == PAN_RESET {
            self.app_pan = self.sfx().pan as i32;
        } else if pan != PAN_KEEP {
            self.app_pan = pan;
        }
        let new_volume =
            (((self.app_volume * self.orig_volume) >> 10) + self.lfo_volume).clamp(0, 127);
        let new_pan = wrap_pan(self.app_pan + self.lfo_pan);
        if new_pan != self.cur_pan || new_volume != self.cur_volume {
            self.cur_volume = new_volume;
            self.cur_pan = new_pan;
            let child_volume = self.app_volume * self.orig_volume / 127;
            for child in &mut self.children {
                child.set_vol_pan(child_volume, pan, mixer);
            }
            for &id in &self.voices {
                let (volume, pan) = (self.cur_volume, self.cur_pan);
                let group = self.group;
                if let Some(slot) = mixer.voice_ref(id) {
                    let (tone_volume, tone_pan) = (slot.volume, slot.pan);
                    let base = mixer.make_volume(127, 0, volume, pan, tone_volume, tone_pan);
                    mixer.set_voice_volume(id, base, group);
                }
            }
        }
    }

    pub(crate) fn set_pitch_mod(&mut self, pitch_mod: i32, mixer: &mut Mixer) {
        for child in &mut self.children {
            child.set_pitch_mod(pitch_mod, mixer);
        }
        self.app_pm = pitch_mod;
        self.update_pitch(mixer);
    }

    fn set_pitch_bend(&mut self, bend: i32, mixer: &mut Mixer) {
        for child in &mut self.children {
            child.set_pitch_bend(bend, mixer);
        }
        self.app_pb = bend;
        self.update_pitch(mixer);
    }

    fn update_pitch(&mut self, mixer: &mut Mixer) {
        self.cur_pm = self.app_pm + self.lfo_pm;
        self.cur_pb = (self.app_pb + self.lfo_pb).clamp(i16::MIN as i32, i16::MAX as i32);
        for &id in &self.voices {
            if let Some(slot) = mixer.voice(id) {
                let pitch =
                    crate::pitch(&slot.tone, self.cur_pb, self.cur_pm, self.note, self.fine);
                slot.voice.pitch = pitch;
            }
        }
    }

    fn do_grain(&mut self, mixer: &mut Mixer) {
        let grain = self.sfx().grains[self.next_grain as usize].clone();
        let extra = self.run(&grain, mixer);
        if self.skip_grains {
            self.grains_to_play = self.grains_to_play.saturating_sub(1);
            if self.grains_to_play == 0 {
                self.next_grain += self.grains_to_skip as isize;
                self.skip_grains = false;
            }
        }
        self.next_grain += 1;
        let grains = &self.sfx().grains;
        if self.next_grain < 0 || self.next_grain as usize >= grains.len() {
            self.done = true;
            return;
        }
        self.countdown = grains[self.next_grain as usize].delay + extra;
    }

    /// Runs one grain; returns ticks to add to the next one's delay.
    fn run(&mut self, grain: &Grain, mixer: &mut Mixer) -> i32 {
        match &grain.op {
            GrainOp::Null | GrainOp::Plugin { .. } => 0,
            GrainOp::Tone(tone) => {
                self.tone(tone, mixer);
                0
            }
            GrainOp::Lfo(lfo) => {
                self.set_lfo(lfo, mixer);
                0
            }
            GrainOp::StartChild(play) => {
                self.start_child(play, mixer);
                0
            }
            GrainOp::StopChild(play) => {
                let sound = play.sound;
                if sound >= 0 {
                    for child in &mut self.children {
                        if child.sound == sound as usize {
                            child.stop(mixer);
                        }
                    }
                    self.children.retain(|child| child.sound != sound as usize);
                }
                0
            }
            GrainOp::Branch(play) => {
                self.branch(play, mixer);
                0
            }
            GrainOp::RandDelay(amount) => {
                if *amount > 0 {
                    (mixer.rng.next() % *amount as u32) as i32
                } else {
                    0
                }
            }
            GrainOp::Control { op, args } => self.control(*op, args, mixer),
        }
    }

    fn tone(&mut self, tone: &Tone, mixer: &mut Mixer) {
        self.cur_volume =
            (((self.app_volume * self.orig_volume) >> 10) + self.lfo_volume).clamp(0, 127);
        self.cur_pan = wrap_pan(self.app_pan + self.lfo_pan);
        // Noise isn't played, nor tones only sent to the reverb, which isn't modelled.
        if tone.flags & 0x18 != 0 {
            return;
        }
        let mut volume = tone.volume as i32;
        if volume < 0 {
            let random = (mixer.rng.next() % 0x7F) as i32;
            volume = register(volume, &self.registers, mixer, random);
        }
        let volume = volume.max(0);
        let mut pan = tone.pan as i32;
        if pan < 0 {
            let random = (mixer.rng.next() % 360) as i32;
            let value = register(pan, &self.registers, mixer, random);
            pan = if pan == -5 { value } else { 360 * value / 127 };
        }
        let pan = wrap_pan(pan);
        let base = mixer.make_volume(127, 0, self.cur_volume, self.cur_pan, volume, pan);
        let pitch = crate::pitch(tone, self.cur_pb, self.cur_pm, self.note, self.fine);
        let id = mixer.start_tone(self.bank_index, tone, pitch, base, self.group, volume, pan);
        if let Some(id) = id {
            self.voices.push(id);
        }
    }

    fn set_lfo(&mut self, settings: &Lfo, mixer: &mut Mixer) {
        let Some(lfo) = self.lfos.get_mut(settings.which as usize) else {
            return;
        };
        lfo.target = match settings.target {
            0 => Target::None,
            1 => Target::Volume,
            2 => Target::Pan,
            3 => Target::PitchMod,
            4 => Target::PitchBend,
            _ => Target::Other,
        };
        if lfo.target == Target::None {
            lfo.shape = Shape::Off;
            return;
        }
        lfo.shape = match settings.shape {
            1 => Shape::Sine,
            2 => Shape::Square,
            3 => Shape::Triangle,
            4 => Shape::Saw,
            5 => Shape::Random,
            _ => Shape::Off,
        };
        lfo.setup_flags = settings.flags;
        lfo.depth = settings.depth as i16 as i32;
        lfo.step_size = settings.step_size;
        lfo.hold1 = if lfo.shape == Shape::Square {
            settings.duty_cycle as i32
        } else {
            0
        };
        lfo.hold2 = 0;
        lfo.next_step = if lfo.setup_flags & 2 != 0 {
            ((mixer.rng.next() & 0x7FF) << 16) as i32
        } else {
            (settings.start_offset as i32) << 16
        };
        if lfo.shape == Shape::Random {
            lfo.hold1 = -(mixer.rng.next() as i32 & 0x7FFF) * (mixer.rng.next() as i32 & 1);
            lfo.hold2 = 1;
        }
        let volume = self.sfx().volume as i32;
        let lfo = &mut self.lfos[settings.which as usize];
        lfo.range = match lfo.target {
            Target::Volume => (volume * lfo.depth) >> 10,
            Target::Pan => (180 * lfo.depth) >> 10,
            Target::PitchMod => (6096 * lfo.depth) >> 10,
            Target::PitchBend => (0x7FFF * lfo.depth) >> 10,
            _ => lfo.range,
        };
        self.tick_lfo(settings.which as usize, mixer);
    }

    fn tick_lfo(&mut self, index: usize, mixer: &mut Mixer) {
        let lfo = &mut self.lfos[index];
        lfo.tick = lfo.tick.wrapping_add(1);
        if lfo.target == Target::None || lfo.tick & 1 == 0 {
            return;
        }
        let range = lfo.range;
        match lfo.target {
            Target::Volume => {
                let volume = (range * (lfo.value(2, &mut mixer.rng) - 0x7FFF)) >> 16;
                if self.lfo_volume != volume {
                    self.lfo_volume = volume;
                    self.set_vol_pan(VOLUME_KEEP, PAN_KEEP, mixer);
                }
            }
            Target::Pan => {
                let pan = (range * lfo.value(2, &mut mixer.rng)) >> 15;
                if self.lfo_pan != pan {
                    self.lfo_pan = pan;
                    self.set_vol_pan(VOLUME_KEEP, PAN_KEEP, mixer);
                }
            }
            Target::PitchMod => {
                let pitch_mod = (lfo.value(2, &mut mixer.rng) * range) >> 15;
                if self.lfo_pm != pitch_mod {
                    self.lfo_pm = pitch_mod;
                    self.update_pitch(mixer);
                }
            }
            Target::PitchBend => {
                let bend = (lfo.value(2, &mut mixer.rng) * range) >> 15;
                if self.lfo_pb != bend {
                    self.lfo_pb = bend;
                    self.update_pitch(mixer);
                }
            }
            Target::None | Target::Other => {}
        }
    }

    fn start_child(&mut self, play: &PlaySound, mixer: &mut Mixer) {
        if play.sound < 0 {
            return;
        }
        let mut volume = play.volume;
        if volume < 0 {
            let random = (mixer.rng.next() % 0x7F) as i32;
            volume = register(volume, &self.registers, mixer, random);
        }
        let volume = volume.abs().clamp(0, 127);
        let mut pan = play.pan;
        if pan < 0 {
            let random = (mixer.rng.next() % 360) as i32;
            let value = register(pan, &self.registers, mixer, random);
            pan = if pan == -5 {
                value
            } else {
                360 * value.abs().min(127) / 127
            };
        }
        let params = Params {
            volume: self.app_volume * self.orig_volume / 127,
            pan: self.app_pan,
            pitch_mod: self.app_pm,
            pitch_bend: self.app_pb,
            registers: self.registers,
        };
        let child = Handler::new(
            self.bank.clone(),
            self.bank_index,
            play.sound as usize,
            Some(volume),
            Some(pan),
            params,
            mixer,
        );
        if let Some(child) = child {
            self.children.push(child);
        }
    }

    fn branch(&mut self, play: &PlaySound, mixer: &mut Mixer) {
        let Some(sound) = usize::try_from(play.sound)
            .ok()
            .filter(|&sound| sound < self.bank.sounds.len())
        else {
            return;
        };
        for &id in &self.voices {
            if let Some(slot) = mixer.voice(id) {
                slot.voice.key_off();
                slot.voice.set_volume(0, 0);
            }
        }
        self.sound = sound;
        let sfx = &self.bank.sounds[sound];
        self.orig_volume = sfx.volume as i32;
        self.cur_volume = (self.app_volume * sfx.volume as i32) >> 10;
        self.group = sfx.group as i32;
        self.countdown = sfx.grains.first().map_or(0, |grain| grain.delay);
        self.next_grain = -1;
        self.skip_grains = false;
        self.grains_to_play = 0;
        self.grains_to_skip = 0;
    }

    /// A register of the sound's own, or below 0 one of the block's.
    fn register_mut<'a>(&'a mut self, register: i16, mixer: &'a mut Mixer) -> Option<&'a mut i8> {
        if register < 0 {
            mixer.block_registers.get_mut((-register - 1) as usize)
        } else {
            self.registers.get_mut(register as usize)
        }
    }

    fn control(&mut self, op: u8, args: &[i16; 4], mixer: &mut Mixer) -> i32 {
        let grains = self.sfx().grains.len() as isize;
        match op {
            LOOP_START | MARKER => {}
            LOOP_END => {
                let found = (0..self.next_grain.max(0))
                    .rev()
                    .find(|&i| self.is_control(i, LOOP_START));
                if let Some(i) = found {
                    self.next_grain = i - 1;
                }
            }
            LOOP_CONTINUE => {
                let found = (self.next_grain + 1..grains).find(|&i| self.is_control(i, LOOP_END));
                if let Some(i) = found {
                    self.next_grain = i;
                }
            }
            STOP => self.done = true,
            RAND_PLAY => {
                let (options, count) = (args[0] as i32, args[1] as i32);
                if options > 0 {
                    let key = (self.bank_index, self.sound, self.next_grain as usize);
                    let previous = mixer
                        .grain_memory
                        .get(&key)
                        .copied()
                        .unwrap_or(args[2] as i32);
                    let mut pick = (mixer.rng.next() % options as u32) as i32;
                    if pick == previous {
                        pick += 1;
                        if pick >= options {
                            pick = 0;
                        }
                    }
                    mixer.grain_memory.insert(key, pick);
                    self.next_grain += (pick * count) as isize;
                    self.grains_to_play = (count + 1).max(0) as u32;
                    self.grains_to_skip = ((options - 1 - pick) * count).max(0) as u32;
                    self.skip_grains = true;
                }
            }
            RAND_PB => {
                let random = (mixer.rng.next() % 0x7FFF) as i32;
                let bend = args[0] as i32 * ((0xFFFF * random) / 0x7FFF - 0x8000) / 100;
                self.set_pitch_bend(bend, mixer);
            }
            ADD_PB => {
                let bend = (self.cur_pb + 0x7FFF * args[0] as i32 / 127)
                    .clamp(i16::MIN as i32, i16::MAX as i32);
                self.set_pitch_bend(bend, mixer);
            }
            PB => {
                let value = args[0] as i32;
                let bend = if value >= 0 {
                    0x7FFF * value / 127
                } else {
                    -0x8000 * value / -128
                };
                self.set_pitch_bend(bend, mixer);
            }
            SET_REGISTER => {
                if let Some(register) = self.register_mut(args[0], mixer) {
                    *register = args[1] as i8;
                }
            }
            SET_REGISTER_RAND => {
                let (low, high) = (args[1] as i32, args[2] as i32);
                let range = high - low + 1;
                let value = if range > 0 {
                    (mixer.rng.next() % range as u32) as i32 + low
                } else {
                    low
                };
                if let Some(register) = self.register_mut(args[0], mixer) {
                    *register = value as i8;
                }
            }
            INC_REGISTER | DEC_REGISTER => {
                let by = if op == INC_REGISTER { 1 } else { -1 };
                if let Some(register) = self.register_mut(args[0], mixer) {
                    *register = (*register as i32 + by).clamp(-128, 127) as i8;
                }
            }
            ADD_REGISTER => {
                let value = args[0] as i32;
                if let Some(register) = self.register_mut(args[1], mixer) {
                    *register = (*register as i32 + value).clamp(-128, 127) as i8;
                }
            }
            COPY_REGISTER => {
                let value = self.register_mut(args[0], mixer).map_or(0, |r| *r);
                if let Some(register) = self.register_mut(args[1], mixer) {
                    *register = value;
                }
            }
            TEST_REGISTER => {
                let value = self.register_mut(args[0], mixer).map_or(0, |r| *r) as i32;
                let (action, compare) = (args[1], args[2] as i32);
                let skip = match action {
                    0 => value >= compare,
                    1 => value != compare,
                    _ => compare >= value,
                };
                if skip {
                    self.next_grain += 1;
                }
            }
            GOTO_MARKER | GOTO_RANDOM_MARKER => {
                let mark = if op == GOTO_MARKER {
                    args[0] as i32
                } else {
                    let (low, high) = (args[0] as i32, args[1] as i32);
                    let range = high - low + 1;
                    if range > 0 {
                        (mixer.rng.next() % range as u32) as i32 + low
                    } else {
                        low
                    }
                };
                let found = (0..grains).find(|&i| match &self.sfx().grains[i as usize].op {
                    GrainOp::Control { op: MARKER, args } => args[0] as i32 == mark,
                    _ => false,
                });
                if let Some(i) = found {
                    self.next_grain = i - 1;
                }
            }
            WAIT_FOR_ALL_VOICES => {
                if !self.voices.is_empty() {
                    self.next_grain -= 1;
                    return 1;
                }
            }
            PLAY_CYCLE => {
                let (size, count) = (args[0] as i32, args[1] as i32);
                if size > 0 {
                    let key = (self.bank_index, self.sound, self.next_grain as usize);
                    let index = mixer
                        .grain_memory
                        .get(&key)
                        .copied()
                        .unwrap_or(args[2] as i32);
                    let next = if index + 1 >= size { 0 } else { index + 1 };
                    mixer.grain_memory.insert(key, next);
                    self.next_grain += (count * index) as isize;
                    self.grains_to_play = (count + 1).max(0) as u32;
                    self.grains_to_skip = ((size - 1 - index) * count).max(0) as u32;
                    self.skip_grains = true;
                }
            }
            KEY_OFF_VOICES | KILL_VOICES => {
                for &id in &self.voices {
                    if let Some(slot) = mixer.voice(id) {
                        slot.voice.key_off();
                        if op == KILL_VOICES {
                            slot.voice.set_volume(0, 0);
                        }
                    }
                }
            }
            ON_STOP_MARKER => self.next_grain = grains - 1,
            // Pitch modulation registers and the rest aren't used by the banks read.
            _ => {}
        }
        0
    }

    fn is_control(&self, grain: isize, wanted: u8) -> bool {
        matches!(
            self.sfx().grains.get(grain as usize).map(|grain| &grain.op),
            Some(GrainOp::Control { op, .. }) if *op == wanted
        )
    }
}
