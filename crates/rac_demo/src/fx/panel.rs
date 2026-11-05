//! Sprites laid in a standing shot's own upright plane, as the Holo Shield's wall
//! (0x378A20 at V1–V2, 0x378298 from V3, and 0x378FF8 from V3 as a hit shakes it), or at
//! its middle facing the camera: drawn once it stands, unfolding where it landed and
//! folding away with it.

use std::collections::BTreeMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::weapons::{FireMode, Shot, ShotKind};
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use rac_sim::SimTick;
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
/// across of the shot's middle, in fully by the second (0x40C040 between them). It shows
/// `when` its shot reacts to a hit or doesn't; with `grow`, it grows that much of the whole
/// a tick from the hit up to `size`, then goes. With `faces_camera` it faces the camera from
/// the shot's middle instead, `size` units across however far the wall has unfolded, as the
/// stars of 57 the game draws there with `fx_billboard` (0x378298).
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
    #[serde(default)]
    pub when: Showing,
    #[serde(default)]
    pub grow: Option<f32>,
    #[serde(default)]
    pub faces_camera: bool,
}

/// When a panel shows, by its shot's reaction to its last hit (`BarrierState::react`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Showing {
    #[default]
    Always,
    Hit,
    Unhit,
}

impl Panel {
    /// How big it is `react` ticks into its shot's reaction to a hit, or none if it isn't
    /// drawn.
    fn size(&self, react: Option<u32>) -> Option<f32> {
        let shows = match self.when {
            Showing::Always => true,
            Showing::Hit => react.is_some(),
            Showing::Unhit => react.is_none(),
        };
        match self.grow {
            _ if !shows => None,
            None => Some(self.size),
            Some(grow) => {
                let grown = grow * (react.unwrap_or(0) + 1) as f32;
                (grown <= self.size + 1e-3).then_some(grown)
            }
        }
    }
}

/// A standing shot's wall as it is drawn now: its middle, the way it faces, and its width
/// and height, unfolding from where it landed and folding away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Wall {
    pub middle: Vec3,
    pub facing: Quat,
    pub size: Vec2,
}

/// `shot`'s wall now, if it stands and its gun draws panels on it, with those panels.
pub(crate) fn wall<'a>(
    shot: &Shot,
    armory: &Armory,
    recipes: &'a FxRecipes,
) -> Option<(Wall, &'a Panels)> {
    let ShotKind::Barrier(state) = &shot.kind else {
        return None;
    };
    let landed = state.landed?;
    let def = armory.get(shot.weapon)?;
    let FireMode::Barrier(fire) = &def.fire else {
        return None;
    };
    let panels = recipes.panels(&def.effects, shot.level, Some(shot.kind.name()))?;
    let share = unfolded(landed, state.folding, panels.unfold, fire.fold_ticks);
    let size = Vec2::new(fire.half_width * 2.0, fire.height) * share;
    let wall = Wall {
        middle: shot.position + Vec3::Y * size.y / 2.0,
        facing: Quat::from_rotation_y(state.yaw),
        size,
    };
    Some((wall, panels))
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
    tick: Res<SimTick>,
    mut places: Query<&mut Transform>,
    mut commands: Commands,
) {
    let eye = eye(&cameras);
    let view = cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map_or(Quat::IDENTITY, |(_, place)| place.rotation());
    // The game turns them a frame at a time, at 60 a second, by a turn picked each frame:
    // the kit's pick holds for the tick, however many frames draw it.
    let frames = time.delta_secs() * 60.0;
    let mut drawn = Vec::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let ShotKind::Barrier(state) = &shot.kind else {
                continue;
            };
            let Some((wall, panels)) = wall(shot, &armory, &recipes) else {
                continue;
            };
            let across = (eye - wall.middle).xz().length_squared();
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
                            Transform::from_translation(wall.middle).with_scale(Vec3::ZERO),
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
                let seed = (hero.player as u64) << 48 | u64::from(shot.serial) << 8 | index as u64;
                entry.roll += FxRng::for_tick(tick.0, seed).between(panel.spin) * frames;
                let size = panel.size(state.react).unwrap_or(0.0);
                if let Ok(mut transform) = places.get_mut(entry.frame) {
                    *transform = if panel.faces_camera {
                        Transform {
                            translation: wall.middle,
                            rotation: view,
                            scale: Vec3::splat(size),
                        }
                    } else {
                        Transform {
                            translation: wall.middle + wall.facing * Vec3::Z * panel.offset,
                            rotation: wall.facing,
                            scale: (wall.size * size).extend(1.0),
                        }
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
    fn a_panel_shows_by_its_shots_reaction_to_a_hit_and_can_grow_from_it() {
        let panel = |when, grow| Panel {
            texture: Tex::Effect(8),
            size: 1.1,
            offset: 0.0,
            spin: (0.0, 0.0),
            colour: opaque(),
            near: None,
            blend: Blend::default(),
            when,
            grow,
            faces_camera: false,
        };
        assert_eq!(panel(Showing::Always, None).size(None), Some(1.1));
        assert_eq!(panel(Showing::Unhit, None).size(Some(3)), None);
        assert_eq!(panel(Showing::Hit, None).size(None), None);
        assert_eq!(panel(Showing::Hit, None).size(Some(3)), Some(1.1));
        // 0.05 a tick from the hit, to 1.1 on its 22nd tick, then gone (0x378FF8).
        let ring = panel(Showing::Hit, Some(0.05));
        assert_eq!(ring.size(Some(0)), Some(0.05));
        assert!(
            ring.size(Some(21))
                .is_some_and(|size| (size - 1.1).abs() < 1e-5)
        );
        assert_eq!(ring.size(Some(22)), None);
    }

    #[test]
    fn a_holo_shields_wall_stands_up_from_where_it_landed_with_stars_at_its_middle() {
        use rac_combat::weapons::{BarrierState, Mods};
        let armory = Armory::uya();
        let recipes = FxRecipes::default();
        let shot = |landed| Shot {
            serial: 1,
            weapon: armory.find("Holo Shield Glove").unwrap(),
            level: 2,
            position: Vec3::new(1.0, 0.0, 2.0),
            velocity: Vec3::ZERO,
            ticks: 0,
            damage: 0.0,
            splash: 0.0,
            mods: Mods::default(),
            kind: ShotKind::Barrier(BarrierState {
                landed,
                yaw: std::f32::consts::FRAC_PI_2,
                ..default()
            }),
        };
        assert!(wall(&shot(None), &armory, &recipes).is_none());
        // Half unfolded, 17 ticks of 34: half its 4 by 2.3, its middle half that up.
        let (half, panels) = wall(&shot(Some(17)), &armory, &recipes).unwrap();
        assert!((half.size - Vec2::new(2.0, 1.15)).length() < 1e-5);
        assert!((half.middle - Vec3::new(1.0, 0.575, 2.0)).length() < 1e-5);
        assert!((half.facing * Vec3::Z - Vec3::X).length() < 1e-5);
        // From V3, two stars of 57 face the camera at its middle (0x378298).
        let stars = panels.layers.iter().filter(|panel| panel.faces_camera);
        assert_eq!(stars.map(|star| star.size).collect::<Vec<_>>(), [0.4, 0.4]);
    }

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
