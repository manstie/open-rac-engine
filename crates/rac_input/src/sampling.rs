//! Turning this machine's devices into actions each frame and into a [`PlayerInput`] each
//! tick.
//!
//! Devices are read once a frame (`PreUpdate`), but the simulation consumes input once a
//! tick. Between ticks the sampler keeps the latest stick and held buttons, and adds up
//! presses and view turning, so nothing is lost when a frame runs no tick and nothing is
//! counted twice when it runs several.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};
use std::f32::consts::TAU;

use crate::{
    ANGLE_UNITS_PER_TURN, Action, Binding, Buttons, InputRecorder, InputSettings, InputSource,
    LocalPlayer, PlayerInput, PlayerInputs, WheelDirection, apply_dead_zone,
};

/// Every action's state this frame, for menus and other local screens. Gameplay reads
/// [`PlayerInputs`] instead.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActionState {
    held: u32,
    just_pressed: u32,
}

impl ActionState {
    pub fn held(&self, action: Action) -> bool {
        self.held & (1 << action as u32) != 0
    }

    pub fn just_pressed(&self, action: Action) -> bool {
        self.just_pressed & (1 << action as u32) != 0
    }

    fn set(&mut self, action: Action, held: bool, just_pressed: bool) {
        let bit = 1 << action as u32;
        if held {
            self.held |= bit;
        }
        if just_pressed {
            self.just_pressed |= bit;
        }
    }
}

/// What the local devices have produced since the last tick. Internal to the plugin; it
/// is public only so [`reset_inputs`] can name it.
#[derive(Resource, Debug, Default)]
pub struct LocalSampler {
    held: Buttons,
    pressed: Buttons,
    move_stick: [i8; 2],
    look_stick: [i8; 2],
    /// Mouse turning not yet sent, in radians. Only whole angle units are sent each tick,
    /// and the remainder carries over, so slow mouse movement isn't rounded away.
    look: Vec2,
}

/// How far the movement stick must tilt to count as a menu direction.
const STICK_MENU_THRESHOLD: f32 = 0.5;

/// The devices this frame, gathered for evaluating bindings.
struct Devices<'a> {
    keys: &'a ButtonInput<KeyCode>,
    mouse: &'a ButtonInput<MouseButton>,
    wheel: f32,
    pads: Vec<&'a Gamepad>,
}

impl Devices<'_> {
    fn held(&self, binding: Binding) -> bool {
        match binding {
            Binding::Key(key) => self.keys.pressed(key),
            Binding::Mouse(button) => self.mouse.pressed(button),
            Binding::Pad(button) => self.pads.iter().any(|pad| pad.pressed(button)),
            Binding::PadBoth(a, b) => self.pads.iter().any(|pad| pad.pressed(a) && pad.pressed(b)),
            Binding::Wheel(direction) => self.wheel_turned(direction),
        }
    }

    fn just_pressed(&self, binding: Binding) -> bool {
        match binding {
            Binding::Key(key) => self.keys.just_pressed(key),
            Binding::Mouse(button) => self.mouse.just_pressed(button),
            Binding::Pad(button) => self.pads.iter().any(|pad| pad.just_pressed(button)),
            Binding::PadBoth(a, b) => self.pads.iter().any(|pad| {
                pad.pressed(a) && pad.pressed(b) && (pad.just_pressed(a) || pad.just_pressed(b))
            }),
            Binding::Wheel(direction) => self.wheel_turned(direction),
        }
    }

    fn wheel_turned(&self, direction: WheelDirection) -> bool {
        match direction {
            WheelDirection::Up => self.wheel > 0.0,
            WheelDirection::Down => self.wheel < 0.0,
        }
    }

    /// The (move, look) sticks of whichever pad is tilted furthest.
    fn sticks(&self, swap: bool) -> (Vec2, Vec2) {
        let pick = |stick: fn(&Gamepad) -> Vec2| {
            self.pads
                .iter()
                .map(|pad| stick(pad))
                .fold(Vec2::ZERO, |best, s| {
                    if s.length_squared() > best.length_squared() {
                        s
                    } else {
                        best
                    }
                })
        };
        let (left, right) = (pick(Gamepad::left_stick), pick(Gamepad::right_stick));
        if swap { (right, left) } else { (left, right) }
    }
}

pub(crate) fn sample_devices(
    settings: Res<InputSettings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    gamepads: Query<&Gamepad>,
    time: Res<Time<Virtual>>,
    mut actions: ResMut<ActionState>,
    mut sampler: ResMut<LocalSampler>,
) {
    let devices = Devices {
        keys: &keys,
        mouse: &mouse,
        wheel: mouse_scroll.delta.y,
        pads: gamepads.iter().collect(),
    };
    let (pad_move, pad_look) = devices.sticks(settings.swap_sticks);
    let pad_move = apply_dead_zone(pad_move, settings.move_dead_zone, false);

    let previous = *actions;
    let mut current = ActionState::default();
    for action in Action::ALL {
        let bindings = || {
            settings
                .bindings_for(action)
                .iter()
                .copied()
                .filter(|binding| !action.is_gamepad_only() || binding.is_gamepad())
        };
        current.set(
            action,
            bindings().any(|b| devices.held(b)),
            bindings().any(|b| devices.just_pressed(b)),
        );
    }
    // The movement stick also navigates menus, with an edge each time it crosses the
    // threshold.
    for (action, tilted) in [
        (Action::UiUp, pad_move.y > STICK_MENU_THRESHOLD),
        (Action::UiDown, pad_move.y < -STICK_MENU_THRESHOLD),
        (Action::UiLeft, pad_move.x < -STICK_MENU_THRESHOLD),
        (Action::UiRight, pad_move.x > STICK_MENU_THRESHOLD),
    ] {
        let was_held = previous.held(action);
        current.set(action, tilted, tilted && !was_held);
    }
    *actions = current;

    // While the game clock is stopped (menus, pause), gameplay input is dropped rather
    // than saved up for the next tick.
    if time.is_paused() {
        sampler.pressed = Buttons::NONE;
        sampler.look = Vec2::ZERO;
        return;
    }

    let mut held = Buttons::NONE;
    let mut pressed = Buttons::NONE;
    for action in Action::ALL.into_iter().filter(|a| a.is_gameplay()) {
        if current.held(action) {
            held.insert(action);
        }
        if current.just_pressed(action) {
            pressed.insert(action);
        }
    }
    sampler.held = held;
    sampler.pressed |= pressed;

    let move_keys = settings.move_keys;
    let axis = |positive: KeyCode, negative: KeyCode| {
        f32::from(i8::from(keys.pressed(positive)) - i8::from(keys.pressed(negative)))
    };
    let key_move = Vec2::new(
        axis(move_keys.right, move_keys.left),
        axis(move_keys.forward, move_keys.back),
    );
    // Keys push each axis fully, like a PS2 stick pressed into a corner.
    let movement = if key_move != Vec2::ZERO {
        key_move
    } else {
        circle_to_square(pad_move)
    };
    sampler.move_stick = [to_stick_counts(movement.x), to_stick_counts(movement.y)];

    // The look stick goes to the simulation as it is, like the movement stick: each
    // camera reads it with its game's own dead zone and response.
    let mut pad_look = circle_to_square(apply_dead_zone(pad_look, settings.look_dead_zone, false));
    let mut look =
        Vec2::new(mouse_motion.delta.x, -mouse_motion.delta.y) * settings.mouse_sensitivity;
    if settings.invert_look_x {
        look.x = -look.x;
        pad_look.x = -pad_look.x;
    }
    if settings.invert_look_y {
        look.y = -look.y;
        pad_look.y = -pad_look.y;
    }
    sampler.look_stick = [to_stick_counts(pad_look.x), to_stick_counts(pad_look.y)];
    sampler.look += look;
}

/// Stretches a stick reading from a round range to a square one, keeping its direction.
///
/// Modern pads report a circle, so a full diagonal reads about 0.71 on each axis. The
/// games' stick code (`docs/research/hero-controller.md`, "Input") gives full speed only
/// when the larger axis or the average of it and the length reaches 1, as a PS2 stick does
/// when pushed into a corner, so an unstretched diagonal would run at two-thirds speed.
fn circle_to_square(stick: Vec2) -> Vec2 {
    let largest_axis = stick.x.abs().max(stick.y.abs());
    if largest_axis <= f32::EPSILON {
        return Vec2::ZERO;
    }
    stick * (stick.length().min(1.0) / largest_axis)
}

fn to_stick_counts(axis: f32) -> i8 {
    (axis.clamp(-1.0, 1.0) * 127.0).round() as i8
}

/// Whole angle units in `radians`, and what is left over.
fn to_angle_units(radians: f32) -> (i16, f32) {
    let units_per_radian = ANGLE_UNITS_PER_TURN / TAU;
    let units = (radians * units_per_radian)
        .round()
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX));
    (units as i16, radians - units / units_per_radian)
}

/// Sets every slot's [`PlayerInput`] for this tick, and records it if a recorder is on.
pub(crate) fn latch_tick_inputs(
    local: Res<LocalPlayer>,
    mut sampler: ResMut<LocalSampler>,
    mut source: ResMut<InputSource>,
    mut inputs: ResMut<PlayerInputs>,
    recorder: Option<ResMut<InputRecorder>>,
) {
    match &mut *source {
        InputSource::Local => {
            let (look_x, rest_x) = to_angle_units(sampler.look.x);
            let (look_y, rest_y) = to_angle_units(sampler.look.y);
            sampler.look = Vec2::new(rest_x, rest_y);
            if let Some(slot) = inputs.0.get_mut(local.0) {
                *slot = PlayerInput {
                    move_stick: sampler.move_stick,
                    look: [look_x, look_y],
                    look_stick: sampler.look_stick,
                    held: sampler.held,
                    pressed: sampler.pressed,
                };
            }
            sampler.pressed = Buttons::NONE;
        }
        InputSource::Replay {
            recording,
            next_tick,
        } => {
            let recorded = recording.tick(*next_tick);
            for (player, slot) in inputs.0.iter_mut().enumerate() {
                *slot = recorded
                    .and_then(|tick| tick.get(player).copied())
                    .unwrap_or_default();
            }
            *next_tick += 1;
        }
    }
    if let Some(mut recorder) = recorder {
        recorder.0.push_tick(&inputs.0);
    }
}

/// Clears all pending and latched input and rewinds a replay. Run it when a level starts,
/// together with resetting `rac_sim::SimTick`.
pub fn reset_inputs(
    mut sampler: ResMut<LocalSampler>,
    mut source: ResMut<InputSource>,
    mut inputs: ResMut<PlayerInputs>,
) {
    sampler.pressed = Buttons::NONE;
    sampler.look = Vec2::ZERO;
    source.rewind();
    *inputs = PlayerInputs::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_sticks_become_square() {
        let diagonal = Vec2::new(1.0, 1.0).normalize();
        let square = circle_to_square(diagonal);
        assert!((square - Vec2::ONE).length() < 1e-6, "{square}");
        assert_eq!(circle_to_square(Vec2::new(0.5, 0.0)), Vec2::new(0.5, 0.0));
        let half_diagonal = circle_to_square(diagonal * 0.5);
        assert!((half_diagonal - Vec2::splat(0.5)).length() < 1e-6);
        assert_eq!(circle_to_square(Vec2::ZERO), Vec2::ZERO);
    }

    #[test]
    fn stick_counts() {
        assert_eq!(to_stick_counts(1.0), 127);
        assert_eq!(to_stick_counts(-1.0), -127);
        assert_eq!(to_stick_counts(0.5), 64);
        assert_eq!(to_stick_counts(3.0), 127);
    }

    #[test]
    fn angle_units_carry_the_remainder() {
        let one_unit = TAU / ANGLE_UNITS_PER_TURN;
        let (units, rest) = to_angle_units(0.4 * one_unit);
        assert_eq!(units, 0);
        assert!((rest - 0.4 * one_unit).abs() < 1e-9);
        let (units, _) = to_angle_units(rest + 0.4 * one_unit);
        assert_eq!(units, 1);
        assert_eq!(to_angle_units(100.0).0, i16::MAX);
    }
}
