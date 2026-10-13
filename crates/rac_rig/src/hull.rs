//! Convex hulls of small point sets. Every block of the rig is the hull of a handful of
//! points, so a shape is described by its corners and the faces come for free, each one
//! flat.

use bevy::math::{DVec3, Vec3};

/// The hull's triangles, wound counter-clockwise seen from outside. Fewer than four
/// points, or points all in one plane, have no hull and give none.
///
/// Every point is first nudged by a tiny fixed amount (a hundred-thousandth of the set's
/// size), so no four are exactly coplanar: a box's faces then each come out as two
/// triangles with the same normal to the eye, and the hull never has to decide which
/// coplanar point to keep.
pub fn convex_hull(points: &[Vec3]) -> Vec<[Vec3; 3]> {
    let mut unique: Vec<DVec3> = Vec::with_capacity(points.len());
    for point in points {
        let point = point.as_dvec3();
        if !unique.iter().any(|p| p.distance_squared(point) < 1e-12) {
            unique.push(point);
        }
    }
    if unique.len() < 4 {
        return Vec::new();
    }
    let (low, high) = unique.iter().fold(
        (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
        |(low, high), p| (low.min(*p), high.max(*p)),
    );
    let size = (high - low).length();
    if size == 0.0 {
        return Vec::new();
    }
    let points: Vec<DVec3> = unique
        .iter()
        .enumerate()
        .map(|(i, p)| *p + nudge(i) * size * 1e-5)
        .collect();
    let eps = size * 1e-12;

    // Anything flatter than a thousandth of its size is a flat set nudged, not a volume.
    let Some(simplex) = first_simplex(&points, size * 1e-3) else {
        return Vec::new();
    };
    let centre = simplex.iter().map(|&i| points[i]).sum::<DVec3>() / 4.0;
    let [a, b, c, d] = simplex;
    let mut faces: Vec<[usize; 3]> = [[a, b, c], [a, b, d], [a, c, d], [b, c, d]]
        .into_iter()
        .map(|face| {
            if normal(&points, face).dot(centre - points[face[0]]) > 0.0 {
                [face[0], face[2], face[1]]
            } else {
                face
            }
        })
        .collect();

    for index in 0..points.len() {
        if simplex.contains(&index) {
            continue;
        }
        let p = points[index];
        let (visible, kept): (Vec<[usize; 3]>, Vec<[usize; 3]>) = faces
            .iter()
            .partition(|&&face| normal(&points, face).dot(p - points[face[0]]) > eps);
        if visible.is_empty() {
            continue;
        }
        let edges: Vec<(usize, usize)> = visible
            .iter()
            .flat_map(|f| [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])])
            .collect();
        faces = kept;
        for &(from, to) in &edges {
            if !edges.contains(&(to, from)) {
                faces.push([from, to, index]);
            }
        }
    }

    faces
        .into_iter()
        .map(|face| face.map(|i| points[i].as_vec3()))
        .collect()
}

/// A fixed, well-spread offset for each point, in a unit cube.
fn nudge(index: usize) -> DVec3 {
    let i = index as f64 + 1.0;
    let fract = |x: f64| x - x.floor();
    DVec3::new(
        fract(i * 0.618_033_988_7),
        fract(i * 0.414_213_562_4 + 0.3),
        fract(i * 0.732_050_807_6 + 0.7),
    ) - 0.5
}

fn normal(points: &[DVec3], [a, b, c]: [usize; 3]) -> DVec3 {
    (points[b] - points[a]).cross(points[c] - points[a])
}

/// Four points spanning a volume: two far apart, the one farthest from their line, and
/// the one farthest from the plane of those three. `None` if the line or plane leaves
/// nothing more than `flat` away.
fn first_simplex(points: &[DVec3], flat: f64) -> Option<[usize; 4]> {
    let farthest = |score: &dyn Fn(DVec3) -> f64| -> (usize, f64) {
        points.iter().enumerate().map(|(i, &p)| (i, score(p))).fold(
            (0, f64::NEG_INFINITY),
            |best, next| {
                if next.1 > best.1 { next } else { best }
            },
        )
    };
    let a = 0;
    let (b, _) = farthest(&|p| p.distance_squared(points[a]));
    let line = (points[b] - points[a]).normalize_or_zero();
    let (c, off_line) = farthest(&|p| {
        let v = p - points[a];
        (v - line * v.dot(line)).length()
    });
    let plane = normal(points, [a, b, c]).normalize_or_zero();
    let (d, off_plane) = farthest(&|p| plane.dot(p - points[a]).abs());
    (off_line > flat && off_plane > flat).then_some([a, b, c, d])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_corners(half: Vec3) -> Vec<Vec3> {
        let mut out = Vec::new();
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    out.push(half * Vec3::new(x, y, z));
                }
            }
        }
        out
    }

    fn volume(triangles: &[[Vec3; 3]]) -> f32 {
        triangles
            .iter()
            .map(|[a, b, c]| a.dot(b.cross(*c)) / 6.0)
            .sum()
    }

    #[test]
    fn a_box_has_twelve_outward_triangles() {
        let triangles = convex_hull(&box_corners(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(triangles.len(), 12);
        assert!((volume(&triangles) - 48.0).abs() < 0.01);
        for [a, b, c] in &triangles {
            let centre = (*a + *b + *c) / 3.0;
            assert!((*b - *a).cross(*c - *a).dot(centre) > 0.0);
        }
    }

    #[test]
    fn points_inside_are_dropped() {
        let mut points = box_corners(Vec3::ONE);
        points.extend([Vec3::ZERO, Vec3::splat(0.5), Vec3::new(0.0, 0.0, 0.99)]);
        let triangles = convex_hull(&points);
        assert_eq!(triangles.len(), 12);
        assert!((volume(&triangles) - 8.0).abs() < 0.01);
    }

    #[test]
    fn flat_or_tiny_sets_have_no_hull() {
        assert!(convex_hull(&[Vec3::ZERO, Vec3::X, Vec3::Y]).is_empty());
        let square = [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)];
        assert!(convex_hull(&square).is_empty());
    }
}
