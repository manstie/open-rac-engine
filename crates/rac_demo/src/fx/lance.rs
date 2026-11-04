//! The Shock Cannon's charged lance while it runs ([`Arsenal::lance`]; `effects.md`, "The
//! Shock Cannon at V5"): a beam from the muzzle along the aim (0x409448), domes round it
//! (0x3873F8) and sprites along it, all where the aim is each tick, so swept as he turns.
//! The game lights the beam by its texture times itself scrolled another way, writing the
//! one's alpha into the frame's and adding the other through it; the kit's strips do the
//! same in one draw (`lance.wesl`).

use std::collections::BTreeMap;

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use rac_combat::{Armory, Arsenal, Lancing};
use rac_hero::Hero;
use serde::Deserialize;

use super::swell::{self, SwellShape, Track};
use super::{FxImages, FxRecipes, FxRng, Making, Tex, Trail, eye};
use crate::DemoEntity;

/// What a charged lance shows while it runs.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Lance {
    #[serde(default)]
    pub beam: Option<LanceBeam>,
    #[serde(default)]
    pub domes: Vec<LanceDome>,
    /// Particles laid each tick at random along the beam...
    #[serde(default)]
    pub along: Vec<Trail>,
    /// ...and at the muzzle.
    #[serde(default)]
    pub muzzle: Vec<Trail>,
}

impl Lance {
    pub(crate) fn textures(&self) -> Vec<Tex> {
        let trails = self.along.iter().chain(&self.muzzle);
        self.beam
            .iter()
            .map(|beam| beam.texture)
            .chain(self.domes.iter().map(|dome| dome.texture))
            .chain(trails.flat_map(|trail| trail.textures.iter().copied()))
            .collect()
    }

    /// Those whose images must repeat.
    pub(crate) fn scrolled(&self) -> Vec<Tex> {
        let beam = self.beam.iter().map(|beam| beam.texture);
        beam.chain(self.domes.iter().map(|dome| dome.texture))
            .collect()
    }
}

/// A beam facing the camera, `length` long from the muzzle, `width` either side of its
/// middle times `fade` at the lance's tick; at the muzzle `head.1` of that, widening to it
/// over `head.0`. Its texture runs once every `repeat` along it, as its `strips`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LanceBeam {
    pub texture: Tex,
    pub length: f32,
    pub width: f32,
    pub fade: Track,
    pub head: (f32, f32),
    pub repeat: f32,
    pub strips: Vec<BeamStrip>,
}

/// A strip of a beam from `across.0` to `across.1` of its width (−1 one edge, 1 the other),
/// added in `colour`: the texture from `v.0` to `v.1` across it scrolled `scroll.1` a tick
/// along, times the alpha of the texture wholly across it scrolled `scroll.0`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct BeamStrip {
    pub across: (f32, f32),
    pub v: (f32, f32),
    pub colour: [f32; 4],
    pub scroll: (f32, f32),
}

/// One of `fx_dome`'s balls round the lance, its pole back at the muzzle and opening out
/// along it (its matrix's third row is against the aim): `rings` rings of `texture` in
/// `colour` at `alpha`, added, `along` long with its middle `ahead` out, its widths
/// `across` (level, for a level lance) and `across_other` at the lance's tick, turned `roll`
/// degrees about the lance; its texture scrolls `scroll` a tick.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LanceDome {
    pub rings: u32,
    pub texture: Tex,
    pub colour: [f32; 3],
    pub alpha: Track,
    pub across: Track,
    pub across_other: Track,
    pub along: f32,
    pub ahead: f32,
    #[serde(default)]
    pub roll: f32,
    #[serde(default)]
    pub scroll: f32,
}

/// The parts of each player's lance on show, by player.
#[derive(Resource, Debug, Default)]
pub(crate) struct Lances(BTreeMap<usize, Vec<Entity>>);

/// A part of a lance on show: a strip of its beam as of its tick, or a dome with its
/// material.
#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) enum LancePart {
    Strip {
        strip: BeamStrip,
        texture: Tex,
        tick: u32,
    },
    Dome(Handle<StandardMaterial>),
}

/// A strip's mesh, its length along +Y and its width ±1 along X, as (u along, v across)
/// for its light and (u along, 0 to 1 across) for its alpha.
pub(crate) fn strip_mesh(beam: &LanceBeam, strip: &BeamStrip) -> Mesh {
    let mut positions = Vec::new();
    let mut lights = Vec::new();
    let mut alphas = Vec::new();
    let head = beam.head.0.min(beam.length);
    for (along, wide) in [(0.0, beam.head.1), (head, 1.0), (beam.length, 1.0)] {
        // The game's u falls from the muzzle (0x408CA0), so its scroll carries it out.
        let u = -along / beam.repeat;
        for (side, across) in [(0.0, strip.across.0), (1.0, strip.across.1)] {
            positions.push([across * wide, along, 0.0]);
            lights.push([u, strip.v.0 + (strip.v.1 - strip.v.0) * side]);
            alphas.push([u, side]);
        }
    }
    let indices = vec![0, 2, 1, 1, 2, 3, 2, 4, 3, 3, 4, 5];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, lights)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, alphas)
    .with_inserted_indices(Indices::U32(indices))
}

/// Where a beam lies at `lancing`, its width `wide`: along the lance, facing `eye`.
pub(crate) fn beam_place(lancing: &Lancing, wide: f32, eye: Vec3) -> Transform {
    let way = lancing.way.normalize_or(Vec3::NEG_Z);
    let side = way
        .cross(eye - lancing.from)
        .try_normalize()
        .unwrap_or_else(|| way.any_orthonormal_vector());
    Transform::from_translation(lancing.from)
        .with_rotation(Quat::from_mat3(&Mat3::from_cols(
            side,
            way,
            side.cross(way),
        )))
        .with_scale(Vec3::new(wide, 1.0, 1.0))
}

/// Where a dome lies at `lancing`.
pub(crate) fn dome_place(dome: &LanceDome, lancing: &Lancing) -> Transform {
    let way = lancing.way.normalize_or(Vec3::NEG_Z);
    let tick = lancing.tick;
    Transform::from_translation(lancing.from + way * dome.ahead)
        .with_rotation(
            Quat::from_rotation_arc(Vec3::Y, way)
                * Quat::from_rotation_x(std::f32::consts::PI)
                * Quat::from_rotation_y(dome.roll.to_radians()),
        )
        .with_scale(Vec3::new(
            dome.across.at(tick),
            dome.along,
            dome.across_other.at(tick),
        ))
}

/// Shows each running lance where its aim is this tick, lays its sprites, and drops those
/// that have stopped.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_lances(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut lances: ResMut<Lances>,
    mut parts: Query<(&mut Transform, &mut LancePart)>,
    mut rng: ResMut<FxRng>,
    mut making: Making,
) {
    let eye = eye(&cameras);
    let mut spawner = making.spawner();
    let mut shown = BTreeMap::new();
    for (hero, arsenal) in &heroes {
        let Some(lancing) = arsenal.lance() else {
            continue;
        };
        let Some(weapon) = arsenal.hand.gun() else {
            continue;
        };
        let level = arsenal.gun(weapon).map_or(0, |gun| gun.level);
        let key = armory.get(weapon).map_or("", |def| def.effects.as_str());
        let Some(lance) = recipes.lance(key, level) else {
            continue;
        };
        let tick = lancing.tick;
        let wide = lance
            .beam
            .as_ref()
            .map_or(0.0, |beam| beam.width * beam.fade.at(tick));
        let made = match lances.0.remove(&hero.player) {
            Some(made) => {
                let mut domes = lance.domes.iter();
                for &entity in &made {
                    let Ok((mut place, mut part)) = parts.get_mut(entity) else {
                        continue;
                    };
                    match &mut *part {
                        LancePart::Strip { tick: now, .. } => {
                            *now = tick;
                            *place = beam_place(&lancing, wide, eye);
                        }
                        LancePart::Dome(material) => {
                            let Some(dome) = domes.next() else {
                                continue;
                            };
                            *place = dome_place(dome, &lancing);
                            if let Some(mut material) = spawner.materials.get_mut(&*material) {
                                material.base_color.set_alpha(dome.alpha.at(tick));
                                material.uv_transform = Affine2::from_translation(Vec2::new(
                                    0.0,
                                    dome.scroll * tick as f32,
                                ));
                            }
                        }
                    }
                }
                made
            }
            None => make(lance, &lancing, wide, eye, &mut spawner),
        };
        shown.insert(hero.player, made);
        if let Some(beam) = &lance.beam {
            for trail in &lance.along {
                for _ in 0..trail.count(0.0, rng.unit()) {
                    let at = lancing.from + lancing.way * (beam.length * rng.unit());
                    spawner.trail(trail, at, Vec3::ZERO, 1.0, &mut rng);
                }
            }
        }
        for trail in &lance.muzzle {
            for _ in 0..trail.count(0.0, rng.unit()) {
                spawner.trail(trail, lancing.from, Vec3::ZERO, 1.0, &mut rng);
            }
        }
    }
    for (_, made) in std::mem::replace(&mut lances.0, shown) {
        for entity in made {
            spawner.commands.entity(entity).despawn();
        }
    }
}

/// Makes a lance's parts where they are at `lancing`.
fn make(
    lance: &Lance,
    lancing: &Lancing,
    wide: f32,
    eye: Vec3,
    spawner: &mut super::Spawner<'_, '_, '_>,
) -> Vec<Entity> {
    let mut made = Vec::new();
    if let Some(beam) = &lance.beam {
        for strip in &beam.strips {
            let mesh = spawner.meshes.add(strip_mesh(beam, strip));
            made.push(
                spawner
                    .commands
                    .spawn((
                        Name::new("lance beam"),
                        DemoEntity,
                        NotShadowCaster,
                        Mesh3d(mesh),
                        beam_place(lancing, wide, eye),
                        LancePart::Strip {
                            strip: *strip,
                            texture: beam.texture,
                            tick: lancing.tick,
                        },
                    ))
                    .id(),
            );
        }
    }
    for dome in &lance.domes {
        let [r, g, b] = dome.colour;
        let material = spawner.materials.add(StandardMaterial {
            base_color: Color::srgba(r, g, b, dome.alpha.at(lancing.tick)),
            base_color_texture: spawner.images.0.get(&dome.texture).cloned(),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let mesh = spawner
            .meshes
            .add(swell::mesh(&SwellShape::Rings(dome.rings), 1.0));
        made.push(
            spawner
                .commands
                .spawn((
                    Name::new("lance dome"),
                    DemoEntity,
                    NotShadowCaster,
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    dome_place(dome, lancing),
                    LancePart::Dome(material),
                ))
                .id(),
        );
    }
    made
}

/// A strip's light: its colour (linear, alpha its strength) and how far each copy of its
/// texture has scrolled, as (alpha's, light's).
#[derive(ShaderType, Debug, Clone, Copy, PartialEq)]
struct StripLook {
    colour: Vec4,
    scroll: Vec4,
}

impl StripLook {
    fn new(strip: &BeamStrip, tick: u32) -> Self {
        let [r, g, b, a] = strip.colour;
        let linear = Color::srgb(r, g, b).to_linear();
        let ticks = tick as f32;
        Self {
            colour: Vec4::new(linear.red, linear.green, linear.blue, a),
            scroll: Vec4::new(strip.scroll.0 * ticks, strip.scroll.1 * ticks, 0.0, 0.0),
        }
    }
}

/// A beam strip's material: its texture times itself scrolled, added.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct StripMaterial {
    #[uniform(0)]
    look: StripLook,
    #[texture(1)]
    #[sampler(2)]
    texture: Option<Handle<Image>>,
}

impl Material for StripMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://rac_demo/fx/lance.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
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

/// Draws the lances' beams, with Bevy's PBR plugin.
pub(crate) fn draw_beams(app: &mut App) {
    embedded_asset!(app, "lance.wesl");
    app.add_plugins(MaterialPlugin::<StripMaterial>::default())
        .add_systems(Update, show_strips.run_if(resource_exists::<FxImages>));
}

/// Gives each beam strip its material, scrolled as of its tick.
fn show_strips(
    images: Res<FxImages>,
    strips: Query<(Entity, &LancePart, Option<&MeshMaterial3d<StripMaterial>>)>,
    mut materials: ResMut<Assets<StripMaterial>>,
    mut commands: Commands,
) {
    for (entity, part, shown) in &strips {
        let LancePart::Strip {
            strip,
            texture,
            tick,
        } = part
        else {
            continue;
        };
        let look = StripLook::new(strip, *tick);
        if let Some(shown) = shown {
            if let Some(mut material) = materials.get_mut(shown.id())
                && material.look != look
            {
                material.look = look;
            }
            continue;
        }
        commands
            .entity(entity)
            .insert(MeshMaterial3d(materials.add(StripMaterial {
                look,
                texture: images.0.get(texture).cloned(),
            })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn beam() -> LanceBeam {
        LanceBeam {
            texture: Tex::Effect(69),
            length: 40.0,
            width: 1.0,
            fade: Track(vec![(0, 0.0), (10, 1.0)]),
            head: (0.55, 0.5),
            repeat: 1.2,
            strips: Vec::new(),
        }
    }

    #[test]
    fn a_beams_strip_narrows_to_its_head_at_the_muzzle_and_runs_its_texture_out() {
        let strip = BeamStrip {
            across: (-0.6, 0.6),
            v: (0.2, 0.8),
            colour: [1.0; 4],
            scroll: (0.07, 0.08),
        };
        let mesh = strip_mesh(&beam(), &strip);
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        assert_eq!(points[0], [-0.3, 0.0, 0.0]);
        assert_eq!(points[3], [0.6, 0.55, 0.0]);
        assert_eq!(points[5], [0.6, 40.0, 0.0]);
        let Some(VertexAttributeValues::Float32x2(lights)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("uvs");
        };
        // Once every 1.2 u, falling from the muzzle; the light's v the strip's share.
        assert!((lights[5][0] + 40.0 / 1.2).abs() < 1e-4);
        assert_eq!([lights[0][1], lights[1][1]], [0.2, 0.8]);
        let Some(VertexAttributeValues::Float32x2(alphas)) = mesh.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            panic!("uvs");
        };
        assert_eq!([alphas[0][1], alphas[1][1]], [0.0, 1.0]);
    }

    #[test]
    fn a_beam_lies_along_the_lance_facing_the_camera() {
        let lancing = Lancing {
            from: Vec3::new(0.0, 1.0, 0.0),
            way: Vec3::NEG_Z,
            tick: 3,
        };
        let place = beam_place(&lancing, 0.75, Vec3::new(0.0, 3.0, 5.0));
        assert!((place.rotation * Vec3::Y).distance(Vec3::NEG_Z) < 1e-5);
        // Across is level here, the camera being above and behind.
        assert!((place.rotation * Vec3::X).y.abs() < 1e-5);
        assert_eq!(place.scale.x, 0.75);
    }

    #[test]
    fn a_dome_sits_ahead_along_the_lance_turned_about_it() {
        let dome = LanceDome {
            rings: 8,
            texture: Tex::Effect(23),
            colour: [1.0; 3],
            alpha: Track(vec![(0, 0.45), (29, 0.0)]),
            across: Track(vec![(0, 1.1), (13, 2.4), (29, 1.2)]),
            across_other: Track(vec![(0, 1.1), (13, 2.4), (29, 0.4)]),
            along: 15.0,
            ahead: 14.5,
            roll: 45.0,
            scroll: 0.15,
        };
        let lancing = Lancing {
            from: Vec3::ZERO,
            way: Vec3::NEG_Z,
            tick: 29,
        };
        let place = dome_place(&dome, &lancing);
        assert!(place.translation.distance(Vec3::new(0.0, 0.0, -14.5)) < 1e-5);
        assert_eq!(place.scale, Vec3::new(1.2, 15.0, 0.4));
        // Its pole back at the muzzle, half a unit behind it, its widths turned 45° about
        // the lance.
        assert!((place.rotation * Vec3::Y).distance(Vec3::Z) < 1e-5);
        let pole = place.transform_point(Vec3::Y);
        assert!(pole.distance(Vec3::new(0.0, 0.0, 0.5)) < 1e-4, "{pole}");
        let across = place.rotation * Vec3::X;
        assert!((across.x.abs() - across.y.abs()).abs() < 1e-5, "{across}");
    }
}
