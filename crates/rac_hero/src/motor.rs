//! Moving the hero's capsule through the world: sliding along what it hits, keeping to
//! the ground, and riding moving platforms.
//!
//! The capsule floats `bottom - radius` above the hero's feet while he is on the ground.
//! A sphere probe below it finds the ground and sets his feet on it, which is what lets
//! him walk up steps lower than that gap, follow slopes down, and stand on edges.

use bevy::prelude::*;

use crate::{
    collision::{BodyId, CapsuleShape, CollisionWorld},
    profile::BodyProfile,
};

/// The ground the hero stands on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    pub normal: Vec3,
    pub body: BodyId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveOutcome {
    pub position: Vec3,
    pub ground: Option<Ground>,
    /// Something above stopped upward movement.
    pub hit_ceiling: bool,
    /// Something too steep to stand on stopped sideways movement.
    pub hit_wall: bool,
}

/// How many times a move may slide along a surface and carry on.
const MAX_SLIDES: usize = 4;
/// How far a sweep stops short of contact.
const SKIN: f32 = 0.005;
/// On the ground, sideways sweeps start this much higher. Without it a step exactly as
/// high as the capsule's float would graze its underside and stop the hero.
const GROUND_CLEARANCE: f32 = 0.02;
/// The ground probe is narrower than the capsule, so standing flush against a step or
/// wall doesn't make it catch that edge and miss the floor.
const PROBE_RADIUS_FACTOR: f32 = 2.0 / 3.0;
/// Extra probe length below an airborne capsule, so a landing is found on the tick the
/// capsule reaches the ground rather than a tick later.
const LANDING_REACH: f32 = 0.05;

pub fn capsule(body: &BodyProfile, airborne: bool) -> (CapsuleShape, f32) {
    let bottom = if airborne {
        body.bottom_airborne
    } else {
        body.bottom
    };
    (
        CapsuleShape {
            half_height: (body.top - bottom) / 2.0,
            radius: body.radius,
        },
        (body.top + bottom) / 2.0,
    )
}

pub fn is_walkable(normal: Vec3, body: &BodyProfile) -> bool {
    normal.y >= bevy::math::ops::cos(body.slope_limit_degrees.to_radians())
}

/// Moves the hero by `displacement` from `position` (his feet).
///
/// `grounded` says whether he stood on the ground before the move: then the ground is
/// searched for further below, and walls are slid along horizontally only, so he can't
/// walk up anything too steep.
pub fn move_body(
    world: &dyn CollisionWorld,
    body: &BodyProfile,
    mut position: Vec3,
    displacement: Vec3,
    grounded: bool,
) -> MoveOutcome {
    let airborne = !grounded;
    let (shape, centre_height) = capsule(body, airborne);
    let clearance = if grounded { GROUND_CLEARANCE } else { 0.0 };
    let centre = |position: Vec3| position + Vec3::Y * centre_height;
    let swept_centre = |position: Vec3| centre(position) + Vec3::Y * clearance;

    // Get out of anything the capsule overlaps, such as a platform that moved into it.
    for penetration in world.capsule_penetrations(shape, swept_centre(position)) {
        position += penetration.normal * (penetration.depth + SKIN);
    }

    let mut hit_ceiling = false;
    let mut hit_wall = false;
    let mut remaining = displacement;
    for _ in 0..MAX_SLIDES {
        if remaining.length_squared() < 1e-10 {
            break;
        }
        let Some(hit) = world.cast_capsule(shape, swept_centre(position), remaining) else {
            position += remaining;
            break;
        };
        let length = remaining.length();
        let travel = (hit.fraction * length - SKIN).max(0.0);
        position += remaining / length * travel;
        // Keep all that wasn't travelled: the push into the surface goes below, and the
        // rest slides on. Dropping the skin's share too would cut a slide along a wall
        // short by it every tick, which a shimmy's small steps into the wall show.
        remaining *= 1.0 - travel / length;

        let mut normal = hit.normal;
        if normal.y < -0.5 {
            hit_ceiling = true;
        }
        if !is_walkable(normal, body) {
            if normal.y > -0.5 {
                hit_wall = true;
            }
            if grounded {
                // Slide along walls and steep slopes without being lifted up them.
                normal = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
            }
        }
        remaining -= normal * remaining.dot(normal).min(0.0);
    }

    // Look for ground below, unless the hero is rising.
    let mut ground = None;
    if grounded || displacement.y <= 0.0 {
        let probe_radius = body.radius * PROBE_RADIUS_FACTOR;
        let probe_from = centre(position) - Vec3::Y * shape.half_height;
        let reach = (probe_from.y - position.y - probe_radius)
            + if grounded {
                body.ground_snap
            } else {
                LANDING_REACH
            };
        if let Some(hit) = world.cast_sphere(probe_radius, probe_from, Vec3::NEG_Y * reach)
            && is_walkable(hit.normal, body)
        {
            position.y = probe_from.y - hit.fraction * reach - probe_radius;
            ground = Some(Ground {
                normal: hit.normal,
                body: hit.body,
            });
        }
    }

    MoveOutcome {
        position,
        ground,
        hit_ceiling,
        hit_wall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HeroProfile,
        collision::{BodyPlacement, Collider, ParryWorld},
    };

    fn block(centre: Vec3, half_extents: Vec3) -> BodyPlacement {
        BodyPlacement::fixed(Collider::Cuboid { half_extents }, centre, Quat::IDENTITY)
    }

    fn floor() -> BodyPlacement {
        block(Vec3::new(0.0, -0.5, 0.0), Vec3::new(50.0, 0.5, 50.0))
    }

    fn body() -> BodyProfile {
        HeroProfile::uya().body
    }

    #[test]
    fn stays_on_flat_ground() {
        let world = ParryWorld::new(&[floor()]);
        let out = move_body(&world, &body(), Vec3::ZERO, Vec3::new(0.1, 0.0, 0.0), true);
        assert!(out.ground.is_some());
        assert!(
            (out.position - Vec3::new(0.1, 0.0, 0.0)).length() < 1e-3,
            "{}",
            out.position
        );
    }

    #[test]
    fn walks_up_a_low_step_but_not_a_high_one() {
        let low = ParryWorld::new(&[
            floor(),
            block(Vec3::new(2.0, 0.125, 0.0), Vec3::new(1.0, 0.125, 5.0)),
        ]);
        let mut position = Vec3::new(0.0, 0.0, 0.0);
        for _ in 0..30 {
            position = move_body(&low, &body(), position, Vec3::new(0.1, 0.0, 0.0), true).position;
        }
        assert!((position.y - 0.25).abs() < 1e-3, "{position}");
        assert!(position.x > 2.0);

        let high = ParryWorld::new(&[
            floor(),
            block(Vec3::new(2.0, 0.25, 0.0), Vec3::new(1.0, 0.25, 5.0)),
        ]);
        let mut position = Vec3::ZERO;
        for _ in 0..30 {
            let out = move_body(&high, &body(), position, Vec3::new(0.1, 0.0, 0.0), true);
            position = out.position;
        }
        assert!(position.y.abs() < 1e-3, "{position}");
        // The lower sphere (centre 0.72 up, with the ground clearance) meets the step's top
        // edge at (1, 0.5), so it stops √(0.45² − 0.22²) short of x = 1.
        let expected = 1.0 - (0.45f32 * 0.45 - 0.22 * 0.22).sqrt();
        assert!((position.x - expected).abs() < 0.01, "{position}");
    }

    #[test]
    fn falls_off_an_edge_and_lands() {
        let world = ParryWorld::new(&[
            floor(),
            block(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 1.0)),
        ]);
        let out = move_body(
            &world,
            &body(),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            true,
        );
        assert!(out.ground.is_none(), "walked off the block");
        let mut position = out.position;
        let mut landed = None;
        for _ in 0..60 {
            let out = move_body(&world, &body(), position, Vec3::new(0.0, -0.2, 0.0), false);
            position = out.position;
            if out.ground.is_some() {
                landed = out.ground;
                break;
            }
        }
        assert!(landed.is_some());
        assert!(position.y.abs() < 1e-3, "{position}");
    }

    #[test]
    fn does_not_walk_up_a_steep_ramp() {
        let tilt = Quat::from_rotation_z(60f32.to_radians());
        let world = ParryWorld::new(&[
            floor(),
            BodyPlacement {
                rotation: tilt,
                ..block(Vec3::new(3.0, 0.0, 0.0), Vec3::new(2.0, 0.2, 5.0))
            },
        ]);
        let mut position = Vec3::ZERO;
        for _ in 0..60 {
            position =
                move_body(&world, &body(), position, Vec3::new(0.1, 0.0, 0.0), true).position;
        }
        assert!(position.y < 0.3, "{position}");
    }

    #[test]
    fn stops_rising_under_a_ceiling() {
        let world = ParryWorld::new(&[
            floor(),
            block(Vec3::new(0.0, 3.0, 0.0), Vec3::new(5.0, 0.5, 5.0)),
        ]);
        let out = move_body(&world, &body(), Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0), false);
        assert!(out.hit_ceiling);
        // The capsule's top (top + radius = 1.5 above the feet) stops at the ceiling (2.5).
        assert!((out.position.y - 1.0).abs() < 0.02, "{}", out.position);
    }
}
