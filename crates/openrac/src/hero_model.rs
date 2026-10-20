//! The hero's models from the user's own discs (PLAN.md §5, 2026-10-13), skinned to the
//! rig's skeleton so the same poses drive them as drive the open model. Read at start-up;
//! nothing is stored.
//! - From UYA: Ratchet in each of his suits ([`SUITS`]) with its textures, and the wrench,
//!   Clank and the heli-pack from Veldin's core; and the multiplayer skins ([`MP_SKINS`]),
//!   each a class of its own on the hero's skeleton, holding the wrench.
//! - From DL or Gladiator: its skins ([`DL_SUITS`]), each a class with a skeleton of its
//!   own, lined up with the rig's by [`DL_SOURCES`]. They hold the open model's wrench
//!   until DL's own is read.
//!
//! - Ratchet's vertices follow UYA's joints, folded onto the rig's by
//!   [`hero_anims::skeleton_map`]: a finger follows the hand, an eyelid the head.
//! - The wrench, Clank and the heli-pack are mobys of their own, which the game puts on
//!   his joints 56 and 5 with those joints' orientation (`moby-format.md`, "Finding the
//!   hero"). The wrench is baked in the pose the game shows while he stands, then follows
//!   the rig's wrench joint. Clank and the heli-pack, which holds Clank's arms and the
//!   rotors, are attachments on their own skeletons, which play their own sequences
//!   (`hero_anims::back_animations`). The wrench's mesh and texture and Clank's texture
//!   come from the armor WAD like Ratchet's; the rest from their classes in the core, the
//!   heli-pack's textures from the level's.
//! - The game draws both sides of every triangle and its strips don't keep a winding, so
//!   each triangle is turned to face the way its vertex normals do.
//! - Metal packets redraw some triangles with the level's chrome map
//!   (`moby-format.md`, "Metal packets"); here those triangles are polished metal instead,
//!   which the scene's own surroundings reflect.

use std::collections::HashSet;

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::{Vec2, Vec3};
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rac_formats::armor::{self, Armor, ArmorTable, Skin as ClassSkin, SkinTable};
use rac_formats::level::Core;
use rac_formats::moby::{
    Mesh as MobyMesh, MeshTriangle, MeshVertex, MobyClass, PacketKind, Skin, normal,
};
use rac_formats::texture::Texture;
use rac_formats::{DiscReader, FormatError};
use rac_rig::{AttachmentData, ModelData, Paint, RigModels, SkeletonMap, Surface, joint, model};

use crate::hero_anims::{
    BACK_PARTS, binds, joint_positions, rest_orientation, skeleton_map, skinning, to_bevy,
};

const WRENCH_CLASS: u32 = 0x47;
/// The hero joint the game puts the wrench on.
const WRENCH_ON: usize = 56;
/// The rig joint Clank and the heli-pack ride on: UYA's hero joint 5, his upper back.
const BACK_ON: usize = joint::CHEST;
/// The sequence and keyframe the wrench is shown in while Ratchet stands, as in our
/// Veldin memory image.
const WRENCH_POSE: (usize, usize) = (1, 0);
/// The core's class table marks the class that wears Clank's texture with this
/// (`level_load_core`): Clank's own. Other classes wear the level's textures.
const CLANK_TEXTURE_GROUP: u32 = 2;
/// What a triangle with no texture is drawn in (linear RGB).
const UNTEXTURED: [f32; 3] = [0.6, 0.62, 0.66];

/// What an imported surface is drawn in.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPaint {
    /// Its texture, or none for the plain, slightly metallic [`UNTEXTURED`].
    pub texture: Option<Texture>,
    /// Polished metal: the game draws its chrome map over these triangles.
    pub chrome: bool,
}

/// A model read from the disc. Its surfaces' paints are placeholders until
/// [`register_disc_models`] makes the materials `paints` describe, one per surface in
/// order; surfaces after the last paint keep their own (a wrench lent by the open model).
/// Each attachment's surfaces take theirs from `attachment_paints` the same way.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub data: ModelData,
    pub paints: Vec<ImportPaint>,
    pub attachment_paints: Vec<Vec<ImportPaint>>,
}

/// Models read from the disc, waiting for their materials.
#[derive(Resource, Debug, Default)]
pub struct DiscModels(pub Vec<Imported>);

/// Why the disc's model couldn't be built.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error("the skeleton doesn't line up with the rig's: {0}")]
    LineUp(#[from] rac_rig::LineUpError),
    #[error("{0} has no sequence {1}, keyframe {2}")]
    Pose(&'static str, usize, usize),
}

/// An armor worth wearing: its entry in the armor WAD, and its model's name and title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suit {
    pub entry: usize,
    pub name: &'static str,
    pub title: &'static str,
}

/// The suits offered as models: every distinct mesh entry of UYA's armor WAD, the armor a
/// new game starts in first (`moby-format.md`, "The armor WAD"). Entries 15–28 repeat 14,
/// and 29–34 are the wrench and two like it. The titles are the game's own names for its
/// armors and skins ("Armor numbers and names"); entry 14 has none, and its title says how
/// it looks.
pub const SUITS: &[Suit] = &[
    Suit {
        entry: 0,
        name: "uya",
        title: "Alpha Combat Suit",
    },
    Suit {
        entry: 1,
        name: "uya-blue",
        title: "Magnaplate Armor",
    },
    Suit {
        entry: 2,
        name: "uya-rust",
        title: "Adamantine Armor",
    },
    Suit {
        entry: 3,
        name: "uya-green",
        title: "Aegis Mark V Armor",
    },
    Suit {
        entry: 4,
        name: "uya-flame",
        title: "Infernox Armor",
    },
    Suit {
        entry: 5,
        name: "uya-casual",
        title: "Old School Ratchet",
    },
    Suit {
        entry: 6,
        name: "uya-snowman",
        title: "Snowman",
    },
    Suit {
        entry: 7,
        name: "uya-tuxedo",
        title: "Tuxedo Ratchet",
    },
    Suit {
        entry: 8,
        name: "uya-bug",
        title: "Buginoid",
    },
    Suit {
        entry: 9,
        name: "uya-dome",
        title: "Brainius",
    },
    Suit {
        entry: 10,
        name: "uya-brown-robot",
        title: "Constructobot",
    },
    Suit {
        entry: 11,
        name: "uya-goggles",
        title: "Robo Rooster",
    },
    Suit {
        entry: 12,
        name: "uya-red-robot",
        title: "Trooper",
    },
    Suit {
        entry: 13,
        name: "uya-mohawk",
        title: "Robo",
    },
    Suit {
        entry: 14,
        name: "uya-orange",
        title: "Orange armor",
    },
];

/// UYA's multiplayer skins offered as models, entries 35–55 of the armor WAD
/// (`moby-format.md`, "Multiplayer skins"), with the game's multiplayer names. Several
/// share a name with a single-player suit, so each title says it's the multiplayer one.
pub const MP_SKINS: &[Suit] = &[
    Suit {
        entry: 35,
        name: "uya-mp-robo",
        title: "MP Robo",
    },
    Suit {
        entry: 36,
        name: "uya-mp-thug",
        title: "MP Thug",
    },
    Suit {
        entry: 37,
        name: "uya-mp-tyhrranoid",
        title: "MP Tyhrranoid",
    },
    Suit {
        entry: 38,
        name: "uya-mp-blarg",
        title: "MP Blarg",
    },
    Suit {
        entry: 39,
        name: "uya-mp-ninja",
        title: "MP Ninja",
    },
    Suit {
        entry: 40,
        name: "uya-mp-snow-man",
        title: "MP Snow Man",
    },
    Suit {
        entry: 41,
        name: "uya-mp-bruiser",
        title: "MP Bruiser",
    },
    Suit {
        entry: 42,
        name: "uya-mp-gray",
        title: "MP Gray",
    },
    Suit {
        entry: 43,
        name: "uya-mp-hotbot",
        title: "MP Hotbot",
    },
    Suit {
        entry: 44,
        name: "uya-mp-gladiola",
        title: "MP Gladiola",
    },
    Suit {
        entry: 45,
        name: "uya-mp-evil-clown",
        title: "MP Evil Clown",
    },
    Suit {
        entry: 46,
        name: "uya-mp-beach-bunny",
        title: "MP Beach Bunny",
    },
    Suit {
        entry: 47,
        name: "uya-mp-robo-rooster",
        title: "MP Robo Rooster",
    },
    Suit {
        entry: 48,
        name: "uya-mp-buginoid",
        title: "MP Buginoid",
    },
    Suit {
        entry: 49,
        name: "uya-mp-brainius",
        title: "MP Brainius",
    },
    Suit {
        entry: 50,
        name: "uya-mp-skrunch",
        title: "MP Skrunch",
    },
    Suit {
        entry: 51,
        name: "uya-mp-bones",
        title: "MP Bones",
    },
    Suit {
        entry: 52,
        name: "uya-mp-nefarious",
        title: "MP Nefarious",
    },
    Suit {
        entry: 53,
        name: "uya-mp-trooper",
        title: "MP Trooper",
    },
    Suit {
        entry: 54,
        name: "uya-mp-constructobot",
        title: "MP Constructobot",
    },
    Suit {
        entry: 55,
        name: "uya-mp-dan",
        title: "MP Dan",
    },
];

/// Each of `suits` on the rig's skeleton, with the wrench from `core` (Veldin's): Ratchet
/// in an armor ([`SUITS`]) with Clank and the heli-pack on his back, or a multiplayer skin
/// ([`MP_SKINS`]) without them, as multiplayer has it. What they share is read once for
/// all of them; a suit that can't be read is an error in its place.
pub fn import(
    disc: &mut impl DiscReader,
    core: &Core,
    hero: &MobyClass,
    suits: &[Suit],
) -> Result<Vec<Result<Imported, ImportError>>, ImportError> {
    let map = skeleton_map(hero)?;
    let table = ArmorTable::read_uya(&rac_formats::level::Toc::read_uya_disc(disc)?)?;
    let kit = Kit::read(disc, core, hero, &map, &table)?;
    Ok(suits
        .iter()
        .map(|suit| {
            if armor::UYA_SKINS.contains(&suit.entry) {
                mp_skin(disc, &table, &kit, suit)
            } else {
                ratchet(disc, &table, &kit, hero, &map, suit)
            }
        })
        .collect())
}

/// Ratchet in the armor `suit`, with the kit's wrench, Clank and heli-pack.
fn ratchet(
    disc: &mut impl DiscReader,
    table: &ArmorTable,
    kit: &Kit,
    hero: &MobyClass,
    map: &SkeletonMap,
    suit: &Suit,
) -> Result<Imported, ImportError> {
    let armor = Armor::read_from(disc, table, suit.entry)?;
    let mut builder = Builder::default();
    add_armor(&mut builder, &armor, hero, map)?;
    builder.surfaces.extend(kit.surfaces.iter().cloned());
    builder.paints.extend(kit.paints.iter().cloned());
    let (surfaces, paints) = builder.finish();
    Ok(Imported {
        data: ModelData {
            name: suit.name.into(),
            title: suit.title.into(),
            rest: kit.rest.clone(),
            // Bound as the hero's class is, with his eyes and brows turned at rest.
            turns: map.turns(),
            surfaces,
            wrench_spin: kit.wrench_spin,
            attachments: kit.attachments.iter().map(|(a, _)| a.clone()).collect(),
        },
        paints,
        attachment_paints: kit.attachments.iter().map(|(_, p)| p.clone()).collect(),
    })
}

/// The multiplayer skin `suit`: its class's own mesh on the hero's skeleton less 25 of
/// his face's joints, holding the kit's wrench, moved from Ratchet's hand into its own.
fn mp_skin(
    disc: &mut impl DiscReader,
    table: &ArmorTable,
    kit: &Kit,
    suit: &Suit,
) -> Result<Imported, ImportError> {
    let skin = ClassSkin::read_uya(disc, table, suit.entry)?;
    let map = mp_skeleton_map(&skin.class)?;
    let mut builder = Builder::default();
    let mesh = skin.class.mesh(PacketKind::High)?;
    let metal = skin.class.mesh(PacketKind::Metal)?;
    add_skinned(
        &mut builder,
        &mesh,
        &metal,
        &skin.textures,
        skin.class.unit(),
        folded(&map),
    );
    let mut rest = map.rest();
    let shift = rest[joint::HAND_R] - kit.rest[joint::HAND_R];
    rest[joint::WRENCH] = kit.rest[joint::WRENCH] + shift;
    for wrench in &kit.surfaces {
        let mut wrench = wrench.clone();
        wrench.positions.iter_mut().for_each(|p| *p += shift);
        builder.surfaces.push(wrench);
    }
    builder.paints.extend(kit.paints.iter().cloned());
    let (surfaces, paints) = builder.finish();
    Ok(Imported {
        data: ModelData {
            name: suit.name.into(),
            title: suit.title.into(),
            rest,
            turns: map.turns(),
            surfaces,
            wrench_spin: kit.wrench_spin + shift,
            attachments: Vec::new(),
        },
        paints,
        attachment_paints: Vec::new(),
    })
}

/// How a multiplayer skin's skeleton lines up with the rig's: through the hero's joint
/// each of its joints is (`armor::UYA_SKIN_JOINTS`), bound with its own turns at rest.
fn mp_skeleton_map(class: &MobyClass) -> Result<SkeletonMap, rac_rig::LineUpError> {
    let rig = model::hero().joints;
    let sources: Vec<Option<usize>> = rig
        .iter()
        .map(|joint| {
            let hero = joint.uya?;
            armor::UYA_SKIN_JOINTS.iter().position(|&j| j == hero)
        })
        .collect();
    let parents: Vec<Option<usize>> = class.joints.iter().map(|j| j.parent).collect();
    let unit = class.unit();
    let rest: Vec<Vec3> = class
        .joints
        .iter()
        .map(|j| to_bevy(Vec3::from_array(j.rest) * unit))
        .collect();
    let turns: Vec<Quat> = class.joints.iter().map(rest_orientation).collect();
    SkeletonMap::new(&rig, &sources, &parents, &rest, &turns)
}

/// Ratchet's own surfaces in `armor`: its mesh, skinned to UYA's hero joints, folded onto
/// the rig's.
fn add_armor(
    builder: &mut Builder,
    armor: &Armor,
    hero: &MobyClass,
    map: &SkeletonMap,
) -> Result<(), ImportError> {
    let mesh = armor.mesh(PacketKind::High)?;
    let metal = armor.mesh(PacketKind::Metal)?;
    add_skinned(
        builder,
        &mesh,
        &metal,
        &armor.textures,
        hero.unit(),
        folded(map),
    );
    Ok(())
}

/// Moves a source joint to the rig joint `map` folds it onto.
fn folded(map: &SkeletonMap) -> impl Fn(u8) -> usize + '_ {
    |source| map.follows(source as usize).unwrap_or(joint::ROOT)
}

/// A mesh at rest in class units of `unit` metres, its joints moved to the ones it's worn
/// on by `follow`, with its textures and the metal packets drawn over it.
fn add_skinned(
    builder: &mut Builder,
    mesh: &MobyMesh,
    metal: &MobyMesh,
    textures: &[Texture],
    unit: f32,
    follow: impl Fn(u8) -> usize,
) {
    let chrome = chromed(mesh, metal);
    let vertices: Vec<Placed> = mesh
        .vertices
        .iter()
        .map(|v| Placed {
            position: to_bevy(position(v) * unit),
            normal: to_bevy(Vec3::from_array(normal(v.normal))),
            uv: Vec2::new(v.st[0] as f32, v.st[1] as f32) / 4096.0,
            skin: fold(v.skin, &follow),
        })
        .collect();
    builder.add(mesh, &vertices, &chrome, false, |t, c| {
        textured(textures, t, c)
    });
}

fn textured(textures: &[Texture], texture: i32, chrome: bool) -> ImportPaint {
    ImportPaint {
        texture: usize::try_from(texture)
            .ok()
            .and_then(|t| textures.get(t))
            .cloned(),
        chrome,
    }
}

/// What every suit shares: the wrench as surfaces on the rig's joints, the rig's rest pose
/// with Ratchet's joints and the wrench's, and Clank and the heli-pack as attachments
/// with their paints.
struct Kit {
    surfaces: Vec<Surface>,
    paints: Vec<ImportPaint>,
    rest: Vec<Vec3>,
    wrench_spin: Vec3,
    attachments: Vec<(AttachmentData, Vec<ImportPaint>)>,
}

impl Kit {
    fn read(
        disc: &mut impl DiscReader,
        core: &Core,
        hero: &MobyClass,
        map: &SkeletonMap,
        table: &ArmorTable,
    ) -> Result<Self, ImportError> {
        let unit = hero.unit();
        let hero_rest = |joint: usize| to_bevy(Vec3::from_array(hero.joints[joint].rest) * unit);
        let mut builder = Builder::default();
        let mut rest = map.rest();

        let wrench = core.moby_class(WRENCH_CLASS)?;
        let wrench_armor = Armor::read_from(disc, table, armor::WRENCH)?;
        let mesh = wrench_armor.mesh(PacketKind::High)?;
        let chrome = chromed(&mesh, &wrench_armor.mesh(PacketKind::Metal)?);
        let (vertices, _) = baked(
            &wrench,
            &mesh,
            WRENCH_POSE,
            "the wrench",
            hero_rest(WRENCH_ON),
            |_| joint::WRENCH,
        )?;
        builder.add(&mesh, &vertices, &chrome, true, |t, c| {
            textured(&wrench_armor.textures, t, c)
        });
        rest[joint::WRENCH] = hero_rest(WRENCH_ON);
        let (low, high) = bounds(vertices.iter().map(|v| v.position));
        let wrench_spin = (low + high) / 2.0;

        let mut attachments = Vec::new();
        for (name, class) in BACK_PARTS {
            let textures = match core.header.moby_class(class) {
                Some(entry) if entry.texture_group == CLANK_TEXTURE_GROUP => {
                    vec![table.read_clank_texture(disc, 0)?]
                }
                _ => core.moby_textures(class)?,
            };
            attachments.push(attachment(name, &core.moby_class(class)?, &textures)?);
        }

        let (surfaces, paints) = builder.finish();
        Ok(Self {
            surfaces,
            paints,
            rest,
            wrench_spin,
            attachments,
        })
    }
}

/// `class` worn on the hero's back as an attachment named `name`: its mesh at rest on its
/// own skeleton, in `textures`, and the paint of each of its surfaces.
fn attachment(
    name: &str,
    class: &MobyClass,
    textures: &[Texture],
) -> Result<(AttachmentData, Vec<ImportPaint>), ImportError> {
    let mut builder = Builder::default();
    let mesh = class.mesh(PacketKind::High)?;
    let metal = class.mesh(PacketKind::Metal)?;
    add_skinned(
        &mut builder,
        &mesh,
        &metal,
        textures,
        class.unit(),
        usize::from,
    );
    let (surfaces, paints) = builder.finish();
    let data = AttachmentData {
        name: name.into(),
        on: BACK_ON,
        parents: class.joints.iter().map(|j| j.parent).collect(),
        binds: binds(class),
        surfaces,
    };
    Ok((data, paints))
}

/// DL's skins offered as models (`moby-format.md`, "DL's skins"): Ratchet in DL's
/// helmet and suit (entry 16, which the game's menu doesn't offer), his five armors, then
/// the eleven skins in the order the game's menu lists them, with its names. The armors
/// wear their head as bangles ([`DL_ARMOR_BANGLES`]).
pub const DL_SUITS: &[Suit] = &[
    Suit {
        entry: 16,
        name: "dl-ratchet",
        title: "DL Ratchet",
    },
    Suit {
        entry: 0,
        name: "dl-marauder",
        title: "Marauder",
    },
    Suit {
        entry: 1,
        name: "dl-avenger",
        title: "Avenger",
    },
    Suit {
        entry: 2,
        name: "dl-crusader",
        title: "Crusader",
    },
    Suit {
        entry: 3,
        name: "dl-vindicator",
        title: "Vindicator",
    },
    Suit {
        entry: 4,
        name: "dl-liberator",
        title: "Liberator",
    },
    Suit {
        entry: 5,
        name: "dl-alpha-clank",
        title: "Alpha Clank",
    },
    Suit {
        entry: 8,
        name: "dl-the-muscle",
        title: "The Muscle",
    },
    Suit {
        entry: 12,
        name: "dl-vernon",
        title: "Vernon",
    },
    Suit {
        entry: 11,
        name: "dl-king-claude",
        title: "King Claude",
    },
    Suit {
        entry: 7,
        name: "dl-landshark",
        title: "LandShark",
    },
    Suit {
        entry: 6,
        name: "dl-squidzor",
        title: "Squidzor",
    },
    Suit {
        entry: 13,
        name: "dl-kid-nova",
        title: "Kid Nova",
    },
    Suit {
        entry: 15,
        name: "dl-jak",
        title: "Jak",
    },
    Suit {
        entry: 9,
        name: "dl-w3rm",
        title: "W3RM",
    },
    Suit {
        entry: 10,
        name: "dl-starshield",
        title: "Starshield",
    },
    Suit {
        entry: 14,
        name: "dl-venus",
        title: "Venus",
    },
];

/// The joint of DL's player skeleton each rig joint stands for (107 joints, the same in
/// every skin; `moby-format.md`, "DL's player skeleton"), matched by where they sit and
/// what they carry. DL numbers the left limbs first, and its palm (57, 79) and the thumb's
/// last joint have no UYA joint, so they follow the hand and the thumb. Its face is laid
/// out differently from UYA's, so only the jaw and ears line up; it has no tail, and its
/// Ratchet carries no Clank.
const DL_SOURCES: &[(usize, usize)] = &[
    (joint::ROOT, 0),
    (joint::HIPS, 4),
    (joint::WAIST, 5),
    (joint::SPINE, 6),
    (joint::SPINE_UPPER, 8),
    (joint::CHEST, 9),
    (joint::CHEST_UPPER, 10),
    (joint::NECK, 11),
    (joint::HEAD, 12),
    (joint::JAW, 19),
    (joint::EAR_L, 29),
    (joint::EAR_L_TIP, 30),
    (joint::EAR_R, 31),
    (joint::EAR_R_TIP, 32),
    (joint::CLAVICLE_L, 51),
    (joint::SHOULDER_L, 52),
    (joint::ELBOW_L, 54),
    (joint::FOREARM_L, 55),
    (joint::HAND_L, 56),
    (joint::INDEX_L[0], 58),
    (joint::INDEX_L[1], 59),
    (joint::INDEX_L[2], 60),
    (joint::MIDDLE_L[0], 61),
    (joint::MIDDLE_L[1], 62),
    (joint::MIDDLE_L[2], 63),
    (joint::RING_L[0], 64),
    (joint::RING_L[1], 65),
    (joint::RING_L[2], 66),
    (joint::PINKY_L[0], 67),
    (joint::PINKY_L[1], 68),
    (joint::PINKY_L[2], 69),
    (joint::THUMB_L, 70),
    (joint::THUMB_L_TIP, 71),
    (joint::CLAVICLE_R, 73),
    (joint::SHOULDER_R, 74),
    (joint::ELBOW_R, 76),
    (joint::FOREARM_R, 77),
    (joint::HAND_R, 78),
    (joint::INDEX_R[0], 80),
    (joint::INDEX_R[1], 81),
    (joint::INDEX_R[2], 82),
    (joint::MIDDLE_R[0], 83),
    (joint::MIDDLE_R[1], 84),
    (joint::MIDDLE_R[2], 85),
    (joint::RING_R[0], 86),
    (joint::RING_R[1], 87),
    (joint::RING_R[2], 88),
    (joint::PINKY_R[0], 89),
    (joint::PINKY_R[1], 90),
    (joint::PINKY_R[2], 91),
    (joint::THUMB_R, 92),
    (joint::THUMB_R_TIP, 93),
    (joint::THIGH_L, 97),
    (joint::KNEE_L, 98),
    (joint::ANKLE_L, 99),
    (joint::HEEL_L, 100),
    (joint::TOE_L, 101),
    (joint::THIGH_R, 102),
    (joint::KNEE_R, 103),
    (joint::ANKLE_R, 104),
    (joint::HEEL_R, 105),
    (joint::TOE_R, 106),
];

/// The bangles of DL's armors that the game shows in play: collar 1 and the helmet, 12
/// (the mask 0x1002 it sets every frame; `moby-format.md`, "DL's skins"). The other skins
/// have none.
const DL_ARMOR_BANGLES: [usize; 2] = [1, 12];

/// How a DL skin's skeleton lines up with the rig's.
fn dl_skeleton_map(class: &MobyClass) -> Result<SkeletonMap, rac_rig::LineUpError> {
    let rig = model::hero().joints;
    let mut sources = vec![None; rig.len()];
    for &(rig_joint, source) in DL_SOURCES {
        sources[rig_joint] = Some(source);
    }
    let parents: Vec<Option<usize>> = class.joints.iter().map(|j| j.parent).collect();
    let unit = class.unit();
    let rest: Vec<Vec3> = class
        .joints
        .iter()
        .map(|j| to_bevy(Vec3::from_array(j.rest) * unit))
        .collect();
    // The skins are placed at rest and posed by turns from rest, so their own turns at rest
    // (the right ear's, half a turn) don't matter.
    SkeletonMap::new(&rig, &sources, &parents, &rest, &[])
}

/// Each of `suits` from DL's skin table, on the rig's skeleton. DL's wrench is a class in
/// its level cores, which aren't read yet, so each holds the open model's wrench; a suit
/// that can't be read is an error in its place.
pub fn import_dl(
    disc: &mut impl DiscReader,
    suits: &[Suit],
) -> Result<Vec<Result<Imported, ImportError>>, ImportError> {
    let table = SkinTable::read_dl(&rac_formats::level::Toc::read_dl_disc(disc)?)?;
    let open = rac_rig::open_model();
    Ok(suits
        .iter()
        .map(|suit| {
            let skin = ClassSkin::read_from(disc, &table, suit.entry)?;
            let map = dl_skeleton_map(&skin.class)?;
            let mut builder = Builder::default();
            let mesh = skin.class.mesh(PacketKind::High)?;
            let metal = skin.class.mesh(PacketKind::Metal)?;
            let unit = skin.class.unit();
            add_skinned(
                &mut builder,
                &mesh,
                &metal,
                &skin.textures,
                unit,
                folded(&map),
            );
            let none = MobyMesh {
                vertices: Vec::new(),
                triangles: Vec::new(),
            };
            for bangle in skin
                .bangles
                .iter()
                .filter(|b| DL_ARMOR_BANGLES.contains(&b.slot))
            {
                let mesh = bangle.mesh(PacketKind::High)?;
                add_skinned(
                    &mut builder,
                    &mesh,
                    &none,
                    &skin.textures,
                    unit,
                    folded(&map),
                );
            }
            let (mut surfaces, paints) = builder.finish();
            let rest = map.rest();
            // The open wrench, moved from the open model's hand into this one's.
            let shift = rest[joint::WRENCH] - open.rest[joint::WRENCH];
            for wrench in open.surfaces.iter().filter(|s| s.wrench) {
                let mut wrench = wrench.clone();
                wrench.positions.iter_mut().for_each(|p| *p += shift);
                surfaces.push(wrench);
            }
            Ok(Imported {
                data: ModelData {
                    name: suit.name.into(),
                    title: suit.title.into(),
                    rest,
                    turns: Vec::new(),
                    surfaces,
                    wrench_spin: open.wrench_spin + shift,
                    attachments: Vec::new(),
                },
                paints,
                attachment_paints: Vec::new(),
            })
        })
        .collect())
}

/// Which of `mesh`'s triangles the metal packets `metal` draw over: those with the same
/// three corners. Every metal vertex sits on one of the mesh's.
fn chromed(mesh: &MobyMesh, metal: &MobyMesh) -> Vec<bool> {
    let corners = |mesh: &MobyMesh, triangle: &MeshTriangle| {
        let mut corners = triangle
            .corners
            .map(|corner| mesh.vertices[corner as usize].position);
        corners.sort_unstable();
        corners
    };
    let over: HashSet<[[i16; 3]; 3]> = metal.triangles.iter().map(|t| corners(metal, t)).collect();
    mesh.triangles
        .iter()
        .map(|t| over.contains(&corners(mesh, t)))
        .collect()
}

fn position(vertex: &MeshVertex) -> Vec3 {
    Vec3::new(
        vertex.position[0] as f32,
        vertex.position[1] as f32,
        vertex.position[2] as f32,
    )
}

/// A vertex in the hero's space at rest, skinned to rig joints.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
    position: Vec3,
    normal: Vec3,
    uv: Vec2,
    skin: ([u16; 4], [f32; 4]),
}

/// A source skin's joints moved to the rig's by `follow`, merged, heaviest first.
fn fold(skin: Skin, follow: impl Fn(u8) -> usize) -> ([u16; 4], [f32; 4]) {
    let mut weights: Vec<(usize, u16)> = Vec::with_capacity(3);
    for (&source, &weight) in skin.joints.iter().zip(&skin.weights) {
        if weight == 0 {
            continue;
        }
        let rig = follow(source);
        match weights.iter_mut().find(|(joint, _)| *joint == rig) {
            Some((_, total)) => *total += weight,
            None => weights.push((rig, weight)),
        }
    }
    weights.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total: u16 = weights.iter().map(|(_, w)| w).sum();
    let mut joints = [0u16; 4];
    let mut out = [0.0f32; 4];
    for (slot, (joint, weight)) in weights.into_iter().take(4).enumerate() {
        joints[slot] = joint as u16;
        out[slot] = weight as f32 / total.max(1) as f32;
    }
    if total == 0 {
        out[0] = 1.0;
    }
    (joints, out)
}

/// `mesh`, skinned to `class`'s skeleton, posed in `sequence`'s keyframe and placed at
/// `attach` in the hero's space, with its joints moved to the rig's by `follow`. Also
/// returns where each of the class's joints ends up.
fn baked(
    class: &MobyClass,
    mesh: &MobyMesh,
    (sequence, keyframe): (usize, usize),
    what: &'static str,
    attach: Vec3,
    follow: impl Fn(u8) -> usize,
) -> Result<(Vec<Placed>, Vec<Vec3>), ImportError> {
    let frame = class
        .sequences
        .get(sequence)
        .and_then(Option::as_ref)
        .and_then(|s| s.keyframes.get(keyframe))
        .ok_or(ImportError::Pose(what, sequence, keyframe))?;
    // As the game skins: each joint's posed transform, scales carried to its children,
    // times its inverse bind. A keyframe's scale can hide a piece: Clank's standing pose
    // scales two pieces of his eyes to nothing.
    let skin = skinning(class, frame);
    let unit = class.unit();
    let vertices = mesh
        .vertices
        .iter()
        .map(|v| {
            let at = to_bevy(position(v) * unit);
            let normal_at = to_bevy(Vec3::from_array(normal(v.normal)));
            let (mut placed, mut turned) = (Vec3::ZERO, Vec3::ZERO);
            for (&joint, &weight) in v.skin.joints.iter().zip(&v.skin.weights) {
                let Some(transform) = skin.get(joint as usize) else {
                    continue;
                };
                let weight = weight as f32 / 256.0;
                placed += weight * transform.transform_point3(at);
                turned += weight * transform.transform_vector3(normal_at);
            }
            Placed {
                position: attach + placed,
                normal: turned.normalize_or_zero(),
                uv: Vec2::new(v.st[0] as f32, v.st[1] as f32) / 4096.0,
                skin: fold(v.skin, &follow),
            }
        })
        .collect();
    let joints = joint_positions(class, frame);
    Ok((vertices, joints.into_iter().map(|p| attach + p).collect()))
}

fn bounds(points: impl Iterator<Item = Vec3>) -> (Vec3, Vec3) {
    points.fold((Vec3::MAX, Vec3::MIN), |(low, high), p| {
        (low.min(p), high.max(p))
    })
}

/// Gathers the import's triangles into one surface per paint.
#[derive(Default)]
struct Builder {
    surfaces: Vec<Surface>,
    paints: Vec<ImportPaint>,
}

impl Builder {
    /// Adds `mesh`'s triangles, whose vertices are `vertices`, as new surfaces: one per
    /// texture the triangles use, and per whether they're `chrome`, drawn in the paint
    /// `paint` gives them.
    fn add(
        &mut self,
        mesh: &MobyMesh,
        vertices: &[Placed],
        chrome: &[bool],
        wrench: bool,
        paint: impl Fn(i32, bool) -> ImportPaint,
    ) {
        // Per texture and chrome: its surface, and where each mesh vertex went in it.
        let mut by_paint: Vec<((i32, bool), usize, Vec<Option<u32>>)> = Vec::new();
        for (index, triangle) in mesh.triangles.iter().enumerate() {
            let key = (
                triangle.texture,
                chrome.get(index).copied().unwrap_or(false),
            );
            let slot = match by_paint.iter().position(|(paint, _, _)| *paint == key) {
                Some(slot) => slot,
                None => {
                    self.paints.push(paint(key.0, key.1));
                    let mut surface = Surface::new(Paint::Look(rac_rig::Look::Main), wrench);
                    surface.uvs = Some(Vec::new());
                    self.surfaces.push(surface);
                    by_paint.push((key, self.surfaces.len() - 1, vec![None; vertices.len()]));
                    by_paint.len() - 1
                }
            };
            let [a, b, c] = triangle.corners.map(|i| i as usize);
            let (pa, pb, pc) = (vertices[a], vertices[b], vertices[c]);
            let facing = (pb.position - pa.position).cross(pc.position - pa.position);
            let normals = pa.normal + pb.normal + pc.normal;
            let corners = if facing.dot(normals) < 0.0 {
                [a, c, b]
            } else {
                [a, b, c]
            };
            let (_, surface, placed) = &mut by_paint[slot];
            let target = &mut self.surfaces[*surface];
            for corner in corners {
                let index = *placed[corner].get_or_insert_with(|| {
                    let v = vertices[corner];
                    target.positions.push(v.position);
                    target.normals.push(v.normal);
                    if let Some(uvs) = &mut target.uvs {
                        uvs.push(v.uv);
                    }
                    target.joints.push(v.skin.0);
                    target.weights.push(v.skin.1);
                    target.positions.len() as u32 - 1
                });
                target.indices.push(index);
            }
        }
    }

    fn finish(self) -> (Vec<Surface>, Vec<ImportPaint>) {
        (self.surfaces, self.paints)
    }
}

/// Makes the materials of the models read from the disc and registers them with the rig.
pub fn register_disc_models(
    mut commands: Commands,
    pending: Option<ResMut<DiscModels>>,
    mut models: ResMut<RigModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let mut ready = Vec::new();
    for imported in pending.0.drain(..) {
        let mut data = imported.data;
        let mut paint = |surfaces: &mut Vec<Surface>, paints: Vec<ImportPaint>| {
            for (surface, paint) in surfaces.iter_mut().zip(paints) {
                surface.paint = Paint::Own(materials.add(material(paint, &mut images)));
            }
        };
        paint(&mut data.surfaces, imported.paints);
        for (attachment, paints) in data.attachments.iter_mut().zip(imported.attachment_paints) {
            paint(&mut attachment.surfaces, paints);
        }
        let name = data.name.clone();
        match models.add(data, &mut meshes, &mut bindposes) {
            Ok(()) => ready.push(name),
            Err(error) => warn!("the model \"{name}\" from the disc doesn't fit the rig: {error}"),
        }
    }
    if !ready.is_empty() {
        info!(
            "the hero's models from the disc are ready (Model in the menus): {}",
            ready.join(", ")
        );
    }
    commands.remove_resource::<DiscModels>();
}

fn material(paint: ImportPaint, images: &mut Assets<Image>) -> StandardMaterial {
    let base = StandardMaterial {
        // The games draw both sides of every triangle.
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.65,
        ..default()
    };
    // The game's chrome map is a blurred photograph of surroundings; a polished metal
    // reflects the scene's own, tinted by the texture under it.
    let base = if paint.chrome {
        StandardMaterial {
            metallic: 1.0,
            perceptual_roughness: 0.3,
            ..base
        }
    } else {
        base
    };
    match paint.texture {
        Some(texture) => {
            let mut image = Image::new(
                Extent3d {
                    width: texture.width,
                    height: texture.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                texture.rgba(),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::linear()
            });
            StandardMaterial {
                base_color_texture: Some(images.add(image)),
                ..base
            }
        }
        None => {
            let [r, g, b] = UNTEXTURED;
            StandardMaterial {
                base_color: Color::linear_rgb(r, g, b),
                metallic: base.metallic.max(0.6),
                perceptual_roughness: base.perceptual_roughness.min(0.4),
                ..base
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skins_fold_onto_rig_joints_heaviest_first() {
        let skin = Skin {
            joints: [60, 56, 3],
            weights: [100, 100, 56],
        };
        // Joints 60 and 56 both land on the hand (13); 3 on the spine (2).
        let follow = |source: u8| if source == 3 { 2 } else { 13 };
        let (joints, weights) = fold(skin, follow);
        assert_eq!(joints, [13, 2, 0, 0]);
        assert!((weights[0] - 200.0 / 256.0).abs() < 1e-6);
        assert!((weights[1] - 56.0 / 256.0).abs() < 1e-6);
        assert_eq!(weights[2..], [0.0, 0.0]);
    }

    #[test]
    fn suits_have_their_own_names_and_titles_that_fit_the_menu() {
        for suits in [SUITS, MP_SKINS, DL_SUITS] {
            for (index, suit) in suits.iter().enumerate() {
                assert!(
                    suit.title.len() <= 20,
                    "{} is too long for the button",
                    suit.title
                );
                assert!(suit.name != rac_rig::OPEN_MODEL);
                for other in &suits[index + 1..] {
                    assert!(suit.name != other.name && suit.entry != other.entry);
                }
            }
        }
        for suit in MP_SKINS.iter().chain(DL_SUITS) {
            assert!(SUITS.iter().all(|other| other.name != suit.name));
            assert!(SUITS.iter().all(|other| other.title != suit.title));
        }
        for suit in DL_SUITS {
            assert!(MP_SKINS.iter().all(|other| other.name != suit.name));
        }
        let entries: Vec<usize> = MP_SKINS.iter().map(|s| s.entry).collect();
        assert_eq!(
            entries,
            armor::UYA_SKINS.collect::<Vec<_>>(),
            "every skin, in order"
        );
        assert_eq!(
            SUITS[0].entry, 0,
            "the armor a new game starts in comes first"
        );
    }

    #[test]
    fn metal_packets_chrome_the_triangles_with_their_corners() {
        let vertex = |x: i16| MeshVertex {
            position: [x, 0, 0],
            normal: [0, 0],
            st: [0, 0],
            skin: Skin::default(),
        };
        let triangle = |corners: [u32; 3], texture: i32| MeshTriangle { corners, texture };
        let mesh = MobyMesh {
            vertices: (0..4).map(vertex).collect(),
            triangles: vec![triangle([0, 1, 2], 0), triangle([1, 2, 3], 0)],
        };
        // The metal packet's own vertices, in another order.
        let metal = MobyMesh {
            vertices: [3, 2, 1].map(vertex).to_vec(),
            triangles: vec![triangle([1, 0, 2], rac_formats::moby::CHROME)],
        };
        assert_eq!(chromed(&mesh, &metal), [false, true]);
    }

    #[test]
    fn triangles_turn_to_face_their_normals() {
        // One triangle stored facing down while its normals point up.
        let mesh = MobyMesh {
            vertices: Vec::new(),
            triangles: vec![rac_formats::moby::MeshTriangle {
                corners: [0, 1, 2],
                texture: -1,
            }],
        };
        let up = |position: Vec3| Placed {
            position,
            normal: Vec3::Y,
            uv: Vec2::ZERO,
            skin: ([0; 4], [1.0, 0.0, 0.0, 0.0]),
        };
        let vertices = [up(Vec3::ZERO), up(Vec3::X), up(Vec3::NEG_Z * -1.0)];
        let mut builder = Builder::default();
        builder.add(&mesh, &vertices, &[false], false, |_, chrome| ImportPaint {
            texture: None,
            chrome,
        });
        let (surfaces, paints) = builder.finish();
        assert_eq!(paints.len(), 1);
        let s = &surfaces[0];
        let [a, b, c] = [0, 1, 2].map(|i| s.positions[s.indices[i] as usize]);
        assert!((b - a).cross(c - a).y > 0.0, "turned to face up");
    }
}

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use crate::discs::{Release, find};
    use bevy::math::Quat;

    fn veldin() -> Core {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap()
    }

    const HELI_PACK_CLASS: u32 = BACK_PARTS[0].1;
    const CLANK_CLASS: u32 = BACK_PARTS[1].1;

    #[test]
    #[ignore = "needs game discs"]
    fn joints_sit_along_their_parents_axes_at_rest() {
        // Read as the inverse bind it is, every record puts its joint at its parent's rest
        // position plus the offset turned by the parent's rest orientation, also where the
        // parent is turned at rest (Clank's face, the wrench).
        let core = veldin();
        for (what, class) in [
            ("the hero", core.hero().unwrap()),
            ("Clank", core.moby_class(CLANK_CLASS).unwrap()),
            ("the heli-pack", core.moby_class(HELI_PACK_CLASS).unwrap()),
            ("the wrench", core.moby_class(WRENCH_CLASS).unwrap()),
        ] {
            for (index, joint) in class.joints.iter().enumerate() {
                let Some(parent) = joint.parent.map(|p| &class.joints[p]) else {
                    continue;
                };
                let along = Quat::from_array(parent.rest_rotation()) * Vec3::from(joint.offset);
                let expected = Vec3::from(parent.rest) + along;
                let error = expected.distance(Vec3::from(joint.rest));
                assert!(error < 0.01, "{what} joint {index}: {error} units off");
            }
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn clanks_eyes_stay_on_his_face_in_a_fidget() {
        // The keyframe of his fidget our Veldin image shows.
        const FIDGET: (usize, usize) = (13, 24);
        let core = veldin();
        let clank = core.moby_class(CLANK_CLASS).unwrap();
        let mesh = clank.mesh(PacketKind::High).unwrap();
        let (vertices, _) = baked(&clank, &mesh, FIDGET, "Clank", Vec3::ZERO, |_| 0).unwrap();
        let frame = &clank.sequences[FIDGET.0].as_ref().unwrap().keyframes[FIDGET.1];
        let skin = skinning(&clank, frame);
        // Where his head (joint 4) alone would carry a vertex from rest.
        const HEAD: usize = 4;
        let unit = clank.unit();
        let carried = |v: &MeshVertex| skin[HEAD].transform_point3(to_bevy(position(v) * unit));
        // His eyes and lids hang from joints 14-25, turned at rest. The pupils (16, 22) and
        // the pieces this pose hides (18, 24) move on their own.
        let on = |v: &MeshVertex, joints: &[u8]| {
            v.skin
                .joints
                .iter()
                .zip(v.skin.weights)
                .any(|(j, w)| w > 0 && joints.contains(j))
        };
        let still: Vec<f32> = mesh
            .vertices
            .iter()
            .zip(&vertices)
            .filter(|(v, _)| on(v, &[14, 19, 20, 25]) && !on(v, &[16, 18, 22, 24]))
            .map(|(v, placed)| placed.position.distance(carried(v)))
            .collect();
        assert!(still.len() > 200, "{} vertices", still.len());
        let worst = still.iter().copied().fold(0.0, f32::max);
        assert!(
            worst < 0.002,
            "an eye vertex {worst} m from where his head has it"
        );
        for hidden in [18, 24] {
            let (low, high) = bounds(
                mesh.vertices
                    .iter()
                    .zip(&vertices)
                    .filter(|(v, _)| on(v, &[hidden]))
                    .map(|(_, placed)| placed.position),
            );
            assert!(
                low.distance(high) < 1e-4,
                "joint {hidden}'s piece is hidden"
            );
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn the_backs_clips_pose_clank_and_the_heli_pack_as_the_game_skins_them() {
        let core = veldin();
        let back = crate::hero_anims::back_animations(&core).unwrap();
        // Rest, the heli jump's, the glide's rotor and a fidget, for the pack; rest, the
        // heli jump's and a fidget, for Clank.
        let poses: [&[(u8, usize)]; 2] = [
            &[(1, 0), (5, 17), (6, 8), (7, 24)],
            &[(1, 0), (11, 7), (13, 24)],
        ];
        for (part, (name, class)) in BACK_PARTS.into_iter().enumerate() {
            let class = core.moby_class(class).unwrap();
            let mesh = class.mesh(PacketKind::High).unwrap();
            let binds = crate::hero_anims::binds(&class);
            let unit = class.unit();
            for &(sequence, keyframe) in poses[part] {
                let pose = &back.clips[part][&sequence].keyframes[keyframe];
                let mut world: Vec<Mat4> = Vec::new();
                for (index, joint) in class.joints.iter().enumerate() {
                    let local = Mat4::from_scale_rotation_translation(
                        pose.scales[index],
                        pose.rotations[index],
                        pose.offsets[index],
                    );
                    let parent = joint.parent.map_or(Mat4::IDENTITY, |p| world[p]);
                    world.push(parent * local);
                }
                let skin: Vec<Mat4> = world
                    .iter()
                    .zip(&binds)
                    .map(|(w, b)| *w * b.inverse())
                    .collect();
                let at = (sequence as usize, keyframe);
                let (game, _) = baked(&class, &mesh, at, "a back part", Vec3::ZERO, |_| 0).unwrap();
                let worst = mesh
                    .vertices
                    .iter()
                    .zip(&game)
                    .map(|(v, game)| {
                        let rest = to_bevy(position(v) * unit);
                        let ours: Vec3 = v
                            .skin
                            .joints
                            .iter()
                            .zip(v.skin.weights)
                            .map(|(&j, w)| {
                                w as f32 / 256.0 * skin[j as usize].transform_point3(rest)
                            })
                            .sum();
                        ours.distance(game.position)
                    })
                    .fold(0.0, f32::max);
                assert!(
                    worst < 1e-4,
                    "{name} in {sequence}, {keyframe}: {worst} m off"
                );
            }
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn the_uya_disc_gives_ratchet_the_wrench_clank_and_the_heli_pack() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let hero = core.hero().unwrap();
        let mut imported = import(&mut disc, &core, &hero, &SUITS[..1]).unwrap();
        let imported = imported.remove(0).unwrap();
        imported.data.check().unwrap();
        let triangles =
            |surfaces: &[Surface]| -> usize { surfaces.iter().map(|s| s.indices.len() / 3).sum() };
        assert!(triangles(&imported.data.surfaces) > 4000);
        let textures = |paints: &[ImportPaint]| {
            let mut textures: Vec<&Texture> = Vec::new();
            for texture in paints.iter().filter_map(|p| p.texture.as_ref()) {
                if !textures.contains(&texture) {
                    textures.push(texture);
                }
            }
            textures.len()
        };
        assert_eq!(
            textures(&imported.paints),
            3,
            "the armor's two and the wrench's"
        );
        assert!(
            imported.paints.iter().any(|p| p.chrome),
            "the wrench's head is chrome"
        );
        // Clank and the heli-pack ride on his upper back, in Clank's texture and the
        // level's two.
        let attachments = &imported.data.attachments;
        let names: Vec<&str> = attachments.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["heli-pack", "clank"]);
        assert_eq!(model::hero().joints[BACK_ON].uya, Some(5));
        assert_eq!(attachments[0].binds.len(), 36);
        assert_eq!(attachments[1].binds.len(), 33);
        assert_eq!(triangles(&attachments[0].surfaces), 1562);
        assert_eq!(triangles(&attachments[1].surfaces), 2116);
        assert_eq!(textures(&imported.attachment_paints[0]), 2);
        assert_eq!(textures(&imported.attachment_paints[1]), 1);
        // Clank's metal packets redraw his own triangles, like the armors' do.
        let clank = core.moby_class(CLANK_CLASS).unwrap();
        let metal = clank.mesh(PacketKind::Metal).unwrap();
        let chrome = chromed(&clank.mesh(PacketKind::High).unwrap(), &metal);
        assert_eq!(chrome.iter().filter(|c| **c).count(), metal.triangles.len());
        assert!(imported.attachment_paints[1].iter().any(|p| p.chrome));
        assert!(imported.data.surfaces.iter().any(|s| s.wrench));
        // He stands about as tall as the open model.
        let top = imported
            .data
            .surfaces
            .iter()
            .flat_map(|s| &s.positions)
            .map(|p| p.y)
            .fold(f32::MIN, f32::max);
        assert!((1.2..1.6).contains(&top), "{top}");
    }

    #[test]
    #[ignore = "needs game discs"]
    fn every_suit_on_the_uya_disc_fits_the_rig() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let hero = core.hero().unwrap();
        let suits = import(&mut disc, &core, &hero, SUITS).unwrap();
        assert_eq!(suits.len(), SUITS.len());
        for (suit, imported) in SUITS.iter().zip(suits) {
            let imported = imported.unwrap_or_else(|e| panic!("{}: {e}", suit.name));
            imported.data.check().unwrap();
            assert_eq!(imported.data.name, suit.name);
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn every_multiplayer_skin_on_the_uya_disc_fits_the_rig_with_the_wrench() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let hero = core.hero().unwrap();
        let skins = import(&mut disc, &core, &hero, MP_SKINS).unwrap();
        assert_eq!(skins.len(), MP_SKINS.len());
        let heads: Vec<f32> = MP_SKINS
            .iter()
            .zip(skins)
            .map(|(suit, imported)| {
                let imported = imported.unwrap_or_else(|e| panic!("{}: {e}", suit.name));
                let data = &imported.data;
                data.check().unwrap();
                assert!(data.attachments.is_empty(), "{}: no Clank", suit.name);
                assert!(data.surfaces.iter().any(|s| s.wrench), "{}", suit.name);
                // The wrench sits in its own right hand.
                let hand = data.rest[joint::HAND_R];
                assert_eq!(data.rest[joint::WRENCH], hand, "{}", suit.name);
                // Standing on the ground, about as tall as Ratchet.
                let (low, high) = bounds(
                    data.surfaces
                        .iter()
                        .filter(|s| !s.wrench)
                        .flat_map(|s| s.positions.iter().copied()),
                );
                assert!(low.y.abs() < 0.05, "{}: feet at {}", suit.name, low.y);
                assert!(
                    (1.2..1.55).contains(&high.y),
                    "{}: top at {}",
                    suit.name,
                    high.y
                );
                data.rest[joint::HEAD].y
            })
            .collect();
        // Every skeleton is the hero's, but Bruiser's class is a tenth bigger.
        let bruiser = MP_SKINS
            .iter()
            .position(|s| s.name == "uya-mp-bruiser")
            .unwrap();
        for (index, head) in heads.iter().enumerate() {
            let expected = if index == bruiser {
                heads[0] * 1.1
            } else {
                heads[0]
            };
            assert!(
                (head - expected).abs() < 0.01,
                "{}: head at {head}",
                MP_SKINS[index].name
            );
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn every_skin_on_the_dl_disc_fits_the_rig() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::DlNtsc).expect("a DL NTSC-U disc");
        let skins = import_dl(&mut disc, DL_SUITS).unwrap();
        // Gladiator, the PAL release, holds the same skins.
        let (_, mut pal) = find(std::path::Path::new(&folder), Release::GladiatorPal)
            .expect("a Gladiator PAL disc");
        let pal = import_dl(&mut pal, DL_SUITS).unwrap();
        for (ntsc, pal) in skins.iter().zip(&pal) {
            let (ntsc, pal) = (ntsc.as_ref().unwrap(), pal.as_ref().unwrap());
            assert_eq!(ntsc.data.rest, pal.data.rest, "{}", ntsc.data.name);
            assert_eq!(ntsc.paints, pal.paints, "{}", ntsc.data.name);
        }
        assert_eq!(skins.len(), DL_SUITS.len());
        for (suit, imported) in DL_SUITS.iter().zip(skins) {
            let imported = imported.unwrap_or_else(|e| panic!("{}: {e}", suit.name));
            imported.data.check().unwrap();
            assert!(
                imported.data.surfaces.iter().any(|s| s.wrench),
                "the open wrench"
            );
            // Standing on the ground, about as tall as Ratchet or a little taller: the
            // armors with their helmets, which their own meshes stop short of at the neck.
            let (low, high) = bounds(
                imported
                    .data
                    .surfaces
                    .iter()
                    .flat_map(|s| s.positions.iter().copied()),
            );
            assert!(low.y.abs() < 0.01, "{}: feet at {}", suit.name, low.y);
            assert!(
                (1.2..1.5).contains(&high.y),
                "{}: top at {}",
                suit.name,
                high.y
            );
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn every_skin_on_the_dl_disc_decodes() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::DlNtsc).expect("a DL NTSC-U disc");
        let toc = rac_formats::level::Toc::read_dl_disc(&mut disc).unwrap();
        let table = rac_formats::armor::SkinTable::read_dl(&toc).unwrap();
        // Five armors, eleven skins and Ratchet in DL's helmet.
        assert_eq!(table.entries.len(), 17);
        for index in 0..table.entries.len() {
            let skin = rac_formats::armor::Skin::read_from(&mut disc, &table, index)
                .unwrap_or_else(|e| panic!("skin {index}: {e}"));
            assert_eq!(skin.class.joints.len(), 107, "skin {index}");
            assert!(skin.class.mesh(PacketKind::High).unwrap().triangles.len() > 1500);
            assert!(
                skin.class
                    .mesh(PacketKind::Metal)
                    .unwrap()
                    .triangles
                    .is_empty()
            );
            assert_eq!(skin.textures.len(), 2, "skin {index}");
        }
    }

    #[test]
    #[ignore = "needs game discs"]
    fn every_armor_on_the_uya_disc_decodes() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let toc = rac_formats::level::Toc::read_uya_disc(&mut disc).unwrap();
        let table = rac_formats::armor::ArmorTable::read_uya(&toc).unwrap();
        // Entries 35 on are whole moby classes, which aren't read yet.
        let mut decoded = 0;
        for index in 0..35 {
            let meshes = Armor::read_from(&mut disc, &table, index)
                .and_then(|a| Ok((a.mesh(PacketKind::High)?, a.mesh(PacketKind::Metal)?)));
            match meshes {
                Ok((mesh, metal)) => {
                    assert!(!mesh.triangles.is_empty(), "armor {index}");
                    // Metal packets only redraw the mesh's own triangles.
                    let chrome = chromed(&mesh, &metal).iter().filter(|c| **c).count();
                    assert_eq!(chrome, metal.triangles.len(), "armor {index}");
                    decoded += 1;
                }
                Err(error) => eprintln!("armor {index}: {error}"),
            }
        }
        assert_eq!(decoded, 35);
        assert_eq!(table.clank_textures.len(), 2);
    }
}
