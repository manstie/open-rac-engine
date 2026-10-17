//! Camera volumes: boxes that retune the follow camera while the hero is inside them,
//! after UYA's class 2329 mobys and their override slots (`camera.md`, "Camera volumes:
//! the follow camera's override slots").

use bevy::prelude::*;

/// A box, centred on the entity's `Transform` and turned with it, that retunes the follow
/// camera while its hero is inside. Each value left `None` keeps the camera's own. The
/// overrides end on the first tick he is outside, and the camera springs back, at the
/// volume's gain.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CameraVolume {
    pub half_extents: Vec3,
    /// Among the volumes he is in, the highest priority wins, and on a tie the one he
    /// entered first. The games' volumes have 5.
    pub priority: i32,
    pub distance: Option<f32>,
    pub height: Option<f32>,
    pub look_height: Option<f32>,
    /// The arm's tilt in radians, positive raising the camera, while the look stick rests.
    pub tilt: Option<f32>,
    /// Replaces the share of the hero's movement the arm takes up. As in the games, it
    /// stays after the volume until the follow camera starts again.
    pub drag: Option<f32>,
}

impl CameraVolume {
    pub fn new(half_extents: Vec3) -> Self {
        Self {
            half_extents,
            priority: 5,
            distance: None,
            height: None,
            look_height: None,
            tilt: None,
            drag: None,
        }
    }

    /// Whether `point` is inside the box placed by `transform`.
    pub fn contains(&self, transform: &Transform, point: Vec3) -> bool {
        let local = transform.rotation.inverse() * (point - transform.translation);
        local.abs().cmple(self.half_extents).all()
    }
}

/// What the winning volume sets this tick.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct VolumeSettings {
    pub distance: Option<f32>,
    pub height: Option<f32>,
    pub look_height: Option<f32>,
    pub tilt: Option<f32>,
    pub drag: Option<f32>,
}

/// Updates `entered`, the volumes the hero is in in the order he entered them, and returns
/// what the winner sets. `volumes` are in entity order.
pub(super) fn settings(
    entered: &mut Vec<Entity>,
    hero: Vec3,
    volumes: &[(Entity, &CameraVolume, &Transform)],
) -> VolumeSettings {
    let inside = |entity: Entity| {
        volumes
            .iter()
            .find(|(candidate, ..)| *candidate == entity)
            .is_some_and(|(_, volume, transform)| volume.contains(transform, hero))
    };
    entered.retain(|entity| inside(*entity));
    for (entity, volume, transform) in volumes {
        if !entered.contains(entity) && volume.contains(transform, hero) {
            entered.push(*entity);
        }
    }
    let mut winner: Option<&CameraVolume> = None;
    for entity in entered.iter() {
        let Some((_, volume, _)) = volumes.iter().find(|(candidate, ..)| candidate == entity)
        else {
            continue;
        };
        if winner.is_none_or(|best| volume.priority > best.priority) {
            winner = Some(volume);
        }
    }
    winner.map_or_else(VolumeSettings::default, |volume| VolumeSettings {
        distance: volume.distance,
        height: volume.height,
        look_height: volume.look_height,
        tilt: volume.tilt,
        drag: volume.drag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_highest_priority_wins_then_the_first_entered() {
        let low = CameraVolume {
            distance: Some(8.0),
            ..CameraVolume::new(Vec3::splat(5.0))
        };
        let high = CameraVolume {
            priority: 6,
            distance: Some(3.0),
            ..CameraVolume::new(Vec3::splat(5.0))
        };
        let tie = CameraVolume {
            distance: Some(4.0),
            ..CameraVolume::new(Vec3::splat(1.0))
        };
        let at = Transform::default();
        let away = Transform::from_xyz(20.0, 0.0, 0.0);
        let (a, b, c) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
            Entity::from_raw_u32(3).unwrap(),
        );
        let mut entered = Vec::new();
        // Inside the low one and outside the high one.
        let volumes = [(a, &low, &at), (b, &high, &away)];
        assert_eq!(
            settings(&mut entered, Vec3::ZERO, &volumes).distance,
            Some(8.0)
        );
        let volumes = [(a, &low, &at), (b, &high, &at)];
        assert_eq!(
            settings(&mut entered, Vec3::ZERO, &volumes).distance,
            Some(3.0)
        );
        // A tie keeps the one entered first.
        let volumes = [(a, &low, &at), (c, &tie, &at)];
        assert_eq!(
            settings(&mut entered, Vec3::ZERO, &volumes).distance,
            Some(8.0)
        );
        assert_eq!(entered, vec![a, c]);
        // Outside all of them, nothing is set.
        assert_eq!(
            settings(&mut entered, Vec3::splat(9.0), &volumes),
            VolumeSettings::default()
        );
        assert!(entered.is_empty());
    }

    #[test]
    fn a_turned_volume_turns_its_box() {
        let volume = CameraVolume::new(Vec3::new(4.0, 1.0, 1.0));
        let turned = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        assert!(volume.contains(&Transform::default(), Vec3::new(3.0, 0.0, 0.0)));
        assert!(!volume.contains(&turned, Vec3::new(3.0, 0.0, 0.0)));
        assert!(volume.contains(&turned, Vec3::new(0.0, 0.0, 3.0)));
    }
}
