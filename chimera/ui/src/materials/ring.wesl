// RingMaterial (materials/mod.rs): a countdown ring in the node's square,
// the arc from the top clockwise over its `share` of the ring, the rest
// the track. Positions in physical pixels (`UiVertexOutput::size`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> track: vec4<f32>;
@group(1) @binding(1) var<uniform> arc: vec4<f32>;
// x: the arc's share (0-1), y: the line's width (physical px).
@group(1) @binding(2) var<uniform> params: vec4<f32>;

const TAU: f32 = 6.28318530718;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * in.size;
    let c = in.size * 0.5;
    let w = params.y;
    let r = min(in.size.x, in.size.y) * 0.5 - w * 0.5 - 0.5;
    let v = p - c;
    let cov = clamp(w * 0.5 - abs(length(v) - r) + 0.5, 0.0, 1.0);
    if (cov <= 0.0) {
        discard;
    }
    // From the top, clockwise (y down).
    var f = atan2(v.x, -v.y) / TAU;
    if (f < 0.0) {
        f = f + 1.0;
    }
    var col = track;
    if (f <= params.x) {
        col = arc;
    }
    return vec4<f32>(col.rgb, col.a * cov);
}
