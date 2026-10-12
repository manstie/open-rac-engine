//! Input recordings: every player's [`PlayerInput`] for every tick, so a session can be
//! replayed exactly (PLAN.md P1.2, P1.19).

use bevy::prelude::*;
use thiserror::Error;

use crate::{MAX_PLAYERS, PlayerInput};

const MAGIC: [u8; 4] = *b"ORIN";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 12;

/// A recorded session's inputs, tick by tick.
///
/// File layout, little-endian: the magic `ORIN`, a `u16` version (1), a `u8` player count,
/// a reserved byte, a `u32` tick count, then for each tick each player's 10-byte
/// [`PlayerInput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRecording {
    players: usize,
    inputs: Vec<PlayerInput>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecordingError {
    #[error("not an input recording (bad magic)")]
    BadMagic,
    #[error("input recording version {0} is not supported (expected {VERSION})")]
    UnsupportedVersion(u16),
    #[error("input recording has {0} players; 1 to {MAX_PLAYERS} are supported")]
    BadPlayerCount(usize),
    #[error("input recording is truncated: expected {expected} bytes, found {found}")]
    Truncated { expected: usize, found: usize },
}

impl InputRecording {
    /// An empty recording for `players` players (clamped to 1..=[`MAX_PLAYERS`]).
    pub fn new(players: usize) -> Self {
        Self {
            players: players.clamp(1, MAX_PLAYERS),
            inputs: Vec::new(),
        }
    }

    pub fn players(&self) -> usize {
        self.players
    }

    pub fn ticks(&self) -> usize {
        self.inputs.len() / self.players
    }

    /// Appends one tick. Missing players are recorded as neutral and extra ones dropped.
    pub fn push_tick(&mut self, inputs: &[PlayerInput]) {
        for player in 0..self.players {
            self.inputs
                .push(inputs.get(player).copied().unwrap_or_default());
        }
    }

    /// Every player's input at `tick`.
    pub fn tick(&self, tick: usize) -> Option<&[PlayerInput]> {
        let start = tick.checked_mul(self.players)?;
        self.inputs.get(start..start + self.players)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes =
            Vec::with_capacity(HEADER_LEN + self.inputs.len() * PlayerInput::ENCODED_LEN);
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.push(self.players as u8);
        bytes.push(0);
        bytes.extend_from_slice(&(self.ticks() as u32).to_le_bytes());
        for input in &self.inputs {
            bytes.extend_from_slice(&input.to_bytes());
        }
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RecordingError> {
        let truncated = |expected| RecordingError::Truncated {
            expected,
            found: bytes.len(),
        };
        let header = bytes.get(..HEADER_LEN).ok_or(truncated(HEADER_LEN))?;
        if header[0..4] != MAGIC {
            return Err(RecordingError::BadMagic);
        }
        let version = u16::from_le_bytes([header[4], header[5]]);
        if version != VERSION {
            return Err(RecordingError::UnsupportedVersion(version));
        }
        let players = usize::from(header[6]);
        if !(1..=MAX_PLAYERS).contains(&players) {
            return Err(RecordingError::BadPlayerCount(players));
        }
        let ticks = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;
        // u32 ticks × at most 4 players × 10 bytes fits in usize on every 64-bit target, but
        // stay safe on 32-bit ones.
        let body_len = ticks
            .checked_mul(players * PlayerInput::ENCODED_LEN)
            .ok_or(truncated(usize::MAX))?;
        let expected = HEADER_LEN.saturating_add(body_len);
        let body = bytes.get(HEADER_LEN..expected).ok_or(truncated(expected))?;
        // `body` is a whole number of inputs long, so nothing is left over.
        let (encoded, _) = body.as_chunks::<{ PlayerInput::ENCODED_LEN }>();
        let inputs = encoded.iter().map(PlayerInput::from_bytes).collect();
        Ok(Self { players, inputs })
    }
}

/// Where the simulation's inputs come from.
#[derive(Resource, Debug, Default)]
pub enum InputSource {
    /// This machine's devices drive the local player's slot.
    #[default]
    Local,
    /// A recording drives every slot. After its last tick, every input is neutral.
    Replay {
        recording: InputRecording,
        next_tick: usize,
    },
}

impl InputSource {
    pub fn replay(recording: InputRecording) -> Self {
        Self::Replay {
            recording,
            next_tick: 0,
        }
    }

    /// Whether a replay has played every recorded tick.
    pub fn is_finished(&self) -> bool {
        match self {
            InputSource::Local => false,
            InputSource::Replay {
                recording,
                next_tick,
            } => *next_tick >= recording.ticks(),
        }
    }

    /// Starts a replay again from its first tick.
    pub fn rewind(&mut self) {
        if let InputSource::Replay { next_tick, .. } = self {
            *next_tick = 0;
        }
    }
}

/// While this resource exists, every tick's [`PlayerInputs`](crate::PlayerInputs) are
/// appended to it.
#[derive(Resource, Debug)]
pub struct InputRecorder(pub InputRecording);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, Buttons};

    fn sample(tick: usize) -> [PlayerInput; 2] {
        let mut held = Buttons::NONE;
        if tick.is_multiple_of(2) {
            held.insert(Action::Jump);
        }
        [
            PlayerInput {
                move_stick: [tick as i8, -(tick as i8)],
                held,
                ..default()
            },
            PlayerInput {
                look: [tick as i16 * 100, 0],
                ..default()
            },
        ]
    }

    #[test]
    fn round_trips() {
        let mut recording = InputRecording::new(2);
        for tick in 0..50 {
            recording.push_tick(&sample(tick));
        }
        let back = InputRecording::from_bytes(&recording.to_bytes()).unwrap();
        assert_eq!(back, recording);
        assert_eq!(back.ticks(), 50);
        assert_eq!(back.tick(7), Some(&sample(7)[..]));
        assert_eq!(back.tick(50), None);
    }

    #[test]
    fn rejects_bad_files() {
        let mut recording = InputRecording::new(1);
        recording.push_tick(&[PlayerInput::default()]);
        let good = recording.to_bytes();

        assert_eq!(
            InputRecording::from_bytes(&good[..5]),
            Err(RecordingError::Truncated {
                expected: HEADER_LEN,
                found: 5
            })
        );
        assert_eq!(
            InputRecording::from_bytes(&good[..good.len() - 1]),
            Err(RecordingError::Truncated {
                expected: good.len(),
                found: good.len() - 1
            })
        );

        let mut bad = good.clone();
        bad[0] = b'X';
        assert_eq!(
            InputRecording::from_bytes(&bad),
            Err(RecordingError::BadMagic)
        );

        let mut bad = good.clone();
        bad[4] = 9;
        assert_eq!(
            InputRecording::from_bytes(&bad),
            Err(RecordingError::UnsupportedVersion(9))
        );

        let mut bad = good.clone();
        bad[6] = 0;
        assert_eq!(
            InputRecording::from_bytes(&bad),
            Err(RecordingError::BadPlayerCount(0))
        );

        let mut bad = good;
        bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            InputRecording::from_bytes(&bad),
            Err(RecordingError::Truncated { .. })
        ));
    }

    #[test]
    fn replay_finishes_after_the_last_tick() {
        let mut recording = InputRecording::new(1);
        recording.push_tick(&[PlayerInput::default()]);
        let mut source = InputSource::replay(recording);
        assert!(!source.is_finished());
        if let InputSource::Replay { next_tick, .. } = &mut source {
            *next_tick = 1;
        }
        assert!(source.is_finished());
        source.rewind();
        assert!(!source.is_finished());
        assert!(!InputSource::Local.is_finished());
    }
}
