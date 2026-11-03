//! The effects' point lights (`effects.md`, "Lights, shake, rumble and the screen"): the
//! game's eight light slots (`light_add` 0x421DE0), each a radius, a colour (1.0 being
//! 0x80) and a place, set by guns' code for a muzzle's flash or a shot in flight, or by
//! the light moby (class 0x27F, 0x300820) from a template whose colour channels each
//! climb to a top and fall back. The game lights what is within the radius by its colour
//! times 1 − distance / radius (0x4A3D80), whichever way it faces, which Bevy's lights
//! can't: the kit draws each as a sphere that brightens what the depth prepass shows
//! inside it (`light.wesl`), before the effects, which the game doesn't light.

use bevy::asset::embedded_asset;
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Face, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use rac_combat::weapons::ShotKind;
use rac_combat::{Armory, Arsenal};
use rac_hero::Hero;
use serde::Deserialize;

use super::FxRecipes;
use crate::DemoEntity;

/// How many lights the game has slots for; a light past them isn't made.
pub(crate) const SLOTS: usize = 8;

/// How much a light of colour 1.0 brightens what it touches where it is, in the game's own
/// (gamma) values: the Qwack-O-Blitzer's (1.0, 0.5, 0.25) makes Veldin's sand beside him
/// 1.54, 1.32 and 1.18 times as bright (`effects.md`, "How the lights fall off").
const STRENGTH: f32 = 0.65;

/// The most a light brightens anything, as the GS's colour times a vertex colour can at
/// most double it.
const MOST: f32 = 2.0;

/// A light as the game's: what is within `radius` of it is brightened by its `colour`
/// (1.0 being 0x80) times 1 − its distance over the radius.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct GameLight {
    pub radius: f32,
    pub colour: [f32; 3],
}

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

/// Takes each light a tick through its life, dropping those whose time is up.
pub(crate) fn step_lights(
    mut lights: Query<(Entity, &mut Lit, &mut GameLight)>,
    mut commands: Commands,
) {
    for (entity, mut lit, mut light) in &mut lights {
        if !lit.step() {
            commands.entity(entity).despawn();
            continue;
        }
        *light = GameLight {
            radius: lit.radius,
            colour: lit.colour,
        };
    }
}

/// What the shader is given for a light: its place and radius, and its colour times
/// [`STRENGTH`] with [`MOST`] in its fourth.
#[derive(ShaderType, Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct LightPlace {
    centre: Vec4,
    colour: Vec4,
}

impl LightPlace {
    fn new(light: &GameLight, at: Vec3) -> Self {
        let [r, g, b] = light.colour.map(|channel| channel * STRENGTH);
        Self {
            centre: at.extend(light.radius),
            colour: Vec4::new(r, g, b, MOST),
        }
    }
}

/// A light's sphere, multiplying what is drawn behind it.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct LightVolume {
    #[uniform(0)]
    light: LightPlace,
}

impl Material for LightVolume {
    fn fragment_shader() -> ShaderRef {
        "embedded://rac_demo/fx/light.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Multiply
    }

    /// Far behind everything, so it is drawn first of what is see-through.
    fn depth_bias(&self) -> f32 {
        -1.0e9
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// Its far side, whatever is in front of it: the camera may be inside it.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Front);
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_compare = Some(CompareFunction::Always);
        }
        Ok(())
    }
}

/// The sphere each light is drawn with, a little bigger than 1 to hold the unit ball.
#[derive(Resource, Debug)]
struct LightSphere(Handle<Mesh>);

/// Draws the lights, with Bevy's PBR plugin: the cameras need a depth prepass.
pub(crate) fn draw_lights(app: &mut App) {
    embedded_asset!(app, "light.wesl");
    app.add_plugins(MaterialPlugin::<LightVolume>::default())
        .add_systems(Startup, make_sphere)
        .add_systems(
            Update,
            (
                prepass_cameras,
                show_lights.run_if(resource_exists::<LightSphere>),
            ),
        );
}

fn make_sphere(mut meshes: ResMut<Assets<Mesh>>, mut commands: Commands) {
    let sphere = Sphere::new(1.1).mesh().ico(2).expect("a small icosphere");
    commands.insert_resource(LightSphere(meshes.add(sphere)));
}

/// Gives each 3D camera the depth prepass the lights read.
fn prepass_cameras(
    cameras: Query<Entity, (With<Camera3d>, Without<DepthPrepass>)>,
    mut commands: Commands,
) {
    for camera in &cameras {
        commands.entity(camera).insert(DepthPrepass);
    }
}

/// Keeps each light's sphere on it, its size and colour as they are now.
fn show_lights(
    sphere: Res<LightSphere>,
    mut lights: Query<(
        Entity,
        &GameLight,
        &mut Transform,
        Option<&MeshMaterial3d<LightVolume>>,
    )>,
    mut materials: ResMut<Assets<LightVolume>>,
    mut commands: Commands,
) {
    for (entity, light, mut transform, volume) in &mut lights {
        transform.scale = Vec3::splat(light.radius.max(0.01));
        let place = LightPlace::new(light, transform.translation);
        if let Some(volume) = volume {
            if let Some(mut material) = materials.get_mut(volume.id())
                && material.light != place
            {
                material.light = place;
            }
            continue;
        }
        commands.entity(entity).insert((
            Mesh3d(sphere.0.clone()),
            MeshMaterial3d(materials.add(LightVolume { light: place })),
        ));
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
                            GameLight {
                                radius: light.radius,
                                colour: light.colour,
                            },
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
    fn a_light_is_drawn_at_its_place_and_radius_in_its_colour() {
        let light = GameLight {
            radius: 7.5,
            colour: [1.0, 0.5, 0.25],
        };
        let place = LightPlace::new(&light, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(place.centre, Vec4::new(1.0, 2.0, 3.0, 7.5));
        // The Qwack-O-Blitzer's makes the sand beside him about 1.54 times as bright.
        let beside = 1.0 + place.colour.x * (1.0 - 1.3 / 7.5);
        assert!((beside - 1.54).abs() < 0.01, "{beside}");
        assert_eq!(place.colour.w, MOST);
    }
}
