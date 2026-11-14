// AmbientMaterial (materials/mod.rs): the shared ambient behind the
// between-match screens (look book V's ambient: slanted light bands, a
// faceted field cross-fading every 24 s, one soft cobalt bloom, a cool
// lift along the bottom, a vignette, a one-step dither), on the
// renderer's clock or frozen. Our port of the look book's shader: its
// colours are display-space, so the field is composed in display space
// and turned to linear light at the end. Opaque: it is the ground.

import bevy_render::globals::Globals;
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(0) @binding(1) var<uniform> globals: Globals;
// Display-space (sRGB) colours.
@group(1) @binding(0) var<uniform> ink: vec4<f32>;
@group(1) @binding(1) var<uniform> fg: vec4<f32>;
@group(1) @binding(2) var<uniform> acc: vec4<f32>;
// x: calm column's right edge (0-1 of the width, < 0: none), y, z: the
// bloom's centre (x, y up; 0-1), w: tan of the slant.
@group(1) @binding(3) var<uniform> place: vec4<f32>;
// x: time offset (s), y: frozen time (< 0: live), z: opacity, w: unused.
@group(1) @binding(4) var<uniform> clock: vec4<f32>;

const TAU: f32 = 6.2831853;

fn hash(p0: vec2<f32>) -> f32 {
    var p = fract(p0 * vec2<f32>(123.34, 456.21));
    p = p + dot(p, p + 45.32);
    return fract(p.x * p.y);
}

fn field(uv: vec2<f32>, t: f32, fa: f32, tan12: f32) -> f32 {
    let s = uv.x - uv.y * tan12;
    let b = 0.5 + 0.25 * sin(TAU * (s * 0.55 - t * 0.011)) + 0.25 * sin(TAU * (s * 1.30 - t * 0.017) + 1.7);
    let q = vec2<f32>(s * 2.2 + 0.37, uv.y * 1.6 + 0.29);
    let id = floor(q);
    let f = fract(q);
    let tri = step(1.0, f.x + f.y);
    let key = id * 2.0 + vec2<f32>(tri, 0.0);
    let ph = hash(key + 7.1);
    let e = t / 24.0 + ph;
    let k = floor(e);
    let w = smoothstep(0.0, 1.0, fract(e));
    let v = mix(hash(key + k * 13.7), hash(key + (k + 1.0) * 13.7), w);
    return b * 0.022 + (v - 0.5) * 0.018 * fa;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let res = in.size;
    // Pixel position, y up (as the look book's gl_FragCoord).
    let px = vec2<f32>(in.uv.x * res.x, (1.0 - in.uv.y) * res.y);
    var time = globals.time + clock.x;
    if (clock.y >= 0.0) {
        time = clock.y;
    }
    let uv = px / res.y;
    let xn = px.x / res.x;
    let tan12 = place.w;
    let m = smoothstep(place.x - 0.04, place.x + 0.20, xn);
    let g = mix(field(uv, time * 0.06, 0.4, tan12), field(uv, time, 1.0, tan12), m);
    let aspect = res.x / res.y;
    let c = vec2<f32>(aspect * (place.y + 0.025 * sin(time * 0.043)), place.z + 0.03 * sin(time * 0.031 + 1.0));
    let d = (uv - c) * vec2<f32>(0.85, 1.15);
    let bloom = exp(-dot(d, d) * 2.6);
    var col = ink.rgb + fg.rgb * g * (0.75 + 1.6 * bloom) + acc.rgb * bloom * 0.15;
    col += acc.rgb * 0.025 * (1.0 - smoothstep(0.0, 0.35, uv.y));
    let vq = px / res - 0.5;
    col *= 1.0 - 0.35 * dot(vq, vq);
    col += (hash(px + fract(time) * 61.0) - 0.5) / 255.0;
    return vec4<f32>(to_linear(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0))), clock.z);
}
