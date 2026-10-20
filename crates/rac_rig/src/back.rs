//! What the parts on the hero's back play (P1.17): UYA's rules for Clank and the heli-pack,
//! read in its code (`moby-format.md`, "Clank and the heli-pack on the hero's back"), over
//! a game's [`BackAnimations`].
//! - When the hero's move changes, a move with a row starts its sequences, unless the lead
//!   part plays them already; any other sends every part back to rest.
//! - While a move with a hold lasts (the glide), its part holds that sequence.
//! - Standing idle, a part that has played through goes back to rest, and now and then all
//!   of them play one of the idle's fidgets.
//! - A change blends from where the part is to the new sequence's first pose, which then
//!   plays on; every sequence loops, as a moby's does.

use crate::clips::{BackAnimations, RigMove, Sample, mix};

/// Ticks to blend back to rest (UYA `hero_back_follow_anim`), and after a hold, such as
/// the glide's.
const RETURN_TICKS: f32 = 7.0;
const RETURN_AFTER_HOLD_TICKS: f32 = 19.0;
/// The idle's fidgets: how long the hero must have stood, the ticks between one and the
/// next (UYA's `clank_fidget_timer`, counted only while every part rests), and the blend.
const FIDGET_AFTER_TICKS: u32 = 10;
const FIDGET_EVERY_TICKS: (u32, u32) = (110, 270);
const FIDGET_BLEND_TICKS: f32 = 7.0;

/// Where one part is.
#[derive(Debug, Clone, PartialEq)]
struct Part {
    sequence: u8,
    /// Ticks into its clip.
    ticks: f32,
    /// Blending in from this pose over `length` ticks, `done` of them gone.
    blend: Option<Blend>,
}

#[derive(Debug, Clone, PartialEq)]
struct Blend {
    from: Sample,
    length: f32,
    done: f32,
}

/// What the rig remembers to choose and time the back's clips.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BackDirector {
    parts: Vec<Part>,
    previous: Option<RigMove>,
    /// Ticks the hero has stood idle, and to the next fidget.
    idle_ticks: u32,
    fidget_timer: u32,
    /// Fractions of a tick not yet counted on those clocks.
    pending: f32,
    rng: u32,
}

impl BackDirector {
    /// `seed` varies the fidgets between heroes.
    pub fn new(seed: u32) -> Self {
        Self {
            parts: Vec::new(),
            previous: None,
            idle_ticks: 0,
            fidget_timer: 0,
            pending: 0.0,
            rng: seed | 1,
        }
    }

    /// Moves on by `seconds` while the hero's clip for `current` plays at `rate`, and
    /// returns each part's pose, or none for one with no clip to play.
    pub fn step(
        &mut self,
        back: &BackAnimations,
        current: Option<RigMove>,
        rate: f32,
        seconds: f32,
    ) -> Vec<Option<Sample>> {
        if self.parts.len() != back.parts.len() {
            self.parts = back
                .rest
                .iter()
                .map(|&sequence| Part {
                    sequence,
                    ticks: 0.0,
                    blend: None,
                })
                .collect();
        }
        let elapsed = seconds * 60.0;
        self.pending += elapsed;
        let whole = self.pending.floor();
        self.pending -= whole;
        let whole = whole as u32;

        if current != self.previous {
            self.follow(back, current);
            if !is_idle(self.previous) {
                self.idle_ticks = 0;
            }
            self.previous = current;
        }
        for hold in &back.holds {
            if Some(hold.movement) == current
                && self
                    .parts
                    .get(hold.part)
                    .is_some_and(|p| p.sequence != hold.sequence)
            {
                self.start(back, hold.part, hold.sequence, hold.blend);
            }
        }
        if is_idle(current) {
            self.idle(back, whole);
        }

        // While the hero's move has rows, the back plays at his rate; a blend sets a
        // part's own rate back to 1, which the rest keep.
        let speed = if self.has_rows(back, current) {
            rate
        } else {
            1.0
        };
        for part in &mut self.parts {
            match &mut part.blend {
                Some(blend) => {
                    blend.done += elapsed;
                    if blend.done >= blend.length {
                        part.ticks = blend.done - blend.length;
                        part.blend = None;
                    }
                }
                None => part.ticks += elapsed * speed,
            }
        }
        (0..self.parts.len()).map(|i| self.pose(back, i)).collect()
    }

    /// The hero's move changed to `current`.
    fn follow(&mut self, back: &BackAnimations, current: Option<RigMove>) {
        if let Some(row) = back.follow.iter().find(|r| Some(r.movement) == current) {
            let lead = row.sequences.first().copied();
            if self.parts.first().map(|p| p.sequence) != lead {
                for (part, &sequence) in row.sequences.iter().enumerate() {
                    self.start(back, part, sequence, row.blend);
                }
            }
            return;
        }
        // Back in the idle, a fidget the lead part plays runs on.
        let lead = self.parts.first().map(|p| p.sequence);
        if current == Some(RigMove::Idle) && back.fidgets.iter().any(|f| f.first() == lead.as_ref())
        {
            return;
        }
        let held = back.holds.iter().any(|h| Some(h.movement) == self.previous);
        let blend = if held {
            RETURN_AFTER_HOLD_TICKS
        } else {
            RETURN_TICKS
        };
        self.rest(back, blend);
    }

    /// Standing idle for `ticks` more.
    fn idle(&mut self, back: &BackAnimations, ticks: u32) {
        for _ in 0..ticks {
            for part in 0..self.parts.len() {
                let rest = back.rest.get(part).copied();
                let played = back
                    .clips
                    .get(part)
                    .and_then(|c| c.get(&self.parts[part].sequence));
                let through = played.is_some_and(|clip| {
                    self.parts[part].blend.is_none() && clip.finished(self.parts[part].ticks)
                });
                if through && Some(self.parts[part].sequence) != rest {
                    self.start(back, part, rest.unwrap_or_default(), RETURN_TICKS);
                }
            }
            self.idle_ticks = self.idle_ticks.saturating_add(1);
            let resting = self
                .parts
                .iter()
                .zip(&back.rest)
                .all(|(part, &rest)| part.sequence == rest);
            if self.idle_ticks <= FIDGET_AFTER_TICKS || !resting || back.fidgets.is_empty() {
                continue;
            }
            // A timer that has run out counts as running out again, as UYA's does.
            if self.fidget_timer > 0 {
                self.fidget_timer -= 1;
                if self.fidget_timer > 0 {
                    continue;
                }
            }
            let (low, high) = FIDGET_EVERY_TICKS;
            self.fidget_timer = low + (self.random() * (high - low + 1) as f32) as u32;
            let pick = (self.random() * back.fidgets.len() as f32) as usize;
            let fidget = back.fidgets[pick.min(back.fidgets.len() - 1)].clone();
            for (part, sequence) in fidget.into_iter().enumerate() {
                self.start(back, part, sequence, FIDGET_BLEND_TICKS);
            }
        }
    }

    fn rest(&mut self, back: &BackAnimations, blend: f32) {
        for (part, &rest) in back.rest.iter().enumerate() {
            if self.parts.get(part).is_some_and(|p| p.sequence != rest) {
                self.start(back, part, rest, blend);
            }
        }
    }

    /// Starts `part` playing `sequence`, blending from where it is over `blend` ticks.
    fn start(&mut self, back: &BackAnimations, part: usize, sequence: u8, blend: f32) {
        let from = self.pose(back, part);
        let Some(state) = self.parts.get_mut(part) else {
            return;
        };
        state.sequence = sequence;
        state.ticks = 0.0;
        state.blend = from.filter(|_| blend > 0.0).map(|from| Blend {
            from,
            length: blend,
            done: 0.0,
        });
    }

    /// Where `part` is now.
    fn pose(&self, back: &BackAnimations, part: usize) -> Option<Sample> {
        let state = self.parts.get(part)?;
        let clip = back.clips.get(part)?.get(&state.sequence)?;
        let pose = clip.at_time(state.ticks)?;
        Some(match &state.blend {
            Some(blend) => mix(&blend.from, &pose, (blend.done / blend.length).min(1.0)),
            None => pose,
        })
    }

    /// Whether the hero's move has rows in the game's table: its own, or the idle's
    /// fidgets.
    fn has_rows(&self, back: &BackAnimations, current: Option<RigMove>) -> bool {
        back.follow.iter().any(|r| Some(r.movement) == current)
            || (current == Some(RigMove::Idle) && !back.fidgets.is_empty())
    }

    /// A number from 0 to 1 (xorshift32), as the hero's fidgets draw theirs.
    fn random(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// The sequence each part plays.
    #[cfg(test)]
    fn sequences(&self) -> Vec<u8> {
        self.parts.iter().map(|p| p.sequence).collect()
    }
}

/// Whether the hero stands in UYA's idle state: the idle, the hurt idle and his fidgets.
fn is_idle(movement: Option<RigMove>) -> bool {
    matches!(
        movement,
        Some(RigMove::Idle | RigMove::IdleHurt | RigMove::Fidget(_))
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::clips::{BackHold, BackRow, RigClip, RigKeyframe};
    use bevy::math::{Quat, Vec3};

    /// A looping clip of one joint, `count` keyframes 10 ticks apart, its translation's x
    /// counting up from `sequence` × 100.
    fn clip(sequence: u8, count: usize) -> RigClip {
        RigClip {
            keyframes: (0..count)
                .map(|i| RigKeyframe {
                    ticks: 10.0,
                    frame: i as f32,
                    rotations: vec![Quat::IDENTITY],
                    offsets: vec![Vec3::X * (sequence as f32 * 100.0 + i as f32)],
                    scales: vec![Vec3::ONE],
                })
                .collect(),
            looping: true,
            start: 0,
        }
    }

    /// UYA's heli-pack table, for a pack and Clank of one joint each.
    fn back() -> BackAnimations {
        let clips = |sequences: &[(u8, usize)]| -> BTreeMap<u8, RigClip> {
            sequences.iter().map(|&(s, n)| (s, clip(s, n))).collect()
        };
        BackAnimations {
            parts: vec!["heli-pack".into(), "clank".into()],
            clips: vec![
                clips(&[(1, 6), (3, 14), (4, 26), (5, 34), (6, 16), (7, 33), (8, 16)]),
                clips(&[(1, 6), (9, 4), (10, 13), (11, 15), (13, 33), (14, 9)]),
            ],
            rest: vec![1, 1],
            follow: vec![
                BackRow {
                    movement: RigMove::HeliJump,
                    sequences: vec![5, 11],
                    blend: 5.0,
                },
                // The landing plays the heli jump's own sequence on.
                BackRow {
                    movement: RigMove::Land(crate::Landing::HeliJump),
                    sequences: vec![5, 11],
                    blend: 7.0,
                },
                BackRow {
                    movement: RigMove::LongJump,
                    sequences: vec![8, 14],
                    blend: 5.0,
                },
            ],
            fidgets: vec![vec![4, 10], vec![3, 9], vec![7, 13]],
            holds: vec![BackHold {
                movement: RigMove::Glide,
                part: 0,
                sequence: 6,
                blend: 10.0,
            }],
        }
    }

    const TICK: f32 = 1.0 / 60.0;

    fn run(director: &mut BackDirector, back: &BackAnimations, movement: RigMove, ticks: usize) {
        for _ in 0..ticks {
            director.step(back, Some(movement), 1.0, TICK);
        }
    }

    fn x(pose: &Option<Sample>) -> f32 {
        pose.as_ref().unwrap().1[0].x
    }

    #[test]
    fn the_heli_jump_starts_the_rows_sequences_and_a_run_sends_them_back_to_rest() {
        let back = back();
        let mut director = BackDirector::new(1);
        run(&mut director, &back, RigMove::Run, 3);
        assert_eq!(director.sequences(), [1, 1]);
        let poses = director.step(&back, Some(RigMove::HeliJump), 1.0, TICK);
        assert_eq!(director.sequences(), [5, 11]);
        // A tick into a 5-tick blend from rest (sequence 1's x is 100 and up) to 500.
        let at = x(&poses[0]);
        assert!(at > 100.0 && at < 500.0, "{at}");
        run(&mut director, &back, RigMove::HeliJump, 4);
        assert!(
            (x(&director.step(&back, Some(RigMove::HeliJump), 1.0, TICK)[0]) - 500.0).abs() < 0.2
        );
        // Landing plays the same row: the pack plays it already, so nothing restarts.
        director.step(
            &back,
            Some(RigMove::Land(crate::Landing::HeliJump)),
            1.0,
            TICK,
        );
        assert_eq!(director.sequences(), [5, 11]);
        run(&mut director, &back, RigMove::Run, 1);
        assert_eq!(director.sequences(), [1, 1]);
    }

    #[test]
    fn gliding_holds_the_rotor_and_its_end_blends_back_slower() {
        let back = back();
        let mut director = BackDirector::new(1);
        run(&mut director, &back, RigMove::Glide, 1);
        assert_eq!(director.sequences(), [6, 1], "Clank stays at rest");
        run(&mut director, &back, RigMove::Glide, 40);
        assert_eq!(director.sequences(), [6, 1], "the rotor loops");
        run(&mut director, &back, RigMove::Fall, 1);
        assert_eq!(director.sequences(), [1, 1]);
        // 19 ticks back, not 7: after 10 it is still on its way from the rotor.
        run(&mut director, &back, RigMove::Fall, 9);
        let at = x(&director.step(&back, Some(RigMove::Fall), 1.0, TICK)[0]);
        assert!(at > 100.5, "still blending: {at}");
    }

    #[test]
    fn standing_idle_clank_fidgets_then_comes_back_to_rest() {
        let back = back();
        let mut director = BackDirector::new(3);
        run(&mut director, &back, RigMove::Idle, 10);
        assert_eq!(director.sequences(), [1, 1], "not in the first 10 ticks");
        run(&mut director, &back, RigMove::Idle, 1);
        let fidget = director.sequences();
        assert!(back.fidgets.contains(&fidget), "{fidget:?}");
        // Once it has played through (at most 330 ticks with its blend) both rest again,
        // and the next waits at least 110 ticks.
        let mut rested = None;
        for tick in 0..400 {
            run(&mut director, &back, RigMove::Idle, 1);
            if director.sequences() == [1, 1] {
                rested = Some(tick);
                break;
            }
        }
        assert!(rested.is_some(), "back to rest");
        run(&mut director, &back, RigMove::Idle, 100);
        assert_eq!(director.sequences(), [1, 1]);
    }

    #[test]
    fn a_fidget_runs_on_when_the_hero_comes_back_to_the_idle() {
        let back = back();
        let mut director = BackDirector::new(5);
        run(&mut director, &back, RigMove::Idle, 11);
        let fidget = director.sequences();
        assert_ne!(fidget, [1, 1]);
        // The hero's own fidget sends the back to rest...
        run(&mut director, &back, RigMove::Fidget(0), 1);
        assert_eq!(director.sequences(), [1, 1]);
        // ...but coming back to the idle from it, a fidget under way runs on.
        let mut director = BackDirector::new(5);
        run(&mut director, &back, RigMove::IdleHurt, 11);
        let fidget = director.sequences();
        run(&mut director, &back, RigMove::Idle, 1);
        assert_eq!(director.sequences(), fidget);
    }

    #[test]
    fn the_back_follows_the_heros_rate_only_while_his_move_has_rows() {
        let back = back();
        let mut director = BackDirector::new(1);
        // Gliding at rate 2: the glide has no row, so the rotor plays at its own pace.
        for _ in 0..30 {
            director.step(&back, Some(RigMove::Glide), 2.0, TICK);
        }
        let glide = director.parts[0].ticks;
        assert!(
            (glide - 20.0).abs() < 0.01,
            "10 of 30 ticks blending: {glide}"
        );
        let mut director = BackDirector::new(1);
        for _ in 0..30 {
            director.step(&back, Some(RigMove::HeliJump), 2.0, TICK);
        }
        let heli = director.parts[0].ticks;
        assert!(
            (heli - 50.0).abs() < 0.01,
            "5 blending, then 25 at twice: {heli}"
        );
    }
}
