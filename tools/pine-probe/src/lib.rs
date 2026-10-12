//! Reverse-engineering probe for PLAN.md tasks RE.3, RE.5 and RE.6.
//!
//! - [`pine`]: a client for PCSX2's PINE IPC, for reading live emulated memory.
//! - [`savestate`]: reads PCSX2 savestates (`.p2s`), including the EE memory image.
//! - [`recording`]: reads PCSX2 input recordings (`.p2m2`).
//! - [`layout`]: per-release memory layouts from `re/<serial>/hero.toml`.
//! - [`symbols`]: named code and data addresses from `re/<serial>/symbols.toml`.
//! - [`capture`] and [`trace`]: per-frame capture of memory regions into a trace file.
//! - [`memcard`] and [`pcsx2_config`]: helpers for the headless replay harness
//!   (`tools/pine-probe/replay.sh`).
//!
//! The tool contains no game data. Savestates, traces and dumps it reads or writes are
//! derived from the owner's discs, so they stay in `/cache` and are never committed.

pub mod capture;
pub mod layout;
pub mod memcard;
pub mod pcsx2_config;
pub mod pine;
pub mod recording;
pub mod savestate;
pub mod symbols;
pub mod trace;
