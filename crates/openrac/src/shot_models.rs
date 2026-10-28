//! The guns' shots' models from the user's UYA disc (PLAN.md P3.18): the shot classes with
//! a mesh in Veldin's core (`docs/research/effects.md`, "Shot classes on the disc"), drawn
//! by the kit in place of its shapes ([`ShotModels`]). Each is its class's mesh at rest,
//! textured from the level's textures; nothing is stored.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::Armory;
use rac_demo::{ShotModel, ShotModels};
use rac_formats::level::Core;
use rac_formats::moby::PacketKind;
use rac_rig::Surface;

use crate::hero_model::{Builder, ImportError, ImportPaint, add_skinned, material};

/// Each gun's shot class by its item, from its code (`effects.md`, "Each gun's effects")
/// [code]: the Nitro Launcher's charge, the Annihilator's missile, the Disc Blade, the
/// Bouncer's canister, the Miniturret and the RY3NO's rocket; each looked right in flight
/// in the app. Their V5 shots' classes aren't drawn. The Plasma Coil's ball (0xA9B,
/// untextured) and the rift (0x186B, 0.05 units across) are drawn by their code, and the
/// Infector's 0x1E05 is a mound of goo 1.4 units high, not its shot; those keep their
/// sprites. Each is drawn at its class's scale times the third number, as its gun's code
/// sets it: the Bouncer's canister at three times (0x1DC8BC [data]).
pub const UYA_SHOT_CLASSES: &[(u16, u32, f32)] = &[
    (0x77, 0x185A, 1.0),
    (0x3F, 0x1894, 1.0),
    (0x4F, 0x1867, 1.0),
    (0x13, 0xE64, 3.0),
    (0x15, 0x1366, 1.0),
    (0x97, 0x1873, 1.0),
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
            let moby = core.moby_class(class)?;
            let textures = core.moby_textures(class)?;
            let mesh = moby.mesh(PacketKind::High)?;
            let metal = moby.mesh(PacketKind::Metal)?;
            let mut builder = Builder::default();
            let unit = moby.unit() * scale;
            add_skinned(&mut builder, &mesh, &metal, &textures, unit, |_| 0);
            let (surfaces, paints) = builder.finish();
            Ok(DiscShot {
                item,
                surfaces,
                paints,
            })
        })
        .collect()
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
}
