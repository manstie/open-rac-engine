//! App flow (PLAN.md P1.1): Boot → MainMenu → InGame ⇄ Paused.
//!
//! The game clock (`Time<Virtual>`, which drives the fixed 60 Hz simulation) runs only
//! while in game and not paused, so the simulation never advances behind a menu.

use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use rac_input::{Action, ActionState, InputRecorder, InputRecording, InputSource, reset_inputs};
use rac_sim::{SimRng, SimTick};
use std::path::PathBuf;

#[derive(States, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Finding a disc and deciding what to run.
    #[default]
    Boot,
    MainMenu,
    InGame,
}

#[derive(SubStates, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[source(AppState = AppState::InGame)]
pub enum Pause {
    #[default]
    Running,
    Paused,
}

/// Choices made on the command line that shape the flow.
#[derive(Resource, Debug, Default, Clone)]
pub struct FlowOptions {
    /// Go straight into the demo instead of the main menu.
    pub skip_menu: bool,
    /// Record every tick's input to this file, written when the level ends or the app
    /// quits.
    pub record_to: Option<PathBuf>,
}

/// The demo level's seed for `SimRng`. Fixed, so every run of the demo is the same.
const DEMO_SEED: u64 = 0x0DE5_0001;

pub struct FlowPlugin;

impl Plugin for FlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<Pause>()
            .init_resource::<FlowOptions>()
            .add_systems(OnEnter(AppState::Boot), boot)
            .add_systems(
                OnEnter(AppState::InGame),
                (
                    start_level,
                    reset_inputs,
                    rac_demo::spawn_demo,
                    start_recording,
                ),
            )
            .add_systems(
                OnExit(AppState::InGame),
                (rac_demo::despawn_demo, save_recording),
            )
            .add_systems(OnEnter(Pause::Running), start_game_clock)
            .add_systems(OnExit(Pause::Running), (stop_game_clock, release_cursor))
            .add_systems(
                Update,
                (
                    toggle_pause.run_if(in_state(AppState::InGame)),
                    (grab_cursor, report_replay_end).run_if(in_state(Pause::Running)),
                ),
            )
            .add_systems(Last, save_recording.run_if(on_message::<AppExit>));
    }
}

fn boot(
    options: Res<FlowOptions>,
    mut time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<AppState>>,
) {
    // The game clock only runs in game (see the module docs).
    time.pause();
    // Disc detection arrives with the disc layer (P2.1). Until then there is never a
    // supported disc, so the app offers the demo.
    next.set(if options.skip_menu {
        AppState::InGame
    } else {
        AppState::MainMenu
    });
}

fn start_level(mut tick: ResMut<SimTick>, mut rng: ResMut<SimRng>) {
    *tick = SimTick(0);
    *rng = SimRng::new(DEMO_SEED, 0);
}

fn start_game_clock(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn stop_game_clock(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

/// Hides and holds the cursor for mouse look while the window has focus. It waits for
/// focus because X11 refuses to grab for a window that isn't on screen yet, and the system
/// keeps checking because losing focus (alt-tab) drops the grab.
fn grab_cursor(mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>) {
    for (window, mut cursor) in &mut windows {
        if window.focused && cursor.grab_mode == CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        } else if !window.focused && cursor.grab_mode != CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }
}

fn release_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut cursor in &mut cursors {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn toggle_pause(
    actions: Res<ActionState>,
    pause: Res<State<Pause>>,
    mut next: ResMut<NextState<Pause>>,
) {
    match pause.get() {
        Pause::Running if actions.just_pressed(Action::Pause) => next.set(Pause::Paused),
        Pause::Paused
            if actions.just_pressed(Action::Pause) || actions.just_pressed(Action::UiBack) =>
        {
            next.set(Pause::Running);
        }
        _ => {}
    }
}

fn start_recording(mut commands: Commands, options: Res<FlowOptions>) {
    if options.record_to.is_some() {
        commands.insert_resource(InputRecorder(InputRecording::new(1)));
    }
}

/// Writes the input recording, if one is running. It runs when the level ends and when the
/// app quits, whichever comes first.
fn save_recording(
    mut commands: Commands,
    options: Res<FlowOptions>,
    recorder: Option<Res<InputRecorder>>,
) {
    let (Some(path), Some(recorder)) = (&options.record_to, recorder) else {
        return;
    };
    match std::fs::write(path, recorder.0.to_bytes()) {
        Ok(()) => info!(
            "recorded {} ticks of input to {}",
            recorder.0.ticks(),
            path.display()
        ),
        Err(error) => error!("could not write {}: {error}", path.display()),
    }
    commands.remove_resource::<InputRecorder>();
}

fn report_replay_end(source: Res<InputSource>, tick: Res<SimTick>, mut reported: Local<bool>) {
    if source.is_finished() && !*reported {
        info!("the input replay ended at tick {}", tick.0);
        *reported = true;
    }
}
