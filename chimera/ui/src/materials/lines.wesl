// LinesMaterial (materials/mod.rs): the match end's broad slanted planes
// (look book V's rule): four layers of planes `w` units wide, w in {16,
// 32, 64, 128} (1 unit = 1/192 of the width), each repeating every 3 w
// (a third covered, so the layers overlap), each moving one spacing per
// `loop` s right to left (the widest drift fastest), at 12°. Each layer
// adds `alpha` (display space) of `fg` over the ground it lies on
// (`ground`, display space); the coverage is the exact overlap of a
// pixel's column with a plane's span on its row. Blended in linear light,
// so the alpha written is the one that lands on the display-space mix.

import bevy_render::globals::Globals;
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(0) @binding(1) var<uniform> globals: Globals;
// Display-space colours: the planes' and the ground under them.
@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> ground: vec4<f32>;
// x: loop (s per spacing), y: alpha per layer (display space), z: unused, w: tan.
@group(1) @binding(2) var<uniform> params: vec4<f32>;
// x: opacity, y: units across the width, z: time offset (s), w: frozen time (< 0: live).
@group(1) @binding(3) var<uniform> extra: vec4<f32>;

fn overlap(px: f32, a: f32, b: f32) -> f32 {
    return max(0.0, min(px + 1.0, b) - max(px, a));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = floor(in.uv * in.size);
    let w = in.size.x;
    let h = in.size.y;
    let u = w / extra.y;
    let sk = h * params.w;
    // The planes lean right: a row's left edge sits `sk` along at the top.
    let lean = sk * (1.0 - (p.y + 0.5) / h);
    var t = globals.time + extra.z;
    if (extra.w >= 0.0) {
        t = extra.w;
    }
    var cov = 0.0;
    for (var k = 0; k < 4; k++) {
        let wu = 16.0 * pow(2.0, f32(k));
        let bw = round(wu * u);
        let sp = 3.0 * wu * u;
        let raw = sp * f32(k) / 4.0 - sp * (t / params.x);
        let off = raw - sp * floor(raw / sp);
        // The plane whose span may hold this pixel, and its neighbour.
        let n = floor((p.x - lean - off) / sp);
        for (var j = -1; j <= 0; j++) {
            let x0 = round(off + (n + f32(j)) * sp) + lean;
            cov += overlap(p.x, x0, x0 + bw);
        }
    }
    let a = cov * params.y * extra.x;
    if (a <= 0.0) {
        discard;
    }
    // The display-space mix over the ground, as a linear-light alpha.
    let target_lin = to_linear(clamp(ground.rgb + (color.rgb - ground.rgb) * a, vec3<f32>(0.0), vec3<f32>(1.0)));
    let g = to_linear(ground.rgb);
    let f = to_linear(color.rgb);
    let lin_a = dot(target_lin - g, vec3<f32>(1.0)) / max(dot(f - g, vec3<f32>(1.0)), 1e-5);
    return vec4<f32>(f, clamp(lin_a, 0.0, 1.0));
}
