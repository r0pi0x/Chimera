// The integer HDR's bloom (specs/tf2/hdr_bloom.md section 5; see
// hdr_integer.rs and `render::source_hdr::bloom_reference`, the same on
// the CPU): from the finished 3D frame as 8-bit gamma bytes, a
// quarter-size soft-threshold downsample, a 13-tap blur across then down
// (the down pass's step from the width: narrower by H / W) times the
// bloom amount, each written as 8-bit, then added to the frame's bytes in
// gamma space and clamped.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct BloomParams {
    // The quarter target's width and height and their inverses.
    quarter: vec4<f32>,
    // x: the bloom amount B.
    amount: vec4<f32>,
}

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var bloom_texture: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var<uniform> params: BloomParams;

const TINT = vec3<f32>(0.3, 0.59, 0.11);
const OFFSETS = array<f32, 7>(0.0, 1.3366, 3.4295, 5.4264, 7.4359, 9.4436, 11.4401);
const WEIGHTS = array<f32, 7>(0.2013, 0.2185, 0.0821, 0.0461, 0.0262, 0.0162, 0.0102);

fn srgb_encode(l: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(l, vec3<f32>(1.0 / 2.4)) - 0.055, l * 12.92, l <= vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// The frame's stored byte at a pixel (gamma, 0..1).
fn stored(p: vec2<i32>, size: vec2<i32>) -> vec3<f32> {
    let c = textureLoad(source_texture, clamp(p, vec2<i32>(0), size - 1), 0).rgb;
    return srgb_encode(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
}

// Four 2x2 averages of the 4x4 block under the quarter texel, each shaped
// (c^2.2 x its luminance), averaged.
@fragment
fn down(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source_texture));
    let base = vec2<i32>(floor(in.position.xy)) * 4;
    var sum = vec3<f32>(0.0);
    for (var by = 0; by < 2; by++) {
        for (var bx = 0; bx < 2; bx++) {
            let o = base + vec2<i32>(bx * 2, by * 2);
            let avg = 0.25 * (stored(o, size) + stored(o + vec2<i32>(1, 0), size) + stored(o + vec2<i32>(0, 1), size)
                + stored(o + vec2<i32>(1, 1), size));
            sum += pow(avg, vec3<f32>(2.2)) * dot(avg, TINT);
        }
    }
    return vec4<f32>(sum * 0.25, 1.0);
}

fn blur(uv: vec2<f32>, step: vec2<f32>) -> vec3<f32> {
    var s = textureSampleLevel(source_texture, linear_sampler, uv, 0.0).rgb * WEIGHTS[0];
    for (var i = 1; i < 7; i++) {
        let d = step * OFFSETS[i];
        s += (textureSampleLevel(source_texture, linear_sampler, uv + d, 0.0).rgb
            + textureSampleLevel(source_texture, linear_sampler, uv - d, 0.0).rgb) * WEIGHTS[i];
    }
    return s;
}

@fragment
fn hblur(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(blur(in.uv, vec2<f32>(params.quarter.z, 0.0)), 1.0);
}

// The step is one texel of the width (Quirks 2), times the bloom amount.
@fragment
fn vblur(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(blur(in.uv, vec2<f32>(0.0, params.quarter.z)) * params.amount.x, 1.0);
}

// The frame's bytes plus the stretched bloom, clamped, back to linear.
@fragment
fn add(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source_texture));
    let c = stored(vec2<i32>(floor(in.position.xy)), size);
    let b = textureSampleLevel(bloom_texture, linear_sampler, in.uv, 0.0).rgb;
    return vec4<f32>(srgb_decode(min(c + b, vec3<f32>(1.0))), 1.0);
}
