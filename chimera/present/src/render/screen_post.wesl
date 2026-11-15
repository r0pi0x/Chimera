// Full-screen passes over the finished frame (see screen_post.rs): the
// scene light, and the focus blur's saturate, downsample, vertical and
// horizontal blur and merge. Everything between the passes is in gamma
// values (as the games' 8-bit frame buffers hold them).

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Params {
    // W, H, 1/W, 1/H of the frame.
    size: vec4<f32>,
    add: vec4<f32>,
    scale: vec4<f32>,
}

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn gamma_at(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSample(image, image_sampler, uv);
    return clamp(to_srgb(max(c.rgb, vec3<f32>(0.0))), vec3<f32>(0.0), vec3<f32>(1.0));
}

// The scene light: c x scale + add.
@fragment
fn light(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let g = gamma_at(in.uv);
    let lit = clamp(g * params.scale.rgb + params.add.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(to_linear(lit), 1.0);
}

// Step 1: halfway to the grey (0.3, 0.59, 0.11).
@fragment
fn saturate(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let g = gamma_at(in.uv);
    let grey = dot(g, vec3<f32>(0.3, 0.59, 0.11));
    return vec4<f32>(g * 0.5 + vec3<f32>(grey) * 0.5, 1.0);
}

// Step 2: four taps one full-screen pixel diagonally around the centre.
@fragment
fn down(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let px = params.size.zw;
    let a = textureSample(image, image_sampler, in.uv + vec2<f32>(px.x, px.y)).rgb;
    let b = textureSample(image, image_sampler, in.uv + vec2<f32>(-px.x, px.y)).rgb;
    let c = textureSample(image, image_sampler, in.uv + vec2<f32>(px.x, -px.y)).rgb;
    let d = textureSample(image, image_sampler, in.uv + vec2<f32>(-px.x, -px.y)).rgb;
    return vec4<f32>((a + b + c + d) * 0.25, 1.0);
}

fn blur(uv: vec2<f32>, step: vec2<f32>) -> vec4<f32> {
    let m = textureSample(image, image_sampler, uv).rgb;
    let p = textureSample(image, image_sampler, uv + step).rgb;
    let n = textureSample(image, image_sampler, uv - step).rgb;
    return vec4<f32>(m * 0.5 + p * 0.25 + n * 0.25, 1.0);
}

// Step 3: taps 8 full-screen pixels up and down.
@fragment
fn vblur(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, 8.0 * params.size.w));
}

// Step 4: taps 8 full-screen pixels left and right.
@fragment
fn hblur(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(8.0 * params.size.z, 0.0));
}
