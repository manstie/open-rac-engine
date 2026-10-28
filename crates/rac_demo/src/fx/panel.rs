//! Sprites laid in a standing shot's own upright plane, as the Holo Shield's wall
//! (0x378A20 at V1–V2, 0x378298 from V3): drawn once it stands, unfolding where it
//! landed and folding away with it.

use std::collections::BTreeMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::weapons::{FireMode, ShotKind};
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use serde::Deserialize;

use super::{Blend, FxImages, FxQuad, FxRecipes, FxRng, Tex, eye, fx_material, opaque};
use crate::DemoEntity;

/// A standing shot's panels, its whole size once it has stood `unfold` ticks, growing to
/// it from its middle as the Holo Shield's wall does (0x376B88's state 2).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Panels {
    pub unfold: u32,
    pub layers: Vec<Panel>,
}

/// A sprite in the shot's plane: `size` times its whole width and height, `offset` in
/// front of it, turning in the plane by between the two of `spin` degrees a tick, and,
/// with `near`, faded out while the camera is within the first of its squared distances
/// across of the shot's middle, in fully by the second (0x40C040 between them).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Panel {
    pub texture: Tex,
    pub size: f32,
    #[serde(default)]
    pub offset: f32,
    #[serde(default)]
    pub spin: (f32, f32),
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub near: Option<(f32, f32)>,
    #[serde(default)]
    pub blend: Blend,
}

/// The panels drawn now, by player, shot and layer.
#[derive(Resource, Debug, Default)]
pub(crate) struct ShownPanels(BTreeMap<(usize, u32, usize), Shown>);

/// One panel drawn: a frame stretched to the wall's size and the sprite turning in it, so
/// that, as the game multiplies them (0x40C960 makes the turn first), the sprite turns
/// inside an outline that keeps level.
#[derive(Debug)]
pub(crate) struct Shown {
    frame: Entity,
    sprite: Entity,
    material: Handle<StandardMaterial>,
    roll: f32,
}

/// Lays each standing shot's panels, making those that start and dropping those whose
/// shots are gone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_panels(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    quad: Res<FxQuad>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    time: Res<Time>,
    mut shown: ResMut<ShownPanels>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    // The game turns them a frame at a time, at 60 a second.
    let frames = time.delta_secs() * 60.0;
    let mut drawn = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let ShotKind::Barrier(state) = &shot.kind else {
                continue;
            };
            let Some(landed) = state.landed else {
                continue;
            };
            let Some(def) = armory.get(shot.weapon) else {
                continue;
            };
            let FireMode::Barrier(fire) = &def.fire else {
                continue;
            };
            let Some(panels) = recipes.panels(&def.effects, shot.level, Some(shot.kind.name()))
            else {
                continue;
            };
            let share = unfolded(landed, state.folding, panels.unfold, fire.fold_ticks);
            let size = Vec2::new(fire.half_width * 2.0, fire.height) * share;
            let centre = shot.position + Vec3::Y * size.y / 2.0;
            let facing = Quat::from_rotation_y(state.yaw);
            let across = (eye - centre).xz().length_squared();
            for (index, panel) in panels.layers.iter().enumerate() {
                let id = (hero.player, shot.serial, index);
                let fade = panel.near.map_or(1.0, |(from, to)| {
                    ((across - from) / (to - from).max(1e-3)).clamp(0.0, 1.0)
                });
                let entry = shown.0.entry(id).or_insert_with(|| {
                    let material = fx_material(
                        &mut materials,
                        &images,
                        panel.texture,
                        panel.colour,
                        panel.blend,
                    );
                    let sprite = commands
                        .spawn((
                            Mesh3d(quad.0.clone()),
                            MeshMaterial3d(material.clone()),
                            NotShadowCaster,
                            Transform::default(),
                        ))
                        .id();
                    let frame = commands
                        .spawn((
                            Name::new("panel"),
                            DemoEntity,
                            Transform::from_translation(centre).with_scale(Vec3::ZERO),
                            Visibility::default(),
                        ))
                        .add_child(sprite)
                        .id();
                    Shown {
                        frame,
                        sprite,
                        material,
                        roll: rng.between((0.0, 360.0)),
                    }
                });
                entry.roll += rng.between(panel.spin) * frames;
                if let Ok(mut transform) = places.get_mut(entry.frame) {
                    *transform = Transform {
                        translation: centre + facing * Vec3::Z * panel.offset,
                        rotation: facing,
                        scale: (size * panel.size).extend(1.0),
                    };
                }
                if let Ok(mut transform) = places.get_mut(entry.sprite) {
                    transform.rotation = Quat::from_rotation_z(entry.roll.to_radians());
                }
                if let Some(mut material) = materials.get_mut(&entry.material) {
                    material.base_color.set_alpha(panel.colour[3] * fade);
                }
                drawn.push(id);
            }
        }
    }
    shown.0.retain(|id, shown| {
        let keep = drawn.contains(id);
        if !keep {
            commands.entity(shown.frame).despawn();
        }
        keep
    });
}

/// How much of its whole size a standing shot shows: unfolding over `unfold` ticks from
/// landing, and back to nothing over the `fold` ticks of its folding.
fn unfolded(landed: u32, folding: Option<u32>, unfold: u32, fold: u32) -> f32 {
    let grown = (landed as f32 / unfold.max(1) as f32).min(1.0);
    folding.map_or(grown, |left| grown * left as f32 / fold.max(1) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_unfolds_where_it_lands_and_folds_away() {
        assert_eq!(unfolded(0, None, 34, 20), 0.0);
        assert_eq!(unfolded(17, None, 34, 20), 0.5);
        assert_eq!(unfolded(900, None, 34, 20), 1.0);
        assert_eq!(unfolded(900, Some(5), 34, 20), 0.25);
        // Folding before it has unfolded shrinks what there is.
        assert_eq!(unfolded(17, Some(10), 34, 20), 0.25);
    }
}
