//! `openrac-tool mobys`: the mobys placed in each level of a UYA or DL disc image, from the
//! levels' gameplay files, with their pvars (`docs/research/gameplay-format.md`).

use std::path::Path;

use anyhow::{Context, Result, bail};
use rac_formats::gameplay::Gameplay;
use rac_formats::level::Toc;
use rac_iso::{Game, Release};

use crate::open_disc;

pub fn run(iso: &Path, level: Option<usize>, class: Option<u32>) -> Result<()> {
    let mut disc = open_disc(iso)?;
    let game = Release::identify(&mut disc.0)?.info().game;
    let count = match game {
        Game::UpYourArsenal => Toc::read_uya_disc(&mut disc)?.levels.len(),
        Game::Deadlocked => Toc::read_dl_disc(&mut disc)?.levels.len(),
        other => bail!("{other:?}'s gameplay files aren't read yet"),
    };
    let levels = match level {
        Some(level) => vec![level],
        None => (0..count).collect(),
    };
    for number in levels {
        let read = match game {
            Game::Deadlocked => Gameplay::read_dl(&mut disc, number),
            _ => Gameplay::read_uya(&mut disc, number),
        };
        let gameplay = match read {
            Ok(gameplay) => gameplay,
            // Gaps in the level table.
            Err(_) if level.is_none() => continue,
            Err(error) => return Err(error.into()),
        };
        let mobys = gameplay
            .mobys()
            .with_context(|| format!("reading level {number}'s mobys"))?;
        let chosen: Vec<_> = mobys
            .iter()
            .enumerate()
            .filter(|(_, moby)| class.is_none_or(|class| moby.class == class))
            .collect();
        // A whole disc's listing names only the levels that have the class asked for.
        if chosen.is_empty() && class.is_some() && level.is_none() {
            continue;
        }
        println!("level {number}: {} of {} mobys", chosen.len(), mobys.len());
        for (index, moby) in chosen {
            let [x, y, z] = moby.position;
            println!(
                "  {index:4}: class {:#06x} uid {} at ({x:.2}, {y:.2}, {z:.2}) group {}",
                moby.class, moby.uid, moby.group
            );
            let pvars = gameplay
                .pvars(moby.pvar_index)
                .with_context(|| format!("reading moby {index}'s pvars"))?;
            // Up to the last nonzero byte: most pvars end in state the game fills in.
            let used = pvars
                .iter()
                .rposition(|&byte| byte != 0)
                .map_or(0, |at| at + 1);
            if !pvars.is_empty() {
                println!("        {} bytes of pvars", pvars.len());
            }
            let used = used.next_multiple_of(4).min(pvars.len());
            for (row, words) in pvars[..used].chunks(32).enumerate() {
                let words: Vec<String> = words
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|word| describe_word(u32::from_le_bytes(*word)))
                    .collect();
                println!("        +{:#05x}: {}", row * 32, words.join(" "));
            }
        }
    }
    Ok(())
}

/// A pvar word as whatever it most likely is: a small integer, −1, a float in a sensible
/// range, or else its hex.
fn describe_word(word: u32) -> String {
    let float = f32::from_bits(word);
    if word < 0x10000 || word == u32::MAX {
        format!("{}", word as i32)
    } else if float.is_finite() && (1e-4..1e7).contains(&float.abs()) {
        format!("{float}")
    } else {
        format!("{word:#010x}")
    }
}
