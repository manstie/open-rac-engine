//! Changing bindings, as a controls screen does: the slots it shows, putting a binding in
//! one and settling the clash that makes, and catching the next button the player presses.

use bevy::prelude::*;

use crate::{Action, Binding, Device, InputSettings, MoveKey};

/// How many bindings of each kind of device a controls screen offers an action.
pub const SLOTS: usize = 2;

/// Something the player can bind: an action, or one of the keys that move them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    Action(Action),
    Move(MoveKey),
}

impl Control {
    /// How many bindings on `device` the control takes: a movement key one key and no
    /// gamepad button (the stick moves), lock-on no key or mouse button (it is gamepad
    /// only), and everything else [`SLOTS`].
    pub fn slots(self, device: Device) -> usize {
        match (self, device) {
            (Control::Move(_), Device::KeyboardMouse) => 1,
            (Control::Move(_), Device::Gamepad) => 0,
            (Control::Action(action), Device::KeyboardMouse) if action.is_gamepad_only() => 0,
            (Control::Action(_), _) => SLOTS,
        }
    }

    fn is_menu(self) -> bool {
        matches!(self, Control::Action(action) if action.is_menu())
    }

    fn must_stay_bound(self) -> bool {
        match self {
            Control::Action(action) => action.must_stay_bound(),
            Control::Move(_) => true,
        }
    }
}

/// One binding cell on a controls screen: the `index`th of the control's bindings on
/// `device`. Filling the slot after the last binding adds one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Slot {
    pub control: Control,
    pub device: Device,
    pub index: usize,
}

/// What putting a binding in a slot did to another control that had it. Two controls
/// clash only when both are read in play, or both in menus: Space can jump and choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The other control took the binding the slot had instead.
    Swapped { control: Control, took: Binding },
    /// The other control lost the binding, as the slot had none to give it.
    Removed { control: Control },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RebindError {
    #[error("the control has no such slot")]
    NoSlot,
    #[error("the binding is on the other kind of device, or isn't a key for a movement key")]
    Unsuitable,
    #[error("{0:?} would be left with no binding on that kind of device")]
    LastBinding(Control),
}

impl InputSettings {
    /// The binding in `slot`, if it has one.
    pub fn binding_in(&self, slot: Slot) -> Option<Binding> {
        if slot.index >= slot.control.slots(slot.device) {
            return None;
        }
        match slot.control {
            Control::Move(key) => Some(Binding::Key(self.move_keys.get(key))),
            Control::Action(action) => self
                .bindings_for(action)
                .iter()
                .copied()
                .filter(|binding| binding.device() == slot.device)
                .nth(slot.index),
        }
    }

    /// Puts `binding` in `slot`. Another control that clashes with it over the binding
    /// takes the slot's old binding in exchange, or loses the binding if the slot had
    /// none; the changes are returned. Nothing changes on an error.
    pub fn assign(&mut self, slot: Slot, binding: Binding) -> Result<Vec<Change>, RebindError> {
        if slot.index >= slot.control.slots(slot.device) {
            return Err(RebindError::NoSlot);
        }
        if binding.device() != slot.device || !suits(slot.control, binding) {
            return Err(RebindError::Unsuitable);
        }
        let old = self.binding_in(slot);
        if old == Some(binding) {
            return Ok(Vec::new());
        }
        let mut next = self.clone();
        let mut changes = Vec::new();
        match slot.control {
            Control::Move(key) => {
                let Binding::Key(code) = binding else {
                    unreachable!("checked by `suits`");
                };
                next.move_keys.set(key, code);
            }
            Control::Action(action) => {
                let list = next.bindings.entry(action).or_default();
                let at = position_of(list, slot);
                match (list.iter().position(|b| *b == binding), at) {
                    // Already in another of its slots: the two slots swap.
                    (Some(other), Some(at)) => list.swap(other, at),
                    (Some(_), None) => {}
                    (None, Some(at)) => list[at] = binding,
                    (None, None) => list.push(binding),
                }
            }
        }
        for control in next.clashing(slot.control, binding) {
            let replacement = old.filter(|old| suits(control, *old) && !next.has(control, *old));
            match replacement {
                Some(took) => {
                    next.replace(control, binding, Some(took));
                    changes.push(Change::Swapped { control, took });
                }
                None if control.must_stay_bound() && next.count_on(control, slot.device) <= 1 => {
                    return Err(RebindError::LastBinding(control));
                }
                None => {
                    next.replace(control, binding, None);
                    changes.push(Change::Removed { control });
                }
            }
        }
        *self = next;
        Ok(changes)
    }

    /// Empties `slot`; the bindings after it move up. A movement key, or the last binding
    /// on a device of an action that must stay bound, can't be cleared.
    pub fn clear(&mut self, slot: Slot) -> Result<(), RebindError> {
        let Some(binding) = self.binding_in(slot) else {
            return Ok(());
        };
        if slot.control.must_stay_bound() && self.count_on(slot.control, slot.device) <= 1 {
            return Err(RebindError::LastBinding(slot.control));
        }
        self.replace(slot.control, binding, None);
        Ok(())
    }

    /// Puts every binding and movement key back to its default, leaving the other options.
    pub fn reset_bindings(&mut self) {
        let defaults = InputSettings::default();
        self.bindings = defaults.bindings;
        self.move_keys = defaults.move_keys;
    }

    /// The controls other than `control`, read where it is, that have `binding`.
    fn clashing(&self, control: Control, binding: Binding) -> Vec<Control> {
        let actions = Action::ALL.into_iter().map(Control::Action);
        let keys = MoveKey::ALL.into_iter().map(Control::Move);
        actions
            .chain(keys)
            .filter(|other| {
                *other != control
                    && other.is_menu() == control.is_menu()
                    && self.has(*other, binding)
            })
            .collect()
    }

    fn has(&self, control: Control, binding: Binding) -> bool {
        match control {
            Control::Move(key) => binding == Binding::Key(self.move_keys.get(key)),
            Control::Action(action) => self.bindings_for(action).contains(&binding),
        }
    }

    fn count_on(&self, control: Control, device: Device) -> usize {
        match control {
            Control::Move(_) => usize::from(device == Device::KeyboardMouse),
            Control::Action(action) => self
                .bindings_for(action)
                .iter()
                .filter(|binding| binding.device() == device)
                .count(),
        }
    }

    /// Replaces `control`'s `binding` with `with`, or removes it. A movement key is only
    /// ever replaced.
    fn replace(&mut self, control: Control, binding: Binding, with: Option<Binding>) {
        match control {
            Control::Move(key) => {
                if let Some(Binding::Key(code)) = with {
                    self.move_keys.set(key, code);
                }
            }
            Control::Action(action) => {
                let Some(list) = self.bindings.get_mut(&action) else {
                    return;
                };
                match with {
                    Some(with) => {
                        for b in list.iter_mut().filter(|b| **b == binding) {
                            *b = with;
                        }
                    }
                    None => list.retain(|b| *b != binding),
                }
            }
        }
    }
}

/// Whether `control` can take `binding` at all.
fn suits(control: Control, binding: Binding) -> bool {
    match control {
        Control::Move(_) => matches!(binding, Binding::Key(_)),
        Control::Action(action) => !action.is_gamepad_only() || binding.is_gamepad(),
    }
}

/// Where in an action's list of bindings `slot`'s binding is.
fn position_of(list: &[Binding], slot: Slot) -> Option<usize> {
    list.iter()
        .enumerate()
        .filter(|(_, binding)| binding.device() == slot.device)
        .nth(slot.index)
        .map(|(at, _)| at)
}

/// Catches the next button the player presses, for a controls screen to bind. While it
/// listens, presses go to it and no action is pressed, so the button doesn't also do what
/// it is bound to.
///
/// Keys, mouse buttons and the wheel are caught as they go down. A gamepad button is
/// caught when it comes up, or as a second button on the same pad goes down while it is
/// held, making a two-button chord ([`Binding::PadBoth`]). Escape, or a press on the
/// other kind of device, calls it off.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BindingCapture(CaptureState);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum CaptureState {
    #[default]
    Off,
    Listening {
        device: Device,
        /// The pad and the button held down on it, for a gamepad slot.
        held: Option<(Entity, GamepadButton)>,
    },
    Caught(Caught),
}

/// What a [`BindingCapture`] caught.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caught {
    Binding(Binding),
    Cancelled,
}

impl BindingCapture {
    /// Starts listening for a binding on `device`.
    pub fn listen(&mut self, device: Device) {
        self.0 = CaptureState::Listening { device, held: None };
    }

    pub fn is_listening(&self) -> bool {
        matches!(self.0, CaptureState::Listening { .. })
    }

    /// What was caught, once.
    pub fn take(&mut self) -> Option<Caught> {
        match self.0 {
            CaptureState::Caught(caught) => {
                self.0 = CaptureState::Off;
                Some(caught)
            }
            _ => None,
        }
    }

    /// Stops listening, catching nothing.
    pub fn stop(&mut self) {
        self.0 = CaptureState::Off;
    }

    /// Looks at this frame's presses while listening.
    pub(crate) fn watch(
        &mut self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
        wheel: f32,
        pads: &[(Entity, &Gamepad)],
    ) {
        let CaptureState::Listening { device, held } = self.0 else {
            return;
        };
        let key = keys.get_just_pressed().next().copied();
        let button = mouse.get_just_pressed().next().copied();
        let pad_press = pads
            .iter()
            .find_map(|(entity, pad)| Some((*entity, *pad.get_just_pressed().next()?)));
        let caught = if key == Some(KeyCode::Escape) {
            Some(Caught::Cancelled)
        } else {
            match device {
                Device::KeyboardMouse => {
                    if let Some(key) = key {
                        Some(Caught::Binding(Binding::Key(key)))
                    } else if let Some(button) = button {
                        Some(Caught::Binding(Binding::Mouse(button)))
                    } else if wheel != 0.0 {
                        let direction = if wheel > 0.0 {
                            crate::WheelDirection::Up
                        } else {
                            crate::WheelDirection::Down
                        };
                        Some(Caught::Binding(Binding::Wheel(direction)))
                    } else {
                        pad_press.map(|_| Caught::Cancelled)
                    }
                }
                Device::Gamepad if key.is_some() || button.is_some() => Some(Caught::Cancelled),
                Device::Gamepad => match held {
                    None => {
                        if let Some((entity, first)) = pad_press {
                            self.0 = CaptureState::Listening {
                                device,
                                held: Some((entity, first)),
                            };
                            // Two buttons down in one frame make a chord at once.
                            pad_second(pads, entity, first).map(|second| chord(first, second))
                        } else {
                            None
                        }
                    }
                    Some((entity, first)) => match pads.iter().find(|(e, _)| *e == entity) {
                        None => Some(Caught::Cancelled),
                        Some((_, pad)) => match pad_second(pads, entity, first) {
                            Some(second) => Some(chord(first, second)),
                            None if !pad.pressed(first) => {
                                Some(Caught::Binding(Binding::Pad(first)))
                            }
                            None => None,
                        },
                    },
                },
            }
        };
        if let Some(caught) = caught {
            self.0 = CaptureState::Caught(caught);
        }
    }
}

/// A button other than `first` that went down on `pad` this frame.
fn pad_second(
    pads: &[(Entity, &Gamepad)],
    pad: Entity,
    first: GamepadButton,
) -> Option<GamepadButton> {
    let (_, pad) = pads.iter().find(|(entity, _)| *entity == pad)?;
    pad.get_just_pressed()
        .copied()
        .find(|button| *button != first)
}

/// Two buttons held together, in a fixed order, so a chord made either way round is one
/// binding.
fn chord(a: GamepadButton, b: GamepadButton) -> Caught {
    Caught::Binding(Binding::PadBoth(a.min(b), a.max(b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Binding::{Key, Mouse, Pad, PadBoth};

    fn slot(action: Action, device: Device, index: usize) -> Slot {
        Slot {
            control: Control::Action(action),
            device,
            index,
        }
    }

    fn keys(action: Action) -> Slot {
        slot(action, Device::KeyboardMouse, 0)
    }

    #[test]
    fn slots_read_each_devices_bindings_in_order() {
        let settings = InputSettings::default();
        let next = |device, index| settings.binding_in(slot(Action::NextWeapon, device, index));
        assert_eq!(
            next(Device::KeyboardMouse, 0),
            Some(Binding::Wheel(crate::WheelDirection::Down))
        );
        assert_eq!(next(Device::KeyboardMouse, 1), Some(Key(KeyCode::KeyE)));
        assert_eq!(
            next(Device::Gamepad, 0),
            Some(Pad(GamepadButton::DPadRight))
        );
        assert_eq!(next(Device::Gamepad, 1), None);
        assert_eq!(settings.binding_in(keys(Action::LockOn)), None);
        let forward = Slot {
            control: Control::Move(MoveKey::Forward),
            device: Device::KeyboardMouse,
            index: 0,
        };
        assert_eq!(settings.binding_in(forward), Some(Key(KeyCode::KeyW)));
    }

    #[test]
    fn a_free_binding_replaces_the_slots() {
        let mut settings = InputSettings::default();
        let changes = settings
            .assign(keys(Action::Jump), Key(KeyCode::KeyJ))
            .unwrap();
        assert!(changes.is_empty());
        assert_eq!(
            settings.binding_in(keys(Action::Jump)),
            Some(Key(KeyCode::KeyJ))
        );
        // The gamepad's binding is untouched.
        assert_eq!(
            settings.bindings_for(Action::Jump)[0],
            Pad(GamepadButton::South)
        );
    }

    #[test]
    fn an_empty_slot_adds_a_binding() {
        let mut settings = InputSettings::default();
        settings
            .assign(
                slot(Action::Jump, Device::KeyboardMouse, 1),
                Key(KeyCode::KeyJ),
            )
            .unwrap();
        assert_eq!(
            settings.bindings_for(Action::Jump),
            [
                Pad(GamepadButton::South),
                Key(KeyCode::Space),
                Key(KeyCode::KeyJ)
            ]
        );
    }

    #[test]
    fn a_taken_binding_swaps_with_the_slots_old_one() {
        let mut settings = InputSettings::default();
        // F is the wrench's; Jump had Space.
        let changes = settings
            .assign(keys(Action::Jump), Key(KeyCode::KeyF))
            .unwrap();
        assert_eq!(
            changes,
            [Change::Swapped {
                control: Control::Action(Action::Melee),
                took: Key(KeyCode::Space),
            }]
        );
        assert_eq!(
            settings.binding_in(keys(Action::Jump)),
            Some(Key(KeyCode::KeyF))
        );
        assert_eq!(
            settings.binding_in(keys(Action::Melee)),
            Some(Key(KeyCode::Space))
        );
    }

    #[test]
    fn into_an_empty_slot_a_taken_binding_moves() {
        let mut settings = InputSettings::default();
        let changes = settings
            .assign(
                slot(Action::Jump, Device::KeyboardMouse, 1),
                Key(KeyCode::KeyF),
            )
            .unwrap();
        assert_eq!(
            changes,
            [Change::Removed {
                control: Control::Action(Action::Melee),
            }]
        );
        assert!(
            !settings
                .bindings_for(Action::Melee)
                .contains(&Key(KeyCode::KeyF))
        );
    }

    #[test]
    fn play_and_menus_dont_clash() {
        let mut settings = InputSettings::default();
        // Enter chooses in menus; jumping with it takes nothing from the menus.
        let changes = settings
            .assign(keys(Action::Jump), Key(KeyCode::Enter))
            .unwrap();
        assert!(changes.is_empty());
        assert!(
            settings
                .bindings_for(Action::UiAccept)
                .contains(&Key(KeyCode::Enter))
        );
    }

    #[test]
    fn movement_keys_swap_too() {
        let mut settings = InputSettings::default();
        let changes = settings
            .assign(keys(Action::Jump), Key(KeyCode::KeyW))
            .unwrap();
        assert_eq!(
            changes,
            [Change::Swapped {
                control: Control::Move(MoveKey::Forward),
                took: Key(KeyCode::Space),
            }]
        );
        assert_eq!(settings.move_keys.forward, KeyCode::Space);
        let forward = Slot {
            control: Control::Move(MoveKey::Forward),
            device: Device::KeyboardMouse,
            index: 0,
        };
        settings.assign(forward, Key(KeyCode::KeyS)).unwrap();
        assert_eq!(settings.move_keys.forward, KeyCode::KeyS);
        assert_eq!(settings.move_keys.back, KeyCode::Space);
        assert_eq!(
            settings.assign(forward, Mouse(MouseButton::Left)),
            Err(RebindError::Unsuitable)
        );
        assert_eq!(
            settings.clear(forward),
            Err(RebindError::LastBinding(forward.control))
        );
    }

    #[test]
    fn a_key_a_movement_key_has_cant_move_into_an_empty_slot() {
        let mut settings = InputSettings::default();
        let before = settings.clone();
        assert_eq!(
            settings.assign(
                slot(Action::Jump, Device::KeyboardMouse, 1),
                Key(KeyCode::KeyW)
            ),
            Err(RebindError::LastBinding(Control::Move(MoveKey::Forward)))
        );
        assert_eq!(settings, before);
    }

    #[test]
    fn the_menus_keep_a_binding_on_each_device() {
        let mut settings = InputSettings::default();
        let pause = keys(Action::Pause);
        assert_eq!(
            settings.clear(pause),
            Err(RebindError::LastBinding(pause.control))
        );
        // UiBack has two keys, so one can go, but not the other.
        settings.clear(keys(Action::UiBack)).unwrap();
        assert_eq!(
            settings.clear(keys(Action::UiBack)),
            Err(RebindError::LastBinding(Control::Action(Action::UiBack)))
        );
        // Clearing what may go is fine.
        settings.clear(keys(Action::Map)).unwrap();
        assert!(settings.binding_in(keys(Action::Map)).is_none());
        settings.clear(keys(Action::Map)).unwrap();
    }

    #[test]
    fn a_binding_already_in_another_of_its_slots_swaps_round() {
        let mut settings = InputSettings::default();
        let second = slot(Action::UiAccept, Device::KeyboardMouse, 1);
        assert_eq!(settings.binding_in(second), Some(Key(KeyCode::Space)));
        settings
            .assign(keys(Action::UiAccept), Key(KeyCode::Space))
            .unwrap();
        assert_eq!(
            settings.binding_in(keys(Action::UiAccept)),
            Some(Key(KeyCode::Space))
        );
        assert_eq!(settings.binding_in(second), Some(Key(KeyCode::Enter)));
    }

    #[test]
    fn lock_on_takes_only_gamepad_bindings() {
        let mut settings = InputSettings::default();
        assert_eq!(
            settings.assign(keys(Action::LockOn), Key(KeyCode::KeyL)),
            Err(RebindError::NoSlot)
        );
        let pad = slot(Action::LockOn, Device::Gamepad, 0);
        assert_eq!(
            settings.assign(pad, Key(KeyCode::KeyL)),
            Err(RebindError::Unsuitable)
        );
        settings
            .assign(
                pad,
                PadBoth(GamepadButton::LeftTrigger, GamepadButton::RightTrigger),
            )
            .unwrap();
        // A chord clashes with neither of its buttons.
        assert!(
            settings
                .bindings_for(Action::FirstPerson)
                .contains(&Pad(GamepadButton::LeftTrigger))
        );
    }

    #[test]
    fn resetting_restores_the_bindings_and_keeps_the_options() {
        let mut settings = InputSettings {
            invert_look_y: true,
            ..default()
        };
        settings
            .assign(keys(Action::Jump), Key(KeyCode::KeyF))
            .unwrap();
        settings.move_keys.forward = KeyCode::ArrowUp;
        settings.reset_bindings();
        let defaults = InputSettings::default();
        assert_eq!(settings.bindings, defaults.bindings);
        assert_eq!(settings.move_keys, defaults.move_keys);
        assert!(settings.invert_look_y);
    }

    struct Devices {
        keys: ButtonInput<KeyCode>,
        mouse: ButtonInput<MouseButton>,
        pad: Gamepad,
    }

    impl Devices {
        fn new() -> Self {
            Self {
                keys: ButtonInput::default(),
                mouse: ButtonInput::default(),
                pad: Gamepad::default(),
            }
        }

        /// One frame: what was pressed last frame is held now.
        fn next_frame(&mut self) {
            self.keys.clear();
            self.mouse.clear();
            self.pad.digital_mut().clear();
        }

        fn watch(&self, capture: &mut BindingCapture, wheel: f32) {
            let pad = Entity::from_raw_u32(1).unwrap();
            capture.watch(&self.keys, &self.mouse, wheel, &[(pad, &self.pad)]);
        }
    }

    #[test]
    fn a_key_is_caught_as_it_goes_down() {
        let mut devices = Devices::new();
        let mut capture = BindingCapture::default();
        capture.listen(Device::KeyboardMouse);
        devices.watch(&mut capture, 0.0);
        assert!(capture.is_listening());
        devices.keys.press(KeyCode::KeyG);
        devices.watch(&mut capture, 0.0);
        assert_eq!(capture.take(), Some(Caught::Binding(Key(KeyCode::KeyG))));
        assert_eq!(capture.take(), None);

        capture.listen(Device::KeyboardMouse);
        devices.next_frame();
        devices.watch(&mut capture, -1.0);
        assert_eq!(
            capture.take(),
            Some(Caught::Binding(Binding::Wheel(crate::WheelDirection::Down)))
        );
    }

    #[test]
    fn escape_or_the_other_device_calls_it_off() {
        let mut devices = Devices::new();
        let mut capture = BindingCapture::default();
        capture.listen(Device::KeyboardMouse);
        devices.keys.press(KeyCode::Escape);
        devices.watch(&mut capture, 0.0);
        assert_eq!(capture.take(), Some(Caught::Cancelled));

        devices.next_frame();
        capture.listen(Device::KeyboardMouse);
        devices.pad.digital_mut().press(GamepadButton::South);
        devices.watch(&mut capture, 0.0);
        assert_eq!(capture.take(), Some(Caught::Cancelled));

        devices.next_frame();
        capture.listen(Device::Gamepad);
        devices.mouse.press(MouseButton::Left);
        devices.watch(&mut capture, 0.0);
        assert_eq!(capture.take(), Some(Caught::Cancelled));
    }

    #[test]
    fn a_pad_button_is_caught_as_it_comes_up() {
        let mut devices = Devices::new();
        let mut capture = BindingCapture::default();
        capture.listen(Device::Gamepad);
        devices.pad.digital_mut().press(GamepadButton::North);
        devices.watch(&mut capture, 0.0);
        devices.next_frame();
        devices.watch(&mut capture, 0.0);
        assert!(capture.is_listening());
        devices.pad.digital_mut().release(GamepadButton::North);
        devices.watch(&mut capture, 0.0);
        assert_eq!(
            capture.take(),
            Some(Caught::Binding(Pad(GamepadButton::North)))
        );
    }

    #[test]
    fn a_second_pad_button_makes_a_chord() {
        let mut devices = Devices::new();
        let mut capture = BindingCapture::default();
        capture.listen(Device::Gamepad);
        devices
            .pad
            .digital_mut()
            .press(GamepadButton::RightTrigger2);
        devices.watch(&mut capture, 0.0);
        devices.next_frame();
        devices.pad.digital_mut().press(GamepadButton::LeftTrigger2);
        devices.watch(&mut capture, 0.0);
        // In the defaults' order, whichever went down first.
        assert_eq!(
            capture.take(),
            Some(Caught::Binding(PadBoth(
                GamepadButton::LeftTrigger2,
                GamepadButton::RightTrigger2
            )))
        );
    }
}
