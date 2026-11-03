//! The display and sound options (PLAN.md P1.12), saved in `settings.toml` and applied
//! whenever they change. The input options are `rac_input::InputSettings`.

use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
    render::settings::WgpuSettings,
    window::{
        Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoMode, VideoModeSelection,
        WindowMode, WindowPosition, WindowResolution,
    },
};
use rac_hero::UnzoomedFov;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use wgpu::{Backend, Backends, DeviceType};

/// How the game's window is shown.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowChoice {
    #[default]
    Windowed,
    /// A window without borders covering its monitor.
    Borderless,
    /// Exclusive fullscreen, in the monitor's current video mode or the one chosen.
    Fullscreen,
}

impl WindowChoice {
    pub const ALL: [WindowChoice; 3] = [
        WindowChoice::Windowed,
        WindowChoice::Borderless,
        WindowChoice::Fullscreen,
    ];
}

/// Missing fields take their defaults, so files written by older builds keep loading.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub window: WindowChoice,
    /// The window's size when windowed, in physical pixels; Bevy's 1280 × 720 if unset.
    pub window_size: Option<UVec2>,
    /// The video mode in exclusive fullscreen; the monitor's own if unset.
    pub fullscreen_mode: Option<VideoMode>,
    pub vsync: bool,
    /// The most frames drawn a second; no limit if unset.
    pub frame_cap: Option<u32>,
    /// The cameras' vertical field of view, in degrees, before a gun's zoom narrows it.
    pub field_of_view: f32,
    /// The HUD's and the menus' size, 1 as they are laid out (Bevy's `UiScale`).
    pub interface_size: f32,
    /// The graphics backend to start with, from the next start.
    pub backend: BackendChoice,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            window: WindowChoice::Windowed,
            window_size: None,
            fullscreen_mode: None,
            vsync: true,
            frame_cap: None,
            // Bevy's default perspective, which the demo has always had; UYA's 63.6° (the
            // profiles' `camera.fov`) isn't known to be vertical (`camera.md`).
            field_of_view: 45.0,
            interface_size: 1.0,
            backend: BackendChoice::Auto,
        }
    }
}

impl DisplaySettings {
    pub fn window_mode(&self) -> WindowMode {
        match self.window {
            WindowChoice::Windowed => WindowMode::Windowed,
            WindowChoice::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            WindowChoice::Fullscreen => WindowMode::Fullscreen(
                MonitorSelection::Current,
                self.fullscreen_mode
                    .map_or(VideoModeSelection::Current, VideoModeSelection::Specific),
            ),
        }
    }

    pub fn window_resolution(&self) -> WindowResolution {
        self.window_size
            .map_or_else(WindowResolution::default, |size| {
                WindowResolution::new(size.x, size.y)
            })
    }

    pub fn present_mode(&self) -> PresentMode {
        if self.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        }
    }
}

/// The window sizes offered when windowed, in physical pixels: the common 16:9 and 16:10
/// ones, smallest first.
const WINDOW_SIZES: [UVec2; 12] = [
    UVec2::new(1280, 720),
    UVec2::new(1280, 800),
    UVec2::new(1366, 768),
    UVec2::new(1440, 900),
    UVec2::new(1600, 900),
    UVec2::new(1680, 1050),
    UVec2::new(1920, 1080),
    UVec2::new(1920, 1200),
    UVec2::new(2560, 1440),
    UVec2::new(2560, 1600),
    UVec2::new(3200, 1800),
    UVec2::new(3840, 2160),
];

/// The order sizes are stepped through: by area, then width.
pub fn size_order(size: UVec2) -> (u32, u32) {
    (size.x * size.y, size.x)
}

/// The sizes a window can be stepped through: the common ones that fit its `monitor`, and
/// its `current` size among them, as a window resized by hand may be any.
pub fn window_sizes(monitor: Option<UVec2>, current: UVec2) -> Vec<UVec2> {
    let mut sizes: Vec<UVec2> = WINDOW_SIZES
        .into_iter()
        .filter(|size| monitor.is_none_or(|monitor| size.cmple(monitor).all()))
        .collect();
    if !sizes.contains(&current) {
        sizes.push(current);
    }
    sizes.sort_by_key(|size| size_order(*size));
    sizes
}

/// The sizes exclusive fullscreen can be stepped through: those the monitor has modes for.
pub fn fullscreen_sizes(modes: &[VideoMode]) -> Vec<UVec2> {
    let mut sizes: Vec<UVec2> = modes.iter().map(|mode| mode.physical_size).collect();
    sizes.sort_by_key(|size| size_order(*size));
    sizes.dedup();
    sizes
}

/// The monitor's mode of `size` for exclusive fullscreen: at the desktop's `refresh` rate
/// if it has one, or else its fastest, in the most colours.
pub fn fullscreen_mode(
    modes: &[VideoMode],
    size: UVec2,
    refresh: Option<u32>,
) -> Option<VideoMode> {
    modes
        .iter()
        .filter(|mode| mode.physical_size == size)
        .max_by_key(|mode| {
            (
                Some(mode.refresh_rate_millihertz) == refresh,
                mode.refresh_rate_millihertz,
                mode.bit_depth,
            )
        })
        .copied()
}

/// The monitor `window` is on: the one holding its middle, or else the primary one.
pub fn window_monitor<'a>(
    window: &Window,
    monitors: impl IntoIterator<Item = (&'a Monitor, bool)>,
) -> Option<&'a Monitor> {
    let middle = match window.position {
        WindowPosition::At(corner) => {
            Some(corner + (window.resolution.physical_size() / 2).as_ivec2())
        }
        _ => None,
    };
    let mut primary = None;
    let mut first = None;
    for (monitor, is_primary) in monitors {
        let corner = monitor.physical_position;
        let size = IVec2::new(
            monitor.physical_width as i32,
            monitor.physical_height as i32,
        );
        if middle
            .is_some_and(|middle| middle.cmpge(corner).all() && middle.cmplt(corner + size).all())
        {
            return Some(monitor);
        }
        if is_primary {
            primary = Some(monitor);
        }
        first = first.or(Some(monitor));
    }
    primary.or(first)
}

/// The frame-rate caps offered, then none.
pub const FRAME_CAPS: [Option<u32>; 8] = [
    Some(30),
    Some(60),
    Some(90),
    Some(120),
    Some(144),
    Some(165),
    Some(240),
    None,
];

/// The order frame-rate caps are stepped through, with no cap last.
pub fn frame_cap_order(cap: Option<u32>) -> u32 {
    cap.unwrap_or(u32::MAX)
}

/// The item after `current` in `items`, sorted by `order`, or before it; `current` itself
/// at the ends. A `current` missing from `items`, as a hand-edited file may hold, steps
/// to its neighbour that way.
pub fn step_through<T: Copy, K: PartialOrd>(
    items: &[T],
    current: T,
    forward: bool,
    order: impl Fn(T) -> K,
) -> T {
    let at = order(current);
    let next = if forward {
        items.iter().copied().find(|item| order(*item) > at)
    } else {
        items.iter().rev().copied().find(|item| order(*item) < at)
    };
    next.unwrap_or(current)
}

/// The interface sizes the menus offer; a hand-edited file is held to them.
pub const INTERFACE_SIZES: (f32, f32) = (0.5, 2.0);

/// The graphics backend the renderer starts with.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendChoice {
    /// wgpu's: Vulkan wherever there's a Vulkan driver, or else the platform's own.
    #[default]
    Auto,
    Vulkan,
    Dx12,
    Metal,
}

impl BackendChoice {
    /// The choices this platform's build can start with (Bevy's defaults leave OpenGL out).
    pub const OFFERED: &[BackendChoice] = if cfg!(windows) {
        &[
            BackendChoice::Auto,
            BackendChoice::Vulkan,
            BackendChoice::Dx12,
        ]
    } else if cfg!(target_vendor = "apple") {
        &[BackendChoice::Auto, BackendChoice::Metal]
    } else {
        &[BackendChoice::Auto, BackendChoice::Vulkan]
    };

    pub fn name(self) -> &'static str {
        match self {
            BackendChoice::Auto => "Auto",
            BackendChoice::Vulkan => "Vulkan",
            BackendChoice::Dx12 => "DirectX 12",
            BackendChoice::Metal => "Metal",
        }
    }

    fn backends(self) -> Option<Backends> {
        match self {
            BackendChoice::Auto => None,
            BackendChoice::Vulkan => Some(Backends::VULKAN),
            BackendChoice::Dx12 => Some(Backends::DX12),
            BackendChoice::Metal => Some(Backends::METAL),
        }
    }
}

/// What a running backend is called.
pub fn backend_name(backend: Backend) -> &'static str {
    match backend {
        Backend::Vulkan => "Vulkan",
        Backend::Dx12 => "DirectX 12",
        Backend::Metal => "Metal",
        Backend::Gl => "OpenGL",
        other => other.to_str(),
    }
}

/// How the renderer was started, for the menus to say.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartedBackend {
    /// The choice saved when the game started.
    pub chosen: BackendChoice,
    pub how: BackendStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendStart {
    /// As chosen.
    Chosen,
    /// The chosen backend had no GPU, so wgpu chose.
    Missing,
    /// `WGPU_BACKEND` chose.
    Environment,
}

/// The renderer's settings for the `chosen` backend: that backend alone if the machine
/// has a GPU for it, or else wgpu's own choice, as a backend without one would stop the
/// game starting. `WGPU_BACKEND` overrides the choice, as it does Bevy's.
pub fn renderer_settings(chosen: BackendChoice) -> (WgpuSettings, StartedBackend) {
    let mut settings = WgpuSettings::default();
    let how = match chosen.backends() {
        _ if Backends::from_env().is_some() => BackendStart::Environment,
        None => BackendStart::Chosen,
        Some(backends) if has_gpu(backends) => {
            settings.backends = Some(backends);
            BackendStart::Chosen
        }
        Some(_) => BackendStart::Missing,
    };
    (settings, StartedBackend { chosen, how })
}

fn has_gpu(backends: Backends) -> bool {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
    });
    let adapters = bevy::tasks::block_on(instance.enumerate_adapters(backends));
    any_gpu(
        adapters
            .iter()
            .map(|adapter| adapter.get_info().device_type),
    )
}

/// Whether any adapter is a GPU: a software one (DirectX's WARP, Mesa's lavapipe) would
/// run the game far slower than whatever wgpu would choose instead.
fn any_gpu(adapters: impl IntoIterator<Item = DeviceType>) -> bool {
    adapters
        .into_iter()
        .any(|device_type| device_type != DeviceType::Cpu)
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
            .init_resource::<FramePacer>()
            .add_systems(
                Update,
                (
                    apply_window.run_if(resource_changed::<DisplaySettings>),
                    apply_field_of_view,
                    apply_interface_size.run_if(resource_changed::<DisplaySettings>),
                    apply_volume.run_if(resource_changed::<SoundSettings>),
                ),
            )
            .add_systems(Startup, report_backend)
            .add_systems(Last, cap_frame_rate);
    }
}

/// Says when the backend chosen had no GPU, once the log is up, which it isn't yet when
/// `main` chooses.
fn report_backend(started: Option<Res<StartedBackend>>) {
    if let Some(started) = started
        && started.how == BackendStart::Missing
    {
        warn!(
            "no GPU was found for {}, so the renderer chose its backend itself",
            started.chosen.name()
        );
    }
}

/// Gives the window what changed in the display settings, and only that: a window resized
/// by hand keeps its size when vsync is turned off. The window starts with the settings
/// (`main`), so the first run only notes them.
fn apply_window(
    display: Res<DisplaySettings>,
    mut applied: Local<Option<DisplaySettings>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Some(before) = applied.replace(*display) else {
        return;
    };
    for mut window in &mut windows {
        if (before.window, before.fullscreen_mode) != (display.window, display.fullscreen_mode) {
            window.mode = display.window_mode();
        }
        let size = display
            .window_size
            .filter(|_| display.window == WindowChoice::Windowed);
        if let Some(size) = size
            && (before.window_size != display.window_size || before.window != display.window)
            && window.resolution.physical_size() != size
        {
            window.resolution.set_physical_resolution(size.x, size.y);
        }
        if before.vsync != display.vsync {
            window.present_mode = display.present_mode();
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

/// Scales the HUD and the menus. The sights keep to the view, as they're placed in the
/// window's pixels (`rac_demo::hud`).
fn apply_interface_size(display: Res<DisplaySettings>, mut scale: ResMut<UiScale>) {
    let (smallest, largest) = INTERFACE_SIZES;
    let size = display.interface_size.clamp(smallest, largest);
    if scale.0 != size {
        scale.0 = size;
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

/// When the last frame was let go, while the frame rate is capped.
#[derive(Resource, Debug, Default)]
struct FramePacer {
    last: Option<Instant>,
}

/// Holds each frame until its time when the frame rate is capped. It runs last, so the
/// next frame reads its input straight after the wait rather than before it.
fn cap_frame_rate(display: Res<DisplaySettings>, mut pacer: ResMut<FramePacer>) {
    let Some(cap) = display.frame_cap.filter(|cap| *cap > 0) else {
        pacer.last = None;
        return;
    };
    let due = due(
        pacer.last,
        Instant::now(),
        Duration::from_secs_f64(1.0 / f64::from(cap)),
    );
    wait_until(due);
    pacer.last = Some(due);
}

/// When a frame may end: a `period` after the `last` one, or straight away if it's later
/// than that, so a slow frame isn't followed by quick ones catching up.
fn due(last: Option<Instant>, now: Instant, period: Duration) -> Instant {
    match last {
        Some(last) if last + period > now => last + period,
        _ => now,
    }
}

/// Sleeps until `due`, spinning through the last half millisecond, which a sleep may
/// overshoot.
fn wait_until(due: Instant) {
    const SPIN: Duration = Duration::from_micros(500);
    loop {
        let now = Instant::now();
        if now >= due {
            return;
        }
        let left = due - now;
        if left > SPIN {
            std::thread::sleep(left - SPIN);
        } else {
            std::hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(width: u32, height: u32, hertz: u32, bit_depth: u16) -> VideoMode {
        VideoMode {
            physical_size: UVec2::new(width, height),
            bit_depth,
            refresh_rate_millihertz: hertz * 1000,
        }
    }

    #[test]
    fn a_window_steps_through_the_sizes_that_fit_its_monitor() {
        let sizes = window_sizes(Some(UVec2::new(1920, 1080)), UVec2::new(1280, 720));
        assert_eq!(sizes.first(), Some(&UVec2::new(1280, 720)));
        assert_eq!(sizes.last(), Some(&UVec2::new(1920, 1080)));
        assert!(!sizes.contains(&UVec2::new(1920, 1200)));
        // A size set by hand is among them, in its place by area.
        let sizes = window_sizes(Some(UVec2::new(1920, 1080)), UVec2::new(1500, 850));
        let step = |forward| step_through(&sizes, UVec2::new(1500, 850), forward, size_order);
        assert_eq!(step(true), UVec2::new(1440, 900));
        assert_eq!(step(false), UVec2::new(1366, 768));
    }

    #[test]
    fn stepping_stays_at_the_ends_and_finds_a_value_off_the_list() {
        let caps = &FRAME_CAPS;
        assert_eq!(step_through(caps, None, true, frame_cap_order), None);
        assert_eq!(step_through(caps, None, false, frame_cap_order), Some(240));
        assert_eq!(
            step_through(caps, Some(30), false, frame_cap_order),
            Some(30)
        );
        assert_eq!(
            step_through(caps, Some(75), true, frame_cap_order),
            Some(90)
        );
        assert_eq!(
            step_through(caps, Some(75), false, frame_cap_order),
            Some(60)
        );
    }

    #[test]
    fn fullscreen_takes_the_desktops_refresh_rate_or_else_the_fastest() {
        let modes = [
            mode(1920, 1080, 60, 32),
            mode(1920, 1080, 144, 32),
            mode(1920, 1080, 144, 24),
            mode(1280, 720, 60, 32),
            mode(1280, 720, 60, 24),
        ];
        assert_eq!(
            fullscreen_sizes(&modes),
            [UVec2::new(1280, 720), UVec2::new(1920, 1080)]
        );
        let size = UVec2::new(1920, 1080);
        assert_eq!(fullscreen_mode(&modes, size, Some(60_000)), Some(modes[0]));
        assert_eq!(fullscreen_mode(&modes, size, Some(59_940)), Some(modes[1]));
        assert_eq!(fullscreen_mode(&modes, size, None), Some(modes[1]));
        assert_eq!(fullscreen_mode(&modes, UVec2::new(800, 600), None), None);
    }

    #[test]
    fn the_window_is_on_the_monitor_holding_its_middle() {
        let monitor = |x: i32, width: u32| Monitor {
            name: None,
            physical_height: 1080,
            physical_width: width,
            physical_position: IVec2::new(x, 0),
            refresh_rate_millihertz: None,
            scale_factor: 1.0,
            video_modes: Vec::new(),
        };
        let left = monitor(0, 1920);
        let right = monitor(1920, 2560);
        let mut window = Window {
            position: WindowPosition::At(IVec2::new(2000, 100)),
            ..default()
        };
        let both = || [(&left, true), (&right, false)];
        let found = |window: &Window| window_monitor(window, both()).map(|m| m.physical_position.x);
        assert_eq!(found(&window), Some(1920));
        window.position = WindowPosition::Automatic;
        assert_eq!(found(&window), Some(0));
    }

    #[test]
    fn a_capped_frame_waits_a_period_after_the_last_unless_it_is_late() {
        let start = Instant::now();
        let period = Duration::from_millis(16);
        assert_eq!(due(None, start, period), start);
        let early = start + Duration::from_millis(5);
        assert_eq!(due(Some(start), early, period), start + period);
        let late = start + Duration::from_millis(20);
        assert_eq!(due(Some(start), late, period), late);
    }

    #[test]
    fn a_backend_needs_a_gpu_not_a_software_renderer() {
        assert!(!any_gpu([]));
        assert!(!any_gpu([DeviceType::Cpu]));
        assert!(any_gpu([DeviceType::Cpu, DeviceType::DiscreteGpu]));
        assert!(any_gpu([DeviceType::IntegratedGpu]));
    }

    #[test]
    fn a_fullscreen_mode_is_the_monitors_own_unless_one_is_chosen() {
        let mut display = DisplaySettings {
            window: WindowChoice::Fullscreen,
            ..default()
        };
        assert_eq!(
            display.window_mode(),
            WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
        );
        let chosen = mode(1280, 720, 60, 32);
        display.fullscreen_mode = Some(chosen);
        assert_eq!(
            display.window_mode(),
            WindowMode::Fullscreen(
                MonitorSelection::Current,
                VideoModeSelection::Specific(chosen)
            )
        );
        display.window_size = Some(UVec2::new(1600, 900));
        assert_eq!(
            display.window_resolution().physical_size(),
            UVec2::new(1600, 900)
        );
    }

    #[test]
    fn the_window_gets_only_what_changed() {
        fn window(app: &mut App) -> Mut<'_, Window> {
            let mut windows = app.world_mut().query::<&mut Window>();
            windows.single_mut(app.world_mut()).unwrap()
        }
        fn change(app: &mut App, change: fn(&mut DisplaySettings)) {
            change(&mut app.world_mut().resource_mut::<DisplaySettings>());
            app.update();
        }
        let mut app = App::new();
        app.init_resource::<DisplaySettings>().add_systems(
            Update,
            apply_window.run_if(resource_changed::<DisplaySettings>),
        );
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.update();

        // Resized by hand, then vsync turned off: the size stays.
        window(&mut app)
            .resolution
            .set_physical_resolution(1500, 850);
        change(&mut app, |display| display.vsync = false);
        assert_eq!(window(&mut app).present_mode, PresentMode::AutoNoVsync);
        assert_eq!(
            window(&mut app).resolution.physical_size(),
            UVec2::new(1500, 850)
        );

        change(&mut app, |display| {
            display.window_size = Some(UVec2::new(1920, 1080));
        });
        assert_eq!(
            window(&mut app).resolution.physical_size(),
            UVec2::new(1920, 1080)
        );

        // Fullscreen leaves the size to the monitor; windowed again brings it back.
        change(&mut app, |display| {
            display.window = WindowChoice::Fullscreen
        });
        assert_eq!(
            window(&mut app).mode,
            WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
        );
        window(&mut app)
            .resolution
            .set_physical_resolution(2560, 1440);
        change(&mut app, |display| display.window = WindowChoice::Windowed);
        assert_eq!(window(&mut app).mode, WindowMode::Windowed);
        assert_eq!(
            window(&mut app).resolution.physical_size(),
            UVec2::new(1920, 1080)
        );
    }
}
