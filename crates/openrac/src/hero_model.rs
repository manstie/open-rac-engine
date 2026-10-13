//! The hero's model from the user's own UYA disc (PLAN.md §5, 2026-10-13): Ratchet in
//! his first armor with its textures, and the wrench and Clank from Veldin's core, skinned
//! to the rig's skeleton so the same poses drive them as drive the open model. Read at
//! start-up; nothing is stored.
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

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::{Quat, Vec2, Vec3};
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rac_formats::armor::{self, Armor, ArmorTable};
use rac_formats::level::Core;
use rac_formats::moby::{Mesh as MobyMesh, MeshVertex, MobyClass, PacketKind, Skin, normal};
use rac_formats::texture::Texture;
use rac_formats::{DiscReader, FormatError};
use rac_rig::{JointDef, ModelData, Paint, RigModels, SkeletonMap, Surface, joint, model};

use crate::hero_anims::{pose, skeleton_map, to_bevy};

/// The armor a new game starts in.
pub const ARMOR: usize = 0;
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
pub enum ImportPaint {
    Texture(Texture),
    /// Linear RGB, slightly metallic.
    Plain([f32; 3]),
}

/// A model read from the disc. Its surfaces' paints are placeholders until
/// [`register_disc_models`] makes the materials `paints` describe.
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

/// Ratchet in armor `armor`, with the wrench and Clank from `core` (Veldin's), on the
/// rig's skeleton.
pub fn import(
    disc: &mut impl DiscReader,
    core: &Core,
    hero: &MobyClass,
    armor: usize,
) -> Result<Imported, ImportError> {
    let map = skeleton_map(hero)?;
    let unit = hero.unit();
    let hero_rest = |joint: usize| to_bevy(Vec3::from_array(hero.joints[joint].rest) * unit);

    let table = ArmorTable::read_uya(&rac_formats::level::Toc::read_uya_disc(disc)?)?;
    let textured = |textures: &[Texture], texture: i32| match usize::try_from(texture)
        .ok()
        .and_then(|t| textures.get(t))
    {
        Some(image) => ImportPaint::Texture(image.clone()),
        None => ImportPaint::Plain(UNTEXTURED),
    };

    let mut builder = Builder::default();
    let armor = Armor::read_from(disc, &table, armor)?;
    let mesh = armor.mesh(PacketKind::High)?;
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
    builder.add(&mesh, &vertices, false, |t| textured(&armor.textures, t));

    let mut rest = map.rest();

    let wrench = core.moby_class(WRENCH_CLASS)?;
    let wrench_armor = Armor::read_from(disc, &table, armor::WRENCH)?;
    let mesh = wrench_armor.mesh(PacketKind::High)?;
    let (vertices, _) = baked(
        &wrench,
        &mesh,
        WRENCH_POSE,
        "the wrench",
        hero_rest(WRENCH_ON),
        |_| joint::WRENCH,
    )?;
    builder.add(&mesh, &vertices, true, |t| {
        textured(&wrench_armor.textures, t)
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
    let attach = hero_rest(CLANK_ON);
    let (vertices, joints) = baked(&clank, &mesh, CLANK_POSE, "Clank", attach, clank_follow)?;
    builder.add(&mesh, &vertices, false, |t| textured(&clank_textures, t));
    let rig = model::hero().joints;
    for (rig_joint, source) in CLANK_SOURCES {
        rest[rig_joint] = match source {
            Some(source) => joints[source],
            None => rest[joint::CLANK] + rig[rig_joint].rest - rig[joint::CLANK].rest,
        };
    }

    let (surfaces, paints) = builder.finish();
    Ok(Imported {
        data: ModelData {
            name: "uya".into(),
            title: "UYA Ratchet".into(),
            rest,
            surfaces,
            wrench_spin,
        },
        paints,
    })
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
    /// texture the triangles use, drawn in the paint `paint` gives it.
    fn add(
        &mut self,
        mesh: &MobyMesh,
        vertices: &[Placed],
        wrench: bool,
        paint: impl Fn(i32) -> ImportPaint,
    ) {
        // Per texture: its surface, and where each mesh vertex went in it.
        let mut by_texture: Vec<(i32, usize, Vec<Option<u32>>)> = Vec::new();
        for triangle in &mesh.triangles {
            let slot = match by_texture
                .iter()
                .position(|(texture, _, _)| *texture == triangle.texture)
            {
                Some(slot) => slot,
                None => {
                    self.paints.push(paint(triangle.texture));
                    let mut surface = Surface::new(Paint::Look(rac_rig::Look::Main), wrench);
                    surface.uvs = Some(Vec::new());
                    self.surfaces.push(surface);
                    by_texture.push((
                        triangle.texture,
                        self.surfaces.len() - 1,
                        vec![None; vertices.len()],
                    ));
                    by_texture.len() - 1
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
            let (_, surface, placed) = &mut by_texture[slot];
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
    for imported in pending.0.drain(..) {
        let mut data = imported.data;
        for (surface, paint) in data.surfaces.iter_mut().zip(imported.paints) {
            surface.paint = Paint::Own(materials.add(material(paint, &mut images)));
        }
        let name = data.name.clone();
        match models.add(data, &mut meshes, &mut bindposes) {
            Ok(()) => info!("the hero's model \"{name}\" is ready (Model in the menus)"),
            Err(error) => warn!("the model from the disc doesn't fit the rig: {error}"),
        }
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
    match paint {
        ImportPaint::Texture(texture) => {
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
        ImportPaint::Plain([r, g, b]) => StandardMaterial {
            base_color: Color::linear_rgb(r, g, b),
            metallic: 0.6,
            perceptual_roughness: 0.4,
            ..base
        },
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
        builder.add(&mesh, &vertices, false, |_| ImportPaint::Plain([1.0; 3]));
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
        let imported = import(&mut disc, &core, &hero, ARMOR).unwrap();
        imported.data.check().unwrap();
        let triangles: usize = imported
            .data
            .surfaces
            .iter()
            .map(|s| s.indices.len() / 3)
            .sum();
        assert!(triangles > 6000, "{triangles} triangles");
        let textured = imported
            .paints
            .iter()
            .filter(|p| matches!(p, ImportPaint::Texture(_)))
            .count();
        assert_eq!(textured, 4, "the armor's two, the wrench's and Clank's");
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
    fn every_armor_on_the_uya_disc_decodes() {
        let folder = std::env::var("OPENRAC_SOURCE_DIR").expect("set OPENRAC_SOURCE_DIR");
        let (_, mut disc) =
            find(std::path::Path::new(&folder), Release::UyaNtsc).expect("a UYA NTSC-U disc");
        let toc = rac_formats::level::Toc::read_uya_disc(&mut disc).unwrap();
        let table = rac_formats::armor::ArmorTable::read_uya(&toc).unwrap();
        // Entries 35 on are whole moby classes, which aren't read yet.
        let mut decoded = 0;
        for index in 0..35 {
            match Armor::read_uya(&mut disc, index).and_then(|a| a.mesh(PacketKind::High)) {
                Ok(mesh) => {
                    assert!(!mesh.triangles.is_empty(), "armor {index}");
                    decoded += 1;
                }
                Err(error) => eprintln!("armor {index}: {error}"),
            }
        }
        assert_eq!(decoded, 35);
        assert_eq!(table.clank_textures.len(), 2);
    }
}
