//! The display and sound options (PLAN.md P1.12), saved in `settings.toml` and applied
//! whenever they change. The input options are `rac_input::InputSettings`.

use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
    window::{MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode},
};
use rac_hero::UnzoomedFov;
use serde::{Deserialize, Serialize};

/// How the game's window is shown.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowChoice {
    #[default]
    Windowed,
    /// A window without borders covering its monitor.
    Borderless,
    /// Exclusive fullscreen, in the monitor's current video mode.
    Fullscreen,
}

impl WindowChoice {
    pub const ALL: [WindowChoice; 3] = [
        WindowChoice::Windowed,
        WindowChoice::Borderless,
        WindowChoice::Fullscreen,
    ];

    pub fn mode(self) -> WindowMode {
        match self {
            WindowChoice::Windowed => WindowMode::Windowed,
            WindowChoice::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            WindowChoice::Fullscreen => {
                WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
            }
        }
    }
}

/// Missing fields take their defaults, so files written by older builds keep loading.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub window: WindowChoice,
    pub vsync: bool,
    /// The cameras' vertical field of view, in degrees, before a gun's zoom narrows it.
    pub field_of_view: f32,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            window: WindowChoice::Windowed,
            vsync: true,
            // Bevy's default perspective, which the demo has always had; UYA's 63.6° (the
            // profiles' `camera.fov`) isn't known to be vertical (`camera.md`).
            field_of_view: 45.0,
        }
    }
}

impl DisplaySettings {
    pub fn present_mode(&self) -> PresentMode {
        if self.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundSettings {
    /// Everything's volume, 0 (silent) to 1 (as mixed).
    pub volume: f32,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self { volume: 1.0 }
    }
}

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DisplaySettings>()
            .init_resource::<SoundSettings>()
            .add_systems(
                Update,
                (
                    apply_window.run_if(resource_changed::<DisplaySettings>),
                    apply_field_of_view,
                    apply_volume.run_if(resource_changed::<SoundSettings>),
                ),
            );
    }
}

fn apply_window(
    display: Res<DisplaySettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    for mut window in &mut windows {
        let mode = display.window.mode();
        if window.mode != mode {
            window.mode = mode;
        }
        let present_mode = display.present_mode();
        if window.present_mode != present_mode {
            window.present_mode = present_mode;
        }
    }
}

/// Gives every 3D camera the chosen field of view: a hero camera keeps it as the one its
/// gun's zoom narrows from (`UnzoomedFov`, which it takes from its projection when it
/// first runs), any other its projection's own.
fn apply_field_of_view(
    display: Res<DisplaySettings>,
    mut cameras: Query<(&mut Projection, Option<&mut UnzoomedFov>), With<Camera3d>>,
) {
    let fov = display.field_of_view.to_radians();
    for (mut projection, unzoomed) in &mut cameras {
        match unzoomed {
            Some(mut unzoomed) => {
                if unzoomed.0 != fov {
                    unzoomed.0 = fov;
                }
            }
            None => {
                if let Projection::Perspective(perspective) = projection.as_ref()
                    && perspective.fov != fov
                    && let Projection::Perspective(perspective) = projection.as_mut()
                {
                    perspective.fov = fov;
                }
            }
        }
    }
}

/// Sets the volume of what plays from now on and of what is playing: the guns' streams
/// play for as long as the level does.
fn apply_volume(
    sound: Res<SoundSettings>,
    mut global: ResMut<GlobalVolume>,
    mut sinks: Query<&mut AudioSink>,
) {
    let volume = Volume::Linear(sound.volume.clamp(0.0, 1.0));
    global.volume = volume;
    for mut sink in &mut sinks {
        sink.set_volume(volume);
    }
}
