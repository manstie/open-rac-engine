//! What the menus call the controls and the bindings. The gamepad's buttons are named as
//! on the PlayStation 2's pad, whose layout the defaults follow.

use bevy::prelude::{GamepadButton, KeyCode, MouseButton};
use rac_input::{Action, Binding, Control, Device, InputSettings, MoveKey, WheelDirection};

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
pub fn binding(binding: Binding) -> String {
    match binding {
        Binding::Key(key) => self::key(key),
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
pub fn bindings_of(settings: &InputSettings, control: Control) -> String {
    let on = |device| {
        let names: Vec<_> = (0..control.slots(device))
            .filter_map(|index| {
                settings.binding_in(rac_input::Slot {
                    control,
                    device,
                    index,
                })
            })
            .map(self::binding)
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

/// The four movement keys together, in the order a keyboard has them: "WASD" when each is
/// a letter, or else listed.
pub fn move_keys(settings: &InputSettings) -> String {
    let names: Vec<_> = [
        MoveKey::Forward,
        MoveKey::Left,
        MoveKey::Back,
        MoveKey::Right,
    ]
    .into_iter()
    .map(|key| self::key(settings.move_keys.get(key)))
    .collect();
    if names.iter().all(|name| name.chars().count() == 1) {
        names.concat()
    } else {
        names.join(", ")
    }
}

fn key(key: KeyCode) -> String {
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
    fn bindings_read_as_the_hint_has_them() {
        let settings = InputSettings::default();
        let of = |action| bindings_of(&settings, Control::Action(action));
        assert_eq!(of(Action::Jump), "Space / Cross");
        assert_eq!(of(Action::Strafe), "right mouse / L2 or R2");
        assert_eq!(of(Action::LockOn), "L2 + R2");
        assert_eq!(of(Action::Crouch), "Left Ctrl / R1");
        assert_eq!(move_keys(&settings), "WASD");
        assert_eq!(binding(Binding::Key(KeyCode::Digit3)), "3");
        assert_eq!(binding(Binding::Key(KeyCode::Numpad3)), "Num 3");
        assert_eq!(binding(Binding::Key(KeyCode::F5)), "F5");
        assert_eq!(capitalised("left mouse"), "Left mouse");
    }

    #[test]
    fn an_action_with_nothing_bound_says_so() {
        let mut settings = InputSettings::default();
        settings.bindings.insert(Action::Map, Vec::new());
        assert_eq!(
            bindings_of(&settings, Control::Action(Action::Map)),
            "unbound"
        );
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
