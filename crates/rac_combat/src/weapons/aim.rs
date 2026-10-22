//! What a gun aims at (`weapons.md`, "Gun aim assist" and "How guns aim with it").

use bevy::{math::ops, prelude::*};
use rac_hero::CollisionWorld;
use std::f32::consts::{PI, TAU};

use super::def::AimAssist;

/// Something a gun can hit, as the guns see it this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunTarget {
    pub entity: Entity,
    /// The middle of its hurtbox.
    pub centre: Vec3,
    pub radius: f32,
    /// It has hit points left, or none to lose.
    pub alive: bool,
    /// Its hit points now and at most, if it has any.
    pub health: Option<(f32, f32)>,
    /// It is an enemy ([`crate::Hostile`]), not a crate or other target.
    pub enemy: bool,
}

/// How far a sight line stops short of a target's middle, as the lock-on's does, since a
/// target can be solid.
const SIGHT_SHORT: f32 = 0.75;

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn yaw_of(direction: Vec3) -> f32 {
    ops::atan2(-direction.x, -direction.z)
}

fn pitch_of(direction: Vec3) -> f32 {
    let flat = Vec2::new(direction.x, direction.z).length();
    ops::atan2(direction.y, flat)
}

/// Whether `to` can be seen from `from`, stopping short of it.
pub fn in_sight(world: &dyn CollisionWorld, from: Vec3, to: Vec3) -> bool {
    let line = to - from;
    let length = line.length();
    if length <= SIGHT_SHORT {
        return true;
    }
    world
        .cast_ray(from, line * ((length - SIGHT_SHORT) / length))
        .is_none()
}

/// The gun aim assist's pick (`gun_aim_target`, 0x44CF68): the living target in sight from
/// `from` that scores best inside the assist's cone round `aim`, whose half-angles grow
/// from near to far as `near + (far − near) × (1 − (1 − d / range)³)`. The score is
/// `yaw off × 3 + pitch off × 0.2 × d + d × 0.1`.
pub fn assist_target(
    targets: &[GunTarget],
    from: Vec3,
    aim: Vec3,
    assist: &AimAssist,
    world: &dyn CollisionWorld,
) -> Option<usize> {
    assist_targets(targets, from, aim, assist, world, |_| true, 1)
        .first()
        .copied()
}

/// The assist's `most` best targets among those `wanted` lets through, best first: what
/// the guns that seek enemies or mark several ask `gun_aim_target` for (its callers pass
/// a list and a count). Only enemies, for those guns, as the game's search is.
pub fn assist_targets(
    targets: &[GunTarget],
    from: Vec3,
    aim: Vec3,
    assist: &AimAssist,
    world: &dyn CollisionWorld,
    wanted: impl Fn(&GunTarget) -> bool,
    most: usize,
) -> Vec<usize> {
    let aim_yaw = yaw_of(aim);
    let aim_pitch = pitch_of(aim);
    let mut scored: Vec<(f32, usize)> = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        if !target.alive || !wanted(target) {
            continue;
        }
        let to = target.centre - from;
        let distance = to.length();
        if distance < 0.1 || distance > assist.range {
            continue;
        }
        let out = 1.0 - distance / assist.range;
        let ease = 1.0 - out * out * out;
        let half = |near: f32, far: f32| (near + (far - near) * ease).to_radians();
        let yaw_off = wrap(yaw_of(to) - aim_yaw).abs();
        let pitch_off = (pitch_of(to) - aim_pitch).abs();
        if yaw_off > half(assist.near.0, assist.far.0)
            || pitch_off > half(assist.near.1, assist.far.1)
        {
            continue;
        }
        let score = yaw_off * 3.0 + pitch_off * 0.2 * distance + distance * 0.1;
        scored.push((score, index));
    }
    // Ties go to the first in the list, as the game's strict comparison keeps it.
    scored.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .map(|(_, index)| index)
        .filter(|&index| in_sight(world, from, targets[index].centre))
        .take(most)
        .collect()
}

/// A cone that takes in everything round, for the searches that look all round.
pub const ALL_ROUND: (f32, f32) = (180.0, 90.0);

/// `v` along the ground.
pub fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// `direction` turned towards `wanted` by at most `most` radians (both unit vectors).
pub fn bend(direction: Vec3, wanted: Vec3, most: f32) -> Vec3 {
    let angle = direction.angle_between(wanted);
    if angle <= most || angle < 1e-6 {
        return wanted;
    }
    let axis = direction.cross(wanted);
    if axis.length_squared() < 1e-12 {
        // Straight back: any axis square to it will do.
        let side = direction.any_orthonormal_vector();
        return Quat::from_axis_angle(side, most) * direction;
    }
    Quat::from_axis_angle(axis.normalize(), most) * direction
}

/// Where the view centres: the first thing its line from `eye` along `direction` meets,
/// within `range`, or the end of the line.
pub fn view_point(
    world: &dyn CollisionWorld,
    targets: &[GunTarget],
    eye: Vec3,
    direction: Vec3,
    range: f32,
) -> Vec3 {
    let line = direction * range;
    let world_hit = world.cast_ray(eye, line).map_or(1.0, |hit| hit.fraction);
    let target_hit = first_target(targets, eye, eye + line, |_| true).map_or(1.0, |(_, t)| t);
    eye + line * world_hit.min(target_hit)
}

/// The first target the segment from `a` to `b` enters, and how far along it (0 to 1),
/// among those `wanted` lets through.
pub fn first_target(
    targets: &[GunTarget],
    a: Vec3,
    b: Vec3,
    wanted: impl Fn(&GunTarget) -> bool,
) -> Option<(usize, f32)> {
    let line = b - a;
    let mut best: Option<(usize, f32)> = None;
    for (index, target) in targets.iter().enumerate() {
        if !wanted(target) {
            continue;
        }
        let Some(t) = segment_enters_ball(a, line, target.centre, target.radius) else {
            continue;
        };
        if best.is_none_or(|(_, nearest)| t < nearest) {
            best = Some((index, t));
        }
    }
    best
}

/// Where along `a + line × t`, t in 0..=1, the segment first touches the ball; 0 if it
/// starts inside.
fn segment_enters_ball(a: Vec3, line: Vec3, centre: Vec3, radius: f32) -> Option<f32> {
    let from_centre = a - centre;
    let c = from_centre.length_squared() - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let a2 = line.length_squared();
    if a2 < 1e-12 {
        return None;
    }
    let b = from_centre.dot(line);
    if b > 0.0 {
        return None;
    }
    let discriminant = b * b - a2 * c;
    if discriminant < 0.0 {
        return None;
    }
    let t = (-b - ops::sqrt(discriminant)) / a2;
    (t <= 1.0).then_some(t.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_hero::ParryWorld;

    fn target(index: u32, centre: Vec3) -> GunTarget {
        GunTarget {
            entity: Entity::from_raw_u32(index).unwrap(),
            centre,
            radius: 0.5,
            alive: true,
            health: None,
            enemy: true,
        }
    }

    const N60: AimAssist = AimAssist {
        range: 35.0,
        near: (45.0, 45.0),
        far: (3.0, 6.0),
    };

    #[test]
    fn the_assist_takes_the_target_nearest_the_aim_and_ignores_ones_outside_the_cone() {
        let world = ParryWorld::new(&[]);
        let targets = [
            // 20 u ahead and 2 u to the side: about 5.7° off, inside the cone's 6.3° there,
            // and nearer, which counts for more than the angle.
            target(1, Vec3::new(2.0, 0.0, -20.0)),
            // Straight ahead, further.
            target(2, Vec3::new(0.0, 0.0, -25.0)),
            // Far off to the side: outside the narrow far cone.
            target(3, Vec3::new(15.0, 0.0, -30.0)),
        ];
        let picked = assist_target(&targets, Vec3::ZERO, Vec3::NEG_Z, &N60, &world);
        assert_eq!(picked, Some(0));
        assert_eq!(
            assist_target(&targets[1..], Vec3::ZERO, Vec3::NEG_Z, &N60, &world),
            Some(0)
        );
        let behind = [target(4, Vec3::new(0.0, 0.0, 10.0))];
        assert_eq!(
            assist_target(&behind, Vec3::ZERO, Vec3::NEG_Z, &N60, &world),
            None
        );
    }

    #[test]
    fn close_by_the_cone_is_wide() {
        let world = ParryWorld::new(&[]);
        // 2 u away at 30° round: inside the cone's 38° there; at 30 u it is 3.1°.
        let side = Vec3::new(
            -ops::sin(30f32.to_radians()),
            0.0,
            -ops::cos(30f32.to_radians()),
        );
        let targets = [target(1, side * 2.0)];
        assert_eq!(
            assist_target(&targets, Vec3::ZERO, Vec3::NEG_Z, &N60, &world),
            Some(0)
        );
        let far = [target(1, side * 30.0)];
        assert_eq!(
            assist_target(&far, Vec3::ZERO, Vec3::NEG_Z, &N60, &world),
            None
        );
    }

    #[test]
    fn bending_stops_at_the_limit() {
        let bent = bend(Vec3::NEG_Z, Vec3::X, 30f32.to_radians());
        assert!((bent.angle_between(Vec3::NEG_Z) - 30f32.to_radians()).abs() < 1e-5);
        assert_eq!(bend(Vec3::NEG_Z, Vec3::NEG_Z, 0.1), Vec3::NEG_Z);
    }

    #[test]
    fn a_segment_finds_the_nearest_ball_it_enters() {
        let targets = [
            target(1, Vec3::new(0.0, 0.0, -10.0)),
            target(2, Vec3::new(0.0, 0.0, -5.0)),
        ];
        let (index, t) =
            first_target(&targets, Vec3::ZERO, Vec3::new(0.0, 0.0, -20.0), |_| true).unwrap();
        assert_eq!(index, 1);
        assert!((t - 4.5 / 20.0).abs() < 1e-6);
        assert!(
            first_target(&targets, Vec3::X * 3.0, Vec3::new(3.0, 0.0, -20.0), |_| {
                true
            })
            .is_none()
        );
    }
}
