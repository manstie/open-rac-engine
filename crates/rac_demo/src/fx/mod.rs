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

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rac_combat::{Armory, Arsenal, GunEvent, GunEventKind, WeaponId};
use rac_hero::Hero;
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

fn whole() -> f32 {
    1.0
}

/// Where a burst's particles head.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Toward {
    /// Round the shot's way (all round where an event has none).
    #[default]
    Way,
    /// Round the way to the camera, as hits throw their sparks back at the eye.
    Camera,
    /// Level, any way round, as a ring along the ground.
    Level,
}

/// One thing an event shows. Sizes are in world units, times in 60 Hz ticks, colours sRGB
/// with alpha (1 shows the texture as it is), angles in degrees.
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
    /// A sprite facing the camera, `size` across growing to `end_size` as its alpha goes to
    /// `end_alpha`, turned by an angle within `roll` and turning `spin` a tick, and going
    /// `ahead` units a second along the shot's way; shown `delay` ticks after the event.
    Glow {
        texture: Tex,
        size: f32,
        end_size: f32,
        ticks: u32,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        end_alpha: f32,
        #[serde(default)]
        roll: Option<(f32, f32)>,
        #[serde(default)]
        spin: f32,
        #[serde(default)]
        ahead: f32,
        #[serde(default)]
        delay: u32,
        #[serde(default)]
        blend: Blend,
    },
    /// `count` particles thrown out at up to `speed` units a second (from `speed_from`, or
    /// 0.4 of it), all round or within `cone` degrees of the way they head, falling at
    /// `gravity` and keeping `keep` of their speed each tick. Each is `size` across (or
    /// between it and `size_to`), changing to `end_size` in proportion while its alpha goes
    /// to `end_alpha` and its colour to `end_colour` over `ticks` (to `ticks_to`, or give
    /// or take a third). They start at any roll and turn up to `spin` a tick either way.
    /// They start spread over `line` back along the shot's way, and up to `jitter` from
    /// where they would. With a `streak` length each is drawn that long along its way,
    /// `size` across. Each shows `delay` ticks after the event, plus up to `over` more.
    Burst {
        texture: Tex,
        count: u32,
        speed: f32,
        #[serde(default)]
        speed_from: Option<f32>,
        #[serde(default)]
        cone: Option<f32>,
        #[serde(default)]
        toward: Toward,
        #[serde(default)]
        gravity: f32,
        #[serde(default = "whole")]
        keep: f32,
        size: f32,
        #[serde(default)]
        size_to: Option<f32>,
        end_size: f32,
        ticks: u32,
        #[serde(default)]
        ticks_to: Option<u32>,
        #[serde(default = "opaque")]
        colour: [f32; 4],
        #[serde(default)]
        end_alpha: f32,
        #[serde(default)]
        end_colour: Option<[f32; 3]>,
        #[serde(default)]
        spin: f32,
        #[serde(default)]
        line: f32,
        #[serde(default)]
        jitter: f32,
        #[serde(default)]
        streak: Option<f32>,
        #[serde(default)]
        delay: u32,
        #[serde(default)]
        over: u32,
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

/// A beam from one point to another, `width` across, fading over `ticks` as it narrows
/// to `end_width` (to nothing if not given).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Beam {
    pub texture: Tex,
    pub width: f32,
    #[serde(default)]
    pub end_width: Option<f32>,
    pub ticks: u32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// One of the sprites drawn on each of a gun's shots in flight, in place of the kit's
/// ball: turning `spin` degrees a tick, or to a new roll each tick if it `flickers`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ShotSprite {
    pub texture: Tex,
    pub size: f32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub spin: f32,
    #[serde(default)]
    pub flickers: bool,
    #[serde(default)]
    pub blend: Blend,
}

/// A sprite laid along a shot's way once it has flown `after` units: from `behind` it
/// back, `length` long and `width` across, each picked afresh every tick between the two.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Streak {
    pub texture: Tex,
    pub behind: f32,
    pub length: (f32, f32),
    pub width: (f32, f32),
    #[serde(default)]
    pub after: f32,
    #[serde(default = "opaque")]
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// Particles a shot leaves along its way: `per_tick` a tick (a fraction being a chance)
/// and one for every `spacing` units of the tick's path, spread along it. Each is in one of `textures` and
/// `colours`, growing from `size` to `end_size` as its alpha goes to `end_alpha` and its
/// colour to `end_colour` over `ticks` to `ticks_to`. It sets off rising at `rise` units
/// a second, going back at `back` times the shot's speed and up to `scatter` units a
/// second either way along each axis; then falls at `gravity`, keeps `keep` of its speed
/// each tick and turns up to `spin` degrees a tick either way.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Trail {
    #[serde(default)]
    pub per_tick: f32,
    #[serde(default)]
    pub spacing: f32,
    pub textures: Vec<Tex>,
    pub colours: Vec<[f32; 4]>,
    pub size: f32,
    pub end_size: f32,
    pub ticks: u32,
    pub ticks_to: u32,
    #[serde(default)]
    pub rise: (f32, f32),
    #[serde(default)]
    pub back: f32,
    #[serde(default)]
    pub scatter: f32,
    #[serde(default)]
    pub gravity: f32,
    #[serde(default = "whole")]
    pub keep: f32,
    #[serde(default)]
    pub spin: f32,
    #[serde(default)]
    pub end_alpha: f32,
    #[serde(default)]
    pub end_colour: Option<[f32; 3]>,
    #[serde(default)]
    pub blend: Blend,
}

impl Trail {
    /// How many it lays over a tick's path `length` long, `roll` (0 to 1) deciding the
    /// fraction of `per_tick`.
    fn count(&self, length: f32, roll: f32) -> u32 {
        let spaced = if self.spacing > 0.0 {
            (length / self.spacing) as u32
        } else {
            0
        };
        self.per_tick as u32 + u32::from(roll < self.per_tick.fract()) + spaced
    }
}

/// A gun's effects; what it leaves out it takes from the defaults. From each level in
/// `from_level` (0 is V1), what that level's entry gives takes the place of this.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
pub struct GunFx {
    #[serde(default)]
    pub fire: Option<Vec<Emit>>,
    #[serde(default)]
    pub shot: Option<Vec<ShotSprite>>,
    #[serde(default)]
    pub streak: Option<Streak>,
    #[serde(default)]
    pub trail: Option<Vec<Trail>>,
    /// Beams laid along each tick of a shot's way, as the Flux Rifle's.
    #[serde(default)]
    pub wake: Option<Vec<Beam>>,
    #[serde(default)]
    pub impact: Option<Vec<Emit>>,
    #[serde(default)]
    pub ray: Option<Vec<Beam>>,
    /// What shows where each of a hitscan's rays stops.
    #[serde(default)]
    pub ray_end: Option<Vec<Emit>>,
    /// Whether the gun's blasts show as the kit's sphere of their reach (the default), or
    /// only through their `impact`, as the game draws the Nitro's.
    #[serde(default)]
    pub blast_sphere: Option<bool>,
    #[serde(default)]
    pub from_level: BTreeMap<u8, GunFx>,
}

impl GunFx {
    /// What `part` gives at `level`: the highest of `from_level`'s up to it that gives it,
    /// else this.
    fn at<'a, T: ?Sized>(
        &'a self,
        level: u8,
        part: impl Fn(&'a GunFx) -> Option<&'a T>,
    ) -> Option<&'a T> {
        self.from_level
            .range(..=level)
            .rev()
            .find_map(|(_, fx)| part(fx))
            .or_else(|| part(self))
    }
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

    /// What `part` gives for gun `key` at `level`, else the defaults'.
    fn part<'a, T: ?Sized>(
        &'a self,
        key: &str,
        level: u8,
        part: impl Fn(&'a GunFx) -> Option<&'a T> + Copy,
    ) -> Option<&'a T> {
        let own = self.gun(key).and_then(|gun| gun.at(level, part));
        own.or_else(|| self.default.at(level, part))
    }

    pub fn fire(&self, key: &str, level: u8) -> &[Emit] {
        self.part(key, level, |fx| fx.fire.as_deref())
            .unwrap_or(&[])
    }

    pub fn impact(&self, key: &str, level: u8) -> &[Emit] {
        self.part(key, level, |fx| fx.impact.as_deref())
            .unwrap_or(&[])
    }

    pub fn ray(&self, key: &str, level: u8) -> &[Beam] {
        self.part(key, level, |fx| fx.ray.as_deref()).unwrap_or(&[])
    }

    pub fn ray_end(&self, key: &str, level: u8) -> &[Emit] {
        self.part(key, level, |fx| fx.ray_end.as_deref())
            .unwrap_or(&[])
    }

    /// A shot's sprites; none, and it is drawn as the kit's ball.
    pub fn shot(&self, key: &str, level: u8) -> &[ShotSprite] {
        self.gun(key)
            .and_then(|gun| gun.at(level, |fx| fx.shot.as_deref()))
            .unwrap_or(&[])
    }

    pub fn streak(&self, key: &str, level: u8) -> Option<&Streak> {
        self.gun(key)
            .and_then(|gun| gun.at(level, |fx| fx.streak.as_ref()))
    }

    pub fn trail(&self, key: &str, level: u8) -> &[Trail] {
        self.gun(key)
            .and_then(|gun| gun.at(level, |fx| fx.trail.as_deref()))
            .unwrap_or(&[])
    }

    pub fn blast_sphere(&self, key: &str, level: u8) -> bool {
        self.part(key, level, |fx| fx.blast_sphere.as_ref())
            .copied()
            .unwrap_or(true)
    }

    pub fn wake(&self, key: &str, level: u8) -> &[Beam] {
        self.gun(key)
            .and_then(|gun| gun.at(level, |fx| fx.wake.as_deref()))
            .unwrap_or(&[])
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
        fn gun(fx: &GunFx, textures: &mut Vec<Tex>) {
            let beams = fx.ray.iter().chain(&fx.wake).flatten();
            textures.extend(beams.map(|beam| beam.texture));
            textures.extend(fx.shot.iter().flatten().map(|shot| shot.texture));
            textures.extend(fx.streak.as_ref().map(|streak| streak.texture));
            let trails = fx.trail.iter().flatten();
            textures.extend(trails.flat_map(|trail| trail.textures.iter().copied()));
            for emits in fx.fire.iter().chain(&fx.impact).chain(&fx.ray_end) {
                textures.extend(emits.iter().filter_map(Emit::texture));
            }
            for fx in fx.from_level.values() {
                gun(fx, textures);
            }
        }
        let mut textures = vec![self.arc.texture];
        for fx in std::iter::once(&self.default).chain(self.guns.values()) {
            gun(fx, &mut textures);
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

/// A sprite turned to face the camera each frame, rolled by `roll` radians, or along an
/// axis to face it as nearly as it can (a flame or a beam).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Billboard {
    pub axis: Option<Vec3>,
    pub roll: f32,
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
    /// Its alpha when new and at its end.
    alpha: f32,
    end_alpha: f32,
    /// Its colour when new and at its end, if that changes.
    fade: Option<(Vec3, Vec3)>,
    /// The share of its velocity it keeps each tick.
    keep: f32,
    /// Ticks before it shows.
    wait: u32,
    /// Radians a tick.
    spin: f32,
    material: Handle<StandardMaterial>,
}

impl Fx {
    /// One that stays where it is put.
    fn still(
        ticks: u32,
        size: Vec2,
        end: Vec2,
        alpha: f32,
        material: Handle<StandardMaterial>,
    ) -> Self {
        Fx {
            velocity: Vec3::ZERO,
            gravity: 0.0,
            ticks_left: ticks.max(1),
            total: ticks.max(1),
            size,
            end,
            alpha,
            end_alpha: 0.0,
            fade: None,
            keep: 1.0,
            wait: 0,
            spin: 0.0,
            material,
        }
    }
}

/// One of a shot's sprites, turning `spin` radians a tick, or to a new roll each tick if
/// it `flickers`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShotLayer {
    pub spin: f32,
    pub flickers: bool,
}

/// The streak laid along a shot's way.
#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct ShotStreak(pub Streak);

/// The effects' own random numbers.
#[derive(Resource, Debug, Clone)]
pub(crate) struct FxRng(u32);

impl FxRng {
    pub(crate) fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }

    pub(crate) fn between(&mut self, (low, high): (f32, f32)) -> f32 {
        low + (high - low) * self.unit()
    }

    /// A whole number from `low` to `high`, both included.
    pub(crate) fn ticks(&mut self, low: u32, high: u32) -> u32 {
        let span = high.saturating_sub(low) + 1;
        low + ((self.unit() * span as f32) as u32).min(span - 1)
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        let index = (self.unit() * items.len() as f32) as usize;
        items.get(index.min(items.len().saturating_sub(1)))
    }

    /// A roll within `range` degrees, in radians.
    pub(crate) fn roll(&mut self, range: Option<(f32, f32)>) -> f32 {
        range.map_or(0.0, |range| self.between(range).to_radians())
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
            .init_resource::<TrailFrom>()
            .add_systems(Startup, paint_placeholders)
            .add_systems(
                FixedUpdate,
                (emit_gun_fx, emit_trails, step_fx)
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

    fn spawn(&mut self, at: Vec3, billboard: Billboard, fx: Fx, name: &'static str) {
        // Hidden while it waits.
        let scale = if fx.wait > 0 {
            Vec3::ZERO
        } else {
            fx.size.extend(1.0)
        };
        // Bevy's shadow pass takes blended meshes too, and the game's effects cast none.
        self.commands.spawn((
            Name::new(name),
            DemoEntity,
            Mesh3d(self.quad.clone()),
            MeshMaterial3d(fx.material.clone()),
            NotShadowCaster,
            billboard,
            SimTransform::from_translation(at),
            Transform::from_translation(at).with_scale(scale),
            fx,
        ));
    }

    /// Shows `emit` at `at`, where a shot heading `way` was fired or landed, seen from
    /// `eye`.
    fn emit(&mut self, emit: &Emit, at: Vec3, way: Option<Vec3>, eye: Vec3, rng: &mut FxRng) {
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
                let billboard = Billboard {
                    axis: Some(way),
                    roll: 0.0,
                };
                let fx = Fx::still(ticks, size, size, colour[3], material);
                self.spawn(at + way * length / 2.0, billboard, fx, "flash");
            }
            Emit::Glow {
                texture,
                size,
                end_size,
                ticks,
                colour,
                end_alpha,
                roll,
                spin,
                ahead,
                delay,
                blend,
            } => {
                let material = self.material(texture, colour, blend);
                let billboard = Billboard {
                    axis: None,
                    roll: rng.roll(roll),
                };
                let fx = Fx {
                    velocity: way.map_or(Vec3::ZERO, Vec3::normalize_or_zero) * ahead,
                    wait: delay,
                    end_alpha,
                    spin: spin.to_radians(),
                    ..Fx::still(
                        ticks,
                        Vec2::splat(size),
                        Vec2::splat(end_size),
                        colour[3],
                        material,
                    )
                };
                self.spawn(at, billboard, fx, "glow");
            }
            Emit::Burst {
                texture,
                count,
                speed,
                speed_from,
                cone,
                toward,
                gravity,
                keep,
                size,
                size_to,
                end_size,
                ticks,
                ticks_to,
                colour,
                end_alpha,
                end_colour,
                spin,
                line,
                jitter,
                streak,
                delay,
                over,
                blend,
            } => {
                // Each fades on its own life, so each has its own material.
                let heading = match toward {
                    Toward::Way => way,
                    Toward::Camera => Some(eye - at),
                    Toward::Level => None,
                };
                let back = way.map_or(Vec3::ZERO, |way| -way.normalize_or_zero());
                for index in 0..count {
                    let around = heading.zip(cone);
                    let speed = rng.between((speed_from.unwrap_or(0.4 * speed), speed));
                    let mut velocity = rng.direction(around);
                    if toward == Toward::Level {
                        velocity = Vec3::new(velocity.x, 0.0, velocity.z).normalize_or(Vec3::X);
                    }
                    let velocity = velocity * speed;
                    let life = match ticks_to {
                        Some(to) => rng.ticks(ticks, to),
                        None => (ticks as f32 * (0.67 + 0.67 * rng.unit())).max(1.0) as u32,
                    };
                    let along = if count > 1 {
                        line * index as f32 / (count - 1) as f32
                    } else {
                        0.0
                    };
                    let from = at + back * along + rng.direction(None) * jitter * rng.unit();
                    let start = size_to.map_or(size, |to| rng.between((size, to)));
                    let end = end_size * start / size;
                    let (billboard, start, end) = match streak {
                        Some(length) => (
                            Billboard {
                                axis: Some(velocity.normalize_or(Vec3::Y)),
                                roll: 0.0,
                            },
                            Vec2::new(length, start),
                            Vec2::new(length, end),
                        ),
                        None => (
                            Billboard {
                                axis: None,
                                roll: rng.roll(Some((0.0, 360.0))),
                            },
                            Vec2::splat(start),
                            Vec2::splat(end),
                        ),
                    };
                    let fx = Fx {
                        velocity,
                        gravity,
                        keep,
                        wait: delay + rng.ticks(0, over),
                        end_alpha,
                        fade: fade(colour, end_colour),
                        spin: rng.between((-spin, spin)).to_radians(),
                        ..Fx::still(
                            life,
                            start,
                            end,
                            colour[3],
                            self.material(texture, colour, blend),
                        )
                    };
                    self.spawn(from, billboard, fx, "particle");
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
        let billboard = Billboard {
            axis: Some(to - from),
            roll: 0.0,
        };
        let fx = Fx::still(
            beam.ticks,
            size,
            Vec2::new(length, beam.end_width.unwrap_or(0.0)),
            beam.colour[3],
            material,
        );
        self.spawn((from + to) / 2.0, billboard, fx, "beam");
    }
}

/// A colour's fade to `end`, if it has one.
fn fade(colour: [f32; 4], end: Option<[f32; 3]>) -> Option<(Vec3, Vec3)> {
    end.map(|end| (Vec3::new(colour[0], colour[1], colour[2]), Vec3::from(end)))
}

/// The eye the effects are seen from, for what turns to it: the active 3D camera's.
fn eye(cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>) -> Vec3 {
    cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map_or(Vec3::ZERO, |(_, place)| place.translation())
}

/// The level of `player`'s `weapon`, 0 (V1) if they have none.
fn level(heroes: &Query<(&Hero, &Arsenal)>, player: usize, weapon: WeaponId) -> u8 {
    heroes
        .iter()
        .filter(|(hero, _)| hero.player == player)
        .find_map(|(_, arsenal)| arsenal.gun(weapon))
        .map_or(0, |gun| gun.level)
}

/// Shows each gun event as its gun's recipe has it at the gun's level.
#[allow(clippy::too_many_arguments)]
fn emit_gun_fx(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    quad: Res<FxQuad>,
    heroes: Query<(&Hero, &Arsenal)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
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
    let eye = eye(&cameras);
    for event in events.read() {
        let key = armory
            .get(event.weapon)
            .map_or("", |def| def.effects.as_str());
        let level = level(&heroes, event.player, event.weapon);
        match event.kind {
            GunEventKind::Fired { at, direction } => {
                for emit in recipes.expand(recipes.fire(key, level)) {
                    spawner.emit(emit, at, Some(direction), eye, &mut rng);
                }
            }
            GunEventKind::Impact { at } => {
                for emit in recipes.expand(recipes.impact(key, level)) {
                    spawner.emit(emit, at, None, eye, &mut rng);
                }
            }
            GunEventKind::Ray { from, to } => {
                for beam in recipes.ray(key, level) {
                    spawner.beam(beam, from, to);
                }
                for emit in recipes.expand(recipes.ray_end(key, level)) {
                    spawner.emit(emit, to, Some(to - from), eye, &mut rng);
                }
            }
            GunEventKind::Arc { from, to } => spawner.beam(&recipes.arc, from, to),
            GunEventKind::LevelUp { .. } | GunEventKind::Empty { .. } => {}
        }
    }
}

/// Where each shot with a trail was last tick, by its hero's player and its serial.
#[derive(Resource, Debug, Default)]
struct TrailFrom(BTreeMap<(usize, u32), Vec3>);

/// Lays each shot's trail along the way it flew this tick.
#[allow(clippy::too_many_arguments)]
fn emit_trails(
    recipes: Res<FxRecipes>,
    armory: Res<Armory>,
    images: Res<FxImages>,
    quad: Res<FxQuad>,
    heroes: Query<(&Hero, &Arsenal)>,
    mut from: ResMut<TrailFrom>,
    mut rng: ResMut<FxRng>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    commands: Commands,
) {
    let mut spawner = Spawner {
        commands,
        materials: &mut materials,
        images: &images,
        quad: quad.0.clone(),
    };
    let mut now = BTreeMap::new();
    for (hero, arsenal) in &heroes {
        for shot in &arsenal.shots {
            let key = armory
                .get(shot.weapon)
                .map_or("", |def| def.effects.as_str());
            let trails = recipes.trail(key, shot.level);
            let wake = recipes.wake(key, shot.level);
            if trails.is_empty() && wake.is_empty() {
                continue;
            }
            let id = (hero.player, shot.serial);
            now.insert(id, shot.position);
            let Some(&last) = from.0.get(&id) else {
                continue;
            };
            for beam in wake {
                spawner.beam(beam, last, shot.position);
            }
            let path = shot.position - last;
            for trail in trails {
                let count = trail.count(path.length(), rng.unit());
                for index in 0..count {
                    let (Some(&texture), Some(&colour)) =
                        (rng.pick(&trail.textures), rng.pick(&trail.colours))
                    else {
                        break;
                    };
                    let at = last + path * (index + 1) as f32 / count as f32;
                    let life = rng.ticks(trail.ticks, trail.ticks_to);
                    let billboard = Billboard {
                        axis: None,
                        roll: rng.roll(Some((0.0, 360.0))),
                    };
                    let scatter = (-trail.scatter, trail.scatter);
                    let scatter = Vec3::new(
                        rng.between(scatter),
                        rng.between(scatter),
                        rng.between(scatter),
                    );
                    // The path is one tick's, so 60 times it is the shot's velocity.
                    let back = -path * 60.0 * trail.back;
                    let fx = Fx {
                        velocity: Vec3::Y * rng.between(trail.rise) + back + scatter,
                        gravity: trail.gravity,
                        keep: trail.keep,
                        end_alpha: trail.end_alpha,
                        fade: fade(colour, trail.end_colour),
                        spin: rng.between((-trail.spin, trail.spin)).to_radians(),
                        ..Fx::still(
                            life,
                            Vec2::splat(trail.size),
                            Vec2::splat(trail.end_size),
                            colour[3],
                            spawner.material(texture, colour, trail.blend),
                        )
                    };
                    spawner.spawn(at, billboard, fx, "trail");
                }
            }
        }
    }
    from.0 = now;
}

/// Moves, grows, turns and fades every sprite and particle a tick, dropping those whose
/// time is up.
fn step_fx(
    mut fx: Query<(
        Entity,
        &mut Fx,
        &mut Billboard,
        &mut SimTransform,
        &mut Transform,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    const TICK: f32 = 1.0 / 60.0;
    for (entity, mut fx, mut billboard, mut place, mut transform) in &mut fx {
        // Drawn first as it is at its age, as the game draws a particle the frame it is
        // made: one of `ticks` ticks shows for that many.
        if fx.wait > 0 {
            fx.wait -= 1;
            continue;
        }
        if fx.ticks_left == 0 {
            commands.entity(entity).despawn();
            continue;
        }
        let left = fx.ticks_left as f32 / fx.total as f32;
        transform.scale = fx.end.lerp(fx.size, left).extend(1.0);
        if let Some(mut material) = materials.get_mut(&fx.material) {
            let alpha = fx.end_alpha + (fx.alpha - fx.end_alpha) * left;
            if let Some((from, to)) = fx.fade {
                let colour = to.lerp(from, left);
                material.base_color = Color::srgb(colour.x, colour.y, colour.z);
            }
            material.base_color.set_alpha(alpha);
        }
        fx.ticks_left -= 1;
        fx.velocity.y -= fx.gravity * TICK;
        let keep = fx.keep;
        fx.velocity *= keep;
        place.translation += fx.velocity * TICK;
        billboard.roll += fx.spin;
    }
}

/// Turns the sprites to the camera.
fn face_camera(
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut sprites: Query<(&Billboard, &mut Transform, &GlobalTransform)>,
) {
    let Some((_, camera)) = cameras.iter().find(|(camera, _)| camera.is_active) else {
        return;
    };
    let (eye, facing) = (camera.translation(), camera.rotation());
    for (billboard, mut transform, place) in &mut sprites {
        let rolled = facing * Quat::from_rotation_z(billboard.roll);
        transform.rotation = match billboard.axis {
            None => rolled,
            Some(axis) => along(axis, eye - place.translation()).unwrap_or(rolled),
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
        assert!(recipes.expand(recipes.impact("n60", 0)).count() > 0);
        assert!(!recipes.ray("shock_blaster", 0).is_empty());
        // The game draws the Nitro's blast only as its effects; the kit's own keep the
        // sphere.
        assert!(!recipes.blast_sphere("nitro", 2));
        assert!(recipes.blast_sphere("agents", 2));
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
    fn from_a_level_its_entry_takes_the_place_of_the_parts_it_gives() {
        let recipes: FxRecipes = ron::from_str(
            r#"(
                placeholders: {},
                default: (ray: Some([(texture: Effect(43), width: 0.1, ticks: 1)])),
                arc: (texture: Effect(47), width: 0.1, ticks: 1),
                guns: {
                    "gun": (
                        fire: Some([]),
                        impact: Some([Set("a")]),
                        from_level: {
                            2: (impact: Some([])),
                            4: (fire: Some([Set("b")]), impact: Some([Set("c"), Set("d")])),
                        },
                    ),
                },
            )"#,
        )
        .unwrap();
        let impacts: Vec<_> = (0..6)
            .map(|level| recipes.impact("gun", level).len())
            .collect();
        assert_eq!(impacts, [1, 1, 0, 0, 2, 2]);
        assert_eq!(recipes.fire("gun", 3).len(), 0);
        assert_eq!(recipes.fire("gun", 4).len(), 1);
        // What no level gives comes from the defaults.
        assert!(!recipes.ray("gun", 5).is_empty());
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
    fn a_trail_lays_one_each_spacing_and_its_share_of_a_tick() {
        let trail: Trail = ron::from_str(
            "(per_tick: 0.25, spacing: 0.1, textures: [], colours: [], size: 1.0, \
             end_size: 1.0, ticks: 1, ticks_to: 1)",
        )
        .unwrap();
        // The N60's bullet flies 1.25 u a tick at 75 u/s: twelve along it.
        assert_eq!(trail.count(1.25, 0.5), 12);
        assert_eq!(trail.count(1.25, 0.2), 13);
        assert_eq!(trail.count(0.0, 0.9), 0);
        assert_eq!(trail.keep, 1.0, "keeps all its speed unless it says");
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
