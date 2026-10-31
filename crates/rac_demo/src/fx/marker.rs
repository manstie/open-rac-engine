//! The mark on the ground where a thrown gun's shot would land (`effects.md`, "The landing
//! marker"; 0x4C3500, called each frame from each glove and lobbing gun): two quads flat on
//! what it would hit, turning opposite ways.

use std::collections::BTreeMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::Armory;
use rac_combat::Arsenal;
use rac_hero::Hero;
use serde::Deserialize;

use super::{Blend, FxImages, FxQuad, FxRecipes, Tex, fx_material, opaque};
use crate::DemoEntity;

/// How a gun marks where its shot would land: `layers` quads of half width `half` turning
/// `turn` degrees a tick, one way and the other; on a slope steeper than the `flat` the
/// marked place wants, in `slope_colours` if it says so.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Marker {
    pub layers: [Tex; 2],
    pub half: f32,
    pub turn: f32,
    #[serde(default = "two_opaque")]
    pub colours: [[f32; 4]; 2],
    #[serde(default)]
    pub slope_colours: Option<[[f32; 4]; 2]>,
    /// How far above the ground it lies, to stay clear of it.
    #[serde(default)]
    pub lift: f32,
}

fn two_opaque() -> [[f32; 4]; 2] {
    [opaque(), opaque()]
}

/// The normal's height at which a place counts as flat for a thrown agent (0x35CDA0).
const FLAT: f32 = 0.9;

/// One mark drawn now: its layers, and how it last looked.
#[derive(Debug)]
struct Shown {
    layers: [Entity; 2],
    materials: [Handle<StandardMaterial>; 2],
    sloped: bool,
    ticks: u32,
    key: String,
}

/// The marks drawn now, by player.
#[derive(Resource, Debug, Default)]
pub(crate) struct Markers(BTreeMap<usize, Shown>);

/// The turn that lays a quad flat on a surface facing `normal`, rolled `roll` radians.
fn flat_on(normal: Vec3, roll: f32) -> Quat {
    Quat::from_rotation_arc(Vec3::Z, normal.normalize_or(Vec3::Y)) * Quat::from_rotation_z(roll)
}

/// Keeps each hero's mark where his gun's shot would land, dropping it with the gun.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_markers(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    quad: Res<FxQuad>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut markers: ResMut<Markers>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let mut seen = Vec::new();
    for (hero, arsenal) in &heroes {
        let Some(landing) = arsenal.sights.landing else {
            continue;
        };
        let Some(weapon) = arsenal.hand.gun() else {
            continue;
        };
        let key = armory.get(weapon).map_or("", |def| def.effects.as_str());
        let level = arsenal.gun(weapon).map_or(0, |gun| gun.level);
        let Some(marker) = recipes.marker(key, level) else {
            continue;
        };
        seen.push(hero.player);
        // Another gun's may look different.
        if let Some(old) = markers.0.get(&hero.player)
            && old.key != key
            && let Some(old) = markers.0.remove(&hero.player)
        {
            for layer in old.layers {
                commands.entity(layer).despawn();
            }
        }
        let shown = markers.0.entry(hero.player).or_insert_with(|| {
            let material = |layer: usize, materials: &mut Assets<StandardMaterial>| {
                fx_material(
                    materials,
                    &images,
                    marker.layers[layer],
                    marker.colours[layer],
                    Blend::Alpha,
                )
            };
            let handles = [material(0, &mut materials), material(1, &mut materials)];
            let layers = [0, 1].map(|layer| {
                commands
                    .spawn((
                        Name::new("landing marker"),
                        DemoEntity,
                        Mesh3d(quad.0.clone()),
                        MeshMaterial3d(handles[layer].clone()),
                        NotShadowCaster,
                        Transform::from_scale(Vec3::splat(marker.half * 2.0)),
                    ))
                    .id()
            });
            Shown {
                layers,
                materials: handles,
                sloped: false,
                ticks: 0,
                key: key.to_string(),
            }
        });
        shown.ticks += 1;
        let sloped = marker.slope_colours.is_some() && landing.normal.y <= FLAT;
        if sloped != shown.sloped {
            shown.sloped = sloped;
            let colours = marker
                .slope_colours
                .filter(|_| sloped)
                .unwrap_or(marker.colours);
            for (handle, [r, g, b, a]) in shown.materials.iter().zip(colours) {
                if let Some(mut material) = materials.get_mut(handle) {
                    material.base_color = Color::srgba(r, g, b, a);
                }
            }
        }
        let roll = (marker.turn * shown.ticks as f32).to_radians();
        for (layer, sign) in shown.layers.iter().zip([1.0, -1.0]) {
            if let Ok(mut transform) = places.get_mut(*layer) {
                transform.translation = landing.at + landing.normal * marker.lift;
                transform.rotation = flat_on(landing.normal, sign * roll);
            }
        }
    }
    markers.0.retain(|player, shown| {
        let keep = seen.contains(player);
        if !keep {
            for layer in shown.layers {
                commands.entity(layer).despawn();
            }
        }
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_lies_flat_on_the_surface_and_rolls_about_its_normal() {
        let normal = Vec3::new(0.0, 0.8, 0.6);
        let turn = flat_on(normal, 0.7);
        assert!((turn * Vec3::Z).abs_diff_eq(normal, 1e-5));
        let unrolled = flat_on(normal, 0.0);
        assert!(((turn * Vec3::X).angle_between(unrolled * Vec3::X) - 0.7).abs() < 1e-4);
    }
}
