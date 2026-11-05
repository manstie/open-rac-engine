//! Sprites laid in a standing shot's own upright plane, as the Holo Shield's wall
//! (0x378A20 at V1–V2, 0x378298 from V3, and 0x378FF8 from V3 as a hit shakes it), or at
//! its middle facing the camera: drawn once it stands, unfolding where it landed and
//! fading as it folds (0x37A5C0 at V1–V2, 0x379CE0 from V3).

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
/// stars of 57 the game draws there with `fx_billboard` (0x378298). As its shot folds, its
/// alpha falls from the first of `fold` by the second a tick, where the game's fold draw
/// gives it its own; without one, from its own alpha by [`FOLD_FADE`]; and it is
/// `fold_size` if that is given.
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
    #[serde(default)]
    pub fold: Option<(f32, f32)>,
    #[serde(default)]
    pub fold_size: Option<f32>,
}

/// How much a panel's alpha falls a tick as its shot folds: 2 of 0x80 a frame, the layers
/// keeping their size (0x37A5C0, 0x379CE0).
pub const FOLD_FADE: f32 = 2.0 / 128.0;

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

    /// Its alpha `tick` ticks into its shot's folding, counting from 1: as the game's, a
    /// byte with 0x80 for 1, so a faint one falling past nothing wraps round to nearly 2.
    /// From V3 the glow of 11 at 0x15 does from the 11th tick, and the folding shield
    /// flashes blue as it goes, as the game's takes show.
    fn folding_alpha(&self, tick: u32) -> f32 {
        let (from, fade) = self.fold.unwrap_or((self.colour[3], FOLD_FADE));
        let byte = (from * 128.0).round() - (fade * 128.0 * tick as f32).round();
        byte.rem_euclid(256.0) / 128.0
    }
}

/// A standing shot's wall as it is drawn now: its middle, the way it faces, and its width
/// and height, unfolding from where it landed; through its last [`ENDING_TICKS`] standing,
/// the ticks into them; and while it folds, the tick of its folding it is on, counting from
/// 1, and how many there are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Wall {
    pub middle: Vec3,
    pub facing: Quat,
    pub size: Vec2,
    pub ending: Option<u32>,
    pub folding: Option<(u32, u32)>,
}

/// How long before its time is up a holo shield shows that it ends: 4 s (0x376B88 swings
/// its model's colour from its 661st frame standing of 900).
pub const ENDING_TICKS: u32 = 240;

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
    let fold = fire.fold_ticks.max(1);
    // Folding, it has as many ticks left as it began with: the first is tick 1.
    let folding = state
        .folding
        .map(|left| (fold + 1 - left.clamp(1, fold), fold));
    // It folds at the size it had reached, the game's rows growing only while it unfolds.
    let stood = landed.saturating_sub(folding.map_or(0, |(tick, _)| tick - 1));
    let share = (stood as f32 / panels.unfold.max(1) as f32).min(1.0);
    let size = Vec2::new(fire.half_width * 2.0, fire.height) * share;
    let wall = Wall {
        middle: shot.position + Vec3::Y * size.y / 2.0,
        facing: Quat::from_rotation_y(state.yaw),
        size,
        ending: stood.checked_sub(fire.stand_ticks.saturating_sub(ENDING_TICKS)),
        folding,
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
                // Folding, it is drawn as if unhit (0x379CE0).
                let react = state.react.filter(|_| wall.folding.is_none());
                let size = panel.size(react).map_or(0.0, |size| match wall.folding {
                    Some(_) => panel.fold_size.unwrap_or(size),
                    None => size,
                });
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
                    // Folding, the game sets each alpha by the tick alone, drawing what the
                    // camera's nearness would show at all.
                    let alpha = match wall.folding {
                        Some((tick, _)) if fade > 0.0 => panel.folding_alpha(tick),
                        Some(_) => 0.0,
                        None => panel.colour[3] * fade,
                    };
                    material.base_color.set_alpha(alpha);
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
            fold: None,
            fold_size: None,
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

    /// A V3 holo shield `landed` ticks ago, with `folding` ticks of its folding left.
    fn holo(armory: &Armory, landed: Option<u32>, folding: Option<u32>) -> Shot {
        use rac_combat::weapons::{BarrierState, Mods};
        Shot {
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
                folding,
                yaw: std::f32::consts::FRAC_PI_2,
                ..default()
            }),
        }
    }

    #[test]
    fn a_holo_shields_wall_stands_up_from_where_it_landed_with_stars_at_its_middle() {
        let armory = Armory::uya();
        let recipes = FxRecipes::default();
        assert!(wall(&holo(&armory, None, None), &armory, &recipes).is_none());
        // Half unfolded, 17 ticks of 34: half its 4 by 2.3, its middle half that up.
        let (half, panels) = wall(&holo(&armory, Some(17), None), &armory, &recipes).unwrap();
        assert!((half.size - Vec2::new(2.0, 1.15)).length() < 1e-5);
        assert!((half.middle - Vec3::new(1.0, 0.575, 2.0)).length() < 1e-5);
        assert!((half.facing * Vec3::Z - Vec3::X).length() < 1e-5);
        // From V3, two stars of 57 face the camera at its middle (0x378298).
        let stars = panels.layers.iter().filter(|panel| panel.faces_camera);
        assert_eq!(stars.map(|star| star.size).collect::<Vec<_>>(), [0.4, 0.4]);
    }

    #[test]
    fn a_folding_wall_keeps_its_size_as_its_panels_fade() {
        let armory = Armory::uya();
        let recipes = FxRecipes::default();
        // Its fold's 5th tick of 20, with 16 left: still its whole size.
        let folding = holo(&armory, Some(905), Some(16));
        let (wall_now, panels) = wall(&folding, &armory, &recipes).unwrap();
        assert_eq!(wall_now.folding, Some((5, 20)));
        // Its last 240 ticks standing began at 660: it folded 241 ticks into them.
        assert_eq!(wall_now.ending, Some(241));
        assert!((wall_now.size - Vec2::new(4.0, 2.3)).length() < 1e-5);
        // Folding as it unfolds, it keeps what it had reached: 17 ticks, then 3 of folding.
        let early = holo(&armory, Some(19), Some(18));
        let (early, _) = wall(&early, &armory, &recipes).unwrap();
        assert!((early.size - Vec2::new(2.0, 1.15)).length() < 1e-5);
        // From its own alpha, 2 of 0x80 a tick: a star from 0x4F; the blue ring of 8 from
        // 0x3F where it stands at 0x1F (0x379CE0); and the faint glow of 11 at 0x15 down to
        // 1 at the 10th tick, then wrapped round to 0xFF and falling again.
        let alpha = |texture, size: f32, tick| {
            let panel = panels
                .layers
                .iter()
                .find(|panel| panel.texture == texture && (panel.size - size).abs() < 1e-3)
                .unwrap();
            panel.folding_alpha(tick)
        };
        assert!((alpha(Tex::Effect(57), 0.4, 5) - (0x4F - 10) as f32 / 128.0).abs() < 2e-3);
        assert!((alpha(Tex::Effect(8), 1.1, 5) - (0x3F - 10) as f32 / 128.0).abs() < 2e-3);
        assert_eq!(alpha(Tex::Effect(11), 1.0, 10), 1.0 / 128.0);
        assert_eq!(alpha(Tex::Effect(11), 1.0, 11), 255.0 / 128.0);
        assert_eq!(alpha(Tex::Effect(11), 1.0, 20), 237.0 / 128.0);
        // Its stars are a little smaller as it folds: half 0.1875 for 0.2.
        let stars = panels.layers.iter().filter(|panel| panel.faces_camera);
        let sizes: Vec<_> = stars.map(|star| star.fold_size).collect();
        assert_eq!(sizes, [Some(0.375), Some(0.375)]);
    }
}
