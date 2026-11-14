// EdgeMaterial (materials/mod.rs): the screen's edges glowing, as an inset
// shadow (`blur` wide from `spread` in) and a hairline border, times an
// opacity. The middle is discarded: the view stays clear. Positions in
// physical pixels (`UiVertexOutput::size`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> color: vec4<f32>;
// x: opacity, y: blur, z: spread, w: border width (physical px).
@group(1) @binding(1) var<uniform> params: vec4<f32>;
// x: the glow's alpha, y: the border's.
@group(1) @binding(2) var<uniform> alphas: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * in.size;
    let d = min(min(p.x, in.size.x - p.x), min(p.y, in.size.y - p.y));
    let half = max(params.y * 0.5, 1.0);
    let glow = alphas.x * (1.0 - smoothstep(params.z - half, params.z + half, d));
    let border = alphas.y * clamp(params.w - d + 0.5, 0.0, 1.0);
    let a = (1.0 - (1.0 - glow) * (1.0 - border)) * params.x;
    if (a <= 0.002) {
        discard;
    }
    return vec4<f32>(color.rgb, a);
}
