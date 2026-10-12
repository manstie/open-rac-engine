//! The hero kit's input layer (PLAN.md P1.3, G3).
//!
//! - [`Action`]s are bound to keys, mouse buttons, the wheel and gamepad buttons through
//!   [`InputSettings`], which the app loads from and saves to the player's settings.
//! - Every simulation tick, [`PlayerInputPlugin`] fills [`PlayerInputs`] with one compact
//!   [`PlayerInput`] per player, in `rac_sim::SimSet::Input`. That is all the simulation
//!   reads, and all that replays record and the netcode sends.
//! - Menus read [`ActionState`] instead, once a frame.
//! - [`InputSource`] switches the simulation between the local devices and a replay of an
//!   [`InputRecording`]; an [`InputRecorder`] captures one.
//!
//! The crate has no dependency on the games' discs or formats.

mod action;
mod player_input;
mod recording;
mod sampling;
mod settings;

use bevy::{input::InputSystems, prelude::*};
use rac_sim::SimSet;

pub use action::{Action, Buttons};
pub use player_input::{ANGLE_UNITS_PER_TURN, LocalPlayer, MAX_PLAYERS, PlayerInput, PlayerInputs};
pub use recording::{InputRecorder, InputRecording, InputSource, RecordingError};
pub use sampling::{ActionState, LocalSampler, reset_inputs};
pub use settings::{Binding, InputSettings, MoveKeys, WheelDirection, apply_dead_zone};

/// Samples the local devices each frame and latches [`PlayerInputs`] each tick.
///
/// Insert [`InputSettings`] before adding the plugin to use saved settings; otherwise the
/// defaults apply.
pub struct PlayerInputPlugin;

impl Plugin for PlayerInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputSettings>()
            .init_resource::<ActionState>()
            .init_resource::<LocalSampler>()
            .init_resource::<PlayerInputs>()
            .init_resource::<LocalPlayer>()
            .init_resource::<InputSource>()
            .add_systems(PreUpdate, sampling::sample_devices.after(InputSystems))
            .add_systems(
                FixedUpdate,
                sampling::latch_tick_inputs.in_set(SimSet::Input),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
        time::TimeUpdateStrategy,
    };
    use rac_sim::{SimPlugin, TICK_HZ};
    use std::time::Duration;

    /// An app with one tick per update and no window. The device resources are added by
    /// hand rather than by Bevy's `InputPlugin`, which would clear the presses the tests
    /// make before the sampler saw them.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin, PlayerInputPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / TICK_HZ,
            )));
        app.update(); // Starts the clock.
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }

    fn release(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
    }

    fn local_input(app: &App) -> PlayerInput {
        app.world().resource::<PlayerInputs>().get(0)
    }

    #[test]
    fn keys_become_a_tick_input() {
        let mut app = app();
        press(&mut app, KeyCode::Space);
        press(&mut app, KeyCode::KeyW);
        press(&mut app, KeyCode::KeyD);
        app.update();
        let input = local_input(&app);
        assert!(input.held(Action::Jump));
        assert!(input.pressed(Action::Jump));
        // Keys push both axes fully, like a PS2 stick in a corner.
        assert_eq!(input.move_stick, [127, 127]);
        assert!(
            app.world()
                .resource::<ActionState>()
                .just_pressed(Action::UiAccept)
        );

        // Still held at the next tick, but no longer newly pressed.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        let input = local_input(&app);
        assert!(input.held(Action::Jump));
        assert!(!input.pressed(Action::Jump));

        release(&mut app, KeyCode::Space);
        app.update();
        assert!(!local_input(&app).held(Action::Jump));
    }

    #[test]
    fn a_replay_reproduces_a_recording() {
        let mut app = app();
        app.insert_resource(InputRecorder(InputRecording::new(2)));
        let script = [
            Some(KeyCode::Space),
            None,
            Some(KeyCode::KeyF),
            Some(KeyCode::KeyA),
            None,
        ];
        let mut live = Vec::new();
        for key in script {
            if let Some(key) = key {
                press(&mut app, key);
            }
            app.update();
            live.push(local_input(&app));
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release_all();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear();
        }
        let recording = app
            .world_mut()
            .remove_resource::<InputRecorder>()
            .unwrap()
            .0;
        assert_eq!(recording.ticks(), script.len());

        let mut replay = self::app();
        replay.insert_resource(InputSource::replay(recording));
        // Local keys must have no effect during a replay.
        press(&mut replay, KeyCode::Space);
        let mut replayed = Vec::new();
        for _ in 0..script.len() {
            replay.update();
            replayed.push(local_input(&replay));
        }
        assert_eq!(replayed, live);
        assert!(replay.world().resource::<InputSource>().is_finished());
        replay.update();
        assert_eq!(local_input(&replay), PlayerInput::default());
    }

    #[test]
    fn nothing_is_saved_up_while_paused() {
        let mut app = app();
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        press(&mut app, KeyCode::Space);
        app.update();
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        release(&mut app, KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert!(!local_input(&app).pressed(Action::Jump));
    }
}
