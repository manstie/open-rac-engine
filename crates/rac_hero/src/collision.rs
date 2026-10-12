//! The hero's view of solid geometry (PLAN.md P1.16).
//!
//! The controller only asks a [`CollisionWorld`] to sweep capsules, spheres and rays, and
//! what each body's [`Material`] is, so a game or mash-up can put any geometry behind it.
//! [`ParryWorld`] is the bundled backend: a list of boxes and balls, queried with
//! `parry3d`.

use crate::rail::GrindRail;
use bevy::prelude::*;
use parry3d::{
    math::Pose,
    query::{Ray, ShapeCastOptions, cast_shapes, contact},
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

/// What a surface does to the hero beyond being solid, after the games' collision
/// materials (`hero-controller.md`, "Wall jump", "Ledges", "Getting hit and dying").
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Material {
    #[default]
    Solid,
    /// A wall-jump surface (the games' material 15): wall jumps off it go higher and can
    /// be chained without limit.
    WallJump,
    /// Can't be grabbed as a ledge or wall-jumped from (the games' materials 9, 10, 12).
    NoGrab,
    /// Hurts the hero standing on it (the games' material 17).
    Hazard,
    /// Kills the hero standing on it.
    Deadly,
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
    /// The first body the segment from `origin` to `origin + motion` enters. A segment
    /// that starts inside a body hits it at fraction 0.
    fn cast_ray(&self, origin: Vec3, motion: Vec3) -> Option<SweepHit>;
    /// Every body `capsule` overlaps at `centre`.
    fn capsule_penetrations(&self, capsule: CapsuleShape, centre: Vec3) -> Vec<Penetration>;
    /// How far `body` moved during the current tick, so the hero can ride it.
    fn body_motion(&self, body: BodyId) -> Vec3;
    fn material(&self, body: BodyId) -> Material;
    /// The rails the hero can grind along. A [`crate::rail::RailPoint`] names one by its
    /// index here.
    fn rails(&self) -> &[GrindRail] {
        &[]
    }
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
    pub material: Material,
}

impl BodyPlacement {
    /// A body that stays put and is plain solid.
    pub fn fixed(collider: Collider, translation: Vec3, rotation: Quat) -> Self {
        Self {
            collider,
            translation,
            rotation,
            motion: Vec3::ZERO,
            material: Material::Solid,
        }
    }
}

struct Body {
    pose: Pose,
    shape: Box<dyn Shape>,
    motion: Vec3,
    material: Material,
}

/// Brute force over a small list of bodies, which suits test levels of a few hundred
/// pieces. Bodies are checked in order and ties go to the first, so results don't
/// depend on anything but the list.
#[derive(Default)]
pub struct ParryWorld {
    bodies: Vec<Body>,
    rails: Vec<GrindRail>,
}

impl std::fmt::Debug for ParryWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParryWorld")
            .field("bodies", &self.bodies.len())
            .field("rails", &self.rails.len())
            .finish()
    }
}

impl ParryWorld {
    pub fn new(bodies: &[BodyPlacement]) -> Self {
        let mut world = Self::default();
        world.set_bodies(bodies);
        world
    }

    /// Replaces every rail.
    pub fn set_rails(&mut self, rails: Vec<GrindRail>) {
        self.rails = rails;
    }

    pub fn with_rails(mut self, rails: Vec<GrindRail>) -> Self {
        self.rails = rails;
        self
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
                Body {
                    pose: Pose::from_parts(body.translation, body.rotation),
                    shape,
                    motion: body.motion,
                    material: body.material,
                }
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
        for (index, body) in self.bodies.iter().enumerate() {
            let Ok(Some(hit)) = cast_shapes(
                &pose,
                motion,
                shape,
                &body.pose,
                Vec3::ZERO,
                body.shape.as_ref(),
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

    fn cast_ray(&self, origin: Vec3, motion: Vec3) -> Option<SweepHit> {
        if motion.length_squared() <= f32::EPSILON {
            return None;
        }
        // With the motion as the direction, time 1 is the segment's end.
        let ray = Ray::new(origin, motion);
        let mut best: Option<SweepHit> = None;
        for (index, body) in self.bodies.iter().enumerate() {
            let Some(hit) = body
                .shape
                .cast_ray_and_get_normal(&body.pose, &ray, 1.0, true)
            else {
                continue;
            };
            if best.is_none_or(|b| hit.time_of_impact < b.fraction) {
                best = Some(SweepHit {
                    fraction: hit.time_of_impact,
                    normal: hit.normal.normalize_or_zero(),
                    body: BodyId(index as u32),
                });
            }
        }
        best
    }

    fn capsule_penetrations(&self, capsule: CapsuleShape, centre: Vec3) -> Vec<Penetration> {
        let shape = Capsule::new_y(capsule.half_height, capsule.radius);
        let pose = Pose::from_translation(centre);
        self.bodies
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                let found = contact(&pose, &shape, &body.pose, body.shape.as_ref(), 0.0).ok()??;
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
            .map_or(Vec3::ZERO, |body| body.motion)
    }

    fn material(&self, body: BodyId) -> Material {
        self.bodies
            .get(body.0 as usize)
            .map_or(Material::Solid, |body| body.material)
    }

    fn rails(&self) -> &[GrindRail] {
        &self.rails
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor_and_wall() -> ParryWorld {
        ParryWorld::new(&[
            BodyPlacement::fixed(
                Collider::Cuboid {
                    half_extents: Vec3::new(10.0, 0.5, 10.0),
                },
                Vec3::new(0.0, -0.5, 0.0),
                Quat::IDENTITY,
            ),
            BodyPlacement {
                motion: Vec3::new(0.0, 0.0, 0.1),
                material: Material::WallJump,
                ..BodyPlacement::fixed(
                    Collider::Cuboid {
                        half_extents: Vec3::new(0.5, 2.0, 10.0),
                    },
                    Vec3::new(3.0, 2.0, 0.0),
                    Quat::IDENTITY,
                )
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
        let world = ParryWorld::new(&[BodyPlacement::fixed(
            Collider::Cuboid {
                half_extents: Vec3::new(5.0, 0.2, 5.0),
            },
            Vec3::ZERO,
            tilt,
        )]);
        let hit = world
            .cast_sphere(0.3, Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -6.0, 0.0))
            .unwrap();
        let expected = tilt * Vec3::Y;
        assert!((hit.normal - expected).length() < 1e-3, "{}", hit.normal);
    }

    #[test]
    fn rays_hit_the_nearest_face_and_report_its_material() {
        let world = floor_and_wall();
        let hit = world
            .cast_ray(Vec3::new(0.0, 1.0, 0.0), Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        // The wall's face is at x = 2.5.
        assert!((hit.fraction - 0.5).abs() < 1e-4, "{}", hit.fraction);
        assert!((hit.normal - Vec3::NEG_X).length() < 1e-4);
        assert_eq!(world.material(hit.body), Material::WallJump);
        assert_eq!(world.material(BodyId(0)), Material::Solid);
        // Too short to reach it.
        assert!(
            world
                .cast_ray(Vec3::new(0.0, 1.0, 0.0), Vec3::new(2.0, 0.0, 0.0))
                .is_none()
        );
        // Starting inside the floor hits it at once.
        let inside = world
            .cast_ray(Vec3::new(0.0, -0.2, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        assert_eq!(inside.fraction, 0.0);
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
