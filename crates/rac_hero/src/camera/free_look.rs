//! The free-look camera behind the lock-strafe and first-person views and the look, after
//! UYA's 0x410B08 and Deadlocked's 0x5620A0 (`camera.md`, "The free-look camera", "The eye
//! point", "Lock-strafe, DL's default").
//!
//! Only the player turns it. The stick sets a turn speed, which speeds up towards its
//! target and drops at once to a smaller one, so the view stops dead when the stick is let
//! go; the mouse turns it directly. The camera sits at the hero's eye in first person, or
//! `offset` behind it along the view in lock-strafe, both from last tick's eye, as in the
//! games, so it is rigid behind him with no lag or drag.
//!
//! With UYA's aim assist (`FreeLookAssist`; `camera.md`, "The free-look camera", items 2
//! and 4 to 6), a target near the middle of the view slows the stick's turn, delays and
//! softens the quick turn, catches a hard push towards it for a few ticks, and is kept in
//! view while he moves. It is the stick's: a player turning with the mouse aims for
//! himself, as with lock-on (PLAN.md §5, 2026-10-22).

use super::{approach, ease_to, look_rotation, view_angles, view_direction, wrap};
use crate::{
    collision::CollisionWorld,
    controller::{HeroMotion, HeroState, JumpKind},
    profile::{FreeLookAssist, FreeLookProfile},
};
use bevy::{math::ops, prelude::*};
use std::f32::consts::FRAC_PI_2;

/// The games' camera mode for the hero (`hero_camera_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EyeMode {
    /// No free-look view is shown; the eye rests at the lock-strafe height.
    Off,
    LockStrafe,
    FirstPerson,
}

/// What the aim assist sees this tick.
pub(super) struct Aim<'a> {
    /// The targets' middles, in a fixed order.
    pub targets: &'a [(Entity, Vec3)],
    /// His velocity (u/s) and the tick's length, for keeping a target in view.
    pub velocity: Vec3,
    pub tick: f32,
    /// Either stick is pushed.
    pub pushed: bool,
    pub world: &'a dyn CollisionWorld,
}

/// The aim assist's target.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Aimed {
    entity: Entity,
    at: Vec3,
    /// How far it moved since last tick, while it was the target then too.
    moved: Vec3,
    /// 1 dead ahead or under the view's ray, 0 at the cone's edge.
    weight: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct FreeLook {
    pub mode: EyeMode,
    /// Radians: yaw about +y (0 looks down −z), pitch up from level.
    pub yaw: f32,
    pub pitch: f32,
    /// Radians a tick; positive turns right and looks up.
    yaw_speed: f32,
    pitch_speed: f32,
    /// Ticks running with the stick past the quick turn's threshold, and whether the
    /// quick turn is on.
    quick_ticks: u32,
    quick: bool,
    /// The aim assist's target, and the push towards it held for `snap_ticks` more.
    target: Option<Aimed>,
    snap_ticks: u32,
    snap_input: f32,
    /// The mouse turned the view last, rather than the stick: no aim assist.
    mouse_aims: bool,
    /// The weight of what the assist would pick, kept while the mouse aims too.
    weight: f32,
    /// The camera's field of view over the profile's: a zoom slows the turn with it.
    pub zoom: f32,
    /// Where the camera was last placed, which the assist looks from.
    camera: Vec3,
    /// Radians the view may look down and up.
    down_limit: f32,
    up_limit: f32,
    /// The eye's height above his feet and its speed (u a tick), last tick's eye point,
    /// and his height then.
    eye_height: f32,
    eye_speed: f32,
    eye: Vec3,
    feet_y: f32,
    /// A lock-strafe jump has risen past the hold, which lets go for the rest of it.
    jump_latched: bool,
    /// The lock-strafe offset's length and speed.
    offset: f32,
    offset_speed: f32,
}

impl FreeLook {
    pub fn new(hero: &HeroMotion, yaw: f32, profile: &FreeLookProfile) -> Self {
        let mut free = Self {
            mode: EyeMode::Off,
            yaw,
            pitch: 0.0,
            yaw_speed: 0.0,
            pitch_speed: 0.0,
            quick_ticks: 0,
            quick: false,
            target: None,
            snap_ticks: 0,
            snap_input: 0.0,
            mouse_aims: false,
            weight: 0.0,
            zoom: 1.0,
            camera: hero.position + Vec3::Y * profile.eye_lock_strafe,
            down_limit: 0.0,
            up_limit: 0.0,
            eye_height: profile.eye_lock_strafe,
            eye_speed: 0.0,
            eye: hero.position + Vec3::Y * profile.eye_lock_strafe,
            feet_y: hero.position.y,
            jump_latched: false,
            offset: profile.offset,
            offset_speed: 0.0,
        };
        free.set_limits(profile);
        free
    }

    /// Starts looking along `yaw` and `pitch`, at rest.
    pub fn look_along(&mut self, yaw: f32, pitch: f32, profile: &FreeLookProfile) {
        self.yaw = yaw;
        self.pitch = pitch;
        self.yaw_speed = 0.0;
        self.pitch_speed = 0.0;
        self.quick_ticks = 0;
        self.quick = false;
        self.snap_ticks = 0;
        self.pitch = self.pitch.clamp(-self.down_limit, self.up_limit);
        self.set_limits(profile);
    }

    /// The games' `hero_camera_set` (UYA 0x510530): switches to `mode`, with its limits and
    /// its eye height.
    pub fn set_mode(&mut self, mode: EyeMode, profile: &FreeLookProfile) {
        self.mode = mode;
        self.set_limits(profile);
        if mode != EyeMode::Off {
            self.eye_height = profile
                .eye_on_switch
                .unwrap_or_else(|| self.eye_target(mode, profile));
            self.eye_speed = 0.0;
        }
    }

    /// Sets the limits of the current mode afresh.
    pub fn set_limits(&mut self, profile: &FreeLookProfile) {
        let limits = match self.mode {
            EyeMode::LockStrafe => profile.lock_strafe_limits,
            EyeMode::Off | EyeMode::FirstPerson => profile.first_person_limits,
        };
        self.down_limit = limits.down_degrees.to_radians();
        self.up_limit = limits.up_degrees.to_radians();
        self.pitch = self.pitch.clamp(-self.down_limit, self.up_limit);
    }

    fn eye_target(&self, mode: EyeMode, profile: &FreeLookProfile) -> f32 {
        match mode {
            EyeMode::FirstPerson => profile.eye_first_person,
            EyeMode::Off | EyeMode::LockStrafe => profile.eye_lock_strafe,
        }
    }

    /// One tick of turning. `stick` is the look stick after the pad's dead zone (x right,
    /// y up), `mouse` the mouse's turn in radians, `speed` the camera-speed option, `aim`
    /// what the aim assist sees, if the game has one. Its rates are the profile's × the
    /// field of view over the profile's ([`Self::zoom`], as UYA's 0x410B08), and so is the
    /// mouse's turn [our own, for aiming zoomed with a mouse].
    pub fn turn(
        &mut self,
        stick: Vec2,
        mouse: Vec2,
        speed: f32,
        profile: &FreeLookProfile,
        aim: Option<&Aim>,
    ) {
        if self.mode != EyeMode::Off {
            let rest = profile.up_limit_rest_degrees.to_radians();
            self.up_limit = approach(
                self.up_limit,
                rest,
                profile.up_limit_ease_degrees.to_radians(),
            );
        }
        let mouse = mouse * self.zoom;
        if mouse != Vec2::ZERO {
            self.mouse_aims = true;
        } else if stick != Vec2::ZERO {
            self.mouse_aims = false;
        }
        let picked = match (profile.assist.as_ref(), aim) {
            (Some(assist), Some(aim)) => self.pick(assist, aim),
            _ => None,
        };
        // The crosshairs turn red by the weight even while the mouse aims.
        self.weight = picked.map_or(0.0, |picked| picked.weight);
        self.target = picked.filter(|_| !self.mouse_aims);
        let assist = profile.assist.as_ref().filter(|_| !self.mouse_aims);
        let b = self.target.map_or(0.0, |target| target.weight);
        let k = (0.25 + speed / 64.0) * self.zoom;
        let shape = |axis: f32| {
            if profile.squared {
                axis * axis.abs()
            } else {
                axis
            }
        };
        let (pushed, y) = (shape(stick.x), shape(stick.y));
        self.snap_ticks = self.snap_ticks.saturating_sub(1);

        // A hard push towards the target is held a few ticks (the snap).
        let mut x = pushed;
        if self.snap_ticks > 0 && x.abs() < self.snap_input.abs() {
            if self.towards(self.yaw_speed) {
                x = self.snap_input;
            } else {
                self.snap_ticks = 0;
            }
        }
        let quick_after = match (assist, self.target) {
            (Some(assist), Some(_)) => assist.quick_after_ticks,
            _ => profile.quick_after_ticks.max(1),
        };
        if x.abs() > profile.quick_stick {
            self.quick_ticks += 1;
            self.quick |= self.quick_ticks >= quick_after;
        } else {
            self.quick_ticks = 0;
            self.quick = false;
        }
        if let Some(assist) = assist {
            if pushed.abs() >= assist.snap_stick {
                if self.towards(x) {
                    self.snap_input = x;
                    self.snap_ticks = assist.snap_ticks;
                }
            } else if self.snap_ticks > 0 && !self.towards(self.yaw_speed) {
                self.snap_ticks = 0;
            }
        }

        let (mut yaw_target, mut yaw_rate) = if self.quick {
            let share = match (assist, self.target) {
                (Some(assist), Some(_)) => assist.quick_accel_share,
                _ => 1.0,
            };
            (profile.quick_speed.copysign(x), profile.quick_accel * share)
        } else {
            let rate = profile.yaw_accel * x.abs();
            // A held snap turns at twice the rate.
            let rate = if pushed.abs() < x.abs() {
                2.0 * rate
            } else {
                rate
            };
            (profile.yaw_speed * x, rate)
        };
        yaw_target *= k;
        yaw_rate *= k;
        let mut pitch_target = profile.pitch_speed * y * k;
        let mut pitch_rate = profile.pitch_accel * y.abs() * k;

        if let (Some(assist), Some(target)) = (assist, self.target) {
            let cut = if self.quick {
                assist.quick_cut
            } else {
                assist.yaw_cut
            };
            yaw_target *= 1.0 - cut * b;
            pitch_target *= 1.0 - assist.pitch_cut * b;
            if pushed.abs() < assist.snap_stick && self.snap_ticks > 0 {
                // The snap ends as it would turn past the target, turning onto it.
                let left = -self.off(target.at);
                let next = approach(self.yaw_speed, yaw_target, yaw_rate)
                    .clamp(-yaw_target.abs(), yaw_target.abs());
                if left.abs() < next.abs() {
                    self.snap_ticks = 0;
                    yaw_target = left;
                }
            }
            if !self.quick
                && let Some(aim) = aim.filter(|aim| aim.pushed)
            {
                // Keeping it in view: his motion against the target's, across and up the
                // view, over its distance.
                let mut moved = aim.velocity * aim.tick - target.moved;
                if moved.length() > assist.strafe_cap {
                    moved = moved.normalize() * assist.strafe_cap;
                }
                let distance = target.at.distance(self.camera).max(f32::EPSILON);
                let rotation = look_rotation(view_direction(self.yaw, self.pitch));
                let left = rotation * Vec3::NEG_X;
                let up = rotation * Vec3::Y;
                let across = (moved.dot(left) / distance * assist.strafe_yaw * b)
                    .clamp(-assist.strafe_turn_most, assist.strafe_turn_most);
                yaw_target += across;
                yaw_rate += across.abs();
                let (near, far) = assist.strafe_fade;
                let fade = if distance < far {
                    ((distance - near) / (far - near)).max(0.0)
                } else {
                    1.0
                };
                let lift = -moved.dot(up) / distance * assist.strafe_pitch * fade * b;
                pitch_target += lift;
                pitch_rate += lift.abs();
            }
        }
        self.yaw_speed = speed_towards(self.yaw_speed, yaw_target, yaw_rate);
        self.pitch_speed = speed_towards(self.pitch_speed, pitch_target, pitch_rate);

        // Turning right lowers the yaw.
        self.yaw = wrap(self.yaw - self.yaw_speed - mouse.x);
        self.pitch =
            (self.pitch + self.pitch_speed + mouse.y).clamp(-self.down_limit, self.up_limit);
    }

    /// The aim assist's target (UYA 0x411918): what the view's ray hits first, weighing
    /// 1; else, among those near the view, the lowest of distance × (1 + weight) in sight
    /// of the camera, weighing from 1 dead ahead to 0 at the cone's edge.
    fn pick(&self, assist: &FreeLookAssist, aim: &Aim) -> Option<Aimed> {
        let forward = view_direction(self.yaw, self.pitch);
        let ray = forward * assist.range;
        let clear = aim
            .world
            .cast_ray(self.camera, ray)
            .map_or(1.0, |hit| hit.fraction);
        let along = |at: Vec3| (at - self.camera).dot(forward);
        let hit = aim
            .targets
            .iter()
            .filter(|(_, at)| {
                let t = along(*at);
                t >= 0.0
                    && t <= clear * assist.range
                    && (self.camera + forward * t).distance(*at) <= assist.hit_radius
            })
            .min_by(|a, b| along(a.1).total_cmp(&along(b.1)));
        let picked = match hit {
            Some(&(entity, at)) => Some((entity, at, 1.0)),
            None => {
                let edge = ops::cos(assist.cone_degrees.to_radians());
                // As the game lists them: each better than all before it, best last.
                let mut better: Vec<(Entity, Vec3, f32)> = Vec::new();
                let mut best = f32::INFINITY;
                for &(entity, at) in aim.targets {
                    let to = at - self.camera;
                    let distance = to.length();
                    if distance > assist.range || distance <= f32::EPSILON {
                        continue;
                    }
                    let cos = to.dot(forward) / distance;
                    if cos < edge {
                        continue;
                    }
                    let weight = (cos - edge) / (1.0 - edge);
                    let score = distance * (1.0 + weight);
                    if score < best {
                        best = score;
                        better.push((entity, at, weight));
                    }
                }
                better
                    .into_iter()
                    .rev()
                    .find(|(_, at, _)| aim.world.cast_ray(self.camera, *at - self.camera).is_none())
            }
        };
        picked.map(|(entity, at, weight)| Aimed {
            entity,
            at,
            moved: match self.target {
                Some(last) if last.entity == entity => at - last.at,
                _ => Vec3::ZERO,
            },
            weight,
        })
    }

    /// The weight of what the assist picks: 1 under the view's ray, falling to 0 at its
    /// cone's edge; 0 for nothing.
    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// How far round `at` is from the view, from −π to π: positive to its left.
    fn off(&self, at: Vec3) -> f32 {
        wrap(view_angles(at - self.camera).0 - self.yaw)
    }

    /// Whether a turn of this sign (positive right) is towards the target.
    fn towards(&self, turn: f32) -> bool {
        self.target
            .is_some_and(|target| turn * -self.off(target.at) > 0.0)
    }

    /// Where the camera goes and how it turns, from last tick's eye (UYA 0x411538).
    pub fn place(&mut self, profile: &FreeLookProfile, world: &dyn CollisionWorld) -> (Vec3, Quat) {
        let (position, rotation) = self.placed(profile, world);
        self.camera = position;
        (position, rotation)
    }

    fn placed(&mut self, profile: &FreeLookProfile, world: &dyn CollisionWorld) -> (Vec3, Quat) {
        let forward = view_direction(self.yaw, self.pitch);
        let rotation = look_rotation(forward);
        let eye = self.eye;
        if self.mode != EyeMode::LockStrafe {
            return (eye, rotation);
        }
        // A ray a little past the offset cuts it at once; clear, it eases back out.
        let reach = profile.offset + profile.offset_ray_extra;
        let mut wanted = profile.offset;
        if let Some(hit) = world.cast_ray(eye, -forward * reach) {
            wanted = hit.fraction * reach - profile.offset_ray_extra;
            if wanted < 0.0 {
                return (eye, rotation);
            }
        }
        if wanted < self.offset {
            self.offset = wanted;
            self.offset_speed = 0.0;
        } else if self.offset < wanted {
            self.offset_speed = self.offset_speed.max(0.0);
            ease_to(
                &mut self.offset,
                &mut self.offset_speed,
                wanted,
                profile.offset_accel,
                profile.offset_accel,
                profile.offset_speed,
            );
        }
        let mut position = eye - forward * self.offset;
        if let Some(radius) = profile.end_sphere_radius {
            // A sphere over the last unit stops it at walls the ray missed.
            let from = eye - forward * (self.offset - 1.0).max(0.0);
            let motion = position - from;
            if let Some(hit) = world.cast_sphere(radius, from, motion) {
                position = from + motion * hit.fraction;
            }
        }
        (position, rotation)
    }

    /// Moves the eye for next tick (UYA 0x510000, DL 0x68DF20): its height above his feet,
    /// held through lock-strafe jumps, moved forward when looking down, and kept out of
    /// walls.
    pub fn update_eye(
        &mut self,
        hero: &HeroMotion,
        profile: &FreeLookProfile,
        world: &dyn CollisionWorld,
    ) {
        let mut target = match self.mode {
            EyeMode::FirstPerson if hero.state == HeroState::Crouch => profile.eye_crouch,
            mode => self.eye_target(mode, profile),
        };
        let jumping = match hero.state {
            HeroState::Jump(JumpKind::Ledge) => profile.jump_hold_ledge,
            HeroState::Jump(_) | HeroState::DoubleJump => true,
            _ => false,
        };
        if self.mode == EyeMode::LockStrafe && jumping {
            if self.jump_latched || hero.height_above_ground >= profile.jump_hold_until {
                self.jump_latched = true;
            } else {
                // The eye keeps its height, aiming at the eye height above the ground.
                self.eye_height -= hero.position.y - self.feet_y;
                target = profile.eye_lock_strafe - hero.height_above_ground;
            }
        } else {
            self.jump_latched = false;
        }
        self.feet_y = hero.position.y;
        ease_to(
            &mut self.eye_height,
            &mut self.eye_speed,
            target,
            profile.eye_accel,
            profile.eye_accel,
            profile.eye_speed,
        );

        let mut eye = hero.position + Vec3::Y * self.eye_height;
        if self.pitch < 0.0 {
            let ahead = view_direction(self.yaw, 0.0);
            eye += ahead * (profile.look_down_forward * -self.pitch / FRAC_PI_2);
        }
        let from = hero.position + Vec3::Y * profile.eye_guard_from;
        let motion = eye - from;
        let guarded = profile
            .eye_guard_below
            .is_none_or(|below| self.eye_height < below);
        if guarded
            && motion.length_squared() > 0.01
            && let Some(hit) = world.cast_sphere(profile.eye_guard_radius, from, motion)
        {
            eye = from + motion * hit.fraction;
        }
        self.eye = eye;
    }
}

/// The free look's speed rule: a smaller target is taken at once, a larger one by `rate`.
fn speed_towards(speed: f32, target: f32, rate: f32) -> f32 {
    if target.abs() < speed.abs() {
        target
    } else {
        approach(speed, target, rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HeroProfile, collision::ParryWorld};

    fn uya() -> FreeLookProfile {
        HeroProfile::uya().camera.free_look
    }

    #[test]
    fn a_zoom_slows_the_turn_by_the_field_of_view() {
        // UYA's rates are × the field of view over 1.11 (0x410B08).
        let profile = uya();
        let mut wide = lock_strafe(&profile);
        let mut zoomed = lock_strafe(&profile);
        zoomed.zoom = 0.25;
        for _ in 0..60 {
            wide.turn(Vec2::new(0.5, 0.0), Vec2::ZERO, 50.0, &profile, None);
            zoomed.turn(Vec2::new(0.5, 0.0), Vec2::ZERO, 50.0, &profile, None);
        }
        assert!(wide.yaw_speed > 0.0);
        assert!((zoomed.yaw_speed - wide.yaw_speed * 0.25).abs() < 1e-6);
    }

    fn lock_strafe(profile: &FreeLookProfile) -> FreeLook {
        let hero = HeroMotion::new(Vec3::ZERO, 0.0);
        let mut free = FreeLook::new(&hero, 0.0, profile);
        free.set_mode(EyeMode::LockStrafe, profile);
        free
    }

    #[test]
    fn the_quick_turn_and_the_stop() {
        // camera.md, "Checked in traces": full right stick turns quickly at once, growing
        // 0.005818 × k a tick (k = 1.03125 at speed 50) up to 0.10472 × k; let go, it
        // stops dead.
        let profile = uya();
        let mut free = lock_strafe(&profile);
        let k = 1.03125;
        free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile, None);
        assert!(
            (free.yaw_speed - 0.005818 * k).abs() < 1e-6,
            "{}",
            free.yaw_speed
        );
        for _ in 0..40 {
            free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile, None);
        }
        assert!(
            (free.yaw_speed - 0.10472 * k).abs() < 1e-5,
            "{}",
            free.yaw_speed
        );
        let yaw = free.yaw;
        free.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile, None);
        assert_eq!(free.yaw_speed, 0.0);
        assert_eq!(free.yaw, yaw);
    }

    #[test]
    fn uya_squares_the_stick_and_deadlocked_does_not() {
        // At x = 0.2105 UYA's speed grew 0.001939 × x² × k a tick; DL's 0.0019393 × x × s
        // (`camera.md`, "The free-look camera", "Lock-strafe, DL's default").
        let k = 1.03125;
        let x = 0.2105;
        let profile = uya();
        let mut free = lock_strafe(&profile);
        free.turn(Vec2::new(x, 0.0), Vec2::ZERO, 50.0, &profile, None);
        assert!((free.yaw_speed - 0.001939 * x * x * k).abs() < 1e-8);
        let profile = HeroProfile::dl().camera.free_look;
        let mut free = lock_strafe(&profile);
        free.turn(Vec2::new(x, 0.0), Vec2::ZERO, 50.0, &profile, None);
        assert!((free.yaw_speed - 0.0019393 * x * k).abs() < 1e-8);
        for _ in 0..100 {
            free.turn(Vec2::new(x, 0.0), Vec2::ZERO, 50.0, &profile, None);
        }
        assert!((free.yaw_speed - 0.05236 * x * k).abs() < 1e-6);
    }

    #[test]
    fn deadlocked_turns_quickly_from_the_second_tick() {
        let profile = HeroProfile::dl().camera.free_look;
        let mut free = lock_strafe(&profile);
        let k = 1.03125;
        free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile, None);
        assert!((free.yaw_speed - 0.0019393 * k).abs() < 1e-6);
        free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile, None);
        assert!((free.yaw_speed - (0.0019393 + 0.0034907) * k).abs() < 1e-6);
    }

    #[test]
    fn the_view_stays_within_its_limits() {
        let profile = uya();
        let mut free = lock_strafe(&profile);
        for _ in 0..200 {
            free.turn(Vec2::new(0.0, 1.0), Vec2::ZERO, 50.0, &profile, None);
        }
        assert!((free.pitch - 60f32.to_radians()).abs() < 1e-5);
        for _ in 0..200 {
            free.turn(Vec2::new(0.0, -1.0), Vec2::ZERO, 50.0, &profile, None);
        }
        assert!((free.pitch + 70f32.to_radians()).abs() < 1e-5);
    }

    #[test]
    fn first_persons_up_limit_eases_to_60_degrees() {
        let profile = uya();
        let mut free = lock_strafe(&profile);
        free.set_mode(EyeMode::FirstPerson, &profile);
        free.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile, None);
        assert!((free.up_limit - 87f32.to_radians()).abs() < 1e-5);
        for _ in 0..20 {
            free.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile, None);
        }
        assert!((free.up_limit - 60f32.to_radians()).abs() < 1e-5);
    }

    #[test]
    fn the_lock_strafe_camera_sits_6_u_behind_last_ticks_eye() {
        let profile = uya();
        let world = ParryWorld::default();
        let mut free = lock_strafe(&profile);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        // The eye springs from 1.75 to 2.0, speeding up and braking by 0.00444 u a tick, in
        // 13 ticks (`camera.md`, "Checked in traces").
        for tick in 1..=13 {
            free.update_eye(&hero, &profile, &world);
            assert_eq!(
                free.eye_height == 2.0,
                tick == 13,
                "{tick}: {}",
                free.eye_height
            );
        }
        // Walking right, the camera trails by one tick of his movement.
        hero.position.x = 0.1;
        let (position, rotation) = free.place(&profile, &world);
        assert!(
            (position - Vec3::new(0.0, 2.0, 6.0)).length() < 1e-5,
            "{position}"
        );
        assert!((rotation * Vec3::NEG_Z - Vec3::NEG_Z).length() < 1e-5);
    }

    #[test]
    fn a_wall_behind_cuts_the_offset_and_it_eases_back() {
        use crate::collision::{BodyPlacement, Collider};
        let profile = uya();
        let mut free = lock_strafe(&profile);
        let hero = HeroMotion::new(Vec3::ZERO, 0.0);
        for _ in 0..20 {
            free.update_eye(&hero, &profile, &ParryWorld::default());
        }
        let wall = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(10.0, 10.0, 0.5),
            },
            Vec3::new(0.0, 0.0, 4.5),
            Quat::IDENTITY,
        )]);
        let (position, _) = free.place(&profile, &wall);
        // The wall's face is 4 u behind the eye: the offset is cut to 3.5.
        assert!((position.z - 3.5).abs() < 1e-3, "{position}");
        let open = ParryWorld::default();
        free.place(&profile, &open);
        let (position, _) = free.place(&profile, &open);
        assert!(position.z > 3.5 && position.z < 6.0, "{position}");
        for _ in 0..60 {
            free.place(&profile, &open);
        }
        let (position, _) = free.place(&profile, &open);
        assert!((position.z - 6.0).abs() < 1e-5, "{position}");
    }

    #[test]
    fn a_lock_strafe_jump_holds_the_eye_until_he_is_3_u_up() {
        let profile = uya();
        let world = ParryWorld::default();
        let mut free = lock_strafe(&profile);
        let mut hero = HeroMotion::new(Vec3::ZERO, 0.0);
        for _ in 0..20 {
            free.update_eye(&hero, &profile, &world);
        }
        hero.state = HeroState::Jump(JumpKind::Standing);
        for height in [0.5, 1.0, 1.4, 1.0, 0.3] {
            hero.position.y = height;
            hero.height_above_ground = height;
            free.update_eye(&hero, &profile, &world);
            assert!((free.eye.y - 2.0).abs() < 1e-4, "{}", free.eye.y);
        }
        // Past 3 u up it lets go and follows him.
        hero.state = HeroState::DoubleJump;
        hero.position.y = 3.1;
        hero.height_above_ground = 3.1;
        free.update_eye(&hero, &profile, &world);
        hero.position.y = 3.5;
        hero.height_above_ground = 3.5;
        free.update_eye(&hero, &profile, &world);
        assert!(free.eye.y > 3.5, "{}", free.eye.y);
    }

    #[test]
    fn deadlocked_guards_the_eye_only_while_it_is_held_under_1_15() {
        use crate::collision::{BodyPlacement, Collider};
        // A ceiling 1.3 u up: the 0.4 u sphere from 0.7 u up stops with the eye at 0.9.
        let ceiling = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(10.0, 0.5, 10.0),
            },
            Vec3::new(0.0, 1.8, 0.0),
            Quat::IDENTITY,
        )]);
        let hero = HeroMotion::new(Vec3::ZERO, 0.0);
        let profile = uya();
        let mut free = lock_strafe(&profile);
        free.update_eye(&hero, &profile, &ceiling);
        assert!((free.eye.y - 0.9).abs() < 0.01, "{}", free.eye.y);

        let profile = HeroProfile::dl().camera.free_look;
        let mut free = lock_strafe(&profile);
        free.update_eye(&hero, &profile, &ceiling);
        assert!((free.eye.y - 2.0).abs() < 1e-5, "{}", free.eye.y);
        // Held low, as a jump or a landing holds it.
        free.eye_height = 1.0;
        free.eye_speed = 0.0;
        free.update_eye(&hero, &profile, &ceiling);
        assert!((free.eye.y - 0.9).abs() < 0.01, "{}", free.eye.y);
    }

    #[test]
    fn looking_down_moves_the_eye_forward() {
        let profile = HeroProfile::dl().camera.free_look;
        let world = ParryWorld::default();
        let mut free = lock_strafe(&profile);
        let hero = HeroMotion::new(Vec3::ZERO, 0.0);
        free.pitch = -70f32.to_radians();
        free.update_eye(&hero, &profile, &world);
        // 0.45 × 70 / 90 = 0.35 u ahead, down −z.
        assert!((free.eye.z + 0.35).abs() < 1e-5, "{}", free.eye);
    }

    /// Targets by their middles, numbered from 1.
    fn targets(at: &[Vec3]) -> Vec<(Entity, Vec3)> {
        at.iter()
            .enumerate()
            .map(|(i, &at)| (Entity::from_raw_u32(i as u32 + 1).unwrap(), at))
            .collect()
    }

    fn aim<'a>(targets: &'a [(Entity, Vec3)], world: &'a ParryWorld) -> Aim<'a> {
        Aim {
            targets,
            velocity: Vec3::ZERO,
            tick: crate::controller::TICK,
            pushed: true,
            world,
        }
    }

    /// The camera's eye, 2 u up in lock-strafe, from a hero at the origin.
    const EYE: Vec3 = Vec3::new(0.0, 2.0, 0.0);

    #[test]
    fn a_target_under_the_views_ray_halves_the_sticks_turn() {
        let profile = uya();
        let world = ParryWorld::default();
        let ahead = targets(&[EYE + Vec3::new(0.0, 0.0, -10.0)]);
        let mut free = lock_strafe(&profile);
        let stick = Vec2::new(0.8, 0.0);
        for _ in 0..12 {
            free.turn(
                stick,
                Vec2::ZERO,
                50.0,
                &profile,
                Some(&aim(&ahead, &world)),
            );
            assert_eq!(
                free.target.map(|t| t.weight),
                Some(1.0),
                "under the view's ray"
            );
        }
        let k = 0.25 + 50.0 / 64.0;
        let full = profile.yaw_speed * 0.64 * k;
        assert!(
            (free.yaw_speed - full * 0.5).abs() < 1e-6,
            "{}",
            free.yaw_speed
        );
        // Turned on past the cone's 15°, it is let go and the turn speeds up again.
        for _ in 0..40 {
            free.turn(
                stick,
                Vec2::ZERO,
                50.0,
                &profile,
                Some(&aim(&ahead, &world)),
            );
        }
        assert!(free.target.is_none());
        assert!((free.yaw_speed - full).abs() < 1e-6, "{}", free.yaw_speed);
    }

    #[test]
    fn with_a_target_the_quick_turn_waits_10_ticks_and_speeds_up_at_a_third() {
        let profile = uya();
        let world = ParryWorld::default();
        let assist = profile.assist.unwrap();
        // Off to the left within the cone, so the turn right leaves it slowly.
        let near = targets(&[EYE + Vec3::new(-1.0, 0.0, -20.0)]);
        let mut free = lock_strafe(&profile);
        let full = Vec2::new(1.0, 0.0);
        for tick in 1..assist.quick_after_ticks {
            free.turn(full, Vec2::ZERO, 50.0, &profile, Some(&aim(&near, &world)));
            assert!(!free.quick, "tick {tick}");
        }
        let before = free.yaw_speed;
        free.turn(full, Vec2::ZERO, 50.0, &profile, Some(&aim(&near, &world)));
        assert!(free.quick);
        let k = 0.25 + 50.0 / 64.0;
        let step = free.yaw_speed - before;
        assert!((step - profile.quick_accel * assist.quick_accel_share * k).abs() < 1e-6);
    }

    #[test]
    fn a_hard_push_towards_a_target_is_held_until_the_view_is_on_it() {
        let profile = uya();
        let world = ParryWorld::default();
        // 10° to the right, 10 u off.
        let angle = 10f32.to_radians();
        let right = targets(&[EYE + Vec3::new(angle.sin(), 0.0, -angle.cos()) * 10.0]);
        let mut free = lock_strafe(&profile);
        free.turn(
            Vec2::new(0.95, 0.0),
            Vec2::ZERO,
            50.0,
            &profile,
            Some(&aim(&right, &world)),
        );
        assert_eq!(free.snap_ticks, 7);
        // Let go: the push is held, and the turn stops on the target.
        let mut ticks = 0;
        while free.snap_ticks > 0 && ticks < 20 {
            free.turn(
                Vec2::ZERO,
                Vec2::ZERO,
                50.0,
                &profile,
                Some(&aim(&right, &world)),
            );
            ticks += 1;
        }
        assert!(free.yaw < 0.0, "turned right: {}", free.yaw);
        assert!(
            free.yaw >= -angle - 1e-3,
            "not past it: {}",
            free.yaw.to_degrees()
        );
        // Without a target, letting go stops it at once.
        let mut alone = lock_strafe(&profile);
        alone.turn(
            Vec2::new(0.95, 0.0),
            Vec2::ZERO,
            50.0,
            &profile,
            Some(&aim(&[], &world)),
        );
        alone.turn(
            Vec2::ZERO,
            Vec2::ZERO,
            50.0,
            &profile,
            Some(&aim(&[], &world)),
        );
        assert_eq!(alone.yaw_speed, 0.0);
    }

    #[test]
    fn strafing_turns_the_view_to_keep_a_target_in_it() {
        let profile = uya();
        let world = ParryWorld::default();
        let ahead = targets(&[EYE + Vec3::new(0.0, 0.0, -8.0)]);
        let mut free = lock_strafe(&profile);
        // Running left, the target drifts right: the view follows it right.
        let mut moving = aim(&ahead, &world);
        moving.velocity = Vec3::new(-7.0, 0.0, 0.0);
        for _ in 0..10 {
            free.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile, Some(&moving));
        }
        assert!(free.yaw_speed > 0.0 && free.yaw < 0.0, "{}", free.yaw_speed);
        // Not with the sticks let go.
        let mut still = lock_strafe(&profile);
        let mut idle = aim(&ahead, &world);
        idle.velocity = moving.velocity;
        idle.pushed = false;
        still.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile, Some(&idle));
        assert_eq!(still.yaw_speed, 0.0);
    }

    #[test]
    fn the_target_is_the_lowest_distance_by_weight_in_sight_and_not_for_the_mouse() {
        let profile = uya();
        let assist = profile.assist.unwrap();
        // Two targets off the ray: the further dead-ahead one scores worse than a nearer
        // one off to the side; behind a wall, the nearer one is passed over.
        let side = EYE + Vec3::new(-1.5, 0.0, -9.0);
        let centre = EYE + Vec3::new(0.0, 1.2, -12.0);
        // As the game lists them, only those better than all before are kept: the centre
        // one comes first, so both are.
        let list = targets(&[centre, side]);
        let open = ParryWorld::default();
        let free = lock_strafe(&profile);
        let picked = free.pick(&assist, &aim(&list, &open)).unwrap();
        assert_eq!(picked.at, side);
        let wall = ParryWorld::new(&[crate::collision::BodyPlacement::fixed(
            crate::collision::Collider::Cuboid {
                half_extents: Vec3::new(0.5, 3.0, 0.5),
            },
            EYE + Vec3::new(-1.2, 0.0, -7.0),
            Quat::IDENTITY,
        )]);
        let picked = free.pick(&assist, &aim(&list, &wall)).unwrap();
        assert_eq!(picked.at, centre);
        // Out of range or the cone, nothing.
        let far = targets(&[
            EYE + Vec3::new(0.0, 0.0, -31.0),
            EYE + Vec3::new(5.0, 0.0, -5.0),
        ]);
        assert!(free.pick(&assist, &aim(&far, &open)).is_none());
        // Once the mouse turns the view, the stick gets no help until it is used again.
        let ahead = targets(&[EYE + Vec3::new(0.0, 0.0, -10.0)]);
        let mut free = lock_strafe(&profile);
        free.turn(
            Vec2::ZERO,
            Vec2::new(0.001, 0.0),
            50.0,
            &profile,
            Some(&aim(&ahead, &open)),
        );
        assert!(free.target.is_none());
        // The crosshairs still read what it would pick.
        assert!(free.weight() > 0.0);
        free.turn(
            Vec2::ZERO,
            Vec2::ZERO,
            50.0,
            &profile,
            Some(&aim(&ahead, &open)),
        );
        assert!(free.target.is_none());
        free.turn(
            Vec2::new(0.3, 0.0),
            Vec2::ZERO,
            50.0,
            &profile,
            Some(&aim(&ahead, &open)),
        );
        assert!(free.target.is_some());
    }
}
