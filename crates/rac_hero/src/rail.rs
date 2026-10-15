//! Rails (`docs/research/hero-controller.md`, "Grinding" and "Zip lines"): polylines the
//! hero rides along, one way only, from the first point to the last, as the games' rails
//! run along their splines. The games keep grind rails and zip lines in one list; which a
//! rail is depends on the level (UYA's are all zip lines), so here each rail says.

use bevy::{math::ops, prelude::*};

/// A rail to grind along, or a zip line's cable. The points run along its top, or along
/// the cable, in the direction of travel.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GrindRail {
    pub points: Vec<Vec3>,
    pub kind: RailKind,
}

/// What the hero does on a rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RailKind {
    /// He lands on it and grinds along it, with the grind boots.
    #[default]
    Grind,
    /// He catches it with his hands and slides along under it (UYA's state 118).
    ZipLine,
}

/// A place on one of a world's rails.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailPoint {
    /// The rail's index in [`crate::CollisionWorld::rails`].
    pub rail: usize,
    pub segment: usize,
    /// How far along the segment, from 0 to 1.
    pub t: f32,
}

impl GrindRail {
    pub fn new(points: Vec<Vec3>) -> Self {
        Self {
            points,
            kind: RailKind::Grind,
        }
    }

    /// A zip line's cable through `points`.
    pub fn zip_line(points: Vec<Vec3>) -> Self {
        Self {
            points,
            kind: RailKind::ZipLine,
        }
    }

    fn segments(&self) -> usize {
        self.points.len().saturating_sub(1)
    }

    pub fn position(&self, at: RailPoint) -> Vec3 {
        match self.segments() {
            0 => self.points.first().copied().unwrap_or(Vec3::ZERO),
            n => {
                let segment = at.segment.min(n - 1);
                self.points[segment].lerp(self.points[segment + 1], at.t)
            }
        }
    }

    /// The direction of travel at `at`.
    pub fn direction(&self, at: RailPoint) -> Vec3 {
        match self.segments() {
            0 => Vec3::ZERO,
            n => {
                let segment = at.segment.min(n - 1);
                (self.points[segment + 1] - self.points[segment]).normalize_or_zero()
            }
        }
    }

    /// Moves `at` along the rail by `distance`. Returns false when the rail ran out first,
    /// leaving `at` at its end.
    pub fn advance(&self, at: &mut RailPoint, distance: f32) -> bool {
        let segments = self.segments();
        if segments == 0 {
            return false;
        }
        let mut remaining = distance.max(0.0);
        loop {
            let length = self.points[at.segment].distance(self.points[at.segment + 1]);
            let left = (1.0 - at.t) * length;
            if remaining <= left && length > f32::EPSILON {
                at.t += remaining / length;
                return true;
            }
            remaining -= left;
            if at.segment + 1 >= segments {
                at.t = 1.0;
                return false;
            }
            at.segment += 1;
            at.t = 0.0;
        }
    }

    /// The rail's pitch over the next `distance`: positive climbing.
    pub fn pitch_ahead(&self, at: RailPoint, distance: f32) -> f32 {
        let start = self.position(at);
        let mut ahead = at;
        self.advance(&mut ahead, distance);
        let end = self.position(ahead);
        let run = Vec2::new(end.x - start.x, end.z - start.z).length();
        if run <= f32::EPSILON {
            return 0.0;
        }
        ops::atan2(end.y - start.y, run)
    }

    /// The point on the rail nearest `position`.
    pub fn nearest(&self, rail: usize, position: Vec3) -> RailPoint {
        let mut best = RailPoint {
            rail,
            segment: 0,
            t: 0.0,
        };
        let mut best_distance = f32::INFINITY;
        for segment in 0..self.segments() {
            let (a, b) = (self.points[segment], self.points[segment + 1]);
            let along = b - a;
            let t = if along.length_squared() > f32::EPSILON {
                ((position - a).dot(along) / along.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance = position.distance_squared(a + along * t);
            // Ties go to the earlier segment, so the result depends only on the rail.
            if distance < best_distance {
                best_distance = distance;
                best = RailPoint { rail, segment, t };
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bent() -> GrindRail {
        // 10 u along -z, then 10 u along +x, rising 1 u on the second leg.
        GrindRail::new(vec![
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(10.0, 1.0, -10.0),
        ])
    }

    #[test]
    fn advancing_crosses_segments_and_stops_at_the_end() {
        let rail = bent();
        let mut at = RailPoint {
            rail: 0,
            segment: 0,
            t: 0.5,
        };
        assert!(rail.advance(&mut at, 7.0));
        assert_eq!(at.segment, 1);
        assert!(rail.position(at).distance(Vec3::new(2.0, 0.2, -10.0)) < 0.05);
        assert!(!rail.advance(&mut at, 100.0));
        assert_eq!(rail.position(at), Vec3::new(10.0, 1.0, -10.0));
    }

    #[test]
    fn nearest_points_and_pitch() {
        let rail = bent();
        let at = rail.nearest(3, Vec3::new(1.0, 2.0, -4.0));
        assert_eq!((at.rail, at.segment), (3, 0));
        assert!((at.t - 0.4).abs() < 1e-6);
        assert_eq!(rail.direction(at), Vec3::NEG_Z);
        assert_eq!(rail.pitch_ahead(at, 0.3), 0.0);
        let climbing = rail.nearest(0, Vec3::new(5.0, 0.5, -10.0));
        assert!((rail.pitch_ahead(climbing, 0.3) - ops::atan2(1.0, 10.0)).abs() < 1e-3);
    }
}
