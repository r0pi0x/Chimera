// The stand-ins' loading look (render/stand_in.rs): a light, faceted
// surface, its accent parts (vertex colour alpha 1) in the accent colour,
// light key-and-fill shading of our own (no map light: a stand-in reads
// the same on every map, dark or bright), light diagonal stripes that
// scroll slowly along the model's stripe axis (UV x, metres;
// `params.scroll` in stripes, advanced on the CPU so tests can hold it
// still), and an accent rim toward the edges. Says "something comes
// here", not "something broke".

import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::view;

struct StandInParams {
    base: vec4<f32>,
    accent: vec4<f32>,
    // rgb: the stripes' colour; a: how strongly they show.
    stripe: vec4<f32>,
    // Stripes moved so far (fractions of a period).
    scroll: f32,
    // Stripes per metre.
    density: f32,
    // The stripes' share of a period (0-1).
    duty: f32,
    rim: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: StandInParams;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var tone = vec4<f32>(1.0, 1.0, 1.0, 0.0);
    @if(VERTEX_COLORS) {
        tone = in.color;
    }
    var n = normalize(in.world_normal);
    if !front {
        n = -n;
    }
    // A key light from above and one side, a softer fill from the other.
    let key = max(dot(n, normalize(vec3<f32>(0.4, 0.85, 0.35))), 0.0);
    let fill = max(dot(n, normalize(vec3<f32>(-0.6, 0.2, -0.5))), 0.0);
    let light = 0.42 + 0.5 * key + 0.18 * fill;
    let surface = mix(params.base.rgb, params.accent.rgb, tone.a) * tone.rgb * light;

    // The stripes, anti-aliased by their screen-space width.
    let s = in.uv.x * params.density - params.scroll;
    let f = fract(s);
    let w = max(fwidth(s), 1e-4);
    let band = smoothstep(0.0, w, f) * (1.0 - smoothstep(params.duty, params.duty + w, f));
    let stripe = band * params.stripe.a * (1.0 - tone.a);
    var color = mix(surface, params.stripe.rgb, stripe);

    // The rim: toward the silhouette, the accent.
    let to_eye = normalize(view.world_position - in.world_position.xyz);
    let rim = pow(1.0 - saturate(abs(dot(n, to_eye))), 3.0);
    color = color + params.accent.rgb * rim * params.rim;
    return vec4<f32>(color, 1.0);
}
