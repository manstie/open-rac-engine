//! Water the hero swims in (`docs/research/hero-controller.md`, "Swimming").
//!
//! The games mark water with collision polygons of their own material, and the ground
//! probe takes the surface height from the one under the hero. The kit's water is a box
//! whose top is the surface instead.

use bevy::prelude::*;

/// A body of water: a box centred on the entity's translation, unrotated, whose top face
/// is the surface.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Water {
    pub half_extents: Vec3,
}

/// A water box in world space, as [`crate::CollisionWorld::water`] lists it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterVolume {
    pub min: Vec3,
    pub max: Vec3,
}

/// How far above the feet the ground probe starts, so water up to this high over them
/// is found (`hero_ground_probe`'s first cast).
pub const PROBE_ABOVE: f32 = 2.2;

/// How far under its bottom a box still holds the feet: a hero standing on the floor the
/// box rests on sits a hair below it, and the games' probe, finding the surface from
/// above, has no bottom.
const BOTTOM_SLACK: f32 = 0.05;

impl WaterVolume {
    pub fn new(centre: Vec3, half_extents: Vec3) -> Self {
        Self {
            min: centre - half_extents,
            max: centre + half_extents,
        }
    }

    pub fn surface(&self) -> f32 {
        self.max.y
    }

    fn holds(&self, feet: Vec3) -> bool {
        (self.min.x..=self.max.x).contains(&feet.x)
            && (self.min.z..=self.max.z).contains(&feet.z)
            && (self.min.y - BOTTOM_SLACK..=self.max.y + PROBE_ABOVE).contains(&feet.y)
    }
}

/// The surface of the water a hero with his feet at `feet` is in or over: the highest of
/// the boxes around him whose surface is less than [`PROBE_ABOVE`] over his feet.
pub fn surface_at(volumes: &[WaterVolume], feet: Vec3) -> Option<f32> {
    volumes
        .iter()
        .filter(|volume| volume.holds(feet))
        .map(WaterVolume::surface)
        .reduce(f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_surface_is_found_from_below_and_a_little_above() {
        let pool = [WaterVolume::new(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(5.0, 1.0, 5.0),
        )];
        assert_eq!(surface_at(&pool, Vec3::new(1.0, 0.5, -2.0)), Some(2.0));
        assert_eq!(surface_at(&pool, Vec3::new(1.0, 4.0, -2.0)), Some(2.0));
        assert_eq!(surface_at(&pool, Vec3::new(1.0, 4.5, -2.0)), None);
        // Standing on the floor at its bottom.
        assert_eq!(surface_at(&pool, Vec3::new(1.0, -1e-6, -2.0)), Some(2.0));
        assert_eq!(surface_at(&pool, Vec3::new(6.0, 1.0, 0.0)), None);
        assert_eq!(surface_at(&pool, Vec3::new(0.0, -0.5, 0.0)), None);
    }

    #[test]
    fn of_two_boxes_the_higher_surface_wins() {
        let pools = [
            WaterVolume::new(Vec3::ZERO, Vec3::splat(2.0)),
            WaterVolume::new(Vec3::new(0.0, 1.0, 0.0), Vec3::splat(2.0)),
        ];
        assert_eq!(surface_at(&pools, Vec3::new(0.0, 1.0, 0.0)), Some(3.0));
    }
}
