//! What warps the frame behind it: the Rift Ripper's lens and its rings of 54 (`effects.md`,
//! "Rift Inducer", "The lens"; 0x3753F8, 0x3750E0). The game writes each one's mask into the
//! frame's alpha, then draws over it a copy of the frame (texture −1, the frame as drawn so
//! far shrunk to 256 by 256), mixed in by that alpha, each point taking the copy from
//! elsewhere: a ring's from nearer its middle, so magnified, and a lens's from nearer its rim,
//! so what lies round it is drawn in toward its middle. As the rift closes (0x375C70) the lens
//! goes and the rings narrow away.
//!
//! The kit draws them in Bevy's transmissive pass, which copies the frame as its opaque passes
//! left it for materials that read it (`refract.wesl`), and depth-tested, as the game's copy
//! is (TEST 0x53001); so a camera that shows them is given [`ScreenSpaceTransmission`].

use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::light::NotShadowCaster;
use bevy::math::ops;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{
    Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin, ScreenSpaceTransmission,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use rac_combat::{Armory, Arsenal};
use rac_hero::{FollowCamera, Hero};
use serde::Deserialize;

use super::{FxImages, FxQuad, FxRecipes, OpenRays, Tex, eye};
use crate::DemoEntity;
use crate::guns::{closing, standing};

/// The lens a shot shows once open (0x3753F8): an ellipse across the view of half-width
/// `wide` and half-height `tall` plus how full it is, `behind` plus that beyond the shot along
/// the view. Fullness grows `fill` a tick to `full` (+0x80, 0x373FE8); each point of the lens
/// takes the frame (1 − cos(fullness × π)) × `pull` of the way out to its rim, and `texture`'s
/// alpha from its top row at the rim to its bottom at the middle mixes that in.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OpenLens {
    pub texture: Tex,
    pub fill: f32,
    pub full: f32,
    pub wide: f32,
    pub tall: f32,
    pub behind: f32,
    pub pull: f32,
    #[serde(default)]
    pub rings: Vec<LensRing>,
}

/// A ring facing the view, `nearer` toward it, `size` across, narrowing `shrink` a tick and
/// starting over at `size` before it is gone, turned `roll` degrees and `spin` more a tick:
/// `texture`'s alpha mixes in the frame from `from` of each point's way out from its middle.
/// As its shot closes it goes as `closing` says, or at once.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LensRing {
    pub texture: Tex,
    pub size: f32,
    #[serde(default)]
    pub shrink: f32,
    #[serde(default)]
    pub roll: f32,
    #[serde(default)]
    pub spin: f32,
    #[serde(default)]
    pub nearer: f32,
    pub from: f32,
    #[serde(default)]
    pub closing: Option<RingClosing>,
}

/// How a ring goes as its shot closes (0x375C70).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum RingClosing {
    /// Narrowing on to nothing, taking the frame from this share of each point's way out.
    Narrows(f32),
    /// As wide as the shot's rays are long, shortening with them.
    WithRays,
}

impl OpenLens {
    pub(crate) fn textures(&self) -> impl Iterator<Item = Tex> + '_ {
        std::iter::once(self.texture).chain(self.rings.iter().map(|ring| ring.texture))
    }

    /// How far toward its rim each point takes the frame from, `fullness` full.
    fn pull_at(&self, fullness: f32) -> f32 {
        (1.0 - ops::cos(fullness.clamp(0.0, 1.0) * PI)) * self.pull
    }

    /// Where the lens lies seen from `eye`, `fullness` full, for a shot at `at`: across the
    /// line from the eye (its width along the view's side, as the game crosses the line with
    /// its down, 0x1A4EC0), beyond the shot.
    fn place(&self, eye: Vec3, at: Vec3, fullness: f32) -> Option<Transform> {
        let way = (at - eye).try_normalize()?;
        let (side, up) = across(way)?;
        Some(Transform {
            translation: at + way * (fullness + self.behind),
            rotation: Quat::from_mat3(&Mat3::from_cols(side, up, side.cross(up))),
            scale: Vec3::new(fullness + self.wide, fullness + self.tall, 1.0),
        })
    }
}

impl LensRing {
    /// Where the ring lies seen from `eye` for a shot at `at`, `size` across and turned `roll`
    /// radians.
    fn place(&self, eye: Vec3, at: Vec3, size: f32, roll: f32) -> Option<Transform> {
        let way = (at - eye).try_normalize()?;
        let (side, up) = across(way)?;
        let facing = Quat::from_mat3(&Mat3::from_cols(side, up, side.cross(up)));
        Some(Transform {
            translation: at - way * self.nearer,
            rotation: facing * Quat::from_rotation_z(roll),
            scale: Vec3::new(size, size, 1.0),
        })
    }
}

/// The view's side and up across the line `way` from the eye, as the game takes them (its
/// down crossed with the line, then the line with that).
fn across(way: Vec3) -> Option<(Vec3, Vec3)> {
    let side = Vec3::NEG_Z.cross(way).try_normalize()?;
    Some((side, way.cross(side)))
}

/// How a refracting effect takes the frame (`refract.wesl`).
#[derive(ShaderType, Debug, Clone, Copy, PartialEq)]
struct Bend {
    /// A lens's: how far toward its rim each point takes the frame from.
    pull: f32,
    /// A ring's: each point takes it from this share of its way out from the middle.
    shrink: f32,
    /// 1 for a lens, whose mask runs from its rim to its middle rather than across its UVs.
    disc: u32,
}

/// A refracting effect's material: its mask, and how it takes the frame.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct RefractMaterial {
    #[uniform(0)]
    bend: Bend,
    #[texture(1)]
    #[sampler(2)]
    mask: Option<Handle<Image>>,
}

impl Material for RefractMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://rac_demo/fx/refract.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn reads_view_transmission_texture(&self) -> bool {
        true
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// Both faces.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// A unit disc facing +z of `wedges` wedges round its middle, its UVs laying it over 0 to 1
/// (v down, as the view's).
fn disc(wedges: u32) -> Mesh {
    let rim = (0..wedges).map(|i| {
        let angle = i as f32 / wedges as f32 * TAU;
        Vec2::new(ops::cos(angle), ops::sin(angle))
    });
    let points: Vec<Vec2> = std::iter::once(Vec2::ZERO).chain(rim).collect();
    let indices = (1..=wedges).flat_map(|i| [0, i, i % wedges + 1]).collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        points.iter().map(|p| [p.x, p.y, 0.0]).collect::<Vec<_>>(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; points.len()])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        points
            .iter()
            .map(|p| [(p.x + 1.0) / 2.0, (1.0 - p.y) / 2.0])
            .collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(indices))
}

/// One ring drawn now: its entity and material, how wide it is and how far turned.
#[derive(Debug)]
struct Ring {
    entity: Entity,
    material: Handle<RefractMaterial>,
    size: f32,
    roll: f32,
}

/// The lens of one open or closing shot, and how long its rays are, as wide as some of its
/// rings are as it closes.
#[derive(Debug)]
struct Shown {
    lens: Entity,
    material: Handle<RefractMaterial>,
    fullness: f32,
    rings: Vec<Ring>,
    rays: f32,
    closing: bool,
}

/// The open shots' lenses drawn now, by player and shot, and the disc they are drawn on.
#[derive(Resource, Debug, Default)]
pub(crate) struct ShownLenses {
    shown: BTreeMap<(usize, u32), Shown>,
    disc: Option<Handle<Mesh>>,
}

/// Draws lenses, with Bevy's PBR plugin.
pub(crate) fn draw_lenses(app: &mut App) {
    embedded_asset!(app, "refract.wesl");
    app.add_plugins(MaterialPlugin::<RefractMaterial>::default())
        .add_systems(Update, (place_lenses, let_cameras_refract));
}

/// Keeps the lens of each open shot whose recipe has one, filling it and turning and
/// narrowing its rings a tick, and of each closing one, its rings narrowing away; dropping
/// those gone. With Bevy's PBR plugin.
#[allow(clippy::too_many_arguments)]
pub(crate) fn step_lenses(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    quad: Res<FxQuad>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut lenses: ResMut<ShownLenses>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Option<ResMut<Assets<RefractMaterial>>>,
    mut commands: Commands,
) {
    let Some(mut materials) = materials else {
        return;
    };
    let round = lenses
        .disc
        .get_or_insert_with(|| meshes.add(disc(32)))
        .clone();
    let mut seen = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let closing = closing(&shot.kind).is_some();
            if !closing && !standing(&shot.kind) {
                continue;
            }
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let name = Some(shot.kind.name());
            let Some(open) = recipes.open(key, shot.level, name) else {
                continue;
            };
            let Some(recipe) = open.lens.as_ref() else {
                continue;
            };
            let id = (hero.player, shot.serial);
            if closing && !lenses.shown.contains_key(&id) {
                continue;
            }
            seen.push(id);
            let mut spawn = |mesh: &Handle<Mesh>, material: Handle<RefractMaterial>| {
                commands
                    .spawn((
                        Name::new("lens"),
                        DemoEntity,
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        NotShadowCaster,
                        Transform::from_scale(Vec3::ZERO),
                    ))
                    .id()
            };
            let shown = lenses.shown.entry(id).or_insert_with(|| {
                let mask = |texture| images.0.get(&texture).cloned();
                let material = materials.add(RefractMaterial {
                    bend: Bend {
                        pull: 0.0,
                        shrink: 1.0,
                        disc: 1,
                    },
                    mask: mask(recipe.texture),
                });
                let rings = recipe
                    .rings
                    .iter()
                    .map(|ring| {
                        let material = materials.add(RefractMaterial {
                            bend: Bend {
                                pull: 0.0,
                                shrink: ring.from,
                                disc: 0,
                            },
                            mask: mask(ring.texture),
                        });
                        Ring {
                            entity: spawn(&quad.0, material.clone()),
                            material,
                            size: ring.size,
                            roll: ring.roll.to_radians(),
                        }
                    })
                    .collect();
                Shown {
                    lens: spawn(&round, material.clone()),
                    material,
                    fullness: 0.0,
                    rings,
                    rays: 0.0,
                    closing: false,
                }
            });
            if closing {
                close_lens(shown, recipe, open.rays.as_ref(), &mut materials);
                continue;
            }
            if let Some(rays) = &open.rays {
                shown.rays = (shown.rays + rays.grow).min(rays.length);
            }
            shown.fullness = (shown.fullness + recipe.fill).min(recipe.full);
            let pull = recipe.pull_at(shown.fullness);
            if let Some(mut material) = materials.get_mut(&shown.material)
                && material.bend.pull != pull
            {
                material.bend.pull = pull;
            }
            for (ring, recipe) in shown.rings.iter_mut().zip(&recipe.rings) {
                ring.roll += recipe.spin.to_radians();
                if recipe.shrink > 0.0 {
                    let size = ring.size - recipe.shrink;
                    ring.size = if size <= 0.2 { recipe.size } else { size };
                }
            }
        }
    }
    lenses.shown.retain(|id, shown| {
        let keep = seen.contains(id);
        if !keep {
            for entity in std::iter::once(shown.lens).chain(shown.rings.iter().map(|r| r.entity)) {
                commands.entity(entity).despawn();
            }
        }
        keep
    });
}

/// A tick of a lens as its shot closes (0x375C70): it goes, and its rings narrow away as
/// their recipes say, from the tick it begins to close.
fn close_lens(
    shown: &mut Shown,
    recipe: &OpenLens,
    rays: Option<&OpenRays>,
    materials: &mut Assets<RefractMaterial>,
) {
    if !std::mem::replace(&mut shown.closing, true) {
        for (ring, recipe) in shown.rings.iter_mut().zip(&recipe.rings) {
            match recipe.closing {
                Some(RingClosing::Narrows(from)) => {
                    if let Some(mut material) = materials.get_mut(&ring.material) {
                        material.bend.shrink = from;
                    }
                }
                Some(RingClosing::WithRays) => {}
                None => ring.size = 0.0,
            }
        }
    }
    for (ring, recipe) in shown.rings.iter_mut().zip(&recipe.rings) {
        ring.roll += recipe.spin.to_radians();
        match recipe.closing {
            Some(RingClosing::Narrows(_)) => ring.size = (ring.size - recipe.shrink).max(0.0),
            Some(RingClosing::WithRays) => ring.size = 2.0 * shown.rays,
            None => {}
        }
    }
    let shorten = rays.and_then(|rays| rays.closing.as_ref());
    shown.rays = (shown.rays - shorten.map_or(shown.rays, |closing| closing.shorten)).max(0.0);
}

/// Lays each lens and its rings across the line from the eye, as they're drawn; a closing
/// shot's lens is gone.
fn place_lenses(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    lenses: Res<ShownLenses>,
    mut places: Query<&mut Transform>,
) {
    let eye = eye(&cameras);
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let Some(shown) = lenses.shown.get(&(hero.player, shot.serial)) else {
                continue;
            };
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let Some(recipe) = recipes
                .open(key, shot.level, Some(shot.kind.name()))
                .and_then(|open| open.lens.as_ref())
            else {
                continue;
            };
            let at = shot.position;
            let mut lay = |entity, place: Option<Transform>| {
                if let (Some(place), Ok(mut transform)) = (place, places.get_mut(entity)) {
                    *transform = place;
                }
            };
            let lens = match shown.closing {
                false => recipe.place(eye, at, shown.fullness),
                true => Some(Transform::from_scale(Vec3::ZERO)),
            };
            lay(shown.lens, lens);
            for (ring, recipe) in shown.rings.iter().zip(&recipe.rings) {
                lay(ring.entity, recipe.place(eye, at, ring.size, ring.roll));
            }
        }
    }
}

/// Gives the follow cameras the copy of the frame the lenses draw from: from the start, as
/// adding it respecialises every mesh's pipeline for the view.
fn let_cameras_refract(
    cameras: Query<Entity, (With<FollowCamera>, Without<ScreenSpaceTransmission>)>,
    mut commands: Commands,
) {
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(ScreenSpaceTransmission::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn lens() -> OpenLens {
        OpenLens {
            texture: Tex::Effect(77),
            fill: 0.005,
            full: 0.75,
            wide: 1.0,
            tall: 0.5,
            behind: 0.5,
            pull: 0.3,
            rings: Vec::new(),
        }
    }

    #[test]
    fn a_lens_pulls_nothing_empty_and_six_tenths_full() {
        let lens = lens();
        assert_eq!(lens.pull_at(0.0), 0.0);
        assert!((lens.pull_at(1.0) - 0.6).abs() < 1e-6);
        // As full as a rift gets.
        assert!((lens.pull_at(0.75) - 0.512).abs() < 1e-3);
    }

    #[test]
    fn a_lens_lies_beyond_its_shot_across_the_view_wider_than_tall() {
        let eye = Vec3::new(0.0, -10.0, 2.0);
        let at = Vec3::new(0.0, 0.0, 2.0);
        let place = lens().place(eye, at, 0.75).unwrap();
        assert!(
            place
                .translation
                .abs_diff_eq(Vec3::new(0.0, 1.25, 2.0), 1e-5)
        );
        // Facing along the line from the eye, its width level and its height up.
        let normal = place.rotation * Vec3::Z;
        assert!(normal.cross(Vec3::Y).length() < 1e-5);
        assert!((place.rotation * Vec3::X).z.abs() < 1e-5);
        assert!((place.rotation * Vec3::Y).abs().abs_diff_eq(Vec3::Z, 1e-5));
        assert!(place.scale.abs_diff_eq(Vec3::new(1.75, 1.25, 1.0), 1e-5));
        // Not laid at all from straight above, where the game's cross has no length.
        assert!(lens().place(at + Vec3::Z, at, 0.5).is_none());
    }

    #[test]
    fn a_ring_lies_toward_the_eye_turned_by_its_roll() {
        let ring = LensRing {
            texture: Tex::Effect(54),
            size: 2.25,
            shrink: 0.08,
            roll: 0.0,
            spin: 2.5,
            nearer: 0.5,
            from: 0.8,
            closing: None,
        };
        let eye = Vec3::new(-10.0, 0.0, 0.0);
        let flat = ring.place(eye, Vec3::ZERO, 2.0, 0.0).unwrap();
        let turned = ring.place(eye, Vec3::ZERO, 2.0, 0.5).unwrap();
        assert!(
            flat.translation
                .abs_diff_eq(Vec3::new(-0.5, 0.0, 0.0), 1e-5)
        );
        let angle = (flat.rotation * Vec3::X).angle_between(turned.rotation * Vec3::X);
        assert!((angle - 0.5).abs() < 1e-5);
        assert!((flat.rotation * Vec3::Z).abs_diff_eq(turned.rotation * Vec3::Z, 1e-5));
    }

    #[test]
    fn closing_narrows_a_lenss_rings_away_with_its_rays() {
        let ring = |shrink, closing| LensRing {
            texture: Tex::Effect(54),
            size: 2.25,
            shrink,
            roll: 0.0,
            spin: 2.5,
            nearer: 0.5,
            from: 0.8,
            closing,
        };
        let recipe = OpenLens {
            rings: vec![
                ring(0.08, Some(RingClosing::Narrows(0.5))),
                ring(0.0, Some(RingClosing::WithRays)),
                ring(0.0, None),
            ],
            ..lens()
        };
        let rays = OpenRays {
            count: 24,
            texture: Tex::Effect(18),
            width: 0.08,
            grow: 0.03,
            length: 1.5,
            middle: [1.0; 4],
            end: [1.0; 4],
            scroll: (0.0, 0.0),
            turn: 1.0,
            blend: Default::default(),
            closing: Some(super::super::RaysClosing {
                shorten: 0.125,
                width: 0.16,
                end: [1.0; 4],
            }),
        };
        let mut materials = Assets::<RefractMaterial>::default();
        let mut material = || {
            materials.add(RefractMaterial {
                bend: Bend {
                    pull: 0.0,
                    shrink: 0.8,
                    disc: 0,
                },
                mask: None,
            })
        };
        let mut shown = Shown {
            lens: Entity::PLACEHOLDER,
            material: material(),
            fullness: 0.75,
            rings: (0..3)
                .map(|_| Ring {
                    entity: Entity::PLACEHOLDER,
                    material: material(),
                    size: 1.0,
                    roll: 0.0,
                })
                .collect(),
            rays: 1.5,
            closing: false,
        };
        close_lens(&mut shown, &recipe, Some(&rays), &mut materials);
        let sizes = |shown: &Shown| shown.rings.iter().map(|ring| ring.size).collect::<Vec<_>>();
        // The narrowing one goes on narrowing, magnifying 2; the other is as wide as the rays
        // were long; the third goes at once.
        assert_eq!(sizes(&shown), [0.92, 3.0, 0.0]);
        let shrink = |ring: &Ring| materials.get(&ring.material).map(|m| m.bend.shrink);
        assert_eq!(shrink(&shown.rings[0]), Some(0.5));
        assert_eq!(shrink(&shown.rings[1]), Some(0.8));
        assert_eq!(shown.rays, 1.375);
        // Never starting over.
        for _ in 0..19 {
            close_lens(&mut shown, &recipe, Some(&rays), &mut materials);
        }
        assert_eq!(sizes(&shown), [0.0, 0.0, 0.0]);
        assert_eq!(shown.rays, 0.0);
    }

    #[test]
    fn a_discs_uvs_lay_its_rim_round_the_middle_of_0_to_1() {
        let mesh = disc(32);
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("no UVs");
        };
        assert_eq!(uvs.len(), 33);
        assert_eq!(uvs[0], [0.5, 0.5]);
        for uv in &uvs[1..] {
            let out = (Vec2::from(*uv) - 0.5).length();
            assert!((out - 0.5).abs() < 1e-5);
        }
        assert_eq!(mesh.indices().map(|indices| indices.len()), Some(96));
    }
}
