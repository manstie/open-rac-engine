//! Locking on to a target, after UYA (`docs/research/hero-controller.md`, "Lock-on"):
//! with both strafe buttons held, a target near the middle of the view is held until a
//! button is let go or it gets too far away. The hero strafes meanwhile, so he faces the
//! camera, and the [`crate::FollowCamera`] turns to look past him at the target.
//!
//! Only the gamepad locks on: `rac_input` ignores keyboard and mouse bindings for
//! [`Action::LockOn`], because a player with a mouse aims for themselves.

use bevy::prelude::*;
use rac_input::{Action, PlayerInput};

use crate::{
    collision::CollisionWorld,
    controller::{HeroMotion, HeroState, target_score},
    profile::LockOnProfile,
};

/// What a hero is locked on to.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LockOn {
    pub target: Option<Entity>,
    /// The target's middle this tick.
    pub at: Vec3,
    /// Whether he may lock on. UYA allows it only while the gun's lock-on mod is on
    /// (`hero-controller.md`, "Lock-on"), which the game's weapons code sets; without one,
    /// it is always allowed.
    pub allowed: bool,
}

impl Default for LockOn {
    fn default() -> Self {
        Self {
            target: None,
            at: Vec3::ZERO,
            allowed: true,
        }
    }
}

impl LockOn {
    /// Where the target is, while locked on.
    pub fn target_point(&self) -> Option<Vec3> {
        self.target.map(|_| self.at)
    }
}

/// One tick of lock-on, as UYA's 0x503428 runs it: a new press of the pair picks a target,
/// and letting either go, the target going away or getting too far drops it. `targets` are
/// the [`crate::AimTarget`]s' middles, in a fixed order.
pub fn update(
    lock: &mut LockOn,
    hero: &HeroMotion,
    input: &PlayerInput,
    camera_yaw: f32,
    targets: &[(Entity, Vec3)],
    profile: &LockOnProfile,
    world: &dyn CollisionWorld,
) {
    if !profile.enabled
        || !lock.allowed
        || !input.held(Action::LockOn)
        || hero.state == HeroState::Dead
    {
        lock.target = None;
        return;
    }
    if let Some(target) = lock.target {
        let kept = targets
            .iter()
            .find(|(entity, _)| *entity == target)
            .filter(|(_, at)| at.distance(hero.position) <= profile.keep_range);
        match kept {
            Some(&(_, at)) => lock.at = at,
            None => lock.target = None,
        }
        // A target lost isn't replaced until the pair is pressed again.
        return;
    }
    if !input.pressed(Action::LockOn) {
        return;
    }
    // The middle of the view first, then whatever he faces.
    let picked = pick(hero, camera_yaw, targets, profile, world)
        .or_else(|| pick(hero, hero.yaw, targets, profile, world));
    if let Some((entity, at)) = picked {
        lock.target = Some(entity);
        lock.at = at;
    }
}

/// The best-scored target around `yaw` that he can see.
fn pick(
    hero: &HeroMotion,
    yaw: f32,
    targets: &[(Entity, Vec3)],
    profile: &LockOnProfile,
    world: &dyn CollisionWorld,
) -> Option<(Entity, Vec3)> {
    let eyes = hero.position + Vec3::Y * profile.sight_height;
    let mut best: Option<(f32, (Entity, Vec3))> = None;
    for &(entity, at) in targets {
        let Some(score) = target_score(at - hero.position, yaw, &profile.aim) else {
            continue;
        };
        if best.is_some_and(|(least, _)| score >= least) {
            continue;
        }
        let sight = at - eyes;
        let length = sight.length();
        if length > profile.sight_short {
            let reach = sight * ((length - profile.sight_short) / length);
            if world.cast_ray(eyes, reach).is_some() {
                continue;
            }
        }
        best = Some((score, (entity, at)));
    }
    best.map(|(_, target)| target)
}
