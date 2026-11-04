//! The menus' choices, which left and right step through: the hero's model, the game he
//! moves like and his camera, and the options. Each step is saved in the settings file.

use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::renderer::RenderAdapterInfo,
    window::{Monitor, PrimaryMonitor, PrimaryWindow},
};
use rac_demo::DemoSettings;
use rac_hero::HeroTuning;
use rac_input::InputSettings;
use rac_rig::RigModels;

use super::names::KeyNames;
use crate::camera;
use crate::moves::{self, ChosenMoves};
use crate::options::{
    self, BackendChoice, BackendStart, DisplaySettings, SoundSettings, StartedBackend, WindowChoice,
};
use crate::settings::{self, Paths, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Model,
    Moves,
    Camera,
    MouseSpeed,
    InvertLookX,
    InvertLookY,
    SwapSticks,
    MoveDeadZone,
    LookDeadZone,
    Window,
    Resolution,
    Vsync,
    FrameCap,
    FieldOfView,
    InterfaceSize,
    Backend,
    Volume,
}

impl Choice {
    pub fn name(self) -> &'static str {
        match self {
            Choice::Model => "Model",
            Choice::Moves => "Moves",
            Choice::Camera => "Camera",
            Choice::MouseSpeed => "Mouse speed",
            Choice::InvertLookX => "Invert look left and right",
            Choice::InvertLookY => "Invert look up and down",
            Choice::SwapSticks => "Swap the sticks",
            Choice::MoveDeadZone => "Movement stick dead zone",
            Choice::LookDeadZone => "Look stick dead zone",
            Choice::Window => "Window",
            Choice::Resolution => "Resolution",
            Choice::Vsync => "Vsync",
            Choice::FrameCap => "Frame-rate cap",
            Choice::FieldOfView => "Field of view",
            Choice::InterfaceSize => "HUD and menu size",
            Choice::Backend => "Graphics API",
            Choice::Volume => "Volume",
        }
    }

    /// Whether holding left or right keeps stepping it: only for numbers.
    pub fn repeats(self) -> bool {
        matches!(
            self,
            Choice::MouseSpeed
                | Choice::MoveDeadZone
                | Choice::LookDeadZone
                | Choice::FieldOfView
                | Choice::InterfaceSize
                | Choice::Volume
        )
    }
}

/// A whole-numbered range a choice steps through.
#[derive(Debug, Clone, Copy)]
struct Steps {
    min: i32,
    max: i32,
    step: i32,
}

/// Mouse speed, as a percentage of the default sensitivity.
const MOUSE_SPEED: Steps = Steps {
    min: 10,
    max: 500,
    step: 10,
};
/// Dead zones, as percentages of full tilt.
const DEAD_ZONE: Steps = Steps {
    min: 0,
    max: 50,
    step: 5,
};
/// The vertical field of view, in degrees.
const FIELD_OF_VIEW: Steps = Steps {
    min: 30,
    max: 110,
    step: 5,
};
/// The HUD's and menus' size, as a percentage (`options::INTERFACE_SIZES`).
const INTERFACE_SIZE: Steps = Steps {
    min: 50,
    max: 200,
    step: 10,
};
/// The volume, as a percentage.
const VOLUME: Steps = Steps {
    min: 0,
    max: 100,
    step: 10,
};

impl Steps {
    /// The next step up or down from `value`, in the units the steps count. A value off
    /// the steps, as a hand-edited file may hold, goes to the nearest step that way.
    fn step(self, value: f32, forward: bool) -> f32 {
        let current = value.round() as i32;
        let next = if forward {
            (current.div_euclid(self.step) + 1) * self.step
        } else {
            (-(-current).div_euclid(self.step) - 1) * self.step
        };
        next.clamp(self.min, self.max) as f32
    }
}

fn default_mouse_sensitivity() -> f32 {
    InputSettings::default().mouse_sensitivity
}

fn on_off(on: bool) -> String {
    if on { "On" } else { "Off" }.into()
}

fn size_text(size: UVec2) -> String {
    format!("{} x {}", size.x, size.y)
}

/// The backend `saved`, as the renderer `started` with it, `running` the one in use.
fn backend_text(
    saved: BackendChoice,
    started: Option<StartedBackend>,
    running: Option<&str>,
) -> String {
    let running = running.unwrap_or("none");
    let Some(started) = started.filter(|started| started.chosen == saved) else {
        return format!("{} (on restart)", saved.name());
    };
    match (started.how, saved) {
        (BackendStart::Environment, _) => format!("{running} (WGPU_BACKEND)"),
        (BackendStart::Missing, _) => format!("{} (no GPU)", saved.name()),
        (BackendStart::Chosen, BackendChoice::Auto) => format!("Auto ({running})"),
        (BackendStart::Chosen, _) => saved.name().into(),
    }
}

/// Everything the choices show and change.
#[derive(SystemParam)]
pub struct Choices<'w, 's> {
    models: ResMut<'w, RigModels>,
    tuning: ResMut<'w, HeroTuning>,
    chosen_moves: ResMut<'w, ChosenMoves>,
    demo: ResMut<'w, DemoSettings>,
    input: ResMut<'w, InputSettings>,
    display: ResMut<'w, DisplaySettings>,
    sound: ResMut<'w, SoundSettings>,
    paths: Option<Res<'w, Paths>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    monitors: Query<'w, 's, (&'static Monitor, Has<PrimaryMonitor>)>,
    keys: Res<'w, KeyNames>,
    started: Option<Res<'w, StartedBackend>>,
    adapter: Option<Res<'w, RenderAdapterInfo>>,
    session: Option<Res<'w, rac_net::Session>>,
}

impl Choices<'_, '_> {
    /// What `choice` is set to, as the menus show it.
    pub fn value(&self, choice: Choice) -> String {
        match choice {
            Choice::Model => self
                .models
                .chosen()
                .map_or("none", |model| model.title.as_str())
                .to_owned(),
            Choice::Moves => self.tuning.0.name.clone(),
            Choice::Camera => camera::label(self.demo.camera, &self.tuning.0),
            Choice::MouseSpeed => format!(
                "{:.0}%",
                self.input.mouse_sensitivity / default_mouse_sensitivity() * 100.0
            ),
            Choice::InvertLookX => on_off(self.input.invert_look_x),
            Choice::InvertLookY => on_off(self.input.invert_look_y),
            Choice::SwapSticks => on_off(self.input.swap_sticks),
            Choice::MoveDeadZone => format!("{:.0}%", self.input.move_dead_zone * 100.0),
            Choice::LookDeadZone => format!("{:.0}%", self.input.look_dead_zone * 100.0),
            Choice::Window => match self.display.window {
                WindowChoice::Windowed => "Windowed",
                WindowChoice::Borderless => "Borderless",
                WindowChoice::Fullscreen => "Fullscreen",
            }
            .into(),
            Choice::Resolution => self.windows.single().map_or_else(
                |_| "-".into(),
                |window| size_text(window.resolution.physical_size()),
            ),
            Choice::Vsync => on_off(self.display.vsync),
            Choice::FrameCap => self
                .display
                .frame_cap
                .map_or_else(|| "Off".into(), |cap| format!("{cap} fps")),
            Choice::FieldOfView => format!("{:.0} degrees", self.display.field_of_view),
            Choice::InterfaceSize => format!("{:.0}%", self.display.interface_size * 100.0),
            Choice::Backend => backend_text(
                self.display.backend,
                self.started.as_deref().copied(),
                self.adapter
                    .as_deref()
                    .map(|adapter| options::backend_name(adapter.backend)),
            ),
            Choice::Volume => format!("{:.0}%", self.sound.volume * 100.0),
        }
    }

    /// Steps `choice` forwards or back, and saves it.
    pub fn step(&mut self, choice: Choice, forward: bool) {
        // Both co-op players agreed on how the heroes move and see before the session
        // started (`crate::coop`).
        if self.session.is_some() && matches!(choice, Choice::Moves | Choice::Camera) {
            info!("the movement and camera can't change during a co-op game");
            return;
        }
        match choice {
            Choice::Model => {
                let model = if forward {
                    self.models.choose_next()
                } else {
                    self.models.choose_previous()
                };
                let Some(model) = model else {
                    return;
                };
                info!("the hero's model: {} ({})", model.title, model.name);
                let name = model.name.clone();
                self.save(|saved| saved.hero_model = Some(name));
            }
            Choice::Moves => {
                let name = moves::step(&self.chosen_moves.0, forward);
                let profile = match moves::load(name) {
                    Ok(profile) => profile,
                    Err(error) => {
                        warn!("{error:#}");
                        return;
                    }
                };
                info!("the hero moves like {} ({name})", profile.name);
                self.tuning.0 = profile;
                self.chosen_moves.0 = name.to_owned();
                self.save(|saved| saved.hero_moves = Some(name.to_owned()));
            }
            Choice::Camera => {
                let choice = camera::step(self.demo.camera, forward);
                info!(
                    "the hero's camera: {}",
                    camera::label(choice, &self.tuning.0)
                );
                self.demo.camera = choice;
                self.save(|saved| {
                    saved.hero_camera = choice.map(|setting| camera::name(setting).to_owned());
                });
            }
            Choice::MouseSpeed => {
                let default = default_mouse_sensitivity();
                let percent = self.input.mouse_sensitivity / default * 100.0;
                self.input.mouse_sensitivity = MOUSE_SPEED.step(percent, forward) / 100.0 * default;
                self.save_input();
            }
            Choice::InvertLookX => {
                self.input.invert_look_x = !self.input.invert_look_x;
                self.save_input();
            }
            Choice::InvertLookY => {
                self.input.invert_look_y = !self.input.invert_look_y;
                self.save_input();
            }
            Choice::SwapSticks => {
                self.input.swap_sticks = !self.input.swap_sticks;
                self.save_input();
            }
            Choice::MoveDeadZone => {
                self.input.move_dead_zone =
                    DEAD_ZONE.step(self.input.move_dead_zone * 100.0, forward) / 100.0;
                self.save_input();
            }
            Choice::LookDeadZone => {
                self.input.look_dead_zone =
                    DEAD_ZONE.step(self.input.look_dead_zone * 100.0, forward) / 100.0;
                self.save_input();
            }
            Choice::Window => {
                let all = WindowChoice::ALL;
                let index = all
                    .iter()
                    .position(|choice| *choice == self.display.window)
                    .unwrap_or(0);
                let next = if forward {
                    (index + 1) % all.len()
                } else {
                    (index + all.len() - 1) % all.len()
                };
                self.display.window = all[next];
                self.save_display();
            }
            Choice::Resolution => {
                if !self.step_resolution(forward) {
                    return;
                }
                self.save_display();
            }
            Choice::Vsync => {
                self.display.vsync = !self.display.vsync;
                self.save_display();
            }
            Choice::FrameCap => {
                self.display.frame_cap = options::step_through(
                    &options::FRAME_CAPS,
                    self.display.frame_cap,
                    forward,
                    options::frame_cap_order,
                );
                self.save_display();
            }
            Choice::FieldOfView => {
                self.display.field_of_view =
                    FIELD_OF_VIEW.step(self.display.field_of_view, forward);
                self.save_display();
            }
            Choice::InterfaceSize => {
                self.display.interface_size =
                    INTERFACE_SIZE.step(self.display.interface_size * 100.0, forward) / 100.0;
                self.save_display();
            }
            Choice::Backend => {
                let offered = BackendChoice::OFFERED;
                let index = offered
                    .iter()
                    .position(|choice| *choice == self.display.backend)
                    .unwrap_or(0);
                let next = if forward {
                    (index + 1) % offered.len()
                } else {
                    (index + offered.len() - 1) % offered.len()
                };
                self.display.backend = offered[next];
                self.save_display();
            }
            Choice::Volume => {
                self.sound.volume = VOLUME.step(self.sound.volume * 100.0, forward) / 100.0;
                let sound = *self.sound;
                self.save(|saved| saved.sound = sound);
            }
        }
    }

    /// Steps the window's size, or the video mode's in exclusive fullscreen, through those
    /// its monitor offers; borderless has the monitor's own. Whether anything changed.
    fn step_resolution(&mut self, forward: bool) -> bool {
        let Ok(window) = self.windows.single() else {
            return false;
        };
        let monitor = options::window_monitor(window, self.monitors.iter());
        let monitor_size =
            monitor.map(|monitor| UVec2::new(monitor.physical_width, monitor.physical_height));
        match self.display.window {
            WindowChoice::Windowed => {
                let current = window.resolution.physical_size();
                let sizes = options::window_sizes(monitor_size, current);
                let next = options::step_through(&sizes, current, forward, options::size_order);
                self.display.window_size = Some(next);
                next != current
            }
            WindowChoice::Borderless => false,
            WindowChoice::Fullscreen => {
                let (Some(monitor), Some(monitor_size)) = (monitor, monitor_size) else {
                    return false;
                };
                let current = self
                    .display
                    .fullscreen_mode
                    .map_or(monitor_size, |mode| mode.physical_size);
                let sizes = options::fullscreen_sizes(&monitor.video_modes);
                let next = options::step_through(&sizes, current, forward, options::size_order);
                // The monitor's own size is its current mode, as the desktop has it.
                self.display.fullscreen_mode = if next == monitor_size {
                    None
                } else {
                    options::fullscreen_mode(
                        &monitor.video_modes,
                        next,
                        monitor.refresh_rate_millihertz,
                    )
                };
                next != current
            }
        }
    }

    pub fn keys(&self) -> &KeyNames {
        &self.keys
    }

    pub fn input(&self) -> &InputSettings {
        &self.input
    }

    /// The input settings to change; the caller saves them with [`Self::save_input`].
    pub fn input_mut(&mut self) -> &mut InputSettings {
        &mut self.input
    }

    pub fn save_input(&self) {
        let input = self.input.clone();
        self.save(|saved| saved.input = input);
    }

    fn save_display(&self) {
        let display = *self.display;
        self.save(|saved| saved.display = display);
    }

    fn save(&self, change: impl FnOnce(&mut Settings)) {
        let Some(paths) = &self.paths else {
            return;
        };
        if let Err(error) = settings::update(&paths.settings_file, change) {
            warn!("could not save the settings: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_stay_on_their_grid_and_in_range() {
        assert_eq!(VOLUME.step(100.0, true), 100.0);
        assert_eq!(VOLUME.step(100.0, false), 90.0);
        assert_eq!(VOLUME.step(0.0, false), 0.0);
        // Off the grid, a step goes to the next one either way.
        assert_eq!(MOUSE_SPEED.step(124.0, true), 130.0);
        assert_eq!(MOUSE_SPEED.step(124.0, false), 120.0);
        assert_eq!(FIELD_OF_VIEW.step(45.0, true), 50.0);
        assert_eq!(DEAD_ZONE.step(10.0, false), 5.0);
        assert_eq!(INTERFACE_SIZE.step(100.0, true), 110.0);
    }

    #[test]
    fn the_backend_says_what_runs_and_what_waits_for_a_restart() {
        let started = |chosen, how| Some(StartedBackend { chosen, how });
        let auto = started(BackendChoice::Auto, BackendStart::Chosen);
        assert_eq!(
            backend_text(BackendChoice::Auto, auto, Some("Vulkan")),
            "Auto (Vulkan)"
        );
        assert_eq!(
            backend_text(BackendChoice::Dx12, auto, Some("Vulkan")),
            "DirectX 12 (on restart)"
        );
        let dx12 = BackendChoice::Dx12;
        assert_eq!(
            backend_text(
                dx12,
                started(dx12, BackendStart::Chosen),
                Some("DirectX 12")
            ),
            "DirectX 12"
        );
        assert_eq!(
            backend_text(dx12, started(dx12, BackendStart::Missing), Some("Vulkan")),
            "DirectX 12 (no GPU)"
        );
        assert_eq!(
            backend_text(
                dx12,
                started(dx12, BackendStart::Environment),
                Some("Vulkan")
            ),
            "Vulkan (WGPU_BACKEND)"
        );
    }
}
