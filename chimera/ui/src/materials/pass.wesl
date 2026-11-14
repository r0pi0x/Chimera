// PassMaterial (materials/mod.rs): the logo pass between screens, one
// full-screen node. Four bands slanted at the theme's angle sweep across
// (accent hairline, the ink band carrying the mark, an accent band, a
// light band) at `from + travel * hold(p)` cq; the mark wipes in and out
// inside the ink band. Sizes in cq of a 16:9 frame (1 cq = height / 56.25);
// wider frames stretch the bands so they still cover.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> ink: vec4<f32>;
@group(1) @binding(1) var<uniform> acc: vec4<f32>;
@group(1) @binding(2) var<uniform> fg: vec4<f32>;
// x: progress 0-1, y: tan of the slant, z: has a mark (0/1), w: unused.
@group(1) @binding(3) var<uniform> state: vec4<f32>;
// x: from, y: travel, z: mark x in the ink band, w: ink band width.
@group(1) @binding(4) var<uniform> sweep: vec4<f32>;
// x: accent hairline offset, y: its width, z: accent band offset, w: width.
@group(1) @binding(5) var<uniform> bands: vec4<f32>;
// x: light band offset, y: width, z: mark width, w: mark height.
@group(1) @binding(6) var<uniform> bands2: vec4<f32>;
// x, y: the mark's wipe in (start, length); z, w: wipe out (start, length).
@group(1) @binding(7) var<uniform> mark_wipe: vec4<f32>;
@group(1) @binding(8) var mark_texture: texture_2d<f32>;
@group(1) @binding(9) var mark_sampler: sampler;

const FRAME_H: f32 = 56.25;

fn hold(p: f32) -> f32 {
    let q = 2.0 * clamp(p, 0.0, 1.0) - 1.0;
    return 0.5 + 0.5 * sign(q) * pow(abs(q), 3.0);
}

fn expo_out(p: f32) -> f32 {
    let q = clamp(p, 0.0, 1.0);
    if (q >= 1.0) { return 1.0; }
    return 1.0 - pow(2.0, -10.0 * q);
}

fn expo_in(p: f32) -> f32 {
    let q = clamp(p, 0.0, 1.0);
    if (q <= 0.0) { return 0.0; }
    return pow(2.0, 10.0 * q - 10.0);
}

// Coverage of [x0, x0 + w) at `u`, `du` units a pixel.
fn band(u: f32, x0: f32, w: f32, du: f32) -> f32 {
    return clamp((u - x0) / du + 0.5, 0.0, 1.0) * clamp((x0 + w - u) / du + 0.5, 0.0, 1.0);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = state.x;
    let tan = state.y;
    let px = in.uv * in.size;
    let cq = in.size.y / FRAME_H;
    let wcq = in.size.x / cq;
    let s = max(1.0, wcq / 100.0);
    let off = (wcq - 100.0 * s) * 0.5;
    let x = px.x / cq;
    let y = px.y / cq;
    let u = (x - off - (FRAME_H - y) * tan) / s;
    let du = 1.0 / (cq * s);
    let x0 = sweep.x + sweep.y * hold(p);

    var col = vec4<f32>(0.0);
    let k_acc = band(u, x0 + bands.x, bands.y, du);
    let k_ink = band(u, x0, sweep.w, du);
    let k_acc2 = band(u, x0 + bands.z, bands.w, du);
    let k_fg = band(u, x0 + bands2.x, bands2.y, du);
    col = mix(col, acc, k_acc);
    col = mix(col, ink, k_ink);
    col = mix(col, acc, k_acc2);
    col = mix(col, fg, k_fg);

    if (state.z > 0.5 && k_ink > 0.0) {
        // The mark, upright, centred at the frame's mid-height row of the band.
        let mx = off + s * (x0 + sweep.z) + (FRAME_H * 0.5) * tan;
        let my = (FRAME_H - bands2.w) * 0.5;
        let mu = (x - mx) / bands2.z;
        let mv = (y - my) / bands2.w;
        let shown_from = expo_in((p - mark_wipe.z) / mark_wipe.w);
        let shown_to = expo_out((p - mark_wipe.x) / mark_wipe.y);
        if (mu >= 0.0 && mu < 1.0 && mv >= 0.0 && mv < 1.0 && mu >= shown_from && mu < shown_to) {
            let m = textureSampleLevel(mark_texture, mark_sampler, vec2<f32>(mu, mv), 0.0);
            col = vec4<f32>(mix(col.rgb, fg.rgb, m.a), col.a);
        }
    }
    if (col.a <= 0.0) {
        discard;
    }
    return col;
}
