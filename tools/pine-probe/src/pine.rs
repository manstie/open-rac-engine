//! A small client for PINE, the IPC protocol PCSX2 exposes for reading emulated memory.
//!
//! Written from the protocol as PCSX2 2.8 implements it (`pcsx2/PINE.cpp`, GPL, read for
//! facts only):
//! - A request is a little-endian `u32` total length (including itself) followed by
//!   commands back to back. A command is an opcode byte plus its arguments.
//! - A reply is a `u32` total length, a status byte (0 = OK, 0xFF = failed) and the
//!   results back to back, in command order.
//! - On Unix the server is a socket at `$XDG_RUNTIME_DIR/pcsx2.sock` (`$TMPDIR` on macOS,
//!   `/tmp` as a fallback), with `.<slot>` appended for any slot but the default. On
//!   Windows it is TCP on `127.0.0.1:<slot>`.
//!
//! PCSX2 answers on its own thread while the game keeps running, so a batch is not atomic
//! with respect to the emulated CPU. [`crate::capture`] deals with that.

use std::io::{self, Read, Write};
use std::ops::Range;
#[cfg(unix)]
use std::path::PathBuf;

use thiserror::Error;

/// PCSX2's default slot. On Unix it picks the socket name, on Windows the TCP port.
pub const DEFAULT_SLOT: u16 = 28011;

/// PCSX2 drops requests longer than this (`MAX_IPC_SIZE`).
const MAX_REQUEST: usize = 650_000;
/// PCSX2 cannot build replies longer than this (`MAX_IPC_RETURN_SIZE`).
const MAX_REPLY: usize = 450_000;
/// Bytes fetched per round trip by [`Pine::read_bytes`]: 40,000 64-bit reads, well inside
/// both limits.
const READ_CHUNK: u32 = 40_000 * 8;

mod opcode {
    pub const READ8: u8 = 0x0;
    pub const READ32: u8 = 0x2;
    pub const READ64: u8 = 0x3;
    pub const WRITE8: u8 = 0x4;
    pub const WRITE32: u8 = 0x6;
    pub const VERSION: u8 = 0x8;
    pub const SAVE_STATE: u8 = 0x9;
    pub const LOAD_STATE: u8 = 0xA;
    pub const TITLE: u8 = 0xB;
    pub const ID: u8 = 0xC;
    pub const STATUS: u8 = 0xF;
}

#[derive(Debug, Error)]
pub enum PineError {
    #[error("PINE connection failed: {0}")]
    Io(#[from] io::Error),
    #[error("PCSX2 rejected the request (is a game running?)")]
    Rejected,
    #[error("malformed PINE reply: {0}")]
    Protocol(&'static str),
    #[error("PINE batch too large ({request} request bytes, {reply} reply bytes)")]
    TooLarge { request: usize, reply: usize },
}

/// What the emulator is doing, as reported by PINE's status command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmuStatus {
    Running,
    Paused,
    Shutdown,
    Other(u32),
}

/// A list of reads sent to PCSX2 in a single round trip.
///
/// Each `read_*` method returns the byte range its result will occupy in the reply that
/// [`Pine::execute`] returns.
#[derive(Debug, Default, Clone)]
pub struct Batch {
    request: Vec<u8>,
    reply_len: usize,
}

impl Batch {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, op: u8, addr: u32, size: usize) -> Range<usize> {
        self.request.push(op);
        self.request.extend_from_slice(&addr.to_le_bytes());
        let start = self.reply_len;
        self.reply_len += size;
        start..self.reply_len
    }

    pub fn read_u32(&mut self, addr: u32) -> Range<usize> {
        self.push(opcode::READ32, addr, 4)
    }

    /// Reads `len` bytes from `addr`: single bytes up to an 8-byte boundary, then 64-bit
    /// reads, then single bytes for the tail.
    pub fn read_bytes(&mut self, addr: u32, len: u32) -> Range<usize> {
        let start = self.reply_len;
        let end = addr.saturating_add(len);
        let mut a = addr;
        while a < end && !a.is_multiple_of(8) {
            self.push(opcode::READ8, a, 1);
            a += 1;
        }
        while end - a >= 8 {
            self.push(opcode::READ64, a, 8);
            a += 8;
        }
        while a < end {
            self.push(opcode::READ8, a, 1);
            a += 1;
        }
        start..self.reply_len
    }

    /// Length of the encoded request, including the length prefix.
    pub fn request_len(&self) -> usize {
        4 + self.request.len()
    }

    /// Length of the reply payload (after the status byte).
    pub fn reply_len(&self) -> usize {
        self.reply_len
    }
}

trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

/// A connection to a running PCSX2.
pub struct Pine {
    stream: Box<dyn Stream>,
}

impl Pine {
    /// Connects to the PCSX2 instance listening on `slot`.
    pub fn connect(slot: u16) -> Result<Self, PineError> {
        #[cfg(unix)]
        {
            let stream = std::os::unix::net::UnixStream::connect(socket_path(slot))?;
            Ok(Self::from_stream(stream))
        }
        #[cfg(not(unix))]
        {
            let stream = std::net::TcpStream::connect(("127.0.0.1", slot))?;
            stream.set_nodelay(true)?;
            Ok(Self::from_stream(stream))
        }
    }

    /// Wraps an already connected stream. Tests use this with an in-memory server.
    pub fn from_stream<S: Read + Write + Send + 'static>(stream: S) -> Self {
        Self {
            stream: Box::new(stream),
        }
    }

    /// Sends one request and returns the reply payload, after checking the status byte.
    fn call(&mut self, request: &[u8]) -> Result<Vec<u8>, PineError> {
        let total = 4 + request.len();
        let mut message = Vec::with_capacity(total);
        message.extend_from_slice(&(total as u32).to_le_bytes());
        message.extend_from_slice(request);
        self.stream.write_all(&message)?;

        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        let reply_total = u32::from_le_bytes(header) as usize;
        if !(5..=MAX_REPLY).contains(&reply_total) {
            return Err(PineError::Protocol("reply length out of range"));
        }
        let mut reply = vec![0u8; reply_total - 4];
        self.stream.read_exact(&mut reply)?;
        if reply[0] != 0 {
            return Err(PineError::Rejected);
        }
        reply.remove(0);
        Ok(reply)
    }

    /// Runs a batch of reads and returns the concatenated results.
    pub fn execute(&mut self, batch: &Batch) -> Result<Vec<u8>, PineError> {
        if batch.request_len() > MAX_REQUEST || 5 + batch.reply_len > MAX_REPLY {
            return Err(PineError::TooLarge {
                request: batch.request_len(),
                reply: 5 + batch.reply_len,
            });
        }
        let reply = self.call(&batch.request)?;
        if reply.len() != batch.reply_len {
            return Err(PineError::Protocol("reply size does not match the batch"));
        }
        Ok(reply)
    }

    /// Reads a block of memory of any size, splitting it into several round trips.
    pub fn read_bytes(&mut self, addr: u32, len: u32) -> Result<Vec<u8>, PineError> {
        let mut out = Vec::with_capacity(len as usize);
        let mut a = addr;
        let end = addr.saturating_add(len);
        while a < end {
            let n = (end - a).min(READ_CHUNK);
            let mut batch = Batch::new();
            batch.read_bytes(a, n);
            out.extend_from_slice(&self.execute(&batch)?);
            a += n;
        }
        Ok(out)
    }

    pub fn read_u32(&mut self, addr: u32) -> Result<u32, PineError> {
        let mut batch = Batch::new();
        batch.read_u32(addr);
        let reply = self.execute(&batch)?;
        Ok(u32::from_le_bytes(reply[..4].try_into().unwrap()))
    }

    /// Writes one byte of EE memory. Only for the private research instances: it changes
    /// the running game, for example to give the hero an item a level hasn't granted yet.
    pub fn write_u8(&mut self, addr: u32, value: u8) -> Result<(), PineError> {
        let mut request = vec![opcode::WRITE8];
        request.extend_from_slice(&addr.to_le_bytes());
        request.push(value);
        self.call(&request).map(drop)
    }

    /// Writes a little-endian 32-bit word of EE memory, for the same research use as
    /// [`Pine::write_u8`].
    pub fn write_u32(&mut self, addr: u32, value: u32) -> Result<(), PineError> {
        self.write_words(addr, &[value])
    }

    /// Writes consecutive little-endian words from `addr` in one request. PCSX2 applies a
    /// request's commands back to back, so the words almost always land in the same frame;
    /// one request per word can let the game run in between, for example snapping a hero
    /// moved along one axis back before the next axis arrives.
    pub fn write_words(&mut self, addr: u32, values: &[u32]) -> Result<(), PineError> {
        let mut request = Vec::with_capacity(values.len() * 9);
        for (word, value) in (addr..).step_by(4).zip(values) {
            request.push(opcode::WRITE32);
            request.extend_from_slice(&word.to_le_bytes());
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.call(&request).map(drop)
    }

    /// Commands that answer with a length-prefixed, NUL-terminated string.
    fn string_command(&mut self, op: u8) -> Result<String, PineError> {
        let reply = self.call(&[op])?;
        if reply.len() < 4 {
            return Err(PineError::Protocol("string reply too short"));
        }
        let len = u32::from_le_bytes(reply[..4].try_into().unwrap()) as usize;
        let bytes = reply
            .get(4..4 + len)
            .ok_or(PineError::Protocol("string reply truncated"))?;
        let text = bytes.split(|&b| b == 0).next().unwrap_or_default();
        Ok(String::from_utf8_lossy(text).into_owned())
    }

    /// PCSX2's version string, for example `PCSX2 v2.8.2`.
    pub fn version(&mut self) -> Result<String, PineError> {
        self.string_command(opcode::VERSION)
    }

    /// The running game's title.
    pub fn title(&mut self) -> Result<String, PineError> {
        self.string_command(opcode::TITLE)
    }

    /// The running disc's serial, for example `SCES-52456`.
    pub fn game_id(&mut self) -> Result<String, PineError> {
        self.string_command(opcode::ID)
    }

    pub fn status(&mut self) -> Result<EmuStatus, PineError> {
        let reply = self.call(&[opcode::STATUS])?;
        let raw = u32::from_le_bytes(
            reply
                .get(..4)
                .ok_or(PineError::Protocol("status reply too short"))?
                .try_into()
                .unwrap(),
        );
        Ok(match raw {
            0 => EmuStatus::Running,
            1 => EmuStatus::Paused,
            2 => EmuStatus::Shutdown,
            other => EmuStatus::Other(other),
        })
    }

    /// Asks PCSX2 to load the savestate in `slot`. PCSX2 queues it, so it completes shortly
    /// after this returns.
    pub fn load_state(&mut self, slot: u8) -> Result<(), PineError> {
        self.call(&[opcode::LOAD_STATE, slot]).map(drop)
    }

    /// Asks PCSX2 to save a savestate to `slot`, asynchronously like [`Self::load_state`].
    pub fn save_state(&mut self, slot: u8) -> Result<(), PineError> {
        self.call(&[opcode::SAVE_STATE, slot]).map(drop)
    }
}

/// Where PCSX2 puts the PINE socket for `slot`.
#[cfg(unix)]
pub fn socket_path(slot: u16) -> PathBuf {
    let var = if cfg!(target_os = "macos") {
        "TMPDIR"
    } else {
        "XDG_RUNTIME_DIR"
    };
    let dir = std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    if slot == DEFAULT_SLOT {
        dir.join("pcsx2.sock")
    } else {
        dir.join(format!("pcsx2.sock.{slot}"))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// An in-memory PINE server backed by a byte array standing in for EE memory.
    #[derive(Clone)]
    pub struct FakePcsx2 {
        pub memory: Arc<Mutex<Vec<u8>>>,
        pending: Arc<Mutex<Vec<u8>>>,
        replies: Arc<Mutex<VecDeque<u8>>>,
    }

    impl FakePcsx2 {
        pub fn new(memory: Vec<u8>) -> Self {
            Self {
                memory: Arc::new(Mutex::new(memory)),
                pending: Arc::default(),
                replies: Arc::default(),
            }
        }

        fn answer(&self, request: &[u8]) -> Vec<u8> {
            let mut memory = self.memory.lock().unwrap();
            let mut out = vec![0u8];
            let mut i = 0;
            let mut ok = true;
            while i < request.len() {
                let op = request[i];
                i += 1;
                let mut addr = || {
                    let a = u32::from_le_bytes(request[i..i + 4].try_into().unwrap()) as usize;
                    i += 4;
                    a
                };
                match op {
                    // Read8, Read16, Read32 and Read64 are opcodes 0 to 3.
                    0..=3 => {
                        let a = addr();
                        let n = 1 << op;
                        match memory.get(a..a + n) {
                            Some(bytes) => out.extend_from_slice(bytes),
                            None => ok = false,
                        }
                    }
                    opcode::WRITE8 => {
                        let a = addr();
                        let value = request[i];
                        i += 1;
                        match memory.get_mut(a) {
                            Some(byte) => *byte = value,
                            None => ok = false,
                        }
                    }
                    opcode::WRITE32 => {
                        let a = addr();
                        let value: [u8; 4] = request[i..i + 4].try_into().unwrap();
                        i += 4;
                        match memory.get_mut(a..a + 4) {
                            Some(word) => word.copy_from_slice(&value),
                            None => ok = false,
                        }
                    }
                    opcode::ID => {
                        let id = b"SCES-00000\0";
                        out.extend_from_slice(&(id.len() as u32).to_le_bytes());
                        out.extend_from_slice(id);
                    }
                    opcode::STATUS => out.extend_from_slice(&1u32.to_le_bytes()),
                    _ => ok = false,
                }
            }
            if !ok {
                out = vec![0xFF];
            }
            let mut reply = ((out.len() + 4) as u32).to_le_bytes().to_vec();
            reply.extend_from_slice(&out);
            reply
        }
    }

    impl Write for FakePcsx2 {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut pending = self.pending.lock().unwrap();
            pending.extend_from_slice(buf);
            while pending.len() >= 4 {
                let total = u32::from_le_bytes(pending[..4].try_into().unwrap()) as usize;
                if pending.len() < total {
                    break;
                }
                let request: Vec<u8> = pending.drain(..total).skip(4).collect();
                let reply = self.answer(&request);
                self.replies.lock().unwrap().extend(reply);
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Read for FakePcsx2 {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut replies = self.replies.lock().unwrap();
            let n = buf.len().min(replies.len());
            for (dst, src) in buf.iter_mut().zip(replies.drain(..n)) {
                *dst = src;
            }
            Ok(n)
        }
    }

    fn memory() -> Vec<u8> {
        (0..4096u32).map(|i| (i * 7 + 3) as u8).collect()
    }

    #[test]
    fn reads_unaligned_blocks_and_words() {
        let mem = memory();
        let mut pine = Pine::from_stream(FakePcsx2::new(mem.clone()));
        assert_eq!(pine.read_bytes(13, 61).unwrap(), &mem[13..74]);
        assert_eq!(
            pine.read_u32(0x40).unwrap(),
            u32::from_le_bytes(mem[0x40..0x44].try_into().unwrap())
        );
    }

    #[test]
    fn writes_a_byte() {
        let mut pine = Pine::from_stream(FakePcsx2::new(memory()));
        pine.write_u8(0x21, 0xAB).unwrap();
        assert_eq!(pine.read_bytes(0x20, 3).unwrap()[1], 0xAB);
        assert!(pine.write_u8(0x10_0000, 1).is_err());
    }

    #[test]
    fn writes_a_word() {
        let mut pine = Pine::from_stream(FakePcsx2::new(memory()));
        pine.write_u32(0x40, 0x0123_4567).unwrap();
        assert_eq!(pine.read_u32(0x40).unwrap(), 0x0123_4567);
        assert!(pine.write_u32(0x10_0000, 1).is_err());
    }

    #[test]
    fn writes_consecutive_words_in_one_request() {
        let mut pine = Pine::from_stream(FakePcsx2::new(memory()));
        pine.write_words(0x40, &[1, 2, 0x4410_0000]).unwrap();
        let bytes = pine.read_bytes(0x40, 12).unwrap();
        let words: Vec<u32> = bytes
            .chunks(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        assert_eq!(words, [1, 2, 0x4410_0000]);
    }

    #[test]
    fn batch_ranges_index_the_reply() {
        let mem = memory();
        let mut pine = Pine::from_stream(FakePcsx2::new(mem.clone()));
        let mut batch = Batch::new();
        let a = batch.read_u32(0x100);
        let b = batch.read_bytes(0x200, 0x30);
        let c = batch.read_u32(0x100);
        let reply = pine.execute(&batch).unwrap();
        assert_eq!(&reply[a.clone()], &mem[0x100..0x104]);
        assert_eq!(&reply[b], &mem[0x200..0x230]);
        assert_eq!(reply[a], reply[c]);
    }

    #[test]
    fn reports_rejection_and_strings() {
        let mut pine = Pine::from_stream(FakePcsx2::new(memory()));
        assert!(matches!(
            pine.read_bytes(4090, 16),
            Err(PineError::Rejected)
        ));
        assert_eq!(pine.game_id().unwrap(), "SCES-00000");
        assert_eq!(pine.status().unwrap(), EmuStatus::Paused);
    }

    #[test]
    fn refuses_oversized_batches() {
        let mut pine = Pine::from_stream(FakePcsx2::new(memory()));
        let mut batch = Batch::new();
        batch.read_bytes(0, 60_000 * 8);
        assert!(matches!(
            pine.execute(&batch),
            Err(PineError::TooLarge { .. })
        ));
    }
}
