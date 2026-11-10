// Prop shadows on the world (multiplied over it): the caster's coverage
// blurred over 5 taps, less the distance fade, darkens toward the shadow
// colour; fog fades the shadow out (specs/cs_source/shadows_sky.md D).
//
// Two kinds of mesh: a resting caster's shadow is its clipped geometry with
// the atlas coordinates in UV0 and the fade in the vertex colour's red; a
// moving one's (render::shadows::Projected, no UV0) is the receiver geometry
// around it, projected here from its frame in the frame table (`frames`,
// at the entity's mesh tag): the same coordinates and fade, per pixel, and
// nothing outside the frame's box or in front of the caster's lit faces.

import bevy_pbr::{
    render::forward_io::VertexOutput,
    render::mesh_functions::get_tag,
    render::view_transformations::position_world_to_view,
};

struct ShadowParams {
    color: vec4<f32>,
    texel: vec2<f32>,
    fog_color: vec4<f32>,
    fog_range: vec4<f32>,
}

// render::shadows::ShadowFrame::gpu.
struct Frame {
    // Origin, fade start.
    origin: vec4<f32>,
    // Axis, width.
    x: vec4<f32>,
    // Axis, height.
    y: vec4<f32>,
    // Direction, reach.
    d: vec4<f32>,
    // Atlas cell: origin, size.
    rect: vec4<f32>,
    // The caster's light-facing planes: n . p <= w.
    faces: array<vec4<f32>, 3>,
    // The geometry's lift off its surface, the fade's amount.
    misc: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: ShadowParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var atlas_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var<storage, read> frames: array<Frame>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = vec2<f32>(0.0);
    var fade = 0.0;
    var inside = true;
    @if(VERTEX_UVS_A) {
        uv = in.uv;
    }
    @if(VERTEX_COLORS) {
        fade = in.color.r;
    }
    @if(!VERTEX_UVS_A) {
        let f = frames[get_tag(in.instance_index)];
        let n = normalize(in.world_normal);
        // Where on the surface (the geometry is lifted off it).
        let p = in.world_position.xyz - n * f.misc.x;
        let r = p - f.origin.xyz;
        let local = vec3<f32>(dot(f.x.xyz, r), dot(f.y.xyz, r), dot(f.d.xyz, r));
        inside = abs(local.x) <= f.x.w * 0.5 && abs(local.y) <= f.y.w * 0.5 && local.z >= 0.0 && local.z <= f.d.w;
        for (var i = 0u; i < 3u; i = i + 1u) {
            inside = inside && dot(f.faces[i].xyz, p) <= f.faces[i].w;
        }
        uv = f.rect.xy + vec2<f32>(local.x / f.x.w + 0.5, local.y / f.y.w + 0.5) * f.rect.zw;
        let span = max(f.d.w - f.origin.w, 1e-4);
        fade = f.misc.y * clamp((local.z - f.origin.w) / span, 0.0, 1.0);
    }
    let t = params.texel;
    let sum = textureSample(atlas, atlas_sampler, uv).r
        + textureSample(atlas, atlas_sampler, uv + t).r
        + textureSample(atlas, atlas_sampler, uv - t).r
        + textureSample(atlas, atlas_sampler, uv + vec2<f32>(t.x, -t.y)).r
        + textureSample(atlas, atlas_sampler, uv + vec2<f32>(-t.x, t.y)).r;
    if !inside {
        discard;
    }
    let coverage = clamp(sum / 5.0 - fade, 0.0, 1.0);
    var result = mix(vec3<f32>(1.0), params.color.rgb, coverage);
    if params.fog_color.w > 0.5 {
        let depth = -position_world_to_view(in.world_position.xyz).z;
        let f = clamp(min(params.fog_range.z, (depth - params.fog_range.x) / (params.fog_range.y - params.fog_range.x)), 0.0, 1.0);
        // The raw fog factor (not the squared blend amount).
        let k = pow(1.0 - f, 4.0);
        result = vec3<f32>(1.0) - (vec3<f32>(1.0) - result) * k;
    }
    return vec4<f32>(result, 1.0);
}
