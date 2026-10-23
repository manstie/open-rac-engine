//! Sound out through Bevy's audio as one endless stereo stream that the game renders itself,
//! so each sound's volume and pan follow the game's rules rather than Bevy's spatial audio
//! (which falls off with the square of the distance from each ear). The kit's [`Mixer`]
//! plays its placeholder clips; a game can put its own synthesizer in its place.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bevy::audio::{ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;

/// The streams' rate, the PS2's.
pub const SAMPLE_RATE: u32 = 48_000;

/// Frames rendered at a time: 5 ms.
const BLOCK_FRAMES: usize = 240;

/// The most clips playing at once; past it the oldest one-shot is dropped.
const MAX_VOICES: usize = 32;

/// How fast a voice's volume moves to a new one, per frame, so changes don't click.
const GAIN_STEP: f32 = 1.0 / 480.0;

/// Something that renders interleaved stereo (left, right, …) at [`SAMPLE_RATE`],
/// overwriting what is in `out`.
pub trait Render: Send + 'static {
    fn render(&mut self, out: &mut [f32]);
}

/// A [`Render`] heard through one [`AudioPlayer`] that never ends.
#[derive(Asset, TypePath, Clone)]
pub struct SoundStream(pub Arc<Mutex<dyn Render>>);

impl Decodable for SoundStream {
    type Decoder = StreamDecoder;

    fn decoder(&self) -> StreamDecoder {
        StreamDecoder {
            render: self.0.clone(),
            block: vec![0.0; 2 * BLOCK_FRAMES],
            at: 2 * BLOCK_FRAMES,
        }
    }
}

/// Renders a block when the audio thread has taken the last one.
pub struct StreamDecoder {
    render: Arc<Mutex<dyn Render>>,
    block: Vec<f32>,
    at: usize,
}

impl Iterator for StreamDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.at == self.block.len() {
            self.render
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .render(&mut self.block);
            self.at = 0;
        }
        let sample = self.block[self.at];
        self.at += 1;
        Some(sample)
    }
}

impl Source for StreamDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("two channels")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).expect("a sample rate")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// A sound's samples, mono at [`SAMPLE_RATE`].
pub type Clip = Arc<[f32]>;

/// One clip playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Voice(u32);

/// The left and right volumes of a sound `volume` loud (1 full) at `pan` degrees round the
/// listener, 0 ahead and 90 to the right: an equal-power pan by how far to the side it is.
pub fn gains(volume: f32, pan: f32) -> [f32; 2] {
    let side = ops::sin(pan.to_radians());
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    [volume * ops::cos(angle), volume * ops::sin(angle)]
}

#[derive(Debug, Clone)]
struct Playing {
    voice: Voice,
    clip: Clip,
    at: usize,
    looped: bool,
    gains: [f32; 2],
    target: [f32; 2],
    /// Fading out to stop.
    stopping: bool,
}

/// Plays clips at the volumes and pans the game gives them.
#[derive(Debug, Default)]
pub struct Mixer {
    playing: Vec<Playing>,
    next: u32,
}

impl Mixer {
    /// Starts `clip` at `gains` ([`gains`]), once or looping until stopped.
    pub fn play(&mut self, clip: Clip, gains: [f32; 2], looped: bool) -> Voice {
        if self.playing.len() >= MAX_VOICES
            && let Some(oldest) = self.playing.iter().position(|playing| !playing.looped)
        {
            self.playing.remove(oldest);
        }
        let voice = Voice(self.next);
        self.next = self.next.wrapping_add(1);
        self.playing.push(Playing {
            voice,
            clip,
            at: 0,
            looped,
            gains,
            target: gains,
            stopping: false,
        });
        voice
    }

    /// Moves a voice to new volumes over a few milliseconds.
    pub fn set_gains(&mut self, voice: Voice, gains: [f32; 2]) {
        if let Some(playing) = self.find(voice)
            && !playing.stopping
        {
            playing.target = gains;
        }
    }

    /// Fades a voice out and drops it.
    pub fn stop(&mut self, voice: Voice) {
        if let Some(playing) = self.find(voice) {
            playing.stopping = true;
            playing.target = [0.0; 2];
        }
    }

    pub fn is_playing(&self, voice: Voice) -> bool {
        self.playing.iter().any(|playing| playing.voice == voice)
    }

    /// How many clips are playing.
    pub fn voices(&self) -> usize {
        self.playing.len()
    }

    fn find(&mut self, voice: Voice) -> Option<&mut Playing> {
        self.playing
            .iter_mut()
            .find(|playing| playing.voice == voice)
    }
}

impl Render for Mixer {
    fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        for playing in &mut self.playing {
            for frame in out.as_chunks_mut::<2>().0 {
                if playing.at >= playing.clip.len() {
                    if !playing.looped || playing.clip.is_empty() {
                        break;
                    }
                    playing.at = 0;
                }
                for (gain, target) in playing.gains.iter_mut().zip(playing.target) {
                    *gain += (target - *gain).clamp(-GAIN_STEP, GAIN_STEP);
                }
                let sample = playing.clip[playing.at];
                frame[0] += sample * playing.gains[0];
                frame[1] += sample * playing.gains[1];
                playing.at += 1;
            }
        }
        self.playing.retain(|playing| {
            let ended = !playing.looped && playing.at >= playing.clip.len();
            let silent = playing.stopping && playing.gains == [0.0; 2];
            !ended && !silent
        });
        for sample in out {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_plays_once_at_its_gains_and_a_loop_until_stopped() {
        let mut mixer = Mixer::default();
        let clip: Clip = vec![0.5; 100].into();
        mixer.play(clip.clone(), [1.0, 0.5], false);
        let looped = mixer.play(clip, [0.2, 0.2], true);
        let mut out = vec![0.0; 2 * 150];
        mixer.render(&mut out);
        assert_eq!(out[0], 0.5 + 0.1);
        assert_eq!(out[1], 0.25 + 0.1);
        assert_eq!(
            out[2 * 120],
            0.1,
            "the first has ended; the loop goes round"
        );
        assert_eq!(mixer.voices(), 1);
        mixer.stop(looped);
        for _ in 0..4 {
            mixer.render(&mut out);
        }
        assert_eq!(mixer.voices(), 0, "faded out and dropped");
    }

    #[test]
    fn sounds_ahead_play_in_both_ears_and_to_the_right_in_the_right() {
        let ahead = gains(1.0, 0.0);
        assert!((ahead[0] - ahead[1]).abs() < 1e-6);
        assert!((ahead[0] * ahead[0] + ahead[1] * ahead[1] - 1.0).abs() < 1e-5);
        let right = gains(1.0, 90.0);
        assert!(right[0].abs() < 1e-6 && (right[1] - 1.0).abs() < 1e-6);
        let behind = gains(1.0, 180.0);
        assert!((behind[0] - behind[1]).abs() < 1e-5);
    }
}
