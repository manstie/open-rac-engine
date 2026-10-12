//! Reads PCSX2 savestates (`.p2s`).
//!
//! A savestate is a zip archive. PCSX2 2.x stores most members with Zstandard (zip method
//! 93) by default and the screenshot uncompressed. Members we use:
//! - `eeMemory.bin`: the 32 MiB EE main memory, which holds the unpacked game and the
//!   current level's code overlay.
//! - `PCSX2 Internal Structures.dat`: emulator state, including the EE registers (Ghidra
//!   needs `$gp`) and the memory-card checksums that [`crate::memcard`] needs.
//!
//! Only the parts of the zip format that PCSX2 writes are supported: no zip64, no
//! encryption, and only the stored and Zstandard methods.

use std::io::Read;
use std::path::Path;

use thiserror::Error;

/// Size of the EE main memory image.
pub const EE_MEMORY_SIZE: usize = 32 * 1024 * 1024;

/// The member holding PCSX2's emulator state.
pub const INTERNALS: &str = "PCSX2 Internal Structures.dat";

/// MIPS names of the 32 general-purpose registers, in register order.
pub const GPR_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
    "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp",
    "ra",
];

const EOCD_SIG: u32 = 0x0605_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const METHOD_STORED: u16 = 0;
const METHOD_ZSTD: u16 = 93;

#[derive(Debug, Error)]
pub enum SavestateError {
    #[error("cannot read savestate: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a PCSX2 savestate (bad zip structure: {0})")]
    Malformed(&'static str),
    #[error("savestate has no member named {0:?}")]
    MissingMember(String),
    #[error(
        "member {name:?} uses zip method {method}, which is not supported (only stored and Zstandard)"
    )]
    UnsupportedMethod { name: String, method: u16 },
    #[error("member {name:?} failed to decompress: {reason}")]
    Corrupt { name: String, reason: String },
}

/// One member of the savestate archive.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub method: u16,
    pub compressed_size: u32,
    pub size: u32,
    local_header: u32,
}

/// A savestate held in memory.
pub struct Savestate {
    data: Vec<u8>,
    entries: Vec<Entry>,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

impl Savestate {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SavestateError> {
        Self::from_bytes(std::fs::read(path)?)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self, SavestateError> {
        use SavestateError::Malformed;
        // The end-of-central-directory record sits in the last 22 bytes plus any comment.
        let min = data.len().saturating_sub(22 + 0xFFFF);
        let eocd = (min..=data.len().saturating_sub(22))
            .rev()
            .find(|&i| u32_at(&data, i) == Some(EOCD_SIG))
            .ok_or(Malformed("no end-of-central-directory record"))?;
        let count = u16_at(&data, eocd + 10).ok_or(Malformed("truncated EOCD"))?;
        let cd_offset = u32_at(&data, eocd + 16).ok_or(Malformed("truncated EOCD"))?;
        if cd_offset == u32::MAX {
            return Err(Malformed("zip64 archives are not supported"));
        }

        let mut entries = Vec::with_capacity(count as usize);
        let mut at = cd_offset as usize;
        for _ in 0..count {
            if u32_at(&data, at) != Some(CENTRAL_SIG) {
                return Err(Malformed("bad central directory entry"));
            }
            let field = |off| u16_at(&data, at + off).ok_or(Malformed("truncated entry"));
            let word = |off| u32_at(&data, at + off).ok_or(Malformed("truncated entry"));
            let method = field(10)?;
            let compressed_size = word(20)?;
            let size = word(24)?;
            let name_len = field(28)? as usize;
            let extra_len = field(30)? as usize;
            let comment_len = field(32)? as usize;
            let local_header = word(42)?;
            let name = data
                .get(at + 46..at + 46 + name_len)
                .ok_or(Malformed("truncated entry name"))?;
            entries.push(Entry {
                name: String::from_utf8_lossy(name).into_owned(),
                method,
                compressed_size,
                size,
                local_header,
            });
            at += 46 + name_len + extra_len + comment_len;
        }
        Ok(Self { data, entries })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Decompresses one member.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, SavestateError> {
        use SavestateError::Malformed;
        let entry = self
            .entries
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| SavestateError::MissingMember(name.to_owned()))?;
        let at = entry.local_header as usize;
        if u32_at(&self.data, at) != Some(LOCAL_SIG) {
            return Err(Malformed("bad local header"));
        }
        let name_len = u16_at(&self.data, at + 26).ok_or(Malformed("truncated header"))?;
        let extra_len = u16_at(&self.data, at + 28).ok_or(Malformed("truncated header"))?;
        let start = at + 30 + name_len as usize + extra_len as usize;
        let raw = self
            .data
            .get(start..start + entry.compressed_size as usize)
            .ok_or(Malformed("member data runs past the end of the file"))?;

        let corrupt = |reason: String| SavestateError::Corrupt {
            name: name.to_owned(),
            reason,
        };
        let out = match entry.method {
            METHOD_STORED => raw.to_vec(),
            METHOD_ZSTD => {
                let mut decoder = ruzstd::decoding::StreamingDecoder::new(raw)
                    .map_err(|e| corrupt(e.to_string()))?;
                let mut out = Vec::with_capacity(entry.size as usize);
                decoder
                    .read_to_end(&mut out)
                    .map_err(|e| corrupt(e.to_string()))?;
                out
            }
            method => {
                return Err(SavestateError::UnsupportedMethod {
                    name: name.to_owned(),
                    method,
                });
            }
        };
        if out.len() != entry.size as usize {
            return Err(corrupt(format!(
                "expected {} bytes, got {}",
                entry.size,
                out.len()
            )));
        }
        Ok(out)
    }

    /// The 32 MiB EE main memory image.
    pub fn ee_memory(&self) -> Result<Vec<u8>, SavestateError> {
        let mem = self.read("eeMemory.bin")?;
        if mem.len() != EE_MEMORY_SIZE {
            return Err(SavestateError::Corrupt {
                name: "eeMemory.bin".into(),
                reason: format!("expected {EE_MEMORY_SIZE} bytes, got {}", mem.len()),
            });
        }
        Ok(mem)
    }

    /// The low 32 bits of the EE's general-purpose registers when the state was saved.
    pub fn ee_registers(&self) -> Result<[u32; 32], SavestateError> {
        ee_registers(&self.read(INTERNALS)?).ok_or_else(|| SavestateError::Corrupt {
            name: INTERNALS.into(),
            reason: "no cpuRegs block".into(),
        })
    }
}

/// Finds the EE registers in `PCSX2 Internal Structures.dat`. PCSX2 writes a 32-byte,
/// NUL-padded tag before each block, and the `cpuRegs` block starts with the 32 GPRs, 16
/// bytes each. Checked against real savestates: `$sp` lands near the top of RAM, and `$gp`
/// is the same in every state of a game and is the base its gp-relative code expects.
pub fn ee_registers(internals: &[u8]) -> Option<[u32; 32]> {
    let mut tag = [0u8; 32];
    tag[..7].copy_from_slice(b"cpuRegs");
    let start = internals.windows(tag.len()).position(|w| w == tag)? + tag.len();
    let mut gpr = [0u32; 32];
    for (i, reg) in gpr.iter_mut().enumerate() {
        *reg = u32_at(internals, start + 16 * i)?;
    }
    Some(gpr)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a minimal zip the way PCSX2 lays out a savestate.
    pub fn build_zip(members: &[(&str, u16, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for &(name, method, content) in members {
            let stored = match method {
                METHOD_ZSTD => ruzstd::encoding::compress_to_vec(
                    content,
                    ruzstd::encoding::CompressionLevel::Fastest,
                ),
                _ => content.to_vec(),
            };
            let offset = out.len() as u32;
            out.extend_from_slice(&LOCAL_SIG.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 8]); // time, date, crc
            out.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            out.extend_from_slice(&(content.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&stored);

            central.extend_from_slice(&CENTRAL_SIG.to_le_bytes());
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            central.extend_from_slice(&(content.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0; 12]); // extra, comment, disk, attributes
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&EOCD_SIG.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&[0; 2]);
        out
    }

    #[test]
    fn reads_stored_and_zstd_members() {
        let big: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let zip = build_zip(&[
            ("PCSX2 Savestate Version.id", METHOD_STORED, b"v2.8.2"),
            ("eeMemory.bin", METHOD_ZSTD, &big),
        ]);
        let state = Savestate::from_bytes(zip).unwrap();
        assert_eq!(state.entries().len(), 2);
        assert_eq!(state.read("PCSX2 Savestate Version.id").unwrap(), b"v2.8.2");
        assert_eq!(state.read("eeMemory.bin").unwrap(), big);
        // A real EE image is 32 MiB, so this one is rejected as the wrong size.
        assert!(matches!(
            state.ee_memory(),
            Err(SavestateError::Corrupt { .. })
        ));
    }

    #[test]
    fn reads_ee_registers_after_the_cpu_tag() {
        let mut internals = vec![0xAAu8; 40];
        let mut tag = [0u8; 32];
        tag[..7].copy_from_slice(b"cpuRegs");
        internals.extend_from_slice(&tag);
        for r in 0..32u32 {
            let mut reg = [0u8; 16];
            reg[..4].copy_from_slice(&(0x1000 + r).to_le_bytes());
            internals.extend_from_slice(&reg);
        }
        let zip = build_zip(&[(INTERNALS, METHOD_ZSTD, &internals)]);
        let regs = Savestate::from_bytes(zip).unwrap().ee_registers().unwrap();
        assert_eq!(regs[0], 0x1000);
        assert_eq!(regs[28], 0x1000 + 28, "gp");
        assert_eq!(GPR_NAMES[28], "gp");

        assert_eq!(ee_registers(b"no tag here"), None);
        // Cut into `$ra`'s low word, the last bytes read.
        assert_eq!(ee_registers(&internals[..internals.len() - 13]), None);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            Savestate::from_bytes(b"not a zip".to_vec()),
            Err(SavestateError::Malformed(_))
        ));
        let zip = build_zip(&[("x", 8, b"deflated?")]);
        let state = Savestate::from_bytes(zip).unwrap();
        assert!(matches!(
            state.read("x"),
            Err(SavestateError::UnsupportedMethod { method: 8, .. })
        ));
        assert!(matches!(
            state.read("y"),
            Err(SavestateError::MissingMember(_))
        ));
    }
}
