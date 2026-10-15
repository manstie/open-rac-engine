//! The hero's models from the user's own discs (PLAN.md §5, 2026-10-13), skinned to the
//! rig's skeleton so the same poses drive them as drive the open model. Read at start-up;
//! nothing is stored.
//! - From UYA: Ratchet in each of his suits ([`SUITS`]) with its textures, and the wrench
//!   and Clank from Veldin's core.
//! - From DL or Gladiator: its skins ([`DL_SUITS`]), each a class with a skeleton of its
//!   own, lined up with the rig's by [`DL_SOURCES`]. They hold the open model's wrench
//!   until DL's own is read.
//!
//! - Ratchet's vertices follow UYA's joints, folded onto the rig's by
//!   [`hero_anims::skeleton_map`]: a finger follows the hand, an eyelid the head.
//! - The wrench and Clank are mobys of their own, which the game puts on his joints 56
//!   and 5 with those joints' orientation (`moby-format.md`, "Finding the hero"). They're
//!   baked in the pose the game shows while he stands, then follow the rig's wrench and
//!   Clank joints. The wrench's mesh and texture and Clank's texture come from the armor
//!   WAD like Ratchet's; Clank's mesh and both skeletons from their classes in the core.
//! - The game draws both sides of every triangle and its strips don't keep a winding, so
//!   each triangle is turned to face the way its vertex normals do.
//! - Metal packets redraw some triangles with the level's chrome map
//!   (`moby-format.md`, "Metal packets"); here those triangles are polished metal instead,
//!   which the scene's own surroundings reflect.

use std::collections::HashSet;

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::{Quat, Vec2, Vec3};
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rac_formats::armor::{self, Armor, ArmorTable, Skin as DlSkin, SkinTable};
use rac_formats::level::Core;
use rac_formats::moby::{
    Mesh as MobyMesh, MeshTriangle, MeshVertex, MobyClass, PacketKind, Skin, normal,
};
use rac_formats::texture::Texture;
use rac_formats::{DiscReader, FormatError};
use rac_rig::{JointDef, ModelData, Paint, RigModels, SkeletonMap, Surface, joint, model};

use crate::hero_anims::{pose, skeleton_map, to_bevy};

const WRENCH_CLASS: u32 = 0x47;
const CLANK_CLASS: u32 = 0x259;
/// The hero joints the game puts them on.
const WRENCH_ON: usize = 56;
const CLANK_ON: usize = 5;
/// The sequence and keyframe each is shown in while Ratchet stands, as in our Veldin
/// memory image.
const WRENCH_POSE: (usize, usize) = (1, 0);
const CLANK_POSE: (usize, usize) = (13, 24);
/// The joints of Clank's class that the rig's Clank joints stand for: his root, head,
/// antenna and feet. His back form has no arm joints.
const CLANK_SOURCES: [(usize, Option<usize>); 7] = [
    (joint::CLANK, Some(0)),
    (joint::CLANK_HEAD, Some(4)),
    (joint::CLANK_ANTENNA, Some(6)),
    (joint::CLANK_ARM_L, None),
    (joint::CLANK_ARM_R, None),
    (joint::CLANK_LEG_L, Some(28)),
    (joint::CLANK_LEG_R, Some(27)),
];
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
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub data: ModelData,
    pub paints: Vec<ImportPaint>,
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
/// and 29–34 are the wrench and two like it. The titles describe how each looks.
pub const SUITS: &[Suit] = &[
    Suit {
        entry: 0,
        name: "uya",
        title: "UYA Ratchet",
    },
    Suit {
        entry: 1,
        name: "uya-blue",
        title: "Blue armor",
    },
    Suit {
        entry: 2,
        name: "uya-rust",
        title: "Rust armor",
    },
    Suit {
        entry: 3,
        name: "uya-green",
        title: "Green armor",
    },
    Suit {
        entry: 4,
        name: "uya-flame",
        title: "Flame armor",
    },
    Suit {
        entry: 5,
        name: "uya-casual",
        title: "Casual wear",
    },
    Suit {
        entry: 6,
        name: "uya-snowman",
        title: "Snowman",
    },
    Suit {
        entry: 7,
        name: "uya-tuxedo",
        title: "Tuxedo",
    },
    Suit {
        entry: 8,
        name: "uya-bug",
        title: "Bug alien",
    },
    Suit {
        entry: 9,
        name: "uya-dome",
        title: "Dome robot",
    },
    Suit {
        entry: 10,
        name: "uya-brown-robot",
        title: "Brown robot",
    },
    Suit {
        entry: 11,
        name: "uya-goggles",
        title: "Goggle robot",
    },
    Suit {
        entry: 12,
        name: "uya-red-robot",
        title: "Red robot",
    },
    Suit {
        entry: 13,
        name: "uya-mohawk",
        title: "Mohawk robot",
    },
    Suit {
        entry: 14,
        name: "uya-orange",
        title: "Orange armor",
    },
];

/// Ratchet in each of `suits`, with the wrench and Clank from `core` (Veldin's), on the
/// rig's skeleton. The wrench and Clank are read once for all of them; a suit that can't
/// be read is an error in its place.
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
            let armor = Armor::read_from(disc, &table, suit.entry)?;
            let mut builder = Builder::default();
            add_armor(&mut builder, &armor, hero, &map)?;
            builder.surfaces.extend(kit.surfaces.iter().cloned());
            builder.paints.extend(kit.paints.iter().cloned());
            let (surfaces, paints) = builder.finish();
            Ok(Imported {
                data: ModelData {
                    name: suit.name.into(),
                    title: suit.title.into(),
                    rest: kit.rest.clone(),
                    surfaces,
                    wrench_spin: kit.wrench_spin,
                },
                paints,
            })
        })
        .collect())
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
    add_skinned(builder, &mesh, &metal, &armor.textures, hero.unit(), map);
    Ok(())
}

/// A mesh skinned to the hero's skeleton that `map` lines up with the rig's, at rest in
/// class units of `unit` metres, with its textures and the metal packets drawn over it.
fn add_skinned(
    builder: &mut Builder,
    mesh: &MobyMesh,
    metal: &MobyMesh,
    textures: &[Texture],
    unit: f32,
    map: &SkeletonMap,
) {
    let chrome = chromed(mesh, metal);
    let follow = |source: u8| map.follows(source as usize).unwrap_or(joint::ROOT);
    let vertices: Vec<Placed> = mesh
        .vertices
        .iter()
        .map(|v| Placed {
            position: to_bevy(position(v) * unit),
            normal: to_bevy(Vec3::from_array(normal(v.normal))),
            uv: Vec2::new(v.st[0] as f32, v.st[1] as f32) / 4096.0,
            skin: fold(v.skin, follow),
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

/// What every suit shares: the wrench and Clank as surfaces on the rig's joints, and the
/// rig's rest pose with Ratchet's joints and theirs.
struct Kit {
    surfaces: Vec<Surface>,
    paints: Vec<ImportPaint>,
    rest: Vec<Vec3>,
    wrench_spin: Vec3,
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

        let clank = core.moby_class(CLANK_CLASS)?;
        let clank_textures = [table.read_clank_texture(disc, 0)?];
        let clank_map = clank_map(&clank)?;
        let clank_follow = |source: u8| {
            clank_map
                .follows(source as usize)
                .map_or(joint::CLANK, |sub| CLANK_SOURCES[sub].0)
        };
        let mesh = clank.mesh(PacketKind::High)?;
        let chrome = chromed(&mesh, &clank.mesh(PacketKind::Metal)?);
        let attach = hero_rest(CLANK_ON);
        let (vertices, joints) = baked(&clank, &mesh, CLANK_POSE, "Clank", attach, clank_follow)?;
        builder.add(&mesh, &vertices, &chrome, false, |t, c| {
            textured(&clank_textures, t, c)
        });
        let rig = model::hero().joints;
        for (rig_joint, source) in CLANK_SOURCES {
            rest[rig_joint] = match source {
                Some(source) => joints[source],
                None => rest[joint::CLANK] + rig[rig_joint].rest - rig[joint::CLANK].rest,
            };
        }

        let (surfaces, paints) = builder.finish();
        Ok(Self {
            surfaces,
            paints,
            rest,
            wrench_spin,
        })
    }
}

/// DL's skins offered as models (`moby-format.md`, "DL's skins"): Ratchet in DL's
/// helmet and suit (entry 16, which the game's menu doesn't offer), then the eleven skins
/// in the order the game's menu lists them, with its names. Entries 0–4 are Ratchet's
/// armors without his head, which comes from elsewhere, so they aren't offered yet.
pub const DL_SUITS: &[Suit] = &[
    Suit {
        entry: 16,
        name: "dl-ratchet",
        title: "DL Ratchet",
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
/// every skin; `moby-format.md`, "DL's player skeleton"). DL numbers the left limbs first;
/// it has no tail, and its Ratchet carries no Clank.
const DL_SOURCES: [(usize, usize); 29] = [
    (joint::ROOT, 0),
    (joint::HIPS, 3),
    (joint::SPINE, 6),
    (joint::CHEST, 9),
    (joint::NECK, 11),
    (joint::HEAD, 12),
    (joint::JAW, 19),
    (joint::EAR_L, 29),
    (joint::EAR_L_TIP, 30),
    (joint::EAR_R, 31),
    (joint::EAR_R_TIP, 32),
    (joint::SHOULDER_L, 52),
    (joint::ELBOW_L, 54),
    (joint::HAND_L, 56),
    (joint::FINGERS_L, 57),
    (joint::THUMB_L, 70),
    (joint::SHOULDER_R, 74),
    (joint::ELBOW_R, 76),
    (joint::HAND_R, 78),
    (joint::FINGERS_R, 79),
    (joint::THUMB_R, 92),
    (joint::THIGH_L, 97),
    (joint::KNEE_L, 98),
    (joint::ANKLE_L, 99),
    (joint::TOE_L, 101),
    (joint::THIGH_R, 102),
    (joint::KNEE_R, 103),
    (joint::ANKLE_R, 104),
    (joint::TOE_R, 106),
];

/// How a DL skin's skeleton lines up with the rig's.
fn dl_skeleton_map(class: &MobyClass) -> Result<SkeletonMap, rac_rig::LineUpError> {
    let rig = model::hero().joints;
    let mut sources = vec![None; rig.len()];
    for (rig_joint, source) in DL_SOURCES {
        sources[rig_joint] = Some(source);
    }
    let parents: Vec<Option<usize>> = class.joints.iter().map(|j| j.parent).collect();
    let unit = class.unit();
    let rest: Vec<Vec3> = class
        .joints
        .iter()
        .map(|j| to_bevy(Vec3::from_array(j.rest) * unit))
        .collect();
    SkeletonMap::new(&rig, &sources, &parents, &rest)
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
            let skin = DlSkin::read_from(disc, &table, suit.entry)?;
            let map = dl_skeleton_map(&skin.class)?;
            let mut builder = Builder::default();
            let mesh = skin.class.mesh(PacketKind::High)?;
            let metal = skin.class.mesh(PacketKind::Metal)?;
            add_skinned(
                &mut builder,
                &mesh,
                &metal,
                &skin.textures,
                skin.class.unit(),
                &map,
            );
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
                    surfaces,
                    wrench_spin: open.wrench_spin + shift,
                },
                paints,
            })
        })
        .collect())
}

/// How Clank's skeleton lines up with the rig's Clank joints, as a skeleton of its own
/// rooted at the rig's Clank joint.
fn clank_map(clank: &MobyClass) -> Result<SkeletonMap, rac_rig::LineUpError> {
    let rig = model::hero().joints;
    let local = |rig_joint: usize| {
        CLANK_SOURCES
            .iter()
            .position(|(joint, _)| *joint == rig_joint)
    };
    let sub: Vec<JointDef> = CLANK_SOURCES
        .iter()
        .map(|&(rig_joint, _)| JointDef {
            parent: rig[rig_joint].parent.and_then(local),
            ..rig[rig_joint].clone()
        })
        .collect();
    let sources: Vec<Option<usize>> = CLANK_SOURCES.iter().map(|(_, s)| *s).collect();
    let parents: Vec<Option<usize>> = clank.joints.iter().map(|j| j.parent).collect();
    let unit = clank.unit();
    let rest: Vec<Vec3> = clank
        .joints
        .iter()
        .map(|j| to_bevy(Vec3::from_array(j.rest) * unit))
        .collect();
    SkeletonMap::new(&sub, &sources, &parents, &rest)
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
    let (rotations, positions) = pose(class, frame);
    let unit = class.unit();
    let rest: Vec<Vec3> = class
        .joints
        .iter()
        .map(|j| to_bevy(Vec3::from_array(j.rest) * unit))
        .collect();
    let skin_matrix = |joint: usize| -> (Quat, Vec3) {
        let rotation = rotations.get(joint).copied().unwrap_or_default();
        let position = positions.get(joint).copied().unwrap_or_default();
        let rest = rest.get(joint).copied().unwrap_or_default();
        (rotation, position - rotation * rest)
    };
    let vertices = mesh
        .vertices
        .iter()
        .map(|v| {
            let at = to_bevy(position(v) * unit);
            let normal_at = to_bevy(Vec3::from_array(normal(v.normal)));
            let (mut placed, mut turned) = (Vec3::ZERO, Vec3::ZERO);
            for (&joint, &weight) in v.skin.joints.iter().zip(&v.skin.weights) {
                let (rotation, shift) = skin_matrix(joint as usize);
                let weight = weight as f32 / 256.0;
                placed += weight * (rotation * at + shift);
                turned += weight * (rotation * normal_at);
            }
            Placed {
                position: attach + placed,
                normal: turned.normalize_or_zero(),
                uv: Vec2::new(v.st[0] as f32, v.st[1] as f32) / 4096.0,
                skin: fold(v.skin, &follow),
            }
        })
        .collect();
    Ok((
        vertices,
        positions.into_iter().map(|p| attach + p).collect(),
    ))
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
        for (surface, paint) in data.surfaces.iter_mut().zip(imported.paints) {
            surface.paint = Paint::Own(materials.add(material(paint, &mut images)));
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
        for suits in [SUITS, DL_SUITS] {
            for (index, suit) in suits.iter().enumerate() {
                assert!(
                    suit.title.len() <= 13,
                    "{} is too long for the button",
                    suit.title
                );
                assert!(suit.name != rac_rig::OPEN_MODEL);
                for other in &suits[index + 1..] {
                    assert!(suit.name != other.name && suit.entry != other.entry);
                }
            }
        }
        for suit in DL_SUITS {
            assert!(SUITS.iter().all(|other| other.name != suit.name));
        }
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

    #[test]
    #[ignore = "needs game discs"]
    fn the_uya_disc_gives_ratchet_the_wrench_and_clank_on_the_rig() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let core = Core::read_uya(&mut disc, crate::hero_anims::LEVEL).unwrap();
        let hero = core.hero().unwrap();
        let mut imported = import(&mut disc, &core, &hero, &SUITS[..1]).unwrap();
        let imported = imported.remove(0).unwrap();
        imported.data.check().unwrap();
        let triangles: usize = imported
            .data
            .surfaces
            .iter()
            .map(|s| s.indices.len() / 3)
            .sum();
        assert!(triangles > 6000, "{triangles} triangles");
        let mut textures: Vec<&Texture> = Vec::new();
        for texture in imported.paints.iter().filter_map(|p| p.texture.as_ref()) {
            if !textures.contains(&texture) {
                textures.push(texture);
            }
        }
        assert_eq!(
            textures.len(),
            4,
            "the armor's two, the wrench's and Clank's"
        );
        assert!(
            imported.paints.iter().any(|p| p.chrome),
            "the wrench's head is chrome"
        );
        // Clank's metal packets redraw his own triangles, like the armors' do.
        let clank = core.moby_class(CLANK_CLASS).unwrap();
        let metal = clank.mesh(PacketKind::Metal).unwrap();
        let chrome = chromed(&clank.mesh(PacketKind::High).unwrap(), &metal);
        assert_eq!(chrome.iter().filter(|c| **c).count(), metal.triangles.len());
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
            // Standing on the ground, about as tall as Ratchet or a little taller.
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
