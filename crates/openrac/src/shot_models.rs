//! The guns' shots' models from the user's UYA disc (PLAN.md P3.18): the shot classes with
//! a mesh in Veldin's core (`docs/research/effects.md`, "Shot classes on the disc"), drawn
//! by the kit in place of its shapes ([`ShotModels`]), the classes their effects spawn
//! ([`FxModels`]) and the Plasma Whip's links ([`RopeModels`]). Each is its class's mesh at
//! rest, textured from the level's textures; nothing is stored.

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::Armory;
use rac_demo::fx::{FxModel, FxModels};
use rac_demo::{ShotModel, ShotModels, ShotSkeleton};
use rac_formats::level::Core;
use rac_formats::moby::PacketKind;
use rac_rig::{MobySkeleton, RigClip, RopeModel, RopeModels, Surface};

use crate::hero_anims::{binds, part_clips};
use crate::hero_model::{Builder, ImportError, ImportPaint, add_skinned, material};

/// What a gun sends out, as a class in Veldin's core: the gun's item, the kind of shot
/// ([`rac_combat::weapons::ShotKind::name`]), the level it shows from (0 for V1), the class,
/// its scale of the class's own as the gun's code sets it, the bangles it shows, the joint
/// that aims it, and the glow it adds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotClass {
    pub item: u16,
    pub kind: &'static str,
    pub level: u8,
    pub class: u32,
    pub scale: f32,
    pub bangles: u16,
    pub aim_joint: Option<usize>,
    pub glow: Option<[u8; 3]>,
}

const fn shot(item: u16, kind: &'static str, level: u8, class: u32, scale: f32) -> ShotClass {
    ShotClass {
        item,
        kind,
        level,
        class,
        scale,
        bangles: 0,
        aim_joint: None,
        glow: None,
    }
}

const fn showing(row: ShotClass, bangles: u16) -> ShotClass {
    ShotClass { bangles, ..row }
}

/// What each gun sends out, from its code (`effects.md`, "Shot classes on the disc";
/// `weapons.md`, "How they play") [code]. The Plasma Coil's ball (0xA9B, untextured), the
/// rift (0x186B, 0.05 units across) and the comet (0x1893, at 0.05) are drawn by their
/// code; those keep their effects, as do the V5 shots of the Shock Blaster's and Nitro
/// Launcher's kind, whose classes aren't read yet.
pub const UYA_SHOT_CLASSES: &[ShotClass] = &[
    shot(0x77, "charge", 0, 0x185A, 1.0),
    shot(0x3F, "missile", 0, 0x1894, 1.0),
    // Its mines are the Bouncer's bomblets, V4's heavy (0x31E2D8's last argument).
    shot(0x3F, "bomblet", 0, 0xECE, 0.75),
    shot(0x3F, "bomblet", 3, 0xECE, 1.5),
    shot(0x4F, "disc", 0, 0x1867, 1.0),
    // 0x1DC8BC and 0x1DC8C8; 0x1DC8F4 and 0x1DC8F8 [data].
    shot(0x13, "canister", 0, 0xE64, 3.0),
    shot(0x13, "canister", 4, 0xE64, 5.0),
    shot(0x13, "bomblet", 0, 0xECE, 0.75),
    shot(0x13, "bomblet", 4, 0xECE, 1.5),
    // At 0.75 (0x32C320); the Megaturret at its class's (0x32F3D8).
    ShotClass {
        aim_joint: Some(1),
        ..shot(0x15, "turret", 0, 0x1366, 0.75)
    },
    ShotClass {
        aim_joint: Some(1),
        ..shot(0x15, "turret", 4, 0x1367, 1.0)
    },
    shot(0x97, "rocket", 0, 0x1873, 1.0),
    // A random 0.25 to 0.3 (0x3DE3A8), a mound of goo that tumbles there and here faces
    // its way.
    shot(0x37, "blob", 0, 0x1E05, 0.275),
    // The pod at 1.2 (0x35B240); an agent's bangles by level (0x3C0F78).
    shot(0x57, "pod", 0, 0x1853, 1.2),
    showing(shot(0x57, "agent", 0, 0x1ADA, 1.0), 0b0001),
    showing(shot(0x57, "agent", 1, 0x1ADA, 1.0), 0b1001),
    showing(shot(0x57, "agent", 4, 0x1ADA, 1.0), 0b0101),
    // The duck at 0.9, showing its bangles from V3 (0x3D2480); V4's another class with a
    // red glow (0x39C2F8's 0x438908 call, the moby's +0x3C); V5's at 0.7 (0x3D6578).
    shot(0x8F, "duck", 0, 0x1BC3, 0.9),
    showing(shot(0x8F, "duck", 2, 0x1BC3, 0.9), 0b11),
    ShotClass {
        glow: Some([0x40, 0x10, 0x10]),
        ..shot(0x8F, "duck", 3, 0x189B, 1.0)
    },
    shot(0x8F, "duck", 4, 0x1D3E, 0.7),
    shot(0x8F, "egg", 0, 0x1C59, 0.25),
];

/// A shot's skeleton read from the disc.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscSkeleton {
    pub parents: Vec<Option<usize>>,
    pub binds: Vec<Mat4>,
    pub clips: BTreeMap<u8, RigClip>,
}

/// A shot's model read from the disc, waiting for its materials.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscShot {
    pub row: ShotClass,
    pub surfaces: Vec<Surface>,
    pub paints: Vec<ImportPaint>,
    pub skeleton: Option<DiscSkeleton>,
}

/// Shot models read from the disc.
#[derive(Resource, Debug, Default)]
pub struct DiscShots(pub Vec<DiscShot>);

/// Each of [`UYA_SHOT_CLASSES`] from `core`.
pub fn import(core: &Core) -> Vec<Result<DiscShot, ImportError>> {
    UYA_SHOT_CLASSES
        .iter()
        .map(|row| read_shot(core, row))
        .collect()
}

/// A shot's class in `core`. One with a skeleton is skinned to it, with the bangles it
/// shows; one with none has its mesh at rest, at its scale.
fn read_shot(core: &Core, row: &ShotClass) -> Result<DiscShot, ImportError> {
    let moby = core.moby_class(row.class)?;
    if moby.joints.is_empty() {
        let (surfaces, paints) = read_class(core, row.class, row.scale)?;
        return Ok(DiscShot {
            row: *row,
            surfaces,
            paints,
            skeleton: None,
        });
    }
    let textures = core.moby_textures(row.class)?;
    let metal = moby.mesh(PacketKind::Metal)?;
    let bangles = core.moby_bangles(row.class)?;
    let shown = bangles
        .iter()
        .filter(|bangle| bangle.slot < 16 && row.bangles & (1 << bangle.slot) != 0)
        .map(|bangle| bangle.mesh(PacketKind::High));
    let mut builder = Builder::default();
    for mesh in std::iter::once(moby.mesh(PacketKind::High)).chain(shown) {
        add_skinned(
            &mut builder,
            &mesh?,
            &metal,
            &textures,
            moby.unit(),
            usize::from,
        );
    }
    let (surfaces, paints) = builder.finish();
    Ok(DiscShot {
        row: *row,
        surfaces,
        paints,
        skeleton: Some(DiscSkeleton {
            parents: moby.joints.iter().map(|joint| joint.parent).collect(),
            binds: binds(&moby),
            clips: part_clips(&moby),
        }),
    })
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
/// kit, by each gun's key, the shot's kind and the level it shows from.
pub fn register_disc_shots(
    mut commands: Commands,
    pending: Option<ResMut<DiscShots>>,
    armory: Res<Armory>,
    mut models: ResMut<ShotModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let mut ready = 0;
    for shot in pending.0.drain(..) {
        let row = shot.row;
        let Some(def) = armory.weapons.iter().find(|def| def.item == row.item) else {
            continue;
        };
        let parts = shot
            .surfaces
            .iter()
            .zip(shot.paints)
            .map(|(surface, paint)| {
                let mut paint = material(paint, &mut images);
                if let Some([r, g, b]) = row.glow {
                    paint.emissive = Color::srgb_u8(r, g, b).into();
                }
                let mesh = if shot.skeleton.is_some() {
                    surface.skinned_mesh()
                } else {
                    plain_mesh(surface)
                };
                (meshes.add(mesh), materials.add(paint))
            })
            .collect();
        let skeleton = shot.skeleton.map(|skeleton| ShotSkeleton {
            moby: Arc::new(MobySkeleton::new(
                skeleton.parents,
                &skeleton.binds,
                skeleton.clips,
                &mut bindposes,
            )),
            scale: row.scale,
            aim_joint: row.aim_joint,
        });
        models.0.insert(
            (def.effects.clone(), row.kind.to_string(), row.level),
            ShotModel { parts, skeleton },
        );
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
        for (shot, row) in import(&core).into_iter().zip(UYA_SHOT_CLASSES) {
            let class = row.class;
            let shot = shot.unwrap_or_else(|error| panic!("{class:#x}: {error}"));
            assert_eq!(shot.row, *row);
            assert!(!shot.surfaces.is_empty(), "{class:#x}");
            assert!(
                shot.paints.iter().any(|paint| paint.texture.is_some()),
                "{class:#x}"
            );
            let scale = if shot.skeleton.is_some() {
                row.scale
            } else {
                1.0
            };
            let size = shot
                .surfaces
                .iter()
                .flat_map(|surface| &surface.positions)
                .fold(0f32, |most, p| most.max(p.abs().max_element()))
                * scale;
            println!("{:#x} {} {class:#x}: {size} units", row.item, row.kind);
            assert!((0.05..5.0).contains(&size), "{class:#x}: {size} units");
        }
    }

    /// The sequences [`rac_combat::weapons::ShotKind::pose`] asks of each skeleton.
    #[test]
    #[ignore = "needs game discs"]
    fn the_skeletons_have_the_sequences_their_shots_play() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) = crate::discs::find(
            std::path::Path::new(&folder),
            crate::discs::Release::UyaNtsc,
        )
        .expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        for (class, sequences) in [
            (0x1ADA, &[1u8, 2, 3, 4, 5, 9][..]),
            (0x1BC3, &[0, 1, 2, 3, 4]),
            (0x189B, &[0, 1, 2, 3, 4]),
            (0x1D3E, &[2, 3, 5]),
            (0x1366, &[0, 1, 2]),
            (0x1367, &[0, 1, 2]),
        ] {
            let clips = part_clips(&core.moby_class(class).unwrap());
            for sequence in sequences {
                assert!(clips.contains_key(sequence), "{class:#x}: {sequence}");
            }
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
