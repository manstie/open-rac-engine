//! Open Ratchet & Clank Engine: application entry point.
//!
//! It runs the hero kit's demo. If the user's own UYA disc is in the discs folder, the
//! hero plays Ratchet's animations read from it, holds and hears its guns, and Ratchet's
//! own model can be chosen in the menus, as can the game he moves like. The roadmap lives in `PLAN.md` at the
//! repository root.

mod camera;
mod discs;
mod flow;
mod fx_textures;
mod gun_models;
mod gun_sounds;
mod hero_anims;
mod hero_model;
mod menu;
mod moves;
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
    /// The hero's camera: `third-person`, `lock-strafe` or `first-person`. Without it, the
    /// one saved in the settings, which the menus change, or else the game's own (third
    /// person for UYA, lock-strafe for DL).
    #[arg(long, value_name = "SETTING", value_parser = parse_camera)]
    camera: Option<rac_hero::CameraSetting>,
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
        camera: args.camera.or_else(|| {
            let saved = settings.hero_camera.as_deref()?;
            let setting = camera::parse(saved);
            if setting.is_none() {
                warn!("the saved camera setting {saved:?} isn't one, so the game's applies");
            }
            setting
        }),
    })
    .insert_resource(rac_hero::HeroTuning(moves.1))
    .insert_resource(moves::ChosenMoves(moves.0))
    .insert_resource(FlowOptions {
        skip_menu: args.demo || replay.is_some(),
        record_to: args.record_input,
    });
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

    Ok(app.run())
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
    for (shot, (_, class, _)) in shot_models::import(&core)
        .into_iter()
        .zip(shot_models::UYA_SHOT_CLASSES)
    {
        match shot {
            Ok(shot) => shots.push(shot),
            Err(error) => warn!(
                "couldn't read shot class {class:#x} from {}: {error}",
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

fn exit_after_frames(
    limit: Res<ExitAfterFrames>,
    frames: Res<FrameCount>,
    mut exit: MessageWriter<AppExit>,
) {
    if frames.0 >= limit.0 {
        exit.write(AppExit::Success);
    }
}
