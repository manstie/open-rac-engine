//! `pine-probe`: per-frame hero telemetry from PCSX2, plus savestate and input-recording
//! tools. See `docs/re-setup.md` for the capture workflow.

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use pine_probe::capture::{StopWhen, capture};
use pine_probe::layout::Layout;
use pine_probe::memcard;
use pine_probe::pcsx2_config::{HeadlessSettings, headless_ini};
use pine_probe::pine::{DEFAULT_SLOT, EmuStatus, Pine};
use pine_probe::recording::InputRecording;
use pine_probe::savestate::{GPR_NAMES, INTERNALS, Savestate};
use pine_probe::trace::{TraceHeader, TraceReader, TraceWriter};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show what the PCSX2 instance on a PINE slot is running.
    Status {
        #[arg(long, default_value_t = DEFAULT_SLOT)]
        slot: u16,
        /// Keep retrying for up to this many seconds until a game is running.
        #[arg(long)]
        wait: Option<f64>,
    },
    /// Print an input recording's header and its per-frame pad input as CSV.
    Recording {
        file: PathBuf,
        /// Controller port, 1 or 2.
        #[arg(long, default_value_t = 1)]
        port: usize,
        /// Print only the header.
        #[arg(long)]
        header_only: bool,
    },
    /// List the members of a savestate.
    SavestateList { file: PathBuf },
    /// Write one member of a savestate (for example eeMemory.bin) to a file.
    SavestateExtract {
        file: PathBuf,
        member: String,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Print the EE's general-purpose registers from a savestate (Ghidra needs `$gp`).
    SavestateRegs { file: PathBuf },
    /// Print a layout's fields as they are in a savestate's EE memory.
    SavestatePeek {
        file: PathBuf,
        #[arg(long)]
        layout: PathBuf,
    },
    /// Capture the layout's regions every frame into a trace file.
    Capture {
        #[arg(long)]
        layout: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, default_value_t = DEFAULT_SLOT)]
        slot: u16,
        /// Stop after this many frames.
        #[arg(long)]
        frames: Option<u64>,
        /// Stop after this many seconds.
        #[arg(long)]
        seconds: Option<f64>,
        /// Stop once the frame counter has stood still this long (after it first moved).
        #[arg(long)]
        until_idle: Option<f64>,
    },
    /// Convert a trace to CSV using the layout's fields.
    Export {
        trace: PathBuf,
        #[arg(long)]
        layout: PathBuf,
    },
    /// Patch copies of memory cards so a savestate loads without PCSX2 ejecting them.
    MemcardSync {
        /// The savestate that will be loaded (for a recording: `<recording>_SaveState.p2s`).
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        card1: PathBuf,
        #[arg(long)]
        card2: PathBuf,
    },
    /// Write a PCSX2.ini for the headless replay instance, based on an existing one.
    HeadlessConfig {
        /// The owner's PCSX2.ini.
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        bios_dir: PathBuf,
        #[arg(long, default_value_t = 28012)]
        slot: u16,
        #[arg(long, default_value_t = 0.5)]
        speed: f32,
        /// Bind controller 1 to the keyboard, so xdotool can play the game.
        #[arg(long)]
        keyboard_pad: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Status { slot, wait } => status(slot, wait),
        Command::Recording {
            file,
            port,
            header_only,
        } => recording(file, port, header_only),
        Command::SavestateList { file } => {
            let state = Savestate::open(&file)?;
            for e in state.entries() {
                println!(
                    "{:<32} method {:>2}  {:>10} -> {:>10} bytes",
                    e.name, e.method, e.compressed_size, e.size
                );
            }
            Ok(())
        }
        Command::SavestateExtract { file, member, out } => {
            let data = Savestate::open(&file)?.read(&member)?;
            std::fs::write(&out, data).with_context(|| format!("writing {out:?}"))?;
            Ok(())
        }
        Command::SavestateRegs { file } => {
            let regs = Savestate::open(&file)?.ee_registers()?;
            for (name, value) in GPR_NAMES.iter().zip(regs) {
                println!("{name:<4} {value:#010x}");
            }
            Ok(())
        }
        Command::SavestatePeek { file, layout } => peek(file, layout),
        Command::Capture {
            layout,
            out,
            slot,
            frames,
            seconds,
            until_idle,
        } => {
            let stop = StopWhen {
                frames,
                seconds,
                idle_seconds: until_idle,
            };
            run_capture(layout, out, slot, stop)
        }
        Command::Export { trace, layout } => export(trace, layout),
        Command::MemcardSync {
            state,
            card1,
            card2,
        } => memcard_sync(state, [card1, card2]),
        Command::HeadlessConfig {
            from,
            out,
            bios_dir,
            slot,
            speed,
            keyboard_pad,
        } => {
            let source =
                std::fs::read_to_string(&from).with_context(|| format!("reading {from:?}"))?;
            let settings = HeadlessSettings {
                bios_dir: bios_dir.to_string_lossy().into_owned(),
                pine_slot: slot,
                speed,
                keyboard_pad,
            };
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&out, headless_ini(&source, &settings))?;
            Ok(())
        }
    }
}

fn status(slot: u16, wait: Option<f64>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs_f64(wait.unwrap_or(0.0));
    loop {
        let attempt = (|| -> Result<_> {
            let mut pine = Pine::connect(slot)?;
            let status = pine.status()?;
            let id = pine.game_id().unwrap_or_default();
            let title = pine.title().unwrap_or_default();
            Ok((pine.version()?, status, id, title))
        })();
        match attempt {
            Ok((version, status, id, title)) if status != EmuStatus::Shutdown && !id.is_empty() => {
                println!("{version}: {status:?}, {id} ({title})");
                return Ok(());
            }
            result if Instant::now() >= deadline => {
                return match result {
                    Ok((version, status, ..)) => {
                        println!("{version}: {status:?}, no game running");
                        Ok(())
                    }
                    Err(e) => Err(e),
                };
            }
            _ => std::thread::sleep(Duration::from_millis(250)),
        }
    }
}

fn recording(file: PathBuf, port: usize, header_only: bool) -> Result<()> {
    if !(1..=2).contains(&port) {
        bail!("--port must be 1 or 2");
    }
    let rec = InputRecording::open(&file)?;
    println!(
        "# {} by {:?} with {}: {} frames, undo count {}, from savestate: {}",
        rec.game, rec.author, rec.emulator, rec.total_frames, rec.undo_count, rec.from_savestate
    );
    if header_only {
        return Ok(());
    }
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    writeln!(
        out,
        "frame,buttons,lx,ly,rx,ry,{}",
        pine_probe::recording::PRESSURE_ORDER
            .map(|b| format!("p_{b}"))
            .join(",")
    )?;
    for (n, frame) in rec.frames.iter().enumerate() {
        let pad = frame[port - 1];
        let pressures: Vec<String> = pad.pressure.iter().map(u8::to_string).collect();
        writeln!(
            out,
            "{n},{},{},{},{},{},{}",
            pad.buttons.names(),
            pad.left_stick[0],
            pad.left_stick[1],
            pad.right_stick[0],
            pad.right_stick[1],
            pressures.join(",")
        )?;
    }
    Ok(())
}

fn peek(file: PathBuf, layout: PathBuf) -> Result<()> {
    let layout = Layout::load(&layout)?;
    let memory = Savestate::open(&file)?.ee_memory()?;
    let read = |base: u32, size: u32| -> Result<&[u8]> {
        memory
            .get((base & 0x01FF_FFFF) as usize..((base & 0x01FF_FFFF) + size) as usize)
            .context("region lies outside EE memory")
    };
    let mut sample = Vec::with_capacity(layout.sample_len());
    for r in &layout.regions {
        sample.extend_from_slice(read(r.base, r.size)?);
    }
    let frame = u32::from_le_bytes(read(layout.frame_counter, 4)?.try_into().unwrap());
    println!("frame_counter = {frame}");
    for (name, value) in layout.columns().iter().zip(layout.format_sample(&sample)) {
        println!("{name} = {value}");
    }
    Ok(())
}

fn run_capture(layout: PathBuf, out: PathBuf, slot: u16, stop: StopWhen) -> Result<()> {
    let layout = Layout::load(&layout)?;
    let mut pine = Pine::connect(slot).context("is PCSX2 running with PINE enabled?")?;
    let id = pine.game_id().unwrap_or_default();
    if !id.is_empty() && id != layout.release {
        bail!(
            "PCSX2 is running {id}, but the layout is for {}",
            layout.release
        );
    }
    let mut writer = TraceWriter::create(&out, &TraceHeader::from_layout(&layout))?;
    let stats = capture(&mut pine, &layout, &mut writer, &stop)?;
    writer.finish()?;
    eprintln!(
        "captured {} frames ({} frame-counter gaps) from {} samples ({} torn) into {out:?}",
        stats.frames, stats.gaps, stats.samples, stats.torn
    );
    Ok(())
}

fn export(trace: PathBuf, layout: PathBuf) -> Result<()> {
    let layout = Layout::load(&layout)?;
    let mut reader = TraceReader::open(&trace)?;
    reader.header.check_layout(&layout)?;
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    writeln!(out, "frame,time,{}", layout.columns().join(","))?;
    while let Some(record) = reader.next_record()? {
        writeln!(
            out,
            "{},{:.4},{}",
            record.frame,
            record.time,
            layout.format_sample(&record.data).join(",")
        )?;
    }
    Ok(())
}

fn memcard_sync(state: PathBuf, cards: [PathBuf; 2]) -> Result<()> {
    let internal = Savestate::open(&state)?.read(INTERNALS)?;
    let mut data = Vec::new();
    let mut current = [0u64; 2];
    for (i, path) in cards.iter().enumerate() {
        let card = std::fs::read(path).with_context(|| format!("reading {path:?}"))?;
        current[i] = memcard::card_checksum(&card)
            .with_context(|| format!("{path:?} is not an 8 MiB PS2 memory card"))?;
        data.push(card);
    }
    let stored = memcard::find_stored_checksums(&internal, current)
        .context("could not find the memory-card checksums in the savestate")?;
    for i in 0..2 {
        if stored[i] == current[i] {
            println!("card {}: already matches the savestate", i + 1);
        } else {
            memcard::set_card_checksum(&mut data[i], stored[i]);
            std::fs::write(&cards[i], &data[i])?;
            println!(
                "card {}: checksum {:#018x} -> {:#018x}",
                i + 1,
                current[i],
                stored[i]
            );
        }
    }
    Ok(())
}
