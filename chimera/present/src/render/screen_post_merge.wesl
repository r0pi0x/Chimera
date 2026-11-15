// The focus blur's merge (see screen_post.rs, step 5).

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Params {
    size: vec4<f32>,
    add: vec4<f32>,
    scale: vec4<f32>,
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var blurred: texture_2d<f32>;
@group(0) @binding(2) var mask: texture_2d<f32>;
@group(0) @binding(3) var merge_sampler: sampler;
@group(0) @binding(4) var<uniform> merge_params: Params;

// Step 5: a S + (1 - a) B, a the mask's alpha stretched over the frame.
@fragment
fn merge(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let s = textureSample(scene, merge_sampler, in.uv).rgb;
    let b = textureSample(blurred, merge_sampler, in.uv).rgb;
    let a = textureSample(mask, merge_sampler, in.uv).a;
    return vec4<f32>(to_linear(clamp(s * a + b * (1.0 - a), vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
