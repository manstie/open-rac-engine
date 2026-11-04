// The screen warp (0x388508; `warp.rs`): the game writes its ring texture's alpha, in four
// mirrored quarters with the ring's (0, 0) at the screen's corners and (1, 1) at its centre,
// into the frame's alpha; then draws its copy of the frame over the screen, magnified about
// the centre, mixed in by that alpha ((Cs − Cd)·Ad + Cd).

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Warp {
    alpha: f32,
    zoom: f32,
}

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var clamped: sampler;
@group(0) @binding(2) var<uniform> warp: Warp;
@group(0) @binding(3) var ring: texture_2d<f32>;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let here = textureSample(frame, clamped, in.uv);
    let quarter = 1.0 - abs(2.0 * in.uv - 1.0);
    let mixed = textureSample(ring, clamped, quarter).a * warp.alpha;
    let pushed = textureSample(frame, clamped, 0.5 + (in.uv - 0.5) / warp.zoom);
    return vec4(mix(here.rgb, pushed.rgb, mixed), here.a);
}
