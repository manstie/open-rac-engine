//! The level's effect and particle textures from the user's UYA disc
//! (`docs/research/effects.md`, "Effect textures" and "Particles"; PLAN.md P3.18). They
//! give the aim indicators ([`SightImages`]) and the guns' effects ([`FxImages`]) their
//! images in place of the demo's placeholders, which go by the game's numbers.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use rac_demo::fx::{FxImages, Tex};
use rac_demo::hud::SightImages;
use rac_formats::texture::Texture;

/// Veldin's effect and particle textures, and each particle type's list of the latter,
/// read from the disc before the app starts.
#[derive(Resource, Debug, Default)]
pub struct DiscEffectTextures {
    pub effects: Vec<Texture>,
    pub particles: Vec<Texture>,
    pub particle_lists: Vec<Vec<u8>>,
}

impl DiscEffectTextures {
    /// The disc's texture for `tex`: a particle type's entry counts round its list, as
    /// the game's spawners pick by number.
    fn get(&self, tex: Tex) -> Option<&Texture> {
        match tex {
            Tex::Effect(number) => self.effects.get(usize::from(number)),
            Tex::Particle(kind, entry) => {
                let list = self.particle_lists.get(usize::from(kind))?;
                let number = list.get(usize::from(entry) % list.len().max(1))?;
                self.particles.get(usize::from(*number))
            }
        }
    }
}

/// Puts the disc's image in place of each placeholder the sights painted that the disc
/// has, keeping the demo's own (the scope's sides) for the rest.
pub fn register_disc_sight_images(
    textures: Res<DiscEffectTextures>,
    mut sights: ResMut<SightImages>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut replaced = 0;
    for (&number, handle) in &mut sights.0 {
        if let Some(texture) = textures.effects.get(usize::from(number)) {
            *handle = images.add(image(texture));
            replaced += 1;
        }
    }
    if replaced > 0 {
        info!("the aim indicators' images from the disc: {replaced}");
    }
}

/// Puts the disc's image in place of each placeholder the guns' effects painted.
pub fn register_disc_fx_images(
    textures: Res<DiscEffectTextures>,
    mut fx: ResMut<FxImages>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut replaced = 0;
    for (&tex, handle) in &mut fx.0 {
        if let Some(texture) = textures.get(tex) {
            *handle = images.add(image(texture));
            replaced += 1;
        }
    }
    if replaced > 0 {
        info!("the guns' effects' images from the disc: {replaced}");
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
