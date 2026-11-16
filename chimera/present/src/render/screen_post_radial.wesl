// The radial blur through a mask's alpha (see screen_post.rs, "The radial
// blur"): three taps along the line from the centre, their mean blended
// in by the mask's alpha, on gamma values.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Radial {
    // W, H of the frame (pixels), the centre (texture coordinates).
    size_centre: vec4<f32>,
    // k (the taps' scales about the centre), one mask texel in frame
    // pixels, unused.
    k_texel: vec4<f32>,
}

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var mask: texture_2d<f32>;
@group(0) @binding(2) var radial_sampler: sampler;
@group(0) @binding(3) var<uniform> radial_params: Radial;

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
    let c = textureSample(image, radial_sampler, uv);
    return clamp(to_srgb(max(c.rgb, vec3<f32>(0.0))), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn radial(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = radial_params.size_centre.xy;
    let c = radial_params.size_centre.zw;
    let k = radial_params.k_texel.xy;
    let d = in.uv - c;
    let s0 = gamma_at(in.uv);
    let s1 = gamma_at(c + d * (vec2<f32>(1.0) - k));
    let s2 = gamma_at(c + d * (vec2<f32>(1.0) + k));
    let blurred = (s0 + s1 + s2) / 3.0;
    // The mask: one texel per k_texel.z frame pixels, centred on c.
    let m = vec2<f32>(textureDimensions(mask));
    let muv = (d * size / radial_params.k_texel.z + m * 0.5) / m;
    let a = textureSample(mask, radial_sampler, muv).a;
    return vec4<f32>(to_linear(mix(s0, blurred, a)), 1.0);
}
