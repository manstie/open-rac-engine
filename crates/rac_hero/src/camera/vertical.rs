//! The follow point's height, after UYA's `camera_vertical_modes` (0x3E2AE8) and
//! `camera_follow_vertical` (0x3E3130): `camera.md`, "The follow point's height", whose
//! table this follows. The kit leaves out the modes for states it doesn't have: jets,
//! rocket jumps, jump pads, state 12 (mode 8) and the bounce's mode 11, which only a jump
//! attack or a reset would leave.

use super::follow::Rig;
use super::view_angles;
use crate::{
    collision::{CapsuleShape, CollisionWorld},
    controller::{HeroMotion, HeroState, JumpKind},
    profile::{HeroProfile, Rise},
};
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// The ground below an airborne hero is looked for this far down.
const GROUND_BELOW: f32 = 30.0;
/// Strafing with the look stick pushed down past this, a flip doesn't rise.
const FLIP_STICK: f32 = 0.3;

/// The vertical mode and what it keeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Vertical {
    /// Modes 0, 1 and 4: his height exactly. 1 follows a fall, 4 the end of a timed rise.
    Follow,
    Falling,
    AfterRise,
    /// Mode 2: the height held through a jump.
    Hold(f32),
    /// Modes 3, 5, 6 and 7: a timed rise from `start` to `target`, `tick` of `ticks` done.
    Rise {
        kind: RiseKind,
        start: f32,
        target: f32,
        tick: u32,
        ticks: u32,
    },
    /// Modes 9 and 10: halfway between his height and the height when the jump attack
    /// started; `air` counts ticks off the ground, `after` the ticks since the attack.
    Attack {
        start: f32,
        air: u32,
        after: Option<u32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RiseKind {
    Heli,
    DoubleJump,
    /// A flip's rise, and its way back down to where it started.
    FlipUp,
    FlipDown,
}

impl Vertical {
    fn rise(kind: RiseKind, start: f32, target: f32, rise: Rise) -> Self {
        Self::Rise {
            kind,
            start,
            target,
            tick: 0,
            ticks: rise.ticks.max(1),
        }
    }
}

/// Changes the mode for this tick and returns the follow point's height.
pub(super) fn step(
    rig: &mut Rig,
    hero: &HeroMotion,
    stick: Vec2,
    profile: &HeroProfile,
    world: &dyn CollisionWorld,
) -> f32 {
    let v = &profile.camera.vertical;
    let state = hero.state;
    let jumping = matches!(state, HeroState::Jump(_) | HeroState::DoubleJump);
    let falling = state == HeroState::Fall;
    let flipping = state == HeroState::Jump(JumpKind::Flip);
    let ground_below = hero.position.y - hero.height_above_ground.min(GROUND_BELOW);
    let current = rig.follow.y;

    rig.vertical = match rig.vertical {
        Vertical::Falling | Vertical::Attack { after: None, .. } => rig.vertical,
        _ if state == HeroState::JumpAttack => Vertical::Attack {
            start: current,
            air: 0,
            after: None,
        },
        mode => mode,
    };
    rig.vertical = match rig.vertical {
        Vertical::Follow if falling => Vertical::Falling,
        Vertical::Follow if state == HeroState::Jump(JumpKind::Heli) => Vertical::rise(
            RiseKind::Heli,
            current,
            ground_below + v.heli_rise.height,
            v.heli_rise,
        ),
        Vertical::Follow if flipping && !(hero.strafing && -stick.y > FLIP_STICK) => {
            Vertical::rise(
                RiseKind::FlipUp,
                current,
                hero.position.y + v.flip_rise.height,
                v.flip_rise,
            )
        }
        Vertical::Follow if state == HeroState::DoubleJump => Vertical::rise(
            RiseKind::DoubleJump,
            current,
            ground_below + v.double_jump_rise.height,
            v.double_jump_rise,
        ),
        Vertical::Follow if jumping => Vertical::Hold(current),
        Vertical::Falling | Vertical::Hold(_) if !jumping && !falling => Vertical::Follow,
        Vertical::Hold(_) if state == HeroState::Jump(JumpKind::Wall) => Vertical::Falling,
        Vertical::Rise {
            kind: RiseKind::Heli,
            ..
        } if flipping => Vertical::rise(
            RiseKind::FlipUp,
            current,
            hero.position.y + v.flip_rise.height,
            v.flip_rise,
        ),
        Vertical::Rise {
            kind: RiseKind::Heli | RiseKind::DoubleJump,
            ..
        } if !jumping => Vertical::Follow,
        Vertical::Rise {
            kind: RiseKind::FlipUp,
            ..
        } if !flipping => Vertical::Follow,
        Vertical::Rise {
            kind: RiseKind::FlipDown,
            ..
        } if falling && hero.past_apex() => Vertical::Falling,
        Vertical::Rise {
            kind: RiseKind::FlipDown,
            ..
        } if flipping && !hero.past_apex() => Vertical::rise(
            RiseKind::FlipUp,
            current,
            ground_below + v.flip_rise.height,
            v.flip_rise,
        ),
        Vertical::AfterRise if !jumping => {
            if falling {
                Vertical::Falling
            } else {
                Vertical::Follow
            }
        }
        Vertical::Attack { air, .. } if falling || air > v.attack_air_ticks => Vertical::Falling,
        Vertical::Attack {
            start,
            air,
            after: None,
        } if state != HeroState::JumpAttack => Vertical::Attack {
            start,
            air,
            after: Some(0),
        },
        Vertical::Attack {
            after: Some(after), ..
        } if after >= v.attack_after_ticks => Vertical::Follow,
        mode => mode,
    };

    match &mut rig.vertical {
        Vertical::Follow | Vertical::Falling | Vertical::AfterRise => hero.position.y,
        Vertical::Hold(held) => {
            let fall_gravity = profile.fall.gravity;
            let landing = hero.ground.is_some()
                || (hero.past_apex() && hero.ticks_to_land(fall_gravity) < v.land_ticks);
            if landing && ground_below >= *held {
                *held = ground_below;
            }
            let held = *held;
            if hero.past_apex()
                && hero.height_above_ground > v.follow_down_above
                && head_below_view(rig, hero, profile)
            {
                rig.vertical = Vertical::Falling;
                return hero.position.y;
            }
            held
        }
        Vertical::Rise {
            kind,
            start,
            target,
            tick,
            ticks,
        } => {
            // A sphere at the camera that touches anything stops the rise where it is.
            let ball = CapsuleShape {
                half_height: 0.0,
                radius: v.rise_stop_radius,
            };
            if *tick < *ticks && world.capsule_penetrations(ball, rig.camera).is_empty() {
                *tick += 1;
            }
            let height = *start + (*target - *start) * *tick as f32 / *ticks as f32;
            if *tick >= *ticks {
                rig.vertical = match *kind {
                    RiseKind::FlipUp => {
                        Vertical::rise(RiseKind::FlipDown, *target, *start, v.flip_rise)
                    }
                    _ => Vertical::AfterRise,
                };
            }
            height
        }
        Vertical::Attack { start, air, after } => {
            if hero.ground.is_none() {
                *air += 1;
            }
            if let Some(after) = after {
                *after += 1;
            }
            (hero.position.y + *start) / 2.0
        }
    }
}

/// Whether his head has dropped under last tick's view (0x3E2638): by more than the
/// profile's angle below it, in the camera's vertical plane, or past straight up.
fn head_below_view(rig: &Rig, hero: &HeroMotion, profile: &HeroProfile) -> bool {
    let camera = &profile.camera;
    let head = hero.position + Vec3::Y * camera.look_height;
    let (_, to_head) = view_angles(head - rig.camera);
    let angle = to_head - rig.view.1;
    angle < camera.vertical.follow_down_angle || angle > FRAC_PI_2
}
