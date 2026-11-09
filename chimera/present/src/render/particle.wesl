// Particles: texture x vertex colour (the particle's colour and alpha),
// alpha-blended or added, then fogged: blended particles toward the fog
// colour, added ones toward black (Source's sprite-card fog). The fog is
// the world's range fog and, below the water's surface, the water's
// (render::fog).

import bevy_pbr::{
    render::forward_io::VertexOutput,
    render::mesh_view_bindings as vb,
    render::view_transformations::depth_ndc_to_view_z,
};
import chimera_present::render::fog::source_fog;

struct FogUniform {
    color: vec4<f32>,
    range: vec4<f32>,
    water_color: vec4<f32>,
    water_range: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var base_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var base_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> fog: FogUniform;

// `$depthblend`'s distance over which a particle fades out as it nears the
// world behind it, in meters: 24 game units (ours; the spec gives no
// distance and the materials that ask name no scale; 50 showed thin things
// such as pipes through the smoke in front of them).
const SOFT_DISTANCE: f32 = 0.6096;

// How much of the particle shows at this pixel for what lies behind it in
// the depth prepass (reversed depth, 0 where nothing was drawn). Views
// without a prepass cut it at the world, as materials without the flag.
fn soft_depth(position: vec4<f32>) -> f32 {
    @if(SOFT_DEPTH && DEPTH_PREPASS) {
        let scene = textureLoad(vb::depth_prepass_texture, vec2<i32>(position.xy), 0);
        if scene <= 0.0 {
            return 1.0;
        }
        let gap = depth_ndc_to_view_z(position.z) - depth_ndc_to_view_z(scene);
        return clamp(gap / SOFT_DISTANCE, 0.0, 1.0);
    } @else {
        return 1.0;
    }
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var c = textureSample(base_texture, base_sampler, in.uv);
    @if(VERTEX_COLORS) {
        c = c * in.color;
    }
    c.a *= soft_depth(in.position);
    let f = source_fog(fog.color, fog.range, fog.water_color, fog.water_range, in.world_position.xyz);
    @if(ADDITIVE) {
        // Premultiplied blending with alpha 0 adds.
        return vec4<f32>(c.rgb * c.a * (1.0 - f.a), 0.0);
    } @else {
        return vec4<f32>(mix(c.rgb, f.rgb, f.a), c.a);
    }
}
