// SlantMaterial (materials/mod.rs): a parallelogram fill at the theme's
// slant, edges anti-aliased over one physical pixel, an optional accent bar
// along the left edge, a wipe that clips from the left and the right along
// the slant, and a horizontal alpha ramp. Positions in physical pixels
// (`UiVertexOutput::size`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> fill: vec4<f32>;
@group(1) @binding(1) var<uniform> accent: vec4<f32>;
// x: tan of the slant, y: left edge slanted (0/1), z: right edge slanted
// (0/1), w: the accent bar's width in physical px.
@group(1) @binding(2) var<uniform> shape: vec4<f32>;
// x: clip from the left (0-1 of the row), y: clip from the right, z, w:
// alpha at the left and the right edge.
@group(1) @binding(3) var<uniform> wipe: vec4<f32>;
// x: a wipe's cut leans with the slant (0/1) even where the fill's own
// edges are upright: only while that side clips.
@group(1) @binding(4) var<uniform> cut: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * in.size;
    let w = in.size.x;
    let h = in.size.y;
    let tan = shape.x;
    // Leaning right: the top edge sits `h * tan` right of the bottom one.
    let xl = shape.y * (h - p.y) * tan;
    let xr = w - shape.z * p.y * tan;
    let span = max(xr - xl, 0.0);
    let lean = cut.x * (h - p.y) * tan;
    let a = xl + wipe.x * span + select(0.0, lean, wipe.x > 0.0);
    let b = xr - wipe.y * span + select(0.0, lean, wipe.y > 0.0);
    // Horizontal distance to a slanted edge times cos(angle) is the
    // perpendicular distance; one physical pixel of ramp.
    let c = inverseSqrt(1.0 + tan * tan);
    let cov = clamp((p.x - a) * c + 0.5, 0.0, 1.0) * clamp((b - p.x) * c + 0.5, 0.0, 1.0);
    if (cov <= 0.0) {
        discard;
    }
    var col = fill;
    if (shape.w > 0.0) {
        let k = clamp(xl + shape.w - p.x + 0.5, 0.0, 1.0);
        col = mix(fill, accent, k);
    }
    let ramp = mix(wipe.z, wipe.w, in.uv.x);
    return vec4<f32>(col.rgb, col.a * cov * ramp);
}
