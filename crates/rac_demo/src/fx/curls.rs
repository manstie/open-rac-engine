//! Rays curling round a point, each turning about an axis of its own and lengthening: the
//! sun over the Rynocirator's first missile (class 0x1DF6, 0x3DD390, 0x3DDB38; drawn by
//! 0x3DD9B0; `effects.md`, "RY3NO").

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use serde::Deserialize;

use super::{Blend, FxRng, Tex};
use crate::DemoEntity;

/// `count` rays from a point `ahead` along the shot's way and `above`, `length` long and
/// each tick `grow` longer and turned `turn` degrees about its axis, for `ticks` from `delay`
/// ticks after the event. Each is `width` across at the point and `flare` times that at its
/// end, going from `root` to `tip`; the texture moves `scroll` a tick.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Curls {
    pub count: u32,
    pub texture: Tex,
    pub length: f32,
    pub grow: f32,
    pub turn: f32,
    pub width: f32,
    pub flare: f32,
    pub root: [f32; 4],
    pub tip: [f32; 4],
    pub ticks: u32,
    #[serde(default)]
    pub delay: u32,
    #[serde(default)]
    pub ahead: f32,
    #[serde(default)]
    pub above: f32,
    #[serde(default)]
    pub scroll: f32,
    #[serde(default)]
    pub blend: Blend,
}

/// One ray: its axis and where it points now.
#[derive(Debug, Clone, Copy)]
struct Ray {
    axis: Vec3,
    out: Vec3,
}

/// A sun of rays on its way.
#[derive(Component, Debug)]
pub(crate) struct Curling {
    curls: Curls,
    rays: Vec<Ray>,
    tick: u32,
}

/// A colour of the recipes' (the game's bytes over 0x80) as a mesh's linear vertex colour.
fn linear(colour: [f32; 4]) -> [f32; 4] {
    Color::srgba(colour[0], colour[1], colour[2], colour[3])
        .to_linear()
        .to_f32_array()
}

/// The rays' mesh, `length` long, each quad's normal up since they're unlit.
fn mesh(rays: &[Ray], curls: &Curls, length: f32, eye: Vec3, scroll: f32) -> Mesh {
    let mut positions = Vec::with_capacity(rays.len() * 4);
    let mut uvs = Vec::with_capacity(rays.len() * 4);
    let mut colours = Vec::with_capacity(rays.len() * 4);
    let mut indices = Vec::with_capacity(rays.len() * 6);
    for ray in rays {
        let side = ray.out.cross(eye).try_normalize().unwrap_or(Vec3::Y) * (curls.width / 2.0);
        let tip = ray.out * length;
        let first = positions.len() as u32;
        positions.extend([
            side.to_array(),
            (-side).to_array(),
            (tip + side * curls.flare).to_array(),
            (tip - side * curls.flare).to_array(),
        ]);
        uvs.extend([
            [scroll, 0.0],
            [scroll, 1.0],
            [scroll + 1.0, 0.0],
            [scroll + 1.0, 1.0],
        ]);
        let (root, tip) = (linear(curls.root), linear(curls.tip));
        colours.extend([root, root, tip, tip]);
        indices.extend([first, first + 1, first + 2, first + 2, first + 1, first + 3]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}

/// What [`super::Spawner`] needs to start a sun.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    image: Option<Handle<Image>>,
    curls: &Curls,
    at: Vec3,
    way: Option<Vec3>,
    eye: Vec3,
    rng: &mut FxRng,
) {
    let rays: Vec<Ray> = (0..curls.count)
        .map(|_| {
            let axis = rng.direction(None);
            let out = axis
                .cross(rng.direction(None))
                .try_normalize()
                .unwrap_or(Vec3::X);
            Ray { axis, out }
        })
        .collect();
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: image,
        alpha_mode: curls.blend.alpha_mode(),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let way = way.unwrap_or(Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);
    let place = at + way * curls.ahead + Vec3::Y * curls.above;
    let shown = mesh(&rays, curls, curls.length, eye - place, 0.0);
    commands.spawn((
        Name::new("curls"),
        DemoEntity,
        Mesh3d(meshes.add(shown)),
        MeshMaterial3d(material),
        NotShadowCaster,
        NoFrustumCulling,
        Transform::from_translation(place),
        if curls.delay > 0 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        },
        Curling {
            curls: curls.clone(),
            rays,
            tick: 0,
        },
    ));
}

/// Turns and lengthens each sun's rays a tick, dropping those whose time is up.
pub(crate) fn step_curls(
    mut suns: Query<(Entity, &mut Curling, &Mesh3d, &Transform, &mut Visibility)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    let eye = super::eye(&cameras);
    for (entity, mut curling, shown, place, mut visibility) in &mut suns {
        curling.tick += 1;
        let tick = curling.tick;
        let delay = curling.curls.delay;
        if tick < delay {
            continue;
        }
        let age = tick - delay;
        if age >= curling.curls.ticks {
            commands.entity(entity).despawn();
            continue;
        }
        *visibility = Visibility::Visible;
        let turn = curling.curls.turn.to_radians();
        for ray in &mut curling.rays {
            ray.out = (Quat::from_axis_angle(ray.axis, turn) * ray.out).normalize_or(ray.out);
        }
        let length = curling.curls.length + curling.curls.grow * age as f32;
        let scroll = curling.curls.scroll * age as f32;
        let rays = mesh(
            &curling.rays,
            &curling.curls,
            length,
            eye - place.translation,
            scroll,
        );
        if let Some(mut mesh_now) = meshes.get_mut(&shown.0) {
            *mesh_now = rays;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn a_rays_end_is_its_length_out_and_flared() {
        let curls = Curls {
            count: 1,
            texture: Tex::Effect(14),
            length: 4.0,
            grow: 0.06,
            turn: 2.0,
            width: 0.6,
            flare: 1.1,
            root: [1.0, 0.5, 0.25, 0.25],
            tip: [1.0, 0.0, 0.0, 0.0],
            ticks: 64,
            delay: 30,
            ahead: 10.0,
            above: 3.0,
            scroll: -0.018,
            blend: Blend::Add,
        };
        let ray = Ray {
            axis: Vec3::Y,
            out: Vec3::X,
        };
        let mesh = mesh(&[ray], &curls, 4.0, Vec3::Z, 0.0);
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let (root, tip) = (Vec3::from(points[0]), Vec3::from(points[2]));
        assert!((root.length() - 0.3).abs() < 1e-5);
        assert!((tip.x - 4.0).abs() < 1e-5);
        assert!((tip.y.abs() - 0.33).abs() < 1e-5);
    }
}
