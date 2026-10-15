//! `identify`: which release each disc image is, from its `SYSTEM.CNF` and its boot
//! executable's SHA-1 (PLAN.md P2.2).

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rac_iso::{Disc, Release, SystemCnf};

pub fn run(paths: &[PathBuf]) -> Result<()> {
    let mut images = Vec::new();
    for path in paths {
        if path.is_dir() {
            images.extend(rac_iso::find_images(path, 2));
        } else {
            images.push(path.clone());
        }
    }
    if images.is_empty() {
        bail!("no disc images (.iso) in {paths:?}");
    }
    let mut unknown = 0;
    for image in &images {
        println!("{}", image.display());
        match identify(image) {
            Ok(release) => describe(release),
            Err(error) => {
                unknown += 1;
                println!("  {error:#}");
            }
        }
    }
    if unknown > 0 {
        bail!(
            "{unknown} of {} images aren't releases we know",
            images.len()
        );
    }
    Ok(())
}

fn identify(path: &Path) -> Result<Release> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut disc = Disc::open(BufReader::new(file))?;
    let release = Release::identify(&mut disc);
    if release.is_err()
        && let Ok(cnf) = SystemCnf::read(&mut disc)
    {
        let field = |value: Option<String>| value.unwrap_or_else(|| "none".to_owned());
        println!(
            "  boots {}, VER {}, VMODE {}",
            cnf.boot,
            field(cnf.version),
            field(cnf.video_mode)
        );
    }
    Ok(release?)
}

fn describe(release: Release) {
    let info = release.info();
    println!(
        "  {} ({}), {} v{}",
        info.title, info.region, info.serial, info.version
    );
    println!(
        "  boots {} ({}), SHA-1 {}; table of contents at sector {}",
        info.boot,
        if info.packed { "packed" } else { "plain ELF" },
        info.boot_sha1,
        info.toc_sector
    );
}
