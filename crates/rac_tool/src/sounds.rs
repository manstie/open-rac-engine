//! `openrac-tool sounds`: lists a UYA sound bank's sounds, a gun's from the gadget WAD or a
//! level's, with the class sound tables that play them, and renders each through
//! `rac_audio` to a WAV file to hear (`docs/research/effects.md`, "Sounds"). The files are
//! the disc's sounds, so they go under `cache/`.

use std::path::Path;

use anyhow::{Context, Result, bail};
use rac_audio::{Play, Synth};
use rac_formats::gadget::{Gadget, GadgetTable};
use rac_formats::level::{Core, Toc, read_uya_sound_bank};
use rac_formats::sound::{ClassSound, GrainOp, SoundBank};

use crate::open_disc;

/// The longest a sound is rendered for, in seconds: loops never end on their own.
const LONGEST: f32 = 6.0;

pub fn run(
    iso: &Path,
    gadget: Option<u32>,
    level: Option<usize>,
    out: Option<&Path>,
) -> Result<()> {
    let mut disc = open_disc(iso)?;
    let (bank, tables) = match (gadget, level) {
        (Some(class), _) => {
            let table = GadgetTable::read_uya(&Toc::read_uya_disc(&mut disc)?)?;
            let Some(index) = table.find(class) else {
                bail!("the gadget WAD has no class {class:#x}");
            };
            let gun = Gadget::read_from(&mut disc, &table, index)?;
            let bank = table
                .read_sounds(&mut disc, index)?
                .with_context(|| format!("gadget entry {index} has no sound bank"))?;
            println!("gadget entry {index}, class {class:#x}");
            (bank, vec![(class, gun.sounds)])
        }
        (None, Some(level)) => {
            let bank = read_uya_sound_bank(&mut disc, level)?;
            let core = Core::read_uya(&mut disc, level)?;
            let mut tables = Vec::new();
            for entry in &core.header.moby_classes {
                if entry.offset != 0
                    && let Ok(sounds) = core.moby_sounds(entry.class)
                    && !sounds.is_empty()
                {
                    tables.push((entry.class, sounds));
                }
            }
            println!("level {level}");
            (bank, tables)
        }
        (None, None) => bail!("give --gadget or --level"),
    };
    describe(&bank, &tables);
    if let Some(out) = out {
        std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
        render_all(bank, out)?;
    }
    Ok(())
}

fn describe(bank: &SoundBank, tables: &[(u32, Vec<ClassSound>)]) {
    let name = String::from_utf8_lossy(&bank.name);
    println!(
        "bank {:?}, version {}, {} sounds, {} bytes of samples",
        name.trim_end_matches('\0'),
        bank.version,
        bank.sounds.len(),
        bank.samples.len()
    );
    for (number, sound) in bank.sounds.iter().enumerate() {
        let tones = sound
            .grains
            .iter()
            .filter(|grain| matches!(grain.op, GrainOp::Tone(_)))
            .count();
        println!(
            "  sound {number}: volume {}, group {}, flags {:#x}, {} grains, {tones} tones",
            sound.volume,
            sound.group,
            sound.flags,
            sound.grains.len()
        );
    }
    for (class, table) in tables {
        println!("class {class:#x}:");
        for (index, entry) in table.iter().enumerate() {
            println!(
                "  {index}: sound {:?} of bank {}, {}–{} u, volume {:#x}–{:#x}, pitch {}–{}{}",
                entry.sound,
                entry.bank,
                entry.near,
                entry.far,
                entry.max_volume,
                entry.min_volume,
                entry.pitch_low,
                entry.pitch_high,
                if entry.looped { ", loops" } else { "" }
            );
        }
    }
}

/// Plays each sound alone at full volume until it ends (or [`LONGEST`]) and writes it as
/// 16-bit stereo at 48 kHz.
fn render_all(bank: SoundBank, out: &Path) -> Result<()> {
    let count = bank.sounds.len();
    let mut synth = Synth::new();
    let id = synth.add_bank(bank);
    for number in 0..count {
        let Some(handle) = synth.play(id, number, Play::default()) else {
            continue;
        };
        let mut samples = Vec::new();
        let mut block = vec![0.0; 2 * 480];
        let most = (LONGEST * rac_audio::SAMPLE_RATE as f32) as usize * 2;
        while synth.is_playing(handle) && samples.len() < most {
            synth.render(&mut block);
            samples.extend_from_slice(&block);
        }
        synth.stop_all();
        while synth.voices() > 0 {
            synth.render(&mut block);
        }
        let peak = samples.iter().fold(0f32, |peak, s| peak.max(s.abs()));
        let path = out.join(format!("sound-{number:03}.wav"));
        write_wav(&path, &samples)?;
        println!(
            "wrote {} ({:.2} s, peak {peak:.2})",
            path.display(),
            samples.len() as f32 / 2.0 / rac_audio::SAMPLE_RATE as f32
        );
    }
    Ok(())
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let data = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&rac_audio::SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(rac_audio::SAMPLE_RATE * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}
