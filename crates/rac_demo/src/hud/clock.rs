//! When the HUD's parts show, by UYA's timings (`weapons.md`, "The HUD"), and its bolt
//! count's roll: a [`HudClock`] per player, moved on each frame by what the hero's view
//! did ([`HudClock::tick`]). It counts seconds of the HUD's own frames, not the
//! simulation's ticks: the HUD only shows the game.
//! - The gun shows while one is in his hand, and a moment after it goes away.
//! - Health shows a while after a kill, longer than after damage, nanotech, more maximum
//!   health or the ring opening; low, it stays up and pulses.
//! - Bolts show a while after they change or the ring opens. The count trails the purse:
//!   changes gather in a tally beside it, which some time after the last change rolls in
//!   and then snaps to the purse.
//! - The gun's ammo pulses under a share of its maximum.

use serde::{Deserialize, Serialize};

/// UYA's HUD timings, in seconds (its frames at 60 Hz) [code].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HudTimings {
    /// The gun shows this long after it goes away (`hud_weapon_fill`: 10 frames).
    pub gun_after: f32,
    /// Health shows this long after a kill, and after damage, nanotech, more maximum
    /// health or the ring (`hud_health_show`: 720 and 360 frames).
    pub health_after_kill: f32,
    pub health_after_change: f32,
    /// Bolts show this long after they change or the ring opens (`hud_bolts_show`: 180).
    pub bolts_after: f32,
    /// Under this share of its maximum, health stays up and pulses, and a gun's ammo
    /// pulses (15%).
    pub low_share: f32,
    /// One pulse (40 frames).
    pub pulse: f32,
    /// The bolt tally fades in over `tally_fade`; `tally_wait` after the last change it
    /// rolls in over `tally_roll`, then the count snaps to the purse (`hud_bolts_roll`:
    /// 15, 61 and 15 frames).
    pub tally_fade: f32,
    pub tally_wait: f32,
    pub tally_roll: f32,
}

impl Default for HudTimings {
    fn default() -> Self {
        const FRAME: f32 = 1.0 / 60.0;
        Self {
            gun_after: 10.0 * FRAME,
            health_after_kill: 720.0 * FRAME,
            health_after_change: 360.0 * FRAME,
            bolts_after: 180.0 * FRAME,
            low_share: 0.15,
            pulse: 40.0 * FRAME,
            tally_fade: 15.0 * FRAME,
            tally_wait: 61.0 * FRAME,
            tally_roll: 15.0 * FRAME,
        }
    }
}

/// What the hero's view did since the last frame, for the clock.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    /// A gun (not the wrench) is in his hand.
    pub gun_out: bool,
    /// Health now and its most.
    pub health: Option<(u32, u32)>,
    pub bolts: Option<u32>,
    /// The quick-select ring is open.
    pub ring: bool,
    /// He killed something since the last frame.
    pub killed: bool,
}

/// The bolt count as shown.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct BoltCount {
    /// The count shown.
    pub shown: u32,
    /// The changes not yet rolled in, and how far it has faded in, 0 to 1.
    pub tally: i64,
    pub tally_alpha: f32,
}

/// One player's HUD timers.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct HudClock {
    /// Seconds each part still shows for.
    gun: f32,
    health: f32,
    bolts: f32,
    /// The pulse's phase, 0 to 1 over a pulse.
    pulse: f32,
    last: Seen,
    started: bool,
    count: BoltCount,
    /// Seconds since the bolts last changed, and the roll under way: the count it started
    /// from and how far it has gone, 0 to 1.
    since_change: f32,
    roll: Option<(u32, f32)>,
}

/// What shows this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Showing {
    pub gun: bool,
    pub health: bool,
    pub bolts: bool,
    pub count: BoltCount,
    /// Health is low: it pulses, by this much now, 0 to 1.
    pub health_pulse: Option<f32>,
    /// How much a low ammo count pulses now, 0 to 1.
    pub pulse: f32,
}

impl HudClock {
    /// Moves on by `seconds` with what the view shows now.
    pub fn tick(&mut self, seen: Seen, seconds: f32, timings: &HudTimings) -> Showing {
        let tick_down = |left: &mut f32| *left = (*left - seconds).max(0.0);
        tick_down(&mut self.gun);
        tick_down(&mut self.health);
        tick_down(&mut self.bolts);
        if seen.gun_out {
            self.gun = timings.gun_after;
        }
        let opened = seen.ring && !self.last.ring;
        if seen.killed {
            self.health = self.health.max(timings.health_after_kill);
        }
        let health_changed = self.started && seen.health != self.last.health;
        if health_changed || opened {
            self.health = self.health.max(timings.health_after_change);
        }
        if opened {
            self.bolts = self.bolts.max(timings.bolts_after);
        }
        self.pulse = (self.pulse + seconds / timings.pulse.max(f32::EPSILON)).fract();
        self.roll_bolts(seen.bolts, seconds, timings);

        let low = seen
            .health
            .is_some_and(|(now, most)| (now as f32) < most as f32 * timings.low_share);
        let pulse = 1.0 - (2.0 * self.pulse - 1.0).abs();
        self.last = seen;
        self.started = true;
        Showing {
            gun: seen.gun_out || self.gun > 0.0,
            health: seen.health.is_some() && (low || self.health > 0.0),
            bolts: seen.bolts.is_some() && self.bolts > 0.0,
            count: self.count,
            health_pulse: low.then_some(pulse),
            pulse,
        }
    }

    /// The count trails the purse: changes gather in the tally, which rolls in once the
    /// purse has kept still a while.
    fn roll_bolts(&mut self, bolts: Option<u32>, seconds: f32, timings: &HudTimings) {
        let Some(purse) = bolts else {
            return;
        };
        if !self.started {
            self.count = BoltCount {
                shown: purse,
                ..Default::default()
            };
            return;
        }
        if Some(purse) != self.last.bolts {
            self.bolts = timings.bolts_after;
            self.since_change = 0.0;
            // A change during the roll starts the tally again from what is shown.
            self.roll = None;
        } else {
            self.since_change += seconds;
        }
        self.count.tally = i64::from(purse) - i64::from(self.count.shown);
        if self.count.tally == 0 {
            self.count.tally_alpha = 0.0;
            return;
        }
        self.count.tally_alpha =
            (self.count.tally_alpha + seconds / timings.tally_fade.max(f32::EPSILON)).min(1.0);
        if self.roll.is_none() && self.since_change >= timings.tally_wait {
            self.roll = Some((self.count.shown, 0.0));
        }
        if let Some((from, done)) = &mut self.roll {
            *done += seconds / timings.tally_roll.max(f32::EPSILON);
            if *done >= 1.0 {
                self.count = BoltCount {
                    shown: purse,
                    ..Default::default()
                };
                self.roll = None;
            } else {
                // A 14th of the tally a frame, over 15 frames: the last is the snap.
                let share = (*done * 15.0 / 14.0).min(1.0);
                let rolled = (i64::from(purse) - i64::from(*from)) as f32 * share;
                let shown = i64::from(*from) + rolled as i64;
                self.count.shown = shown.clamp(0, i64::from(u32::MAX)) as u32;
                self.count.tally = i64::from(purse) - i64::from(self.count.shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;

    fn run(clock: &mut HudClock, seen: Seen, frames: u32) -> Showing {
        let timings = HudTimings::default();
        let mut showing = clock.tick(seen, FRAME, &timings);
        for _ in 1..frames {
            showing = clock.tick(seen, FRAME, &timings);
        }
        showing
    }

    fn seen(health: u32, bolts: u32) -> Seen {
        Seen {
            health: Some((health, 10)),
            bolts: Some(bolts),
            ..Default::default()
        }
    }

    #[test]
    fn the_gun_shows_while_out_and_10_frames_after() {
        let mut clock = HudClock::default();
        let out = Seen {
            gun_out: true,
            ..seen(10, 0)
        };
        assert!(run(&mut clock, out, 30).gun);
        assert!(run(&mut clock, seen(10, 0), 9).gun);
        assert!(!run(&mut clock, seen(10, 0), 2).gun);
    }

    #[test]
    fn health_shows_after_damage_for_360_frames_and_after_a_kill_for_720() {
        let mut clock = HudClock::default();
        assert!(!run(&mut clock, seen(10, 0), 5).health, "nothing happened");
        assert!(run(&mut clock, seen(8, 0), 1).health);
        assert!(run(&mut clock, seen(8, 0), 358).health);
        assert!(!run(&mut clock, seen(8, 0), 3).health);
        let kill = Seen {
            killed: true,
            ..seen(8, 0)
        };
        run(&mut clock, kill, 1);
        assert!(run(&mut clock, seen(8, 0), 718).health);
        assert!(!run(&mut clock, seen(8, 0), 3).health);
    }

    #[test]
    fn low_health_stays_up_and_pulses_every_40_frames() {
        let mut clock = HudClock::default();
        let low = seen(1, 0);
        let showing = run(&mut clock, low, 1000);
        assert!(showing.health);
        let pulse = showing.health_pulse.unwrap();
        let later = run(&mut clock, low, 20).health_pulse.unwrap();
        assert!((pulse - later).abs() > 0.9, "{pulse} then {later}");
        let round = run(&mut clock, low, 20).health_pulse.unwrap();
        assert!((pulse - round).abs() < 1e-3, "{pulse} then {round}");
    }

    #[test]
    fn the_ring_opening_shows_health_and_bolts() {
        let mut clock = HudClock::default();
        run(&mut clock, seen(10, 50), 5);
        let ring = Seen {
            ring: true,
            ..seen(10, 50)
        };
        let showing = run(&mut clock, ring, 1);
        assert!(showing.health && showing.bolts);
        // Held open, they don't keep showing past their time.
        let showing = run(&mut clock, ring, 362);
        assert!(!showing.health && !showing.bolts);
    }

    #[test]
    fn the_bolt_count_trails_and_rolls_in_61_frames_after_the_last_change() {
        let mut clock = HudClock::default();
        assert_eq!(run(&mut clock, seen(10, 100), 1).count.shown, 100);
        let showing = run(&mut clock, seen(10, 120), 1);
        assert!(showing.bolts);
        assert_eq!((showing.count.shown, showing.count.tally), (100, 20));
        // Another change 30 frames on starts the wait again.
        run(&mut clock, seen(10, 120), 30);
        let showing = run(&mut clock, seen(10, 148), 60);
        assert_eq!((showing.count.shown, showing.count.tally), (100, 48));
        assert_eq!(showing.count.tally_alpha, 1.0);
        // Then it rolls in over 15 frames and snaps.
        let showing = run(&mut clock, seen(10, 148), 8);
        assert!(showing.count.shown > 100 && showing.count.shown < 148);
        let showing = run(&mut clock, seen(10, 148), 8);
        assert_eq!((showing.count.shown, showing.count.tally), (148, 0));
        // Spending counts down the same way.
        let showing = run(&mut clock, seen(10, 140), 1);
        assert_eq!(showing.count.tally, -8);
        assert!(run(&mut clock, seen(10, 140), 178).bolts);
        assert!(!run(&mut clock, seen(10, 140), 3).bolts);
        assert_eq!(clock.count.shown, 140);
    }
}
