//! How the follow camera keeps clear of the level, after UYA's `camera_follow_motion`
//! (0x3E39E0) and `camera_obstacles` (0x3E64E0): `camera.md`, "Obstacles and collision".
//!
//! - **The clearance point** is the arm's pivot: the hero's height plus the follow height,
//!   kept a clearance radius under a low ceiling, or halfway in a gap too low for it.
//! - **The sweep:** spheres along the arm push it sideways round anything beside it.
//! - **The camera's end:** a sphere of the clearance radius there turns the arm away from
//!   floors and ceilings, and a wall pulls it in.
//! - **The pull-in** stays while anything touches, grows while the player turns the camera
//!   into a wall, and eases back out by a cosine once clear.
//!
//! The games ignore mobys that the kit has no counterpart for (step 1 of `camera_obstacles`
//! hides a few classes), and their spheres test the hero's own collision layers; the kit's
//! [`CollisionWorld`] holds only the level, so neither applies.

use super::follow::Rig;
use super::{arm, wrap};
use crate::{
    collision::{CapsuleShape, CollisionWorld},
    controller::HeroMotion,
    profile::CameraProfile,
};
use bevy::{math::ops, prelude::*};
use std::f32::consts::PI;

/// A contact pushing the camera down more than this is a ceiling; up more than
/// `FLOOR`, a floor; anything else a wall.
const CEILING: f32 = 0.75;
const FLOOR: f32 = 0.5;

/// The arm's pivot this tick, and the follow height a low ceiling asks for.
pub(super) struct Clearance {
    pub point: Vec3,
    pub height: Option<f32>,
}

/// The clearance point (follow +0xB0): rays up and down from just above his feet find the
/// floor and ceiling.
pub(super) fn clearance(
    hero: &HeroMotion,
    follow: Vec3,
    height: f32,
    profile: &CameraProfile,
    world: &dyn CollisionWorld,
) -> Clearance {
    let c = &profile.clearance;
    let from = hero.position + Vec3::Y * c.ray_from;
    let floor = world
        .cast_ray(from, Vec3::NEG_Y * c.ray_length)
        .map(|hit| from.y - hit.fraction * c.ray_length);
    let ceiling = world
        .cast_ray(from, Vec3::Y * c.ray_length)
        .map(|hit| from.y + hit.fraction * c.ray_length);
    let at = |y: f32| Vec3::new(hero.position.x, y, hero.position.z);
    match (floor, ceiling) {
        (Some(floor), Some(ceiling)) if ceiling - floor < 2.0 * c.radius => {
            let middle = (floor + ceiling) / 2.0;
            Clearance {
                point: at(middle),
                height: Some(middle - follow.y),
            }
        }
        (_, Some(ceiling)) if ceiling <= follow.y + height + c.radius => {
            let under = ceiling - c.radius;
            Clearance {
                point: at(under),
                height: Some(under - follow.y),
            }
        }
        _ => Clearance {
            point: at(floor.map_or(follow.y, |floor| floor.max(follow.y)) + height),
            height: None,
        },
    }
}

/// Steps 1 to 7 of `camera_obstacles`: turns the dragged arm round obstacles and sets its
/// pull-in.
pub(super) fn avoid(
    rig: &mut Rig,
    pivot: Vec3,
    stick: Vec2,
    profile: &CameraProfile,
    world: &dyn CollisionWorld,
) {
    let o = &profile.obstacles;
    let turning = stick.x.abs() > o.wall_stick;
    rig.on_floor = false;
    let mut touched = false;

    if let Some(wall) = sweep(rig, pivot, profile, world) {
        touched = true;
        if wall && turning {
            rig.pull_in += o.wall_pull;
            rig.pull_in_timer = o.pull_in_wall;
        }
    }
    if let Some(wall) = end_sphere(rig, pivot, profile, world) {
        touched = true;
        if wall && turning {
            rig.pull_in += o.wall_pull;
            rig.pull_in_timer = o.pull_in_wall;
        }
    }
    rig.pull_in = rig.pull_in.clamp(0.0, (rig.distance - o.min_arm).max(0.0));

    let resting = stick.x.abs() < o.idle_stick && stick.y.abs() < o.idle_stick;
    if touched {
        if rig.pull_in_timer <= o.pull_in_rest {
            rig.pull_in_timer = o.pull_in_rest;
        } else {
            rig.pull_in_timer -= 1;
        }
    } else if (1..o.pull_in_rest).contains(&rig.pull_in_timer) || resting {
        rig.pull_in_timer = rig.pull_in_timer.saturating_sub(1);
        let t = rig.pull_in_timer as f32 / o.pull_in_rest as f32;
        rig.pull_in *= (1.0 - ops::cos(PI * t)) / 2.0;
    }
}

/// `camera_arm_sweep` (0x3E6128): growing spheres along the arm; where one touches, the
/// arm turns away until its line passes the contact by the clearance for that far along,
/// keeping its length. Returns whether anything touched, and if so whether it was a wall.
fn sweep(
    rig: &mut Rig,
    pivot: Vec3,
    profile: &CameraProfile,
    world: &dyn CollisionWorld,
) -> Option<bool> {
    let o = &profile.obstacles;
    let length = rig.arm_length();
    if length <= 1e-3 || rig.distance <= 1e-3 {
        return None;
    }
    let scale = length / rig.distance;
    let mut touched: Option<bool> = None;
    for (index, &(along, radius)) in o.sweep.iter().enumerate() {
        let along = along * scale;
        for _ in 0..3 {
            let direction = arm(rig.arm_yaw, rig.arm_tilt, 1.0);
            let centre = pivot + direction * along;
            let ball = CapsuleShape {
                half_height: 0.0,
                radius,
            };
            let contact = world
                .capsule_penetrations(ball, centre)
                .into_iter()
                .map(|push| (centre - push.normal * (radius - push.depth), push.normal))
                // The first sphere ignores what is behind the pivot.
                .filter(|(point, _)| index > 0 || (*point - pivot).dot(direction) >= 0.0)
                .max_by(|a, b| {
                    let depth = |point: Vec3| -(point - centre).length();
                    depth(a.0).total_cmp(&depth(b.0))
                });
            let Some((point, normal)) = contact else {
                break;
            };
            let wall = normal.y.abs() <= CEILING;
            touched = Some(touched.unwrap_or(false) || wall);
            let clear =
                o.sweep_clear_scale * (o.sweep_clear + o.sweep_clear_along * along / length);
            if !push_arm(rig, pivot, point, clear) {
                break;
            }
        }
    }
    touched
}

/// `camera_arm_push` (0x3E5D00): turns the arm about the pivot, away from `point`, until
/// its line passes `point` by `clear`. Returns false if it already did.
fn push_arm(rig: &mut Rig, pivot: Vec3, point: Vec3, clear: f32) -> bool {
    let direction = arm(rig.arm_yaw, rig.arm_tilt, 1.0);
    let to = point - pivot;
    let reach = to.length();
    if reach < 1e-4 {
        return false;
    }
    let along = to.dot(direction);
    let across = to - direction * along;
    let off = across.length();
    if off >= clear {
        return false;
    }
    // Turn in the plane of the arm and the contact, away from the contact.
    let away = if off > 1e-5 {
        -across / off
    } else {
        // Dead ahead: go round it sideways.
        Vec3::new(direction.z, 0.0, -direction.x).normalize_or(Vec3::X)
    };
    let now = ops::asin((off / reach).min(1.0));
    let needed = ops::asin((clear / reach).min(1.0));
    let angle = needed - now;
    let turned = direction * ops::cos(angle) + away * ops::sin(angle);
    set_direction(rig, turned);
    true
}

/// `camera_end_sphere` (0x3E5930): the camera's end keeps a clearance radius from
/// everything. Returns whether anything touched, and if so whether it was a wall.
fn end_sphere(
    rig: &mut Rig,
    pivot: Vec3,
    profile: &CameraProfile,
    world: &dyn CollisionWorld,
) -> Option<bool> {
    let o = &profile.obstacles;
    let radius = profile.clearance.radius;
    let length = rig.arm_length();
    let mut spot = pivot + arm(rig.arm_yaw, rig.arm_tilt, length);
    // Moving further than the radius, it stops at the first wall on the way.
    let travel = spot - rig.camera;
    if travel.length() > radius
        && let Some(hit) = world.cast_ray(rig.camera, travel)
    {
        spot = rig.camera + travel * hit.fraction - travel.normalize() * o.end_skin;
    }
    let ball = CapsuleShape {
        half_height: 0.0,
        radius,
    };
    let mut first = None;
    for _ in 0..3 {
        let deepest = world
            .capsule_penetrations(ball, spot)
            .into_iter()
            .max_by(|a, b| a.depth.total_cmp(&b.depth));
        let Some(push) = deepest else {
            break;
        };
        first.get_or_insert(push.normal);
        spot += push.normal * push.depth;
    }
    let normal = first?;
    let moved = spot - pivot;
    let moved_length = moved.length();
    if moved_length < 1e-4 {
        return Some(false);
    }
    set_direction(rig, moved / moved_length);
    let ceiling = normal.y < -CEILING;
    let floor = normal.y > FLOOR;
    if floor {
        rig.on_floor = true;
    }
    if ceiling || floor {
        return Some(false);
    }
    // A wall: the arm comes in to the pushed spot, though never under its least length.
    if moved_length < length {
        rig.pull_in = rig.distance - moved_length.max(o.min_arm_wall);
    }
    Some(true)
}

fn set_direction(rig: &mut Rig, direction: Vec3) {
    let horizontal = Vec2::new(direction.x, direction.z).length();
    if horizontal > 1e-5 {
        rig.arm_yaw = wrap(ops::atan2(direction.x, direction.z));
    }
    rig.arm_tilt = ops::atan2(direction.y, horizontal);
}
