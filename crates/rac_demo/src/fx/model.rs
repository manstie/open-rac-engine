//! Models the effects spawn by the game's class numbers ([`Emit::Model`]), such as the
//! ring class 0x307 the Infector's burst and the Plasma Whip's wave leave (0x3013B0; its
//! update 0x301530 grows its scale and lowers its opacity each frame; `effects.md`,
//! "Rings"). A game gives the models ([`FxModels`]); without one the kit draws its own
//! crown of flared quads in their place.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

#[cfg(doc)]
use super::Emit;

/// One of a game's models for the effects: each part a mesh in its class's units and the
/// image it is drawn with.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FxModel {
    pub parts: Vec<(Handle<Mesh>, Option<Handle<Image>>)>,
}

/// The models a game gives the effects, by the game's class number.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct FxModels(pub std::collections::BTreeMap<u32, FxModel>);

/// The kit's crown, drawn for a model the game gives none of.
#[derive(Resource, Debug, Clone)]
pub(crate) struct Crown(pub Handle<Mesh>);

/// A model growing by `grow` times a tick as its alpha goes to `end_alpha`.
#[derive(Component, Debug, Clone)]
pub(crate) struct Grows {
    pub ticks_left: u32,
    pub total: u32,
    pub grow: f32,
    pub alpha: f32,
    pub end_alpha: f32,
    pub material: Handle<StandardMaterial>,
}

/// Grows and fades each model a tick, dropping those whose time is up; as 0x301530, it
/// shows first as it was made.
pub(crate) fn step_models(
    mut models: Query<(Entity, &mut Grows, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (entity, mut grows, mut transform) in &mut models {
        if grows.ticks_left == 0 {
            commands.entity(entity).despawn();
            continue;
        }
        grows.ticks_left -= 1;
        transform.scale *= grows.grow;
        let left = grows.ticks_left as f32 / grows.total as f32;
        if let Some(mut material) = materials.get_mut(&grows.material) {
            let alpha = grows.end_alpha + (grows.alpha - grows.end_alpha) * left;
            material.base_color.set_alpha(alpha);
        }
    }
}

/// The kit's crown: twelve walls round the up axis, 0.15 from it at their foot and leaning
/// out to 0.22 at 0.3 high, faint at the top: about the size and shape of the game's ring
/// class at its own scale.
pub(crate) fn crown() -> Mesh {
    const SIDES: u32 = 12;
    let (foot, top, height) = (0.15, 0.22, 0.3);
    let mut positions = Vec::new();
    let mut colours = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for side in 0..=SIDES {
        let turn = side as f32 / SIDES as f32 * std::f32::consts::TAU;
        let out = Vec3::new(ops::cos(turn), 0.0, ops::sin(turn));
        positions.push((out * foot).to_array());
        positions.push((out * top + Vec3::Y * height).to_array());
        colours.extend([[1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.0]]);
        let u = side as f32 / SIDES as f32;
        uvs.extend([[u, 1.0], [u, 0.0]]);
    }
    for side in 0..SIDES {
        let at = side * 2;
        indices.extend([at, at + 2, at + 1, at + 1, at + 2, at + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_normals()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn the_crown_flares_out_from_its_foot() {
        let mesh = crown();
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        for pair in points.chunks(2) {
            let (foot, top) = (Vec3::from(pair[0]), Vec3::from(pair[1]));
            assert_eq!(foot.y, 0.0);
            assert!(top.y > 0.0 && top.with_y(0.0).length() > foot.length());
        }
    }
}
