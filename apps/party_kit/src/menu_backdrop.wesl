// The party menu's own backdrop (menu_backdrop.rs): drawn whenever no map
// scene is behind the menu. Original, all in code: a dark slate gradient in
// the menu's colours with a warm and a cool glow, three layers of soft
// diagonal bands drifting at different speeds (parallax), a halftone dot
// field rising from the foot, a slowly turning reticle of dashed rings,
// a vignette and a dither against banding. A few dozen ALU operations a
// pixel and no texture reads.

import bevy_render::globals::Globals;
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(0) @binding(1) var<uniform> globals: Globals;
// x: opacity (the crossfade to a map scene), yzw unused.
@group(1) @binding(0) var<uniform> params: vec4<f32>;

const TAU: f32 = 6.28318530718;

fn lin(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3<f32>(2.2));
}

fn glow(p: vec2<f32>, at: vec2<f32>, spread: f32) -> f32 {
    let d = p - at;
    return exp(-dot(d, d) * spread);
}

// Soft bands across `p` along `n`: `freq` bands a unit, moving at `speed`.
fn bands(p: vec2<f32>, n: vec2<f32>, freq: f32, speed: f32, width: f32, t: f32) -> f32 {
    let x = fract(dot(p, n) * freq + t * speed);
    let d = abs(x - 0.5);
    return 1.0 - smoothstep(width * 0.5, width * 0.5 + 0.08, d);
}

// A dashed ring at radius `r`, `dashes` dashes turning at `spin`.
fn ring(p: vec2<f32>, r: f32, thick: f32, dashes: f32, spin: f32, t: f32) -> f32 {
    let d = abs(length(p) - r);
    let line = 1.0 - smoothstep(thick, thick + 0.0025, d);
    let a = atan2(p.y, p.x) / TAU + 0.5;
    let dash = step(0.35, fract(a * dashes + t * spin));
    return line * dash;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    let aspect = in.size.x / max(in.size.y, 1.0);
    // Centered, height 1.
    let p = (in.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);

    // Composed in display (gamma) values, made linear at the end: the
    // menu's colours as they show.
    // The gradient: slate at the top to near black at the foot.
    var col = mix(vec3<f32>(0.135, 0.150, 0.175), vec3<f32>(0.045, 0.050, 0.058), in.uv.y);
    // A warm glow low on the right, a cool one high on the left; the warm
    // one breathes slowly.
    let breathe = 0.85 + 0.15 * sin(t * 0.21);
    col += vec3<f32>(0.78, 0.42, 0.14) * 0.26 * breathe * glow(p, vec2<f32>(0.42 * aspect, 0.38), 2.2);
    col += vec3<f32>(0.20, 0.42, 0.52) * 0.15 * glow(p, vec2<f32>(-0.38 * aspect, -0.42), 3.0);

    // Parallax bands: far (thin, slow), middle, near (wide, faster).
    let n = vec2<f32>(0.940, 0.342);
    let band = bands(p, n, 9.0, 0.010, 0.06, t) * 0.012
        + bands(p, n, 4.5, 0.022, 0.10, t) * 0.018
        + bands(p, n, 1.6, 0.045, 0.16, t) * 0.026;
    col += vec3<f32>(0.95, 0.80, 0.62) * band;

    // Halftone dots, larger towards the foot, drifting sideways.
    let g = p * 34.0 + vec2<f32>(t * 0.15, 0.0);
    let cell = fract(g) - vec2<f32>(0.5);
    let r = 0.30 * clamp(in.uv.y * 1.3 - 0.25, 0.0, 1.0);
    let dot_ = 1.0 - smoothstep(r - 0.06, r, length(cell));
    col += vec3<f32>(0.85, 0.55, 0.30) * dot_ * 0.028;

    // The reticle: two dashed rings and a crosshair, turning slowly.
    let c = p - vec2<f32>(0.30 * aspect, 0.02);
    let reticle = ring(c, 0.34, 0.0018, 24.0, 0.004, t)
        + ring(c, 0.255, 0.0012, 60.0, -0.006, t) * 0.7
        + (1.0 - smoothstep(0.0012, 0.0037, min(abs(c.x), abs(c.y))))
            * step(0.04, length(c)) * (1.0 - smoothstep(0.12, 0.16, length(c)));
    col += vec3<f32>(0.95, 0.62, 0.30) * reticle * 0.07;

    // Vignette, a dither against the gradient's banding, then linear.
    col *= 1.0 - 0.45 * dot(p, p);
    let h = fract(sin(dot(in.position.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    col += vec3<f32>((h - 0.5) / 255.0);
    return vec4<f32>(lin(max(col, vec3<f32>(0.0))), params.x);
}
