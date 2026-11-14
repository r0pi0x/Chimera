// ShadeMaterial (materials/mod.rs): a shade falling off to the right, its
// display-space alpha `from` at the left easing (smoothstep) to 0 between
// `start` and `end` of the width, computed per pixel in float (a gradient
// node's stops land in 8 bits: near-opaque ink banded in wide stripes),
// with a one-step dither, then turned to the linear-light alpha that
// lands there (the theme's ink curve, `Theme::tone_a`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

// Linear colour; its alpha scales the shade.
@group(1) @binding(0) var<uniform> color: vec4<f32>;
// x: alpha at the left (display space), y: start, z: end (0-1 of the
// width), w: the dither's amplitude (display-space alpha).
@group(1) @binding(1) var<uniform> params: vec4<f32>;

fn hash(p0: vec2<f32>) -> f32 {
    var p = fract(p0 * vec2<f32>(123.34, 456.21));
    p = p + dot(p, p + 45.32);
    return fract(p.x * p.y);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let fall = smoothstep(params.y, params.z, in.uv.x);
    let shade = params.x * (1.0 - fall);
    if (shade <= 0.0) {
        discard;
    }
    let px = floor(in.uv * in.size);
    let a = clamp(shade + (hash(px) - 0.5) * params.w, 0.0, 1.0);
    let k = 1.0 - pow(1.0 - a, 2.2);
    return vec4<f32>(color.rgb, color.a * k);
}
