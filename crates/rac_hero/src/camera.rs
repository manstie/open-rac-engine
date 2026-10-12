//! A basic third-person camera (the start of PLAN.md P1.7).
//!
//! It runs in the simulation, because the hero reads the stick relative to the camera's
//! yaw, as in the games: what the camera did last tick decides where "forward" is this
//! tick. Lag, automatic recentring and collision come with RE.4 (`docs/research/camera.md`).

use bevy::prelude::*;
use rac_input::PlayerInputs;
use rac_sim::SimTransform;

use crate::{Hero, HeroBody};

/// Follows the hero of the same player slot, orbiting a point at his chest.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FollowCamera {
    pub player: usize,
    /// Radians about +y; 0 looks down −z.
    pub yaw: f32,
    /// Radians; negative looks down.
    pub pitch: f32,
    /// From the orbit point to the camera.
    pub distance: f32,
    /// Height of the orbit point above the hero's feet.
    pub pivot_height: f32,
}

/// At rest the games' camera sits 4.64 u behind the hero and 2.0 u above his feet
/// (`camera.md`, "Default follow position" [code; measured in three savestates]). Orbiting
/// a point 1 u up, that is this distance and pitch.
const REST_BEHIND: f32 = 4.64;
const REST_ABOVE: f32 = 2.0;
const PIVOT_HEIGHT: f32 = 1.0;
const PITCH_MIN: f32 = -1.2;
const PITCH_MAX: f32 = 0.6;

impl Default for FollowCamera {
    fn default() -> Self {
        let rise = REST_ABOVE - PIVOT_HEIGHT;
        Self {
            player: 0,
            yaw: 0.0,
            pitch: -bevy::math::ops::atan2(rise, REST_BEHIND),
            distance: (REST_BEHIND * REST_BEHIND + rise * rise).sqrt(),
            pivot_height: PIVOT_HEIGHT,
        }
    }
}

impl FollowCamera {
    pub fn for_player(player: usize, yaw: f32) -> Self {
        Self {
            player,
            yaw,
            ..default()
        }
    }

    /// Where the camera is and how it is turned, around a hero standing at `feet`.
    pub fn placement(&self, feet: Vec3) -> (Vec3, Quat) {
        let rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0);
        let pivot = feet + Vec3::Y * self.pivot_height;
        (pivot + rotation * Vec3::Z * self.distance, rotation)
    }
}

pub(crate) fn follow(
    inputs: Res<PlayerInputs>,
    heroes: Query<(&Hero, &HeroBody)>,
    mut cameras: Query<(&mut FollowCamera, &mut SimTransform)>,
) {
    for (mut camera, mut transform) in &mut cameras {
        let look = inputs.get(camera.player).look_radians();
        // Turning right is a negative turn about +y.
        camera.yaw -= look.x;
        camera.pitch = (camera.pitch + look.y).clamp(PITCH_MIN, PITCH_MAX);
        let Some((_, body)) = heroes.iter().find(|(hero, _)| hero.player == camera.player) else {
            continue;
        };
        let (translation, rotation) = camera.placement(body.0.position);
        transform.translation = translation;
        transform.rotation = rotation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rests_behind_and_above_the_hero() {
        let camera = FollowCamera::default();
        let (position, rotation) = camera.placement(Vec3::new(10.0, 5.0, 0.0));
        assert!(
            (position - Vec3::new(10.0, 7.0, 4.64)).length() < 1e-4,
            "{position}"
        );
        // It looks at the orbit point.
        let towards_pivot = (Vec3::new(10.0, 6.0, 0.0) - position).normalize();
        assert!((rotation * Vec3::NEG_Z - towards_pivot).length() < 1e-4);
    }
}
