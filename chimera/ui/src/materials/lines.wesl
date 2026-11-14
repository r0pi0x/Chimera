// LinesMaterial (materials/mod.rs): the match end's background, one system
// of slanted lines on a 24-column grid moving right to left on the
// renderer's clock. Far layer: hairlines (one physical pixel) in a
// 6-column bar, columns 0, 1 and 3 of each 6 (a 2 + 1 rhythm), moving 6
// columns a loop. Near layer: a half-column slab every 12 columns (from
// column 4), twice as fast. Each pixel's coverage is the exact overlap of
// its column with the line's span on its row (a box filter along x).

import bevy_render::globals::Globals;
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(0) @binding(1) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> color: vec4<f32>;
// x: loop in seconds, y: far alpha, z: near alpha, w: tan of the slant.
@group(1) @binding(1) var<uniform> params: vec4<f32>;
// x: opacity, y: columns, z: time offset (s), w: frozen time (< 0: live).
@group(1) @binding(2) var<uniform> extra: vec4<f32>;

fn overlap(px: f32, a: f32, b: f32) -> f32 {
    return max(0.0, min(px + 1.0, b) - max(px, a));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = floor(in.uv * in.size);
    let w = in.size.x;
    let h = in.size.y;
    let cols = extra.y;
    let pitch = w / cols;
    let sk = h * params.w;
    let row = sk * (1.0 - (p.y + 0.5) / h);
    var t = globals.time + extra.z;
    if (extra.w >= 0.0) {
        t = extra.w;
    }
    let ph = fract(t / params.x);
    var cov_far = 0.0;
    let o1 = 6.0 * pitch * ph;
    let q1 = floor((p.x - row + o1) / pitch);
    for (var k = -1; k <= 1; k++) {
        let c = q1 + f32(k);
        let m = c - 6.0 * floor(c / 6.0);
        if (m == 0.0 || m == 1.0 || m == 3.0) {
            let x = round(c * pitch - o1) + row;
            cov_far += overlap(p.x, x, x + 1.0);
        }
    }
    var cov_near = 0.0;
    let o2 = 12.0 * pitch * ph;
    let slab = round(pitch * 0.5);
    let q2 = floor(((p.x - row + o2) / pitch - 4.0) / 12.0);
    for (var k = -1; k <= 1; k++) {
        let c = (q2 + f32(k)) * 12.0 + 4.0;
        let x = round(c * pitch - o2) + row;
        cov_near += overlap(p.x, x, x + slab);
    }
    let a = (min(cov_far, 1.0) * params.y + min(cov_near, 1.0) * params.z) * extra.x;
    if (a <= 0.0) {
        discard;
    }
    return vec4<f32>(color.rgb, a * color.a);
}
