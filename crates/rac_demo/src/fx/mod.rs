//! The guns' effects (PLAN.md P3.18): flames at the muzzle, sprites on shots in flight,
//! beams for rays and arcs, and glows and bursts of particles where shots land, drawn as
//! the game draws its own, as camera-facing sprites in its effect and particle textures
//! (`docs/research/effects.md`).
//!
//! - Each gun's are a recipe in `assets/effects.ron` ([`FxRecipes`]), by its effects key.
//! - Textures are named by the game's numbers ([`Tex`]): an effect texture, or an entry of
//!   a particle type's list. The kit paints a placeholder for each the recipes name
//!   ([`FxImages`]); a game puts its own in their place, as the app does from the user's
//!   disc.
//! - Sprites and particles move at the simulation's 60 Hz, as the game's do frame by
//!   frame, and are drawn between ticks. Their randomness is their own, never the level's:
//!   they don't touch the simulation.

mod paint;

use std::collections::BTreeMap;

use bevy::prelude::*;
use rac_combat::{Armory, GunEvent, GunEventKind};
use rac_sim::SimTransform;
use serde::Deserialize;

pub use paint::{Paint, Shape};

use crate::DemoEntity;

/// The guns' effects: the recipes, and what the placeholders look like.
pub const EFFECTS_RON: &str = include_str!("../../assets/effects.ron");

/// A texture by the game's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub enum Tex {
    /// An effect texture (0–99 the engine's own, in every level).
    Effect(u16),
    /// Entry `n` of particle type `t`'s list, which differs from level to level.
    Particle(u8, u8),
}

/// How a sprite meets what's behind it (the game's blends 1 and 0; its brightening blend
/// is drawn as `Add`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Blend {
    #[default]
    Add,
    Alpha,
}

impl Blend {
    pub fn alpha_mode(self) -> AlphaMode {
        match self {
            Blend::Add => AlphaMode::Add,
            Blend::Alpha => AlphaMode::Blend,
        }
    }
}

fn opaque() -> [f32; 4] {
    [1.0; 4]
}

/// One thing an event shows. Sizes are in world units, times in 60 Hz ticks, colours sRGB
/// with alpha (1 shows the texture as it is).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Emit {
    /// A sprite along the shot's way from the muzzle, `length` by `width`: a flame.
    Flash {
        texture: Tex,
        length: f32,
        width: f32,
        ticks: u32,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        blend: Blend,
    },
    /// A sprite facing the camera, `size` across growing to `end_size` as it fades.
    Glow {
        texture: Tex,
        size: f32,
        end_size: f32,
        ticks: u32,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        blend: Blend,
    },
    /// `count` particles thrown out at up to `speed` units a second, all round or within
    /// `cone` degrees of the shot's way, falling at `gravity`, shrinking from `size` to
    /// `end_size` and fading over `ticks` (give or take a third).
    Burst {
        texture: Tex,
        count: u32,
        speed: f32,
        #[serde(default)]
        cone: Option<f32>,
        #[serde(default)]
        gravity: f32,
        size: f32,
        end_size: f32,
        ticks: u32,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        blend: Blend,
    },
    /// The emits of one of [`FxRecipes::sets`].
    Set(String),
}

impl Emit {
    fn texture(&self) -> Option<Tex> {
        match self {
            Emit::Flash { texture, .. }
            | Emit::Glow { texture, .. }
            | Emit::Burst { texture, .. } => Some(*texture),
            Emit::Set(_) => None,
        }
    }
}

/// A beam from one point to another, `width` across, fading over `ticks`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Beam {
    pub texture: Tex,
    pub width: f32,
    pub ticks: u32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// The sprite drawn on each of a gun's shots in flight, in place of the kit's ball.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShotSprite {
    pub texture: Tex,
    pub size: f32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// A gun's effects; what it leaves out it takes from the defaults.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
pub struct GunFx {
    #[serde(default)]
    pub fire: Option<Vec<Emit>>,
    #[serde(default)]
    pub shot: Option<ShotSprite>,
    #[serde(default)]
    pub impact: Option<Vec<Emit>>,
    #[serde(default)]
    pub ray: Option<Beam>,
}

/// The guns' effects, from `assets/effects.ron`.
#[derive(Resource, Debug, Clone, PartialEq, Deserialize)]
pub struct FxRecipes {
    /// What each texture the recipes name looks like until a game gives its own.
    pub placeholders: BTreeMap<Tex, Paint>,
    /// Lists of emits the recipes share by name ([`Emit::Set`]).
    #[serde(default)]
    pub sets: BTreeMap<String, Vec<Emit>>,
    /// What a gun shows that its recipe doesn't name.
    pub default: GunFx,
    /// The shock mod's arcs.
    pub arc: Beam,
    pub guns: BTreeMap<String, GunFx>,
}

impl Default for FxRecipes {
    fn default() -> Self {
        ron::from_str(EFFECTS_RON).expect("the kit's effects.ron parses")
    }
}

impl FxRecipes {
    fn gun(&self, key: &str) -> Option<&GunFx> {
        self.guns.get(key)
    }

    pub fn fire(&self, key: &str) -> &[Emit] {
        let own = self.gun(key).and_then(|gun| gun.fire.as_deref());
        own.or(self.default.fire.as_deref()).unwrap_or(&[])
    }

    pub fn impact(&self, key: &str) -> &[Emit] {
        let own = self.gun(key).and_then(|gun| gun.impact.as_deref());
        own.or(self.default.impact.as_deref()).unwrap_or(&[])
    }

    pub fn ray(&self, key: &str) -> Option<&Beam> {
        let own = self.gun(key).and_then(|gun| gun.ray.as_ref());
        own.or(self.default.ray.as_ref())
    }

    pub fn shot(&self, key: &str) -> Option<&ShotSprite> {
        self.gun(key).and_then(|gun| gun.shot.as_ref())
    }

    /// `emits` with each [`Emit::Set`] in it replaced by its set's emits (sets don't nest).
    pub fn expand<'a>(&'a self, emits: &'a [Emit]) -> impl Iterator<Item = &'a Emit> {
        emits.iter().flat_map(move |emit| match emit {
            Emit::Set(name) => self.sets.get(name).map_or(&[][..], Vec::as_slice),
            _ => std::slice::from_ref(emit),
        })
    }

    /// Every texture the recipes draw with.
    pub fn textures(&self) -> Vec<Tex> {
        let mut textures = vec![self.arc.texture];
        let guns = std::iter::once(&self.default).chain(self.guns.values());
        for fx in guns {
            textures.extend(fx.ray.as_ref().map(|beam| beam.texture));
            textures.extend(fx.shot.as_ref().map(|shot| shot.texture));
            for emits in fx.fire.iter().chain(&fx.impact) {
                textures.extend(self.expand(emits).filter_map(Emit::texture));
            }
        }
        for emits in self.sets.values() {
            textures.extend(emits.iter().filter_map(Emit::texture));
        }
        textures.sort();
        textures.dedup();
        textures
    }
}

/// The images the effects draw with, by the game's numbers: the kit's placeholders, or a
/// game's own put in their place.
#[derive(Resource, Debug, Default, Clone)]
pub struct FxImages(pub BTreeMap<Tex, Handle<Image>>);

/// The one quad every sprite is, facing +z.
#[derive(Resource, Debug, Clone)]
pub struct FxQuad(pub Handle<Mesh>);

/// A sprite turned to face the camera each frame, or along an axis to face it as nearly
/// as it can (a flame or a beam).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Billboard {
    pub axis: Option<Vec3>,
}

/// A sprite or particle living out its ticks.
#[derive(Component, Debug, Clone)]
struct Fx {
    velocity: Vec3,
    gravity: f32,
    ticks_left: u32,
    total: u32,
    /// Its scale when new and at its end.
    size: Vec2,
    end: Vec2,
    alpha: f32,
    material: Handle<StandardMaterial>,
}

/// The effects' own random numbers.
#[derive(Resource, Debug, Clone)]
struct FxRng(u32);

impl FxRng {
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A direction round `around` within `cone` degrees, or any direction.
    fn direction(&mut self, around: Option<(Vec3, f32)>) -> Vec3 {
        let (axis, cone) = around.unwrap_or((Vec3::Y, 180.0));
        let cos = 1.0 - (1.0 - ops::cos(cone.to_radians())) * self.unit();
        let sin = ops::sqrt((1.0 - cos * cos).max(0.0));
        let turn = self.unit() * std::f32::consts::TAU;
        let local = Vec3::new(sin * ops::cos(turn), sin * ops::sin(turn), cos);
        Quat::from_rotation_arc(Vec3::Z, axis.normalize_or(Vec3::Y)) * local
    }
}

/// The guns' effects. It needs Bevy's asset and render plugins for its images and quad;
/// without them (as in tests) nothing is drawn.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxRecipes>()
            .insert_resource(FxRng(0x6F1D_2C35))
            .add_systems(Startup, paint_placeholders)
            .add_systems(
                FixedUpdate,
                (emit_gun_fx, step_fx)
                    .chain()
                    .in_set(rac_sim::SimSet::Particles)
                    .run_if(resource_exists::<FxQuad>),
            )
            .add_systems(Update, face_camera);
    }
}

/// Paints a placeholder for every texture the recipes use, a plain glow for any the
/// recipes give no look.
fn paint_placeholders(
    recipes: Res<FxRecipes>,
    images: Option<ResMut<Assets<Image>>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    mut commands: Commands,
) {
    let (Some(mut images), Some(mut meshes)) = (images, meshes) else {
        commands.init_resource::<FxImages>();
        return;
    };
    let mut handles = BTreeMap::new();
    for texture in recipes.textures() {
        let paint = recipes
            .placeholders
            .get(&texture)
            .copied()
            .unwrap_or(Paint {
                shape: Shape::Glow,
                colour: [1.0, 1.0, 1.0],
            });
        handles.insert(texture, images.add(paint::image(paint)));
    }
    commands.insert_resource(FxImages(handles));
    commands.insert_resource(FxQuad(meshes.add(Rectangle::new(1.0, 1.0))));
}

/// What spawning a sprite needs.
struct Spawner<'w, 's, 'a> {
    commands: Commands<'w, 's>,
    materials: &'a mut Assets<StandardMaterial>,
    images: &'a FxImages,
    quad: Handle<Mesh>,
}

impl Spawner<'_, '_, '_> {
    fn material(
        &mut self,
        texture: Tex,
        colour: [f32; 4],
        blend: Blend,
    ) -> Handle<StandardMaterial> {
        self.materials.add(StandardMaterial {
            base_color: Color::srgba(colour[0], colour[1], colour[2], colour[3]),
            base_color_texture: self.images.0.get(&texture).cloned(),
            alpha_mode: blend.alpha_mode(),
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    }

    fn spawn(&mut self, at: Vec3, axis: Option<Vec3>, fx: Fx, name: &'static str) {
        let scale = fx.size.extend(1.0);
        self.commands.spawn((
            Name::new(name),
            DemoEntity,
            Mesh3d(self.quad.clone()),
            MeshMaterial3d(fx.material.clone()),
            Billboard { axis },
            SimTransform::from_translation(at),
            Transform::from_translation(at).with_scale(scale),
            fx,
        ));
    }

    fn emit(&mut self, emit: &Emit, at: Vec3, way: Option<Vec3>, rng: &mut FxRng) {
        match *emit {
            Emit::Flash {
                texture,
                length,
                width,
                ticks,
                colour,
                blend,
            } => {
                let way = way.unwrap_or(Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);
                let material = self.material(texture, colour, blend);
                let size = Vec2::new(length, width);
                self.spawn(
                    at + way * length / 2.0,
                    Some(way),
                    Fx {
                        velocity: Vec3::ZERO,
                        gravity: 0.0,
                        ticks_left: ticks.max(1),
                        total: ticks.max(1),
                        size,
                        end: size,
                        alpha: colour[3],
                        material,
                    },
                    "flash",
                );
            }
            Emit::Glow {
                texture,
                size,
                end_size,
                ticks,
                colour,
                blend,
            } => {
                let material = self.material(texture, colour, blend);
                self.spawn(
                    at,
                    None,
                    Fx {
                        velocity: Vec3::ZERO,
                        gravity: 0.0,
                        ticks_left: ticks.max(1),
                        total: ticks.max(1),
                        size: Vec2::splat(size),
                        end: Vec2::splat(end_size),
                        alpha: colour[3],
                        material,
                    },
                    "glow",
                );
            }
            Emit::Burst {
                texture,
                count,
                speed,
                cone,
                gravity,
                size,
                end_size,
                ticks,
                colour,
                blend,
            } => {
                let material = self.material(texture, colour, blend);
                for _ in 0..count {
                    let around = way.zip(cone);
                    let velocity = rng.direction(around) * speed * (0.4 + 0.6 * rng.unit());
                    let life = (ticks as f32 * (0.67 + 0.67 * rng.unit())).max(1.0) as u32;
                    self.spawn(
                        at,
                        None,
                        Fx {
                            velocity,
                            gravity,
                            ticks_left: life,
                            total: life,
                            size: Vec2::splat(size),
                            end: Vec2::splat(end_size),
                            alpha: colour[3],
                            material: material.clone(),
                        },
                        "particle",
                    );
                }
            }
            Emit::Set(_) => {}
        }
    }

    fn beam(&mut self, beam: &Beam, from: Vec3, to: Vec3) {
        let length = from.distance(to);
        if length < 1e-3 {
            return;
        }
        let material = self.material(beam.texture, beam.colour, beam.blend);
        let size = Vec2::new(length, beam.width);
        self.spawn(
            (from + to) / 2.0,
            Some(to - from),
            Fx {
                velocity: Vec3::ZERO,
                gravity: 0.0,
                ticks_left: beam.ticks.max(1),
                total: beam.ticks.max(1),
                size,
                end: Vec2::new(length, 0.0),
                alpha: beam.colour[3],
                material,
            },
            "beam",
        );
    }
}

/// Shows each gun event as its gun's recipe has it.
#[allow(clippy::too_many_arguments)]
fn emit_gun_fx(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    quad: Res<FxQuad>,
    mut rng: ResMut<FxRng>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut events: MessageReader<GunEvent>,
    commands: Commands,
) {
    let mut spawner = Spawner {
        commands,
        materials: &mut materials,
        images: &images,
        quad: quad.0.clone(),
    };
    for event in events.read() {
        let key = armory
            .get(event.weapon)
            .map_or("", |def| def.effects.as_str());
        match event.kind {
            GunEventKind::Fired { at, direction } => {
                for emit in recipes.expand(recipes.fire(key)) {
                    spawner.emit(emit, at, Some(direction), &mut rng);
                }
            }
            GunEventKind::Impact { at } => {
                for emit in recipes.expand(recipes.impact(key)) {
                    spawner.emit(emit, at, None, &mut rng);
                }
            }
            GunEventKind::Ray { from, to } => {
                if let Some(beam) = recipes.ray(key) {
                    spawner.beam(beam, from, to);
                }
            }
            GunEventKind::Arc { from, to } => spawner.beam(&recipes.arc, from, to),
            GunEventKind::LevelUp { .. } | GunEventKind::Empty { .. } => {}
        }
    }
}

/// Moves, grows and fades every sprite and particle a tick, dropping those whose time is up.
fn step_fx(
    mut fx: Query<(Entity, &mut Fx, &mut SimTransform, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    const TICK: f32 = 1.0 / 60.0;
    for (entity, mut fx, mut place, mut transform) in &mut fx {
        fx.ticks_left = fx.ticks_left.saturating_sub(1);
        if fx.ticks_left == 0 {
            commands.entity(entity).despawn();
            continue;
        }
        fx.velocity.y -= fx.gravity * TICK;
        place.translation += fx.velocity * TICK;
        let left = fx.ticks_left as f32 / fx.total as f32;
        transform.scale = fx.end.lerp(fx.size, left).extend(1.0);
        if let Some(mut material) = materials.get_mut(&fx.material) {
            material.base_color.set_alpha(fx.alpha * left);
        }
    }
}

/// Turns the sprites to the camera.
fn face_camera(
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut sprites: Query<(&Billboard, &mut Transform)>,
) {
    let Some((_, camera)) = cameras.iter().find(|(camera, _)| camera.is_active) else {
        return;
    };
    let (eye, facing) = (camera.translation(), camera.rotation());
    for (billboard, mut transform) in &mut sprites {
        transform.rotation = match billboard.axis {
            None => facing,
            Some(axis) => along(axis, eye - transform.translation).unwrap_or(facing),
        };
    }
}

/// The turn that lays a quad's +x along `axis` and turns its face (+z) as nearly toward
/// `view` as that allows.
fn along(axis: Vec3, view: Vec3) -> Option<Quat> {
    let x = axis.try_normalize()?;
    let z = (view - x * view.dot(x)).try_normalize()?;
    let y = z.cross(x);
    Some(Quat::from_mat3(&Mat3::from_cols(x, y, z)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_has_effects_and_every_texture_a_look() {
        let recipes = FxRecipes::default();
        let armory = Armory::uya();
        for def in &armory.weapons {
            assert!(recipes.guns.contains_key(&def.effects), "{}", def.effects);
        }
        for texture in recipes.textures() {
            assert!(recipes.placeholders.contains_key(&texture), "{texture:?}");
        }
        assert_eq!(recipes.expand(recipes.impact("n60")).count(), 3);
        assert!(recipes.ray("shock_blaster").is_some());
        for (name, emits) in &recipes.sets {
            assert!(
                !emits.iter().any(|emit| matches!(emit, Emit::Set(_))),
                "{name} nests"
            );
        }
        for fx in recipes.guns.values() {
            for emit in fx.fire.iter().chain(&fx.impact).flatten() {
                if let Emit::Set(name) = emit {
                    assert!(recipes.sets.contains_key(name), "no set {name}");
                }
            }
        }
    }

    #[test]
    fn a_beam_lies_along_its_way_and_faces_the_eye() {
        let turn = along(Vec3::X, Vec3::new(0.0, 0.0, 5.0)).unwrap();
        assert!((turn * Vec3::X).abs_diff_eq(Vec3::X, 1e-5));
        assert!((turn * Vec3::Z).abs_diff_eq(Vec3::Z, 1e-5));
        let tilted = along(Vec3::X, Vec3::new(1.0, 3.0, 0.0)).unwrap();
        assert!((tilted * Vec3::Z).abs_diff_eq(Vec3::Y, 1e-5));
        assert_eq!(along(Vec3::X, Vec3::X), None);
    }

    #[test]
    fn particles_go_within_their_cone() {
        let mut rng = FxRng(1);
        for _ in 0..200 {
            let way = rng.direction(Some((Vec3::NEG_Z, 20.0)));
            assert!((way.length() - 1.0).abs() < 1e-4);
            assert!(way.dot(Vec3::NEG_Z) >= ops::cos(20f32.to_radians()) - 1e-4);
        }
    }
}
