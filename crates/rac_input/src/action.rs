//! What the player can do, independent of which key or button does it.

use serde::{Deserialize, Serialize};

/// A button-like action. Moving and looking are axes, configured separately in
/// `InputSettings`.
///
/// Gameplay actions travel in every tick's `PlayerInput`, so replays record them and the
/// netcode sends them. The others (the map, pausing and menu navigation) only drive local
/// screens and are read once a frame from `ActionState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Action {
    Jump,
    Crouch,
    /// The wrench.
    Melee,
    Fire,
    /// Tap to swap to the last weapon, hold for the quick-select ring.
    QuickSelect,
    /// Hold to strafe: the hero faces the camera and moves with the stick.
    Strafe,
    /// Lock on to a target, as UYA's L2 and R2 held together. Gamepad only: with a mouse
    /// the player aims for themselves, so keyboard and mouse bindings for it are ignored.
    LockOn,
    FirstPerson,
    NextWeapon,
    PrevWeapon,
    Map,
    Pause,
    UiUp,
    UiDown,
    UiLeft,
    UiRight,
    UiAccept,
    UiBack,
}

impl Action {
    pub const ALL: [Action; 18] = [
        Action::Jump,
        Action::Crouch,
        Action::Melee,
        Action::Fire,
        Action::QuickSelect,
        Action::Strafe,
        Action::LockOn,
        Action::FirstPerson,
        Action::NextWeapon,
        Action::PrevWeapon,
        Action::Map,
        Action::Pause,
        Action::UiUp,
        Action::UiDown,
        Action::UiLeft,
        Action::UiRight,
        Action::UiAccept,
        Action::UiBack,
    ];

    /// The bit this action has in [`Buttons`], if it is a gameplay action.
    fn bit(self) -> Option<u16> {
        let index = match self {
            Action::Jump => 0,
            Action::Crouch => 1,
            Action::Melee => 2,
            Action::Fire => 3,
            Action::QuickSelect => 4,
            Action::Strafe => 5,
            Action::FirstPerson => 6,
            Action::NextWeapon => 7,
            Action::PrevWeapon => 8,
            Action::LockOn => 9,
            _ => return None,
        };
        Some(1 << index)
    }

    /// Whether this action is part of the simulation's input.
    pub fn is_gameplay(self) -> bool {
        self.bit().is_some()
    }

    /// Whether only gamepad bindings can trigger this action.
    pub fn is_gamepad_only(self) -> bool {
        self == Action::LockOn
    }
}

/// A set of gameplay actions, one bit each. The bit layout is part of the replay and
/// network format, so existing bits never move.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Buttons(pub u16);

impl Buttons {
    pub const NONE: Buttons = Buttons(0);

    /// Whether `action` is in the set. Always false for actions that aren't gameplay.
    pub fn contains(self, action: Action) -> bool {
        action.bit().is_some_and(|bit| self.0 & bit != 0)
    }

    /// Adds `action`. Does nothing for actions that aren't gameplay.
    pub fn insert(&mut self, action: Action) {
        if let Some(bit) = action.bit() {
            self.0 |= bit;
        }
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Buttons {
    type Output = Buttons;

    fn bitor(self, other: Buttons) -> Buttons {
        Buttons(self.0 | other.0)
    }
}

impl std::ops::BitOrAssign for Buttons {
    fn bitor_assign(&mut self, other: Buttons) {
        self.0 |= other.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameplay_actions_have_distinct_bits() {
        let mut seen = 0u16;
        for action in Action::ALL.into_iter().filter(|a| a.is_gameplay()) {
            let bit = action.bit().unwrap();
            assert_eq!(seen & bit, 0, "{action:?} shares a bit");
            seen |= bit;
        }
        assert_eq!(seen.count_ones(), 10);
    }

    #[test]
    fn menu_actions_never_enter_the_set() {
        let mut buttons = Buttons::NONE;
        buttons.insert(Action::Pause);
        buttons.insert(Action::UiAccept);
        assert!(buttons.is_empty());
        buttons.insert(Action::Jump);
        assert!(buttons.contains(Action::Jump));
        assert!(!buttons.contains(Action::Pause));
    }
}
