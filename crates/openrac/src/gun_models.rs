//! The guns' models from the user's UYA disc (PLAN.md P1.9), worn in the hero's fist by the
//! rig ([`rac_rig::RigGuns`]) in place of its open placeholders. Each is its class in the
//! gadget WAD (`moby-format.md`, "The gadget WAD"), read at start-up; nothing is stored.
//!
//! - UYA puts a held gun's class origin on the hero's joint 56, his right hand, with the
//!   joint's axes and the class's own scale (`hero_place_item_mobys`, 0x4E7FD8). The rig
//!   holds every gun by its grip, the same way in every model ([`rac_rig::model::gun_grip`]),
//!   so each class is moved once into the rig's gun space from where joint 56 sits at rest
//!   in UYA's hero. Worn by UYA's hero, a gun then sits where the game puts it; any other
//!   model holds it the same way in its own fist.
//! - A gun's class has a skeleton and three sequences of its own: 0 grows it from nothing
//!   as it comes out, 1 holds it at rest, and 2 is its recoil [data]. Which the game plays
//!   when isn't read, so here it is baked at rest and moves with the hand.
//! - Its model changes with its level ([`UYA_GUN_MODELS`]): from V2 its update shows more of
//!   its bangles, and from V5 its rows name another class.

use std::collections::BTreeMap;

use bevy::math::Affine3A;
use bevy::prelude::*;
use rac_formats::gadget::{Gadget, GadgetTable};
use rac_formats::level::{Core, Toc};
use rac_formats::moby::{MobyClass, PacketKind};
use rac_formats::{DiscReader, FormatError};
use rac_rig::{GunData, Paint, RigGuns, joint, model};

use crate::hero_anims::rest_orientation;
use crate::hero_model::{Builder, ImportError, ImportPaint, baked, chromed, material, textured};

/// One of a gun's models: the gun's item number, the level it shows from (0 for V1), its
/// class, and which of the class's bangles it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GunModel {
    pub item: u16,
    pub level: u8,
    pub class: u32,
    pub bangles: u16,
}

const fn at(item: u16, level: u8, class: u32, bangles: u16) -> GunModel {
    GunModel {
        item,
        level,
        class,
        bangles,
    }
}

/// UYA's guns, each level's model in order (`weapons.md`, "Each gun"):
/// - the class is row +0x10 of the level's row in Veldin's weapon table [data];
/// - the bangles are the mask each gun's update writes to its moby's +0x7C by level
///   [code]. That +0x7C is the moby's bangle mask is inferred: the masks fill exactly the
///   bangles each class has (2 for the N60, 3 for most, slots 0, 1 and 4 for the Suck
///   Cannon), and the V5 classes have none. The Shock Blaster never sets its third. The
///   Lava Gun, Plasma Coil, Bouncer, Miniturret Glove, Agents of Doom and Plasma Whip
///   have none and set none.
/// - The Shield Charger's row names the shield round him (0xA5C), not something in his
///   hand, so it has no model here: he holds nothing ([`rac_rig::open_guns`]).
pub const UYA_GUN_MODELS: &[GunModel] = &[
    // Shock Blaster (0x36A790).
    at(0x27, 0, 0x1863, 0),
    at(0x27, 1, 0x1863, 0b10),
    at(0x27, 2, 0x1863, 0b11),
    at(0x27, 4, 0x1882, 0),
    // N60 Storm (0x3A5928).
    at(0x2F, 0, 0x18BE, 0),
    at(0x2F, 1, 0x18BE, 0b1),
    at(0x2F, 2, 0x18BE, 0b11),
    at(0x2F, 4, 0x184F, 0),
    // Annihilator (0x36D5A0).
    at(0x3F, 0, 0x1864, 0),
    at(0x3F, 1, 0x1864, 0b1),
    at(0x3F, 2, 0x1864, 0b11),
    at(0x3F, 3, 0x1864, 0b111),
    at(0x3F, 4, 0x1887, 0),
    // Flux Rifle (0x3444B0), whose class stays the same at every level.
    at(0x6F, 0, 0xC04, 0),
    at(0x6F, 1, 0xC04, 0b1),
    at(0x6F, 2, 0xC04, 0b11),
    at(0x6F, 3, 0xC04, 0b111),
    // Nitro Launcher (0x36E360).
    at(0x77, 0, 0x1865, 0),
    at(0x77, 1, 0x1865, 0b1),
    at(0x77, 2, 0x1865, 0b11),
    at(0x77, 3, 0x1865, 0b111),
    at(0x77, 4, 0x1885, 0),
    // Lava Gun.
    at(0x11, 0, 0x5C9, 0),
    at(0x11, 4, 0xC59, 0),
    // Infector (0x3688E8).
    at(0x37, 0, 0x1861, 0),
    at(0x37, 1, 0x1861, 0b1),
    at(0x37, 2, 0x1861, 0b11),
    at(0x37, 3, 0x1861, 0b111),
    at(0x37, 4, 0x1886, 0),
    // Spitting Hydra (0x31FB90).
    at(0x47, 0, 0x184E, 0),
    at(0x47, 1, 0x184E, 0b1),
    at(0x47, 2, 0x184E, 0b11),
    at(0x47, 3, 0x184E, 0b111),
    at(0x47, 4, 0x1B4F, 0),
    // Disc Blade Gun (0x36EFC0).
    at(0x4F, 0, 0x1866, 0),
    at(0x4F, 1, 0x1866, 0b1),
    at(0x4F, 2, 0x1866, 0b11),
    at(0x4F, 3, 0x1866, 0b111),
    at(0x4F, 4, 0x1C44, 0),
    // Agents of Doom's glove (0x35C9F0), the same at every level.
    at(0x57, 0, 0x1852, 0),
    // Rift Inducer (0x372990).
    at(0x5F, 0, 0x1C67, 0),
    at(0x5F, 1, 0x1C67, 0b1),
    at(0x5F, 2, 0x1C67, 0b11),
    at(0x5F, 3, 0x1C67, 0b111),
    at(0x5F, 4, 0x186A, 0),
    // Holo Shield Glove (0x3717A8); its V5 class is in the level's core, not the WAD.
    at(0x67, 0, 0x1868, 0),
    at(0x67, 1, 0x1868, 0b1),
    at(0x67, 2, 0x1868, 0b11),
    at(0x67, 3, 0x1868, 0b111),
    at(0x67, 4, 0x1C65, 0),
    // Plasma Whip's handle (0x356110).
    at(0x7F, 0, 0x1843, 0),
    // Suck Cannon (0x3619C8).
    at(0x87, 0, 0x185B, 0),
    at(0x87, 1, 0x185B, 0b1),
    at(0x87, 2, 0x185B, 0b11),
    at(0x87, 3, 0x185B, 0b1_0011),
    at(0x87, 4, 0x1892, 0),
    // Qwack-O-Ray (0x37DEB0).
    at(0x8F, 0, 0x1870, 0),
    at(0x8F, 1, 0x1870, 0b1),
    at(0x8F, 2, 0x1870, 0b11),
    at(0x8F, 3, 0x1870, 0b111),
    at(0x8F, 4, 0x1C63, 0),
    // RY3NO (0x3805B0), whose V2 shows the third bangle first.
    at(0x97, 0, 0x1871, 0),
    at(0x97, 1, 0x1871, 0b100),
    at(0x97, 2, 0x1871, 0b110),
    at(0x97, 3, 0x1871, 0b111),
    at(0x97, 4, 0x1DE1, 0),
    // Plasma Coil (0x315638).
    at(0x10, 0, 0xC4A, 0),
    at(0x10, 4, 0x5CA, 0),
    // Bouncer (0x31C890).
    at(0x13, 0, 0xE14, 0),
    at(0x13, 4, 0xE9E, 0),
    // Miniturret Glove (0x346078).
    at(0x15, 0, 0xC5A, 0),
    at(0x15, 4, 0xA65, 0),
];

/// The gloves, worn on the hand rather than held by a grip, which the disc test bounds
/// apart.
#[cfg(test)]
const GLOVES: &[u16] = &[0x57, 0x67, 0x15];

/// The pose a gun is baked in: its sequence 1, at rest, or for a class with one sequence
/// (the gloves and the whip) that one's.
fn gun_pose(class: &MobyClass) -> (usize, usize) {
    (usize::from(class.sequences.len() > 1), 0)
}

/// A gun's model read from the disc. Its surfaces' paints are placeholders until
/// [`register_disc_guns`] makes the materials `paints` describe, one per surface in order.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscGun {
    pub data: GunData,
    pub paints: Vec<ImportPaint>,
}

/// Guns read from the disc, waiting for their materials.
#[derive(Resource, Debug, Default)]
pub struct DiscGuns(pub Vec<DiscGun>);

/// A class as a gadget: from the gadget WAD, or failing that from `core`, which has the
/// few the WAD lacks (the Holo Shield's V5 glove).
fn read_class(
    disc: &mut impl DiscReader,
    table: &GadgetTable,
    core: &Core,
    class: u32,
) -> Result<Gadget, String> {
    match table.find(class) {
        Some(index) => Gadget::read_from(disc, table, index).map_err(|e| e.to_string()),
        None => (|| {
            Ok::<_, FormatError>(Gadget {
                class: core.moby_class(class)?,
                textures: core.moby_textures(class)?,
                bangles: core.moby_bangles(class)?,
            })
        })()
        .map_err(|e| format!("neither the gadget WAD nor the core has it: {e}")),
    }
}

/// Each of [`UYA_GUN_MODELS`] from a UYA disc, held as `hero` (Veldin's, from `core`)
/// holds a gun, each class read once. A model that can't be read is an error in its place.
pub fn import(
    disc: &mut impl DiscReader,
    core: &Core,
    hero: &MobyClass,
) -> Result<Vec<Result<DiscGun, ImportError>>, ImportError> {
    let table = GadgetTable::read_uya(&Toc::read_uya_disc(disc)?)?;
    let into_grip = into_grip(hero)?;
    let mut classes: BTreeMap<u32, Result<Gadget, String>> = BTreeMap::new();
    Ok(UYA_GUN_MODELS
        .iter()
        .map(|model| {
            let gadget = classes
                .entry(model.class)
                .or_insert_with(|| read_class(disc, &table, core, model.class));
            match gadget {
                Ok(gadget) => gun(gadget, model, into_grip),
                Err(detail) => Err(FormatError::Invalid {
                    what: "gadget",
                    address: model.class,
                    detail: detail.clone(),
                }
                .into()),
            }
        })
        .collect())
}

/// What takes a gun's class, placed in the hero's space as the game places it on his hand
/// at rest (origin on the hand, the hand's axes), to the rig's gun space: the grip at the
/// origin, the barrel along −Z and the top along +Y, as `model::gun_grip` turns into the
/// fist from the hand at rest.
fn into_grip(hero: &MobyClass) -> Result<Affine3A, ImportError> {
    let hand = hero
        .joints
        .get(joint::HAND_R)
        .ok_or_else(|| FormatError::Invalid {
            what: "hero",
            address: joint::HAND_R as u32,
            detail: format!("the hero has {} joints", hero.joints.len()),
        })?;
    Ok(Affine3A::from_quat(model::gun_grip().inverse())
        * Affine3A::from_translation(-model::GRIP_FROM_HAND)
        * Affine3A::from_quat(rest_orientation(hand)))
}

/// One model: its class's mesh and the bangles it shows, baked in [`gun_pose`] in the rig's
/// gun space.
fn gun(gadget: &Gadget, model: &GunModel, into_grip: Affine3A) -> Result<DiscGun, ImportError> {
    let class = &gadget.class;
    let metal = class.mesh(PacketKind::Metal)?;
    let shown = gadget
        .bangles
        .iter()
        .filter(|bangle| bangle.slot < 16 && model.bangles & (1 << bangle.slot) != 0)
        .map(|bangle| bangle.mesh(PacketKind::High));
    let mut builder = Builder::default();
    for mesh in std::iter::once(class.mesh(PacketKind::High)).chain(shown) {
        let mesh = mesh?;
        let chrome = chromed(&mesh, &metal);
        let (mut vertices, _) = baked(class, &mesh, gun_pose(class), "a gun", Vec3::ZERO, |_| 0)?;
        for vertex in &mut vertices {
            vertex.position = into_grip.transform_point3(vertex.position);
            vertex.normal = into_grip.transform_vector3(vertex.normal);
        }
        builder.add(&mesh, &vertices, &chrome, false, |t, c| {
            textured(&gadget.textures, t, c)
        });
    }
    let (surfaces, paints) = builder.finish();
    Ok(DiscGun {
        data: GunData {
            item: Some(model.item),
            level: model.level,
            surfaces,
        },
        paints,
    })
}

/// Makes the materials of the guns read from the disc and gives them to the rig.
pub fn register_disc_guns(
    mut commands: Commands,
    pending: Option<ResMut<DiscGuns>>,
    mut guns: ResMut<RigGuns>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let count = pending.0.len();
    for DiscGun { mut data, paints } in pending.0.drain(..) {
        for (surface, paint) in data.surfaces.iter_mut().zip(paints) {
            surface.paint = Paint::Own(materials.add(material(paint, &mut images)));
        }
        guns.add(data, &mut meshes);
    }
    if count > 0 {
        info!("the guns' models from the disc are ready: {count}");
    }
    commands.remove_resource::<DiscGuns>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_gun_lists_its_models_from_v1_up_with_more_bangles_each_level() {
        let mut items: Vec<u16> = UYA_GUN_MODELS.iter().map(|m| m.item).collect();
        items.dedup();
        assert_eq!(
            items,
            [
                0x27, 0x2F, 0x3F, 0x6F, 0x77, 0x11, 0x37, 0x47, 0x4F, 0x57, 0x5F, 0x67, 0x7F, 0x87,
                0x8F, 0x97, 0x10, 0x13, 0x15
            ]
        );
        for item in items {
            let models: Vec<&GunModel> = UYA_GUN_MODELS.iter().filter(|m| m.item == item).collect();
            assert_eq!(models[0].level, 0, "{item:#x}");
            assert_eq!(models[0].bangles, 0, "{item:#x}: V1 shows none");
            for pair in models.windows(2) {
                assert!(pair[0].level < pair[1].level, "{item:#x}");
                // A class keeps what it showed and adds more; a new class starts afresh.
                if pair[0].class == pair[1].class {
                    assert_eq!(pair[1].bangles & pair[0].bangles, pair[0].bangles);
                } else {
                    assert_eq!(pair[1].level, 4, "{item:#x}: the V5 class");
                }
            }
        }
    }
}

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use crate::discs::{Release, find};

    #[test]
    #[ignore = "needs game discs"]
    fn uyas_guns_sit_in_the_fist_barrel_ahead() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let core = rac_formats::level::Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let hero = core.hero().unwrap();
        let guns = import(&mut disc, &core, &hero).unwrap();
        assert_eq!(guns.len(), UYA_GUN_MODELS.len());
        let mut previous: Option<(GunModel, usize)> = None;
        for (gun, model) in guns.into_iter().zip(UYA_GUN_MODELS) {
            let name = format!("{:#x} from V{}", model.item, model.level + 1);
            let gun = gun.unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(gun.data.item, Some(model.item));
            assert_eq!(gun.data.level, model.level);
            assert_eq!(gun.paints.len(), gun.data.surfaces.len());
            assert!(gun.paints.iter().all(|p| p.texture.is_some()), "{name}");
            let points: Vec<Vec3> = gun
                .data
                .surfaces
                .iter()
                .flat_map(|s| s.positions.iter().copied())
                .collect();
            let (low, high) = points
                .iter()
                .fold((Vec3::MAX, Vec3::MIN), |(l, h), p| (l.min(*p), h.max(*p)));
            println!("{name}: {} vertices, {low:.2} to {high:.2}", points.len());
            if GLOVES.contains(&model.item) {
                // A glove is worn round the fist.
                assert!(low.z < 0.0 && high.z > -0.1, "{name}: {low} to {high}");
                assert!(
                    low.length() < 0.6 && high.length() < 0.6,
                    "{name}: {low} to {high}"
                );
            } else if model.item == 0x7F {
                // The whip's handle is in the fist.
                assert!(low.z < -0.05 && high.z < 0.3, "{name}: {low} to {high}");
            } else {
                // Held as the game holds it, the barrel reaches well ahead of the grip and
                // less behind it (the RY3NO rests on his shoulder), and the gun sits on the
                // fist.
                assert!(low.z < -0.5 && high.z < 0.45, "{name}: {low} to {high}");
                assert!(high.y > 0.15 && low.y > -0.4, "{name}: {low} to {high}");
            }
            // A level that shows more bangles has more to it.
            let triangles = gun.data.surfaces.iter().map(|s| s.indices.len()).sum();
            if let Some((before, count)) = previous
                && before.class == model.class
            {
                assert!(triangles > count, "{name}: {triangles} after {count}");
            }
            previous = Some((*model, triangles));
        }
    }
}
