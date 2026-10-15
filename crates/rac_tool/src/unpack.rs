//! `unpack-elf`: a packed executable from a disc image, unpacked into a plain ELF for
//! Ghidra and other tools (PLAN.md P2.5).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rac_formats::elf::{self, Program, SectionKind};
use rac_iso::Disc;

/// The executable a `SYSTEM.CNF` boots, from its `BOOT2` line (`cdrom0:\SCUS_973.53;1`).
fn boot_name(cnf: &str) -> Option<&str> {
    let line = cnf
        .lines()
        .find(|line| line.trim_start().starts_with("BOOT2"))?;
    let path = line.split_once('=')?.1.trim();
    let name = path.rsplit(['\\', ':']).next()?;
    name.split(';').next()
}

/// `file`, or else the executable the disc boots, read from the disc image at `iso`.
fn read_executable(iso: &Path, file: Option<&str>) -> Result<(String, Vec<u8>)> {
    let image = File::open(iso).with_context(|| format!("opening {}", iso.display()))?;
    let mut disc = Disc::open(BufReader::new(image))?;
    let name = match file {
        Some(name) => name.to_owned(),
        None => {
            let cnf = disc
                .find("SYSTEM.CNF")?
                .context("the disc has no SYSTEM.CNF")?;
            let text = String::from_utf8_lossy(&disc.read_file(&cnf)?).into_owned();
            boot_name(&text)
                .context("SYSTEM.CNF names no boot executable")?
                .to_owned()
        }
    };
    let entry = disc
        .find(&name)?
        .with_context(|| format!("the disc has no {name}"))?;
    Ok((name, disc.read_file(&entry)?))
}

pub fn run(iso: &Path, file: Option<&str>, out: Option<&Path>) -> Result<()> {
    let (name, bytes) = read_executable(iso, file)?;
    if !elf::is_packed(&bytes) {
        bail!("{name} isn't packed: tools can read it as it is");
    }
    let program = elf::unpack(&bytes).with_context(|| format!("unpacking {name}"))?;
    describe(&name, &program);
    if let Some(path) = out {
        let written = program.to_elf();
        std::fs::write(path, &written).with_context(|| format!("writing {}", path.display()))?;
        println!("wrote {} ({} bytes)", path.display(), written.len());
    }
    Ok(())
}

fn describe(name: &str, program: &Program) {
    let gp = program
        .gp()
        .map_or_else(|| "unknown".to_owned(), |gp| format!("{gp:#x}"));
    println!("{name}: entry {:#x}, $gp {gp}", program.entry);
    for section in &program.sections {
        let kind = match section.kind {
            SectionKind::Progbits => "progbits".to_owned(),
            SectionKind::Nobits => "nobits".to_owned(),
            SectionKind::RegInfo => "reginfo".to_owned(),
            SectionKind::Other(kind) => format!("type {kind:#x}"),
        };
        println!(
            "  {:<14} {:#010x} {:#9x}  {kind}",
            section.name,
            section.address,
            section.data.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_boot_executable_from_system_cnf() {
        let cnf = "BOOT2 = cdrom0:\\SCUS_973.53;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n";
        assert_eq!(boot_name(cnf), Some("SCUS_973.53"));
        assert_eq!(boot_name("BOOT2=cdrom0:\\SLES_123.45"), Some("SLES_123.45"));
        assert_eq!(boot_name("VER = 1.00"), None);
    }
}

/// Reads the real discs: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use std::path::PathBuf;

    fn images(folder: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                images(&path, out);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("iso"))
            {
                out.push(path);
            }
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn the_packed_executables_unpack() {
        // Each boot executable's entry, `$gp`, and `.text` address and size
        // (`docs/research/disc-layout.md`, "Packed executables").
        let expected = [
            (
                "SCES_524.56",
                0x0013_ADA8,
                0x001D_C8B0,
                0x0038_1080,
                0x0007_16F8,
            ),
            (
                "SCUS_973.53",
                0x0013_ADA8,
                0x001D_C8B0,
                0x0038_1180,
                0x0007_1438,
            ),
            (
                "SCUS_974.65",
                0x0015_74E8,
                0x0022_597F,
                0x003F_1000,
                0x0015_C6D8,
            ),
            (
                "SCES_532.85",
                0x0015_74E8,
                0x0022_59FF,
                0x003F_1000,
                0x0015_DC40,
            ),
        ];
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let mut isos = Vec::new();
        images(Path::new(&folder), &mut isos);
        let mut unpacked = Vec::new();
        for iso in isos {
            let (name, bytes) = read_executable(&iso, None).unwrap();
            let Some(&(_, entry, gp, text, size)) = expected.iter().find(|e| e.0 == name) else {
                // R&C1's and GC's are plain ELFs.
                assert!(!elf::is_packed(&bytes), "{name}");
                continue;
            };
            let program = elf::unpack(&bytes).unwrap();
            assert_eq!((program.entry, program.gp()), (entry, Some(gp)), "{name}");
            let section = program.section(".text").unwrap();
            assert_eq!(
                (section.address, section.data.len()),
                (text, size),
                "{name}"
            );
            unpacked.push(name);

            // R&C3 and UYA also carry the network start-up, packed the same way.
            if let Ok((name, bytes)) = read_executable(&iso, Some("I5BOOTN.ELF")) {
                let program = elf::unpack(&bytes).unwrap();
                let core = program.section("core.text").unwrap();
                let end = core.address + core.data.len() as u32;
                assert!((core.address..end).contains(&program.entry), "{name}");
                unpacked.push(name);
            }
        }
        println!("unpacked {unpacked:?}");
        assert!(!unpacked.is_empty(), "no packed executables in {folder}");
    }
}
