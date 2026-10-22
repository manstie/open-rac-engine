//! The quick-select ring (`weapons.md`, "The quick-select ring"): two rings of eight slots
//! that guns fill as they are given, and the ring on screen while the quick-select button
//! is held, worked by UYA's rules (`quick_select_ring_update`, 0x41AB20).
//!
//! UYA can pause the game while its ring is open, an option (QUICK SELECT PAUSE). The kit
//! plays it with the option off, so that co-op peers stay in step: the ring opens over the
//! world as the button is pressed and the world runs on, and some ticks later the hero
//! takes no more gameplay input until it has closed ([`rac_hero::GunHand::selecting`]).
//! The ring is part of the simulation, worked only by each tick's `PlayerInput`.
//! - Picking starts once the button has been held through the hold check. Nothing is
//!   picked when the ring opens, and a stick already pushed is ignored until it drops
//!   under half way or the lock-out runs out, which picks nothing again.
//! - The move stick past 0.9 of full picks the slot it points at, keeps the one picked
//!   while it points within 33.75° of its middle, and leans off an empty slot onto a
//!   filled neighbour. Between half way and 0.9 nothing changes.
//! - With the stick short of 0.9, the d-pad steps between slots. The kit's d-pad left and
//!   right are its weapon buttons; up and down have no button in its input [simplified].
//! - Crouch (the pad's R1) held shows the second ring, and for a while after.
//! - Letting go closes the ring over as many ticks as it had been opening, up to 8, and
//!   then chooses the slot picked, if it holds a gun.

use bevy::math::ops;
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

use super::def::WeaponId;

/// The quick-select rings, at 0x1D4C60 and 0x1D4C80 [code].
pub const RINGS: usize = 2;
/// Slots in each ring: eight item words [code].
pub const SLOTS: usize = 8;

/// The slot each d-pad direction steps to, by slot: left, right, up and down
/// (`quick_select_slots` +0x08 to +0x14, read in Veldin's image [data]).
const NEIGHBOURS: [[usize; 4]; SLOTS] = [
    [7, 1, 0, 4],
    [0, 2, 0, 2],
    [6, 2, 1, 3],
    [4, 2, 2, 4],
    [5, 3, 0, 4],
    [6, 4, 6, 4],
    [6, 2, 7, 5],
    [6, 0, 0, 6],
];
/// Where the d-pad steps with nothing picked: left, right, up and down [code].
const FIRST_STEP: [usize; 4] = [6, 2, 0, 4];

/// How the ring is worked, UYA's by default (`weapons.md`, "The quick-select ring").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RingRules {
    /// Ticks from the button's press until the hero takes no more input while the ring
    /// is up, and the same just after a tap that switched no gun (`quick_select_input_delay`:
    /// 15 and 27) [code].
    pub input_delay_ticks: u32,
    pub input_delay_after_tap_ticks: u32,
    /// Ticks the button must be held from its press before the stick picks: held 2, then
    /// the hold check's countdown of 4 (`quick_select_hold_update`) [code].
    pub hold_ticks: u32,
    /// Ticks a stick pushed as the ring opens is ignored, unless it drops under
    /// `release` first (`quick_select_ring_init`) [code].
    pub lock_out_ticks: u8,
    /// Shares of the stick's full tilt: past `pick` it picks, under `release` it comes
    /// out of the lock-out [code].
    pub pick: f32,
    pub release: f32,
    /// The slot picked stays while the stick points within this of its middle [code].
    pub keep_degrees: f32,
    /// The share of a straight up, down, left or right slot's 45° the stick picks it in;
    /// the diagonals take the rest (27° and 63°) [code].
    pub cardinal_share: f32,
    /// Ticks the ring takes to open fully, and to close from fully open before it
    /// chooses (`open`, 0 to 8) [code].
    pub open_ticks: u8,
    /// Ticks the second ring shows, held at this while its button is held
    /// (`quick_select_ring2_timer`) [code].
    pub ring_two_ticks: u8,
}

impl Default for RingRules {
    fn default() -> Self {
        Self {
            input_delay_ticks: 15,
            input_delay_after_tap_ticks: 27,
            hold_ticks: 6,
            lock_out_ticks: 30,
            pick: 0.9,
            release: 0.5,
            keep_degrees: 33.75,
            cardinal_share: 0.6,
            open_ticks: 8,
            ring_two_ticks: 15,
        }
    }
}

/// The ring while it is on screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpenRing {
    /// The slot picked, which letting go chooses if it holds a gun.
    pub highlighted: Option<usize>,
    /// Ticks the stick is still locked out for, from the opening; `None` once it isn't.
    pub lock_out: Option<u8>,
    /// How far open it is on screen, 0 to [`RingRules::open_ticks`]: up a tick while the
    /// button is held, down a tick once it is let go.
    pub shown: u8,
    /// The button was let go: it closes, then chooses.
    pub closing: bool,
    /// Ticks until the hero takes no more input; 0 once he doesn't.
    pub input_delay: u32,
}

/// A hero's quick-select rings.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QuickSelect {
    /// Each ring's slots, slot 0 at the top and the rest clockwise.
    pub rings: [[Option<WeaponId>; SLOTS]; RINGS],
    /// The ring shown.
    pub ring: usize,
    /// Set while the ring is on screen.
    pub open: Option<OpenRing>,
    /// Ticks the second ring still shows.
    pub(crate) ring_two: u8,
}

/// What the ring reads from a tick's input.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RingInput {
    /// The quick-select button is held, and has been since its press this many ticks ago.
    pub held: Option<u32>,
    pub move_stick: [i8; 2],
    /// The second ring's button is held.
    pub ring_two: bool,
    /// The d-pad left or right was pressed.
    pub left: bool,
    pub right: bool,
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

    /// The ring is up and the hero takes no gameplay input meanwhile.
    pub fn blocks_input(&self) -> bool {
        self.open.is_some_and(|open| open.input_delay == 0)
    }

    /// Opens the ring on the first ring, nothing picked and the stick locked out; the hero
    /// takes no input once `input_delay` ticks have passed.
    pub(crate) fn open(&mut self, input_delay: u32, rules: &RingRules) {
        self.ring = 0;
        self.ring_two = 0;
        self.open = Some(OpenRing {
            highlighted: None,
            lock_out: Some(rules.lock_out_ticks),
            shown: 0,
            closing: false,
            input_delay,
        });
    }

    /// One tick of the open ring. Returns the gun chosen when it has closed.
    pub(crate) fn update(&mut self, input: &RingInput, rules: &RingRules) -> Option<WeaponId> {
        let open = self.open.as_mut()?;
        open.input_delay = open.input_delay.saturating_sub(1);
        let held = input.held.filter(|_| !open.closing);
        if held.is_some() {
            if input.ring_two {
                self.ring_two = rules.ring_two_ticks;
            }
            self.ring = usize::from(self.ring_two > 0);
        }
        self.ring_two = self.ring_two.saturating_sub(1);

        let [x, y] = input.move_stick.map(|count| f32::from(count) / 127.0);
        let tilt = ops::sqrt(x * x + y * y);
        if let Some(ticks) = open.lock_out {
            if input.left || input.right || tilt < rules.release || ticks == 0 {
                open.lock_out = None;
                open.highlighted = None;
            } else {
                open.lock_out = Some(ticks - 1);
            }
        }

        let slots = &self.rings[self.ring];
        if open.lock_out.is_none() && held.is_some_and(|ticks| ticks >= rules.hold_ticks) {
            let mut lean = 0;
            if tilt > rules.pick {
                // Clockwise from straight up.
                let angle = ops::atan2(x, y);
                let kept = open
                    .highlighted
                    .is_some_and(|slot| off(angle, slot).abs() <= rules.keep_degrees.to_radians());
                if !kept {
                    let slot = slot_at(angle, rules.cardinal_share);
                    open.highlighted = Some(slot);
                    lean = if off(angle, slot) > 0.0 { 1 } else { -1 };
                }
            } else if input.left || input.right {
                let direction = usize::from(input.right);
                open.highlighted = Some(match open.highlighted {
                    Some(slot) => NEIGHBOURS[slot][direction],
                    None => FIRST_STEP[direction],
                });
            }
            // An empty slot gives way to the filled neighbour the stick leans to.
            if let Some(slot) = open.highlighted
                && slots[slot].is_none()
                && lean != 0
            {
                let neighbour = (slot as i32 + lean).rem_euclid(SLOTS as i32) as usize;
                if slots[neighbour].is_some() {
                    open.highlighted = Some(neighbour);
                }
            }
        }

        if held.is_some() {
            open.shown = (open.shown + 1).min(rules.open_ticks);
            return None;
        }
        open.closing = true;
        open.shown = open.shown.saturating_sub(1);
        if open.shown > 0 {
            return None;
        }
        let chosen = self.highlighted();
        self.open = None;
        chosen
    }
}

/// How far `angle` (clockwise from straight up) is round from `slot`'s middle, from −π to
/// π: positive clockwise of it.
fn off(angle: f32, slot: usize) -> f32 {
    let middle = slot as f32 * TAU / SLOTS as f32;
    (angle - middle + PI).rem_euclid(TAU) - PI
}

/// The slot `angle` (clockwise from straight up) picks: the nearest, but a straight up,
/// down, left or right one only in the middle `cardinal_share` of its 45°, the diagonals
/// taking the rest.
pub fn slot_at(angle: f32, cardinal_share: f32) -> usize {
    let step = TAU / SLOTS as f32;
    let at = (angle + step / 2.0).rem_euclid(TAU) / step;
    let slot = (at as usize).min(SLOTS - 1);
    let within = at - slot as f32;
    let edge = (1.0 - cardinal_share) / 2.0;
    let slot = if slot % 2 == 1 {
        slot
    } else if within > 1.0 - edge {
        slot + 1
    } else if within < edge {
        slot + SLOTS - 1
    } else {
        slot
    };
    slot % SLOTS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clockwise from straight up, in degrees.
    fn at(degrees: f32) -> usize {
        slot_at(degrees.to_radians(), RingRules::default().cardinal_share)
    }

    #[test]
    fn straight_slots_are_27_degrees_wide_and_diagonals_63() {
        for slot in 0..SLOTS {
            assert_eq!(at(slot as f32 * 45.0), slot, "slot {slot}'s middle");
        }
        // Up is picked within 13.5° either side; past that the diagonals take over.
        assert_eq!(at(13.0), 0);
        assert_eq!(at(14.0), 1);
        assert_eq!(at(-13.0), 0);
        assert_eq!(at(-14.0), 7);
        assert_eq!(at(359.0), 0);
        // Up-right reaches from 13.5° to 76.5°.
        assert_eq!(at(76.0), 1);
        assert_eq!(at(77.0), 2);
        assert_eq!(at(180.0 + 14.0), 5);
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

    /// A ring with guns in slots 0, 2, 3 and 4, open with the button held `ticks` since
    /// its press.
    fn ring() -> (QuickSelect, RingRules) {
        let rules = RingRules::default();
        let mut quick = QuickSelect::default();
        quick.rings[0] = [0, 9, 2, 3, 4, 9, 9, 9].map(|w| (w != 9).then_some(WeaponId(w)));
        quick.rings[1][0] = Some(WeaponId(10));
        quick.open(rules.input_delay_ticks, &rules);
        (quick, rules)
    }

    fn held(ticks: u32, move_stick: [i8; 2]) -> RingInput {
        RingInput {
            held: Some(ticks),
            move_stick,
            ..Default::default()
        }
    }

    #[test]
    fn the_stick_picks_only_after_the_hold_check_and_past_point_nine() {
        let (mut quick, rules) = ring();
        // Centred, the lock-out ends at once.
        quick.update(&held(1, [0, 0]), &rules);
        assert_eq!(quick.open.unwrap().lock_out, None);
        quick.update(&held(5, [127, 0]), &rules);
        assert_eq!(quick.highlighted(), None, "before the hold check");
        quick.update(&held(6, [110, 0]), &rules);
        assert_eq!(quick.highlighted(), None, "short of 0.9");
        quick.update(&held(7, [127, 0]), &rules);
        assert_eq!(quick.highlighted(), Some(WeaponId(2)));
        // Back between half way and 0.9, nothing changes.
        quick.update(&held(8, [0, -100]), &rules);
        assert_eq!(quick.highlighted(), Some(WeaponId(2)));
    }

    #[test]
    fn a_stick_pushed_as_it_opens_is_locked_out_until_it_drops_or_30_ticks_pass() {
        let (mut quick, rules) = ring();
        for tick in 1..=30 {
            quick.update(&held(tick, [127, 0]), &rules);
            assert_eq!(quick.highlighted(), None, "tick {tick}");
        }
        quick.update(&held(31, [127, 0]), &rules);
        assert_eq!(quick.open.unwrap().lock_out, None);
        quick.update(&held(32, [127, 0]), &rules);
        assert_eq!(quick.highlighted(), Some(WeaponId(2)));
        // Dropping under half way ends it early.
        let (mut quick, rules) = ring();
        quick.update(&held(1, [127, 0]), &rules);
        quick.update(&held(2, [60, 0]), &rules);
        assert_eq!(quick.open.unwrap().lock_out, None);
    }

    #[test]
    fn the_slot_picked_is_kept_within_33_75_degrees_and_empty_ones_lean_to_a_neighbour() {
        let (mut quick, rules) = ring();
        quick.update(&held(1, [0, 0]), &rules);
        let point = |degrees: f32| {
            let r = degrees.to_radians();
            [(ops::sin(r) * 127.0) as i8, (ops::cos(r) * 127.0) as i8]
        };
        quick.update(&held(6, point(90.0)), &rules);
        assert_eq!(quick.open.unwrap().highlighted, Some(2));
        // 30° round towards slot 3 still keeps slot 2; 40° doesn't.
        quick.update(&held(7, point(120.0)), &rules);
        assert_eq!(quick.open.unwrap().highlighted, Some(2));
        quick.update(&held(8, point(130.0)), &rules);
        assert_eq!(quick.open.unwrap().highlighted, Some(3));
        // Slot 1 is empty: leaning back towards 0 takes 0, leaning on towards 2 takes 2.
        quick.update(&held(9, point(40.0)), &rules);
        assert_eq!(quick.open.unwrap().highlighted, Some(0));
        quick.update(&held(10, point(90.0)), &rules);
        quick.update(&held(11, point(50.0)), &rules);
        assert_eq!(quick.open.unwrap().highlighted, Some(2));
        // Slot 6 is empty and so are both its neighbours: it stays, choosing nothing.
        quick.update(&held(12, point(270.0)), &rules);
        assert_eq!(quick.open.unwrap().highlighted, Some(6));
        assert_eq!(quick.highlighted(), None);
    }

    #[test]
    fn the_d_pad_steps_by_the_neighbour_table() {
        let (mut quick, rules) = ring();
        let step = |quick: &mut QuickSelect, right: bool, tick: u32| {
            let input = RingInput {
                held: Some(tick),
                left: !right,
                right,
                ..Default::default()
            };
            quick.update(&input, &rules);
            quick.open.unwrap().highlighted
        };
        // The first press also ends the lock-out, which comes before the hold check.
        assert_eq!(step(&mut quick, true, 6), Some(2));
        assert_eq!(step(&mut quick, true, 7), Some(2));
        assert_eq!(step(&mut quick, false, 8), Some(6));
        assert_eq!(step(&mut quick, false, 9), Some(6));
        assert_eq!(step(&mut quick, true, 10), Some(2));
    }

    #[test]
    fn letting_go_closes_over_its_opening_then_chooses() {
        let (mut quick, rules) = ring();
        for tick in 1..=20 {
            assert_eq!(quick.update(&held(tick, [127, 0]), &rules), None);
        }
        // Lock-out still on (pushed since it opened): nothing yet.
        assert_eq!(quick.open.unwrap().shown, 8);
        for tick in 21..=40 {
            quick.update(&held(tick, [127, 0]), &rules);
        }
        assert_eq!(quick.highlighted(), Some(WeaponId(2)));
        let released = RingInput::default();
        for _ in 0..7 {
            assert_eq!(quick.update(&released, &rules), None);
            assert!(quick.open.unwrap().closing);
        }
        assert_eq!(quick.update(&released, &rules), Some(WeaponId(2)));
        assert_eq!(quick.open, None);
        // Opened a few ticks only, it closes in as many.
        let (mut quick, rules) = ring();
        quick.update(&held(1, [0, 0]), &rules);
        quick.update(&held(2, [0, 0]), &rules);
        assert_eq!(quick.update(&released, &rules), None);
        assert_eq!(quick.update(&released, &rules), None);
        assert_eq!(quick.open, None);
    }

    #[test]
    fn the_second_ring_shows_while_its_button_is_held_and_14_ticks_after() {
        let (mut quick, rules) = ring();
        let mut input = held(1, [0, 0]);
        input.ring_two = true;
        quick.update(&input, &rules);
        assert_eq!(quick.ring, 1);
        for tick in 2..=15 {
            quick.update(&held(tick, [0, 0]), &rules);
            assert_eq!(quick.ring, 1, "tick {tick}");
        }
        quick.update(&held(16, [0, 0]), &rules);
        assert_eq!(quick.ring, 0);
    }

    #[test]
    fn the_hero_takes_no_input_once_the_delay_runs_out() {
        let (mut quick, rules) = ring();
        for tick in 1..rules.input_delay_ticks {
            quick.update(&held(tick, [0, 0]), &rules);
            assert!(!quick.blocks_input(), "tick {tick}");
        }
        quick.update(&held(rules.input_delay_ticks, [0, 0]), &rules);
        assert!(quick.blocks_input());
    }
}
