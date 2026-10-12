//! The simulation's random numbers.

use bevy::prelude::*;

/// The one source of randomness for gameplay: a PCG32 generator (O'Neill, "PCG: A Family of
/// Simple Fast Space-Efficient Statistically Good Algorithms for Random Number
/// Generation", 2014), reseeded from the level's seed when a level starts.
///
/// It is integer-only, so it gives the same sequence on every platform, and it is two
/// `u64`s, so rollback can snapshot it by copying. Gameplay must not use any other random
/// source.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimRng {
    state: u64,
    increment: u64,
}

const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

impl SimRng {
    /// A generator for `seed`. Different `stream`s give independent sequences from the same
    /// seed.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            increment: (stream << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(self.increment);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rotation = (old >> 59) as u32;
        xorshifted.rotate_right(rotation)
    }

    /// A uniform integer in `0..bound`, without modulo bias. Returns 0 when `bound` is 0.
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        // Reject the few values at the bottom that would make some results more likely.
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next_u32();
            if value >= threshold {
                return value % bound;
            }
        }
    }

    /// A uniform float in `[0, 1)`, built from 24 random bits so every value is exact.
    pub fn unit_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// A uniform float in `[low, high)`.
    pub fn range_f32(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit_f32()
    }
}

impl Default for SimRng {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_pcg32_reference_sequence() {
        // The first outputs of the PCG reference implementation's demo, seeded with 42 on
        // stream 54.
        let mut rng = SimRng::new(42, 54);
        let outputs: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
        assert_eq!(
            outputs,
            [
                0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e
            ]
        );
    }

    #[test]
    fn bounded_values_stay_in_range() {
        let mut rng = SimRng::new(7, 0);
        for bound in [1, 2, 3, 10, 1000, u32::MAX] {
            for _ in 0..100 {
                assert!(rng.below(bound) < bound);
            }
        }
        assert_eq!(rng.below(0), 0);
    }

    #[test]
    fn floats_stay_in_range() {
        let mut rng = SimRng::new(7, 1);
        for _ in 0..1000 {
            let unit = rng.unit_f32();
            assert!((0.0..1.0).contains(&unit));
            let ranged = rng.range_f32(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&ranged));
        }
    }

    #[test]
    fn streams_differ() {
        let mut a = SimRng::new(1, 1);
        let mut b = SimRng::new(1, 2);
        assert_ne!(
            (0..4).map(|_| a.next_u32()).collect::<Vec<_>>(),
            (0..4).map(|_| b.next_u32()).collect::<Vec<_>>()
        );
    }
}
