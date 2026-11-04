// A strip of the Shock Cannon's charged lance (0x409448; `lance.rs`): the game writes its
// texture's alpha, scrolled one way, into the frame's alpha (ALPHA 0x6A), then adds the
// texture's colour, scrolled another way, weighted by that (0x58: Cs·Ad + Cd). So its light
// is the one times the other's alpha, added.

import bevy_pbr::render::forward_io::VertexOutput;

struct StripLook {
    colour: vec4<f32>,
    scroll: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> look: StripLook;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var strip_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var strip_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(VERTEX_UVS_A && VERTEX_UVS_B)
    {
        let light = textureSample(strip_texture, strip_sampler, in.uv + vec2(look.scroll.y, 0.0));
        let through = textureSample(strip_texture, strip_sampler, in.uv_b + vec2(look.scroll.x, 0.0));
        // Added: nothing in alpha, so what is behind keeps all of its own.
        return vec4(look.colour.rgb * light.rgb * through.a * look.colour.a, 0.0);
    }
    return vec4(0.0);
}
