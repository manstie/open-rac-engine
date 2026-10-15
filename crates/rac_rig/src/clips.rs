//! Animation clips a game can hand the rig in place of its procedural poses (P1.17's swap
//! point for real animation).
//!
//! The rig never reads a disc. A game that has animations (`openrac` reads the hero's from
//! the user's own UYA disc) converts them to the rig's joints and inserts
//! [`RigAnimations`]. Moves with a clip play it; the rest keep their procedural poses, and
//! so do the joints the clips don't drive (Clank's, the wrench's).

use std::collections::BTreeMap;

use bevy::prelude::*;
use rac_hero::{HeroMotion, HeroState, JumpKind, Side, StrafeSide};

/// A move the rig can play a clip for: roughly one per hero state, split where the games
/// animate a state differently (walking off and running, crouching still and moving).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RigMove {
    Idle,
    /// Starting to walk.
    SetOff,
    Run,
    /// Strafing backwards: walking backwards.
    StrafeBack,
    Skid,
    Crouch,
    CrouchMove,
    Fall,
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

impl RigMove {
    /// The move for what the hero is doing.
    pub fn of(hero: &HeroMotion) -> Option<Self> {
        let horizontal = Vec2::new(hero.velocity.x, hero.velocity.z).length();
        Some(match hero.state {
            HeroState::Idle => RigMove::Idle,
            HeroState::Walk if hero.strafing_back => RigMove::StrafeBack,
            // UYA's walk sets off below 2.35 u/s (`hero-controller.md`).
            HeroState::Walk if horizontal < 2.35 => RigMove::SetOff,
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
            RigMove::Combo(_) | RigMove::JumpAttack | RigMove::Throw
        )
    }
}

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
}

/// A clip: keyframes the rig interpolates between.
#[derive(Debug, Clone, PartialEq)]
pub struct RigClip {
    pub keyframes: Vec<RigKeyframe>,
    /// Whether it starts again at the end; otherwise it holds its last pose.
    pub looping: bool,
}

/// The clips a game gives the rig. Joints not in `driven` keep their procedural poses.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct RigAnimations {
    pub clips: BTreeMap<RigMove, RigClip>,
    /// Which rig joints the clips drive.
    pub driven: Vec<bool>,
}

impl RigClip {
    /// The pose `ticks` into the clip.
    pub fn at_time(&self, ticks: f32) -> Option<(Vec<Quat>, Vec<Vec3>)> {
        let total: f32 = self.keyframes.iter().map(|k| k.ticks).sum();
        let count = self.keyframes.len();
        if count == 0 {
            return None;
        }
        let mut time = ticks.max(0.0);
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
    pub fn at_frame(&self, frame: f32) -> Option<(Vec<Quat>, Vec<Vec3>)> {
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

    fn blend(&self, from: usize, to: usize, t: f32) -> (Vec<Quat>, Vec<Vec3>) {
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
        (rotations, offsets)
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
        }
    }

    fn angle(pose: (Vec<Quat>, Vec<Vec3>)) -> f32 {
        pose.1[0].y
    }

    #[test]
    fn looping_clips_wrap_through_their_first_keyframe() {
        let clip = RigClip {
            keyframes: vec![keyframe(2.0, 0.0, 0.0), keyframe(2.0, 1.0, 1.0)],
            looping: true,
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
    }

    #[test]
    fn other_clips_hold_their_last_pose() {
        let clip = RigClip {
            keyframes: vec![keyframe(2.0, 0.0, 0.0), keyframe(2.0, 1.0, 1.0)],
            looping: false,
        };
        assert!((angle(clip.at_time(100.0).unwrap()) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn attacks_are_sampled_by_animation_frame() {
        let clip = RigClip {
            keyframes: vec![keyframe(2.0, 16.0, 0.0), keyframe(2.0, 18.0, 1.0)],
            looping: false,
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
        hero.state = HeroState::Combo(2);
        assert!(RigMove::of(&hero).unwrap().follows_attack_frame());
        hero.state = HeroState::Walk;
        hero.strafing = true;
        hero.strafing_back = true;
        assert_eq!(RigMove::of(&hero), Some(RigMove::StrafeBack));
    }
}
