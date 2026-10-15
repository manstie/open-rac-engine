//! Which clip plays, and how fast (P1.17): the hero's move ([`RigMove::of`]), refined by
//! what the rig remembers, as UYA's own animation code does (`hero-controller.md`,
//! "Animations per state", read in code on UYA NTSC):
//! - the walk sets off first, runs above 2.35 u/s and sets off again below 1.9, keeping its
//!   place in the stride, each at a rate that follows his speed;
//! - standing idle, fidgets play now and then by the game's rules; at low health a hurt
//!   idle stands in, with a fidget of its own;
//! - coming down on his feet, a jump finishes its own animation and a fall has its landing,
//!   deeper after a long fall, until he moves off;
//! - a skid straight out of a run, or out of a long jump, has the run's own skid.

use bevy::math::Vec2;
use rac_hero::HeroMotion;

use crate::clips::{Landing, RUN_BELOW, RigAnimations, RigClip, RigMove, SET_OFF_BELOW};

/// At or under this share of his full health the hero stands hurt (UYA 0x4DC080).
pub(crate) const HURT_AT: f32 = 0.2;
/// Ticks after any fidget before another may play (UYA sets 120).
const FIDGET_GAP: u32 = 120;
/// The hurt idle's fidget plays at the end of one of its loops this often.
const HURT_FIDGET_CHANCE: f32 = 0.33;
/// A fall plays its falling clip once it has lasted this many ticks, or at once from
/// higher than [`FALL_FAR_ABOVE`] u; before that it holds the step-off pose, and lands
/// without a landing (UYA's state 6).
const FALL_SHORT_TICKS: u32 = 17;
const FALL_FAR_ABOVE: f32 = 1.75;
/// A fall at least this long lands deeper.
const LONG_FALL_TICKS: u32 = 90;
/// The walk changes between setting off and running only after this many ticks of it.
const WALK_SETTLE_TICKS: u32 = 4;
/// A landing plays while he is no faster than this. A jump that lands with the stick let
/// go keeps at most 2.5 u/s (`uya.ron`, `air.land_release_speed`); with it held he walks
/// off, as the game's landing gives way to the walk.
const LANDING_UP_TO: f32 = 2.5;
/// A skid out of a run whose cycle is between these frames takes the run's skid.
const RUN_SKID_FRAMES: (f32, f32) = (3.0, 16.0);

/// What the rig remembers to choose and time its clips.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Director {
    /// The move whose clip plays, and how far into it (ticks).
    pub current: Option<RigMove>,
    pub ticks: f32,
    /// Ticks left before each fidget may play again, and before any may.
    cooldowns: Vec<u32>,
    gap: u32,
    /// Fractions of a tick not yet counted on those clocks.
    pending: f32,
    rng: u32,
    /// The fall under way: how long it has lasted, and whether it is falling far.
    fall_ticks: u32,
    fell_far: bool,
    /// His speed when the landing began; speeding up ends it.
    landing_speed: f32,
}

impl Director {
    /// `seed` varies the fidgets between heroes.
    pub fn new(seed: u32) -> Self {
        Self {
            current: None,
            ticks: 0.0,
            cooldowns: Vec::new(),
            gap: 0,
            pending: 0.0,
            rng: seed | 1,
            fall_ticks: 0,
            fell_far: false,
            landing_speed: 0.0,
        }
    }

    /// Moves on by `seconds`, and returns the move whose clip plays, if any. `health` is
    /// his share of full health, if he has any.
    pub fn step(
        &mut self,
        hero: &HeroMotion,
        health: Option<f32>,
        animations: Option<&RigAnimations>,
        seconds: f32,
    ) -> Option<RigMove> {
        let elapsed = seconds * 60.0;
        self.pending += elapsed;
        let whole = self.pending.floor();
        self.pending -= whole;
        let whole = whole as u32;
        if let Some(animations) = animations {
            self.cooldowns.resize(animations.fidgets.len(), 0);
        }
        for cooldown in &mut self.cooldowns {
            *cooldown = cooldown.saturating_sub(whole);
        }
        self.gap = self.gap.saturating_sub(whole);

        let speed = Vec2::new(hero.velocity.x, hero.velocity.z).length();
        let hurt = health.is_some_and(|share| share <= HURT_AT);
        let clip = |movement: RigMove| animations.and_then(|a| a.clips.get(&movement));
        let previous = self.current;
        let base = RigMove::of(hero);

        if base == Some(RigMove::Fall) {
            if !matches!(previous, Some(RigMove::Fall | RigMove::FallStart)) {
                self.fell_far = false;
            }
            self.fall_ticks = hero.ticks;
            self.fell_far |=
                hero.ticks > FALL_SHORT_TICKS || hero.height_above_ground > FALL_FAR_ABOVE;
        }

        let mut next = match base {
            Some(RigMove::Fall) if !self.fell_far => Some(RigMove::FallStart),
            Some(RigMove::Skid) => Some(match previous {
                Some(RigMove::LongJump | RigMove::RunSkid) => RigMove::RunSkid,
                Some(RigMove::Run)
                    if clip(RigMove::Run).is_some_and(|run| {
                        let frame = run.frame_at(self.ticks);
                        frame > RUN_SKID_FRAMES.0 && frame < RUN_SKID_FRAMES.1
                    }) =>
                {
                    RigMove::RunSkid
                }
                _ => RigMove::Skid,
            }),
            Some(RigMove::SetOff | RigMove::Run) => Some(match previous {
                Some(RigMove::SetOff)
                    if speed > SET_OFF_BELOW && hero.ticks > WALK_SETTLE_TICKS =>
                {
                    RigMove::Run
                }
                Some(RigMove::Run) if speed < RUN_BELOW && hero.ticks > WALK_SETTLE_TICKS => {
                    RigMove::SetOff
                }
                Some(stride @ (RigMove::SetOff | RigMove::Run)) => stride,
                // The game always sets off first, even landing at a run.
                _ => RigMove::SetOff,
            }),
            Some(RigMove::Idle) => Some(self.idle(previous, hurt, animations, whole)),
            other => other,
        };

        // Coming down standing: the jump's own landing, or the fall's.
        if matches!(base, Some(RigMove::Idle | RigMove::SetOff | RigMove::Run)) {
            if let Some(RigMove::Land(landing)) = previous {
                let going_on = clip(RigMove::Land(landing))
                    .is_some_and(|land| !land.finished(self.ticks))
                    && speed <= self.landing_speed + 0.05;
                if going_on {
                    next = Some(RigMove::Land(landing));
                }
            } else if let Some(landing) = previous.and_then(|p| self.landing_after(p))
                && speed <= LANDING_UP_TO
                && clip(RigMove::Land(landing)).is_some()
            {
                self.landing_speed = speed;
                next = Some(RigMove::Land(landing));
            }
        }

        if next == previous {
            self.ticks += elapsed * rate(next, speed);
        } else {
            self.ticks = match (previous, next, animations) {
                (Some(from @ (RigMove::SetOff | RigMove::Run)), Some(to), Some(animations))
                    if matches!(to, RigMove::SetOff | RigMove::Run) =>
                {
                    stride_kept(animations, from, to, self.ticks).unwrap_or(0.0)
                }
                _ => 0.0,
            };
            self.current = next;
        }
        self.current
    }

    /// Standing idle: the idle, the hurt idle, or a fidget.
    fn idle(
        &mut self,
        previous: Option<RigMove>,
        hurt: bool,
        animations: Option<&RigAnimations>,
        ticks: u32,
    ) -> RigMove {
        let clip = |movement: RigMove| animations.and_then(|a| a.clips.get(&movement));
        let standing = if hurt && clip(RigMove::IdleHurt).is_some() {
            RigMove::IdleHurt
        } else {
            RigMove::Idle
        };
        let Some(animations) = animations else {
            return standing;
        };
        if let Some(RigMove::Fidget(number)) = previous
            && clip(RigMove::Fidget(number)).is_some_and(|f| !f.finished(self.ticks))
        {
            return RigMove::Fidget(number);
        }
        let rules = &animations.fidgets;
        let cooldowns = self.cooldowns.clone();
        let ready = |number: usize| {
            cooldowns.get(number) == Some(&0)
                && clip(RigMove::Fidget(number as u8)).is_some()
                && rules[number].interval > 0.0
        };
        match previous {
            // The hurt idle's fidget, now and then as its loop comes round.
            Some(RigMove::IdleHurt) if standing == RigMove::IdleHurt => {
                let round = clip(RigMove::IdleHurt).map_or(0.0, RigClip::cycle);
                if round > 0.0 && self.ticks >= round {
                    self.ticks -= round;
                    let chosen = (0..rules.len()).find(|&n| rules[n].low_health && ready(n));
                    if let Some(number) = chosen
                        && self.random() < HURT_FIDGET_CHANCE
                    {
                        return self.start_fidget(number, animations);
                    }
                }
            }
            // Each tick standing in the plain idle, a chance of each fidget that's ready.
            Some(RigMove::Idle) if standing == RigMove::Idle => {
                for _ in 0..ticks {
                    if self.gap > 0 {
                        break;
                    }
                    let chances: Vec<(usize, f32)> = (0..rules.len())
                        .filter(|&n| !rules[n].low_health && ready(n))
                        .map(|n| (n, 1.0 / (rules[n].interval * 60.0).floor().max(1.0)))
                        .collect();
                    let roll = self.random();
                    let mut below = 0.0;
                    for (number, chance) in chances {
                        below += chance;
                        if roll < below {
                            return self.start_fidget(number, animations);
                        }
                    }
                }
            }
            _ => {}
        }
        standing
    }

    fn start_fidget(&mut self, number: usize, animations: &RigAnimations) -> RigMove {
        self.gap = FIDGET_GAP;
        if let (Some(cooldown), Some(rule)) = (
            self.cooldowns.get_mut(number),
            animations.fidgets.get(number),
        ) {
            *cooldown = (rule.cooldown * 60.0) as u32;
        }
        RigMove::Fidget(number as u8)
    }

    /// The landing that follows `air` when he comes down standing.
    fn landing_after(&self, air: RigMove) -> Option<Landing> {
        match air {
            RigMove::Fall if self.fall_ticks >= LONG_FALL_TICKS => Some(Landing::LongFall),
            other => other.landing(),
        }
    }

    /// A number from 0 to 1 (xorshift32): the fidgets are for show, so each rig keeps its
    /// own.
    fn random(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// How fast a move's clip plays. UYA times the walk by his speed in u per frame
/// (`hero_walk_anim`, 0x5024B8): setting off 157 times it (0.6 to 4), running 13 times
/// (0.6 to 2.2), walking backwards 25 times (0.7 to 2.2). The double jump plays at 0.8.
fn rate(movement: Option<RigMove>, speed: f32) -> f32 {
    let per_frame = speed / 60.0;
    match movement {
        Some(RigMove::SetOff) => (per_frame * 157.0).clamp(0.6, 4.0),
        Some(RigMove::Run) => (per_frame * 13.0).clamp(0.6, 2.2),
        Some(RigMove::StrafeBack) => (per_frame * 25.0).clamp(0.7, 2.2),
        Some(RigMove::DoubleJump) => 0.8,
        _ => 1.0,
    }
}

/// Where the walk's new clip picks up when it changes between setting off and running:
/// at the same share of the stride, moved on by the game's few keyframes (UYA 0x1DBE60).
fn stride_kept(animations: &RigAnimations, from: RigMove, to: RigMove, ticks: f32) -> Option<f32> {
    let (old, new) = (animations.clips.get(&from)?, animations.clips.get(&to)?);
    let (old_count, new_count) = (old.keyframes.len(), new.keyframes.len());
    if old_count == 0 || new_count == 0 {
        return None;
    }
    let along = match to {
        RigMove::Run => animations.walk_phase[0],
        _ => animations.walk_phase[1],
    };
    let keyframe = (old.keyframe_at(ticks) * new_count / old_count + along) % new_count;
    Some(new.ticks_to(keyframe))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clips::{FidgetRule, RigKeyframe};
    use bevy::math::{Quat, Vec3};
    use rac_hero::{HeroState, JumpKind};

    /// A clip of `count` keyframes `ticks` long each, whose frames count up from 0.
    fn clip(count: usize, ticks: f32, looping: bool) -> RigClip {
        RigClip {
            keyframes: (0..count)
                .map(|i| RigKeyframe {
                    ticks,
                    frame: i as f32,
                    rotations: vec![Quat::IDENTITY],
                    offsets: vec![Vec3::ZERO],
                    scales: vec![Vec3::ONE],
                })
                .collect(),
            looping,
            start: 0,
        }
    }

    fn animations() -> RigAnimations {
        let mut animations = RigAnimations {
            fidgets: vec![
                FidgetRule {
                    interval: 1.0 / 60.0,
                    cooldown: 5.0,
                    low_health: false,
                },
                FidgetRule {
                    interval: 4.0,
                    cooldown: 2.5,
                    low_health: true,
                },
            ],
            walk_phase: [1, 5],
            ..Default::default()
        };
        for (movement, count, looping) in [
            (RigMove::Idle, 10, true),
            (RigMove::IdleHurt, 10, true),
            (RigMove::Fidget(0), 5, false),
            (RigMove::Fidget(1), 5, false),
            (RigMove::SetOff, 33, true),
            (RigMove::Run, 23, true),
            (RigMove::Skid, 4, false),
            (RigMove::RunSkid, 4, false),
            (RigMove::Fall, 6, true),
            (RigMove::Jump, 20, false),
            (RigMove::Land(Landing::Jump), 8, false),
            (RigMove::Land(Landing::Fall), 6, false),
            (RigMove::Land(Landing::LongFall), 11, false),
        ] {
            animations.clips.insert(movement, clip(count, 2.0, looping));
        }
        animations
    }

    const TICK: f32 = 1.0 / 60.0;

    fn run(director: &mut Director, hero: &HeroMotion, a: &RigAnimations, ticks: usize) {
        for _ in 0..ticks {
            director.step(hero, Some(1.0), Some(a), TICK);
        }
    }

    #[test]
    fn the_walk_sets_off_then_runs_and_keeps_its_place_in_the_stride() {
        let a = animations();
        let mut director = Director::new(1);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.state = HeroState::Walk;
        hero.velocity = Vec3::new(0.0, 0.0, -6.0);
        hero.ticks = 0;
        assert_eq!(
            director.step(&hero, None, Some(&a), TICK),
            Some(RigMove::SetOff)
        );
        hero.ticks = 5;
        // 6 u/s: setting off plays at its fastest, 4.
        let before = director.ticks;
        director.step(&hero, None, Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::Run), "past 2.35 u/s");
        // Set-off keyframe k of 33 becomes run keyframe k·23/33 + 1.
        let set_off_keyframe = a.clips[&RigMove::SetOff].keyframe_at(before);
        let expected = (set_off_keyframe * 23 / 33 + 1) % 23;
        assert_eq!(a.clips[&RigMove::Run].keyframe_at(director.ticks), expected);
        // Slowing to 2 u/s keeps running (above 1.9); below it, he sets off again.
        hero.velocity = Vec3::new(0.0, 0.0, -2.0);
        director.step(&hero, None, Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::Run));
        hero.velocity = Vec3::new(0.0, 0.0, -1.5);
        director.step(&hero, None, Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::SetOff));
    }

    #[test]
    fn the_run_plays_faster_the_faster_he_runs() {
        assert!((rate(Some(RigMove::Run), 6.0) - 1.3).abs() < 1e-5);
        assert_eq!(rate(Some(RigMove::Run), 30.0), 2.2);
        assert_eq!(rate(Some(RigMove::SetOff), 0.0), 0.6);
        assert_eq!(rate(Some(RigMove::SetOff), 2.0), 4.0);
    }

    #[test]
    fn standing_idle_a_fidget_plays_then_the_gap_and_cooldown_hold_off_the_next() {
        let a = animations();
        let mut director = Director::new(7);
        let hero = HeroMotion::new(Vec3::ZERO, 0.0);
        director.step(&hero, Some(1.0), Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::Idle));
        // Fidget 0 is due every tick on average; it plays at once.
        run(&mut director, &hero, &a, 3);
        assert_eq!(director.current, Some(RigMove::Fidget(0)));
        // It plays to its end (8 ticks), then the idle comes back and stays for the gap.
        run(&mut director, &hero, &a, 10);
        assert_eq!(director.current, Some(RigMove::Idle));
        run(&mut director, &hero, &a, 100);
        assert_eq!(director.current, Some(RigMove::Idle), "120 ticks' gap");
        // The cooldown is 5 s, so after the gap it still waits.
        run(&mut director, &hero, &a, 150);
        assert_eq!(director.current, Some(RigMove::Idle));
        let mut again = false;
        for _ in 0..100 {
            run(&mut director, &hero, &a, 1);
            again |= director.current == Some(RigMove::Fidget(0));
        }
        assert!(again, "once the cooldown is over");
    }

    #[test]
    fn at_low_health_he_stands_hurt() {
        let a = animations();
        let mut director = Director::new(3);
        let hero = HeroMotion::new(Vec3::ZERO, 0.0);
        director.step(&hero, Some(0.2), Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::IdleHurt));
        // Its fidget comes now and then as the loop comes round, never the other's.
        let mut seen = false;
        for _ in 0..2000 {
            director.step(&hero, Some(0.2), Some(&a), TICK);
            assert_ne!(director.current, Some(RigMove::Fidget(0)));
            seen |= director.current == Some(RigMove::Fidget(1));
        }
        assert!(seen);
    }

    #[test]
    fn a_jump_lands_on_its_own_animation_until_he_moves_off() {
        let a = animations();
        let mut director = Director::new(1);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.state = HeroState::Jump(JumpKind::Standing);
        run(&mut director, &hero, &a, 20);
        assert_eq!(director.current, Some(RigMove::Jump));
        // He lands into the walk, braking from 2 u/s.
        hero.state = HeroState::Walk;
        hero.velocity = Vec3::new(0.0, 0.0, -2.0);
        director.step(&hero, None, Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::Land(Landing::Jump)));
        hero.velocity = Vec3::new(0.0, 0.0, -1.0);
        run(&mut director, &hero, &a, 3);
        assert_eq!(director.current, Some(RigMove::Land(Landing::Jump)));
        // Pushing off again ends it.
        hero.velocity = Vec3::new(0.0, 0.0, -4.0);
        director.step(&hero, None, Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::SetOff));
        // Landing at a run, he runs on.
        hero.state = HeroState::Jump(JumpKind::Standing);
        run(&mut director, &hero, &a, 5);
        hero.state = HeroState::Walk;
        hero.velocity = Vec3::new(0.0, 0.0, -6.0);
        director.step(&hero, None, Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::SetOff));
    }

    #[test]
    fn falls_land_by_how_far_they_fell() {
        let a = animations();
        let land_after = |ticks: u32, height: f32| {
            let mut director = Director::new(1);
            let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
            hero.state = HeroState::Fall;
            hero.height_above_ground = height;
            for tick in 0..ticks {
                hero.ticks = tick;
                director.step(&hero, None, Some(&a), TICK);
            }
            let falling = director.current;
            hero.state = HeroState::Idle;
            hero.ticks = 0;
            (falling, director.step(&hero, None, Some(&a), TICK))
        };
        assert_eq!(
            land_after(10, 0.5),
            (Some(RigMove::FallStart), Some(RigMove::Idle)),
            "a short drop lands without a landing"
        );
        assert_eq!(
            land_after(30, 0.5),
            (Some(RigMove::Fall), Some(RigMove::Land(Landing::Fall)))
        );
        assert_eq!(
            land_after(5, 3.0).0,
            Some(RigMove::Fall),
            "from high up he falls at once"
        );
        assert_eq!(
            land_after(100, 3.0).1,
            Some(RigMove::Land(Landing::LongFall))
        );
    }

    #[test]
    fn a_long_jump_and_a_run_skid_the_runs_way() {
        let a = animations();
        let mut director = Director::new(1);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.state = HeroState::Jump(JumpKind::Long);
        director.step(&hero, None, Some(&a), TICK);
        hero.state = HeroState::Skid;
        assert_eq!(
            director.step(&hero, None, Some(&a), TICK),
            Some(RigMove::RunSkid)
        );
        // A skid from standing still is the plain one.
        hero.state = HeroState::Idle;
        director.step(&hero, None, Some(&a), TICK);
        hero.state = HeroState::Skid;
        assert_eq!(
            director.step(&hero, None, Some(&a), TICK),
            Some(RigMove::Skid)
        );
    }
}
