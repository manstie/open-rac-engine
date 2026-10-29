//! The guns' shots' models from the user's UYA disc (PLAN.md P3.18): the shot classes with
//! a mesh in Veldin's core (`docs/research/effects.md`, "Shot classes on the disc"), drawn
//! by the kit in place of its shapes ([`ShotModels`]), the classes their effects spawn
//! ([`FxModels`]) and the Plasma Whip's links ([`RopeModels`]). Each is its class's mesh at
//! rest, textured from the level's textures; nothing is stored.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::Armory;
use rac_demo::fx::{FxModel, FxModels};
use rac_demo::{ShotModel, ShotModels};
use rac_formats::level::Core;
use rac_formats::moby::PacketKind;
use rac_rig::{RopeModel, RopeModels, Surface};

use crate::hero_model::{Builder, ImportError, ImportPaint, add_skinned, material};

/// Each gun's shot class by its item, from its code (`effects.md`, "Each gun's effects")
/// [code]: the Nitro Launcher's charge, the Annihilator's missile, the Disc Blade, the
/// Bouncer's canister, the Miniturret, the RY3NO's rocket and the Infector's blob; each
/// but the blob looked right in flight in the app. Their V5 shots' classes aren't drawn.
/// The Plasma Coil's ball (0xA9B, untextured) and the rift (0x186B, 0.05 units across)
/// are drawn by their code; those keep their sprites. Each is drawn at its class's scale
/// times the third number, as its gun's code sets it: the Bouncer's canister at three
/// times (0x1DC8BC [data]), the blob, a mound of goo 1.4 units high, at a random 0.25 to
/// 0.3 (0x3DE3A8), which tumbles there and here faces its way.
pub const UYA_SHOT_CLASSES: &[(u16, u32, f32)] = &[
    (0x77, 0x185A, 1.0),
    (0x3F, 0x1894, 1.0),
    (0x4F, 0x1867, 1.0),
    (0x13, 0xE64, 3.0),
    (0x15, 0x1366, 1.0),
    (0x97, 0x1873, 1.0),
    (0x37, 0x1E05, 0.275),
];

/// A shot's model read from the disc, waiting for its materials.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscShot {
    pub item: u16,
    pub surfaces: Vec<Surface>,
    pub paints: Vec<ImportPaint>,
}

/// Shot models read from the disc.
#[derive(Resource, Debug, Default)]
pub struct DiscShots(pub Vec<DiscShot>);

/// Each of [`UYA_SHOT_CLASSES`] from `core`.
pub fn import(core: &Core) -> Vec<Result<DiscShot, ImportError>> {
    UYA_SHOT_CLASSES
        .iter()
        .map(|&(item, class, scale)| {
            let (surfaces, paints) = read_class(core, class, scale)?;
            Ok(DiscShot {
                item,
                surfaces,
                paints,
            })
        })
        .collect()
}

/// The classes the guns' effects spawn as models, at their classes' scale: the ring the
/// Infector's burst and the Plasma Whip's wave leave (0x3013B0; `effects.md`, "Rings"), a
/// crown of sixteen walls in a streaked white texture, tinted by its caller.
pub const UYA_EFFECT_CLASSES: &[u32] = &[0x307];

/// An effect's model read from the disc, waiting for its images.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscEffectModel {
    pub class: u32,
    pub surfaces: Vec<Surface>,
    pub paints: Vec<ImportPaint>,
}

/// Effects' models read from the disc.
#[derive(Resource, Debug, Default)]
pub struct DiscEffectModels(pub Vec<DiscEffectModel>);

/// Each of [`UYA_EFFECT_CLASSES`] from `core`.
pub fn import_effects(core: &Core) -> Vec<Result<DiscEffectModel, ImportError>> {
    UYA_EFFECT_CLASSES
        .iter()
        .map(|&class| {
            let (surfaces, paints) = read_class(core, class, 1.0)?;
            Ok(DiscEffectModel {
                class,
                surfaces,
                paints,
            })
        })
        .collect()
}

/// The Plasma Whip's links as its rope draws them (`weapons.md`, "Plasma Whip"): eight of
/// class 0x183A and the tip, 0x183C, each at [`ROPE_SCALE`] of its class's scale, its
/// length along the class's +x. Both take one strip of a 128-pixel texture of four, by
/// coordinates below 0, so their images repeat.
pub const UYA_ROPE_CLASSES: [u32; 2] = [0x183A, 0x183C];
/// The links' scale against their classes' (0x3572B0).
const ROPE_SCALE: f32 = 0.2;

/// The whip's links read from the disc, waiting for their images.
#[derive(Resource, Debug, Default)]
pub struct DiscRopeModels(pub Vec<DiscEffectModel>);

/// Each of [`UYA_ROPE_CLASSES`] from `core`.
pub fn import_rope(core: &Core) -> Vec<Result<DiscEffectModel, ImportError>> {
    UYA_ROPE_CLASSES
        .iter()
        .map(|&class| {
            let (surfaces, paints) = read_class(core, class, ROPE_SCALE)?;
            Ok(DiscEffectModel {
                class,
                surfaces,
                paints,
            })
        })
        .collect()
}

/// Class `class`'s mesh at rest in `core`, `scale` times its class's size, with its paints.
fn read_class(
    core: &Core,
    class: u32,
    scale: f32,
) -> Result<(Vec<Surface>, Vec<ImportPaint>), ImportError> {
    let moby = core.moby_class(class)?;
    let textures = core.moby_textures(class)?;
    let mesh = moby.mesh(PacketKind::High)?;
    let metal = moby.mesh(PacketKind::Metal)?;
    let mut builder = Builder::default();
    let unit = moby.unit() * scale;
    add_skinned(&mut builder, &mesh, &metal, &textures, unit, |_| 0);
    Ok(builder.finish())
}

/// A surface as a plain mesh: its skin isn't needed for a shot at rest.
fn plain_mesh(surface: &Surface) -> Mesh {
    let uvs = surface
        .uvs
        .clone()
        .unwrap_or_else(|| vec![Vec2::ZERO; surface.positions.len()]);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, surface.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(surface.indices.clone()))
}

/// Makes the meshes and materials of the shots read from the disc and gives them to the
/// kit, by each gun's key.
pub fn register_disc_shots(
    mut commands: Commands,
    pending: Option<ResMut<DiscShots>>,
    armory: Res<Armory>,
    mut models: ResMut<ShotModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let mut ready = 0;
    for shot in pending.0.drain(..) {
        let Some(def) = armory.weapons.iter().find(|def| def.item == shot.item) else {
            continue;
        };
        let parts = shot
            .surfaces
            .iter()
            .zip(shot.paints)
            .map(|(surface, paint)| {
                (
                    meshes.add(plain_mesh(surface)),
                    materials.add(material(paint, &mut images)),
                )
            })
            .collect();
        models.0.insert(def.effects.clone(), ShotModel { parts });
        ready += 1;
    }
    if ready > 0 {
        info!("the guns' shots' models from the disc: {ready}");
    }
    commands.remove_resource::<DiscShots>();
}

/// Gives the kit the effects' models read from the disc, by class, each surface with its
/// texture as the effects' own images are made (`fx_textures`).
pub fn register_disc_effect_models(
    mut commands: Commands,
    pending: Option<ResMut<DiscEffectModels>>,
    mut models: ResMut<FxModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let mut ready = 0;
    for model in pending.0.drain(..) {
        let parts = model
            .surfaces
            .iter()
            .zip(model.paints)
            .map(|(surface, paint)| {
                let image = paint
                    .texture
                    .map(|texture| images.add(crate::fx_textures::image(&texture)));
                (meshes.add(plain_mesh(surface)), image)
            })
            .collect();
        models.0.insert(model.class, FxModel { parts });
        ready += 1;
    }
    if ready > 0 {
        info!("the guns' effects' models from the disc: {ready}");
    }
    commands.remove_resource::<DiscEffectModels>();
}

/// Gives the rig the whip's links read from the disc, each surface with its texture
/// repeating.
pub fn register_disc_rope_models(
    mut commands: Commands,
    pending: Option<ResMut<DiscRopeModels>>,
    mut models: ResMut<RopeModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    for model in pending.0.drain(..) {
        let parts = model
            .surfaces
            .iter()
            .zip(model.paints)
            .map(|(surface, paint)| {
                let image = paint.texture.map(|texture| {
                    let mut image = crate::fx_textures::image(&texture);
                    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::Repeat,
                        ..ImageSamplerDescriptor::linear()
                    });
                    images.add(image)
                });
                (meshes.add(plain_mesh(surface)), image)
            })
            .collect();
        let rope = Some(RopeModel { parts });
        match model.class {
            0x183A => models.link = rope,
            0x183C => models.tip = rope,
            _ => {}
        }
    }
    if models.link.is_some() {
        info!("the whip's links from the disc");
    }
    commands.remove_resource::<DiscRopeModels>();
}

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;

    #[test]
    #[ignore = "needs game discs"]
    fn every_shot_class_has_a_textured_mesh_of_a_shots_size() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) = crate::discs::find(
            std::path::Path::new(&folder),
            crate::discs::Release::UyaNtsc,
        )
        .expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        for (shot, (item, class, _)) in import(&core).into_iter().zip(UYA_SHOT_CLASSES) {
            let shot = shot.unwrap_or_else(|error| panic!("{class:#x}: {error}"));
            assert_eq!(shot.item, *item);
            assert!(!shot.surfaces.is_empty(), "{class:#x}");
            assert!(
                shot.paints.iter().any(|paint| paint.texture.is_some()),
                "{class:#x}"
            );
            let size = shot
                .surfaces
                .iter()
                .flat_map(|surface| &surface.positions)
                .fold(0f32, |most, p| most.max(p.abs().max_element()));
            assert!((0.05..5.0).contains(&size), "{class:#x}: {size} units");
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn the_ring_is_a_textured_crown_about_as_wide_as_high() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) = crate::discs::find(
            std::path::Path::new(&folder),
            crate::discs::Release::UyaNtsc,
        )
        .expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let ring = import_effects(&core).remove(0).unwrap();
        assert_eq!(ring.class, 0x307);
        assert!(ring.paints.iter().all(|paint| paint.texture.is_some()));
        let points: Vec<Vec3> = ring
            .surfaces
            .iter()
            .flat_map(|surface| surface.positions.iter().copied())
            .collect();
        assert_eq!(points.len(), 64);
        // Standing up from about its foot, 0.37 high and 0.46 across at its widest.
        let high = points.iter().map(|point| point.y).fold(f32::MIN, f32::max);
        let low = points.iter().map(|point| point.y).fold(f32::MAX, f32::min);
        let wide = points
            .iter()
            .map(|point| point.with_y(0.0).length())
            .fold(0.0, f32::max);
        assert!(
            (high - 0.373).abs() < 0.01 && (low + 0.022).abs() < 0.01,
            "{low} {high}"
        );
        assert!((wide - 0.23).abs() < 0.01, "{wide}");
    }

    #[test]
    #[ignore = "needs game discs"]
    fn the_whips_links_are_short_bars_along_minus_z_leading_on_from_their_joint_0() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) = crate::discs::find(
            std::path::Path::new(&folder),
            crate::discs::Release::UyaNtsc,
        )
        .expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        for (model, (class, far)) in import_rope(&core)
            .into_iter()
            .zip([(0x183A, 0.196), (0x183C, 0.272)])
        {
            let model = model.unwrap_or_else(|error| panic!("{class:#x}: {error}"));
            assert!(model.paints.iter().all(|paint| paint.texture.is_some()));
            let points: Vec<Vec3> = model
                .surfaces
                .iter()
                .flat_map(|surface| surface.positions.iter().copied())
                .collect();
            // Its length along −Z, from about 0.1 behind where it sits.
            let ahead = points.iter().map(|point| -point.z).fold(f32::MIN, f32::max);
            let behind = points.iter().map(|point| -point.z).fold(f32::MAX, f32::min);
            assert!((ahead - far).abs() < 0.005, "{class:#x}: {ahead}");
            assert!((behind + 0.098).abs() < 0.005, "{class:#x}: {behind}");
        }
        // The rope's lead is the link's joint 0 at the rope's scale.
        let link = core.moby_class(0x183A).unwrap();
        let joint = link.joints[0].rest[0] * link.unit() * ROPE_SCALE;
        assert!((joint - rac_rig::LINK_LEAD).abs() < 0.001, "{joint}");
    }
}
