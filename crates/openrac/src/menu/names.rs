//! What the menus call the controls and the bindings. The gamepad's buttons are named as
//! on the PlayStation 2's pad, whose layout the defaults follow; the keys as the player's
//! keyboard layout has them, once they've been pressed ([`KeyNames`]).

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use rac_input::{Action, Binding, Control, Device, InputSettings, MoveKey, WheelDirection};
use serde::{Deserialize, de::IntoDeserializer};
use std::collections::BTreeMap;

use crate::settings::{self, Paths};

/// What the keyboard's layout calls the keys it doesn't name as a US keyboard does
/// (`KeyCode` is the key's place): an AZERTY keyboard's `KeyW` is its Z. The platforms
/// have no common way to ask the layout, so each key's name is learned as it is pressed,
/// and saved (`key_names` in `settings.toml`) to be right from the next start.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct KeyNames(BTreeMap<KeyCode, String>);

impl KeyNames {
    /// The names saved, by their keys' `KeyCode` names; any the build doesn't know are
    /// left out.
    pub fn from_saved(saved: &BTreeMap<String, String>) -> Self {
        Self(
            saved
                .iter()
                .filter_map(|(code, name)| {
                    let deserializer: serde::de::value::StrDeserializer<
                        '_,
                        serde::de::value::Error,
                    > = code.as_str().into_deserializer();
                    Some((KeyCode::deserialize(deserializer).ok()?, name.clone()))
                })
                .collect(),
        )
    }

    pub fn to_saved(&self) -> BTreeMap<String, String> {
        self.0
            .iter()
            .map(|(code, name)| (format!("{code:?}"), name.clone()))
            .collect()
    }

    /// Notes that `key` typed `typed`; whether its name changed.
    pub fn learn(&mut self, key: KeyCode, typed: &Key) -> bool {
        let Some(name) = typed_name(key, typed) else {
            return false;
        };
        if name == place_name(key) {
            self.0.remove(&key).is_some()
        } else {
            self.0.insert(key, name.clone()) != Some(name)
        }
    }
}

/// The keys that type a letter or a sign, which layouts move about. The digits are left
/// with their own names: an AZERTY keyboard's 1 types & but has 1 on it too.
fn types_a_sign(key: KeyCode) -> bool {
    use KeyCode::*;
    matches!(
        key,
        KeyA | KeyB
            | KeyC
            | KeyD
            | KeyE
            | KeyF
            | KeyG
            | KeyH
            | KeyI
            | KeyJ
            | KeyK
            | KeyL
            | KeyM
            | KeyN
            | KeyO
            | KeyP
            | KeyQ
            | KeyR
            | KeyS
            | KeyT
            | KeyU
            | KeyV
            | KeyW
            | KeyX
            | KeyY
            | KeyZ
            | Backquote
            | Minus
            | Equal
            | BracketLeft
            | BracketRight
            | Backslash
            | Semicolon
            | Quote
            | Comma
            | Period
            | Slash
            | IntlBackslash
            | IntlRo
            | IntlYen
    )
}

/// What a press of `key` that typed `typed` names it: the sign, a letter in capitals.
fn typed_name(key: KeyCode, typed: &Key) -> Option<String> {
    if !types_a_sign(key) {
        return None;
    }
    let Key::Character(text) = typed else {
        return None;
    };
    let mut chars = text.chars();
    let (Some(sign), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if sign.is_control() || sign.is_whitespace() {
        return None;
    }
    // One capital, not ß's SS.
    let capital: String = sign.to_uppercase().collect();
    Some(if capital.chars().count() == 1 {
        capital
    } else {
        sign.into()
    })
}

/// Learns the names of the keys pressed without a modifier, which would change what they
/// type, and saves them when one changes.
pub fn learn_key_names(
    mut presses: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut names: ResMut<KeyNames>,
    paths: Option<Res<Paths>>,
) {
    let modified = keys.any_pressed([
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::AltLeft,
        KeyCode::AltRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let mut changed = false;
    for press in presses.read() {
        if press.state == ButtonState::Pressed && !press.repeat && !modified {
            changed |= names
                .bypass_change_detection()
                .learn(press.key_code, &press.logical_key);
        }
    }
    if !changed {
        return;
    }
    names.set_changed();
    if let Some(paths) = paths {
        let saved = names.to_saved();
        if let Err(error) = settings::update(&paths.settings_file, |settings| {
            settings.key_names = saved;
        }) {
            warn!("could not save the keys' names: {error:#}");
        }
    }
}

pub fn control(control: Control) -> &'static str {
    match control {
        Control::Move(MoveKey::Forward) => "Move forward",
        Control::Move(MoveKey::Back) => "Move back",
        Control::Move(MoveKey::Left) => "Move left",
        Control::Move(MoveKey::Right) => "Move right",
        Control::Action(action) => self::action(action),
    }
}

pub fn action(action: Action) -> &'static str {
    match action {
        Action::Jump => "Jump",
        Action::Crouch => "Crouch",
        Action::Melee => "Wrench",
        Action::Fire => "Fire",
        Action::QuickSelect => "Quick select",
        Action::Strafe => "Strafe",
        Action::LockOn => "Lock on",
        Action::FirstPerson => "Look round",
        Action::NextWeapon => "Next gun",
        Action::PrevWeapon => "Previous gun",
        Action::Map => "Map",
        Action::Pause => "Pause",
        Action::UiUp => "Menu up",
        Action::UiDown => "Menu down",
        Action::UiLeft => "Menu left",
        Action::UiRight => "Menu right",
        Action::UiAccept => "Menu choose",
        Action::UiBack => "Menu back",
        Action::UiClear => "Clear a binding",
    }
}

/// A binding's name, in lower case where it isn't a name of its own ("left mouse",
/// "d-pad up"), for running text; [`capitalised`] makes a label of it.
pub fn binding(binding: Binding, keys: &KeyNames) -> String {
    match binding {
        Binding::Key(key) => self::key(key, keys),
        Binding::Mouse(button) => mouse(button),
        Binding::Pad(button) => pad(button),
        Binding::PadBoth(a, b) => format!("{} + {}", pad(a), pad(b)),
        Binding::Wheel(WheelDirection::Up) => "wheel up".into(),
        Binding::Wheel(WheelDirection::Down) => "wheel down".into(),
    }
}

pub fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A control's bindings for running text: the keyboard and mouse's, then the gamepad's,
/// as "right mouse / L2 or R2".
pub fn bindings_of(settings: &InputSettings, keys: &KeyNames, control: Control) -> String {
    let on = |device| {
        let names: Vec<_> = (0..control.slots(device))
            .filter_map(|index| {
                settings.binding_in(rac_input::Slot {
                    control,
                    device,
                    index,
                })
            })
            .map(|binding| self::binding(binding, keys))
            .collect();
        names.join(" or ")
    };
    let sides: Vec<_> = [on(Device::KeyboardMouse), on(Device::Gamepad)]
        .into_iter()
        .filter(|side| !side.is_empty())
        .collect();
    if sides.is_empty() {
        "unbound".into()
    } else {
        sides.join(" / ")
    }
}

fn key(key: KeyCode, keys: &KeyNames) -> String {
    keys.0.get(&key).cloned().unwrap_or_else(|| place_name(key))
}

/// What a US keyboard calls the key in `key`'s place.
fn place_name(key: KeyCode) -> String {
    let named = match key {
        KeyCode::Space => "Space",
        KeyCode::Enter => "Enter",
        KeyCode::Escape => "Esc",
        KeyCode::Backspace => "Backspace",
        KeyCode::Tab => "Tab",
        KeyCode::Delete => "Delete",
        KeyCode::Insert => "Insert",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "Page up",
        KeyCode::PageDown => "Page down",
        KeyCode::CapsLock => "Caps lock",
        KeyCode::ShiftLeft => "Left Shift",
        KeyCode::ShiftRight => "Right Shift",
        KeyCode::ControlLeft => "Left Ctrl",
        KeyCode::ControlRight => "Right Ctrl",
        KeyCode::AltLeft => "Left Alt",
        KeyCode::AltRight => "Right Alt",
        KeyCode::SuperLeft => "Left Super",
        KeyCode::SuperRight => "Right Super",
        KeyCode::ArrowUp => "Up",
        KeyCode::ArrowDown => "Down",
        KeyCode::ArrowLeft => "Left",
        KeyCode::ArrowRight => "Right",
        KeyCode::Backquote => "`",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        KeyCode::Backslash => "\\",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "'",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::NumpadEnter => "Num Enter",
        KeyCode::NumpadAdd => "Num +",
        KeyCode::NumpadSubtract => "Num -",
        KeyCode::NumpadMultiply => "Num *",
        KeyCode::NumpadDivide => "Num /",
        KeyCode::NumpadDecimal => "Num .",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    // The rest by their codes' names: `KeyW` is W, `Digit1` is 1, `Numpad1` Num 1, `F5` F5.
    let code = format!("{key:?}");
    if let Some(letter) = code.strip_prefix("Key") {
        letter.into()
    } else if let Some(digit) = code.strip_prefix("Digit") {
        digit.into()
    } else if let Some(digit) = code.strip_prefix("Numpad") {
        format!("Num {digit}")
    } else {
        code
    }
}

fn mouse(button: MouseButton) -> String {
    match button {
        MouseButton::Left => "left mouse".into(),
        MouseButton::Right => "right mouse".into(),
        MouseButton::Middle => "middle mouse".into(),
        MouseButton::Back => "mouse back".into(),
        MouseButton::Forward => "mouse forward".into(),
        MouseButton::Other(number) => format!("mouse {number}"),
    }
}

fn pad(button: GamepadButton) -> String {
    let named = match button {
        GamepadButton::South => "Cross",
        GamepadButton::East => "Circle",
        GamepadButton::North => "Triangle",
        GamepadButton::West => "Square",
        GamepadButton::C => "C",
        GamepadButton::Z => "Z",
        GamepadButton::LeftTrigger => "L1",
        GamepadButton::LeftTrigger2 => "L2",
        GamepadButton::RightTrigger => "R1",
        GamepadButton::RightTrigger2 => "R2",
        GamepadButton::Select => "Select",
        GamepadButton::Start => "Start",
        GamepadButton::Mode => "Home",
        GamepadButton::LeftThumb => "L3",
        GamepadButton::RightThumb => "R3",
        GamepadButton::DPadUp => "d-pad up",
        GamepadButton::DPadDown => "d-pad down",
        GamepadButton::DPadLeft => "d-pad left",
        GamepadButton::DPadRight => "d-pad right",
        GamepadButton::Other(number) => return format!("button {number}"),
    };
    named.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_read_as_running_text() {
        let settings = InputSettings::default();
        let keys = KeyNames::default();
        let of = |action| bindings_of(&settings, &keys, Control::Action(action));
        assert_eq!(of(Action::Jump), "Space / Cross");
        assert_eq!(of(Action::Strafe), "Left Shift / L2 or R2");
        assert_eq!(of(Action::FirstPerson), "right mouse / L1");
        assert_eq!(of(Action::LockOn), "L2 + R2");
        assert_eq!(of(Action::Crouch), "Left Ctrl / R1");
        assert_eq!(
            bindings_of(&settings, &keys, Control::Move(MoveKey::Forward)),
            "W"
        );
        let key = |code| binding(Binding::Key(code), &keys);
        assert_eq!(key(KeyCode::Digit3), "3");
        assert_eq!(key(KeyCode::Numpad3), "Num 3");
        assert_eq!(key(KeyCode::F5), "F5");
        assert_eq!(capitalised("left mouse"), "Left mouse");
    }

    #[test]
    fn an_action_with_nothing_bound_says_so() {
        let mut settings = InputSettings::default();
        settings.bindings.insert(Action::Map, Vec::new());
        assert_eq!(
            bindings_of(
                &settings,
                &KeyNames::default(),
                Control::Action(Action::Map)
            ),
            "unbound"
        );
    }

    #[test]
    fn keys_take_the_names_their_layout_types() {
        let settings = InputSettings::default();
        let mut keys = KeyNames::default();
        let typed = |text: &str| Key::Character(text.into());
        // AZERTY: W's place types z, A's q.
        assert!(keys.learn(KeyCode::KeyW, &typed("z")));
        assert!(keys.learn(KeyCode::KeyA, &typed("q")));
        assert!(!keys.learn(KeyCode::KeyW, &typed("z")));
        assert!(keys.learn(KeyCode::Semicolon, &typed("m")));
        let move_key = |key| bindings_of(&settings, &keys, Control::Move(key));
        assert_eq!(move_key(MoveKey::Forward), "Z");
        assert_eq!(move_key(MoveKey::Left), "Q");
        assert_eq!(move_key(MoveKey::Back), "S");
        assert_eq!(binding(Binding::Key(KeyCode::Semicolon), &keys), "M");
        // QWERTZ's ß stays one sign; the digits and keys typing nothing keep their names.
        assert!(keys.learn(KeyCode::Minus, &typed("ß")));
        assert_eq!(binding(Binding::Key(KeyCode::Minus), &keys), "ß");
        assert!(!keys.learn(KeyCode::Digit1, &typed("&")));
        assert!(!keys.learn(KeyCode::KeyQ, &Key::Dead(Some('^'))));
        // Back on QWERTY a key's name goes, and nothing is kept for it.
        assert!(keys.learn(KeyCode::KeyW, &typed("w")));
        assert_eq!(binding(Binding::Key(KeyCode::KeyW), &keys), "W");
        let saved = keys.to_saved();
        assert_eq!(saved.get("KeyA").map(String::as_str), Some("Q"));
        assert!(!saved.contains_key("KeyW"));
        assert_eq!(KeyNames::from_saved(&saved), keys);
        let mut odd = saved.clone();
        odd.insert("NoSuchKey".into(), "?".into());
        assert_eq!(KeyNames::from_saved(&odd), keys);
    }

    #[test]
    fn every_action_has_a_name() {
        let mut seen = std::collections::BTreeSet::new();
        for action in Action::ALL {
            assert!(
                seen.insert(self::action(action)),
                "{action:?} shares its name"
            );
        }
    }
}
