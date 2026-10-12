//! How the games read the movement stick (`docs/research/hero-controller.md`, "Input").

use bevy::prelude::*;

use crate::profile::StickProfile;

/// The stick as the hero code sees it.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Stick {
    /// Each axis in −1..=1 after the dead zone: x right, y forward. Zero when the stick is
    /// under the cut-off.
    pub vector: Vec2,
    /// How hard it is pushed, 0..=1: `(max(|x|, |y|) + length) / 2`, so diagonals count
    /// as fuller than their length.
    pub magnitude: f32,
}

impl Stick {
    pub fn is_active(&self) -> bool {
        self.magnitude > 0.0
    }
}

pub fn read_stick(counts: [i8; 2], profile: &StickProfile) -> Stick {
    let axis = |count: i8| {
        let from_centre = f32::from(count).abs();
        let output = ((from_centre - profile.dead_zone) / profile.live_range).clamp(0.0, 1.0);
        output.copysign(f32::from(count))
    };
    let vector = Vec2::new(axis(counts[0]), axis(counts[1]));
    let length = vector.length();
    if length < profile.cut_off {
        return Stick::default();
    }
    let largest = vector.x.abs().max(vector.y.abs());
    Stick {
        vector,
        magnitude: ((largest + length) / 2.0).clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HeroProfile;

    fn read(x: i8, y: i8) -> Stick {
        read_stick([x, y], &HeroProfile::uya().stick)
    }

    #[test]
    fn dead_zone_and_full_range() {
        assert_eq!(read(48, 0), Stick::default());
        assert_eq!(read(124, 0).vector, Vec2::new(1.0, 0.0));
        assert_eq!(read(-127, 0).vector, Vec2::new(-1.0, 0.0));
        // 86 counts is half way through the live range.
        assert_eq!(read(0, 86).vector, Vec2::new(0.0, 0.5));
    }

    #[test]
    fn short_vectors_are_cut_off() {
        // 0.2 of full output on one axis is under the 0.25 cut-off.
        let counts = (48.0 + 0.2 * 76.0) as i8;
        assert!(!read(counts, 0).is_active());
        // About half the axis's travel is needed (`hero-controller.md`: "about 67 counts").
        assert!(!read(66, 0).is_active());
        assert!(read(68, 0).is_active());
    }

    #[test]
    fn diagonals_count_as_full() {
        let stick = read(127, 127);
        assert_eq!(stick.magnitude, 1.0);
        let half = read(86, 0);
        assert_eq!(half.magnitude, 0.5);
    }
}
