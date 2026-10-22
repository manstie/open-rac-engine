//! The level's effect textures from the user's UYA disc (`docs/research/effects.md`,
//! "Effect textures"; PLAN.md P3.18). So far they give the aim indicators their images, in
//! place of the demo's placeholders ([`SightImages`]), which share the game's numbers.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use rac_demo::hud::SightImages;
use rac_formats::texture::Texture;

/// Veldin's effect textures, read from the disc before the app starts.
#[derive(Resource, Debug, Default)]
pub struct DiscEffectTextures(pub Vec<Texture>);

/// Puts the disc's image in place of each placeholder the sights painted that the disc
/// has, keeping the demo's own (the scope's sides) for the rest.
pub fn register_disc_sight_images(
    textures: Res<DiscEffectTextures>,
    mut sights: ResMut<SightImages>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut replaced = 0;
    for (&number, handle) in &mut sights.0 {
        if let Some(texture) = textures.0.get(usize::from(number)) {
            *handle = images.add(image(texture));
            replaced += 1;
        }
    }
    if replaced > 0 {
        info!("the aim indicators' images from the disc: {replaced}");
    }
}

/// An effect texture as an sRGB image, its alpha doubled so the GS's 0x80 is opaque.
fn image(texture: &Texture) -> Image {
    Image::new(
        Extent3d {
            width: texture.width,
            height: texture.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texture.rgba(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
