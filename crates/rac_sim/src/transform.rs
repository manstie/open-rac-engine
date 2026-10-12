//! Simulated positions and their interpolation for rendering.
//!
//! The simulation moves things 60 times a second, but the screen refreshes at its own rate.
//! Each simulated entity keeps where it was at the last two ticks, and each frame its
//! `Transform` is set part-way between them, by how far the clock is into the next tick.
//! Rendering therefore runs up to one tick behind the simulation, in exchange for smooth
//! motion at any frame rate.

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

/// Where a simulated entity is, as of the latest tick. Simulation systems read and write
/// this, never `Transform`, which belongs to rendering.
///
/// - Changing it through `&mut SimTransform` is ordinary movement, and is interpolated.
/// - Inserting it (`commands.entity(e).insert(..)`) is a teleport: the entity appears at
///   the new place on the next frame, without sliding there.
///
/// Interpolation writes the local `Transform`, so simulated entities should not have a
/// parent.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
#[require(Transform, PreviousSimTransform)]
#[component(on_insert = snap_to_current)]
pub struct SimTransform {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl SimTransform {
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            rotation: Quat::IDENTITY,
        }
    }

    pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self::from_translation(Vec3::new(x, y, z))
    }

    pub fn with_rotation(self, rotation: Quat) -> Self {
        Self { rotation, ..self }
    }

    /// The rendered transform `alpha` of the way from `previous` to `self`.
    pub fn interpolate_from(&self, previous: &SimTransform, alpha: f32) -> (Vec3, Quat) {
        (
            previous.translation.lerp(self.translation, alpha),
            previous.rotation.slerp(self.rotation, alpha),
        )
    }
}

/// [`SimTransform`] as it was at the end of the previous tick.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct PreviousSimTransform(pub SimTransform);

fn snap_to_current(mut world: DeferredWorld, context: HookContext) {
    let Some(current) = world.get::<SimTransform>(context.entity).copied() else {
        return;
    };
    if let Some(mut previous) = world.get_mut::<PreviousSimTransform>(context.entity) {
        previous.0 = current;
    }
    if let Some(mut transform) = world.get_mut::<Transform>(context.entity) {
        transform.translation = current.translation;
        transform.rotation = current.rotation;
    }
}

pub(crate) fn remember_previous(mut query: Query<(&SimTransform, &mut PreviousSimTransform)>) {
    for (current, mut previous) in &mut query {
        previous.0 = *current;
    }
}

pub(crate) fn interpolate(
    time: Res<Time<Fixed>>,
    mut query: Query<(&SimTransform, &PreviousSimTransform, &mut Transform)>,
) {
    let alpha = time.overstep_fraction();
    for (current, previous, mut transform) in &mut query {
        let (translation, rotation) = current.interpolate_from(&previous.0, alpha);
        transform.translation = translation;
        transform.rotation = rotation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_between_ticks() {
        let previous = SimTransform::from_xyz(0.0, 0.0, 0.0);
        let current = SimTransform::from_xyz(2.0, 4.0, -6.0);
        assert_eq!(
            current.interpolate_from(&previous, 0.25).0,
            Vec3::new(0.5, 1.0, -1.5)
        );
        assert_eq!(
            current.interpolate_from(&previous, 1.0).0,
            current.translation
        );
    }

    #[test]
    fn inserting_teleports_without_interpolation() {
        let mut world = World::new();
        let entity = world.spawn(SimTransform::from_xyz(1.0, 2.0, 3.0)).id();
        let entity_ref = world.entity(entity);
        assert_eq!(
            entity_ref.get::<PreviousSimTransform>().unwrap().0,
            SimTransform::from_xyz(1.0, 2.0, 3.0)
        );
        assert_eq!(
            entity_ref.get::<Transform>().unwrap().translation,
            Vec3::new(1.0, 2.0, 3.0)
        );

        world
            .entity_mut(entity)
            .insert(SimTransform::from_xyz(-5.0, 0.0, 0.0));
        assert_eq!(
            world
                .entity(entity)
                .get::<PreviousSimTransform>()
                .unwrap()
                .0,
            SimTransform::from_xyz(-5.0, 0.0, 0.0)
        );
    }
}
