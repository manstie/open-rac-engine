//! A free-flying camera driven by a player's input, for looking round the level until the
//! hero and its camera exist (P1.5, P1.7).

use bevy::prelude::*;
use rac_input::{Action, PlayerInputs};
use rac_sim::{SimTransform, TICK_HZ};

/// Flies with the movement stick, turns with the look input, rises with Jump and sinks
/// with Crouch. Holding Strafe flies faster.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct SpectatorCamera {
    /// Which player slot steers it.
    pub player: usize,
    /// Radians about +y; 0 looks down −z.
    pub yaw: f32,
    /// Radians up from level.
    pub pitch: f32,
    /// Units per second.
    pub speed: f32,
}

impl Default for SpectatorCamera {
    fn default() -> Self {
        Self {
            player: 0,
            yaw: 0.0,
            pitch: -0.25,
            speed: 10.0,
        }
    }
}

/// Keeps the camera from flipping over the top.
const PITCH_LIMIT: f32 = 1.5;
const FAST_FACTOR: f32 = 3.0;
/// Radians per second the look stick turns it at full tilt: UYA's follow camera's rate.
const STICK_TURN: f32 = 3.01;

pub(crate) fn fly(
    inputs: Res<PlayerInputs>,
    mut cameras: Query<(&mut SpectatorCamera, &mut SimTransform)>,
) {
    let dt = (1.0 / TICK_HZ) as f32;
    for (mut camera, mut transform) in &mut cameras {
        let input = inputs.get(camera.player);
        let look = input.look_radians() + input.look_vector() * STICK_TURN * dt;
        // Turning right is a negative turn about +y.
        camera.yaw -= look.x;
        camera.pitch = (camera.pitch + look.y).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);

        let stick = input.move_vector().clamp_length_max(1.0);
        let forward = transform.rotation * Vec3::NEG_Z;
        let right = transform.rotation * Vec3::X;
        let vertical = f32::from(i8::from(input.held(Action::Jump)))
            - f32::from(i8::from(input.held(Action::Crouch)));
        let mut speed = camera.speed;
        if input.held(Action::Strafe) {
            speed *= FAST_FACTOR;
        }
        let velocity = (forward * stick.y + right * stick.x + Vec3::Y * vertical) * speed;
        transform.translation += velocity * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use rac_input::{ANGLE_UNITS_PER_TURN, Buttons, PlayerInput};
    use rac_sim::{SimPlugin, SimSet};
    use std::time::Duration;

    fn app_with(input: PlayerInput) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / TICK_HZ,
            )))
            .add_systems(FixedUpdate, fly.in_set(SimSet::Camera));
        let mut inputs = PlayerInputs::default();
        inputs.0[0] = input;
        app.insert_resource(inputs);
        let camera = app
            .world_mut()
            .spawn((
                SpectatorCamera {
                    pitch: 0.0,
                    ..default()
                },
                SimTransform::default(),
            ))
            .id();
        app.update(); // Starts the clock.
        (app, camera)
    }

    #[test]
    fn flies_forward_at_its_speed() {
        let (mut app, camera) = app_with(PlayerInput {
            move_stick: [0, 127],
            ..default()
        });
        for _ in 0..60 {
            app.update();
        }
        let position = app.world().get::<SimTransform>(camera).unwrap().translation;
        // One second at 10 u/s down −z.
        assert!(
            (position - Vec3::new(0.0, 0.0, -10.0)).length() < 1e-3,
            "{position}"
        );
    }

    #[test]
    fn turns_right_and_rises() {
        let mut held = Buttons::NONE;
        held.insert(Action::Jump);
        let quarter_turn_per_second = (ANGLE_UNITS_PER_TURN / 4.0 / 60.0) as i16;
        let (mut app, camera) = app_with(PlayerInput {
            look: [quarter_turn_per_second, 0],
            held,
            ..default()
        });
        for _ in 0..60 {
            app.update();
        }
        let transform = app.world().get::<SimTransform>(camera).unwrap();
        let facing = transform.rotation * Vec3::NEG_Z;
        // About a quarter turn right from −z is +x.
        assert!(facing.x > 0.99, "{facing}");
        assert!((transform.translation.y - 10.0).abs() < 1e-3);
    }
}
