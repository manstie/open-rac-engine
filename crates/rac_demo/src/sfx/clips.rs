//! The kit's placeholder sounds, made at start-up from short recipes in `assets/sounds.ron`
//! rather than recorded, so the kit ships no audio files.

use serde::Deserialize;

use super::mixer::{Clip, SAMPLE_RATE};

/// The shape of a recipe's wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Wave {
    Sine,
    Triangle,
    Square,
    Saw,
    /// White noise, its pitch ignored.
    Noise,
}

/// A placeholder sound: a wave at `from` hertz swept `sweep` octaves (down when negative)
/// over `seconds`, rising for `attack` seconds then dying away, with `noise` of white noise
/// mixed in. A looped one holds its volume, and its pitch at `from`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Recipe {
    pub wave: Wave,
    pub from: f32,
    #[serde(default)]
    pub sweep: f32,
    pub seconds: f32,
    #[serde(default)]
    pub attack: f32,
    #[serde(default)]
    pub noise: f32,
    /// 1 is full scale.
    #[serde(default = "full")]
    pub volume: f32,
    /// A second wave this many times the first's pitch, half as loud, for a richer tone;
    /// 0 for none.
    #[serde(default)]
    pub overtone: f32,
    #[serde(default)]
    pub looped: bool,
}

fn full() -> f32 {
    1.0
}

/// A noise source that gives every run the same placeholders.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

fn wave(shape: Wave, phase: f32, noise: &mut Noise) -> f32 {
    let phase = phase.fract();
    match shape {
        Wave::Sine => bevy::math::ops::sin(phase * std::f32::consts::TAU),
        Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        Wave::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Saw => 2.0 * phase - 1.0,
        Wave::Noise => noise.next(),
    }
}

impl Recipe {
    /// The recipe's samples. A loop holds a whole number of its wave's cycles, so it goes
    /// round without a click.
    pub fn render(&self) -> Clip {
        let rate = SAMPLE_RATE as f32;
        let mut frames = (self.seconds.max(0.0) * rate) as usize;
        if self.looped && self.wave != Wave::Noise && self.from > 0.0 {
            let cycles = (self.seconds * self.from).round().max(1.0);
            frames = (cycles / self.from * rate) as usize;
        }
        let sweep = if self.looped { 0.0 } else { self.sweep };
        let mut noise = Noise(0x2545_F491);
        let (mut phase, mut over) = (0.0f32, 0.0f32);
        let mut samples = Vec::with_capacity(frames);
        for frame in 0..frames {
            let t = frame as f32 / frames.max(1) as f32;
            let hz = self.from * bevy::math::ops::exp2(sweep * t);
            phase += hz / rate;
            let mut sample = wave(self.wave, phase, &mut noise);
            if self.overtone > 0.0 {
                over += hz * self.overtone / rate;
                sample = (sample + 0.5 * wave(self.wave, over, &mut noise)) / 1.5;
            }
            if self.noise > 0.0 {
                sample = sample * (1.0 - self.noise) + noise.next() * self.noise;
            }
            samples.push(sample * self.volume * self.envelope(frame as f32 / rate));
        }
        samples.into()
    }

    /// Up over `attack`, then for a one-shot down to nothing by its end (quickly at first).
    fn envelope(&self, at: f32) -> f32 {
        if at < self.attack {
            return at / self.attack;
        }
        if self.looped {
            return 1.0;
        }
        let left = 1.0 - (at - self.attack) / (self.seconds - self.attack).max(1e-3);
        left.max(0.0) * left.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(wave: Wave, looped: bool) -> Recipe {
        Recipe {
            wave,
            from: 440.0,
            sweep: -2.0,
            seconds: 0.25,
            attack: 0.01,
            noise: 0.1,
            volume: 0.5,
            overtone: 2.0,
            looped,
        }
    }

    #[test]
    fn a_one_shot_is_as_long_as_its_recipe_and_dies_away() {
        let clip = recipe(Wave::Saw, false).render();
        assert_eq!(clip.len(), 12_000);
        assert!(clip.iter().all(|s| s.abs() <= 0.5));
        assert!(clip[..2000].iter().any(|s| s.abs() > 0.2));
        assert!(clip[11_900..].iter().all(|s| s.abs() < 0.01));
    }

    #[test]
    fn a_loop_holds_whole_cycles_and_its_volume() {
        let clip = recipe(Wave::Sine, true).render();
        // 110 cycles of 440 Hz.
        assert_eq!(clip.len(), 12_000);
        assert!(clip[11_000..].iter().any(|s| s.abs() > 0.2));
        assert_eq!(
            recipe(Wave::Sine, true).render(),
            clip,
            "the same every run"
        );
    }
}
