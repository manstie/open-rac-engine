// A light as the game's (0x4A3D80; `light.rs`): what the depth prepass shows within its
// radius is brightened by 1 + its colour × (1 − distance / radius), at most `colour.w`
// times. The game multiplies its own (gamma) values, so the kit's linear ones take that
// to the power 2.2. Drawn with multiply blending and nothing in alpha, so what it returns
// plus one multiplies what is there.

import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::view_transformations::{frag_coord_to_ndc, position_ndc_to_world};
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;

struct LightPlace {
    centre: vec4<f32>,
    colour: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> light: LightPlace;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(DEPTH_PREPASS)
    {
        let depth = prepass_depth(in.position, 0u);
        // Nothing drawn there: the sky.
        if depth <= 0.0 {
            discard;
        }
        let at = position_ndc_to_world(vec3(frag_coord_to_ndc(in.position).xy, depth));
        let near = 1.0 - distance(at, light.centre.xyz) / light.centre.w;
        if near <= 0.0 {
            discard;
        }
        let times = min(vec3(1.0) + light.colour.rgb * near, vec3(light.colour.w));
        return vec4(pow(times, vec3(2.2)) - vec3(1.0), 0.0);
    }
    return vec4(0.0);
}
