//! The placeholder images for the effects' textures, painted when the app starts so the kit
//! ships no art. Each is a shape in a colour, roughly what the game's texture of that
//! number shows (`docs/research/effects.md`).

use bevy::{
    asset::RenderAssetUsages,
    math::ops,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use serde::Deserialize;

/// Painted images are this many pixels a side.
const SIDE: u32 = 64;

/// What a placeholder looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Shape {
    /// A soft round glow.
    Glow,
    /// A glow with four thin rays.
    Star,
    /// A flame along +x, widest and brightest at the left (the muzzle).
    Flame,
    /// A lumpy soft cloud.
    Smoke,
    /// A small hot dot.
    Spark,
    /// A soft band across, for beams.
    Streak,
    /// A jagged line across, for bolts.
    Bolt,
    /// A thin circle.
    Ring,
    /// A narrow leaf-shaped blade.
    Feather,
}

/// A placeholder: its shape and colour (sRGB).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Paint {
    pub shape: Shape,
    pub colour: [f32; 3],
}

/// A fixed hash of a cell, 0 to 1, for the clouds' lumps.
fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth value noise over cells `scale` across.
fn noise(p: Vec2, scale: f32) -> f32 {
    let q = p * scale;
    let (cell, f) = (q.floor(), q - q.floor());
    let (x, y) = (cell.x as i32, cell.y as i32);
    let s = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let top = hash(x, y) + (hash(x + 1, y) - hash(x, y)) * s.x;
    let bottom = hash(x, y + 1) + (hash(x + 1, y + 1) - hash(x, y + 1)) * s.x;
    top + (bottom - top) * s.y
}

/// How much of the shape covers `p` (−1 to 1 across the image), and how much brighter
/// than its colour it is there (toward white).
fn coverage(shape: Shape, p: Vec2) -> (f32, f32) {
    let r = p.length();
    let soft = |r: f32| (1.0 - r).clamp(0.0, 1.0);
    match shape {
        Shape::Glow => (soft(r) * soft(r), soft(r * 1.6)),
        Shape::Star => {
            let core = soft(r * 1.4);
            let rays =
                soft(p.x.abs() * 14.0) * soft(p.y.abs()) + soft(p.y.abs() * 14.0) * soft(p.x.abs());
            ((core * core + rays).min(1.0), core)
        }
        Shape::Flame => {
            // From the left edge to the right, tapering to a point.
            let along = (p.x + 1.0) / 2.0;
            let half = 0.55 * (1.0 - along) * ops::sqrt(along.max(0.0) * 4.0).min(1.0);
            let across = if half > 0.0 { p.y.abs() / half } else { 1.0 };
            let body = soft(across) * soft(along * 0.9);
            (body, soft(across * 1.5) * (1.0 - along))
        }
        Shape::Smoke => {
            let lumps = noise(p + 1.0, 3.0) * 0.6 + noise(p + 1.0, 7.0) * 0.4;
            (soft(r * (1.3 - 0.5 * lumps)).powi(2), 0.0)
        }
        Shape::Spark => (soft(r * 2.2) + 0.3 * soft(r) * soft(r), soft(r * 4.0)),
        Shape::Streak => {
            let band = soft(p.y.abs() * 1.2);
            (band * band, soft(p.y.abs() * 4.0))
        }
        Shape::Bolt => {
            let path = 0.35 * (noise(Vec2::new(p.x + 1.0, 0.5), 4.0) - 0.5)
                + 0.15 * (noise(Vec2::new(p.x + 1.0, 2.5), 11.0) - 0.5);
            let off = (p.y - path).abs();
            (soft(off * 9.0) + 0.4 * soft(off * 3.0), soft(off * 20.0))
        }
        Shape::Ring => {
            let off = (r - 0.75).abs();
            (soft(off * 7.0), soft(off * 14.0))
        }
        Shape::Feather => {
            let across = p.y.abs() / (0.3 * soft(p.x.abs()).max(1e-3));
            (soft(across) * 0.85, 0.0)
        }
    }
}

/// The placeholder for `paint`, an sRGB image with its alpha.
pub fn image(paint: Paint) -> Image {
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let colour = Vec3::from(paint.colour);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) / SIDE as f32 * 2.0 - 1.0;
            let (alpha, hot) = coverage(paint.shape, p);
            let rgb = colour.lerp(Vec3::ONE, hot.clamp(0.0, 1.0));
            data.extend(
                [rgb.x, rgb.y, rgb.z, alpha]
                    .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8),
            );
        }
    }
    Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_paints_something_and_fades_to_nothing_at_the_corners() {
        let shapes = [
            Shape::Glow,
            Shape::Star,
            Shape::Flame,
            Shape::Smoke,
            Shape::Spark,
            Shape::Streak,
            Shape::Bolt,
            Shape::Ring,
            Shape::Feather,
        ];
        for shape in shapes {
            let image = image(Paint {
                shape,
                colour: [1.0, 0.5, 0.2],
            });
            let data = image.data.unwrap();
            let alphas: Vec<u8> = data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel[3])
                .collect();
            assert!(alphas.iter().any(|&a| a > 128), "{shape:?}");
            assert!(alphas[0] < 16, "{shape:?}: its corner");
        }
    }
}
