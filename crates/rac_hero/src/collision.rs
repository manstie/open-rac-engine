//! The hero's view of solid geometry (PLAN.md P1.16).
//!
//! The controller only asks a [`CollisionWorld`] to sweep capsules and spheres, so a game
//! or mash-up can put any geometry behind it. [`ParryWorld`] is the bundled backend: a
//! list of boxes and balls, queried with `parry3d`.

use bevy::prelude::*;
use parry3d::{
    math::Pose,
    query::{ShapeCastOptions, cast_shapes, contact},
    shape::{Ball, Capsule, Cuboid, Shape},
};

/// Identifies a body in a [`CollisionWorld`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(pub u32);

/// A vertical capsule: two sphere centres `half_height` above and below its centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapsuleShape {
    pub half_height: f32,
    pub radius: f32,
}

/// Where a sweep first touched something.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepHit {
    /// How far along the motion the shape got before touching, from 0 to 1.
    pub fraction: f32,
    /// The surface normal, pointing out of the obstacle towards the swept shape.
    pub normal: Vec3,
    pub body: BodyId,
}

/// A body the swept shape already overlaps, and how to get out of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Penetration {
    /// The direction to move the shape to separate it.
    pub normal: Vec3,
    pub depth: f32,
    pub body: BodyId,
}

pub trait CollisionWorld: Send + Sync {
    /// The first body hit by `capsule` moving from `centre` by `motion`.
    fn cast_capsule(&self, capsule: CapsuleShape, centre: Vec3, motion: Vec3) -> Option<SweepHit>;
    /// The first body hit by a sphere moving from `centre` by `motion`.
    fn cast_sphere(&self, radius: f32, centre: Vec3, motion: Vec3) -> Option<SweepHit>;
    /// Every body `capsule` overlaps at `centre`.
    fn capsule_penetrations(&self, capsule: CapsuleShape, centre: Vec3) -> Vec<Penetration>;
    /// How far `body` moved during the current tick, so the hero can ride it.
    fn body_motion(&self, body: BodyId) -> Vec3;
}

/// The shapes [`ParryWorld`] supports.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum Collider {
    Cuboid { half_extents: Vec3 },
    Ball { radius: f32 },
}

/// One body for [`ParryWorld::set_bodies`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyPlacement {
    pub collider: Collider,
    pub translation: Vec3,
    pub rotation: Quat,
    /// How far it moved this tick.
    pub motion: Vec3,
}

/// Brute force over a small list of bodies, which suits test levels of a few hundred
/// pieces. Bodies are checked in order and ties go to the first, so results don't
/// depend on anything but the list.
#[derive(Default)]
pub struct ParryWorld {
    bodies: Vec<(Pose, Box<dyn Shape>, Vec3)>,
}

impl std::fmt::Debug for ParryWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParryWorld")
            .field("bodies", &self.bodies.len())
            .finish()
    }
}

impl ParryWorld {
    pub fn new(bodies: &[BodyPlacement]) -> Self {
        let mut world = Self::default();
        world.set_bodies(bodies);
        world
    }

    /// Replaces every body. A body's [`BodyId`] is its index in `bodies`.
    pub fn set_bodies(&mut self, bodies: &[BodyPlacement]) {
        self.bodies = bodies
            .iter()
            .map(|body| {
                let shape: Box<dyn Shape> = match body.collider {
                    Collider::Cuboid { half_extents } => Box::new(Cuboid::new(half_extents)),
                    Collider::Ball { radius } => Box::new(Ball::new(radius)),
                };
                (
                    Pose::from_parts(body.translation, body.rotation),
                    shape,
                    body.motion,
                )
            })
            .collect();
    }

    fn cast(&self, shape: &dyn Shape, centre: Vec3, motion: Vec3) -> Option<SweepHit> {
        let length = motion.length();
        if length <= f32::EPSILON {
            return None;
        }
        let pose = Pose::from_translation(centre);
        let options = ShapeCastOptions {
            // Casting with the full motion as the velocity makes time 1 the end of it.
            max_time_of_impact: 1.0,
            // Exact contact: callers keep their own margins.
            target_distance: 0.0,
            stop_at_penetration: false,
            compute_impact_geometry_on_penetration: true,
        };
        let mut best: Option<SweepHit> = None;
        for (index, (body_pose, body_shape, _)) in self.bodies.iter().enumerate() {
            let Ok(Some(hit)) = cast_shapes(
                &pose,
                motion,
                shape,
                body_pose,
                Vec3::ZERO,
                body_shape.as_ref(),
                options,
            ) else {
                continue;
            };
            // `normal1` is the swept shape's outward normal in its own frame, which is
            // the world's because the shape is never rotated; reversed, it points out of
            // the obstacle. (`normal2` would be in the obstacle's frame, wrong for ramps.)
            let normal = (-hit.normal1).normalize_or_zero();
            if normal == Vec3::ZERO {
                continue;
            }
            if best.is_none_or(|b| hit.time_of_impact < b.fraction) {
                best = Some(SweepHit {
                    fraction: hit.time_of_impact,
                    normal,
                    body: BodyId(index as u32),
                });
            }
        }
        best
    }
}

impl CollisionWorld for ParryWorld {
    fn cast_capsule(&self, capsule: CapsuleShape, centre: Vec3, motion: Vec3) -> Option<SweepHit> {
        self.cast(
            &Capsule::new_y(capsule.half_height, capsule.radius),
            centre,
            motion,
        )
    }

    fn cast_sphere(&self, radius: f32, centre: Vec3, motion: Vec3) -> Option<SweepHit> {
        self.cast(&Ball::new(radius), centre, motion)
    }

    fn capsule_penetrations(&self, capsule: CapsuleShape, centre: Vec3) -> Vec<Penetration> {
        let shape = Capsule::new_y(capsule.half_height, capsule.radius);
        let pose = Pose::from_translation(centre);
        self.bodies
            .iter()
            .enumerate()
            .filter_map(|(index, (body_pose, body_shape, _))| {
                let found = contact(&pose, &shape, body_pose, body_shape.as_ref(), 0.0).ok()??;
                (found.dist < 0.0).then(|| Penetration {
                    // `normal2` points out of the body, which is the way to push the hero.
                    normal: found.normal2,
                    depth: -found.dist,
                    body: BodyId(index as u32),
                })
            })
            .collect()
    }

    fn body_motion(&self, body: BodyId) -> Vec3 {
        self.bodies
            .get(body.0 as usize)
            .map_or(Vec3::ZERO, |(_, _, motion)| *motion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor_and_wall() -> ParryWorld {
        ParryWorld::new(&[
            BodyPlacement {
                collider: Collider::Cuboid {
                    half_extents: Vec3::new(10.0, 0.5, 10.0),
                },
                translation: Vec3::new(0.0, -0.5, 0.0),
                rotation: Quat::IDENTITY,
                motion: Vec3::ZERO,
            },
            BodyPlacement {
                collider: Collider::Cuboid {
                    half_extents: Vec3::new(0.5, 2.0, 10.0),
                },
                translation: Vec3::new(3.0, 2.0, 0.0),
                rotation: Quat::IDENTITY,
                motion: Vec3::new(0.0, 0.0, 0.1),
            },
        ])
    }

    #[test]
    fn a_falling_sphere_hits_the_floor_from_above() {
        let world = floor_and_wall();
        let hit = world
            .cast_sphere(0.5, Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, -4.0, 0.0))
            .unwrap();
        assert_eq!(hit.body, BodyId(0));
        // It falls 1.5 of the 4 units before touching.
        assert!((hit.fraction - 1.5 / 4.0).abs() < 0.01, "{}", hit.fraction);
        assert!((hit.normal - Vec3::Y).length() < 1e-3, "{}", hit.normal);
    }

    #[test]
    fn a_capsule_hits_the_wall_side_on() {
        let world = floor_and_wall();
        let capsule = CapsuleShape {
            half_height: 0.5,
            radius: 0.5,
        };
        let hit = world
            .cast_capsule(capsule, Vec3::new(0.0, 1.5, 0.0), Vec3::new(4.0, 0.0, 0.0))
            .unwrap();
        assert_eq!(hit.body, BodyId(1));
        // The wall's face is at x = 2.5, so the capsule's side reaches it after 2 units.
        assert!((hit.fraction - 0.5).abs() < 0.01, "{}", hit.fraction);
        assert!((hit.normal - Vec3::NEG_X).length() < 1e-3);
        assert_eq!(world.body_motion(hit.body), Vec3::new(0.0, 0.0, 0.1));
    }

    #[test]
    fn normals_on_a_tilted_ramp_are_in_world_space() {
        let tilt = Quat::from_rotation_x(30f32.to_radians());
        let world = ParryWorld::new(&[BodyPlacement {
            collider: Collider::Cuboid {
                half_extents: Vec3::new(5.0, 0.2, 5.0),
            },
            translation: Vec3::ZERO,
            rotation: tilt,
            motion: Vec3::ZERO,
        }]);
        let hit = world
            .cast_sphere(0.3, Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -6.0, 0.0))
            .unwrap();
        let expected = tilt * Vec3::Y;
        assert!((hit.normal - expected).length() < 1e-3, "{}", hit.normal);
    }

    #[test]
    fn misses_and_overlaps() {
        let world = floor_and_wall();
        assert!(
            world
                .cast_sphere(0.5, Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
                .is_none()
        );
        let capsule = CapsuleShape {
            half_height: 0.5,
            radius: 0.5,
        };
        let sunk = world.capsule_penetrations(capsule, Vec3::new(0.0, 0.8, 0.0));
        assert_eq!(sunk.len(), 1);
        assert!((sunk[0].depth - 0.2).abs() < 1e-3, "{}", sunk[0].depth);
        assert!((sunk[0].normal - Vec3::Y).length() < 1e-3);
    }
}
