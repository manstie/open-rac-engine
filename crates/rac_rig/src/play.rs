//! Which clip plays, and how fast (P1.17): the hero's move ([`RigMove::of`]), refined by
//! what the rig remembers, as UYA's own animation code does (`hero-controller.md`,
//! "Animations per state", read in code on UYA NTSC):
//! - the walk sets off first, runs above 2.35 u/s and sets off again below 1.9, keeping its
//!   place in the stride, each at a rate that follows his speed;
//! - standing idle, fidgets play now and then by the game's rules; at low health a hurt
//!   idle stands in, with a fidget of its own;
//! - a jump's air animation is timed to its arc, to show its peak frame at the apex and
//!   its landing frame as he lands (`hero-controller.md`, "Landings");
//! - coming down on his feet, a jump finishes its own animation and a fall has its landing,
//!   deeper after a long fall, until he moves off;
//! - a skid straight out of a run, or out of a long jump, has the run's own skid;
//! - firing a gun standing plays the gun's whole-body firing animation over the idle and
//!   the landings, started again by each shot; on the move or crouched, its arms-only one
//!   is laid over the arms ([`ArmLayer`]; `weapons.md`, "The hero's side of firing");
//! - with a gun in hand, the arms hold it as the gun's hold has them: one arm or both
//!   ([`HoldLayer`]; `weapons.md`, "Holding a gun").

use bevy::math::Vec2;
use rac_hero::{CameraSetting, HeroMotion, HeroState, Hold, JumpKind};

use crate::clips::{
    FlipSide, JumpTiming, Landing, RUN_BELOW, RigAnimations, RigClip, RigMove, SET_OFF_BELOW,
};

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
/// UYA's timing of a jump's clip (`hero_transitions`): the fastest it plays to reach its
/// peak frame; past the apex, how much its rate moves a tick towards the landing frame,
/// the range it's kept to, and the rate above which it skips to its landing; the ticks
/// to land it reckons with when he's over no ground; and the double jump's settling
/// towards its own pace before frame 37.
const APEX_RATE_MAX: f32 = 1.5;
const LANDING_STEP: f32 = 0.2;
const LANDING_RATES: (f32, f32) = (0.2, 2.7);
const SKIP_ABOVE: f32 = 1.8;
const LAND_TICKS_CAP: f32 = 60.0;
const DOUBLE_JUMP_SETTLES_BEFORE: f32 = 37.0;
const DOUBLE_JUMP_STEP: f32 = 0.15;
/// A jump that lands more than this many frames short of its landing frame starts its
/// landing afresh; otherwise its own animation plays on.
const LANDING_SHORT: f32 = 1.5;
/// The ticks a glove's throw takes to reach the keyframe after its first.
const GLOVE_THROW_BLEND: f32 = 9.0;
/// The arms-only firing layer's weight rises this much a tick while it plays, and falls
/// this much once firing ends (UYA 0x4E6A08).
const ARMS_IN: f32 = 0.2;
const ARMS_OUT: f32 = 0.07;
/// The hold layers' weight rises this much a tick while the gun is held, falls this much
/// while the firing arms layer plays, and this much as they leave for another hold
/// (UYA 0x4E61E8).
const HOLD_IN: f32 = 0.25;
const HOLD_OUT: f32 = 0.15;
const HOLD_LEAVING: f32 = 0.1;
/// Ticks the held arms take to go over to their rest, and how much of the way back they
/// come a tick.
const HOLD_TO_REST_TICKS: f32 = 15.0;
const HOLD_FROM_REST: f32 = 0.1;
/// In the flips the arms don't follow, the clavicles lift the held gun past this frame
/// (UYA 0x4E55A8).
const FLIP_LIFT_AFTER: f32 = 25.0;

/// What the rig remembers to choose and time its clips.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Director {
    /// The move whose clip plays, and how far into it (ticks).
    pub current: Option<RigMove>,
    pub ticks: f32,
    /// How fast its clip plays: 1 is a keyframe's own ticks.
    pub rate: f32,
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
    /// The jump under way has been timed to its apex.
    apex_timed: bool,
    /// The gun's shot count last tick: a new shot starts its firing animation again.
    shots: u8,
}

impl Director {
    /// `seed` varies the fidgets between heroes.
    pub fn new(seed: u32) -> Self {
        Self {
            current: None,
            ticks: 0.0,
            rate: 1.0,
            cooldowns: Vec::new(),
            gap: 0,
            pending: 0.0,
            rng: seed | 1,
            fall_ticks: 0,
            fell_far: false,
            landing_speed: 0.0,
            apex_timed: false,
            shots: 0,
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
        let firing = firing_standing(hero, base).filter(|&firing| clip(firing).is_some());
        let shot = hero.gun.shots != self.shots;
        self.shots = hero.gun.shots;

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
            Some(RigMove::Idle) if firing.is_none() => {
                let two_hands = gun_in(hero) == Hold::TwoHands;
                Some(self.idle(previous, hurt, two_hands, animations, whole))
            }
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

        // A glove's throw plays out under the idle (`weapons.md`, "The gloves' throw").
        if base == Some(RigMove::Idle)
            && previous == Some(RigMove::GloveThrow)
            && clip(RigMove::GloveThrow).is_some_and(|throw| !throw.finished(self.ticks))
        {
            next = previous;
        }

        // The gun sets its firing animation over whatever he stands in.
        if firing.is_some() {
            next = firing;
        }

        if next == previous && firing.is_some() && shot {
            self.ticks = 0.0;
            self.rate = 1.0;
        } else if next == previous {
            let timing = next.and_then(|m| animations.and_then(|a| a.jumps.get(&m)));
            match (timing, next.and_then(clip)) {
                // Timed from where this tick leaves it, for the ticks to come.
                (Some(timing), Some(played)) => {
                    self.ticks += elapsed * self.rate;
                    let double = next == Some(RigMove::DoubleJump);
                    self.rate = self.timed(hero, played, timing, double, whole);
                }
                _ => {
                    self.rate = rate(next, speed);
                    self.ticks += elapsed * self.rate;
                }
            }
        } else {
            self.ticks = match (previous, next, animations) {
                (Some(from @ (RigMove::SetOff | RigMove::Run)), Some(to), Some(animations))
                    if matches!(to, RigMove::SetOff | RigMove::Run) =>
                {
                    stride_kept(animations, from, to, self.ticks).unwrap_or(0.0)
                }
                (Some(air), Some(land @ RigMove::Land(_)), Some(animations)) => {
                    played_on(animations, air, land, self.ticks).unwrap_or(0.0)
                }
                // The first keyframe takes the throw's 9 frames' blend (`hero_anim_set`).
                (_, Some(RigMove::GloveThrow), _) => clip(RigMove::GloveThrow)
                    .and_then(|throw| throw.keyframes.get(throw.start))
                    .map_or(0.0, |first| first.ticks - GLOVE_THROW_BLEND),
                _ => 0.0,
            };
            self.rate = rate(next, speed);
            self.apex_timed = false;
            self.current = next;
        }
        self.current
    }

    /// The rate of a jump's clip this frame, `ticks` of it whole, timed to the jump's arc
    /// as UYA's are: once the push-off is over, to show the peak frame at the apex; past
    /// the apex, steered towards showing the landing frame as he lands, skipping to the
    /// landing when that would take too fast a rate.
    fn timed(
        &mut self,
        hero: &HeroMotion,
        clip: &RigClip,
        timing: &JumpTiming,
        double: bool,
        ticks: u32,
    ) -> f32 {
        let mut rate = self.rate;
        let frame = clip.frame_at(self.ticks);
        let keyframe = clip.keyframe_at(self.ticks);
        let keyframe_ticks = clip.keyframes.get(keyframe).map_or(1.0, |k| k.ticks);
        // The rate that shows `target` `within` ticks from now.
        let reaching = |target: f32, within: f32| {
            (target - frame).max(0.0) / timing.keyframe_frames * keyframe_ticks / within
        };
        if !double && !self.apex_timed && hero.ticks > timing.pushoff {
            self.apex_timed = true;
            let within = hero.ticks_to_apex();
            if within > 0.0 {
                rate = reaching(timing.peak, within).min(APEX_RATE_MAX);
            }
        }
        if !hero.past_apex() || keyframe >= timing.landing_keyframe {
            return rate;
        }
        for _ in 0..ticks {
            if double && frame < DOUBLE_JUMP_SETTLES_BEFORE {
                rate = approach(rate, 1.0, DOUBLE_JUMP_STEP);
            } else {
                let within = hero.ticks_to_land(0.0).floor();
                let within = if within >= 1.0 {
                    within.min(LAND_TICKS_CAP)
                } else {
                    LAND_TICKS_CAP
                };
                rate = approach(rate, reaching(timing.landing, within), LANDING_STEP);
                if rate > SKIP_ABOVE {
                    // On to the landing's own start, at the clip's own pace.
                    self.ticks = clip.ticks_to(timing.landing_keyframe + 2);
                    return 1.0;
                }
            }
            rate = rate.clamp(LANDING_RATES.0, LANDING_RATES.1);
        }
        rate
    }

    /// Standing idle: the idle, the hurt idle, or a fidget. `two_hands`: he holds a gun in
    /// both hands.
    fn idle(
        &mut self,
        previous: Option<RigMove>,
        hurt: bool,
        two_hands: bool,
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
                && (rules[number].two_hands || !two_hands)
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

/// `value` moved towards `target` by at most `step` (UYA's `approach`).
fn approach(value: f32, target: f32, step: f32) -> f32 {
    value + (target - value).clamp(-step, step)
}

/// Where the landing after `air` picks up, if not at its start: a jump that came down at
/// or near its landing frame plays on from where it is, the landing being the rest of its
/// own clip (UYA restarts it only when he lands short).
fn played_on(animations: &RigAnimations, air: RigMove, land: RigMove, ticks: f32) -> Option<f32> {
    let timing = animations.jumps.get(&air)?;
    let (jump, landing) = (animations.clips.get(&air)?, animations.clips.get(&land)?);
    if jump.frame_at(ticks) < timing.landing - LANDING_SHORT {
        return None;
    }
    Some((ticks - jump.ticks_to(landing.start)).max(0.0))
}

/// How fast a move's clip plays. UYA times the walk by his speed in u per frame
/// (`hero_walk_anim`, 0x5024B8): setting off 157 times it (0.6 to 4), running 13 times
/// (0.6 to 2.2), walking backwards 25 times (0.7 to 2.2). The double jump plays at 0.8.
/// The gun's whole-body firing animation, while it fires and he stands to fire.
fn firing_standing(hero: &HeroMotion, base: Option<RigMove>) -> Option<RigMove> {
    let gun = &hero.gun;
    if !gun.firing || gun.item.is_none() || !stands_to_fire(hero, base) {
        return None;
    }
    gun.pose.standing.map(RigMove::Fire)
}

/// He fires with the gun's whole-body animation (`hero_fire_anim`, 0x4E6A08): standing
/// (state type 0: the idle and the look, which [`RigMove::of`] both makes
/// [`RigMove::Idle`]), the gun arm not raised against a wall, and not in the lock-strafe
/// camera pitched more than 10° (either way: the sign isn't read).
fn stands_to_fire(hero: &HeroMotion, base: Option<RigMove>) -> bool {
    let pitched = hero.camera_setting == CameraSetting::LockStrafe
        && hero.camera_pitch.abs() > STRAFE_PITCHED.to_radians();
    base == Some(RigMove::Idle) && !hero.gun_arm.raised && !pitched
}

/// In the lock-strafe camera pitched more than this, in degrees, he fires with the arms
/// only.
const STRAFE_PITCHED: f32 = 10.0;

/// The arms-only firing animation laid over the arms (`weapons.md`, "The hero's side of
/// firing"): while the gun fires and he isn't standing, on the right arm, and the left
/// too for a gun held in both hands. Crouched it is the gun's crouched one, otherwise its
/// arms one; most guns have none, and then nothing plays. It blends in at
/// [`ARMS_IN`] of full weight a tick and out at [`ARMS_OUT`] once firing ends, and each
/// shot starts it again.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ArmLayer {
    /// The clip it plays, and how far into it.
    pub playing: Option<RigMove>,
    pub ticks: f32,
    pub weight: f32,
    /// Whether it lies over both arms.
    pub both: bool,
    /// It plays for the gun's firing, rather than fading out.
    pub firing: bool,
    shots: u8,
}

impl ArmLayer {
    /// Moves on by `seconds`.
    pub fn step(&mut self, hero: &HeroMotion, animations: Option<&RigAnimations>, seconds: f32) {
        let elapsed = seconds * 60.0;
        let gun = &hero.gun;
        let wanted = if gun.firing && gun.item.is_some() {
            if hero.state == HeroState::Crouch {
                gun.pose.crouched
            } else if stands_to_fire(hero, RigMove::of(hero)) {
                None
            } else {
                gun.pose.arms
            }
        } else {
            None
        }
        .map(RigMove::FireArms)
        .filter(|arms| animations.is_some_and(|a| a.clips.contains_key(arms)));
        let shot = gun.shots != self.shots;
        self.shots = gun.shots;
        self.firing = wanted.is_some();
        match wanted {
            Some(arms) => {
                if self.playing != Some(arms) || shot {
                    self.ticks = 0.0;
                } else {
                    self.ticks += elapsed;
                }
                self.playing = Some(arms);
                self.both = gun.pose.hold == Hold::TwoHands;
                self.weight = (self.weight + ARMS_IN * elapsed).min(1.0);
            }
            None => {
                self.ticks += elapsed;
                self.weight = (self.weight - ARMS_OUT * elapsed).max(0.0);
                if self.weight == 0.0 {
                    self.playing = None;
                }
            }
        }
    }
}

/// How he holds the gun in his hand: nothing with the wrench, or while a gun comes out.
pub(crate) fn gun_in(hero: &HeroMotion) -> Hold {
    if hero.gun.item.is_some() {
        hero.gun.pose.hold
    } else {
        Hold::None
    }
}

/// The states his hands are on a ledge in, where UYA drops the hold layers (0x1A71D0,
/// set by states 23 to 27).
fn on_ledge(state: HeroState) -> bool {
    matches!(
        state,
        HeroState::LedgeGrab
            | HeroState::LedgeHang
            | HeroState::LedgeShimmy(_)
            | HeroState::Jump(JumpKind::Ledge)
    )
}

/// The arms holding the gun in hand (`weapons.md`, "Holding a gun"; UYA 0x4E61E8): the
/// right arm's layer for a gun held in one hand or both, and the left's for one in both,
/// each playing its arm's clip for the body's move ([`crate::HoldAnimations`]). A layer rises to
/// full weight at [`HOLD_IN`] a tick, and gives way at [`HOLD_OUT`] while the firing arms
/// layer plays. A change of hold, and a ledge, send both off at [`HOLD_LEAVING`] a tick
/// before any comes back. Through the moves the arms don't follow they go over to their
/// rest in [`HOLD_TO_REST_TICKS`], playing it from its start, and come back at
/// [`HOLD_FROM_REST`] a tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct HoldLayer {
    /// The hold the layers are for.
    held: Hold,
    /// Each arm's weight, right then left, and whether it is leaving.
    pub weights: [f32; 2],
    leaving: [bool; 2],
    /// How far over to their rest the arms are, from 0 to 1, and how far into it.
    pub resting: f32,
    pub rest_ticks: f32,
}

impl HoldLayer {
    /// Moves on by `seconds`, the body playing `current` and the firing arms layer
    /// playing for a shot if `firing_arms`.
    pub fn step(
        &mut self,
        hero: &HeroMotion,
        current: Option<RigMove>,
        firing_arms: bool,
        animations: Option<&RigAnimations>,
        seconds: f32,
    ) {
        let elapsed = seconds * 60.0;
        let hold = gun_in(hero);
        let ledge = on_ledge(hero.state);
        if hold != self.held || ledge {
            self.held = hold;
            for (leaving, weight) in self.leaving.iter_mut().zip(&self.weights) {
                *leaving |= *weight > 0.0;
            }
        }
        for side in 0..2 {
            let weight = &mut self.weights[side];
            if self.leaving[side] {
                *weight = (*weight - HOLD_LEAVING * elapsed).max(0.0);
                self.leaving[side] = *weight > 0.0;
                continue;
            }
            let wanted = !ledge
                && match hold {
                    Hold::None => false,
                    Hold::OneHand => side == 0,
                    Hold::TwoHands => true,
                };
            *weight = if wanted && !firing_arms {
                (*weight + HOLD_IN * elapsed).min(1.0)
            } else {
                (*weight - HOLD_OUT * elapsed).max(0.0)
            };
        }
        let resting = animations
            .and_then(|a| a.hold.as_ref())
            .zip(current)
            .is_some_and(|(hold, current)| hold.rest_in.contains(&current));
        if resting {
            if self.resting == 0.0 {
                self.rest_ticks = 0.0;
            } else {
                self.rest_ticks += elapsed;
            }
            self.resting = (self.resting + elapsed / HOLD_TO_REST_TICKS).min(1.0);
        } else {
            self.resting = (self.resting - HOLD_FROM_REST * elapsed).max(0.0);
        }
    }

    /// Whether either arm's layer is on.
    pub fn on(&self) -> bool {
        self.weights.iter().any(|&weight| weight > 0.0)
    }

    /// Whether the clavicles lift the held gun: past [`FLIP_LIFT_AFTER`] of the left,
    /// right and back flips, whose sequences UYA's arms don't follow (0x4E61E8, the
    /// joint springs' records 8 and 9). The forward flip's isn't one of them.
    pub fn lifts(&self, current: Option<RigMove>, ticks: f32, animations: &RigAnimations) -> bool {
        use FlipSide::{Back, Left, Right};
        let flip = matches!(
            current,
            Some(
                RigMove::Flip(Left | Right | Back)
                    | RigMove::Land(Landing::Flip(Left | Right | Back))
            )
        );
        self.on()
            && flip
            && current
                .and_then(|current| animations.clips.get(&current))
                .is_some_and(|clip| clip.frame_at(ticks) > FLIP_LIFT_AFTER)
    }
}

fn rate(movement: Option<RigMove>, speed: f32) -> f32 {
    let per_frame = speed / 60.0;
    match movement {
        Some(RigMove::SetOff) => (per_frame * 157.0).clamp(0.6, 4.0),
        Some(RigMove::Run) => (per_frame * 13.0).clamp(0.6, 2.2),
        Some(RigMove::StrafeBack) => (per_frame * 25.0).clamp(0.7, 2.2),
        Some(RigMove::Wade) => (per_frame * 35.0).clamp(0.6, 1.05),
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
                    two_hands: true,
                },
                FidgetRule {
                    interval: 4.0,
                    cooldown: 2.5,
                    low_health: true,
                    two_hands: true,
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
        // Wading: 35 × the speed a frame, from 0.6 to 1.05, so 1.05 from 1.8 u/s.
        assert_eq!(rate(Some(RigMove::Wade), 0.5), 0.6);
        assert!((rate(Some(RigMove::Wade), 1.2) - 0.7).abs() < 1e-5);
        assert_eq!(rate(Some(RigMove::Wade), 4.9), 1.05);
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
    fn a_jumps_clip_shows_its_peak_frame_at_the_apex_and_its_landing_frame_as_he_lands() {
        use crate::clips::JumpTiming;
        use rac_hero::{BodyPlacement, Collider, HeroProfile, ParryWorld, controller};
        use rac_input::{Action, Buttons, PlayerInput};

        // UYA's standing jump: two frames a keyframe of two ticks, its peak frame 18 and
        // its landing frame 30, the landing from keyframe 14.
        let mut a = animations();
        let mut jump = clip(40, 2.0, false);
        for keyframe in &mut jump.keyframes {
            keyframe.frame *= 2.0;
        }
        let landing = RigClip {
            start: 14,
            ..jump.clone()
        };
        a.clips.insert(RigMove::Jump, jump);
        a.clips.insert(RigMove::Land(Landing::Jump), landing);
        a.jumps.insert(
            RigMove::Jump,
            JumpTiming {
                pushoff: 1,
                peak: 18.0,
                landing: 30.0,
                landing_keyframe: 12,
                keyframe_frames: 2.0,
            },
        );

        let world = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(50.0, 0.5, 50.0),
            },
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
        )]);
        let profile = HeroProfile::uya();
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        let mut input = PlayerInput::default();
        controller::step(&mut hero, &input, 0.0, &profile, &world);
        input.held = Buttons::NONE;
        input.held.insert(Action::Jump);
        input.pressed = input.held;
        let mut director = Director::new(1);
        let frame = |director: &Director, a: &RigAnimations| {
            a.clips[&director.current.unwrap()].frame_at(director.ticks)
        };
        // The frame on the last tick rising, and on the last in the air.
        let (mut at_apex, mut before_landing) = (None, None);
        for _ in 0..120 {
            controller::step(&mut hero, &input, 0.0, &profile, &world);
            input = PlayerInput::default();
            director.step(&hero, Some(1.0), Some(&a), TICK);
            if hero.state == HeroState::Jump(JumpKind::Standing) {
                before_landing = Some(frame(&director, &a));
                if hero.velocity.y > 0.0 {
                    at_apex = before_landing;
                }
            } else if before_landing.is_some() {
                break;
            }
        }
        let (at_apex, landed) = (at_apex.unwrap(), before_landing.unwrap());
        assert!((at_apex - 18.0).abs() < 1.0, "frame {at_apex} at the apex");
        assert!((landed - 30.0).abs() < 1.0, "frame {landed} on landing");
        // Landing at its landing frame, the jump's animation plays on into the landing.
        assert_eq!(director.current, Some(RigMove::Land(Landing::Jump)));
        let on = frame(&director, &a);
        assert!(on >= landed && on < landed + 2.0, "{on} after {landed}");
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

    /// A gun in his hand, firing or not, with UYA's N60's animations.
    fn n60(firing: bool, shots: u8) -> rac_hero::GunHand {
        rac_hero::GunHand {
            out: true,
            firing,
            item: Some(0x2F),
            pose: rac_hero::GunPose {
                hold: Hold::OneHand,
                standing: Some(51),
                arms: None,
                crouched: Some(57),
            },
            shots,
            ..Default::default()
        }
    }

    fn with_guns() -> RigAnimations {
        let mut a = animations();
        a.clips.insert(RigMove::Fire(51), clip(5, 4.0, false));
        a.clips.insert(RigMove::FireArms(57), clip(5, 4.0, false));
        a
    }

    #[test]
    fn a_gloves_throw_plays_out_under_the_idle_from_its_blend() {
        let mut a = animations();
        a.clips.insert(RigMove::GloveThrow, clip(10, 4.0, false));
        a.clips.get_mut(&RigMove::GloveThrow).unwrap().start = 7;
        let mut director = Director::new(3);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.state = HeroState::GloveThrow;
        director.step(&hero, None, Some(&a), 1.0 / 60.0);
        assert_eq!(director.current, Some(RigMove::GloveThrow));
        // Its first keyframe takes the 9 frames' blend, not its own 4.
        assert_eq!(director.ticks, 4.0 - 9.0);
        // Back in the idle it goes on: 5 ticks of blend, 12 of the clip from key 7.
        hero.state = HeroState::Idle;
        run(&mut director, &hero, &a, 10);
        assert_eq!(director.current, Some(RigMove::GloveThrow));
        run(&mut director, &hero, &a, 10);
        assert_ne!(director.current, Some(RigMove::GloveThrow));
    }

    #[test]
    fn firing_standing_plays_the_guns_animation_from_the_start_each_shot() {
        let a = with_guns();
        let mut director = Director::new(3);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.gun = n60(false, 0);
        run(&mut director, &hero, &a, 3);
        assert_ne!(director.current, Some(RigMove::Fire(51)), "out, not firing");

        hero.gun = n60(true, 1);
        run(&mut director, &hero, &a, 10);
        assert_eq!(director.current, Some(RigMove::Fire(51)));
        assert!((director.ticks - 9.0).abs() < 1e-3, "{}", director.ticks);
        // The next shot, 14 ticks after the first as the N60's are, starts it again.
        run(&mut director, &hero, &a, 4);
        hero.gun.shots = 2;
        director.step(&hero, Some(1.0), Some(&a), TICK);
        assert_eq!(director.current, Some(RigMove::Fire(51)));
        assert_eq!(director.ticks, 0.0);
        // Holding its last pose until firing ends, then back to the idle.
        run(&mut director, &hero, &a, 40);
        assert_eq!(director.current, Some(RigMove::Fire(51)));
        hero.gun.firing = false;
        director.step(&hero, Some(1.0), Some(&a), TICK);
        assert!(matches!(
            director.current,
            Some(RigMove::Idle | RigMove::Fidget(_))
        ));
    }

    #[test]
    fn firing_without_the_guns_clip_or_on_the_move_keeps_the_moves_own() {
        let mut director = Director::new(3);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.gun = n60(true, 1);
        let a = animations();
        run(&mut director, &hero, &a, 3);
        assert_ne!(
            director.current,
            Some(RigMove::Fire(51)),
            "no clip for sequence 51"
        );

        let a = with_guns();
        hero.state = HeroState::Walk;
        hero.velocity = Vec3::new(0.0, 0.0, -6.0);
        run(&mut director, &hero, &a, 3);
        assert_eq!(director.current, Some(RigMove::SetOff));
    }

    #[test]
    fn crouched_firing_lays_the_arms_animation_over_and_blends_it_in_and_out() {
        let a = with_guns();
        let mut layer = ArmLayer::default();
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.state = HeroState::Crouch;
        hero.gun = n60(true, 1);
        layer.step(&hero, Some(&a), TICK);
        assert_eq!(layer.playing, Some(RigMove::FireArms(57)));
        assert!(!layer.both, "the N60 is held in one hand");
        assert!((layer.weight - 0.2).abs() < 1e-5);
        for _ in 0..10 {
            layer.step(&hero, Some(&a), TICK);
        }
        assert_eq!(layer.weight, 1.0);
        hero.gun.shots = 2;
        layer.step(&hero, Some(&a), TICK);
        assert_eq!(layer.ticks, 0.0, "a shot starts it again");

        hero.gun.firing = false;
        layer.step(&hero, Some(&a), TICK);
        assert!((layer.weight - 0.93).abs() < 1e-5);
        for _ in 0..14 {
            layer.step(&hero, Some(&a), TICK);
        }
        assert_eq!(layer.weight, 0.0);
        assert_eq!(layer.playing, None);

        // Standing, the whole-body animation plays instead.
        hero.state = HeroState::Idle;
        hero.gun.firing = true;
        layer.step(&hero, Some(&a), TICK);
        assert_eq!(layer.playing, None);
    }

    /// A gun in his hand held as `hold`, not firing.
    fn holding(hold: Hold) -> rac_hero::GunHand {
        rac_hero::GunHand {
            pose: rac_hero::GunPose {
                hold,
                ..n60(false, 0).pose
            },
            ..n60(false, 0)
        }
    }

    fn with_holds() -> RigAnimations {
        let mut a = with_guns();
        let flip = RigMove::Flip(FlipSide::Left);
        a.clips.insert(flip, clip(40, 2.0, false));
        a.hold = Some(crate::HoldAnimations {
            follow: [
                [(RigMove::Idle, clip(10, 2.0, true))].into(),
                [(RigMove::Idle, clip(10, 2.0, true))].into(),
            ],
            rest: [Some(clip(10, 2.0, true)), Some(clip(10, 2.0, true))],
            rest_in: vec![flip],
        });
        a
    }

    fn step_hold(layer: &mut HoldLayer, hero: &HeroMotion, a: &RigAnimations, ticks: usize) {
        for _ in 0..ticks {
            layer.step(hero, RigMove::of(hero), false, Some(a), TICK);
        }
    }

    #[test]
    fn a_gun_in_both_hands_brings_both_arms_in_and_one_in_one_hand_the_right() {
        let a = with_holds();
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        let mut layer = HoldLayer::default();
        step_hold(&mut layer, &hero, &a, 5);
        assert!(!layer.on(), "the wrench holds no gun");

        hero.gun = holding(Hold::TwoHands);
        step_hold(&mut layer, &hero, &a, 1);
        assert!((layer.weights[0] - 0.25).abs() < 1e-5);
        assert!((layer.weights[1] - 0.25).abs() < 1e-5);
        step_hold(&mut layer, &hero, &a, 3);
        assert_eq!(layer.weights, [1.0, 1.0]);

        // A gun in one hand: both go off at 0.1 a tick, then the right comes back alone.
        hero.gun = holding(Hold::OneHand);
        step_hold(&mut layer, &hero, &a, 1);
        assert!((layer.weights[0] - 0.9).abs() < 1e-5);
        step_hold(&mut layer, &hero, &a, 9);
        assert!(layer.weights[0] < 1e-5 && layer.weights[1] < 1e-5);
        step_hold(&mut layer, &hero, &a, 5);
        assert_eq!(layer.weights, [1.0, 0.0]);

        // The whip and the gloves are held in no hold.
        hero.gun = holding(Hold::None);
        step_hold(&mut layer, &hero, &a, 10);
        assert!(!layer.on());
    }

    #[test]
    fn the_held_arms_give_way_to_the_firing_arms_and_leave_on_a_ledge() {
        let a = with_holds();
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.gun = holding(Hold::TwoHands);
        let mut layer = HoldLayer::default();
        step_hold(&mut layer, &hero, &a, 4);
        layer.step(&hero, Some(RigMove::Idle), true, Some(&a), TICK);
        assert!((layer.weights[0] - 0.85).abs() < 1e-5);
        step_hold(&mut layer, &hero, &a, 1);
        assert_eq!(layer.weights[0], 1.0, "back at 0.25 a tick");

        hero.state = HeroState::LedgeHang;
        step_hold(&mut layer, &hero, &a, 1);
        assert!((layer.weights[1] - 0.9).abs() < 1e-5);
        step_hold(&mut layer, &hero, &a, 30);
        assert!(!layer.on(), "off while he hangs");
        hero.state = HeroState::Idle;
        step_hold(&mut layer, &hero, &a, 4);
        assert_eq!(layer.weights, [1.0, 1.0]);
    }

    #[test]
    fn through_the_flips_the_held_arms_go_to_their_rest_and_lift_the_gun_late() {
        let a = with_holds();
        let flip = Some(RigMove::Flip(FlipSide::Left));
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.gun = holding(Hold::TwoHands);
        let mut layer = HoldLayer::default();
        step_hold(&mut layer, &hero, &a, 4);
        layer.step(&hero, flip, false, Some(&a), TICK);
        assert!((layer.resting - 1.0 / 15.0).abs() < 1e-5);
        assert_eq!(layer.rest_ticks, 0.0, "its rest plays from the start");
        for _ in 0..20 {
            layer.step(&hero, flip, false, Some(&a), TICK);
        }
        assert_eq!(layer.resting, 1.0);
        assert!((layer.rest_ticks - 20.0).abs() < 1e-3);
        // Back over ten ticks.
        layer.step(&hero, Some(RigMove::Idle), false, Some(&a), TICK);
        assert!((layer.resting - 0.9).abs() < 1e-5);

        // The flip's frame passes 25 after 50 ticks: a frame a keyframe of 2 ticks.
        assert!(!layer.lifts(flip, 50.0, &a));
        assert!(layer.lifts(flip, 51.0, &a));
        assert!(!layer.lifts(Some(RigMove::Flip(FlipSide::Forward)), 51.0, &a));
        assert!(
            !HoldLayer::default().lifts(flip, 51.0, &a),
            "not without a gun held"
        );
    }

    #[test]
    fn the_third_fidget_waits_while_he_holds_a_gun_in_both_hands() {
        let mut a = animations();
        a.fidgets[0].two_hands = false;
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        hero.gun = holding(Hold::TwoHands);
        let mut director = Director::new(5);
        run(&mut director, &hero, &a, 300);
        assert_eq!(director.current, Some(RigMove::Idle));
        hero.gun = holding(Hold::OneHand);
        run(&mut director, &hero, &a, 1);
        assert_eq!(director.current, Some(RigMove::Fidget(0)));
    }
}
