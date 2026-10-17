//! One player's input for one simulation tick.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

use crate::{Action, Buttons};

/// The most players a session can hold.
pub const MAX_PLAYERS: usize = 4;

/// Binary angle units in a full turn. Look deltas travel in these so every peer turns by
/// exactly the same amount.
pub const ANGLE_UNITS_PER_TURN: f32 = 65_536.0;

/// Everything the simulation learns from one player in one tick: 12 bytes, which replays
/// store and the netcode sends.
///
/// All of it is integers, so a replay or a remote peer reproduces it exactly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerInput {
    /// The movement stick: x to the right, y forward. In PS2 stick counts (−127..=127 from
    /// centre), which is what the games' own stick code works on
    /// (`docs/research/hero-controller.md`, "Input"). The range is square, as on a PS2
    /// stick: keys and a pad pushed fully diagonally give 127 on both axes.
    pub move_stick: [i8; 2],
    /// How far the mouse turns the view this tick, in binary angle units
    /// ([`ANGLE_UNITS_PER_TURN`] per turn): x turns right, y looks up. The player's
    /// sensitivity and invert settings are already applied.
    pub look: [i16; 2],
    /// The look stick, in PS2 stick counts like `move_stick`: x to the right, y up (looking
    /// up, once the player's invert settings are applied). The camera turns it into a turn
    /// rate with the game's own response, which differs between its cameras
    /// (`docs/research/camera.md`).
    pub look_stick: [i8; 2],
    /// Gameplay buttons held at the tick.
    pub held: Buttons,
    /// Gameplay buttons pressed since the previous tick, including taps too short to be
    /// held at any tick.
    pub pressed: Buttons,
}

impl PlayerInput {
    pub const ENCODED_LEN: usize = 12;

    /// The movement stick as a vector with each axis in −1..=1.
    pub fn move_vector(&self) -> Vec2 {
        Vec2::new(
            f32::from(self.move_stick[0]) / 127.0,
            f32::from(self.move_stick[1]) / 127.0,
        )
    }

    /// The look stick as a vector with each axis in −1..=1: x right, y up.
    pub fn look_vector(&self) -> Vec2 {
        Vec2::new(
            f32::from(self.look_stick[0]) / 127.0,
            f32::from(self.look_stick[1]) / 127.0,
        )
    }

    /// The mouse's look delta in radians: x turns right, y looks up.
    pub fn look_radians(&self) -> Vec2 {
        Vec2::new(
            f32::from(self.look[0]) * (TAU / ANGLE_UNITS_PER_TURN),
            f32::from(self.look[1]) * (TAU / ANGLE_UNITS_PER_TURN),
        )
    }

    pub fn held(&self, action: Action) -> bool {
        self.held.contains(action)
    }

    pub fn pressed(&self, action: Action) -> bool {
        self.pressed.contains(action)
    }

    /// The wire and replay encoding, little-endian.
    pub fn to_bytes(&self) -> [u8; Self::ENCODED_LEN] {
        let mut bytes = [0; Self::ENCODED_LEN];
        bytes[0] = self.move_stick[0] as u8;
        bytes[1] = self.move_stick[1] as u8;
        bytes[2..4].copy_from_slice(&self.look[0].to_le_bytes());
        bytes[4..6].copy_from_slice(&self.look[1].to_le_bytes());
        bytes[6..8].copy_from_slice(&self.held.0.to_le_bytes());
        bytes[8..10].copy_from_slice(&self.pressed.0.to_le_bytes());
        bytes[10] = self.look_stick[0] as u8;
        bytes[11] = self.look_stick[1] as u8;
        bytes
    }

    pub fn from_bytes(bytes: &[u8; Self::ENCODED_LEN]) -> Self {
        Self {
            move_stick: [bytes[0] as i8, bytes[1] as i8],
            look: [
                i16::from_le_bytes([bytes[2], bytes[3]]),
                i16::from_le_bytes([bytes[4], bytes[5]]),
            ],
            held: Buttons(u16::from_le_bytes([bytes[6], bytes[7]])),
            pressed: Buttons(u16::from_le_bytes([bytes[8], bytes[9]])),
            look_stick: [bytes[10] as i8, bytes[11] as i8],
        }
    }
}

/// The current tick's input for every player slot. Simulation systems read only this.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct PlayerInputs(pub [PlayerInput; MAX_PLAYERS]);

impl PlayerInputs {
    /// The input for `player`, or a neutral input for a slot out of range.
    pub fn get(&self, player: usize) -> PlayerInput {
        self.0.get(player).copied().unwrap_or_default()
    }
}

/// Which slot this machine's own player fills.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LocalPlayer(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip() {
        let mut held = Buttons::NONE;
        held.insert(Action::Jump);
        held.insert(Action::PrevWeapon);
        let mut pressed = Buttons::NONE;
        pressed.insert(Action::Fire);
        let input = PlayerInput {
            move_stick: [-127, 64],
            look: [i16::MIN, 1234],
            look_stick: [127, -100],
            held,
            pressed,
        };
        assert_eq!(PlayerInput::from_bytes(&input.to_bytes()), input);
    }

    #[test]
    fn encoding_is_stable() {
        // The byte layout is the replay and network format; this pins it.
        let input = PlayerInput {
            move_stick: [-1, 2],
            look: [0x0304, -2],
            look_stick: [-3, 4],
            held: Buttons(0x0506),
            pressed: Buttons(0x0708),
        };
        assert_eq!(
            input.to_bytes(),
            [
                0xFF, 0x02, 0x04, 0x03, 0xFE, 0xFF, 0x06, 0x05, 0x08, 0x07, 0xFD, 0x04
            ]
        );
    }

    #[test]
    fn converts_to_floats() {
        let input = PlayerInput {
            move_stick: [127, -127],
            look: [16_384, 0],
            look_stick: [-127, 127],
            ..default()
        };
        assert_eq!(input.move_vector(), Vec2::new(1.0, -1.0));
        assert_eq!(input.look_radians().x, TAU / 4.0);
        assert_eq!(input.look_vector(), Vec2::new(-1.0, 1.0));
    }

    #[test]
    fn out_of_range_players_are_neutral() {
        assert_eq!(
            PlayerInputs::default().get(MAX_PLAYERS + 3),
            PlayerInput::default()
        );
    }
}
