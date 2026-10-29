//! The gun's own animation in the hero's hand (`weapons.md`, "The gun's own animation"),
//! as UYA plays a held gun's moby:
//! - It comes out on its sequence 0, which grows it from nothing; once that has come round,
//!   it goes to its rest, sequence 1, over a couple of ticks (`hand_slots_update`,
//!   0x4E93B8).
//! - Each shot does what the gun's [`Recoil`] says, mostly playing sequence 2 once.
//! - Any sequence that comes round with no blend running goes back to its rest the same
//!   way, so a recoil plays once and settles.
//! - A gun whose [`GunMotion::sustain`] says so plays sequence 2 for as long as it keeps
//!   something up: each tick after the rule above, it cuts to it if it isn't on it.
//! - Every sequence loops, as a moby's does; a change of sequence blends from where the gun
//!   is to the new sequence's first pose, which then plays on, at speed 1.
//!
//! Put away, the game's gun is gone a frame later, so the rig drops it at once and the next
//! grows in.

use std::collections::BTreeMap;

use crate::clips::{RigClip, Sample, mix};
use crate::guns::{GunMotion, Recoil};

const DRAW: u8 = 0;
const REST: u8 = 1;
const RECOIL: u8 = 2;

#[derive(Debug, Clone, PartialEq)]
struct Blend {
    from: Sample,
    length: f32,
    done: f32,
}

/// Where a held gun's own animation is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GunPlayer {
    sequence: u8,
    /// Ticks into the sequence's loop, and how fast it plays.
    ticks: f32,
    speed: f32,
    blend: Option<Blend>,
    /// The sequence came round on the last tick (UYA's bit 0x02 of the moby's +0x60).
    wrapped: bool,
    /// The hero's shots as last seen ([`rac_hero::GunHand::shots`]).
    shots: u8,
    /// A fraction of a tick not yet played.
    pending: f32,
}

impl GunPlayer {
    /// A gun just out, with the hero's shot count as it comes.
    pub fn new(shots: u8) -> Self {
        Self {
            sequence: DRAW,
            ticks: 0.0,
            speed: 1.0,
            blend: None,
            wrapped: false,
            shots,
            pending: 0.0,
        }
    }

    /// Moves on by `seconds` while the hero has fired `shots` (counting round) and the gun
    /// is `sustaining` or not, and returns the gun's pose, if it has a clip for where it is.
    pub fn step(
        &mut self,
        clips: &BTreeMap<u8, RigClip>,
        motion: &GunMotion,
        shots: u8,
        sustaining: bool,
        seconds: f32,
    ) -> Option<Sample> {
        self.pending += seconds * 60.0;
        let whole = self.pending.floor();
        self.pending -= whole;
        for _ in 0..whole as u32 {
            self.tick(clips, motion, shots, sustaining);
        }
        self.pose(clips)
    }

    fn tick(
        &mut self,
        clips: &BTreeMap<u8, RigClip>,
        motion: &GunMotion,
        shots: u8,
        sustaining: bool,
    ) {
        if shots != self.shots {
            self.shots = shots;
            self.fire(clips, motion);
        }
        // Come round with no blend running, it goes to its rest.
        if self.wrapped
            && self.blend.is_none()
            && self.sequence != REST
            && clips.contains_key(&REST)
        {
            self.start(clips, REST, motion.settle_ticks);
        }
        // Kept up, it goes straight back (the Hydra's 0x441C78 call, after its rest's).
        if sustaining && motion.sustain && self.sequence != RECOIL && clips.contains_key(&RECOIL) {
            self.start(clips, RECOIL, 0.0);
        }
        self.wrapped = false;
        match &mut self.blend {
            Some(blend) => {
                blend.done += 1.0;
                if blend.done >= blend.length {
                    self.ticks = blend.done - blend.length;
                    self.blend = None;
                }
            }
            None => {
                self.ticks += self.speed;
                let cycle = clips.get(&self.sequence).map_or(0.0, RigClip::cycle);
                if cycle > 0.0 && self.ticks >= cycle {
                    self.ticks %= cycle;
                    self.wrapped = true;
                }
            }
        }
    }

    /// A shot.
    fn fire(&mut self, clips: &BTreeMap<u8, RigClip>, motion: &GunMotion) {
        if !clips.contains_key(&RECOIL) {
            return;
        }
        if let Some(ticks) = motion.early
            && self.sequence == DRAW
        {
            self.start(clips, REST, ticks);
            return;
        }
        match motion.recoil {
            Recoil::None => {}
            Recoil::Cut => self.start(clips, RECOIL, 0.0),
            Recoil::Restart { speed } => {
                if self.sequence != RECOIL {
                    self.start(clips, RECOIL, 0.0);
                }
                self.speed = speed;
            }
            Recoil::Blend { ticks } => self.start(clips, RECOIL, ticks),
        }
    }

    /// Plays `sequence` from its start, blending from where the gun is over `blend` ticks
    /// (a cut for none).
    fn start(&mut self, clips: &BTreeMap<u8, RigClip>, sequence: u8, blend: f32) {
        let from = self.pose(clips);
        self.sequence = sequence;
        self.ticks = 0.0;
        self.speed = 1.0;
        self.blend = from.filter(|_| blend > 0.0).map(|from| Blend {
            from,
            length: blend,
            done: 0.0,
        });
    }

    fn pose(&self, clips: &BTreeMap<u8, RigClip>) -> Option<Sample> {
        let pose = clips.get(&self.sequence)?.at_time(self.ticks)?;
        Some(match &self.blend {
            Some(blend) => mix(&blend.from, &pose, (blend.done / blend.length).min(1.0)),
            None => pose,
        })
    }

    #[cfg(test)]
    fn at(&self) -> (u8, bool) {
        (self.sequence, self.blend.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clips::RigKeyframe;
    use bevy::prelude::*;

    /// A one-joint clip of `keyframes` keyframes 2 ticks apart, its joint turned about Y
    /// by the keyframe's number plus `base`, in radians.
    fn clip(keyframes: usize, base: f32) -> RigClip {
        RigClip {
            keyframes: (0..keyframes)
                .map(|i| RigKeyframe {
                    ticks: 2.0,
                    frame: i as f32,
                    rotations: vec![Quat::from_rotation_y(base + i as f32 * 0.1)],
                    offsets: vec![Vec3::ZERO],
                    scales: vec![Vec3::ONE],
                })
                .collect(),
            looping: true,
            start: 0,
        }
    }

    /// A draw of 22 ticks (UYA's Shock Blaster's: 11 keyframes of 2), a rest of 8 and a
    /// recoil of 6.
    fn clips() -> BTreeMap<u8, RigClip> {
        BTreeMap::from([
            (DRAW, clip(11, 0.0)),
            (REST, clip(4, 1.0)),
            (RECOIL, clip(3, 2.0)),
        ])
    }

    const UYA: GunMotion = GunMotion {
        recoil: Recoil::Cut,
        sustain: false,
        early: None,
        settle_ticks: 2.0,
    };

    fn run(player: &mut GunPlayer, motion: &GunMotion, shots: u8, ticks: u32) {
        held(player, motion, shots, false, ticks);
    }

    fn held(player: &mut GunPlayer, motion: &GunMotion, shots: u8, on: bool, ticks: u32) {
        for _ in 0..ticks {
            player.step(&clips(), motion, shots, on, 1.0 / 60.0);
        }
    }

    #[test]
    fn a_gun_grows_in_then_blends_into_its_rest_over_two_ticks() {
        let mut player = GunPlayer::new(0);
        run(&mut player, &UYA, 0, 22);
        // Its draw came round on the 22nd tick; the next starts the blend.
        assert_eq!(player.at(), (DRAW, false));
        run(&mut player, &UYA, 0, 1);
        assert_eq!(player.at(), (REST, true));
        run(&mut player, &UYA, 0, 2);
        assert_eq!(player.at(), (REST, false));
        // It stays at rest as its loop comes round.
        run(&mut player, &UYA, 0, 40);
        assert_eq!(player.at(), (REST, false));
    }

    #[test]
    fn a_shot_cuts_to_the_recoil_which_plays_once_and_settles() {
        let mut player = GunPlayer::new(0);
        run(&mut player, &UYA, 0, 30);
        run(&mut player, &UYA, 1, 1);
        assert_eq!(player.at(), (RECOIL, false));
        run(&mut player, &UYA, 1, 5);
        assert_eq!(player.at(), (RECOIL, false));
        run(&mut player, &UYA, 1, 1);
        assert_eq!(player.at(), (REST, true));
    }

    #[test]
    fn the_n90_restarts_its_recoil_only_when_it_isnt_playing_it_and_plays_it_fast() {
        let motion = GunMotion {
            recoil: Recoil::Restart { speed: 3.0 },
            ..UYA
        };
        let mut player = GunPlayer::new(0);
        run(&mut player, &motion, 0, 30);
        run(&mut player, &motion, 1, 1);
        assert_eq!(player.at(), (RECOIL, false));
        let ticks = player.ticks;
        // Another shot while it plays doesn't start it again.
        run(&mut player, &motion, 2, 1);
        assert_eq!(player.at(), (RECOIL, false));
        assert_eq!(player.ticks, (ticks + 3.0) % 6.0);
    }

    #[test]
    fn fired_while_still_growing_in_a_gun_with_an_early_rule_goes_to_its_rest() {
        let motion = GunMotion {
            early: Some(4.0),
            ..UYA
        };
        let mut player = GunPlayer::new(0);
        run(&mut player, &motion, 0, 5);
        run(&mut player, &motion, 1, 1);
        assert_eq!(player.at(), (REST, true));
        run(&mut player, &motion, 1, 4);
        assert_eq!(player.at(), (REST, false));
    }

    #[test]
    fn a_blended_recoil_eases_from_the_rest_pose() {
        let motion = GunMotion {
            recoil: Recoil::Blend { ticks: 2.0 },
            ..UYA
        };
        let mut player = GunPlayer::new(0);
        run(&mut player, &motion, 0, 30);
        let rest = player.pose(&clips()).unwrap().0[0];
        player.step(&clips(), &motion, 1, false, 1.0 / 60.0);
        let halfway = player.pose(&clips()).unwrap().0[0];
        let recoil = clips()[&RECOIL].at_time(0.0).unwrap().0[0];
        assert_eq!(player.at(), (RECOIL, true));
        assert!(halfway.angle_between(rest) > 0.01);
        assert!(halfway.angle_between(recoil) > 0.01);
    }

    #[test]
    fn a_gun_kept_up_loops_its_sequence_2_until_let_go_then_settles() {
        // The Spitting Hydra's: its shots do nothing to it.
        let motion = GunMotion {
            recoil: Recoil::None,
            sustain: true,
            ..UYA
        };
        let mut player = GunPlayer::new(0);
        run(&mut player, &motion, 0, 30);
        held(&mut player, &motion, 0, true, 1);
        assert_eq!(player.at(), (RECOIL, false));
        // Round and round, never back to its rest.
        for _ in 0..20 {
            held(&mut player, &motion, 0, true, 1);
            assert_eq!(player.at(), (RECOIL, false));
        }
        // Let go as it fires, it plays out its loop, then settles.
        let mut out = 0;
        while player.at().0 == RECOIL {
            held(&mut player, &motion, 1, false, 1);
            out += 1;
        }
        assert!(out <= 7, "{out}");
        assert_eq!(player.at(), (REST, true));
        // A gun without the rule keeps its rest while kept up.
        let mut player = GunPlayer::new(0);
        run(&mut player, &UYA, 0, 30);
        held(&mut player, &UYA, 0, true, 10);
        assert_eq!(player.at(), (REST, false));
    }
}
