//! Open Ratchet & Clank Engine: application entry point.
//!
//! With no supported disc (there is no disc support yet) it runs the hero kit's demo. The
//! roadmap lives in `PLAN.md` at the repository root.

mod flow;
mod menu;
mod settings;

use anyhow::{Context, Result};
// Named imports: the Bevy prelude's `ContextExt` would clash with anyhow's `Context`.
use bevy::{
    diagnostic::FrameCount,
    prelude::{
        App, AppExit, DefaultPlugins, MessageWriter, PluginGroup, Res, Resource, Update, Window,
        WindowPlugin, default, info,
    },
    render::RenderPlugin,
};
use clap::Parser;
use rac_input::{InputRecording, InputSource, PlayerInputPlugin};
use std::path::PathBuf;

use crate::{
    flow::{FlowOptions, FlowPlugin},
    menu::MenuPlugin,
    settings::Paths,
};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Quit with success after this many frames, so CI and smoke tests can check that the
    /// app boots and renders without a human present.
    #[arg(long, value_name = "N")]
    exit_after_frames: Option<u32>,
    /// Go straight into the demo, skipping the main menu.
    #[arg(long)]
    demo: bool,
    /// Record every tick's input to FILE (an `.orin` recording).
    #[arg(long, value_name = "FILE")]
    record_input: Option<PathBuf>,
    /// Replay the input recording in FILE instead of reading the devices. Implies --demo.
    #[arg(long, value_name = "FILE", conflicts_with = "record_input")]
    replay_input: Option<PathBuf>,
    /// Use FILE for settings instead of the platform's config directory.
    #[arg(long, value_name = "FILE")]
    settings: Option<PathBuf>,
}

fn main() -> Result<AppExit> {
    let args = Args::parse();
    let paths = Paths::resolve(args.settings.clone())?;
    let settings = settings::load_or_create(&paths.settings_file);
    let replay = args
        .replay_input
        .as_ref()
        .map(|path| -> Result<InputRecording> {
            let bytes =
                std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            InputRecording::from_bytes(&bytes)
                .with_context(|| format!("loading the input recording {}", path.display()))
        })
        .transpose()?;

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Open Ratchet & Clank Engine".into(),
                    ..default()
                }),
                ..default()
            })
            // Compile pipelines one at a time. With NVIDIA's 595.91.07 Linux driver,
            // Bevy's default parallel compilation makes the driver's shader compiler
            // fail ("NVVM compilation failed: 3"), losing the device and segfaulting
            // within a second of startup. Revisit when the driver or wgpu changes.
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            }),
    )
    // Saved settings go in before the input plugin, which would otherwise use defaults.
    .insert_resource(settings.input)
    .add_plugins((
        rac_sim::SimPlugin,
        PlayerInputPlugin,
        rac_demo::DemoPlugin,
        FlowPlugin,
        MenuPlugin,
    ))
    .insert_resource(FlowOptions {
        skip_menu: args.demo || replay.is_some(),
        record_to: args.record_input,
    });
    if let Some(recording) = replay {
        app.insert_resource(InputSource::replay(recording));
    }
    info!(
        "settings: {}; data: {}",
        paths.settings_file.display(),
        paths.data_dir.display()
    );
    app.insert_resource(paths);

    if let Some(frames) = args.exit_after_frames {
        app.insert_resource(ExitAfterFrames(frames))
            .add_systems(Update, exit_after_frames);
    }

    Ok(app.run())
}

#[derive(Resource)]
struct ExitAfterFrames(u32);

fn exit_after_frames(
    limit: Res<ExitAfterFrames>,
    frames: Res<FrameCount>,
    mut exit: MessageWriter<AppExit>,
) {
    if frames.0 >= limit.0 {
        exit.write(AppExit::Success);
    }
}
