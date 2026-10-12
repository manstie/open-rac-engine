//! The greybox test level (PLAN.md P1.4), as plain data.
//!
//! Each piece says what it is for, so the hero's collision and abilities (P1.5) can be
//! built from the same list that draws it. Sizes come from the movement figures in
//! `docs/research/hero-controller.md`: a standing jump covers about 5 u and rises 1.4–2.7
//! u, and a long jump from a full run covers about 11 u.
//!
//! Bevy's axes: y is up, and the player starts at the origin looking down −z.

use bevy::prelude::*;
use rac_hero::{GrindRail, Material};

/// What a piece is for.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    /// Walkable: the floor, slopes, steps and platform tops.
    Ground,
    /// Blocks for ledge grabs: their top edges can be hung from.
    Ledge,
    /// Tall walls facing each other, for wall jumps.
    WallJump,
    /// A platform that moves along a path.
    Moving,
    /// A rail to grind along.
    GrindRail,
    /// A point to swing from with the swingshot.
    SwingTarget,
    /// Hurts the hero who stands on it.
    Hazard,
    /// A box that makes its centre the hero's new spawn point when he enters it.
    Checkpoint,
}

impl Surface {
    /// What the hero's collision makes of it.
    pub fn material(self) -> Material {
        match self {
            Surface::WallJump => Material::WallJump,
            Surface::Hazard => Material::Hazard,
            _ => Material::Solid,
        }
    }

    /// Whether the hero collides with it. Rails and swing targets wait for their moves,
    /// and checkpoints are only volumes.
    pub fn is_solid(self) -> bool {
        !matches!(
            self,
            Surface::GrindRail | Surface::SwingTarget | Surface::Checkpoint
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// An axis-aligned box before the piece's rotation.
    Box {
        half_extents: Vec3,
    },
    /// A thin cylinder from `start` to `end`, in world space.
    Rail {
        start: Vec3,
        end: Vec3,
        radius: f32,
    },
    Sphere {
        radius: f32,
    },
}

/// Moves a piece back and forth between two points, easing at each end. Its position
/// depends only on the tick, so every peer agrees on it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Mover {
    pub from: Vec3,
    pub to: Vec3,
    /// Ticks for a full trip there and back.
    pub period_ticks: u32,
}

impl Mover {
    pub fn position_at(&self, tick: u64) -> Vec3 {
        let period = u64::from(self.period_ticks.max(2));
        let phase = (tick % period) as f32 / period as f32;
        // 0 → 1 → 0 over the period, smoothed so the platform eases in and out.
        let s = 1.0 - (2.0 * phase - 1.0).abs();
        let eased = s * s * (3.0 - 2.0 * s);
        self.from.lerp(self.to, eased)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub name: String,
    pub surface: Surface,
    pub shape: Shape,
    pub translation: Vec3,
    pub rotation: Quat,
    pub mover: Option<Mover>,
}

impl Piece {
    /// What the hero grinds along, for a grind rail: its top, from start to end.
    pub fn grind_rail(&self) -> Option<GrindRail> {
        match (self.surface, self.shape) {
            (Surface::GrindRail, Shape::Rail { start, end, radius }) => Some(GrindRail::new(vec![
                start + Vec3::Y * radius,
                end + Vec3::Y * radius,
            ])),
            _ => None,
        }
    }

    fn new(name: impl Into<String>, surface: Surface, shape: Shape, translation: Vec3) -> Self {
        Self {
            name: name.into(),
            surface,
            shape,
            translation,
            rotation: Quat::IDENTITY,
            mover: None,
        }
    }

    /// A box resting on the floor, its footprint centred at (x, z).
    fn block(name: impl Into<String>, surface: Surface, x: f32, z: f32, size: Vec3) -> Self {
        Self::new(
            name,
            surface,
            Shape::Box {
                half_extents: size / 2.0,
            },
            Vec3::new(x, size.y / 2.0, z),
        )
    }

    /// The world-space height of the top of a box piece.
    pub fn top(&self) -> Option<f32> {
        match self.shape {
            Shape::Box { half_extents } if self.rotation == Quat::IDENTITY => {
                Some(self.translation.y + half_extents.y)
            }
            _ => None,
        }
    }
}

/// A ramp rising `rise` along −z from its foot at (x, 0, z), with a landing block at the
/// top so it leads somewhere.
fn ramp(pieces: &mut Vec<Piece>, x: f32, z: f32, degrees: f32, rise: f32, width: f32) {
    const THICKNESS: f32 = 0.4;
    let angle = degrees.to_radians();
    let run = rise / angle.tan();
    let length = rise / angle.sin();
    // Tilting about +x by a positive angle raises the −z end.
    let rotation = Quat::from_rotation_x(angle);
    let surface_centre = Vec3::new(x, rise / 2.0, z - run / 2.0);
    let centre = surface_centre - rotation * Vec3::Y * (THICKNESS / 2.0);
    pieces.push(Piece {
        rotation,
        ..Piece::new(
            format!("{degrees}° ramp"),
            Surface::Ground,
            Shape::Box {
                half_extents: Vec3::new(width / 2.0, THICKNESS / 2.0, length / 2.0),
            },
            centre,
        )
    });
    pieces.push(Piece::block(
        format!("{degrees}° ramp landing"),
        Surface::Ground,
        x,
        z - run - 2.0,
        Vec3::new(width, rise, 4.0),
    ));
}

/// `count` steps of `rise` × `depth` climbing along −z from (x, z).
fn stairs(pieces: &mut Vec<Piece>, name: &str, x: f32, z: f32, count: u32, rise: f32, depth: f32) {
    for step in 1..=count {
        let height = rise * step as f32;
        pieces.push(Piece::block(
            format!("{name} {step}"),
            Surface::Ground,
            x,
            z - depth * (step as f32 - 0.5),
            Vec3::new(3.0, height, depth),
        ));
    }
}

/// The widths of the gaps on the jump course, from the start, in units. 4 u needs a jump,
/// 7 u a double jump or flip, and 10 u a long jump.
pub const GAP_WIDTHS: [f32; 3] = [4.0, 7.0, 10.0];

/// Every piece of the greybox level.
pub fn greybox_level() -> Vec<Piece> {
    let mut pieces = Vec::new();

    pieces.push(Piece::new(
        "floor",
        Surface::Ground,
        Shape::Box {
            half_extents: Vec3::new(80.0, 0.5, 80.0),
        },
        Vec3::new(0.0, -0.5, -40.0),
    ));

    // Slopes, from easy to too steep to walk up.
    for (i, degrees) in [15.0, 30.0, 45.0, 60.0].into_iter().enumerate() {
        ramp(
            &mut pieces,
            -34.0 + 6.0 * i as f32,
            -10.0,
            degrees,
            3.0,
            4.0,
        );
    }

    // Steps: low ones to walk up, and high ones that need a step-up limit or a jump.
    stairs(&mut pieces, "low step", 12.0, -10.0, 8, 0.25, 0.6);
    stairs(&mut pieces, "high step", 18.0, -10.0, 4, 0.5, 0.8);

    // Blocks to grab the ledges of, below, at and above jump height.
    for (i, height) in [2.0, 3.0, 4.0].into_iter().enumerate() {
        pieces.push(Piece::block(
            format!("{height} u ledge"),
            Surface::Ledge,
            -30.0 + 6.0 * i as f32,
            -36.0,
            Vec3::new(4.0, height, 4.0),
        ));
    }

    // A wall-jump shaft: two wall-jump surfaces 3 u apart. The right one is the high
    // platform's edge, and the left one stands taller, so the last jump off it carries
    // the hero over onto the platform.
    pieces.push(Piece::block(
        "wall-jump wall left",
        Surface::WallJump,
        20.0,
        -36.0,
        Vec3::new(1.5, 15.0, 6.0),
    ));
    pieces.push(Piece::block(
        "wall-jump wall right",
        Surface::WallJump,
        24.5,
        -36.0,
        Vec3::new(1.5, 12.0, 6.0),
    ));
    pieces.push(Piece::block(
        "high platform",
        Surface::Ground,
        28.25,
        -36.0,
        Vec3::new(6.0, 12.0, 6.0),
    ));
    pieces.push(Piece::new(
        "high platform checkpoint",
        Surface::Checkpoint,
        Shape::Box {
            half_extents: Vec3::new(2.5, 2.0, 2.5),
        },
        Vec3::new(28.5, 12.0, -36.0),
    ));
    // A lift to the high platform for the way back up.
    pieces.push(Piece {
        mover: Some(Mover {
            from: Vec3::new(33.0, 0.25, -36.0),
            to: Vec3::new(33.0, 12.25, -36.0),
            period_ticks: 8 * 60,
        }),
        ..Piece::new(
            "lift",
            Surface::Moving,
            Shape::Box {
                half_extents: Vec3::new(1.5, 0.25, 1.5),
            },
            Vec3::new(33.0, 0.25, -36.0),
        )
    });

    // A pad that hurts, to try getting hit.
    pieces.push(Piece::new(
        "hazard pad",
        Surface::Hazard,
        Shape::Box {
            half_extents: Vec3::new(2.0, 0.05, 2.0),
        },
        Vec3::new(6.0, 0.05, -24.0),
    ));

    // The jump course: 2 u platforms separated by wider and wider gaps.
    let mut z = -50.0;
    pieces.push(Piece::block(
        "course start",
        Surface::Ground,
        0.0,
        z - 3.0,
        Vec3::new(4.0, 2.0, 6.0),
    ));
    z -= 6.0;
    for (i, gap) in GAP_WIDTHS.into_iter().enumerate() {
        z -= gap;
        pieces.push(Piece::block(
            format!("course platform {}", i + 1),
            Surface::Ground,
            0.0,
            z - 3.0,
            Vec3::new(4.0, 2.0, 6.0),
        ));
        z -= 6.0;
    }
    pieces.push(Piece::new(
        "course checkpoint",
        Surface::Checkpoint,
        Shape::Box {
            half_extents: Vec3::new(2.0, 2.0, 3.0),
        },
        Vec3::new(0.0, 2.0, z + 3.0),
    ));

    // A shuttle across a gap too wide to jump, between two 3 u towers.
    for (end, z) in [("near", -52.0), ("far", -76.0)] {
        pieces.push(Piece::block(
            format!("shuttle tower {end}"),
            Surface::Ground,
            -24.0,
            z,
            Vec3::new(5.0, 3.0, 5.0),
        ));
    }
    pieces.push(Piece {
        mover: Some(Mover {
            from: Vec3::new(-24.0, 2.75, -56.5),
            to: Vec3::new(-24.0, 2.75, -71.5),
            period_ticks: 10 * 60,
        }),
        ..Piece::new(
            "shuttle",
            Surface::Moving,
            Shape::Box {
                half_extents: Vec3::new(2.0, 0.25, 2.0),
            },
            Vec3::new(-24.0, 2.75, -56.5),
        )
    });

    // A grind rail on posts, climbing from 1.2 u to 4 u.
    let rail_start = Vec3::new(36.0, 1.2, -4.0);
    let rail_end = Vec3::new(36.0, 4.0, -44.0);
    pieces.push(Piece::new(
        "grind rail",
        Surface::GrindRail,
        Shape::Rail {
            start: rail_start,
            end: rail_end,
            radius: 0.08,
        },
        (rail_start + rail_end) / 2.0,
    ));
    for i in 0..=4 {
        let at = rail_start.lerp(rail_end, i as f32 / 4.0);
        pieces.push(Piece::block(
            format!("rail post {i}"),
            Surface::Ground,
            at.x,
            at.z,
            Vec3::new(0.2, at.y - 0.1, 0.2),
        ));
    }

    // A swingshot target high over a gap between two 4 u platforms.
    for (end, z) in [("near", -84.0), ("far", -104.0)] {
        pieces.push(Piece::block(
            format!("swing platform {end}"),
            Surface::Ground,
            14.0,
            z,
            Vec3::new(6.0, 4.0, 6.0),
        ));
    }
    pieces.push(Piece::new(
        "swing target",
        Surface::SwingTarget,
        Shape::Sphere { radius: 0.5 },
        Vec3::new(14.0, 12.0, -94.0),
    ));

    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(pieces: &'a [Piece], name: &str) -> &'a Piece {
        pieces
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no piece named {name}"))
    }

    fn z_extent(piece: &Piece) -> (f32, f32) {
        let Shape::Box { half_extents } = piece.shape else {
            panic!("{} is not a box", piece.name)
        };
        (
            piece.translation.z - half_extents.z,
            piece.translation.z + half_extents.z,
        )
    }

    #[test]
    fn has_everything_p1_4_asks_for() {
        let pieces = greybox_level();
        for surface in [
            Surface::Ground,
            Surface::Ledge,
            Surface::WallJump,
            Surface::Moving,
            Surface::GrindRail,
            Surface::SwingTarget,
            Surface::Hazard,
            Surface::Checkpoint,
        ] {
            assert!(
                pieces.iter().any(|p| p.surface == surface),
                "no {surface:?} piece"
            );
        }
        assert!(pieces.iter().any(|p| p.name.contains("ramp")));
        assert!(pieces.iter().any(|p| p.name.contains("step")));
        assert!(find(&pieces, "high platform").top().unwrap() >= 10.0);
    }

    #[test]
    fn course_gaps_have_their_widths() {
        let pieces = greybox_level();
        let mut previous = find(&pieces, "course start");
        for (i, gap) in GAP_WIDTHS.into_iter().enumerate() {
            let next = find(&pieces, &format!("course platform {}", i + 1));
            // The course runs along −z, so the gap is from the previous platform's far
            // (low z) edge to the next one's near (high z) edge.
            let width = z_extent(previous).0 - z_extent(next).1;
            assert!((width - gap).abs() < 1e-4, "gap {i} is {width}, not {gap}");
            previous = next;
        }
    }

    #[test]
    fn ramps_rise_at_their_angle_and_meet_their_landings() {
        let pieces = greybox_level();
        for degrees in [15.0f32, 30.0, 45.0, 60.0] {
            let ramp = find(&pieces, &format!("{degrees}° ramp"));
            let Shape::Box { half_extents } = ramp.shape else {
                unreachable!()
            };
            // The top surface's far edge is at the landing's height.
            let far_top =
                ramp.translation + ramp.rotation * Vec3::new(0.0, half_extents.y, -half_extents.z);
            let landing = find(&pieces, &format!("{degrees}° ramp landing"));
            assert!((far_top.y - landing.top().unwrap()).abs() < 1e-4);
            let normal = ramp.rotation * Vec3::Y;
            assert!((normal.angle_between(Vec3::Y).to_degrees() - degrees).abs() < 1e-3);
        }
    }

    #[test]
    fn movers_shuttle_between_their_ends() {
        let mover = Mover {
            from: Vec3::ZERO,
            to: Vec3::new(0.0, 10.0, 0.0),
            period_ticks: 100,
        };
        assert_eq!(mover.position_at(0), Vec3::ZERO);
        assert_eq!(mover.position_at(50), Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(mover.position_at(100), Vec3::ZERO);
        assert!(mover.position_at(25).y > 0.0 && mover.position_at(25).y < 10.0);
    }

    #[test]
    fn fixed_boxes_do_not_overlap() {
        // Touching is fine; intersecting means a layout mistake, like a platform standing
        // in the middle of a jump gap.
        let boxes: Vec<_> = greybox_level()
            .into_iter()
            .filter(|p| {
                p.name != "floor"
                    && p.surface.is_solid()
                    && p.mover.is_none()
                    && p.rotation == Quat::IDENTITY
            })
            .filter_map(|p| match p.shape {
                Shape::Box { half_extents } => Some((p.name, p.translation, half_extents)),
                _ => None,
            })
            .collect();
        for (i, (name_a, centre_a, half_a)) in boxes.iter().enumerate() {
            for (name_b, centre_b, half_b) in &boxes[i + 1..] {
                let gap = (*centre_a - *centre_b).abs() - (*half_a + *half_b);
                assert!(gap.max_element() > -1e-3, "{name_a} overlaps {name_b}");
            }
        }
    }

    #[test]
    fn the_wall_jump_shaft_ends_on_the_high_platform() {
        let pieces = greybox_level();
        let left = find(&pieces, "wall-jump wall left");
        let right = find(&pieces, "wall-jump wall right");
        let platform = find(&pieces, "high platform");
        assert!(left.top().unwrap() > right.top().unwrap());
        assert_eq!(right.top(), platform.top());
        let half_x = |p: &Piece| match p.shape {
            Shape::Box { half_extents } => half_extents.x,
            _ => unreachable!(),
        };
        let gap = (right.translation.x - half_x(right)) - (left.translation.x + half_x(left));
        assert!((gap - 3.0).abs() < 1e-4, "{gap}");
        // The platform starts where the right wall ends.
        let joint =
            (platform.translation.x - half_x(platform)) - (right.translation.x + half_x(right));
        assert!(joint.abs() < 1e-4, "{joint}");
    }

    #[test]
    fn names_are_unique() {
        let pieces = greybox_level();
        let mut names: Vec<_> = pieces.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
