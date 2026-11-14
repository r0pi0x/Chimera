// BackdropsMaterial (menu_backdrops.rs): the menu's backdrops shown in
// their area, the right part of the window. Two canvases (premultiplied:
// painted over a clear transparent image): `a` the current backdrop, `b`
// the one it swaps from. While a swap runs the current one shows left of
// a 12° line whose foot is at `wipe.x` (screen px) with a 3 px accent edge
// right of it, the previous one beyond. The whole fades out toward the
// menu column (the look book's ramp: 0 at the left edge, .08 at 27 % of
// the width, 1 from 46 %).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> acc: vec4<f32>;
// x: the wipe's x at the foot (screen px), y: on (0/1), z: the edge's
// width (px), w: tan of the slant.
@group(1) @binding(1) var<uniform> wipe: vec4<f32>;
// x: the area's left (screen px), y, z: the screen's size (px), w: opacity.
@group(1) @binding(2) var<uniform> frame: vec4<f32>;
@group(1) @binding(3) var a_texture: texture_2d<f32>;
@group(1) @binding(4) var a_sampler: sampler;
@group(1) @binding(5) var b_texture: texture_2d<f32>;
@group(1) @binding(6) var b_sampler: sampler;
// The current backdrop's sheen: band position t = x * sx + y * sy + c
// (0-1 across it), w its strength (0: none).
@group(1) @binding(7) var<uniform> sheen: vec4<f32>;
@group(1) @binding(8) var<uniform> fg: vec4<f32>;

// The sheen's profile (the look book's stops), display-space alpha.
fn sheen_at(t: f32) -> f32 {
    let s = array<vec2<f32>, 11>(
        vec2(0.0, 0.0), vec2(0.2, 0.015), vec2(0.35, 0.05), vec2(0.44, 0.13), vec2(0.49, 0.3), vec2(0.5, 0.38),
        vec2(0.51, 0.3), vec2(0.56, 0.13), vec2(0.65, 0.05), vec2(0.8, 0.015), vec2(1.0, 0.0),
    );
    if (t <= 0.0 || t >= 1.0) { return 0.0; }
    for (var i = 1; i < 11; i++) {
        if (t <= s[i].x) {
            return mix(s[i - 1].y, s[i].y, (t - s[i - 1].x) / (s[i].x - s[i - 1].x));
        }
    }
    return 0.0;
}

fn fade(xn: f32) -> f32 {
    if (xn <= 0.0) { return 0.0; }
    if (xn < 0.27) { return 0.08 * xn / 0.27; }
    if (xn < 0.46) { return mix(0.08, 1.0, (xn - 0.27) / 0.19); }
    return 1.0;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let local = in.uv * in.size;
    let x = frame.x + local.x;
    let y = local.y;
    var c = textureSample(a_texture, a_sampler, in.uv);
    if (sheen.w > 0.0) {
        // Over what is painted only (the facets), blended as the look
        // book's (display-space alpha, as light_alpha).
        let k = pow(sheen_at(x * sheen.x + y * sheen.y + sheen.z) * sheen.w, 2.2);
        let m = smoothstep(0.02, 0.25, c.a);
        c = c * (1.0 - k * m) + vec4<f32>(fg.rgb, 1.0) * c.a * k * m;
    }
    let prev = textureSample(b_texture, b_sampler, in.uv);
    if (wipe.y > 0.5) {
        // The wipe's line: at the foot `wipe.x`, leaning right going up.
        let xb = wipe.x + (frame.z - y) * wipe.w;
        let k = clamp(x - xb + 0.5, 0.0, 1.0);
        let e = clamp(x - xb - wipe.z + 0.5, 0.0, 1.0);
        let edge = vec4<f32>(acc.rgb, 1.0);
        c = mix(c, mix(edge, prev, e), k);
    }
    let a = c.a * fade(x / frame.y) * frame.w;
    if (a <= 0.0) {
        discard;
    }
    return vec4<f32>(c.rgb / max(c.a, 1e-4), a);
}
