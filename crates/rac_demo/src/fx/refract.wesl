// The frame behind a refracting effect (`refract.rs`): the game writes its mask's alpha into
// the frame's alpha, then draws its copy of the frame (texture −1) over it mixed in by that
// ((Cs − Cd)·Ad + Cd), each point taking the copy from elsewhere. Here the copy is Bevy's
// view transmission texture, and the mask's alpha mixes it in.

import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_functions::get_world_from_local;
import bevy_pbr::render::mesh_view_bindings::view;

@if(VIEW_TRANSMISSION_TEXTURE)
import bevy_pbr::render::mesh_view_bindings::{view_transmission_texture, view_transmission_sampler};

struct Bend {
    pull: f32,
    shrink: f32,
    disc: u32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> bend: Bend;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var mask_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var mask_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(VERTEX_UVS_A && VIEW_TRANSMISSION_TEXTURE)
    {
        let middle = get_world_from_local(in.instance_index)[3].xyz;
        let disc = bend.disc != 0u;
        // A lens's UVs lay its unit disc over 0 to 1. Each point of it takes the frame `pull`
        // of the way out to its rim, as its wedges' middles take it from their rims' (0x3753F8),
        // and its mask runs down its texture from the rim to the middle.
        let out = max(length(in.uv * 2.0 - 1.0), 1e-4);
        let share = select(bend.shrink, 1.0 - bend.pull + bend.pull / out, disc);
        let rows = f32(textureDimensions(mask_texture).y);
        let down = clamp(1.0 - out, 0.5 / rows, 1.0 - 0.5 / rows);
        let mask = textureSample(mask_texture, mask_sampler, select(in.uv, vec2(0.5, down), disc)).a;

        let source = middle + (in.world_position.xyz - middle) * share;
        let clip = view.clip_from_world * vec4(source, 1.0);
        let on_view = clip.xy / clip.w * vec2(0.5, -0.5) + 0.5;
        let size = vec2<f32>(textureDimensions(view_transmission_texture));
        let at = (on_view * view.viewport.zw + view.viewport.xy) / size;
        // The game's copy is the frame shrunk to 256 by 256, so blurred about a texel of that.
        let blur = 0.5 / 256.0 * view.viewport.zw / size;
        var behind = vec3(0.0);
        for (var i = 0; i < 4; i++) {
            let corner = vec2(f32(i & 1), f32(i >> 1u)) * 2.0 - 1.0;
            behind += textureSampleLevel(
                view_transmission_texture,
                view_transmission_sampler,
                at + corner * blur,
                0.0,
            ).rgb / 4.0;
        }
        return vec4(behind, mask);
    }
    return vec4(0.0);
}
