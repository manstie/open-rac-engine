//! How the games read the movement stick (`docs/research/hero-controller.md`, "Input").

use bevy::{math::ops, prelude::*};
use std::f32::consts::TAU;

use crate::profile::{FlickProfile, StickMagnitude, StickProfile};

/// The stick as the hero code sees it.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Stick {
    /// Each axis in −1..=1 after the dead zone: x right, y forward. Zero when the stick is
    /// under the cut-off.
    pub vector: Vec2,
    /// How hard it is pushed, 0..=1, by the profile's [`StickMagnitude`].
    pub magnitude: f32,
}

impl Stick {
    pub fn is_active(&self) -> bool {
        self.magnitude > 0.0
    }
}

/// Each axis after the pad layer's dead zone, before the hero code's cut-off.
pub fn read_axes(counts: [i8; 2], profile: &StickProfile) -> Vec2 {
    let axis = |count: i8| {
        let from_centre = f32::from(count).abs();
        let output = ((from_centre - profile.dead_zone) / profile.live_range).clamp(0.0, 1.0);
        output.copysign(f32::from(count))
    };
    Vec2::new(axis(counts[0]), axis(counts[1]))
}

pub fn read_stick(counts: [i8; 2], profile: &StickProfile) -> Stick {
    let vector = read_axes(counts, profile);
    let length = vector.length();
    if length < profile.cut_off {
        return Stick::default();
    }
    let magnitude = match profile.magnitude {
        StickMagnitude::LargestAndLength => (vector.x.abs().max(vector.y.abs()) + length) / 2.0,
        StickMagnitude::Length => length,
    };
    Stick {
        vector,
        magnitude: magnitude.clamp(0.0, 1.0),
    }
}

/// Ticks of axes [`StickPresses`] keeps: this tick and the four before.
const PRESS_HISTORY: usize = 5;

/// Spots the stick being pushed a new way, which UYA's pad layer turns into button
/// presses (0x45D5C8; `hero-controller.md`, "Input"). The crouch flip looks for one.
///
/// - Each axis sets a d-pad bit while it is past its dead zone, so an axis leaving the
///   dead zone, or crossing to the other side, presses that direction.
/// - A flick presses a bit of its own ([`FlickProfile`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StickPresses {
    /// The axes after the dead zone, newest at `newest`.
    axes: [Vec2; PRESS_HISTORY],
    newest: usize,
}

impl Default for StickPresses {
    fn default() -> Self {
        Self {
            axes: [Vec2::ZERO; PRESS_HISTORY],
            newest: 0,
        }
    }
}

impl StickPresses {
    /// Takes this tick's axes, from [`read_axes`], and says whether they press anything.
    pub fn update(&mut self, axes: Vec2, flick: &FlickProfile) -> bool {
        let before = self.back(0);
        self.newest = (self.newest + 1) % PRESS_HISTORY;
        self.axes[self.newest] = axes;
        let pressed =
            |now: f32, then: f32| (now > 0.0 && then <= 0.0) || (now < 0.0 && then >= 0.0);
        pressed(axes.x, before.x) || pressed(axes.y, before.y) || self.flicked(flick)
    }

    fn back(&self, ticks: usize) -> Vec2 {
        self.axes[(self.newest + PRESS_HISTORY - ticks) % PRESS_HISTORY]
    }

    fn flicked(&self, flick: &FlickProfile) -> bool {
        let now = self.back(0);
        if now.length() <= flick.to {
            return false;
        }
        // Up from near the centre in a few ticks, through nothing past `to`.
        let within = flick.within_ticks.min(PRESS_HISTORY - 1);
        if self.back(1).length() <= flick.to {
            for ticks in 1..=within {
                let length = self.back(ticks).length();
                if length < flick.from {
                    return true;
                }
                if ticks == within || self.back(ticks + 1).length() > flick.to {
                    break;
                }
            }
        }
        // Swung round. A centred stick's angle is 0, as the pad's atan2 gives.
        let angle = |v: Vec2| ops::atan2(v.y, v.x);
        let swing = flick.swing_degrees.to_radians();
        (1..=flick.swing_ticks.min(PRESS_HISTORY - 1)).any(|ticks| {
            let turned = (angle(now) - angle(self.back(ticks))).rem_euclid(TAU);
            turned.min(TAU - turned) > swing
        })
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
    fn deadlocked_keeps_short_vectors_and_takes_the_length() {
        let dl = HeroProfile::dl().stick;
        // 0.2 of full output on one axis, which UYA cuts off.
        let counts = (48.0 + 0.2 * 76.0) as i8;
        let output = (f32::from(counts) - 48.0) / 76.0;
        assert!(close(read_stick([counts, 0], &dl).magnitude, output));
        // Half on both axes: UYA counts the diagonal fuller than its length.
        let length = 0.5 * std::f32::consts::SQRT_2;
        assert!(close(read_stick([86, 86], &dl).magnitude, length));
        assert!(close(read(86, 86).magnitude, (0.5 + length) / 2.0));
    }

    #[test]
    fn a_diagonal_counts_its_largest_axis_and_its_length_as_uya_traced() {
        // uyantsc-veldin-diag-{0.5,0.75}-{1,2}: two keys at part tilt, the bytes 191 or 64
        // and 223 or 32. At half tilt neither axis passes the cut-off alone, but together
        // they do, and he walked.
        assert!(!read(64, 0).is_active());
        for (x, y, traced) in [
            (64, 63, 0.249_550_73),
            (-63, 63, 0.238_244_76),
            (96, 95, 0.757_755_64),
            (-95, 95, 0.746_500_25),
        ] {
            let magnitude = read(x, y).magnitude;
            assert!((magnitude - traced).abs() < 1e-6, "({x}, {y}): {magnitude}");
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    /// Feeds stick counts through [`StickPresses`] and says which ticks pressed.
    fn presses(counts: &[[i8; 2]]) -> Vec<bool> {
        let profile = HeroProfile::uya().stick;
        let mut presses = StickPresses::default();
        counts
            .iter()
            .map(|&c| presses.update(read_axes(c, &profile), &profile.flick))
            .collect()
    }

    #[test]
    fn leaving_the_dead_zone_or_crossing_presses_a_direction() {
        // Out of the dead zone to the left, held, then across to the right.
        assert_eq!(
            presses(&[[0, 0], [-60, 0], [-60, 0], [60, 0], [60, 0]]),
            [false, true, false, true, false]
        );
        // Inside the dead zone nothing presses.
        assert_eq!(presses(&[[0, 0], [40, -40], [-40, 40]]), [false; 3]);
        // A second axis leaving its dead zone presses too.
        assert_eq!(presses(&[[60, 0], [60, 60]]), [true, true]);
    }

    #[test]
    fn a_flick_presses_while_it_is_swung_from_where_it_rested() {
        // From rest to full at once: the direction and the flick, then the flick alone
        // while one of the last four ticks rested more than 55° away (a centred stick's
        // angle is 0, to the right). Pushed right, there is no such swing.
        assert_eq!(
            presses(&[
                [0, 0],
                [-127, 0],
                [-127, 0],
                [-127, 0],
                [-127, 0],
                [-127, 0]
            ]),
            [false, true, true, true, true, false]
        );
        assert_eq!(
            presses(&[[0, 0], [127, 0], [127, 0], [127, 0]]),
            [false, true, false, false]
        );
        // Held up long enough to settle, then swung to full left: more than 55° from up
        // for four ticks.
        let mut counts = vec![[0, 127]; 5];
        counts.extend([[-127, 0]; 5]);
        assert_eq!(
            presses(&counts)[4..],
            [false, true, true, true, true, false]
        );
    }

    #[test]
    fn a_flick_is_from_near_the_centre_to_past_0_9_within_three_ticks() {
        let profile = HeroProfile::uya().stick;
        // Half way out (0.5) and then on to full right: no direction press after the
        // first, and the length rule sees under 0.25 three ticks back at most.
        let half = (48.0 + 0.5 * 76.0) as i8;
        let slow = |ticks_at_half: usize| {
            let mut presses = StickPresses::default();
            let mut counts = vec![[0, 0], [20, 0]];
            counts.extend(std::iter::repeat_n([half, 0], ticks_at_half));
            counts.push([127, 0]);
            counts
                .iter()
                .map(|&c| presses.update(read_axes(c, &profile), &profile.flick))
                .last()
                .unwrap()
        };
        assert!(slow(1));
        assert!(slow(2));
        assert!(!slow(3));
    }

    #[test]
    fn diagonals_count_as_full() {
        let stick = read(127, 127);
        assert_eq!(stick.magnitude, 1.0);
        let half = read(86, 0);
        assert_eq!(half.magnitude, 0.5);
    }
}
