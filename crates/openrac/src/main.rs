//! Open Ratchet & Clank Engine: application entry point.
//!
//! It runs the hero kit's demo. If the user's own UYA disc is in the discs folder, the
//! hero plays Ratchet's animations read from it. The roadmap lives in `PLAN.md` at the
//! repository root.

mod discs;
mod flow;
mod hero_anims;
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
    /// Fly a free camera round the demo level instead of playing the hero.
    #[arg(long)]
    spectator: bool,
    /// Where the user's disc images are.
    #[arg(long, value_name = "DIR", default_value = "source")]
    discs: PathBuf,
}

// Bevy's GPU buffers lay out mesh data with `size_of`, which matches the shaders only while
// `Vec4` is 16-byte aligned. glam's `scalar-math` feature (enabled by parry3d's
// `enhanced-determinism`) drops that alignment, and the renderer then fails validation
// whenever a render phase holds a single mesh, such as the demo's water.
const _: () = assert!(
    align_of::<bevy::math::Vec4>() == 16,
    "glam's `scalar-math` feature is on; Bevy's renderer needs 16-byte-aligned Vec4"
);

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
    .insert_resource(rac_demo::DemoSettings {
        spectator: args.spectator,
    })
    .insert_resource(FlowOptions {
        skip_menu: args.demo || replay.is_some(),
        record_to: args.record_input,
    });
    if let Some(recording) = replay {
        app.insert_resource(InputSource::replay(recording));
    }
    if let Some(animations) = hero_animations(&args.discs) {
        app.insert_resource(animations);
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

/// The hero's animations from the user's UYA disc, if one is in `folder`.
fn hero_animations(folder: &std::path::Path) -> Option<rac_rig::RigAnimations> {
    let Some((path, mut disc)) = discs::find(folder, discs::Release::UyaNtsc) else {
        info!(
            "no UYA NTSC-U disc in {}: the hero's animation is procedural",
            folder.display()
        );
        return None;
    };
    let hero = rac_formats::level::Core::read_uya(&mut disc, hero_anims::LEVEL)
        .and_then(|core| core.hero());
    match hero {
        Ok(hero) => {
            let animations = hero_anims::rig_animations(&hero);
            info!(
                "the hero's animations: {} moves from {}",
                animations.clips.len(),
                path.display()
            );
            Some(animations)
        }
        Err(error) => {
            bevy::prelude::warn!("couldn't read the hero from {}: {error}", path.display());
            None
        }
    }
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
