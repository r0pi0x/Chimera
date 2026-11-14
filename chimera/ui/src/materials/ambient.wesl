// AmbientMaterial (materials/mod.rs): the main menu's ambient background,
// a stub until Look Book V settles it: the ink with a slow, faint accent
// glow drifting along the slant on the renderer's clock, and a dither
// against banding.

import bevy_render::globals::Globals;
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(0) @binding(1) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> ink: vec4<f32>;
@group(1) @binding(1) var<uniform> acc: vec4<f32>;
// x: glow strength, y: loop in seconds, z: tan of the slant, w: opacity.
@group(1) @binding(2) var<uniform> params: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let aspect = in.size.x / max(in.size.y, 1.0);
    let p = vec2<f32>(in.uv.x * aspect, in.uv.y);
    let ph = fract(globals.time / params.y) * 6.28318530718;
    let c = vec2<f32>(aspect * (0.62 + 0.10 * sin(ph)), 0.42 + 0.06 * cos(ph));
    let d = p - c;
    // Stretched along the slant.
    let s = vec2<f32>(d.x - d.y * params.z, d.y * 1.6);
    let g = exp(-dot(s, s) * 3.0) * params.x;
    let dither = (hash(in.uv * in.size) - 0.5) / 255.0;
    let rgb = ink.rgb + acc.rgb * g + vec3<f32>(dither);
    return vec4<f32>(rgb, params.w);
}
