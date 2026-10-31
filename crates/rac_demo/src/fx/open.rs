//! What a rift shows once it stands open (`effects.md`, "Rift Inducer"; 0x3753F8): its
//! sprites are [`crate::guns`]'s and its smoke [`super::emit_trails`]'s; here, the rays
//! turning round it, each from its middle out, growing, in a colour going from its middle
//! to its end, their texture scrolling. Not drawn: its lens, which warps the frame behind
//! it (32 quads through texture 77), for which its rings of 54 stand in faintly.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use rac_sim::SimTransform;
use serde::Deserialize;

use super::{
    Billboard, Blend, FxImages, FxRecipes, FxRng, ShotSprite, Tex, Trail, eye, fx_material, opaque,
};
use crate::DemoEntity;
use crate::guns::standing;

/// What a shot shows once it stands open.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
pub struct Open {
    #[serde(default)]
    pub sprites: Vec<ShotSprite>,
    #[serde(default)]
    pub rays: Option<OpenRays>,
    #[serde(default)]
    pub trail: Vec<Trail>,
}

/// `count` rays from a shot's middle out, each laid a random way across the view as it
/// opens and turning from there up to `turn` degrees a tick either way, `width` across and
/// growing `grow` a tick to `length` long, in `middle` going to `end` along each; the
/// texture lies along each, scrolling between the two of `scroll` a tick.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OpenRays {
    pub count: u32,
    pub texture: Tex,
    pub width: f32,
    pub grow: f32,
    pub length: f32,
    #[serde(default = "opaque")]
    pub middle: [f32; 4],
    #[serde(default = "opaque")]
    pub end: [f32; 4],
    pub scroll: (f32, f32),
    pub turn: f32,
    #[serde(default)]
    pub blend: Blend,
}

/// One ray: its entity, the way it points and how it turns.
#[derive(Debug)]
struct Ray {
    entity: Entity,
    way: Vec3,
    turn: Quat,
}

/// The rays of one open shot.
#[derive(Debug)]
struct Shown {
    rays: Vec<Ray>,
    length: f32,
    scroll: f32,
    material: Handle<StandardMaterial>,
}

/// The open shots' rays drawn now, by player and shot.
#[derive(Resource, Debug, Default)]
pub(crate) struct ShownRays(BTreeMap<(usize, u32), Shown>);

/// A quad along x from −½ to ½ with its colour going from `middle` to `end`, as the game's
/// bytes over 0x80 (the effects' sRGB).
fn ray_quad(middle: [f32; 4], end: [f32; 4]) -> Mesh {
    let linear = |[r, g, b, a]: [f32; 4]| {
        let colour = Color::srgba(r, g, b, a).to_linear();
        [colour.red, colour.green, colour.blue, colour.alpha]
    };
    let (middle, end) = (linear(middle), linear(end));
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-0.5, -0.5, 0.0],
            [-0.5, 0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
        ],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![middle, middle, end, end])
    .with_inserted_indices(Indices::U32(vec![0, 2, 1, 1, 2, 3]))
}

/// Keeps the rays of each open shot whose recipe has them, dropping those gone or closed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn step_open_rays(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut shown: ResMut<ShownRays>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut rays: Query<(&mut SimTransform, &mut Transform, &mut Billboard)>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let mut seen = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in arsenal.shots.iter().filter(|shot| standing(&shot.kind)) {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let name = Some(shot.kind.name());
            let Some(recipe) = recipes
                .open(key, shot.level, name)
                .and_then(|open| open.rays.as_ref())
            else {
                continue;
            };
            let id = (hero.player, shot.serial);
            seen.push(id);
            let open = shown.0.entry(id).or_insert_with(|| {
                let view = (eye - shot.position).normalize_or(Vec3::Z);
                let side = view.any_orthonormal_vector();
                let quad = meshes.add(ray_quad(recipe.middle, recipe.end));
                let material = fx_material(
                    &mut materials,
                    &images,
                    recipe.texture,
                    opaque(),
                    recipe.blend,
                );
                let rays = (0..recipe.count)
                    .map(|_| {
                        let way = Quat::from_axis_angle(view, rng.unit() * TAU) * side;
                        let turn = rng.between((-recipe.turn, recipe.turn)).to_radians();
                        let entity = commands
                            .spawn((
                                Name::new("open ray"),
                                DemoEntity,
                                Mesh3d(quad.clone()),
                                MeshMaterial3d(material.clone()),
                                NotShadowCaster,
                                Billboard::default(),
                                SimTransform::from_translation(shot.position),
                                Transform::from_translation(shot.position).with_scale(Vec3::ZERO),
                            ))
                            .id();
                        Ray {
                            entity,
                            way,
                            turn: Quat::from_axis_angle(view, turn),
                        }
                    })
                    .collect();
                Shown {
                    rays,
                    length: 0.0,
                    scroll: 0.0,
                    material,
                }
            });
            open.length = (open.length + recipe.grow).min(recipe.length);
            open.scroll = (open.scroll - rng.between(recipe.scroll)).rem_euclid(8.0);
            if let Some(mut material) = materials.get_mut(&open.material) {
                material.uv_transform = Affine2::from_translation(Vec2::new(open.scroll, 0.0));
            }
            for ray in &mut open.rays {
                ray.way = ray.turn * ray.way;
                let Ok((mut simulated, mut transform, mut billboard)) = rays.get_mut(ray.entity)
                else {
                    continue;
                };
                billboard.axis = Some(ray.way);
                simulated.translation = shot.position + ray.way * open.length / 2.0;
                transform.scale = Vec3::new(open.length, recipe.width, 1.0);
            }
        }
    }
    shown.0.retain(|id, open| {
        let keep = seen.contains(id);
        if !keep {
            for ray in &open.rays {
                commands.entity(ray.entity).despawn();
            }
        }
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn a_rays_colour_goes_from_its_middle_to_its_end_along_x() {
        let mesh = ray_quad([1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.5]);
        let Some(VertexAttributeValues::Float32x4(colours)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("no colours");
        };
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        for (position, colour) in positions.iter().zip(colours) {
            assert_eq!(position[0] < 0.0, colour[3] == 0.0);
        }
    }
}
