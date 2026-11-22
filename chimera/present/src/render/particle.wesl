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
import chimera_present::render::fog::{source_fog, fog_mix};

struct FogUniform {
    color: vec4<f32>,
    range: vec4<f32>,
    water_color: vec4<f32>,
    water_range: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var base_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var base_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> fog: FogUniform;
// x: overbright x `$addself` (premultiplied sprite cards); y: 1 on an
// integer-HDR map (`map::HdrModel::Integer`).
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var<uniform> add_self: vec4<f32>;

// Integer HDR (specs/tf2/hdr_bloom.md section 7): the game writes each
// fragment times the tone-map scale k to an 8-bit target, clamped to 1
// before blending. Our frame holds the scene before k (the tone-map pass
// scales it), so a fragment is clamped to 1 / k; k rides in the view's
// colour-grading exposure (log2 k, `client::hdr`). Elsewhere the 8-bit or
// float target is left as it was.
fn integer_clamp(rgb: vec3<f32>) -> vec3<f32> {
    if add_self.y > 0.5 {
        return min(rgb, vec3<f32>(exp2(-vb::view.color_grading.exposure)));
    }
    return rgb;
}

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
    @if(VERTEX_UVS_B) {
        // A sheet's next frame mixed in (specs/tf2/particles.md 3): UV1 is
        // its coordinates with the mix in v as 4 x step + 1 + v, the step
        // in 1/255 (particles.rs `encode_next_uv`).
        let step = floor(in.uv_b.y / 4.0);
        let next_uv = vec2<f32>(in.uv_b.x, in.uv_b.y - 4.0 * step - 1.0);
        let next = textureSample(base_texture, base_sampler, next_uv);
        c = mix(c, next, step / 255.0);
    }
    var vertex_alpha = 1.0;
    @if(VERTEX_COLORS) {
        c = c * in.color;
        vertex_alpha = in.color.a;
    }
    let soft = soft_depth(in.position);
    c.a *= soft;
    vertex_alpha *= soft;
    let f = source_fog(fog.color, fog.range, fog.water_color, fog.water_range, in.world_position.xyz);
    @if(PREMULTIPLIED) {
        // SpriteCard's `$addself` (specs/tf2/particles.md 2): the colour
        // over its own coverage, brightened by its alpha, added over the
        // scene dimmed by that coverage (the overbright is in the
        // particle's colour).
        let rgb = c.rgb * c.a * (1.0 + add_self.x * vertex_alpha);
        return vec4<f32>(integer_clamp(rgb * (1.0 - f.a)), c.a);
    }
    @if(ADDITIVE) {
        // Premultiplied blending with alpha 0 adds.
        return vec4<f32>(integer_clamp(c.rgb * c.a * (1.0 - f.a)), 0.0);
    } @else {
        return vec4<f32>(integer_clamp(fog_mix(c.rgb, f, fog.range)), c.a);
    }
}
