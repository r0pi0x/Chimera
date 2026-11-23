// Runtime decals (bullet holes, slashes): the framebuffer is multiplied by
// a factor F. The blend is the game's own for these decals, src x dst +
// dst x src, so the shader returns F / 2: F may exceed 1 (brightening)
// while the output stays within an 8-bit target's [0, 1], up to F = 2.
//
// Modulate-2x decals (specs/cs_source/overlays_decals.md, "Decal
// blending"): the game multiplies its gamma-space framebuffer by 2 x the
// texel, so 0.5 grey leaves the surface unchanged and lighter texels
// brighten it. Our framebuffer is linear, so the same change is that
// factor raised to 2.2 (clamped at 2 here; see tech-debt). No alpha.
//
// Other decals ("lit" decals, alpha blended and lightmapped in the game)
// are approximated the same way, as a stain: factor = texel colour where
// the texel is opaque; on glass (where a stain on what shows through
// wouldn't show) they are alpha-blended, lit by the light probe there.
//
// Halo's framebuffer blend functions (specs/halo_ce/shaders.md): the
// game's value src = texel x the decal's colour (gamma space), moved
// toward the blend's neutral value (1/2 for double multiply, white for
// multiply and min, black for add, subtract and max) by 1 minus its
// strength, edge fade and fog, then multiply (src x dst), add (src +
// dst), subtract (dst - src), min or max per channel by the pipeline's
// blend state (`DecalMaterial::specialize`). Their output is src in
// linear light: exact for multiply, min and max; add and subtract are in
// linear light, not the game's gamma space.

import bevy_pbr::render::forward_io::VertexOutput;
import chimera_present::render::fog::{source_fog, fog_mix};

struct DecalParams {
    // The blend (render::decal::blend_code): 0 stain, 1 modulate 2x, 2
    // multiply, 3 add, 4 subtract, 5 min, 6 max.
    blend: f32,
    // The fog fade's power (map::decal::MapDecal::fog_fade; 0: Source's
    // 0.4).
    fog_fade: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: DecalParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var base_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var base_sampler: sampler;

struct FogUniform {
    color: vec4<f32>,
    range: vec4<f32>,
    water_color: vec4<f32>,
    water_range: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(3) var<uniform> fog: FogUniform;
// rgb: the surface's light; w 1: a lit decal drawn alpha-blended (on
// glass), texel x light over what is there.
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var<uniform> light: vec4<f32>;
// rgb: the decal's colour (multiplies the texel); w: its strength.
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var<uniform> tint: vec4<f32>;

// Share of the decal's width over which its edge fades out (about 3
// texels of a 64-texel decal).
const EDGE_FADE: f32 = 0.05;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(base_texture, base_sampler, in.uv);
    if light.w > 0.5 {
        // Lit, alpha-blended (the game's lightmapped alpha decals): the
        // texel lit by the light where it lies, fogged like a surface.
        var alpha = texel.a * tint.w;
        @if(VERTEX_UVS_B) {
            let e = min(min(in.uv_b.x, 1.0 - in.uv_b.x), min(in.uv_b.y, 1.0 - in.uv_b.y));
            alpha = alpha * smoothstep(0.0, EDGE_FADE, e);
        }
        if alpha < 0.004 {
            discard;
        }
        let lf = source_fog(fog.color, fog.range, fog.water_color, fog.water_range, in.world_position.xyz);
        let lit = fog_mix(texel.rgb * tint.rgb * light.rgb, lf, fog.range);
        return vec4(lit, alpha);
    }
    // The texture is sRGB: back to the game's gamma-space value, times
    // the decal's colour.
    let gamma = pow(max(texel.rgb, vec3(0.0)), vec3(1.0 / 2.2)) * tint.rgb;
    let blend = u32(params.blend + 0.5);
    // How much of the decal shows: its strength, the edge fade and fog.
    var k = clamp(tint.w, 0.0, 1.0);
    // Fade to neutral over the decal's outer edge: decals share an atlas,
    // and filtering (most of all the small mip levels seen from afar)
    // pulls in the neighbouring decal's texels there, as dark lines.
    @if(VERTEX_UVS_B) {
        let t = in.uv_b;
        let edge = min(min(t.x, 1.0 - t.x), min(t.y, 1.0 - t.y));
        k = k * smoothstep(0.0, EDGE_FADE, edge);
    }
    // Fog: toward neutral by the amount raised to the game's power (mirror
    // `render::decal::fog_fade`). Source's (specs/cs_source/shaders.md
    // section 5, DecalModulate): toward 0.5 grey by the fog factor raised
    // to 0.4 before section 7's use of it: (f^0.4)^2 for range fog, f^0.4
    // for the water's height fog; both are the surface's amount^0.4.
    // Halo's (specs/halo_ce/shaders.md "Fade and fog stage"): double
    // multiply toward 1/2, multiply toward white, add toward black, by the
    // amount itself (linear fog: `source_fog`'s f), power 1.
    let f = source_fog(fog.color, fog.range, fog.water_color, fog.water_range, in.world_position.xyz);
    let fade = select(0.4, params.fog_fade, params.fog_fade > 0.0);
    k = k * (1.0 - pow(clamp(f.a, 0.0, 1.0), fade));
    if blend >= 2u {
        // Halo's other blend functions: src toward its neutral value.
        let neutral = select(vec3(0.0), vec3(1.0), blend == 2u || blend == 5u);
        let src = mix(neutral, gamma, k);
        // Nothing to show: skip the blend.
        if all(abs(src - neutral) < vec3(0.008)) {
            discard;
        }
        return vec4(pow(max(src, vec3(0.0)), vec3(2.2)), 1.0);
    }
    var factor: vec3<f32>;
    if blend == 1u {
        factor = 2.0 * gamma;
    } else {
        factor = mix(vec3(1.0), gamma, texel.a);
    }
    factor = mix(vec3(1.0), factor, k);
    // Nothing to show: skip the blend.
    if all(abs(factor - vec3(1.0)) < vec3(0.02)) {
        discard;
    }
    return vec4(min(pow(factor, vec3(2.2)), vec3(2.0)) * 0.5, 1.0);
}
