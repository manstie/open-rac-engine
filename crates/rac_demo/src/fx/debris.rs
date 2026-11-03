//! Pieces of debris a blast throws (0x329470; `effects.md`, "Nitro Eruptor's burst"): each
//! a model of the game's class (UYA's chunk, 0x1A50) that tumbles about two of its axes,
//! falls, bounces off the world, may trail smoke, and shrinks away over the last quarter
//! of its life (its update, 0x329678). A game gives the models ([`FxModels`]); without
//! one the kit throws a small rock of its own.

use bevy::prelude::*;
use rac_sim::SimTransform;
use serde::Deserialize;

use super::{FxRng, Spawner, Tex, Trail};
use crate::DemoEntity;

#[cfg(doc)]
use super::FxModels;

/// `count` pieces of the game's class `class`, each `scale` times its size, thrown at
/// between the two of `speed` units a second, level within `turn` degrees either way of
/// the way to the camera and between the two of `pitch` degrees up, for between the two
/// of `ticks`. Each turns between the two of `spin` degrees a tick about its x and about
/// its z, and falls at `gravity`; with `bounce`, it leaves the world it meets at that much
/// of its speed, turned off the surface. Each lays `smoke` at one of `smokes` a tick (a
/// fraction being a chance), picked at random, as the game picks one of its kinds. All are
/// thrown `delay` ticks after the event, tinted `colour` (sRGB; 1.0 is the game's 0x80).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Debris {
    pub class: u32,
    pub count: u32,
    pub scale: f32,
    pub speed: (f32, f32),
    #[serde(default = "all_round")]
    pub turn: f32,
    #[serde(default)]
    pub pitch: (f32, f32),
    pub ticks: (u32, u32),
    #[serde(default)]
    pub spin: (f32, f32),
    #[serde(default)]
    pub gravity: f32,
    #[serde(default)]
    pub bounce: Option<f32>,
    #[serde(default)]
    pub smoke: Option<Trail>,
    #[serde(default)]
    pub smokes: Vec<f32>,
    #[serde(default)]
    pub delay: u32,
    #[serde(default = "white")]
    pub colour: [f32; 3],
}

fn white() -> [f32; 3] {
    [1.0; 3]
}

fn all_round() -> f32 {
    180.0
}

impl Debris {
    pub(crate) fn textures(&self) -> Vec<Tex> {
        self.smoke
            .iter()
            .flat_map(|smoke| smoke.textures.iter().copied())
            .collect()
    }
}

/// The kit's rock, thrown for a class the game gives no model of.
#[derive(Resource, Debug, Clone)]
pub(crate) struct Rock(pub Handle<Mesh>);

/// The kit's rock: an icosahedron about the size of UYA's chunk at its class's scale.
pub(crate) fn rock() -> Mesh {
    Sphere::new(0.08)
        .mesh()
        .ico(0)
        .expect("an icosahedron of no subdivisions")
        .with_duplicated_vertices()
        .with_computed_flat_normals()
}

/// A piece in flight, or hidden where it will be thrown from for `wait` ticks.
#[derive(Component, Debug, Clone)]
pub(crate) struct Piece {
    wait: u32,
    /// Units a second.
    velocity: Vec3,
    /// Radians a tick about its x and its z.
    spin: Vec2,
    turned: Vec2,
    gravity: f32,
    bounce: Option<f32>,
    ticks_left: u32,
    total: u32,
    size: f32,
    smoke: Option<Trail>,
}

/// The share of its size a piece of `total` ticks has with `left` of them left: whole,
/// then shrinking to nothing over the last quarter of its life.
pub(crate) fn piece_share(total: u32, left: u32) -> f32 {
    let quarter = total / 4;
    if left < quarter {
        left as f32 / quarter as f32
    } else {
        1.0
    }
}

impl Spawner<'_, '_, '_> {
    /// Throws `debris` from `at`, seen from `eye`.
    pub(super) fn debris(&mut self, debris: &Debris, at: Vec3, eye: Vec3, rng: &mut FxRng) {
        let parts = self.models.0.get(&debris.class).map_or_else(
            || vec![(self.rock.clone(), None)],
            |model| model.parts.clone(),
        );
        // Lit as the level lights it, as the game's mobys are.
        let [r, g, b] = debris.colour;
        let parts: Vec<_> = parts
            .into_iter()
            .map(|(mesh, image)| {
                let material = self.materials.add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    base_color_texture: image,
                    perceptual_roughness: 1.0,
                    ..default()
                });
                (mesh, material)
            })
            .collect();
        for _ in 0..debris.count {
            let level = rng.level_within(eye - at, debris.turn);
            let pitch = rng.between(debris.pitch).to_radians();
            let speed = rng.between(debris.speed);
            let velocity = (level * ops::cos(pitch) + Vec3::Y * ops::sin(pitch)) * speed;
            let spin = Vec2::new(
                rng.between(debris.spin).to_radians(),
                rng.between(debris.spin).to_radians(),
            );
            let total = rng.ticks(debris.ticks.0, debris.ticks.1).max(1);
            let smokes = rng.pick(&debris.smokes).copied();
            let smoke = debris
                .smoke
                .as_ref()
                .zip(smokes)
                .filter(|&(_, per_tick)| per_tick > 0.0)
                .map(|(smoke, per_tick)| Trail {
                    per_tick,
                    ..smoke.clone()
                });
            let piece = self
                .commands
                .spawn((
                    Name::new("debris"),
                    DemoEntity,
                    SimTransform::from_translation(at),
                    Transform::from_translation(at).with_scale(Vec3::splat(debris.scale)),
                    if debris.delay > 0 {
                        Visibility::Hidden
                    } else {
                        Visibility::default()
                    },
                    Piece {
                        wait: debris.delay,
                        velocity,
                        spin,
                        turned: Vec2::ZERO,
                        gravity: debris.gravity,
                        bounce: debris.bounce,
                        ticks_left: total,
                        total,
                        size: debris.scale,
                        smoke,
                    },
                ))
                .id();
            for (mesh, material) in &parts {
                self.commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    ChildOf(piece),
                ));
            }
        }
    }
}

/// Flies, tumbles, bounces and shrinks each piece a tick, laying its smoke where it then
/// is, and drops those whose time is up; as 0x329678, it moves before it falls.
pub(crate) fn step_debris(
    mut pieces: Query<(
        Entity,
        &mut Piece,
        &mut SimTransform,
        &mut Transform,
        &mut Visibility,
    )>,
    mut rng: ResMut<FxRng>,
    mut making: super::Making,
) {
    const TICK: f32 = 1.0 / 60.0;
    let mut spawner = making.spawner();
    for (entity, mut piece, mut place, mut transform, mut shown) in &mut pieces {
        if piece.wait > 0 {
            piece.wait -= 1;
            if piece.wait == 0 {
                *shown = Visibility::Inherited;
            }
            continue;
        }
        if piece.ticks_left == 0 {
            spawner.commands.entity(entity).despawn();
            continue;
        }
        piece.ticks_left -= 1;
        let spin = piece.spin;
        piece.turned += spin;
        place.rotation =
            Quat::from_rotation_z(piece.turned.y) * Quat::from_rotation_x(piece.turned.x);
        let from = place.translation;
        let motion = piece.velocity * TICK;
        place.translation = from + motion;
        if let (Some(keep), Some(world)) = (piece.bounce, spawner.world)
            && let Some(hit) = world.cast_ray(from, motion)
        {
            let speed = piece.velocity.length();
            let off = piece.velocity.reflect(hit.normal);
            piece.velocity = off.normalize_or_zero() * speed * keep;
            place.translation = from + motion * hit.fraction + hit.normal * 0.01;
        }
        piece.velocity.y -= piece.gravity * TICK;
        transform.scale = Vec3::splat(piece.size * piece_share(piece.total, piece.ticks_left));
        if let Some(smoke) = &piece.smoke {
            for _ in 0..smoke.count(0.0, rng.unit()) {
                spawner.trail(smoke, place.translation, Vec3::ZERO, 1.0, &mut rng);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_piece_keeps_its_size_then_shrinks_to_nothing_over_its_last_quarter() {
        assert_eq!(piece_share(100, 100), 1.0);
        assert_eq!(piece_share(100, 25), 1.0);
        assert_eq!(piece_share(100, 20), 0.8);
        assert_eq!(piece_share(100, 0), 0.0);
        // A quarter of 130 is 32 ticks, rounded down as the game does.
        assert_eq!(piece_share(130, 16), 0.5);
    }

    #[test]
    fn the_kits_rock_is_a_faceted_stone_about_the_chunks_size() {
        let rock = rock();
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) =
            rock.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let widest = points
            .iter()
            .map(|point| Vec3::from(*point).length())
            .fold(0.0, f32::max);
        assert!((widest - 0.08).abs() < 1e-4, "{widest}");
        assert!(rock.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(rock.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
    }
}
