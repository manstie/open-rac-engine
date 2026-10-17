//! Bindings and input options, saved with the player's settings.

use bevy::prelude::*;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

use crate::Action;

/// One physical input that can trigger an [`Action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
    Pad(GamepadButton),
    /// Two buttons of one gamepad held together, as UYA locks on with L2 and R2. It counts
    /// as pressed on the frame either goes down while the other is held.
    PadBoth(GamepadButton, GamepadButton),
    /// A notch of the mouse wheel. It counts as pressed for the frame it turns.
    Wheel(WheelDirection),
}

impl Binding {
    pub fn is_gamepad(self) -> bool {
        matches!(self, Binding::Pad(_) | Binding::PadBoth(..))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WheelDirection {
    Up,
    Down,
}

/// The keys that move the player. On a gamepad the left stick moves and the right stick
/// looks (swapped by [`InputSettings::swap_sticks`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveKeys {
    pub forward: KeyCode,
    pub back: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
}

impl Default for MoveKeys {
    fn default() -> Self {
        Self {
            forward: KeyCode::KeyW,
            back: KeyCode::KeyS,
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
        }
    }
}

/// Everything the player can configure about input. Missing fields in a saved file take
/// their defaults, so older settings files keep loading.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputSettings {
    /// Each action's bindings. An action missing from a saved file gets its default ones,
    /// so actions added since the file was written are bound.
    #[serde(deserialize_with = "bindings_or_defaults")]
    pub bindings: BTreeMap<Action, Vec<Binding>>,
    pub move_keys: MoveKeys,
    /// Radial dead zone of the movement stick, as a fraction of full tilt. Below it the
    /// stick reads as centred; above it the value passes through unscaled, because the
    /// hero controller applies the games' own (much larger) dead zone on top.
    pub move_dead_zone: f32,
    /// Radial dead zone of the look stick, as a fraction of full tilt, treated as the
    /// movement stick's is: the camera applies the games' own dead zone on top.
    pub look_dead_zone: f32,
    /// Radians turned per pixel of mouse movement.
    pub mouse_sensitivity: f32,
    pub invert_look_x: bool,
    pub invert_look_y: bool,
    pub swap_sticks: bool,
}

impl Default for InputSettings {
    fn default() -> Self {
        use Binding::{Key, Mouse, Pad, PadBoth, Wheel};
        // The gamepad follows UYA's layout (`docs/re-setup.md`, "Moves on UYA").
        let bindings = [
            (
                Action::Jump,
                vec![Pad(GamepadButton::South), Key(KeyCode::Space)],
            ),
            (
                Action::Crouch,
                vec![Pad(GamepadButton::RightTrigger), Key(KeyCode::ControlLeft)],
            ),
            (
                Action::Melee,
                vec![Pad(GamepadButton::West), Key(KeyCode::KeyF)],
            ),
            (
                Action::Fire,
                vec![Pad(GamepadButton::East), Mouse(MouseButton::Left)],
            ),
            (
                Action::QuickSelect,
                vec![Pad(GamepadButton::North), Key(KeyCode::Tab)],
            ),
            (
                Action::Strafe,
                vec![
                    Pad(GamepadButton::LeftTrigger2),
                    Pad(GamepadButton::RightTrigger2),
                    Mouse(MouseButton::Right),
                ],
            ),
            (
                Action::LockOn,
                vec![PadBoth(
                    GamepadButton::LeftTrigger2,
                    GamepadButton::RightTrigger2,
                )],
            ),
            (
                Action::FirstPerson,
                vec![Pad(GamepadButton::LeftTrigger), Key(KeyCode::KeyV)],
            ),
            (
                Action::NextWeapon,
                vec![
                    Pad(GamepadButton::DPadRight),
                    Wheel(WheelDirection::Down),
                    Key(KeyCode::KeyE),
                ],
            ),
            (
                Action::PrevWeapon,
                vec![
                    Pad(GamepadButton::DPadLeft),
                    Wheel(WheelDirection::Up),
                    Key(KeyCode::KeyQ),
                ],
            ),
            (
                Action::Map,
                vec![Pad(GamepadButton::Select), Key(KeyCode::KeyM)],
            ),
            (
                Action::Pause,
                vec![Pad(GamepadButton::Start), Key(KeyCode::Escape)],
            ),
            (
                Action::UiUp,
                vec![
                    Pad(GamepadButton::DPadUp),
                    Key(KeyCode::ArrowUp),
                    Key(KeyCode::KeyW),
                ],
            ),
            (
                Action::UiDown,
                vec![
                    Pad(GamepadButton::DPadDown),
                    Key(KeyCode::ArrowDown),
                    Key(KeyCode::KeyS),
                ],
            ),
            (
                Action::UiLeft,
                vec![
                    Pad(GamepadButton::DPadLeft),
                    Key(KeyCode::ArrowLeft),
                    Key(KeyCode::KeyA),
                ],
            ),
            (
                Action::UiRight,
                vec![
                    Pad(GamepadButton::DPadRight),
                    Key(KeyCode::ArrowRight),
                    Key(KeyCode::KeyD),
                ],
            ),
            (
                Action::UiAccept,
                vec![
                    Pad(GamepadButton::South),
                    Key(KeyCode::Enter),
                    Key(KeyCode::Space),
                ],
            ),
            (
                Action::UiBack,
                vec![
                    Pad(GamepadButton::East),
                    Key(KeyCode::Escape),
                    Key(KeyCode::Backspace),
                ],
            ),
        ];
        Self {
            bindings: bindings.into_iter().collect(),
            move_keys: MoveKeys::default(),
            move_dead_zone: 0.1,
            look_dead_zone: 0.1,
            mouse_sensitivity: 0.0025,
            invert_look_x: false,
            invert_look_y: false,
            swap_sticks: false,
        }
    }
}

fn bindings_or_defaults<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<Action, Vec<Binding>>, D::Error> {
    let mut bindings = BTreeMap::<Action, Vec<Binding>>::deserialize(deserializer)?;
    for (action, defaults) in InputSettings::default().bindings {
        bindings.entry(action).or_insert(defaults);
    }
    Ok(bindings)
}

impl InputSettings {
    /// The bindings for `action`, which may be none.
    pub fn bindings_for(&self, action: Action) -> &[Binding] {
        self.bindings.get(&action).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// `stick` with a radial dead zone of `dead_zone`. Outside it the value is kept as it is,
/// or, with `rescale`, stretched so the live range starts from zero.
pub fn apply_dead_zone(stick: Vec2, dead_zone: f32, rescale: bool) -> Vec2 {
    let dead_zone = dead_zone.clamp(0.0, 0.99);
    let length = stick.length();
    if length <= dead_zone {
        return Vec2::ZERO;
    }
    let stick = stick.clamp_length_max(1.0);
    if rescale {
        let length = length.min(1.0);
        stick * ((length - dead_zone) / (1.0 - dead_zone) / length)
    } else {
        stick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_a_default_binding() {
        let settings = InputSettings::default();
        for action in Action::ALL {
            assert!(
                !settings.bindings_for(action).is_empty(),
                "{action:?} is unbound"
            );
        }
    }

    #[test]
    fn settings_round_trip_through_toml() {
        let mut settings = InputSettings {
            invert_look_y: true,
            ..default()
        };
        settings
            .bindings
            .insert(Action::Jump, vec![Binding::Key(KeyCode::KeyJ)]);
        let text = toml::to_string(&settings).unwrap();
        let back: InputSettings = toml::from_str(&text).unwrap();
        assert_eq!(back, settings);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let settings: InputSettings = toml::from_str("invert_look_y = true").unwrap();
        assert!(settings.invert_look_y);
        assert_eq!(settings.bindings, InputSettings::default().bindings);
    }

    #[test]
    fn actions_missing_from_a_saved_file_get_their_defaults() {
        let mut saved = InputSettings::default();
        saved.bindings.remove(&Action::LockOn);
        saved
            .bindings
            .insert(Action::Jump, vec![Binding::Key(KeyCode::KeyJ)]);
        saved.bindings.insert(Action::Map, Vec::new());
        let back: InputSettings = toml::from_str(&toml::to_string(&saved).unwrap()).unwrap();
        let defaults = InputSettings::default();
        assert_eq!(
            back.bindings_for(Action::LockOn),
            defaults.bindings_for(Action::LockOn)
        );
        // What the file says stays, an empty list too.
        assert_eq!(
            back.bindings_for(Action::Jump),
            [Binding::Key(KeyCode::KeyJ)]
        );
        assert!(back.bindings_for(Action::Map).is_empty());
    }

    #[test]
    fn dead_zone() {
        assert_eq!(apply_dead_zone(Vec2::new(0.05, 0.0), 0.1, true), Vec2::ZERO);
        assert_eq!(
            apply_dead_zone(Vec2::new(0.5, 0.0), 0.1, false),
            Vec2::new(0.5, 0.0)
        );
        let rescaled = apply_dead_zone(Vec2::new(0.55, 0.0), 0.1, true);
        assert!((rescaled.x - 0.5).abs() < 1e-6);
        assert_eq!(
            apply_dead_zone(Vec2::new(1.0, 0.0), 0.1, true),
            Vec2::new(1.0, 0.0)
        );
    }
}
