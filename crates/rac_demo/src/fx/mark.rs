//! What shots leave on the world where they end: the Lava Gun's burning pools (0x3063C8,
//! drawn by 0x306838; `effects.md`, "Lava Gun"), glows lying on the surface struck and
//! flames standing on it, which grow, burn a while and fade.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use serde::Deserialize;

use super::{Billboard, Blend, FxRng, Spawner, Tex, fx_material};
use crate::DemoEntity;

/// What a shot leaves where it ends on the world, one time in `chance`, while fewer than
/// `most` of its gun's are shown: glows lying on the surface (`flat`) and flames standing
/// on it. Its strength follows the game's spring (`game_spring` 0x445168: `spring` is its
/// gain, damping and most change a tick) up to 1, holds there `hold` ticks and springs
/// back to 0; the flames grow with it on the way up, and everything fades with it on the
/// way down.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Mark {
    pub chance: u32,
    pub most: usize,
    pub spring: (f32, f32, f32),
    pub hold: u32,
    #[serde(default)]
    pub flat: Vec<Flat>,
    #[serde(default)]
    pub flames: Vec<Flame>,
}

/// A glow lying on the surface, `size` across.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Flat {
    pub texture: Tex,
    pub size: f32,
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

/// A flame standing up to `spread` from the mark's middle either way along each level
/// axis: `width` (between the two) across at its foot and `top` of that at its top,
/// `height` (between the two) high, its colour fading out up it. Its texture runs up it
/// and scrolls up by `scroll` (between the two) of its height a tick, mirrored half the
/// time.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Flame {
    pub texture: Tex,
    pub width: (f32, f32),
    pub top: f32,
    pub height: (f32, f32),
    #[serde(default)]
    pub spread: f32,
    #[serde(default)]
    pub scroll: (f32, f32),
    pub colour: [f32; 4],
    #[serde(default)]
    pub blend: Blend,
}

impl Mark {
    pub(crate) fn textures(&self) -> impl Iterator<Item = Tex> + '_ {
        let flat = self.flat.iter().map(|flat| flat.texture);
        flat.chain(self.flames.iter().map(|flame| flame.texture))
    }
}

/// Where a mark is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Grow,
    /// With the ticks left.
    Hold(u32),
    Fade,
}

/// A mark's strength through its life, as the game's spring takes it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Life {
    strength: f32,
    rate: f32,
    phase: Phase,
    spring: (f32, f32, f32),
    hold: u32,
}

impl Life {
    fn new(mark: &Mark) -> Self {
        Self {
            strength: 0.0,
            rate: 0.0,
            phase: Phase::Grow,
            spring: mark.spring,
            hold: mark.hold,
        }
    }

    /// A tick on; false once it has faded out.
    fn step(&mut self) -> bool {
        match self.phase {
            Phase::Grow => {
                spring(&mut self.strength, &mut self.rate, 1.0, self.spring);
                if self.strength == 1.0 {
                    self.phase = Phase::Hold(self.hold);
                }
            }
            Phase::Hold(left) if left <= 1 => self.phase = Phase::Fade,
            Phase::Hold(left) => self.phase = Phase::Hold(left - 1),
            Phase::Fade => {
                spring(&mut self.strength, &mut self.rate, 0.0, self.spring);
                return self.strength > 0.0;
            }
        }
        true
    }

    /// How much of their full size the flames show.
    fn size(&self) -> f32 {
        if self.phase == Phase::Grow {
            self.strength
        } else {
            1.0
        }
    }

    /// How much of their alpha everything shows.
    fn alpha(&self) -> f32 {
        if self.phase == Phase::Fade {
            self.strength
        } else {
            1.0
        }
    }
}

/// `game_spring` (0x445168, rate step 0x445010): the rate kept within the way left, gaining
/// `gain` of it and losing `damping` of itself, kept within `cap`, and within the way left
/// again; the value snaps to `target` within a hundredth of `cap`.
fn spring(value: &mut f32, rate: &mut f32, target: f32, (gain, damping, cap): (f32, f32, f32)) {
    let error = target - *value;
    let left = error.abs();
    *rate = rate.clamp(-left, left);
    *rate += gain * error - damping * *rate;
    if cap > 0.0 {
        *rate = rate.clamp(-cap, cap);
    }
    *rate = rate.clamp(-left, left);
    *value += *rate;
    if (target - *value).abs() < cap * 0.01 {
        *value = target;
    }
}

/// One glow or flame of a mark.
#[derive(Component, Debug, Clone)]
pub(crate) struct MarkPiece {
    /// Its gun's effects key; one piece of each mark leads it, to count the gun's marks.
    pub key: String,
    pub lead: bool,
    life: Life,
    alpha: f32,
    material: Handle<StandardMaterial>,
    flame: Option<Standing>,
}

/// A flame's foot, full size (height by width) and scroll a tick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Standing {
    foot: Vec3,
    size: Vec2,
    scroll: f32,
}

impl Spawner<'_, '_, '_> {
    /// Leaves `mark` of gun `key` at `at` on a surface facing `normal`.
    pub(crate) fn mark(&mut self, mark: &Mark, key: &str, at: Vec3, normal: Vec3, rng: &mut FxRng) {
        let normal = normal.normalize_or(Vec3::Y);
        let life = Life::new(mark);
        let mut lead = true;
        let mut piece = |alpha, material, flame| {
            let piece = MarkPiece {
                key: key.to_owned(),
                lead,
                life,
                alpha,
                material,
                flame,
            };
            lead = false;
            piece
        };
        for flat in &mark.flat {
            let material = fx_material(
                self.materials,
                self.images,
                flat.texture,
                flat.colour,
                flat.blend,
            );
            // Lifted off the surface, which the game draws it right on.
            let place = Transform::from_translation(at + normal * 0.02)
                .with_rotation(Quat::from_rotation_arc(Vec3::Z, normal))
                .with_scale(Vec3::new(flat.size, flat.size, 1.0));
            self.commands.spawn((
                Name::new("mark"),
                DemoEntity,
                Mesh3d(self.quad.clone()),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                place,
                piece(flat.colour[3], material, None),
            ));
        }
        for flame in &mark.flames {
            let material = fx_material(
                self.materials,
                self.images,
                flame.texture,
                flame.colour,
                flame.blend,
            );
            if rng.unit() < 0.5
                && let Some(mut mirrored) = self.materials.get_mut(&material)
            {
                mirrored.uv_transform =
                    Affine2::from_scale_angle_translation(Vec2::new(1.0, -1.0), 0.0, Vec2::Y);
            }
            let spread = (-flame.spread, flame.spread);
            let foot = at + Vec3::new(rng.between(spread), 0.0, rng.between(spread));
            let standing = Standing {
                foot,
                size: Vec2::new(rng.between(flame.height), rng.between(flame.width)),
                scroll: rng.between(flame.scroll),
            };
            self.commands.spawn((
                Name::new("mark flame"),
                DemoEntity,
                Mesh3d(self.meshes.add(flame_mesh(flame.top))),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                Billboard {
                    axis: Some(Vec3::Y),
                    roll: 0.0,
                },
                // Nothing until it grows.
                Transform::from_translation(foot).with_scale(Vec3::ZERO),
                piece(flame.colour[3], material, Some(standing)),
            ));
        }
    }
}

/// How many marks each gun shows now.
pub(crate) fn shown(pieces: &Query<&MarkPiece>) -> std::collections::BTreeMap<String, usize> {
    let mut shown = std::collections::BTreeMap::new();
    for piece in pieces.iter().filter(|piece| piece.lead) {
        *shown.entry(piece.key.clone()).or_insert(0) += 1;
    }
    shown
}

/// Takes each mark a tick through its life: the flames' size and scroll, everything's
/// alpha; dropping those that have faded out.
pub(crate) fn step_marks(
    mut pieces: Query<(Entity, &mut MarkPiece, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (entity, mut piece, mut transform) in &mut pieces {
        if !piece.life.step() {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(standing) = piece.flame {
            let size = standing.size * piece.life.size();
            transform.scale = size.extend(1.0);
            transform.translation = standing.foot + Vec3::Y * size.x / 2.0;
        }
        if let Some(mut material) = materials.get_mut(&piece.material) {
            material
                .base_color
                .set_alpha(piece.alpha * piece.life.alpha());
            if let Some(standing) = piece.flame {
                // The texture's u runs up the flame; taking from u moves it up.
                material.uv_transform.translation.x -= standing.scroll;
            }
        }
    }
}

/// A flame one unit high and wide at its foot, `top` as wide at its top, standing along +x
/// from its middle as [`Billboard`]s with an axis do, its alpha going from 1 at its foot to
/// 0 at its top; u runs up it.
fn flame_mesh(top: f32) -> Mesh {
    let corners = [
        (-0.5, -0.5),
        (-0.5, 0.5),
        (0.5, -0.5 * top),
        (0.5, 0.5 * top),
    ];
    let positions: Vec<[f32; 3]> = corners.iter().map(|&(x, y)| [x, y, 0.0]).collect();
    let uvs: Vec<[f32; 2]> = corners
        .iter()
        .map(|&(x, y)| [x + 0.5, if y < 0.0 { 1.0 } else { 0.0 }])
        .collect();
    let colours: Vec<[f32; 4]> = corners
        .iter()
        .map(|&(x, _)| [1.0, 1.0, 1.0, if x < 0.0 { 1.0 } else { 0.0 }])
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(vec![0, 2, 1, 1, 2, 3]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> Mark {
        Mark {
            chance: 11,
            most: 10,
            spring: (0.002, 0.0002, 0.04),
            hold: 60,
            flat: Vec::new(),
            flames: Vec::new(),
        }
    }

    #[test]
    fn a_pool_grows_over_36_ticks_burns_60_and_fades_over_46() {
        // The Lava Gun's spring to 0.5 and back, at twice the scale.
        let mut life = Life::new(&pool());
        let mut ticks = 0;
        while life.phase == Phase::Grow {
            assert!(life.step());
            ticks += 1;
        }
        assert_eq!(ticks, 36);
        // It eases in: a quarter of the way through it is under a tenth grown.
        let mut early = Life::new(&pool());
        (0..9).for_each(|_| _ = early.step());
        assert!(early.size() < 0.1, "{}", early.size());
        let mut held = 0;
        while life.phase != Phase::Fade {
            assert_eq!((life.size(), life.alpha()), (1.0, 1.0));
            assert!(life.step());
            held += 1;
        }
        assert_eq!(held, 60);
        // The spring keeps the rate it reached growing, as the game's (one rate for both
        // ways), so it swells a little before it fades.
        let mut fading = 0;
        let mut most: f32 = 0.0;
        while life.step() {
            assert_eq!(life.size(), 1.0);
            most = most.max(life.alpha());
            fading += 1;
        }
        assert_eq!(fading + 1, 46);
        assert!(most > 1.05 && most < 1.08, "{most}");
    }

    #[test]
    fn a_flame_narrows_to_its_top_and_fades_up_it() {
        use bevy::mesh::VertexAttributeValues;
        let mesh = flame_mesh(0.4);
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let Some(VertexAttributeValues::Float32x4(colours)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("colours");
        };
        for (point, colour) in points.iter().zip(colours) {
            let top = point[0] > 0.0;
            assert_eq!(point[1].abs(), if top { 0.2 } else { 0.5 });
            assert_eq!(colour[3], if top { 0.0 } else { 1.0 });
        }
    }
}
