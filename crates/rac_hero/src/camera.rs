//! The third-person follow camera (PLAN.md P1.7), after UYA's (`docs/research/camera.md`,
//! "How the follow camera works").
//!
//! It runs in the simulation, because the hero reads the stick relative to the camera's
//! yaw, as in the games: what the camera did last tick decides where "forward" is this
//! tick.
//!
//! It is a drag camera. Each tick:
//! 1. The follow point is the hero's position, held at the take-off height during jumps.
//! 2. A look point trails the follow point on springs.
//! 3. The arm from the follow point to the camera takes up most of the hero's movement, so
//!    the camera stays roughly where it was and swings round to follow. The player turns
//!    and tilts it, and its length is the follow distance, which grows while the hero runs
//!    at the camera.
//! 4. The camera's actual arm chases that arm on springs.
//! 5. The camera sits at the look point, raised by the follow height, plus the actual arm,
//!    and aims a little above the follow point.

use bevy::{math::ops, prelude::*};
use rac_input::PlayerInputs;
use rac_sim::SimTransform;
use std::f32::consts::{PI, TAU};

use crate::{
    CollisionScene, Hero, HeroBody, HeroTuning,
    collision::CollisionWorld,
    controller::{HeroMotion, HeroState, JumpKind, TICK},
    profile::{CameraProfile, Spring},
};

/// A camera whose follow point ends up this far from its hero, sideways or vertically,
/// lost him, as after a respawn, and starts again behind him.
const LOST_SIDEWAYS: f32 = 8.0;
const LOST_VERTICALLY: f32 = 20.0;
/// How far short of geometry the camera stops, beyond its collision radius.
const SKIN: f32 = 0.01;

/// Follows the hero of the same player slot.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct FollowCamera {
    pub player: usize,
    /// The direction the camera faces, radians about +y; 0 looks down −z. The hero reads the
    /// stick relative to this.
    pub yaw: f32,
    /// Radians; negative looks down.
    pub pitch: f32,
    rig: Option<Rig>,
}

/// The camera's state between ticks.
#[derive(Debug, Clone, PartialEq)]
struct Rig {
    /// The point followed: the hero's feet, held vertically during jumps.
    pivot: Vec3,
    /// The followed height is catching up with a landing at a new height.
    catching_up: bool,
    look: Vec3,
    look_rate: Vec3,
    /// The dragged arm, from the pivot towards the camera: its yaw (the camera's view yaw,
    /// so 0 puts the camera at +z), its tilt (positive raises the camera) and the tilt the
    /// player asked for.
    arm_yaw: f32,
    arm_tilt: f32,
    tilt_target: f32,
    distance: f32,
    distance_rate: f32,
    /// The distance target, raised while the hero runs at the camera.
    distance_target: f32,
    height: f32,
    height_rate: f32,
    /// The arm the camera actually uses, chasing the dragged one.
    actual_yaw: f32,
    actual_yaw_rate: f32,
    actual_tilt: f32,
    actual_tilt_rate: f32,
    actual_length: f32,
    actual_length_rate: f32,
}

impl FollowCamera {
    /// A camera for `player` that will start behind the hero, facing `yaw`.
    pub fn for_player(player: usize, yaw: f32) -> Self {
        Self {
            player,
            yaw,
            pitch: 0.0,
            rig: None,
        }
    }

    /// Where the camera rests, and how it is turned, behind a hero standing at `feet` with
    /// the camera facing `self.yaw`.
    pub fn placement(&self, feet: Vec3, profile: &CameraProfile) -> (Vec3, Quat) {
        let position = feet + Vec3::Y * profile.height + arm(self.yaw, 0.0, profile.distance);
        let aim = feet + Vec3::Y * profile.look_height;
        (position, look_rotation(aim - position))
    }
}

impl Rig {
    /// At rest behind the hero.
    fn behind(hero: &HeroMotion, yaw: f32, profile: &CameraProfile) -> Self {
        Self {
            pivot: hero.position,
            catching_up: false,
            look: hero.position,
            look_rate: Vec3::ZERO,
            arm_yaw: yaw,
            arm_tilt: 0.0,
            tilt_target: 0.0,
            distance: profile.distance,
            distance_rate: 0.0,
            distance_target: profile.distance,
            height: profile.height,
            height_rate: 0.0,
            actual_yaw: yaw,
            actual_yaw_rate: 0.0,
            actual_tilt: 0.0,
            actual_tilt_rate: 0.0,
            actual_length: profile.distance,
            actual_length_rate: 0.0,
        }
    }
}

/// The arm with the given yaw, tilt and length.
fn arm(yaw: f32, tilt: f32, length: f32) -> Vec3 {
    let horizontal = ops::cos(tilt) * length;
    Vec3::new(
        ops::sin(yaw) * horizontal,
        ops::sin(tilt) * length,
        ops::cos(yaw) * horizontal,
    )
}

fn look_rotation(direction: Vec3) -> Quat {
    let (yaw, pitch) = view_angles(direction);
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
}

/// The yaw and pitch of a camera looking along `direction`.
fn view_angles(direction: Vec3) -> (f32, f32) {
    let horizontal = Vec2::new(direction.x, direction.z).length();
    (
        ops::atan2(-direction.x, -direction.z),
        ops::atan2(direction.y, horizontal),
    )
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn approach(value: f32, target: f32, step: f32) -> f32 {
    if value < target {
        (value + step).min(target)
    } else {
        (value - step).max(target)
    }
}

/// One tick of the games' spring: the rate gains `gain × error`, loses `damping × rate`,
/// and never carries the value past the target.
fn spring(value: f32, target: f32, spring: Spring, rate: &mut f32) -> f32 {
    let error = target - value;
    *rate += spring.gain * error - spring.damping * *rate;
    *rate = rate.clamp(-error.abs(), error.abs());
    value + *rate
}

/// [`spring`] for an angle, the short way round.
fn angle_spring(value: f32, target: f32, spring: Spring, rate: &mut f32) -> f32 {
    let error = wrap(target - value);
    *rate += spring.gain * error - spring.damping * *rate;
    *rate = rate.clamp(-error.abs(), error.abs());
    wrap(value + *rate)
}

fn lerp_spring(a: Spring, b: Spring, t: f32) -> Spring {
    Spring {
        gain: a.gain + (b.gain - a.gain) * t,
        damping: a.damping + (b.damping - a.damping) * t,
    }
}

/// Advances the rig one tick and returns where the camera goes and where it aims.
fn step(
    rig: &mut Rig,
    hero: &HeroMotion,
    look_input: Vec2,
    view_yaw: f32,
    profile: &CameraProfile,
    world: &dyn CollisionWorld,
) -> (Vec3, Vec3) {
    // 1. The follow point holds its height through a jump, unless the hero drops below it,
    // and catches up after landing somewhere else.
    let previous_pivot = rig.pivot;
    // Wall jumps and climbs carry the hero up a level, so the camera follows those.
    let jumping = match hero.state {
        HeroState::Jump(kind) => !matches!(kind, JumpKind::Wall | JumpKind::Ledge),
        HeroState::DoubleJump => true,
        _ => false,
    };
    let height = if jumping {
        rig.catching_up = true;
        rig.pivot.y.min(hero.position.y)
    } else if rig.catching_up && hero.state != HeroState::Fall {
        let y = approach(
            rig.pivot.y,
            hero.position.y,
            profile.landing_catch_up * TICK,
        );
        rig.catching_up = y != hero.position.y;
        y
    } else {
        rig.catching_up = false;
        hero.position.y
    };
    rig.pivot = Vec3::new(hero.position.x, height, hero.position.z);

    // 2. The look point trails it.
    rig.look.x = spring(
        rig.look.x,
        rig.pivot.x,
        profile.look_spring,
        &mut rig.look_rate.x,
    );
    rig.look.z = spring(
        rig.look.z,
        rig.pivot.z,
        profile.look_spring,
        &mut rig.look_rate.z,
    );
    rig.look.y = spring(
        rig.look.y,
        rig.pivot.y,
        profile.look_spring_vertical,
        &mut rig.look_rate.y,
    );

    // 3. The arm takes up most of the movement, then the player turns and tilts it.
    let dragged =
        arm(rig.arm_yaw, rig.arm_tilt, rig.distance) - profile.drag * (rig.pivot - previous_pivot);
    let horizontal = Vec2::new(dragged.x, dragged.z).length();
    if horizontal > 1e-4 {
        rig.arm_yaw = ops::atan2(dragged.x, dragged.z);
    }
    let tilt = ops::atan2(dragged.y, horizontal);
    // Turning right lowers the yaw; looking up lowers the camera.
    rig.arm_yaw = wrap(rig.arm_yaw - look_input.x);
    let limit = profile.pitch_limit_degrees.to_radians();
    rig.tilt_target = (rig.tilt_target - look_input.y).clamp(-limit, limit);
    rig.arm_tilt = approach(tilt, rig.tilt_target, profile.pitch_step);

    // 4. Running at the camera pushes its distance out.
    let motion = Vec3::new(hero.velocity.x, 0.0, hero.velocity.z);
    let speed = motion.length();
    let view = Vec3::new(-ops::sin(view_yaw), 0.0, -ops::cos(view_yaw));
    let approaching = if speed > 1e-3 && motion.dot(view) / speed < profile.approach_facing {
        (speed / profile.approach_full_speed).min(1.0)
    } else {
        0.0
    };
    if approaching > 0.0 {
        let target =
            profile.distance + (profile.approach_distance - profile.distance) * approaching;
        rig.distance_target = rig.distance_target.max(target);
    } else {
        rig.distance_target = profile.distance;
    }

    // 5. The actual arm chases the dragged one.
    rig.actual_yaw = angle_spring(
        rig.actual_yaw,
        rig.arm_yaw,
        profile.arm_spring,
        &mut rig.actual_yaw_rate,
    );
    rig.actual_tilt = angle_spring(
        rig.actual_tilt,
        rig.arm_tilt,
        profile.arm_spring,
        &mut rig.actual_tilt_rate,
    );
    let length_spring = lerp_spring(
        profile.arm_length_spring,
        profile.arm_length_spring_approach,
        approaching,
    );
    rig.actual_length = spring(
        rig.actual_length,
        rig.distance,
        length_spring,
        &mut rig.actual_length_rate,
    );

    // The distance and height settle at the end of the tick, as in the game.
    rig.distance = spring(
        rig.distance,
        rig.distance_target,
        profile.distance_spring,
        &mut rig.distance_rate,
    );
    rig.height = spring(
        rig.height,
        profile.height,
        profile.height_spring,
        &mut rig.height_rate,
    );

    // 6. Place the camera, keeping it out of geometry.
    let wanted =
        rig.look + Vec3::Y * rig.height + arm(rig.actual_yaw, rig.actual_tilt, rig.actual_length);
    let aim = rig.pivot + Vec3::Y * profile.look_height;
    let reach = wanted - aim;
    let position = match world.cast_sphere(profile.collision_radius, aim, reach) {
        Some(hit) => {
            let length = reach.length();
            aim + reach / length * (hit.fraction * length - SKIN).max(0.0)
        }
        None => wanted,
    };
    (position, aim)
}

pub(crate) fn follow(
    inputs: Res<PlayerInputs>,
    tuning: Res<HeroTuning>,
    scene: Res<CollisionScene>,
    heroes: Query<(&Hero, &HeroBody)>,
    mut cameras: Query<(&mut FollowCamera, &mut SimTransform)>,
) {
    let profile = &tuning.0.camera;
    for (mut camera, mut transform) in &mut cameras {
        let Some((_, body)) = heroes.iter().find(|(hero, _)| hero.player == camera.player) else {
            continue;
        };
        let hero = &body.0;
        let view_yaw = camera.yaw;
        let lost = camera.rig.as_ref().is_none_or(|rig| {
            let away = hero.position - rig.pivot;
            Vec2::new(away.x, away.z).length() > LOST_SIDEWAYS || away.y.abs() > LOST_VERTICALLY
        });
        if lost {
            camera.rig = Some(Rig::behind(hero, view_yaw, profile));
        }
        let look = inputs.get(camera.player).look_radians();
        let Some(rig) = camera.rig.as_mut() else {
            continue;
        };
        let (position, aim) = step(rig, hero, look, view_yaw, profile, &scene.0);
        let (yaw, pitch) = view_angles(aim - position);
        camera.yaw = yaw;
        camera.pitch = pitch;
        transform.translation = position;
        transform.rotation = look_rotation(aim - position);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HeroProfile,
        collision::{BodyPlacement, Collider, ParryWorld},
    };

    fn profile() -> CameraProfile {
        HeroProfile::uya().camera
    }

    fn empty() -> ParryWorld {
        ParryWorld::default()
    }

    /// A rig at rest behind a hero at the origin facing −z.
    fn rest() -> (Rig, HeroMotion) {
        let hero = HeroMotion::new(Vec3::ZERO, 0.0);
        (Rig::behind(&hero, 0.0, &profile()), hero)
    }

    #[test]
    fn rests_behind_and_above_the_hero() {
        let camera = FollowCamera::for_player(0, 0.0);
        let profile = profile();
        let (position, rotation) = camera.placement(Vec3::new(10.0, 5.0, 0.0), &profile);
        assert!(
            (position - Vec3::new(10.0, 7.0, 4.64)).length() < 1e-4,
            "{position}"
        );
        // It aims 1.5 u above the feet.
        let towards_aim = (Vec3::new(10.0, 6.5, 0.0) - position).normalize();
        assert!((rotation * Vec3::NEG_Z - towards_aim).length() < 1e-4);
    }

    #[test]
    fn a_still_hero_keeps_the_camera_at_rest() {
        let (mut rig, hero) = rest();
        for _ in 0..60 {
            let (position, aim) = step(&mut rig, &hero, Vec2::ZERO, 0.0, &profile(), &empty());
            assert!(
                (position - Vec3::new(0.0, 2.0, 4.64)).length() < 1e-4,
                "{position}"
            );
            assert_eq!(aim, Vec3::new(0.0, 1.5, 0.0));
        }
    }

    #[test]
    fn running_sideways_drags_the_arm_round() {
        // camera.md, step 3: at 0.1028 u per tick across the arm, it turns
        // 0.85 × 0.1028 / 4.64 = 0.01883 rad a tick (measured in uyantsc-veldin-cam-1). The
        // hero runs to the camera's right, so the camera turns right: its yaw falls.
        let (mut rig, mut hero) = rest();
        let speed = 6.17;
        hero.velocity = Vec3::X * speed;
        hero.state = HeroState::Walk;
        let mut yaw = rig.arm_yaw;
        for tick in 0..20 {
            hero.position += hero.velocity * TICK;
            step(&mut rig, &hero, Vec2::ZERO, 0.0, &profile(), &empty());
            let turned = wrap(rig.arm_yaw - yaw);
            yaw = rig.arm_yaw;
            if tick < 5 {
                assert!(
                    (turned + 0.85 * speed * TICK / 4.64).abs() < 1e-4,
                    "{turned}"
                );
            }
        }
        // The actual arm trails the dragged one.
        assert!(wrap(rig.arm_yaw - rig.actual_yaw) < -0.05);
    }

    #[test]
    fn the_look_point_trails_a_running_hero_by_1_27() {
        // camera.md, step 2: after each tick it is (0.2 / 0.015 − 1) × v per tick behind.
        let (mut rig, mut hero) = rest();
        hero.velocity = Vec3::NEG_Z * 6.17;
        hero.state = HeroState::Walk;
        for _ in 0..300 {
            hero.position += hero.velocity * TICK;
            step(&mut rig, &hero, Vec2::ZERO, 0.0, &profile(), &empty());
        }
        let lag = rig.pivot.z - rig.look.z;
        assert!(
            (lag + (0.2 / 0.015 - 1.0) * 6.17 * TICK).abs() < 0.01,
            "{lag}"
        );
    }

    #[test]
    fn the_distance_springs_back_as_in_the_trace() {
        // uyantsc-veldin-cam-1, frames 178299 and 178300: 5.0949955 then 5.087694; 300
        // frames later, 4.6434946 (camera.md, step 5).
        let (mut rig, hero) = rest();
        rig.distance = 5.087_694;
        rig.distance_rate = 5.087_694 - 5.094_995_5;
        rig.actual_length = rig.distance;
        for _ in 0..300 {
            step(&mut rig, &hero, Vec2::ZERO, 0.0, &profile(), &empty());
        }
        assert!(
            (rig.distance - 4.643_494_6).abs() < 1e-4,
            "{}",
            rig.distance
        );
    }

    #[test]
    fn running_at_the_camera_pushes_it_back() {
        let (mut rig, mut hero) = rest();
        hero.velocity = Vec3::Z * 6.17;
        hero.state = HeroState::Walk;
        for _ in 0..60 {
            hero.position += hero.velocity * TICK;
            step(&mut rig, &hero, Vec2::ZERO, 0.0, &profile(), &empty());
        }
        assert_eq!(rig.distance_target, 6.0);
        assert!(rig.distance > 5.0, "{}", rig.distance);
    }

    #[test]
    fn a_jump_does_not_lift_the_camera() {
        let (mut rig, mut hero) = rest();
        hero.state = HeroState::Jump(JumpKind::Standing);
        for tick in 0..30 {
            hero.position.y = 1.5 * ops::sin(tick as f32 / 30.0 * PI);
            let (position, _) = step(&mut rig, &hero, Vec2::ZERO, 0.0, &profile(), &empty());
            assert!((position.y - 2.0).abs() < 1e-4, "{position}");
        }
    }

    #[test]
    fn the_player_turns_and_tilts_the_arm() {
        let (mut rig, hero) = rest();
        step(
            &mut rig,
            &hero,
            Vec2::new(0.1, 0.0),
            0.0,
            &profile(),
            &empty(),
        );
        assert!((rig.arm_yaw + 0.1).abs() < 1e-5);
        // Tilting is limited to 40° and to 0.0305 rad a tick.
        for _ in 0..200 {
            step(
                &mut rig,
                &hero,
                Vec2::new(0.0, -0.1),
                0.0,
                &profile(),
                &empty(),
            );
        }
        assert!(
            (rig.arm_tilt - 40f32.to_radians()).abs() < 1e-4,
            "{}",
            rig.arm_tilt
        );
    }

    #[test]
    fn walls_keep_the_camera_in_front_of_them() {
        // A wall 2 u behind the hero.
        let world = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(10.0, 10.0, 0.5),
            },
            Vec3::new(0.0, 0.0, 2.5),
            Quat::IDENTITY,
        )]);
        let (mut rig, hero) = rest();
        let (position, _) = step(&mut rig, &hero, Vec2::ZERO, 0.0, &profile(), &world);
        assert!(position.z < 2.0 - 0.24, "{position}");
    }
}
