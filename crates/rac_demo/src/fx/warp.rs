//! The screen warp a V5 Shock Cannon shot leaves (`effects.md`, "The Shock Cannon at V5"):
//! the game writes its ring texture's alpha into the frame's alpha in four mirrored quarters
//! meeting at the screen's centre, then draws its copy of the frame over the screen a little
//! magnified, mixed in by that alpha (0x388508). So for a few frames all but the middle shows
//! the frame pushed out. One timer runs it (0x1E1F20), so a new warp takes the place of one
//! running.
//!
//! The kit draws it after the 3D passes, before tone mapping, from the frame as drawn so far,
//! effects and all, as the game copies its frame.

use bevy::asset::{embedded_asset, load_embedded_asset};
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::prelude::*;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
    UniformComponentPlugin,
};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId,
    Canonical, ColorTargetState, ColorWrites, FilterMode, FragmentState, Operations, PipelineCache,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, Specializer,
    SpecializerKey, TextureFormat, TextureSampleType, Variants,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use rac_hero::FollowCamera;

/// The warp running, by the game's one timer.
#[derive(Resource, Debug, Default, Clone)]
pub(crate) struct ScreenWarp {
    left: u32,
    ticks: u32,
    alpha: f32,
    zoom: f32,
    ring: Option<Handle<Image>>,
}

impl ScreenWarp {
    pub(crate) fn new(ticks: u32, alpha: f32, zoom: f32, ring: Option<Handle<Image>>) -> Self {
        Self {
            left: ticks,
            ticks,
            alpha,
            zoom,
            ring,
        }
    }

    /// This tick's look, its alpha falling evenly from `alpha` as the game's 5 × the timer
    /// does; `None` once it has run out.
    fn next(&mut self) -> Option<Warp> {
        if self.left == 0 {
            return None;
        }
        let look = Warp {
            alpha: self.alpha * self.left as f32 / self.ticks as f32,
            zoom: self.zoom,
        };
        self.left -= 1;
        Some(look)
    }
}

/// A follow camera's warp this tick, as its shader takes it.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, ShaderType, ExtractComponent)]
#[extract_app(RenderApp)]
pub(crate) struct Warp {
    alpha: f32,
    zoom: f32,
}

/// The ring the warp is mixed in by, in the render world.
#[derive(Resource, Debug, Default, Clone)]
struct WarpRing(Option<Handle<Image>>);

impl ExtractResource<RenderApp> for WarpRing {
    type Source = ScreenWarp;

    fn extract_resource(source: &ScreenWarp) -> Self {
        Self(source.ring.clone())
    }
}

/// Steps the warp a tick, putting its look on the follow cameras or taking it off.
pub(crate) fn run_warp(
    mut warp: ResMut<ScreenWarp>,
    cameras: Query<(Entity, Has<Warp>), With<FollowCamera>>,
    mut commands: Commands,
) {
    // Only touched while it runs, so it isn't extracted every tick.
    let look = if warp.left > 0 { warp.next() } else { None };
    for (camera, warped) in &cameras {
        match look {
            Some(look) => {
                commands.entity(camera).insert(look);
            }
            None if warped => {
                commands.entity(camera).remove::<Warp>();
            }
            None => {}
        }
    }
}

/// Draws the warp when the render plugins are there.
pub(crate) fn draw_warps(app: &mut App) {
    embedded_asset!(app, "warp.wesl");
    app.add_plugins((
        ExtractComponentPlugin::<Warp>::default(),
        UniformComponentPlugin::<Warp>::default(),
        ExtractResourcePlugin::<WarpRing>::default(),
    ));
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .add_systems(RenderStartup, make_warp_pipeline)
        .add_systems(Render, choose_warp_pipelines.in_set(RenderSystems::Prepare))
        .add_systems(
            Core3d,
            warp_frame
                .in_set(Core3dSystems::PostProcess)
                .before(tonemapping),
        );
}

#[derive(Resource)]
struct WarpPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    variants: Variants<RenderPipeline, WarpTarget>,
}

/// Makes the warp's pipeline for each view's texture format.
struct WarpTarget;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, SpecializerKey)]
struct WarpTargetKey(TextureFormat);

impl Specializer<RenderPipeline> for WarpTarget {
    type Key = WarpTargetKey;

    fn specialize(
        &self,
        key: WarpTargetKey,
        descriptor: &mut RenderPipelineDescriptor,
    ) -> Result<Canonical<WarpTargetKey>, BevyError> {
        descriptor.fragment_mut()?.set_target(
            0,
            ColorTargetState {
                format: key.0,
                blend: None,
                write_mask: ColorWrites::ALL,
            },
        );
        Ok(key)
    }
}

fn make_warp_pipeline(
    device: Res<RenderDevice>,
    assets: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    mut commands: Commands,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "screen_warp_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<Warp>(true),
                texture_2d(TextureSampleType::Float { filterable: true }),
            ),
        ),
    );
    // Clamped, so the ring's quarters don't take in its far edges.
    let sampler = device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    let descriptor = RenderPipelineDescriptor {
        label: Some("screen_warp_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: load_embedded_asset!(assets.as_ref(), "warp.wesl"),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };
    commands.insert_resource(WarpPipeline {
        layout,
        sampler,
        variants: Variants::new(WarpTarget, descriptor),
    });
}

#[derive(Component)]
struct WarpPipelineId(CachedRenderPipelineId);

fn choose_warp_pipelines(
    cache: Res<PipelineCache>,
    mut pipeline: ResMut<WarpPipeline>,
    views: Query<(Entity, &ExtractedView), With<Warp>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    for (view, extracted) in &views {
        let id = pipeline
            .variants
            .specialize(&cache, WarpTargetKey(extracted.target_format))?;
        commands.entity(view).insert(WarpPipelineId(id));
    }
    Ok(())
}

fn warp_frame(
    view: ViewQuery<(&ViewTarget, &DynamicUniformIndex<Warp>, &WarpPipelineId), With<Warp>>,
    pipeline: Res<WarpPipeline>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<Warp>>,
    ring: Option<Res<WarpRing>>,
    images: Res<RenderAssets<GpuImage>>,
    mut ctx: RenderContext,
) {
    let (target, index, id) = view.into_inner();
    let ring = ring
        .as_deref()
        .and_then(|ring| ring.0.as_ref())
        .and_then(|ring| images.get(ring));
    let (Some(render_pipeline), Some(uniform), Some(ring)) = (
        cache.get_render_pipeline(id.0),
        uniforms.uniforms().binding(),
        ring,
    ) else {
        return;
    };
    // Only once it is sure to draw: this swaps the view's textures.
    let frame = target.post_process_write();
    let bind_group = ctx.render_device().create_bind_group(
        "screen_warp",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((
            frame.source,
            &pipeline.sampler,
            uniform,
            &ring.texture_view,
        )),
    );
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("screen_warp"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: frame.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, &bind_group, &[index.index()]);
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_warp_fades_evenly_over_its_ticks_and_a_new_one_starts_it_again() {
        let mut warp = ScreenWarp::new(6, 0.24, 1.06, None);
        let alphas: Vec<f32> = std::iter::from_fn(|| warp.next())
            .map(|look| look.alpha)
            .collect();
        let expected = [0.24, 0.2, 0.16, 0.12, 0.08, 0.04];
        assert_eq!(alphas.len(), expected.len());
        for (alpha, expected) in alphas.iter().zip(expected) {
            assert!((alpha - expected).abs() < 1e-6, "{alphas:?}");
        }
        assert_eq!(warp.next(), None);
        warp = ScreenWarp::new(27, 1.055, 1.06, None);
        assert_eq!(
            warp.next(),
            Some(Warp {
                alpha: 1.055,
                zoom: 1.06
            })
        );
    }
}
