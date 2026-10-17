//! The free-look camera behind the lock-strafe and first-person views and the look, after
//! UYA's 0x410B08 and Deadlocked's 0x5620A0 (`camera.md`, "The free-look camera", "The eye
//! point", "Lock-strafe, DL's default").
//!
//! Only the player turns it. The stick sets a turn speed, which speeds up towards its
//! target and drops at once to a smaller one, so the view stops dead when the stick is let
//! go; the mouse turns it directly. The camera sits at the hero's eye in first person, or
//! `offset` behind it along the view in lock-strafe, both from last tick's eye, as in the
//! games, so it is rigid behind him with no lag or drag.

use super::{approach, ease_to, look_rotation, view_direction, wrap};
use crate::{
    collision::CollisionWorld,
    controller::{HeroMotion, HeroState, JumpKind},
    profile::FreeLookProfile,
};
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// The games' camera mode for the hero (`hero_camera_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EyeMode {
    /// No free-look view is shown; the eye rests at the lock-strafe height.
    Off,
    LockStrafe,
    FirstPerson,
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
    /// Ticks running with the stick past the quick turn's threshold.
    quick_ticks: u32,
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
    /// y up), `mouse` the mouse's turn in radians, `speed` the camera-speed option.
    pub fn turn(&mut self, stick: Vec2, mouse: Vec2, speed: f32, profile: &FreeLookProfile) {
        if self.mode != EyeMode::Off {
            let rest = profile.up_limit_rest_degrees.to_radians();
            self.up_limit = approach(
                self.up_limit,
                rest,
                profile.up_limit_ease_degrees.to_radians(),
            );
        }
        let k = 0.25 + speed / 64.0;
        let shape = |axis: f32| {
            if profile.squared {
                axis * axis.abs()
            } else {
                axis
            }
        };
        let (x, y) = (shape(stick.x), shape(stick.y));

        self.quick_ticks = if x.abs() > profile.quick_stick {
            self.quick_ticks + 1
        } else {
            0
        };
        let (target, rate) = if self.quick_ticks >= profile.quick_after_ticks.max(1) {
            (profile.quick_speed.copysign(x), profile.quick_accel)
        } else {
            (profile.yaw_speed * x, profile.yaw_accel * x.abs())
        };
        self.yaw_speed = speed_towards(self.yaw_speed, target * k, rate * k);
        self.pitch_speed = speed_towards(
            self.pitch_speed,
            profile.pitch_speed * y * k,
            profile.pitch_accel * y.abs() * k,
        );

        // Turning right lowers the yaw.
        self.yaw = wrap(self.yaw - self.yaw_speed - mouse.x);
        self.pitch =
            (self.pitch + self.pitch_speed + mouse.y).clamp(-self.down_limit, self.up_limit);
    }

    /// Where the camera goes and how it turns, from last tick's eye (UYA 0x411538).
    pub fn place(&mut self, profile: &FreeLookProfile, world: &dyn CollisionWorld) -> (Vec3, Quat) {
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
        if motion.length_squared() > 0.01
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
        free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile);
        assert!(
            (free.yaw_speed - 0.005818 * k).abs() < 1e-6,
            "{}",
            free.yaw_speed
        );
        for _ in 0..40 {
            free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile);
        }
        assert!(
            (free.yaw_speed - 0.10472 * k).abs() < 1e-5,
            "{}",
            free.yaw_speed
        );
        let yaw = free.yaw;
        free.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile);
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
        free.turn(Vec2::new(x, 0.0), Vec2::ZERO, 50.0, &profile);
        assert!((free.yaw_speed - 0.001939 * x * x * k).abs() < 1e-8);
        let profile = HeroProfile::dl().camera.free_look;
        let mut free = lock_strafe(&profile);
        free.turn(Vec2::new(x, 0.0), Vec2::ZERO, 50.0, &profile);
        assert!((free.yaw_speed - 0.0019393 * x * k).abs() < 1e-8);
        for _ in 0..100 {
            free.turn(Vec2::new(x, 0.0), Vec2::ZERO, 50.0, &profile);
        }
        assert!((free.yaw_speed - 0.05236 * x * k).abs() < 1e-6);
    }

    #[test]
    fn deadlocked_turns_quickly_from_the_second_tick() {
        let profile = HeroProfile::dl().camera.free_look;
        let mut free = lock_strafe(&profile);
        let k = 1.03125;
        free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile);
        assert!((free.yaw_speed - 0.0019393 * k).abs() < 1e-6);
        free.turn(Vec2::new(1.0, 0.0), Vec2::ZERO, 50.0, &profile);
        assert!((free.yaw_speed - (0.0019393 + 0.0034907) * k).abs() < 1e-6);
    }

    #[test]
    fn the_view_stays_within_its_limits() {
        let profile = uya();
        let mut free = lock_strafe(&profile);
        for _ in 0..200 {
            free.turn(Vec2::new(0.0, 1.0), Vec2::ZERO, 50.0, &profile);
        }
        assert!((free.pitch - 60f32.to_radians()).abs() < 1e-5);
        for _ in 0..200 {
            free.turn(Vec2::new(0.0, -1.0), Vec2::ZERO, 50.0, &profile);
        }
        assert!((free.pitch + 70f32.to_radians()).abs() < 1e-5);
    }

    #[test]
    fn first_persons_up_limit_eases_to_60_degrees() {
        let profile = uya();
        let mut free = lock_strafe(&profile);
        free.set_mode(EyeMode::FirstPerson, &profile);
        free.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile);
        assert!((free.up_limit - 87f32.to_radians()).abs() < 1e-5);
        for _ in 0..20 {
            free.turn(Vec2::ZERO, Vec2::ZERO, 50.0, &profile);
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
}
