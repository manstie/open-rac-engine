//! Animation clips a game can hand the rig in place of its procedural poses (P1.17's swap
//! point for real animation).
//!
//! The rig never reads a disc. A game that has animations (`openrac` reads the hero's from
//! the user's own UYA disc) converts them to the rig's joints and inserts
//! [`RigAnimations`]. Moves with a clip play it; the rest keep their procedural poses, and
//! so do the joints the clips don't drive (the open model's Clank, the wrench). Parts a
//! model carries on the hero's back play clips of their own ([`BackAnimations`]).

use std::collections::BTreeMap;

use bevy::prelude::*;
use rac_hero::{HeroMotion, HeroState, JumpKind, Side, StrafeSide};

/// A move the rig can play a clip for: roughly one per hero state, split where the games
/// animate a state differently (walking off and running, crouching still and moving).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RigMove {
    Idle,
    /// Standing at low health.
    IdleHurt,
    /// One of the idle's occasional fidgets, 0 to 3 (`hero-controller.md`, "Idle
    /// fidgets").
    Fidget(u8),
    /// Starting to walk.
    SetOff,
    Run,
    /// Strafing backwards: walking backwards.
    StrafeBack,
    Skid,
    /// The skid straight out of a run, or out of a long jump's landing.
    RunSkid,
    Crouch,
    CrouchMove,
    /// Stepping off a low edge: the fall before it becomes [`RigMove::Fall`].
    FallStart,
    Fall,
    /// Coming down on his feet, standing.
    Land(Landing),
    Jump,
    RunJump,
    LongJump,
    Flip(FlipSide),
    HeliJump,
    WallJump,
    WaterJump,
    LedgeJump,
    GrindJump,
    GrindSwitch,
    DoubleJump,
    Glide,
    /// A swing of the combo, 0 to 2.
    Combo(u8),
    JumpAttack,
    Throw,
    Hit,
    LedgeGrab,
    LedgeHang,
    ShimmyLeft,
    ShimmyRight,
    Grind,
    /// A swing of the wrench on a rail, 0 or 1.
    GrindAttack(u8),
    ZipLine,
    Swim,
    SwimIdle,
    SwimFast,
    SurfaceSwim,
    SurfaceIdle,
    Dead,
}

/// Which way a flip goes, relative to where the hero faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlipSide {
    Left,
    Right,
    Forward,
    Back,
}

/// What the hero lands from: each jump finishes its own animation on the ground, and a
/// fall has a landing of its own, deeper after a long one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Landing {
    Fall,
    /// A fall of 90 ticks or more.
    LongFall,
    Jump,
    RunJump,
    Flip(FlipSide),
    HeliJump,
    WallJump,
    WaterJump,
    LedgeJump,
    DoubleJump,
}

impl RigMove {
    /// The move for what the hero is doing. The rig refines it with what it remembers:
    /// fidgets, landings, the walk's speed bands ([`crate::RigPlugin`]).
    pub fn of(hero: &HeroMotion) -> Option<Self> {
        let horizontal = Vec2::new(hero.velocity.x, hero.velocity.z).length();
        Some(match hero.state {
            HeroState::Idle | HeroState::Look => RigMove::Idle,
            HeroState::Walk if hero.strafing_back => RigMove::StrafeBack,
            // UYA's walk sets off below 2.35 u/s (`hero-controller.md`).
            HeroState::Walk if horizontal < SET_OFF_BELOW => RigMove::SetOff,
            HeroState::Walk => RigMove::Run,
            HeroState::Skid => RigMove::Skid,
            HeroState::Crouch if horizontal > 0.5 => RigMove::CrouchMove,
            HeroState::Crouch => RigMove::Crouch,
            HeroState::Fall => RigMove::Fall,
            HeroState::Jump(kind) => match kind {
                JumpKind::Standing => RigMove::Jump,
                JumpKind::Run => RigMove::RunJump,
                JumpKind::Long => RigMove::LongJump,
                JumpKind::Flip => RigMove::Flip(flip_side(hero)),
                JumpKind::Heli => RigMove::HeliJump,
                JumpKind::Wall => RigMove::WallJump,
                JumpKind::Water => RigMove::WaterJump,
                JumpKind::Ledge => RigMove::LedgeJump,
                JumpKind::Grind => RigMove::GrindJump,
                JumpKind::GrindSwitch => RigMove::GrindSwitch,
            },
            HeroState::DoubleJump => RigMove::DoubleJump,
            HeroState::Glide => RigMove::Glide,
            HeroState::Combo(swing) => RigMove::Combo(swing),
            HeroState::JumpAttack => RigMove::JumpAttack,
            HeroState::Throw => RigMove::Throw,
            HeroState::Hit => RigMove::Hit,
            HeroState::LedgeGrab => RigMove::LedgeGrab,
            HeroState::LedgeHang => RigMove::LedgeHang,
            HeroState::LedgeShimmy(Side::Left) => RigMove::ShimmyLeft,
            HeroState::LedgeShimmy(Side::Right) => RigMove::ShimmyRight,
            HeroState::Grind => RigMove::Grind,
            HeroState::GrindAttack(swing) => RigMove::GrindAttack(swing),
            HeroState::ZipLine => RigMove::ZipLine,
            HeroState::UnderwaterSwim => RigMove::Swim,
            HeroState::UnderwaterIdle => RigMove::SwimIdle,
            HeroState::UnderwaterFast => RigMove::SwimFast,
            HeroState::SurfaceSwim => RigMove::SurfaceSwim,
            HeroState::SurfaceIdle => RigMove::SurfaceIdle,
            HeroState::Dead => RigMove::Dead,
            // DL's charge boots: no UYA animation.
            HeroState::Charge | HeroState::Bounce => return None,
        })
    }

    /// Attacks follow the controller's animation frame, which times their hits, instead of
    /// their own clock.
    pub fn follows_attack_frame(self) -> bool {
        matches!(
            self,
            RigMove::Combo(_) | RigMove::JumpAttack | RigMove::Throw | RigMove::GrindAttack(_)
        )
    }

    /// The landing that finishes this move when the hero comes down on his feet, if it has
    /// one. A long jump skids instead, and grind jumps come down on rails.
    pub fn landing(self) -> Option<Landing> {
        Some(match self {
            RigMove::Fall => Landing::Fall,
            RigMove::Jump => Landing::Jump,
            RigMove::RunJump => Landing::RunJump,
            RigMove::Flip(side) => Landing::Flip(side),
            RigMove::HeliJump => Landing::HeliJump,
            RigMove::WallJump => Landing::WallJump,
            RigMove::WaterJump => Landing::WaterJump,
            RigMove::LedgeJump => Landing::LedgeJump,
            RigMove::DoubleJump => Landing::DoubleJump,
            _ => return None,
        })
    }
}

/// UYA's walk changes from setting off to running above this speed, and back below
/// [`RUN_BELOW`] (u/s; `walk_substate_speeds`, 0x1DBE40).
pub const SET_OFF_BELOW: f32 = 2.35;
pub const RUN_BELOW: f32 = 1.9;

/// The flip's direction, as the controller chose it: the game plays sequence 28 plus that
/// side (`hero-controller.md`, "Animations per state"). A crouch flip from a run moves
/// forward as much as sideways, so the motion can't tell.
fn flip_side(hero: &HeroMotion) -> FlipSide {
    match hero.flip_side {
        StrafeSide::Left => FlipSide::Left,
        StrafeSide::Right => FlipSide::Right,
        StrafeSide::Forward => FlipSide::Forward,
        StrafeSide::Back => FlipSide::Back,
    }
}

/// One pose of a clip.
#[derive(Debug, Clone, PartialEq)]
pub struct RigKeyframe {
    /// Ticks (at 60 Hz) from this keyframe to the next, or 0 if the clip stops here.
    pub ticks: f32,
    /// The animation frame here: what the games time attack windows by.
    pub frame: f32,
    /// Each rig joint's rotation relative to its parent ([`crate::model::joint`] order).
    pub rotations: Vec<Quat>,
    /// Each rig joint's offset from where it sits at rest, in its parent's space.
    pub offsets: Vec<Vec3>,
    /// Each rig joint's scale along its own axes, which carries to its children.
    pub scales: Vec<Vec3>,
}

/// A clip: keyframes the rig interpolates between.
#[derive(Debug, Clone, PartialEq)]
pub struct RigClip {
    pub keyframes: Vec<RigKeyframe>,
    /// Whether it starts again at the end, from its first keyframe; otherwise it holds its
    /// last pose.
    pub looping: bool,
    /// The keyframe it starts at: the games start some moves part-way in.
    pub start: usize,
}

/// One sampled pose: rotations, offsets and scales per rig joint.
pub type Sample = (Vec<Quat>, Vec<Vec3>, Vec<Vec3>);

/// The clips a game gives the rig, and the game's rules for some of them. Joints not in
/// `driven` keep their procedural poses.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct RigAnimations {
    pub clips: BTreeMap<RigMove, RigClip>,
    /// Which rig joints the clips drive.
    pub driven: Vec<bool>,
    /// When each of the idle's fidgets plays, by [`RigMove::Fidget`] number.
    pub fidgets: Vec<FidgetRule>,
    /// Keyframes the walk moves on by when it changes from setting off to running, and
    /// from running to setting off, past the one at the same point of the stride.
    pub walk_phase: [usize; 2],
    /// The clips of the parts models carry on the hero's back, and their rules.
    pub back: Option<BackAnimations>,
    /// How each jump's air animation is timed to its arc.
    pub jumps: BTreeMap<RigMove, JumpTiming>,
}

/// How a jump's clip is timed to its arc, as UYA times its jumps (`hero-controller.md`,
/// "Landings"): sped up or slowed to show its peak frame at the apex and its landing
/// frame as he lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpTiming {
    /// Ticks of the jump's push-off: on the tick after, the clip is timed to the apex.
    pub pushoff: u32,
    /// The animation frames it shows at the apex and on landing.
    pub peak: f32,
    pub landing: f32,
    /// The keyframe it stops being timed at. Its landing plays from two keyframes on.
    pub landing_keyframe: usize,
    /// The frames the game counts to a keyframe when it times the clip: 2, or 1 for UYA's
    /// ledge jump.
    pub keyframe_frames: f32,
}

/// Clips for the parts a model carries on the hero's back ([`crate::AttachmentData`]),
/// and the game's rules for choosing them from the hero's move: UYA's heli-pack and Clank
/// (`moby-format.md`, "Clank and the heli-pack on the hero's back"). Every list of
/// sequences has one for each part, in the order of `parts`.
///
/// A part's clip gives each of its joints' whole transform from its parent: its
/// `rotations` and `scales` as they are, and its `offsets` as the translation from the
/// parent, not from rest.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BackAnimations {
    /// The attachments it animates, by name. The first leads: a row it plays already
    /// isn't started again, and a fidget it plays runs on into the idle.
    pub parts: Vec<String>,
    /// Each part's clips, by the game's sequence number.
    pub clips: Vec<BTreeMap<u8, RigClip>>,
    /// The sequences they rest in.
    pub rest: Vec<u8>,
    /// The sequences some of the hero's moves start them in.
    pub follow: Vec<BackRow>,
    /// The fidgets they play now and then while the hero stands idle.
    pub fidgets: Vec<Vec<u8>>,
    /// Moves during which one part holds a sequence of its own.
    pub holds: Vec<BackHold>,
}

/// The sequences the hero's move starts the back's parts in.
#[derive(Debug, Clone, PartialEq)]
pub struct BackRow {
    pub movement: RigMove,
    pub sequences: Vec<u8>,
    /// Ticks they blend in over: the move's own clip's.
    pub blend: f32,
}

/// A sequence one part of the back holds while the hero's move lasts: UYA's heli-pack
/// spins its rotors while he glides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackHold {
    pub movement: RigMove,
    pub part: usize,
    pub sequence: u8,
    /// Ticks it blends in over.
    pub blend: f32,
}

/// `t` of the way from pose `a` to pose `b`.
pub(crate) fn mix(a: &Sample, b: &Sample, t: f32) -> Sample {
    let rotations = a.0.iter().zip(&b.0).map(|(x, y)| x.slerp(*y, t)).collect();
    let offsets = a.1.iter().zip(&b.1).map(|(x, y)| x.lerp(*y, t)).collect();
    let scales = a.2.iter().zip(&b.2).map(|(x, y)| x.lerp(*y, t)).collect();
    (rotations, offsets, scales)
}

/// When one of the idle's fidgets plays. Standing idle, each fidget whose cooldown is over
/// plays with a chance each tick of one in `interval` seconds' worth of ticks, and none
/// plays for a short gap after another (UYA's own rules, `hero-controller.md`, "Idle
/// fidgets").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FidgetRule {
    /// Seconds between plays, on average.
    pub interval: f32,
    /// Seconds after it plays before it may play again.
    pub cooldown: f32,
    /// Plays only at low health, now and then at the end of the hurt idle's loop,
    /// instead of at random.
    pub low_health: bool,
}

impl RigClip {
    /// Ticks from the first keyframe to the one it starts at.
    fn lead(&self) -> f32 {
        self.keyframes
            .iter()
            .take(self.start)
            .map(|k| k.ticks.max(0.0))
            .sum()
    }

    /// Ticks once round a looping clip, or from start to end of one that holds.
    fn total(&self) -> f32 {
        let count = self.keyframes.len();
        let played = if self.looping {
            count
        } else {
            count.saturating_sub(1)
        };
        self.keyframes
            .iter()
            .take(played)
            .map(|k| k.ticks.max(0.0))
            .sum()
    }

    /// How long it plays from where it starts until it holds its last pose; infinite for
    /// a looping clip.
    pub fn length(&self) -> f32 {
        if self.looping {
            f32::INFINITY
        } else {
            (self.total() - self.lead()).max(0.0)
        }
    }

    /// Ticks once round a looping clip, from its first keyframe back to it.
    pub fn cycle(&self) -> f32 {
        if self.looping { self.total() } else { 0.0 }
    }

    /// Whether `ticks` from its start it has played to its end. A looping clip has played
    /// through once when it comes back round to where it started.
    pub fn finished(&self, ticks: f32) -> bool {
        if self.looping {
            ticks >= self.total() && self.total() > 0.0
        } else {
            ticks >= self.length()
        }
    }

    /// The keyframe it is in, `ticks` from its start.
    pub fn keyframe_at(&self, ticks: f32) -> usize {
        let mut time = self.lead() + ticks.max(0.0);
        let total = self.total();
        if self.looping && total > 0.0 {
            time %= total;
        }
        for (index, keyframe) in self.keyframes.iter().enumerate() {
            if time < keyframe.ticks {
                return index;
            }
            time -= keyframe.ticks.max(0.0);
        }
        self.keyframes.len().saturating_sub(1)
    }

    /// The animation frame `ticks` from its start, between keyframes' time stamps.
    pub fn frame_at(&self, ticks: f32) -> f32 {
        let index = self.keyframe_at(ticks);
        let Some(keyframe) = self.keyframes.get(index) else {
            return 0.0;
        };
        let into = self.ticks_from(index, ticks);
        match self.keyframes.get(index + 1) {
            Some(next) if keyframe.ticks > 0.0 => {
                keyframe.frame + (next.frame - keyframe.frame) * (into / keyframe.ticks).min(1.0)
            }
            _ => keyframe.frame,
        }
    }

    /// How far into keyframe `index` the clip is, `ticks` from its start.
    fn ticks_from(&self, index: usize, ticks: f32) -> f32 {
        let total = self.total();
        let mut time = self.lead() + ticks.max(0.0);
        if self.looping && total > 0.0 {
            time %= total;
        }
        let before: f32 = self
            .keyframes
            .iter()
            .take(index)
            .map(|k| k.ticks.max(0.0))
            .sum();
        (time - before).max(0.0)
    }

    /// Ticks from its start to keyframe `index`, coming round the end of a looping clip.
    pub fn ticks_to(&self, index: usize) -> f32 {
        let at: f32 = self
            .keyframes
            .iter()
            .take(index)
            .map(|k| k.ticks.max(0.0))
            .sum();
        let lead = self.lead();
        if at >= lead {
            at - lead
        } else if self.looping {
            self.total() - lead + at
        } else {
            0.0
        }
    }

    /// The pose `ticks` into the clip, from where it starts.
    pub fn at_time(&self, ticks: f32) -> Option<Sample> {
        let total = self.total();
        let count = self.keyframes.len();
        if count == 0 {
            return None;
        }
        let mut time = self.lead() + ticks.max(0.0);
        if self.looping && total > 0.0 {
            time %= total;
        }
        for (index, keyframe) in self.keyframes.iter().enumerate() {
            let next = if index + 1 < count {
                index + 1
            } else if self.looping {
                0
            } else {
                break;
            };
            if keyframe.ticks <= 0.0 {
                break;
            }
            if time < keyframe.ticks {
                return Some(self.blend(index, next, time / keyframe.ticks));
            }
            time -= keyframe.ticks;
        }
        let last = self
            .keyframes
            .iter()
            .position(|k| k.ticks <= 0.0)
            .unwrap_or(count - 1);
        Some(self.blend(last, last, 0.0))
    }

    /// The pose at animation frame `frame`, held at the clip's ends.
    pub fn at_frame(&self, frame: f32) -> Option<Sample> {
        let first = self.keyframes.first()?;
        if frame <= first.frame {
            return Some(self.blend(0, 0, 0.0));
        }
        for index in 0..self.keyframes.len() - 1 {
            let (a, b) = (&self.keyframes[index], &self.keyframes[index + 1]);
            if frame < b.frame {
                let span = (b.frame - a.frame).max(1e-6);
                return Some(self.blend(index, index + 1, (frame - a.frame) / span));
            }
        }
        let last = self.keyframes.len() - 1;
        Some(self.blend(last, last, 0.0))
    }

    fn blend(&self, from: usize, to: usize, t: f32) -> Sample {
        let (a, b) = (&self.keyframes[from], &self.keyframes[to]);
        let rotations = a
            .rotations
            .iter()
            .zip(&b.rotations)
            .map(|(x, y)| x.slerp(*y, t))
            .collect();
        let offsets = a
            .offsets
            .iter()
            .zip(&b.offsets)
            .map(|(x, y)| x.lerp(*y, t))
            .collect();
        let scales = a
            .scales
            .iter()
            .zip(&b.scales)
            .map(|(x, y)| x.lerp(*y, t))
            .collect();
        (rotations, offsets, scales)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyframe(ticks: f32, frame: f32, angle: f32) -> RigKeyframe {
        RigKeyframe {
            ticks,
            frame,
            rotations: vec![Quat::from_rotation_x(angle)],
            offsets: vec![Vec3::Y * angle],
            scales: vec![Vec3::ONE + Vec3::X * angle],
        }
    }

    fn angle(pose: Sample) -> f32 {
        pose.1[0].y
    }

    fn clip(looping: bool, start: usize) -> RigClip {
        RigClip {
            keyframes: vec![
                keyframe(2.0, 0.0, 0.0),
                keyframe(2.0, 1.0, 1.0),
                keyframe(4.0, 2.0, 2.0),
            ],
            looping,
            start,
        }
    }

    #[test]
    fn looping_clips_wrap_through_their_first_keyframe() {
        let clip = RigClip {
            keyframes: vec![keyframe(2.0, 0.0, 0.0), keyframe(2.0, 1.0, 1.0)],
            looping: true,
            start: 0,
        };
        assert!((angle(clip.at_time(1.0).unwrap()) - 0.5).abs() < 1e-5);
        assert!(
            (angle(clip.at_time(3.0).unwrap()) - 0.5).abs() < 1e-5,
            "back towards 0"
        );
        assert!(
            (angle(clip.at_time(5.0).unwrap()) - 0.5).abs() < 1e-5,
            "wrapped"
        );
        let scale = clip.at_time(1.0).unwrap().2[0];
        assert!((scale.x - 1.5).abs() < 1e-5, "scales blend too: {scale}");
    }

    #[test]
    fn other_clips_hold_their_last_pose() {
        let clip = RigClip {
            keyframes: vec![keyframe(2.0, 0.0, 0.0), keyframe(2.0, 1.0, 1.0)],
            looping: false,
            start: 0,
        };
        assert!((angle(clip.at_time(100.0).unwrap()) - 1.0).abs() < 1e-5);
        assert_eq!(clip.length(), 2.0, "the last keyframe's ticks lead nowhere");
        assert!(!clip.finished(1.9) && clip.finished(2.0));
    }

    #[test]
    fn clips_can_start_part_way_in_and_loop_from_the_first_keyframe() {
        let looping = clip(true, 1);
        assert!((angle(looping.at_time(0.0).unwrap()) - 1.0).abs() < 1e-5);
        assert!((angle(looping.at_time(2.0).unwrap()) - 2.0).abs() < 1e-5);
        // Half way from the last keyframe back round to the first.
        assert!((angle(looping.at_time(4.0).unwrap()) - 1.0).abs() < 1e-5);
        // 2 + 4 ticks later it has come round to its first keyframe, then on to the start.
        assert!(angle(looping.at_time(6.0).unwrap()).abs() < 1e-5);
        assert_eq!(looping.keyframe_at(0.0), 1);
        assert_eq!(looping.keyframe_at(7.0), 0);
        assert_eq!(looping.ticks_to(2), 2.0);
        assert_eq!(looping.ticks_to(0), 6.0, "round the end");
        assert!(!looping.finished(7.9) && looping.finished(8.0));

        let held = clip(false, 1);
        assert_eq!(held.length(), 2.0);
        assert!((angle(held.at_time(50.0).unwrap()) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn attacks_are_sampled_by_animation_frame() {
        let clip = RigClip {
            keyframes: vec![keyframe(2.0, 16.0, 0.0), keyframe(2.0, 18.0, 1.0)],
            looping: false,
            start: 0,
        };
        assert_eq!(angle(clip.at_frame(10.0).unwrap()), 0.0);
        assert!((angle(clip.at_frame(17.0).unwrap()) - 0.5).abs() < 1e-5);
        assert_eq!(angle(clip.at_frame(30.0).unwrap()), 1.0);
    }

    #[test]
    fn states_pick_their_moves() {
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        assert_eq!(RigMove::of(&hero), Some(RigMove::Idle));
        hero.state = HeroState::Walk;
        hero.velocity = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(RigMove::of(&hero), Some(RigMove::SetOff));
        hero.velocity = Vec3::new(0.0, 0.0, -6.0);
        assert_eq!(RigMove::of(&hero), Some(RigMove::Run));
        hero.state = HeroState::Jump(JumpKind::Flip);
        hero.flip_side = StrafeSide::Back;
        assert_eq!(RigMove::of(&hero), Some(RigMove::Flip(FlipSide::Back)));
        hero.flip_side = StrafeSide::Left;
        assert_eq!(RigMove::of(&hero), Some(RigMove::Flip(FlipSide::Left)));
        assert_eq!(
            RigMove::of(&hero).unwrap().landing(),
            Some(Landing::Flip(FlipSide::Left))
        );
        hero.state = HeroState::Combo(2);
        assert!(RigMove::of(&hero).unwrap().follows_attack_frame());
        hero.state = HeroState::Walk;
        hero.strafing = true;
        hero.strafing_back = true;
        assert_eq!(RigMove::of(&hero), Some(RigMove::StrafeBack));
        assert_eq!(RigMove::LongJump.landing(), None, "a long jump skids");
    }
}
