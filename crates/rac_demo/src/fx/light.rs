//! The effects' point lights (`effects.md`, "Lights, shake, rumble and the screen"): the
//! game's eight light slots (`light_add` 0x421DE0), each a radius, a colour (1.0 being
//! 0x80) and a place, set by guns' code for a muzzle's flash or a shot in flight, or by
//! the light moby (class 0x27F, 0x300820) from a template whose colour channels each
//! climb to a top and fall back. The game lights its world's vertices within the radius
//! by its own falloff (not read); the kit's are Bevy's point lights reaching to the
//! radius, as bright as `LUMENS` makes them, which is the kit's choice.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::weapons::ShotKind;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use serde::Deserialize;

use super::FxRecipes;
use crate::DemoEntity;

/// How many lights the game has slots for; a light past them isn't made.
pub(crate) const SLOTS: usize = 8;

/// Lumens per unit of colour per square unit of radius: a light of colour 1.0 lights what
/// is half its radius away about a quarter as much as the demo's sun (10,000 lux), enough
/// to show on its pale floor; the game adds its lights to what its sun gives.
const LUMENS: f32 = 8_000.0;

/// A point light of `colour` reaching `radius`, `above` a shot; only while it flies if
/// `flying`, as the Holo Shield's and the Rift Inducer's go once theirs lands or opens.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShotLight {
    pub radius: f32,
    pub colour: [f32; 3],
    #[serde(default)]
    pub above: f32,
    #[serde(default)]
    pub flying: bool,
}

/// Whether a shot is still on its way: a shield not landed, a rift not opened.
fn flying(kind: &ShotKind) -> bool {
    match kind {
        ShotKind::Barrier(state) => state.landed.is_none(),
        ShotKind::Rift(state) => state.open.is_none(),
        _ => true,
    }
}

/// A light living out its ticks: each channel of its colour climbing by `rise` a tick to
/// its top and then falling by `fall` to nothing, as the light moby's do.
#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct Lit {
    pub ticks_left: u32,
    pub colour: [f32; 3],
    pub top: [f32; 3],
    pub rise: [f32; 3],
    pub fall: [f32; 3],
    pub falling: [bool; 3],
    pub radius: f32,
}

impl Lit {
    /// One that starts at `top`, or at nothing when it climbs.
    pub fn new(
        radius: f32,
        top: [f32; 3],
        ticks: u32,
        rise: Option<[f32; 3]>,
        fall: [f32; 3],
    ) -> Self {
        Self {
            ticks_left: ticks.max(1),
            colour: if rise.is_some() { [0.0; 3] } else { top },
            top,
            rise: rise.unwrap_or([0.0; 3]),
            fall,
            falling: [rise.is_none(); 3],
            radius,
        }
    }

    /// A tick on: false once its time is up.
    fn step(&mut self) -> bool {
        if self.ticks_left == 0 {
            return false;
        }
        self.ticks_left -= 1;
        for channel in 0..3 {
            let colour = &mut self.colour[channel];
            if self.falling[channel] {
                *colour = (*colour - self.fall[channel]).max(0.0);
            } else {
                *colour += self.rise[channel];
                if *colour >= self.top[channel] {
                    *colour = self.top[channel];
                    self.falling[channel] = true;
                }
            }
        }
        true
    }
}

/// A point light of `colour` reaching `radius`, in the kit's brightness.
pub(crate) fn point_light(radius: f32, colour: [f32; 3]) -> PointLight {
    let strength = colour.iter().copied().fold(0.0, f32::max);
    let tint = if strength > 0.0 {
        LinearRgba::rgb(
            colour[0] / strength,
            colour[1] / strength,
            colour[2] / strength,
        )
    } else {
        LinearRgba::BLACK
    };
    PointLight {
        color: tint.into(),
        intensity: LUMENS * radius * radius * strength,
        range: radius,
        shadow_maps_enabled: false,
        ..default()
    }
}

/// Takes each light a tick through its life, dropping those whose time is up.
pub(crate) fn step_lights(
    mut lights: Query<(Entity, &mut Lit, &mut PointLight)>,
    mut commands: Commands,
) {
    for (entity, mut lit, mut light) in &mut lights {
        if !lit.step() {
            commands.entity(entity).despawn();
            continue;
        }
        *light = point_light(lit.radius, lit.colour);
    }
}

/// The lights on shots in flight: by player, shot and the light's entity.
#[derive(Resource, Debug, Default)]
pub(crate) struct ShotLights(std::collections::BTreeMap<(usize, u32), Entity>);

/// A light riding a shot.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct Riding;

/// Puts a light on each shot whose recipe gives one, moves them with their shots and drops
/// those of shots gone or landed, within the game's slots.
pub(crate) fn ride_shots(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, &Arsenal)>,
    lit: Query<(), With<Lit>>,
    mut riding: ResMut<ShotLights>,
    mut places: Query<&mut Transform, With<Riding>>,
    mut commands: Commands,
) {
    let mut free = SLOTS.saturating_sub(lit.iter().count() + riding.0.len());
    let mut shown = std::collections::BTreeMap::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let Some(light) = recipes.light(key, shot.level, Some(shot.kind.name())) else {
                continue;
            };
            if light.flying && !flying(&shot.kind) {
                continue;
            }
            let id = (hero.player, shot.serial);
            let at = shot.position + Vec3::Y * light.above;
            let entity = match riding.0.remove(&id) {
                Some(entity) => {
                    if let Ok(mut place) = places.get_mut(entity) {
                        place.translation = at;
                    }
                    entity
                }
                None if free > 0 => {
                    free -= 1;
                    commands
                        .spawn((
                            Name::new("shot light"),
                            DemoEntity,
                            Riding,
                            NotShadowCaster,
                            point_light(light.radius, light.colour),
                            Transform::from_translation(at),
                        ))
                        .id()
                }
                None => continue,
            };
            shown.insert(id, entity);
        }
    }
    for (_, entity) in std::mem::replace(&mut riding.0, shown) {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_templated_light_climbs_to_its_top_and_falls_away() {
        // The small blast's (0x2E9D20): red climbs 0.19 a frame to 2.55, falls 0.15.
        let mut lit = Lit::new(
            10.4,
            [2.55, 2.55, 1.0],
            45,
            Some([0.19, 0.19, 0.19]),
            [0.15, 0.17, 0.19],
        );
        let mut reds = Vec::new();
        while lit.step() {
            reds.push(lit.colour[0]);
        }
        assert_eq!(reds.len(), 45);
        let top = reds.iter().position(|&red| red == 2.55).unwrap();
        assert_eq!(top, 13);
        assert!(reds[top + 1] < 2.55 && reds[44] < 0.3);
        // Blue tops out at 1.0 and is gone by the end.
        assert_eq!(lit.colour[2], 0.0);
    }

    #[test]
    fn a_light_reaches_its_radius_in_its_colour() {
        let light = point_light(8.0, [1.0, 0.8, 0.5]);
        assert_eq!(light.range, 8.0);
        let tint = light.color.to_linear();
        assert_eq!((tint.red, tint.green, tint.blue), (1.0, 0.8, 0.5));
        assert_eq!(point_light(8.0, [0.0; 3]).intensity, 0.0);
    }
}
