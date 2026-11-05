//! Open Ratchet & Clank Engine: application entry point.
//!
//! It runs the hero kit's demo. If the user's own UYA disc is in the discs folder, the
//! hero plays Ratchet's animations read from it, holds and hears its guns, and Ratchet's
//! own model can be chosen in the menus, as can the game he moves like. The roadmap lives in `PLAN.md` at the
//! repository root.

// Release builds are a Windows GUI program, so starting the game opens no console window
// beside it (`attach_parent_console` brings the log back in a terminal). Debug builds keep
// the console. Other platforms ignore it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod camera;
mod coop;
mod discs;
mod flow;
mod fx_textures;
mod gun_models;
mod gun_sounds;
mod hero_anims;
mod hero_model;
mod menu;
mod moves;
mod options;
mod settings;
mod shot_models;

use anyhow::{Context, Result};
// Named imports: the Bevy prelude's `ContextExt` would clash with anyhow's `Context`.
use bevy::{
    diagnostic::FrameCount,
    prelude::{
        App, AppExit, DefaultPlugins, MessageWriter, PluginGroup, Res, Resource, Update, Window,
        WindowPlugin, default, info, warn,
    },
    render::RenderPlugin,
};
use clap::Parser;
use rac_input::{InputRecording, InputSource, PlayerInputPlugin};
use std::path::PathBuf;

use crate::{
    flow::{FlowOptions, FlowPlugin},
    menu::MenuPlugin,
    options::OptionsPlugin,
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
    /// The hero's model: `open`, or one imported from a disc (`uya` for UYA's first armor,
    /// `uya-snowman` and others for its other suits, `uya-mp-robo` and others for its
    /// multiplayer skins, `dl-ratchet`, `dl-jak` and others for DL's skins). Overrides the
    /// one saved in the settings, which the menus change.
    #[arg(long, value_name = "NAME")]
    model: Option<String>,
    /// Which game the hero moves like: `uya` (the default) or `dl`, or a movement profile's
    /// RON file. Overrides the one saved in the settings, which the menus change.
    #[arg(long, value_name = "GAME|FILE")]
    moves: Option<String>,
    /// Whose guns the hero carries: `uya` (the default) or `dl`, Deadlocked's read so far.
    #[arg(long, value_name = "GAME")]
    guns: Option<String>,
    /// The level the demo gives every gun, 1 (V1) to 8 (V8); without it, 3.
    #[arg(long, value_name = "LEVEL", value_parser = clap::value_parser!(u8).range(1..=8))]
    gun_level: Option<u8>,
    /// The hero's camera: `third-person`, `lock-strafe` or `first-person`. Without it, the
    /// one saved in the settings, which the menus change, or else the game's own (third
    /// person for UYA, lock-strafe for DL).
    #[arg(long, value_name = "SETTING", value_parser = parse_camera)]
    camera: Option<rac_hero::CameraSetting>,
    /// Draw on a sandy floor with a tone mapping that keeps effects' colours, to judge them
    /// against the game's.
    #[arg(long)]
    plain_colours: bool,
    /// Run one simulation tick for each frame drawn, however long drawing it takes, so an
    /// off-screen recording on a slow renderer shows every tick, slowed down.
    #[arg(long)]
    every_tick: bool,
    /// Host a co-op game: wait for player 2 on this UDP port (26450 if none is given).
    /// Implies --demo.
    #[arg(long, value_name = "PORT", num_args = 0..=1, default_missing_value = "26450",
          conflicts_with_all = ["join", "sync_test", "replay_input", "spectator"])]
    host: Option<u16>,
    /// Join a co-op game as player 2, by the host's join code or at HOST[:PORT]. Implies
    /// --demo.
    #[arg(long, value_name = "CODE|HOST[:PORT]", value_parser = coop::parse_join,
          conflicts_with_all = ["sync_test", "replay_input", "spectator"])]
    join: Option<coop::HostAddresses>,
    /// Play both co-op heroes here, rolling every tick back and simulating it again to
    /// check that rollback restores the game exactly. Implies --demo.
    #[arg(long, conflicts_with_all = ["replay_input", "spectator"])]
    sync_test: bool,
    /// In co-op, the ticks each player's input waits before it applies (default 2): more
    /// hides more of the connection's delay, but makes the controls feel later.
    #[arg(long, value_name = "TICKS")]
    input_delay: Option<usize>,
    /// When hosting, here or from the menus, don't ask the router to forward the port
    /// (PCP, NAT-PMP or UPnP): player 2 then reaches this machine by its code's other
    /// addresses, or a port forwarded by hand.
    #[arg(long)]
    no_port_mapping: bool,
}

fn parse_camera(name: &str) -> Result<rac_hero::CameraSetting, String> {
    camera::parse(name)
        .ok_or_else(|| format!("{name:?} is not third-person, lock-strafe or first-person"))
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
    #[cfg(windows)]
    attach_parent_console();
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

    let moves = match &args.moves {
        Some(choice) => (choice.clone(), moves::load(choice)?),
        None => {
            let choice = settings.hero_moves.as_deref().unwrap_or(moves::DEFAULT);
            match moves::load(choice) {
                Ok(profile) => (choice.to_owned(), profile),
                Err(error) => {
                    warn!("the saved movement profile can't be used, so UYA's applies: {error:#}");
                    (moves::DEFAULT.to_owned(), moves::load(moves::DEFAULT)?)
                }
            }
        }
    };

    let (wgpu_settings, started_backend) = options::renderer_settings(settings.display.backend);
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Open Ratchet & Clank Engine".into(),
                    mode: settings.display.window_mode(),
                    resolution: settings.display.window_resolution(),
                    present_mode: settings.display.present_mode(),
                    ..default()
                }),
                ..default()
            })
            // Compile pipelines one at a time. With NVIDIA's 595.91.07 Linux driver,
            // Bevy's default parallel compilation makes the driver's shader compiler
            // fail ("NVVM compilation failed: 3"), losing the device and segfaulting
            // within a second of startup. Revisit when the driver or wgpu changes.
            .set(RenderPlugin {
                render_creation: wgpu_settings.into(),
                synchronous_pipeline_compilation: true,
                ..default()
            }),
    )
    // Saved settings go in before the plugins, which would otherwise use defaults.
    .insert_resource(settings.input)
    .insert_resource(settings.display)
    .insert_resource(started_backend)
    .insert_resource(menu::KeyNames::from_saved(&settings.key_names))
    .insert_resource(settings.sound)
    .add_plugins((
        rac_sim::SimPlugin,
        PlayerInputPlugin,
        rac_demo::DemoPlugin,
        FlowPlugin,
        MenuPlugin,
        OptionsPlugin,
    ))
    .insert_resource(rac_demo::DemoSettings {
        spectator: args.spectator,
        camera: args.camera.or_else(|| {
            let saved = settings.hero_camera.as_deref()?;
            let setting = camera::parse(saved);
            if setting.is_none() {
                warn!("the saved camera setting {saved:?} isn't one, so the game's applies");
            }
            setting
        }),
        ..Default::default()
    })
    .insert_resource(
        args.gun_level
            .map_or_else(rac_demo::DemoGunLevel::default, |level| {
                rac_demo::DemoGunLevel(level - 1)
            }),
    )
    .insert_resource(match args.guns.as_deref() {
        None | Some("uya") => rac_combat::Armory::uya(),
        Some("dl") => rac_combat::Armory::dl(),
        Some(other) => anyhow::bail!("{other:?} isn't a game with guns: uya or dl"),
    })
    .insert_resource(rac_hero::HeroTuning(moves.1))
    .insert_resource(moves::ChosenMoves(moves.0))
    .insert_resource(FlowOptions {
        skip_menu: args.demo
            || replay.is_some()
            || args.host.is_some()
            || args.join.is_some()
            || args.sync_test,
        record_to: args.record_input,
    })
    .add_plugins(coop::CoopPlugin)
    .insert_resource(match (args.host, args.join, args.sync_test) {
        (Some(port), ..) => coop::CoopChoice::Host(port),
        (_, Some(host), _) => coop::CoopChoice::Join(host.0),
        (.., true) => coop::CoopChoice::SyncTest,
        _ => coop::CoopChoice::Alone,
    });
    if args.no_port_mapping {
        app.insert_resource(coop::NoPortMapping);
    }
    if let Some(delay) = args.input_delay {
        app.insert_resource(coop::CoopSettings(rac_net::SessionSettings {
            input_delay: delay,
            ..rac_net::SessionSettings::default()
        }));
    }
    if let Some(recording) = replay {
        app.insert_resource(InputSource::replay(recording));
    }
    let read = from_disc(&args.discs);
    if let Some(animations) = read.animations {
        app.insert_resource(animations);
    }
    if let Some(sounds) = read.gun_sounds {
        gun_sounds::add(&mut app, sounds);
    }
    app.insert_resource(hero_model::DiscModels(read.models))
        .insert_resource(gun_models::DiscGuns(read.guns))
        .insert_resource(shot_models::DiscShots(read.shots))
        .insert_resource(shot_models::DiscEffectModels(read.effect_models))
        .insert_resource(shot_models::DiscRopeModels(read.rope_models))
        .insert_resource(read.effect_textures)
        .add_systems(
            bevy::prelude::Startup,
            (
                hero_model::register_disc_models,
                gun_models::register_disc_guns,
                shot_models::register_disc_shots,
                shot_models::register_disc_effect_models,
                shot_models::register_disc_rope_models,
            ),
        )
        // After the HUD has painted its placeholders.
        .add_systems(
            bevy::prelude::PostStartup,
            (
                fx_textures::register_disc_sight_images,
                fx_textures::register_disc_fx_images,
            ),
        );
    if let Some(model) = args.model.as_ref().or(settings.hero_model.as_ref()) {
        app.world_mut()
            .resource_mut::<rac_rig::RigModels>()
            .choose(model);
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
    if args.plain_colours {
        app.add_systems(Update, plain_colours)
            .insert_resource(rac_demo::DemoGround((0.4, 0.31, 0.16)));
    }
    if args.every_tick {
        app.insert_resource(bevy::time::TimeUpdateStrategy::FixedTimesteps(1));
    }

    Ok(app.run())
}

/// Sends the log, `--help` and errors to the terminal the game was started from: a Windows
/// GUI program has no console of its own. Started from Explorer there is none, and debug
/// builds already have one, so it does nothing; it is built in them anyway so CI lints it.
#[cfg(windows)]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: it takes no pointers, and failing (no parent console, or one already
    // attached) leaves the process as it was.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// What the user's discs give the hero.
#[derive(Default)]
struct FromDisc {
    animations: Option<rac_rig::RigAnimations>,
    models: Vec<hero_model::Imported>,
    guns: Vec<gun_models::DiscGun>,
    shots: Vec<shot_models::DiscShot>,
    effect_models: Vec<shot_models::DiscEffectModel>,
    rope_models: Vec<shot_models::DiscEffectModel>,
    effect_textures: fx_textures::DiscEffectTextures,
    gun_sounds: Option<gun_sounds::DiscGunSounds>,
}

/// The hero's animations, models and guns from the user's discs in `folder`: from UYA his
/// animations, suits, multiplayer skins and guns with their sounds, from DL its skins.
fn from_disc(folder: &std::path::Path) -> FromDisc {
    let mut read = from_uya(folder);
    read.models.extend(from_dl(folder));
    read
}

/// The hero's animations, suits, multiplayer skins and guns, with the guns' sounds, from a
/// UYA NTSC-U disc, if one is in `folder`.
fn from_uya(folder: &std::path::Path) -> FromDisc {
    let Some((path, mut disc)) = discs::find(folder, discs::Release::UyaNtsc) else {
        info!(
            "no UYA NTSC-U disc in {}: the hero's animation is procedural",
            folder.display()
        );
        return FromDisc::default();
    };
    let read = rac_formats::level::Core::read_uya(&mut disc, hero_anims::LEVEL)
        .and_then(|core| core.hero().map(|hero| (core, hero)));
    let (core, hero) = match read {
        Ok(read) => read,
        Err(error) => {
            warn!("couldn't read the hero from {}: {error}", path.display());
            return FromDisc::default();
        }
    };
    let animations = match hero_anims::rig_animations(&hero) {
        Ok(mut animations) => {
            info!(
                "the hero's animations: {} moves from {}",
                animations.clips.len(),
                path.display()
            );
            match hero_anims::back_animations(&core) {
                Ok(back) => animations.back = Some(back),
                Err(error) => warn!("Clank and the heli-pack in {}: {error}", path.display()),
            }
            match hero_anims::hold_animations(&core, &hero) {
                Ok(hold) => animations.hold = Some(hold),
                Err(error) => warn!("the arms holding a gun in {}: {error}", path.display()),
            }
            Some(animations)
        }
        Err(error) => {
            warn!("the hero's skeleton in {}: {error}", path.display());
            None
        }
    };
    let suits = [hero_model::SUITS, hero_model::MP_SKINS].concat();
    let read = hero_model::import(&mut disc, &core, &hero, &suits);
    let models = imported(&suits, read, &path);
    let effects = core.fx_textures().unwrap_or_else(|error| {
        warn!(
            "couldn't read the effect textures from {}: {error}",
            path.display()
        );
        Vec::new()
    });
    let particles = core
        .particle_textures()
        .and_then(|textures| Ok((textures, core.header.particle_lists()?)))
        .unwrap_or_else(|error| {
            warn!(
                "couldn't read the particle textures from {}: {error}",
                path.display()
            );
            Default::default()
        });
    let effect_textures = fx_textures::DiscEffectTextures {
        effects,
        particles: particles.0,
        particle_lists: particles.1,
    };
    let gun_sounds = match gun_sounds::import(&mut disc, &core, hero_anims::LEVEL) {
        Ok(sounds) => Some(sounds),
        Err(error) => {
            warn!(
                "couldn't read the guns' sounds from {}: {error}",
                path.display()
            );
            None
        }
    };
    let mut shots = Vec::new();
    for (shot, row) in shot_models::import(&core)
        .into_iter()
        .zip(shot_models::UYA_SHOT_CLASSES)
    {
        match shot {
            Ok(shot) => shots.push(shot),
            Err(error) => warn!(
                "couldn't read shot class {:#x} from {}: {error}",
                row.class,
                path.display()
            ),
        }
    }
    let mut effect_models = Vec::new();
    for (model, class) in shot_models::import_effects(&core)
        .into_iter()
        .zip(shot_models::UYA_EFFECT_CLASSES)
    {
        match model {
            Ok(model) => effect_models.push(model),
            Err(error) => warn!(
                "couldn't read effect class {class:#x} from {}: {error}",
                path.display()
            ),
        }
    }
    let mut rope_models = Vec::new();
    for (model, class) in shot_models::import_rope(&core)
        .into_iter()
        .zip(shot_models::UYA_ROPE_CLASSES)
    {
        match model {
            Ok(model) => rope_models.push(model),
            Err(error) => warn!(
                "couldn't read the whip's link class {class:#x} from {}: {error}",
                path.display()
            ),
        }
    }
    FromDisc {
        animations,
        models,
        guns: guns(&mut disc, &core, &hero, &path),
        shots,
        effect_models,
        rope_models,
        effect_textures,
        gun_sounds,
    }
}

/// The guns' models from a UYA disc, with a warning for each that wasn't read.
fn guns(
    disc: &mut impl rac_formats::DiscReader,
    core: &rac_formats::level::Core,
    hero: &rac_formats::moby::MobyClass,
    path: &std::path::Path,
) -> Vec<gun_models::DiscGun> {
    let read = match gun_models::import(disc, core, hero) {
        Ok(read) => read,
        Err(error) => {
            warn!("couldn't read the guns from {}: {error}", path.display());
            return Vec::new();
        }
    };
    let mut guns = Vec::new();
    for (model, gun) in gun_models::UYA_GUN_MODELS.iter().zip(read) {
        match gun {
            Ok(gun) => guns.push(gun),
            Err(error) => warn!(
                "couldn't read gun {:#x}'s model from V{} from {}: {error}",
                model.item,
                model.level + 1,
                path.display()
            ),
        }
    }
    guns
}

/// The skins from a DL NTSC-U or Gladiator PAL disc, if one is in `folder`. Both hold the
/// same skins, so one is enough.
fn from_dl(folder: &std::path::Path) -> Vec<hero_model::Imported> {
    let found = discs::find(folder, discs::Release::DlNtsc)
        .or_else(|| discs::find(folder, discs::Release::GladiatorPal));
    let Some((path, mut disc)) = found else {
        return Vec::new();
    };
    let skins = hero_model::import_dl(&mut disc, hero_model::DL_SUITS);
    imported(hero_model::DL_SUITS, skins, &path)
}

/// The models of `suits` that were read, with a warning for each that wasn't.
fn imported(
    suits: &[hero_model::Suit],
    read: Result<
        Vec<Result<hero_model::Imported, hero_model::ImportError>>,
        hero_model::ImportError,
    >,
    path: &std::path::Path,
) -> Vec<hero_model::Imported> {
    let read = match read {
        Ok(read) => read,
        Err(error) => {
            warn!(
                "couldn't read the hero's models from {}: {error}",
                path.display()
            );
            return Vec::new();
        }
    };
    let mut models = Vec::new();
    for (suit, model) in suits.iter().zip(read) {
        match model {
            Ok(model) => models.push(model),
            Err(error) => warn!(
                "couldn't read the model \"{}\" from {}: {error}",
                suit.name,
                path.display()
            ),
        }
    }
    models
}

#[derive(Resource)]
struct ExitAfterFrames(u32);

fn plain_colours(
    mut cameras: bevy::prelude::Query<
        &mut bevy::core_pipeline::tonemapping::Tonemapping,
        bevy::prelude::Added<bevy::prelude::Camera3d>,
    >,
) {
    for mut tonemapping in &mut cameras {
        *tonemapping = bevy::core_pipeline::tonemapping::Tonemapping::ReinhardLuminance;
    }
}

fn exit_after_frames(
    limit: Res<ExitAfterFrames>,
    frames: Res<FrameCount>,
    mut exit: MessageWriter<AppExit>,
) {
    if frames.0 >= limit.0 {
        exit.write(AppExit::Success);
    }
}
