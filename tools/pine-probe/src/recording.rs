//! Reads PCSX2 input recordings (`.p2m2`, file version 1).
//!
//! Layout, as PCSX2 2.8 writes it (facts from `pcsx2/Recording/InputRecordingFile.*` and
//! `PadData.cpp`, GPL, read for facts only):
//!
//! | Offset | Size | Field |
//! |---|---|---|
//! | 0 | 1 | file version (1) |
//! | 1 | 50 | emulator version, NUL-padded |
//! | 51 | 255 | author, NUL-padded |
//! | 306 | 255 | game name, NUL-padded |
//! | 561 | 4 | total frames, `u32` LE |
//! | 565 | 4 | undo count, `u32` LE |
//! | 569 | 1 | 1 if the recording starts from a savestate |
//! | 570 | 36 × frames | per frame: 18 bytes for port 1, then 18 for port 2 |
//!
//! Per port and frame: two active-low button bytes, the right stick X/Y, the left stick
//! X/Y, then 12 pressure bytes (see [`PRESSURE_ORDER`]). A recording that starts from a
//! savestate expects `<recording file name>_SaveState.p2s` next to it.
//!
//! Every recording we have so far has an all-zero frame 0 (which would read as every
//! button held) and one more frame block than `total_frames`. Consumers should treat
//! frame 0 as "no input".

use std::path::Path;

use thiserror::Error;

pub const HEADER_LEN: usize = 570;
pub const PORT_BYTES: usize = 18;
pub const FRAME_BYTES: usize = 2 * PORT_BYTES;

/// Which button each pressure byte belongs to, in file order.
pub const PRESSURE_ORDER: [&str; 12] = [
    "right", "left", "up", "down", "triangle", "circle", "cross", "square", "l1", "r1", "l2", "r2",
];

#[derive(Debug, Error)]
pub enum RecordingError {
    #[error("cannot read input recording: {0}")]
    Io(#[from] std::io::Error),
    #[error("input recording is too short for its header ({0} bytes)")]
    TooShort(usize),
    #[error("unsupported input recording version {0} (expected 1)")]
    Version(u8),
    #[error("input recording declares {declared} frames but holds only {present}")]
    Truncated { declared: u32, present: usize },
}

/// Buttons held in one frame, active-high, in the PS2 pad's own bit order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Buttons(pub u16);

impl Buttons {
    pub const SELECT: u16 = 1 << 0;
    pub const L3: u16 = 1 << 1;
    pub const R3: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const UP: u16 = 1 << 4;
    pub const RIGHT: u16 = 1 << 5;
    pub const DOWN: u16 = 1 << 6;
    pub const LEFT: u16 = 1 << 7;
    pub const L2: u16 = 1 << 8;
    pub const R2: u16 = 1 << 9;
    pub const L1: u16 = 1 << 10;
    pub const R1: u16 = 1 << 11;
    pub const TRIANGLE: u16 = 1 << 12;
    pub const CIRCLE: u16 = 1 << 13;
    pub const CROSS: u16 = 1 << 14;
    pub const SQUARE: u16 = 1 << 15;

    pub const NAMES: [(u16, &'static str); 16] = [
        (Self::SELECT, "select"),
        (Self::L3, "l3"),
        (Self::R3, "r3"),
        (Self::START, "start"),
        (Self::UP, "up"),
        (Self::RIGHT, "right"),
        (Self::DOWN, "down"),
        (Self::LEFT, "left"),
        (Self::L2, "l2"),
        (Self::R2, "r2"),
        (Self::L1, "l1"),
        (Self::R1, "r1"),
        (Self::TRIANGLE, "triangle"),
        (Self::CIRCLE, "circle"),
        (Self::CROSS, "cross"),
        (Self::SQUARE, "square"),
    ];

    pub fn held(self, button: u16) -> bool {
        self.0 & button != 0
    }

    /// Space-separated names of the held buttons.
    pub fn names(self) -> String {
        Self::NAMES
            .iter()
            .filter(|(bit, _)| self.held(*bit))
            .map(|(_, name)| *name)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// One controller port's input for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PadFrame {
    pub buttons: Buttons,
    /// Raw left stick X and Y. 0 is left/up, 255 right/down, and rest is about 0x7F–0x80.
    pub left_stick: [u8; 2],
    pub right_stick: [u8; 2],
    /// Pressure per button, 0–255, in [`PRESSURE_ORDER`].
    pub pressure: [u8; 12],
}

impl PadFrame {
    fn parse(b: &[u8]) -> Self {
        let raw = u16::from(b[0]) | u16::from(b[1]) << 8;
        Self {
            buttons: Buttons(!raw),
            right_stick: [b[2], b[3]],
            left_stick: [b[4], b[5]],
            pressure: b[6..18].try_into().unwrap(),
        }
    }

    /// True for the all-zero block PCSX2 leaves in frame 0.
    pub fn is_blank(&self) -> bool {
        *self
            == Self {
                buttons: Buttons(0xFFFF),
                ..Self::default()
            }
    }
}

/// A raw stick axis mapped to -1.0..=1.0, taking 127.5 as the centre.
pub fn axis(raw: u8) -> f32 {
    (f32::from(raw) - 127.5) / 127.5
}

#[derive(Debug, Clone)]
pub struct InputRecording {
    pub file_version: u8,
    pub emulator: String,
    pub author: String,
    pub game: String,
    pub total_frames: u32,
    pub undo_count: u32,
    pub from_savestate: bool,
    /// `frames[n][port]`, one entry per declared frame.
    pub frames: Vec<[PadFrame; 2]>,
}

fn text(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

impl InputRecording {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RecordingError> {
        Self::parse(&std::fs::read(path)?)
    }

    pub fn parse(data: &[u8]) -> Result<Self, RecordingError> {
        if data.len() < HEADER_LEN {
            return Err(RecordingError::TooShort(data.len()));
        }
        if data[0] != 1 {
            return Err(RecordingError::Version(data[0]));
        }
        let total_frames = u32::from_le_bytes(data[561..565].try_into().unwrap());
        let undo_count = u32::from_le_bytes(data[565..569].try_into().unwrap());
        let present = (data.len() - HEADER_LEN) / FRAME_BYTES;
        if present < total_frames as usize {
            return Err(RecordingError::Truncated {
                declared: total_frames,
                present,
            });
        }
        let frames = data[HEADER_LEN..]
            .as_chunks::<FRAME_BYTES>()
            .0
            .iter()
            .take(total_frames as usize)
            .map(|f| {
                [
                    PadFrame::parse(&f[..PORT_BYTES]),
                    PadFrame::parse(&f[PORT_BYTES..]),
                ]
            })
            .collect();
        Ok(Self {
            file_version: data[0],
            emulator: text(&data[1..51]),
            author: text(&data[51..306]),
            game: text(&data[306..561]),
            total_frames,
            undo_count,
            from_savestate: data[569] != 0,
            frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(frames: u32) -> Vec<u8> {
        let mut d = vec![0u8; HEADER_LEN];
        d[0] = 1;
        d[1..13].copy_from_slice(b"PCSX2-v2.8.2");
        d[51..54].copy_from_slice(b"Max");
        d[306..315].copy_from_slice(b"Test Game");
        d[561..565].copy_from_slice(&frames.to_le_bytes());
        d[569] = 1;
        d
    }

    #[test]
    fn parses_header_and_pad_bytes() {
        let mut d = header(2);
        d.extend_from_slice(&[0; FRAME_BYTES]); // frame 0: the blank block
        // Frame 1, port 1: cross and up held, left stick pushed up, cross fully pressed.
        let mut port = [0u8; PORT_BYTES];
        port[0] = !0b0001_0000; // up
        port[1] = !0b0100_0000; // cross
        port[2..6].copy_from_slice(&[0x80, 0x7F, 0x80, 0x00]);
        port[12] = 0xFF;
        d.extend_from_slice(&port);
        d.extend_from_slice(&[
            0xFF, 0xFF, 0x7F, 0x7F, 0x7F, 0x7F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        d.extend_from_slice(&[0; FRAME_BYTES]); // the extra trailing block PCSX2 leaves

        let rec = InputRecording::parse(&d).unwrap();
        assert_eq!(rec.emulator, "PCSX2-v2.8.2");
        assert_eq!(rec.author, "Max");
        assert_eq!(rec.game, "Test Game");
        assert!(rec.from_savestate);
        assert_eq!(rec.frames.len(), 2);
        assert!(rec.frames[0][0].is_blank());

        let pad = rec.frames[1][0];
        assert!(pad.buttons.held(Buttons::UP) && pad.buttons.held(Buttons::CROSS));
        assert_eq!(pad.buttons.names(), "up cross");
        assert_eq!(pad.left_stick, [0x80, 0x00]);
        assert_eq!(pad.pressure[6], 0xFF);
        assert!(axis(pad.left_stick[1]) < -0.99);
        assert_eq!(rec.frames[1][1].buttons, Buttons(0));
    }

    #[test]
    fn rejects_truncated_or_foreign_files() {
        assert!(matches!(
            InputRecording::parse(&[1, 2, 3]),
            Err(RecordingError::TooShort(3))
        ));
        let mut d = header(3);
        d.extend_from_slice(&[0; FRAME_BYTES]);
        assert!(matches!(
            InputRecording::parse(&d),
            Err(RecordingError::Truncated {
                declared: 3,
                present: 1
            })
        ));
        d[0] = 2;
        assert!(matches!(
            InputRecording::parse(&d),
            Err(RecordingError::Version(2))
        ));
    }
}
