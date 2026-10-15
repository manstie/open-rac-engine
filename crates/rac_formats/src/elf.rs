//! The games' boot executables (PLAN.md P2.5): unpacking the packed ones of R&C3, UYA,
//! Deadlocked and Gladiator, and writing a program out as a plain ELF that Ghidra and other
//! tools load (`docs/research/disc-layout.md`, "Packed executables").
//!
//! Those games boot a small stub whose `.rodata` holds the real program as a WAD. Unpacked,
//! the program is a list of sections, each a 16-byte record `{address, size, type, entry}`
//! followed by its bytes. The stub copies each section to its address and jumps to the
//! entry. Level overlays use the same records (`docs/research/engine-notes.md`, "Code
//! layout and overlays").

use crate::{FormatError, Image, wad};

/// Where the stub's loader reads the packed program (the address it hands the
/// decompressor): 0x680 bytes into `.rodata`, file offset 0x7080 on every disc we have.
const PACKED_PROGRAM: u32 = 0x0080_6080;

/// `e_flags` of every boot ELF in the series: noreorder, R5900, eabi64, mips3.
const ELF_FLAGS: u32 = 0x2092_4001;

const PT_LOAD: u32 = 1;
const SHT_PROGBITS: u32 = 1;
const SHT_STRTAB: u32 = 3;
const SHT_NOBITS: u32 = 8;
const SHT_MIPS_REGINFO: u32 = 0x7000_0006;
const SHF_WRITE: u32 = 1;
const SHF_ALLOC: u32 = 2;
const SHF_EXECINSTR: u32 = 4;
/// MIPS: addressed relative to `$gp`.
const SHF_MIPS_GPREL: u32 = 0x1000_0000;

const CODE: u32 = SHF_ALLOC | SHF_EXECINSTR;
const DATA: u32 = SHF_ALLOC | SHF_WRITE;
const READ_ONLY: u32 = SHF_ALLOC;
const SMALL: u32 = DATA | SHF_MIPS_GPREL;

/// The engine's sections in link order, with the flags GC's plain boot ELF gives them. Every
/// packed program we have starts with exactly these; the three level tables hold one entry
/// each (12, 20 and 8 bytes), which pins the match.
const LINK_ORDER: [(&str, SectionKind, u32, Option<usize>); 14] = [
    (".reginfo", SectionKind::RegInfo, 0, None),
    (".vutext", SectionKind::Progbits, CODE, None),
    ("core.text", SectionKind::Progbits, CODE, None),
    ("core.data", SectionKind::Progbits, DATA, None),
    ("core.rdata", SectionKind::Progbits, READ_ONLY, None),
    ("core.bss", SectionKind::Nobits, SMALL, None),
    ("core.lit", SectionKind::Progbits, SMALL, None),
    (".lit", SectionKind::Progbits, SMALL, None),
    (".bss", SectionKind::Nobits, SMALL, None),
    (".data", SectionKind::Progbits, DATA, None),
    ("lvl.vtbl", SectionKind::Progbits, READ_ONLY, Some(12)),
    ("lvl.camvtbl", SectionKind::Progbits, READ_ONLY, Some(20)),
    ("lvl.sndvtbl", SectionKind::Progbits, READ_ONLY, Some(8)),
    (".text", SectionKind::Progbits, CODE, None),
];

/// A section's ELF type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    /// Code or data (`SHT_PROGBITS`).
    Progbits,
    /// Zero-filled data (`SHT_NOBITS`). The packed programs carry its bytes anyway, all
    /// zero, and the stub copies them like any other.
    Nobits,
    /// MIPS register usage (`SHT_MIPS_REGINFO`), whose last word is the program's `$gp`.
    RegInfo,
    /// Any other type.
    Other(u32),
}

impl SectionKind {
    fn from_elf(value: u32) -> Self {
        match value {
            SHT_PROGBITS => Self::Progbits,
            SHT_NOBITS => Self::Nobits,
            SHT_MIPS_REGINFO => Self::RegInfo,
            other => Self::Other(other),
        }
    }

    fn to_elf(self) -> u32 {
        match self {
            Self::Progbits => SHT_PROGBITS,
            Self::Nobits => SHT_NOBITS,
            Self::RegInfo => SHT_MIPS_REGINFO,
            Self::Other(value) => value,
        }
    }
}

/// One section of a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Its name in the engine's link order (`core.text`, `.text` …) when the program has
    /// the engine's layout, or else one made from its address.
    pub name: String,
    pub address: u32,
    pub kind: SectionKind,
    /// The bytes the loader copies to `address`.
    pub data: Vec<u8>,
}

impl Section {
    /// Whether the ELF can leave its bytes out.
    fn zero_filled(&self) -> bool {
        self.kind == SectionKind::Nobits && self.data.iter().all(|&byte| byte == 0)
    }

    /// Its ELF section flags: the link order's for the engine's sections. Others are
    /// unknown, so they may hold anything (Deadlocked's past `.text` holds code).
    fn flags(&self) -> u32 {
        match LINK_ORDER.iter().find(|(name, ..)| *name == self.name) {
            Some(&(_, _, flags, _)) => flags,
            None if self.kind == SectionKind::RegInfo => 0,
            None => DATA | SHF_EXECINSTR,
        }
    }
}

/// A program as its loader sees it: sections to copy, then where to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub entry: u32,
    /// In the order the loader copies them.
    pub sections: Vec<Section>,
}

impl Program {
    /// Reads a program's section records, stopping as the loader does at the end or at
    /// the first record whose entry differs from the first's.
    pub fn read(bytes: &[u8]) -> Result<Self, FormatError> {
        let image = Image::new(bytes, 0);
        let mut entry = None;
        let mut sections = Vec::new();
        let mut at = 0u32;
        while (at as usize) < bytes.len() {
            let [address, size, kind, record_entry] = [0, 4, 8, 12].map(|i| image.u32(at + i));
            let record_entry = record_entry?;
            if *entry.get_or_insert(record_entry) != record_entry {
                break;
            }
            let size = size?;
            let data = image.bytes(at + 16, size as usize)?.to_vec();
            sections.push(Section {
                name: String::new(),
                address: address?,
                kind: SectionKind::from_elf(kind?),
                data,
            });
            at = (at + 16)
                .checked_add(size)
                .ok_or(FormatError::OutOfBounds {
                    address: at,
                    len: size as usize,
                })?;
        }
        let entry = entry.ok_or_else(|| FormatError::Invalid {
            what: "program",
            address: 0,
            detail: "no sections".into(),
        })?;
        name_sections(&mut sections);
        Ok(Self { entry, sections })
    }

    /// The section called `name`.
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.name == name)
    }

    /// `$gp`, from the first register-info section.
    pub fn gp(&self) -> Option<u32> {
        let info = self
            .sections
            .iter()
            .find(|section| section.kind == SectionKind::RegInfo)?;
        let last = info.data.get(20..24)?;
        Some(u32::from_le_bytes([last[0], last[1], last[2], last[3]]))
    }

    /// The program as an ELF executable: a loadable segment per section, and section
    /// headers with the sections' names.
    pub fn to_elf(&self) -> Vec<u8> {
        // UYA's and Deadlocked's programs carry a second register-info section after
        // `.text`, all zero; tools take `$gp` from the first one they meet, so only the
        // first is written.
        let mut reginfo = false;
        let sections: Vec<&Section> = self
            .sections
            .iter()
            .filter(|section| {
                section.kind != SectionKind::RegInfo || !std::mem::replace(&mut reginfo, true)
            })
            .collect();
        let loaded = sections
            .iter()
            .filter(|section| section.flags() & SHF_ALLOC != 0)
            .count();

        // The headers go first, filled in once the file offsets are known.
        let headers = 52 + 32 * loaded;
        let mut out = vec![0; headers];
        let mut offsets = Vec::with_capacity(sections.len());
        for section in &sections {
            // Each section's file offset matches its address modulo 16, as loaders want.
            let pad = (section.address as usize).wrapping_sub(out.len()) % 16;
            offsets.push(out.len() + pad);
            if !section.zero_filled() {
                out.resize(out.len() + pad, 0);
                out.extend_from_slice(&section.data);
            }
        }

        let mut names = vec![0u8];
        let mut name_offsets = Vec::with_capacity(sections.len() + 1);
        for name in sections
            .iter()
            .map(|section| section.name.as_str())
            .chain([".shstrtab"])
        {
            name_offsets.push(names.len() as u32);
            names.extend_from_slice(name.as_bytes());
            names.push(0);
        }
        let names_offset = out.len();
        out.extend_from_slice(&names);
        out.resize(out.len().next_multiple_of(4), 0);

        let section_headers = out.len();
        out.extend([0; 40]);
        for ((section, &offset), &name) in sections.iter().zip(&offsets).zip(&name_offsets) {
            let kind = if section.kind == SectionKind::Nobits && !section.zero_filled() {
                SHT_PROGBITS
            } else {
                section.kind.to_elf()
            };
            let align = if section.kind == SectionKind::RegInfo {
                4
            } else {
                1 << section.address.trailing_zeros().min(6)
            };
            push_words(
                &mut out,
                &[
                    name,
                    kind,
                    section.flags(),
                    section.address,
                    offset as u32,
                    section.data.len() as u32,
                    0,
                    0,
                    align,
                    0,
                ],
            );
        }
        push_words(
            &mut out,
            &[
                name_offsets[sections.len()],
                SHT_STRTAB,
                0,
                0,
                names_offset as u32,
                names.len() as u32,
                0,
                0,
                1,
                0,
            ],
        );

        let mut head = Vec::with_capacity(headers);
        head.extend_from_slice(b"\x7fELF\x01\x01\x01");
        head.resize(16, 0);
        head.extend(2u16.to_le_bytes()); // An executable,
        head.extend(8u16.to_le_bytes()); // for MIPS.
        push_words(
            &mut head,
            &[1, self.entry, 52, section_headers as u32, ELF_FLAGS],
        );
        for half in [52, 32, loaded as u16, 40, sections.len() as u16 + 2] {
            head.extend(u16::to_le_bytes(half));
        }
        head.extend((sections.len() as u16 + 1).to_le_bytes());
        for (section, &offset) in sections.iter().zip(&offsets) {
            let flags = section.flags();
            if flags & SHF_ALLOC == 0 {
                continue;
            }
            let file_size = if section.zero_filled() {
                0
            } else {
                section.data.len() as u32
            };
            let readable = 4;
            let writable = if flags & SHF_WRITE != 0 { 2 } else { 0 };
            let executable = if flags & SHF_EXECINSTR != 0 { 1 } else { 0 };
            push_words(
                &mut head,
                &[
                    PT_LOAD,
                    offset as u32,
                    section.address,
                    section.address,
                    file_size,
                    section.data.len() as u32,
                    readable | writable | executable,
                    16,
                ],
            );
        }
        out[..headers].copy_from_slice(&head);
        out
    }
}

/// Names the sections after the engine's link order if they follow it, or else after
/// their addresses.
fn name_sections(sections: &mut [Section]) {
    let linked = sections.len() >= LINK_ORDER.len()
        && sections
            .iter()
            .zip(&LINK_ORDER)
            .all(|(section, (_, kind, _, size))| {
                section.kind == *kind && size.is_none_or(|size| section.data.len() == size)
            });
    for (i, section) in sections.iter_mut().enumerate() {
        section.name = match LINK_ORDER.get(i) {
            Some((name, ..)) if linked => (*name).to_owned(),
            _ => format!("sec_{:08x}", section.address),
        };
    }
}

fn push_words(out: &mut Vec<u8>, words: &[u32]) {
    for word in words {
        out.extend(word.to_le_bytes());
    }
}

/// The file offset of `address` in `elf`, through its loadable segments.
fn file_offset(elf: &[u8], address: u32) -> Result<usize, FormatError> {
    let image = Image::new(elf, 0);
    if image.bytes(0, 6)? != b"\x7fELF\x01\x01" {
        return Err(FormatError::Invalid {
            what: "ELF header",
            address: 0,
            detail: "not a 32-bit little-endian ELF".into(),
        });
    }
    let table = image.u32(0x1C)?;
    let entry_size = image.u16(0x2A)? as u32;
    for i in 0..image.u16(0x2C)? as u32 {
        let header = table
            .checked_add(i * entry_size)
            .filter(|header| header.checked_add(20).is_some())
            .ok_or(FormatError::OutOfBounds {
                address: table,
                len: 32,
            })?;
        let [kind, offset, start, size] = [0, 4, 8, 16].map(|at| image.u32(header + at));
        let (start, size) = (start?, size?);
        if kind? == PT_LOAD && address >= start && address - start < size {
            return Ok(offset? as usize + (address - start) as usize);
        }
    }
    Err(FormatError::Invalid {
        what: "ELF",
        address,
        detail: "no loadable segment holds this address".into(),
    })
}

/// Whether `elf` is one of the packed boot executables.
pub fn is_packed(elf: &[u8]) -> bool {
    file_offset(elf, PACKED_PROGRAM)
        .is_ok_and(|offset| elf.get(offset..).and_then(wad::wad_size).is_some())
}

/// The program a packed boot executable's stub loads.
pub fn unpack(elf: &[u8]) -> Result<Program, FormatError> {
    let offset = file_offset(elf, PACKED_PROGRAM)?;
    let packed = elf.get(offset..).unwrap_or_default();
    if wad::wad_size(packed).is_none() {
        return Err(FormatError::Invalid {
            what: "packed executable",
            address: PACKED_PROGRAM,
            detail: "no WAD where the stub reads its program".into(),
        });
    }
    Program::read(&wad::decompress(packed)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A section record and its bytes.
    fn record(address: u32, kind: u32, entry: u32, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        push_words(&mut out, &[address, data.len() as u32, kind, entry]);
        out.extend_from_slice(data);
        out
    }

    const ENTRY: u32 = 0x0013_ADA8;
    const GP: u32 = 0x001D_C8B0;

    /// A small program laid out like UYA's: the engine's link order, then an empty
    /// register-info section and one more section.
    fn engine_program() -> Vec<u8> {
        let mut reginfo = [0u8; 24];
        reginfo[20..].copy_from_slice(&GP.to_le_bytes());
        let mut bytes = record(0x0010_0000, SHT_MIPS_REGINFO, ENTRY, &reginfo);
        let sections: [(u32, u32, usize); 13] = [
            (0x0010_0080, SHT_PROGBITS, 40),
            (0x0010_0100, SHT_PROGBITS, 64),
            (0x0010_0180, SHT_PROGBITS, 24),
            (0x0010_0200, SHT_PROGBITS, 16),
            (0x0010_0280, SHT_NOBITS, 128),
            (0x0010_0300, SHT_PROGBITS, 8),
            (0x0010_0380, SHT_PROGBITS, 8),
            (0x0010_0400, SHT_NOBITS, 256),
            (0x0010_0500, SHT_PROGBITS, 32),
            (0x0010_0580, SHT_PROGBITS, 12),
            (0x0010_0600, SHT_PROGBITS, 20),
            (0x0010_0680, SHT_PROGBITS, 8),
            (0x0010_0700, SHT_PROGBITS, 100),
        ];
        for (i, (address, kind, size)) in sections.into_iter().enumerate() {
            let data: Vec<u8> = if kind == SHT_NOBITS {
                vec![0; size]
            } else {
                (0..size).map(|j| (i * 31 + j) as u8).collect()
            };
            bytes.extend(record(address, kind, ENTRY, &data));
        }
        bytes.extend(record(0x0010_0780, SHT_MIPS_REGINFO, ENTRY, &[0; 24]));
        bytes.extend(record(0x0180_0000, SHT_PROGBITS, ENTRY, b"legal"));
        bytes
    }

    /// A stub's ELF around `program`: one segment from 0x800000 at file offset 0x1000,
    /// with the packed program 0x6080 into it.
    fn packed_elf(program: &[u8]) -> Vec<u8> {
        let packed = wad::build::stored(program);
        let mut elf = b"\x7fELF\x01\x01\x01".to_vec();
        elf.resize(0x1C, 0);
        push_words(&mut elf, &[52]);
        elf.resize(0x2A, 0);
        elf.extend(32u16.to_le_bytes());
        elf.extend(1u16.to_le_bytes());
        elf.resize(52, 0);
        let size = 0x6080 + packed.len() as u32;
        push_words(
            &mut elf,
            &[PT_LOAD, 0x1000, 0x80_0000, 0x80_0000, size, size, 7, 0x1000],
        );
        elf.resize(0x7080, 0);
        elf.extend(packed);
        elf
    }

    #[test]
    fn unpacks_a_packed_executable_into_named_sections() {
        let elf = packed_elf(&engine_program());
        assert!(is_packed(&elf));
        let program = unpack(&elf).unwrap();
        assert_eq!(program.entry, ENTRY);
        assert_eq!(program.gp(), Some(GP));
        let names: Vec<&str> = program.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names[..14], LINK_ORDER.map(|(name, ..)| name));
        assert_eq!(names[14..], ["sec_00100780", "sec_01800000"]);
        let text = program.section(".text").unwrap();
        assert_eq!(text.address, 0x0010_0700);
        assert_eq!(text.data.len(), 100);
        assert_eq!(text.data[1], (12 * 31 + 1) as u8);
        assert_eq!(program.section(".bss").unwrap().kind, SectionKind::Nobits);
    }

    #[test]
    fn other_layouts_get_names_from_their_addresses() {
        // The level tables' sizes don't match: two entries in `lvl.vtbl`.
        let mut bytes = engine_program();
        let program = Program::read(&bytes).unwrap();
        let vtbl = program.sections[..10]
            .iter()
            .map(|s| 16 + s.data.len())
            .sum::<usize>();
        bytes[vtbl + 4] = 24;
        bytes.splice(vtbl + 16..vtbl + 16, [0; 12]);
        let program = Program::read(&bytes).unwrap();
        assert_eq!(program.sections[2].name, "sec_00100100");
        assert_eq!(program.section(".text"), None);
    }

    #[test]
    fn a_change_of_entry_ends_the_program() {
        let mut bytes = record(0x0010_0000, SHT_PROGBITS, ENTRY, b"code");
        bytes.extend(record(0x0010_0080, SHT_PROGBITS, ENTRY + 4, b"more"));
        let program = Program::read(&bytes).unwrap();
        assert_eq!(program.sections.len(), 1);
        assert_eq!(program.sections[0].data, b"code");
    }

    #[test]
    fn plain_elfs_are_not_packed() {
        let mut elf = packed_elf(&engine_program());
        elf.truncate(0x7080);
        elf.resize(0x8000, 0);
        assert!(!is_packed(&elf));
        assert!(unpack(&elf).is_err());
        assert!(!is_packed(b"not an ELF"));
    }

    #[test]
    fn bad_programs_are_errors_not_panics() {
        assert!(Program::read(&[]).is_err());
        // A header cut short.
        assert!(Program::read(&[0; 12]).is_err());
        // A size past the end.
        let mut bytes = record(0x0010_0000, SHT_PROGBITS, ENTRY, b"code");
        bytes[4] = 0xFF;
        assert!(Program::read(&bytes).is_err());
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Program::read(&bytes).is_err());
        // A segment table pointing outside the file.
        let mut elf = packed_elf(&engine_program());
        elf[0x1C] = 0xF0;
        elf[0x1F] = 0x7F;
        assert!(unpack(&elf).is_err());
    }

    /// Reads back what [`Program::to_elf`] wrote.
    struct Written<'a> {
        image: Image<'a>,
    }

    impl Written<'_> {
        fn u32(&self, at: u32) -> u32 {
            self.image.u32(at).unwrap()
        }

        fn u16(&self, at: u32) -> u16 {
            self.image.u16(at).unwrap()
        }

        /// Each section's (name, type, flags, address, offset, size).
        fn sections(&self) -> Vec<(String, u32, u32, u32, u32, u32)> {
            let table = self.u32(0x20);
            let count = self.u16(0x30) as u32;
            let names = table + 40 * self.u16(0x32) as u32;
            let names_at = self.u32(names + 16);
            (0..count)
                .map(|i| {
                    let header = table + 40 * i;
                    let name_at = names_at + self.u32(header);
                    let name: Vec<u8> = (name_at..)
                        .map(|at| self.image.u8(at).unwrap())
                        .take_while(|&byte| byte != 0)
                        .collect();
                    let word = |at| self.u32(header + at);
                    let name = String::from_utf8(name).unwrap();
                    (name, word(4), word(8), word(12), word(16), word(20))
                })
                .collect()
        }

        /// Each loadable segment's (offset, address, file size, memory size, flags).
        fn segments(&self) -> Vec<(u32, u32, u32, u32, u32)> {
            (0..self.u16(0x2C) as u32)
                .map(|i| {
                    let header = self.u32(0x1C) + 32 * i;
                    assert_eq!(self.u32(header), PT_LOAD);
                    let word = |at| self.u32(header + at);
                    (word(4), word(8), word(16), word(20), word(24))
                })
                .collect()
        }
    }

    #[test]
    fn writes_an_elf_with_a_segment_per_section() {
        let program = Program::read(&engine_program()).unwrap();
        let bytes = program.to_elf();
        let elf = Written {
            image: Image::new(&bytes, 0),
        };
        assert_eq!(&bytes[..7], b"\x7fELF\x01\x01\x01");
        assert_eq!((elf.u16(0x10), elf.u16(0x12)), (2, 8));
        assert_eq!(elf.u32(0x18), ENTRY);
        assert_eq!(elf.u32(0x24), ELF_FLAGS);

        let sections = elf.sections();
        // The null section, 13 engine sections and one more past the first `.reginfo`
        // (the empty second one is left out), and the names.
        assert_eq!(sections.len(), 1 + 14 + 1 + 1);
        assert_eq!(sections[0], (String::new(), 0, 0, 0, 0, 0));
        assert_eq!(sections[1].0, ".reginfo");
        assert_eq!(sections[1].1, SHT_MIPS_REGINFO);
        assert_eq!(elf.u32(sections[1].4 + 20), GP);
        let shstrtab = sections.last().unwrap();
        assert_eq!((shstrtab.0.as_str(), shstrtab.1), (".shstrtab", SHT_STRTAB));
        let written: Vec<_> = sections[1..16].iter().map(|s| s.0.as_str()).collect();
        let mut expected = LINK_ORDER.map(|(name, ..)| name).to_vec();
        expected.push("sec_01800000");
        assert_eq!(written, expected);

        // Every section but `.reginfo` loads, and its bytes are at its offset.
        let segments = elf.segments();
        assert_eq!(segments.len(), 14);
        let mut loaded = sections[2..16].iter().zip(&segments);
        for ((name, kind, flags, address, offset, size), segment) in loaded.by_ref() {
            let section = program.section(name).unwrap();
            assert_eq!(
                (*address, *size as usize),
                (section.address, section.data.len())
            );
            assert_eq!(segment.0 % 16, address % 16, "{name}");
            assert_eq!((segment.1, segment.3), (*address, *size));
            if *kind == SHT_NOBITS {
                assert_eq!(segment.2, 0, "{name}");
            } else {
                assert_eq!(*offset, segment.0);
                assert_eq!(segment.2, *size);
                assert_eq!(
                    elf.image.bytes(*offset, *size as usize).unwrap(),
                    section.data
                );
            }
            let executable = segment.4 & 1 != 0;
            assert_eq!(executable, flags & SHF_EXECINSTR != 0, "{name}");
        }
        let text = sections.iter().find(|s| s.0 == ".text").unwrap();
        assert_eq!(text.2, CODE);
        let lit = sections.iter().find(|s| s.0 == "core.lit").unwrap();
        assert_eq!(lit.2, SMALL);
    }

    #[test]
    fn nobits_that_are_not_zero_keep_their_bytes() {
        let mut bytes = record(0x0010_0000, SHT_NOBITS, ENTRY, &[0, 0, 7, 0]);
        bytes.extend(record(0x0010_0080, SHT_NOBITS, ENTRY, &[0; 8]));
        let program = Program::read(&bytes).unwrap();
        let elf = program.to_elf();
        let written = Written {
            image: Image::new(&elf, 0),
        };
        let sections = written.sections();
        assert_eq!(sections[1].1, SHT_PROGBITS);
        assert_eq!(written.image.bytes(sections[1].4, 4).unwrap(), [0, 0, 7, 0]);
        assert_eq!(sections[2].1, SHT_NOBITS);
        assert_eq!(written.segments()[1].2, 0);
    }
}
