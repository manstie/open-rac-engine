//! The quick-select ring (`weapons.md`, "Where the data lives" and "Switching and firing"):
//! two rings of eight slots that guns fill as they are given, and, while the quick-select
//! button is held past the quick switch, the ring on screen, whose slot the move stick
//! points at and which letting go of the button chooses.
//!
//! UYA pauses the game while its ring is open. The kit's doesn't, so that co-op peers stay
//! in step: the ring is part of the simulation, worked only by each tick's `PlayerInput`,
//! and the hero stands still meanwhile ([`rac_hero::GunHand::selecting`]) rather than walk
//! with the stick that picks the slot.
//!
//! How the stick picks a slot, which ring opens, and what the next and previous weapon
//! buttons do while it is open are the kit's own [placeholder] until the game's ring is
//! read in code.

use bevy::math::ops;
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

use super::def::WeaponId;

/// The quick-select rings, at 0x1D4C60 and 0x1D4C80 [code].
pub const RINGS: usize = 2;
/// Slots in each ring: eight item words [code].
pub const SLOTS: usize = 8;

/// How the ring is worked [placeholder].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RingRules {
    /// The move stick points at a slot once it is pushed this far from centre, in PS2
    /// stick counts (of 127): about half way, so a stick resting off centre picks nothing.
    pub dead_zone: u8,
}

impl Default for RingRules {
    fn default() -> Self {
        Self { dead_zone: 64 }
    }
}

/// The ring while it is on screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpenRing {
    /// The slot the stick last pointed at. It stays while the stick is back in the dead
    /// zone, so letting go of the stick and the button together still chooses it.
    pub highlighted: Option<usize>,
}

/// A hero's quick-select rings.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QuickSelect {
    /// Each ring's slots, slot 0 at the top and the rest clockwise.
    pub rings: [[Option<WeaponId>; SLOTS]; RINGS],
    /// The ring shown, kept between openings.
    pub ring: usize,
    /// Set while the ring is on screen.
    pub open: Option<OpenRing>,
}

impl QuickSelect {
    /// Puts `weapon` in the first free slot of the first ring, then of the second
    /// (`quick_select_add`, 0x416A10). One already in a ring isn't added again, nor is one
    /// with both rings full. Returns whether it was added.
    pub fn add(&mut self, weapon: WeaponId) -> bool {
        if self.contains(weapon) {
            return false;
        }
        let free = self.rings.iter_mut().flatten().find(|slot| slot.is_none());
        match free {
            Some(slot) => {
                *slot = Some(weapon);
                true
            }
            None => false,
        }
    }

    pub fn contains(&self, weapon: WeaponId) -> bool {
        self.rings
            .iter()
            .flatten()
            .any(|&slot| slot == Some(weapon))
    }

    /// No slot holds anything.
    pub fn is_empty(&self) -> bool {
        self.rings.iter().flatten().all(Option::is_none)
    }

    /// The gun in the highlighted slot of the ring shown, while it is open.
    pub fn highlighted(&self) -> Option<WeaponId> {
        let slot = self.open?.highlighted?;
        self.rings[self.ring][slot]
    }

    pub(crate) fn open(&mut self) {
        self.open = Some(OpenRing::default());
    }

    /// Highlights the slot the move stick points at, if it is out of the dead zone.
    pub(crate) fn point(&mut self, stick: [i8; 2], rules: &RingRules) {
        if let Some(open) = &mut self.open
            && let Some(slot) = slot_at(stick, rules.dead_zone)
        {
            open.highlighted = Some(slot);
        }
    }

    /// Shows the next ring, or the previous one.
    pub(crate) fn turn(&mut self, forward: bool) {
        self.ring = if forward {
            (self.ring + 1) % RINGS
        } else {
            (self.ring + RINGS - 1) % RINGS
        };
    }

    /// Closes the ring, returning the gun chosen: the highlighted slot's, if any.
    pub(crate) fn close(&mut self) -> Option<WeaponId> {
        let chosen = self.highlighted();
        self.open = None;
        chosen
    }
}

/// The slot the move stick points at, or `None` within `dead_zone` counts of centre.
/// Slot 0 is straight ahead (up on screen), then each is the next 45° clockwise.
pub fn slot_at(stick: [i8; 2], dead_zone: u8) -> Option<usize> {
    let [x, y] = stick.map(i32::from);
    // In whole counts, so every peer agrees exactly where the dead zone ends.
    if x * x + y * y < i32::from(dead_zone).pow(2) {
        return None;
    }
    // Clockwise from straight ahead.
    let angle = ops::atan2(x as f32, y as f32);
    let step = TAU / SLOTS as f32;
    let slot = ops::round(angle / step) as i32;
    Some(slot.rem_euclid(SLOTS as i32) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_of_the_eight_directions_points_at_its_slot() {
        let directions = [
            [0, 127],
            [127, 127],
            [127, 0],
            [127, -127],
            [0, -127],
            [-127, -127],
            [-127, 0],
            [-127, 127],
        ];
        for (slot, stick) in directions.into_iter().enumerate() {
            assert_eq!(slot_at(stick, 64), Some(slot), "{stick:?}");
        }
        // Just either side of the boundary between up and up-right (22.5°).
        assert_eq!(slot_at([40, 100], 64), Some(0));
        assert_eq!(slot_at([42, 100], 64), Some(1));
    }

    #[test]
    fn the_dead_zone_points_at_nothing() {
        assert_eq!(slot_at([0, 0], 64), None);
        assert_eq!(slot_at([45, 45], 64), None);
        assert_eq!(slot_at([0, 63], 64), None);
        assert_eq!(slot_at([0, 64], 64), Some(0));
        assert_eq!(slot_at([46, 46], 64), Some(1));
    }

    #[test]
    fn guns_fill_the_first_ring_then_the_second_and_no_more() {
        let mut quick = QuickSelect::default();
        assert!(quick.is_empty());
        for gun in 0..16 {
            assert!(quick.add(WeaponId(gun)));
        }
        assert!(!quick.add(WeaponId(16)));
        assert!(!quick.add(WeaponId(3)));
        assert_eq!(quick.rings[0][7], Some(WeaponId(7)));
        assert_eq!(quick.rings[1][0], Some(WeaponId(8)));
        // A slot freed in the first ring is filled first.
        quick.rings[0][2] = None;
        quick.rings[1][5] = None;
        assert!(quick.add(WeaponId(20)));
        assert_eq!(quick.rings[0][2], Some(WeaponId(20)));
        assert!(quick.add(WeaponId(21)));
        assert_eq!(quick.rings[1][5], Some(WeaponId(21)));
    }

    #[test]
    fn the_highlight_stays_when_the_stick_comes_back_and_closing_chooses_it() {
        let rules = RingRules::default();
        let mut quick = QuickSelect::default();
        quick.add(WeaponId(4));
        quick.add(WeaponId(5));
        quick.open();
        quick.point([0, 0], &rules);
        assert_eq!(quick.open.unwrap().highlighted, None);
        quick.point([127, 127], &rules);
        quick.point([0, 0], &rules);
        assert_eq!(quick.highlighted(), Some(WeaponId(5)));
        // The other ring's slot 1 is empty.
        quick.turn(true);
        assert_eq!(quick.highlighted(), None);
        quick.turn(false);
        assert_eq!(quick.close(), Some(WeaponId(5)));
        assert_eq!(quick.open, None);
    }
}
