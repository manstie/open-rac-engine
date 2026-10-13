//! The rig's building blocks, as corner points for [`crate::hull::convex_hull`].
//!
//! Positions are authored the way the reference measurements are written
//! (`docs/research/moby-format.md`): forward, left, up from the hero's feet, in metres.
//! [`at`] turns that into the hero's local space in Bevy (x right, y up, z back).

use bevy::math::{Vec2, Vec3};

/// Forward, left, up → the hero's local space.
pub const fn at(forward: f32, left: f32, up: f32) -> Vec3 {
    Vec3::new(-left, up, -forward)
}

/// A rectangle with its corners cut by `cut`, as eight points (four if `cut` is 0).
fn chamfered_rectangle(half: Vec2, cut: f32) -> Vec<Vec2> {
    if cut <= 0.0 {
        return vec![
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
        ];
    }
    let mut out = Vec::with_capacity(8);
    for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        out.push(Vec2::new(sx * half.x, sy * (half.y - cut)));
        out.push(Vec2::new(sx * (half.x - cut), sy * half.y));
    }
    out
}

/// A block running from `from` to `to`, with every edge chamfered by `bevel`. Its cross
/// section is a rectangle with half sizes `start` at `from` and `end` at `to`: x along
/// `across` (made square to the run), y along the third direction. A tiny end makes a
/// point, so the same shape gives beams, tapered limbs, wedges and spikes.
pub fn beam(from: Vec3, to: Vec3, across: Vec3, start: Vec2, end: Vec2, bevel: f32) -> Vec<Vec3> {
    let run = to - from;
    let length = run.length();
    let along = run / length;
    let across = (across - along * across.dot(along)).normalize();
    let deep = along.cross(across);
    let mut points = Vec::new();
    for (centre, half, inward) in [(from, start, along), (to, end, -along)] {
        let cut = bevel.min(half.x * 0.5).min(half.y * 0.5).min(length * 0.25);
        let rings = if cut > 0.0 {
            vec![
                (centre, chamfered_rectangle(half - Vec2::splat(cut), cut)),
                (centre + inward * cut, chamfered_rectangle(half, cut)),
            ]
        } else {
            vec![(centre, chamfered_rectangle(half, 0.0))]
        };
        for (ring_centre, ring) in rings {
            points.extend(ring.iter().map(|p| ring_centre + across * p.x + deep * p.y));
        }
    }
    points
}

/// An upright block centred at `centre`, `half` its half sizes (right, up, back), with
/// every edge chamfered by `bevel`.
pub fn block(centre: Vec3, half: Vec3, bevel: f32) -> Vec<Vec3> {
    let start = Vec2::new(half.x, half.z);
    beam(
        centre - Vec3::Y * half.y,
        centre + Vec3::Y * half.y,
        Vec3::X,
        start,
        start,
        bevel,
    )
}

/// [`block`] with its size given as forward, left, up half sizes.
pub fn block_at(centre: Vec3, forward: f32, left: f32, up: f32, bevel: f32) -> Vec<Vec3> {
    block(centre, Vec3::new(left, up, forward), bevel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hull::convex_hull;

    #[test]
    fn authored_axes_map_to_the_heros_local_space() {
        assert_eq!(at(1.0, 0.0, 0.0), Vec3::NEG_Z);
        assert_eq!(at(0.0, 1.0, 0.0), Vec3::NEG_X);
        assert_eq!(at(0.0, 0.0, 1.0), Vec3::Y);
    }

    #[test]
    fn a_chamfered_block_has_chamfered_faces() {
        let triangles = convex_hull(&block(Vec3::ZERO, Vec3::splat(1.0), 0.2));
        // Each end: an octagon (6 triangles) and its ring of eight bevels; then eight
        // sides between the ends.
        assert_eq!(triangles.len(), 2 * (6 + 8 * 2) + 8 * 2);
        for [a, b, c] in &triangles {
            for p in [a, b, c] {
                assert!(p.abs().max_element() <= 1.0 + 1e-4);
            }
        }
    }

    #[test]
    fn a_beam_runs_between_its_ends() {
        let points = beam(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::X,
            Vec2::new(0.5, 0.25),
            Vec2::splat(0.01),
            0.0,
        );
        assert_eq!(points.len(), 8);
        assert!(points[..4].iter().all(|p| p.z == 0.0 && p.x.abs() == 0.5));
        assert!(points[4..].iter().all(|p| p.z == -2.0));
    }
}
