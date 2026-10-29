//! The greybox test level (PLAN.md P1.4), as plain data.
//!
//! Each piece says what it is for, so the hero's collision and abilities (P1.5) can be
//! built from the same list that draws it. Sizes come from the movement figures in
//! `docs/research/hero-controller.md`: a standing jump covers about 5 u and rises 1.4–2.7
//! u, and a long jump from a full run covers about 11 u.
//!
//! Bevy's axes: y is up, and the player starts at the origin looking down −z.

use bevy::prelude::*;
use rac_combat::CrateKind;
use rac_hero::{GrindRail, Material, Water};

use crate::enemy::def::EnemyKind;

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
    /// A cable to hang from and slide down.
    ZipLine,
    /// A point to swing from with the swingshot.
    SwingTarget,
    /// Hurts the hero who stands on it.
    Hazard,
    /// A box that makes its centre the hero's new spawn point when he enters it.
    Checkpoint,
    /// Water to swim in, its top the surface.
    Water,
    /// A box that sells rounds for the gun of a hero standing in it (`rac_combat::Vendor`),
    /// shown as a pad at its centre.
    Vendor,
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

    /// Whether the hero collides with it. Rails, zip lines and swing targets wait for
    /// their moves, and checkpoints, water and vendors are only volumes.
    pub fn is_solid(self) -> bool {
        !matches!(
            self,
            Surface::GrindRail
                | Surface::ZipLine
                | Surface::SwingTarget
                | Surface::Checkpoint
                | Surface::Water
                | Surface::Vendor
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
    /// What the hero rides along, for a grind rail (its top, from start to end) or a zip
    /// line (its cable).
    pub fn rail(&self) -> Option<GrindRail> {
        match (self.surface, self.shape) {
            (Surface::GrindRail, Shape::Rail { start, end, radius }) => Some(GrindRail::new(vec![
                start + Vec3::Y * radius,
                end + Vec3::Y * radius,
            ])),
            (Surface::ZipLine, Shape::Rail { start, end, .. }) => {
                Some(GrindRail::zip_line(vec![start, end]))
            }
            _ => None,
        }
    }

    /// What the hero swims in, for water: the whole box.
    pub fn water(&self) -> Option<Water> {
        match (self.surface, self.shape) {
            (Surface::Water, Shape::Box { half_extents }) => Some(Water { half_extents }),
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
    // A zip line from the high platform, falling 11.2 u over 50 u along −z: jump under its
    // start to catch it (UYA's state 118, `hero-controller.md`, "Zip lines").
    let zip_start = Vec3::new(28.25, 14.2, -34.0);
    let zip_end = Vec3::new(28.25, 3.0, -84.0);
    pieces.push(Piece::new(
        "zip line",
        Surface::ZipLine,
        Shape::Rail {
            start: zip_start,
            end: zip_end,
            radius: 0.05,
        },
        (zip_start + zip_end) / 2.0,
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

    // A vendor's pad beside the melee yard (P1.10): standing on it tops up the gun in hand
    // for bolts.
    pieces.push(Piece::new(
        "vendor pad",
        Surface::Vendor,
        Shape::Box {
            half_extents: Vec3::new(1.5, 2.0, 1.5),
        },
        Vec3::new(-9.0, 0.0, 3.0),
    ));

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

    // A raised pool to swim in: water 3.5 u deep inside walls 4 u high, with steps up to
    // the near wall's top. A water jump clears the walls.
    let pool = Vec3::new(-50.0, 0.0, -30.0);
    let (inside, wall) = (12.0, 1.0);
    let span = inside + 2.0 * wall;
    for (side, x, z, size) in [
        (
            "near",
            0.0,
            (inside + wall) / 2.0,
            Vec3::new(span, 4.0, wall),
        ),
        (
            "far",
            0.0,
            -(inside + wall) / 2.0,
            Vec3::new(span, 4.0, wall),
        ),
        (
            "left",
            -(inside + wall) / 2.0,
            0.0,
            Vec3::new(wall, 4.0, inside),
        ),
        (
            "right",
            (inside + wall) / 2.0,
            0.0,
            Vec3::new(wall, 4.0, inside),
        ),
    ] {
        pieces.push(Piece::block(
            format!("pool wall {side}"),
            Surface::Ground,
            pool.x + x,
            pool.z + z,
            size,
        ));
    }
    stairs(
        &mut pieces,
        "pool step",
        pool.x,
        pool.z + span / 2.0 + 4.8,
        16,
        0.25,
        0.3,
    );
    pieces.push(Piece::new(
        "pool water",
        Surface::Water,
        Shape::Box {
            half_extents: Vec3::new(inside / 2.0, 1.75, inside / 2.0),
        },
        Vec3::new(pool.x, 1.75, pool.z),
    ));

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

    // The enemy arena beside the melee yard (`enemy_spawns`): cover low enough to jump
    // onto and to shoot over standing on it, which hides a crouching or far hero from the
    // shooters, and a pillar to break the chargers' line.
    for (name, x, z, size) in [
        ("arena cover west", 24.0, 18.0, Vec3::new(3.0, 1.2, 0.8)),
        ("arena cover east", 36.0, 18.0, Vec3::new(3.0, 1.2, 0.8)),
        ("arena cover north", 30.0, 26.0, Vec3::new(0.8, 1.2, 3.0)),
        ("arena pillar", 22.0, 30.0, Vec3::new(1.5, 2.5, 1.5)),
    ] {
        pieces.push(Piece::block(name, Surface::Ground, x, z, size));
    }

    pieces
}

/// Something in the melee yard to hit.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// Solid, and broken by one hit into what it holds (`rac_combat::Crate`).
    Crate(CrateKind),
    /// Pushed about by hits, and back to full health when knocked out.
    Dummy,
}

/// A target for the hero's wrench (P1.8), standing on the floor at `position`.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub name: String,
    pub kind: TargetKind,
    pub position: Vec3,
}

/// The crate's half size, and the dummy's radius and height.
pub const CRATE_HALF: f32 = 0.45;
pub const DUMMY_RADIUS: f32 = 0.35;
pub const DUMMY_HEIGHT: f32 = 1.6;

/// The melee yard behind the start: a row of bolt crates with two ammo crates, a nanotech
/// crate and a doubler beside it, two explosive crates past it, and a few training
/// dummies, spaced for combos, throws and jump attacks; and crates by the arena, to fill
/// up between fights.
pub fn melee_targets() -> Vec<Target> {
    let mut targets = Vec::new();
    let crate_at = |name: String, kind, x: f32, z: f32| Target {
        name,
        kind: TargetKind::Crate(kind),
        position: Vec3::new(x, 0.0, z),
    };
    for i in 0..5 {
        targets.push(crate_at(
            format!("bolt crate {i}"),
            CrateKind::Bolts,
            -4.0 + 2.0 * i as f32,
            8.0,
        ));
    }
    for (i, z) in [8.0, 10.0].into_iter().enumerate() {
        targets.push(crate_at(
            format!("ammo crate {i}"),
            CrateKind::Ammo,
            -8.0,
            z,
        ));
    }
    // West of the arena, in the first charger's sight.
    for (i, z) in [12.0, 14.0].into_iter().enumerate() {
        targets.push(crate_at(
            format!("arena ammo crate {i}"),
            CrateKind::Ammo,
            15.0,
            z,
        ));
    }
    targets.push(crate_at(
        "arena bolt crate".into(),
        CrateKind::Bolts,
        15.0,
        16.0,
    ));
    // Past the yard's ammo crates, a nanotech crate and a doubler; east of its bolt crates,
    // two explosive crates close enough for one's blast to set off the other, and two
    // more by the arena's crates.
    targets.push(crate_at(
        "nanotech crate".into(),
        CrateKind::Nanotech,
        -8.0,
        12.0,
    ));
    targets.push(crate_at(
        "doubler crate".into(),
        CrateKind::Doubler,
        -8.0,
        14.0,
    ));
    for (i, (x, z)) in [(7.0, 8.0), (8.3, 8.8), (16.0, 20.0), (17.2, 20.8)]
        .into_iter()
        .enumerate()
    {
        targets.push(crate_at(
            format!("explosive crate {i}"),
            CrateKind::Explosive,
            x,
            z,
        ));
    }
    for (i, x) in [-3.0, 0.0, 3.0].into_iter().enumerate() {
        targets.push(Target {
            name: format!("dummy {i}"),
            kind: TargetKind::Dummy,
            position: Vec3::new(x, 0.0, 14.0),
        });
    }
    // Far off, for the throw.
    targets.push(Target {
        name: "far dummy".into(),
        kind: TargetKind::Dummy,
        position: Vec3::new(0.0, 0.0, 22.0),
    });
    targets
}

/// An enemy of the arena (P1.11): where it stands and faces, and the points it walks
/// between.
#[derive(Debug, Clone, PartialEq)]
pub struct EnemySpawn {
    pub name: String,
    pub kind: EnemyKind,
    pub position: Vec3,
    pub yaw: f32,
    pub patrol: Vec<Vec3>,
}

/// The arena, from x 18 to 42 and z 10 to 34, right of the melee yard: three chargers
/// and two shooters, far enough from the start that none notices the hero there.
pub fn enemy_spawns() -> Vec<EnemySpawn> {
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    let spawn = |name: &str, kind, position: Vec3, yaw: f32, patrol: Vec<Vec3>| EnemySpawn {
        name: name.into(),
        kind,
        position,
        yaw: yaw.to_radians(),
        patrol,
    };
    vec![
        spawn(
            "charger 1",
            EnemyKind::Charger,
            at(26.0, 14.0),
            90.0,
            vec![at(22.0, 14.0), at(34.0, 14.0)],
        ),
        spawn(
            "charger 2",
            EnemyKind::Charger,
            at(34.0, 24.0),
            180.0,
            vec![at(34.0, 30.0), at(40.0, 30.0), at(34.0, 24.0)],
        ),
        spawn(
            "charger 3",
            EnemyKind::Charger,
            at(20.0, 22.0),
            90.0,
            vec![at(20.0, 26.0), at(20.0, 20.0)],
        ),
        spawn(
            "shooter 1",
            EnemyKind::Shooter,
            at(30.0, 32.0),
            0.0,
            vec![at(26.0, 32.0), at(36.0, 32.0)],
        ),
        spawn(
            "shooter 2",
            EnemyKind::Shooter,
            at(40.0, 12.0),
            90.0,
            vec![at(40.0, 20.0), at(40.0, 12.0)],
        ),
    ]
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
            Surface::ZipLine,
            Surface::SwingTarget,
            Surface::Hazard,
            Surface::Checkpoint,
            Surface::Water,
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
    fn the_melee_yard_is_clear_of_the_level() {
        let pieces = greybox_level();
        for target in melee_targets() {
            for piece in &pieces {
                let Shape::Box { half_extents } = piece.shape else {
                    continue;
                };
                if piece.name == "floor" || !piece.surface.is_solid() {
                    continue;
                }
                let gap = (target.position - piece.translation).abs() - half_extents;
                assert!(
                    gap.max_element() > 1.0,
                    "{} is in {}",
                    target.name,
                    piece.name
                );
            }
        }
    }

    #[test]
    fn the_arenas_enemies_walk_clear_of_the_level_and_out_of_reach_of_the_start() {
        let pieces = greybox_level();
        let bestiary = crate::Bestiary::default();
        for spawn in enemy_spawns() {
            let def = bestiary.get(spawn.kind);
            let mut points = vec![spawn.position];
            points.extend(&spawn.patrol);
            // Every point it stands on, and the way between each and the next.
            let mut route = Vec::new();
            for (i, from) in points.iter().enumerate() {
                let to = points[(i + 1) % points.len()];
                route.extend((0..=10).map(|step| from.lerp(to, step as f32 / 10.0)));
            }
            for at in route {
                assert!(
                    at.length() > def.senses.sight + 5.0,
                    "{} can see the start from {at}",
                    spawn.name
                );
                for piece in &pieces {
                    let Shape::Box { half_extents } = piece.shape else {
                        continue;
                    };
                    if piece.name == "floor" || !piece.surface.is_solid() {
                        continue;
                    }
                    let gap = (at - piece.translation).abs() - half_extents;
                    assert!(
                        gap.x.max(gap.z) > def.radius + 0.5,
                        "{} walks into {} at {at}",
                        spawn.name,
                        piece.name
                    );
                }
            }
        }
    }

    #[test]
    fn the_yard_and_the_arena_have_their_crates_and_the_start_a_vendor() {
        let crates = |kind: CrateKind, near: Vec3| {
            melee_targets()
                .into_iter()
                .filter(|target| {
                    target.kind == TargetKind::Crate(kind) && target.position.distance(near) < 12.0
                })
                .count()
        };
        let yard = Vec3::new(0.0, 0.0, 10.0);
        let arena = Vec3::new(20.0, 0.0, 14.0);
        for near in [yard, arena] {
            assert!(crates(CrateKind::Ammo, near) >= 2, "{near}");
            assert!(crates(CrateKind::Bolts, near) >= 1, "{near}");
            assert!(crates(CrateKind::Explosive, near) >= 2, "{near}");
        }
        assert!(crates(CrateKind::Nanotech, yard) >= 1);
        assert!(crates(CrateKind::Doubler, yard) >= 1);
        let pieces = greybox_level();
        let vendor = find(&pieces, "vendor pad");
        assert!(!vendor.surface.is_solid());
        assert!(vendor.translation.distance(Vec3::ZERO) < 12.0);
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
