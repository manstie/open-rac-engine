//! Meshes of revolution an effect leaves, growing and fading on tracks: the game's
//! `fx_dome` (0x409A50: a unit ball from its pole down a chosen number of rings, in a
//! matrix each caller scales, its colour fading by the last ring), which the Shock
//! Cannon's lances draw, and the meshes in the game's data that the Agents of Dread's blast
//! draws the same way (0x391AC0, 0x391F48, 0x3923A0; `effects.md`, "The Dread blast").

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::math::{Affine2, ops};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use serde::Deserialize;

use super::{Blend, Tex};
use crate::DemoEntity;

/// Rings of 18°, as 0x409A50 turns from its pole.
const RING_DEGREES: f32 = 18.0;
/// Quads round each of its rings, one texture wide and mirrored each time.
const AROUND: u32 = 10;

/// A value over a shape's ticks, straight between its `(tick, value)` keys and holding past
/// its last.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Track(pub Vec<(u32, f32)>);

impl Track {
    pub fn at(&self, tick: u32) -> f32 {
        let keys = &self.0;
        let Some(first) = keys.first() else {
            return 0.0;
        };
        if tick <= first.0 {
            return first.1;
        }
        for pair in keys.windows(2) {
            let ((from, a), (to, b)) = (pair[0], pair[1]);
            if tick < to {
                let share = (tick - from) as f32 / (to - from).max(1) as f32;
                return a + (b - a) * share;
            }
        }
        keys[keys.len() - 1].1
    }
}

/// The way a shape's axis lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum SwellAxis {
    #[default]
    Up,
    /// Along the shot's way.
    Way,
    /// Towards the camera as it starts.
    Eye,
}

/// What a shape is, in units its tracks scale.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum SwellShape {
    /// `fx_dome`'s ball from its pole at +1 down this many rings, nothing at the last.
    Rings(u32),
    /// Rings of `around` quads through `points`, each (radius, along, v, alpha), the
    /// texture `repeat` times round them.
    Outline {
        around: u32,
        repeat: f32,
        points: Vec<(f32, f32, f32, f32)>,
    },
    /// `count` spokes from the middle, in the plane across the axis, each 1 long and `root`
    /// to `tip` from its centre line, the texture's right half running out along it
    /// (0x387BF0).
    Spokes { count: u32, root: f32, tip: f32 },
    /// A column of `rings` rings up to `height`, nothing at its foot: `radius` × (2 +
    /// the cosine of how high it stands), so its waist climbs as it stretches (0x391F48).
    Column {
        around: u32,
        rings: u32,
        height: f32,
        radius: f32,
        v: (f32, f32),
    },
}

/// A shape an event leaves ([`super::Emit::Swell`]).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Swell {
    pub shape: SwellShape,
    pub texture: Tex,
    pub colour: [f32; 3],
    /// Its opacity, and how far it lasts: it goes at the last key.
    pub alpha: Track,
    /// Its scale across its axis and along it.
    pub across: Track,
    pub along: Track,
    /// How far along its axis its middle is from the event.
    #[serde(default = "no_ahead")]
    pub ahead: Track,
    #[serde(default)]
    pub axis: SwellAxis,
    /// How far its texture moves a tick, as (round, along).
    #[serde(default)]
    pub scroll: (f32, f32),
    #[serde(default)]
    pub blend: Blend,
}

fn no_ahead() -> Track {
    Track(vec![(0, 0.0)])
}

impl Swell {
    fn ticks(&self) -> u32 {
        self.alpha.0.last().map_or(1, |key| key.0.max(1))
    }
}

/// The rings of a shape as (radius, along, v, alpha), from the pole or foot.
fn rings(shape: &SwellShape, along: f32) -> (Vec<(f32, f32, f32, f32)>, u32, f32) {
    match shape {
        SwellShape::Rings(count) => {
            let count = (*count).max(1);
            let points = (0..=count)
                .map(|ring| {
                    let angle = (ring as f32 * RING_DEGREES).to_radians();
                    let fade = 1.0 - ring as f32 / count as f32;
                    (ops::sin(angle), ops::cos(angle), ring as f32, fade)
                })
                .collect();
            (points, AROUND, AROUND as f32 / 2.0)
        }
        SwellShape::Outline {
            around,
            repeat,
            points,
        } => (points.clone(), *around, *repeat),
        SwellShape::Spokes { .. } => (Vec::new(), 0, 0.0),
        SwellShape::Column {
            around,
            rings,
            height,
            radius,
            v,
        } => {
            let points = (0..=*rings)
                .map(|ring| {
                    let share = ring as f32 / (*rings).max(1) as f32;
                    let high = share * height;
                    let wide = radius * (2.0 + ops::cos(high * along));
                    let fade = if ring == 0 { 0.0 } else { 1.0 };
                    (wide, high * along, v.0 + (v.1 - v.0) * share, fade)
                })
                .collect();
            (points, *around, 1.0)
        }
    }
}

/// Spokes' mesh: one quad each.
fn spokes(count: u32, root: f32, tip: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for spoke in 0..count {
        let turn = spoke as f32 / count.max(1) as f32 * std::f32::consts::TAU;
        let out = Vec3::new(ops::cos(turn), 0.0, ops::sin(turn));
        let side = Vec3::new(-out.z, 0.0, out.x);
        let at = positions.len() as u32;
        for (place, uv) in [
            (side * root, [0.5, 0.0]),
            (-side * root, [0.5, 1.0]),
            (out + side * tip, [1.0, 0.0]),
            (out - side * tip, [1.0, 1.0]),
        ] {
            positions.push(place.to_array());
            uvs.push(uv);
        }
        indices.extend([at, at + 2, at + 1, at + 1, at + 2, at + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_normals()
}

/// A shape's mesh, its axis +Y, at `along` for those that change as they stretch.
pub(crate) fn mesh(shape: &SwellShape, along: f32) -> Mesh {
    if let SwellShape::Spokes { count, root, tip } = *shape {
        return spokes(count, root, tip);
    }
    let (points, around, repeat) = rings(shape, along);
    let around = around.max(3);
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut colours = Vec::new();
    let mut indices = Vec::new();
    for (radius, high, v, alpha) in &points {
        for column in 0..=around {
            let turn = column as f32 / around as f32 * std::f32::consts::TAU;
            positions.push([radius * ops::cos(turn), *high, radius * ops::sin(turn)]);
            uvs.push([column as f32 / around as f32 * repeat, *v]);
            colours.push([1.0, 1.0, 1.0, *alpha]);
        }
    }
    let row = around + 1;
    for ring in 0..points.len().saturating_sub(1) as u32 {
        for column in 0..around {
            let at = ring * row + column;
            indices.extend([at, at + row, at + 1, at + 1, at + row, at + row + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_normals()
}

/// A shape on its tracks.
#[derive(Component, Debug, Clone)]
pub(crate) struct Swelling {
    swell: Swell,
    tick: u32,
    from: Vec3,
    axis: Vec3,
    material: Handle<StandardMaterial>,
}

/// What [`super::Spawner`] needs to leave a shape.
pub(crate) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    image: Option<Handle<Image>>,
    swell: &Swell,
    at: Vec3,
    way: Option<Vec3>,
    eye: Vec3,
) {
    let axis = match swell.axis {
        SwellAxis::Up => Vec3::Y,
        SwellAxis::Way => way.unwrap_or(Vec3::NEG_Z).normalize_or(Vec3::NEG_Z),
        SwellAxis::Eye => (eye - at).normalize_or(Vec3::Y),
    };
    let [r, g, b] = swell.colour;
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(r, g, b, swell.alpha.at(0)),
        base_color_texture: image,
        alpha_mode: swell.blend.alpha_mode(),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let along = swell.along.at(0);
    let swelling = Swelling {
        swell: swell.clone(),
        tick: 0,
        from: at,
        axis,
        material: material.clone(),
    };
    let place = swelling.place(0);
    commands.spawn((
        Name::new("swell"),
        DemoEntity,
        Mesh3d(meshes.add(mesh(&swell.shape, along))),
        MeshMaterial3d(material),
        NotShadowCaster,
        place,
        swelling,
    ));
}

impl Swelling {
    fn place(&self, tick: u32) -> Transform {
        let across = self.swell.across.at(tick);
        let along = match self.swell.shape {
            // Its mesh takes the stretch itself.
            SwellShape::Column { .. } => 1.0,
            _ => self.swell.along.at(tick),
        };
        Transform::from_translation(self.from + self.axis * self.swell.ahead.at(tick))
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, self.axis))
            .with_scale(Vec3::new(across, along, across))
    }
}

/// Moves each shape a tick along its tracks, dropping those whose time is up.
pub(crate) fn step_swells(
    mut shapes: Query<(Entity, &mut Swelling, &mut Transform, &Mesh3d)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    for (entity, mut swelling, mut transform, shown) in &mut shapes {
        swelling.tick += 1;
        let tick = swelling.tick;
        if tick >= swelling.swell.ticks() {
            commands.entity(entity).despawn();
            continue;
        }
        *transform = swelling.place(tick);
        if matches!(swelling.swell.shape, SwellShape::Column { .. })
            && let Some(mut mesh_now) = meshes.get_mut(&shown.0)
        {
            *mesh_now = mesh(&swelling.swell.shape, swelling.swell.along.at(tick));
        }
        if let Some(mut material) = materials.get_mut(&swelling.material) {
            material.base_color.set_alpha(swelling.swell.alpha.at(tick));
            let (u, v) = swelling.swell.scroll;
            material.uv_transform = Affine2::from_translation(Vec2::new(u, v) * tick as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn points(mesh: &Mesh) -> Vec<[f32; 3]> {
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        points.clone()
    }

    #[test]
    fn a_track_runs_straight_between_its_keys_and_holds_past_its_last() {
        let track = Track(vec![(0, 1.0), (4, 3.0), (20, 0.0)]);
        assert_eq!(track.at(0), 1.0);
        assert_eq!(track.at(2), 2.0);
        assert_eq!(track.at(4), 3.0);
        assert_eq!(track.at(12), 1.5);
        assert_eq!(track.at(99), 0.0);
    }

    #[test]
    fn fx_dome_starts_at_its_pole_and_turns_18_degrees_a_ring() {
        // Eight rings of the Shock Cannon's lance reach 144° from the pole.
        let mesh = mesh(&SwellShape::Rings(8), 1.0);
        let all = points(&mesh);
        assert_eq!(all.len(), 9 * 11);
        assert!((all[0][1] - 1.0).abs() < 1e-5 && all[0][0].abs() < 1e-5);
        let last = all[8 * 11];
        assert!((last[1] - ops::cos(144f32.to_radians())).abs() < 1e-5);
        assert!((last[0] - ops::sin(144f32.to_radians())).abs() < 1e-5);
    }

    #[test]
    fn the_last_ring_of_a_dome_fades_to_nothing() {
        let (rings, ..) = rings(&SwellShape::Rings(4), 1.0);
        assert_eq!(rings[0].3, 1.0);
        assert_eq!(rings[4].3, 0.0);
    }

    #[test]
    fn a_columns_foot_is_unseen_and_its_waist_climbs_as_it_stretches() {
        let shape = SwellShape::Column {
            around: 12,
            rings: 16,
            height: 1.0,
            radius: 0.2,
            v: (0.16, 0.8),
        };
        let (low, ..) = rings(&shape, 1.0);
        assert_eq!(low[0].3, 0.0);
        assert_eq!(low[16].3, 1.0);
        let (high, ..) = rings(&shape, std::f32::consts::PI);
        assert!((high[16].0 - 0.2).abs() < 1e-5, "{}", high[16].0);
        assert!(low[16].0 > high[16].0);
    }
}
