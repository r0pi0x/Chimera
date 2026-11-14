// TickBarMaterial (materials/mod.rs): `n` ticks, the first `lit` lit, the
// lit ones from `hot` on in the hot colour. A tick is chosen by whole
// physical pixel column (`units::TickLayout`), so ticks never blur at any
// scale. Optionally skewed at the slant, a whole pixel per row step.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> on: vec4<f32>;
@group(1) @binding(1) var<uniform> off: vec4<f32>;
@group(1) @binding(2) var<uniform> hot: vec4<f32>;
// x: n, y: lit, z: hot from, w: pitch (physical px).
@group(1) @binding(3) var<uniform> counts: vec4<f32>;
// x: tick width, y: tick height (physical px), z: tan of the skew (0:
// upright), w: unused.
@group(1) @binding(4) var<uniform> dims: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let px = floor(in.uv * in.size);
    let th = dims.y;
    let y0 = floor((in.size.y - th) * 0.5);
    let yy = px.y - y0;
    if (yy < 0.0 || yy >= th) {
        discard;
    }
    let shift = floor((th - 1.0 - yy) * dims.z);
    let x = px.x - shift;
    let pitch = max(counts.w, 1.0);
    let i = floor(x / pitch);
    if (x < 0.0 || i >= counts.x || x - i * pitch >= dims.x) {
        discard;
    }
    var col = off;
    if (i < counts.y) {
        col = on;
        if (i >= counts.z) {
            col = hot;
        }
    }
    return col;
}
