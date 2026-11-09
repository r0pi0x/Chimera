// World surfaces the way Source's LightmappedGeneric draws them at LDR:
// texture x baked light, plus the scene's point lights (muzzle flashes). With a normal map, the light
// is Valve's radiosity normal mapping ("Shading in Valve's Source Engine",
// SIGGRAPH 2006): three directional lightmaps, weighted per pixel by the
// squared, normalised alignment of the tangent-space normal with three
// fixed basis directions.

import bevy_pbr::{
    render::forward_io::VertexOutput,
    render::mesh_view_bindings::{view, globals},
    render::mesh_view_bindings as view_bindings,
    render::clustered_forward as clustering,
    render::mesh_view_types::POINT_LIGHT_FLAGS_SPOT_LIGHT_Y_NEGATIVE,
    render::view_transformations::position_world_to_view,
};
import chimera_present::render::fog::source_fog;

// A spot light's cone at `to`, the line from the point to the light (as
// Bevy's own: its direction packed in x, z and a flag for y's sign). Same as
// prop.wesl.
fn spot_cone(custom: vec4<f32>, flags: u32, to: vec3<f32>) -> f32 {
    var spot_dir = vec3<f32>(custom.x, 0.0, custom.y);
    spot_dir.y = sqrt(max(0.0, 1.0 - spot_dir.x * spot_dir.x - spot_dir.z * spot_dir.z));
    if (flags & POINT_LIGHT_FLAGS_SPOT_LIGHT_Y_NEGATIVE) != 0u {
        spot_dir.y = -spot_dir.y;
    }
    let cone = saturate(dot(-spot_dir, normalize(to)) * custom.z + custom.w);
    return cone * cone;
}

// The scene's point lights (muzzle flashes; map::DynamicLight) and spot
// lights (a flashlight), in lightmap units: colour x (1 - d^2/r^2) x cosine
// (x the cone). Same as prop.wesl.
fn dynamic_light(p: vec3<f32>, n: vec3<f32>, frag_coord: vec2<f32>) -> vec3<f32> {
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), vec4<f32>(p, 1.0));
    let cluster = clustering::view_fragment_cluster_index(frag_coord, view_z, false);
    let ranges = clustering::unpack_clusterable_object_index_ranges(cluster);
    var sum = vec3<f32>(0.0);
    for (var i = ranges.first_point_light_index_offset; i < ranges.first_spot_light_index_offset; i = i + 1u) {
        let light = &view_bindings::clustered_lights.data[clustering::get_clusterable_object_id(i)];
        let to = (*light).position_radius.xyz - p;
        let falloff = saturate(1.0 - dot(to, to) * (*light).color_inverse_square_range.w);
        sum = sum + (*light).color_inverse_square_range.rgb * falloff * max(dot(n, normalize(to)), 0.0);
    }
    // Spot lights (a flashlight), the same with the cone's attenuation.
    for (var i = ranges.first_spot_light_index_offset; i < ranges.first_rect_light_index_offset; i = i + 1u) {
        let light = &view_bindings::clustered_lights.data[clustering::get_clusterable_object_id(i)];
        let to = (*light).position_radius.xyz - p;
        let falloff = saturate(1.0 - dot(to, to) * (*light).color_inverse_square_range.w);
        sum = sum + (*light).color_inverse_square_range.rgb * falloff * spot_cone((*light).light_custom_data, (*light).flags, to) * max(dot(n, normalize(to)), 0.0);
    }
    return sum;
}

struct WorldParams {
    base_color: vec4<f32>,
    light_scale: f32,
    // 0 = flat lightmap only, 1 = radiosity normal mapping.
    bumped: f32,
    // Sign applied to the normal map's green channel.
    normal_g_sign: f32,
    alpha_cutoff: f32,
    // $basetexturetransform rows: (m0, m1, m2, scroll per second).
    base_uv_u: vec4<f32>,
    base_uv_v: vec4<f32>,
    // 0 normal, 1 lighting only, 2 albedo only.
    debug_view: f32,
    // Sign applied to the normal map's red channel.
    normal_x_sign: f32,
    // Multiplier on sampled lightmap values.
    lightmap_scale: f32,
    // Second texture blended in by the vertex alpha (WorldVertexTransition).
    blend: f32,
    blend_masked: f32,
    blend_normal: f32,
    // Detail texture: 0 none, 1 + Source's $detailblendmode, 20 and 21
    // WorldTwoTextureBlend's detail over base and 2x grime mask.
    detail: f32,
    detail_factor: f32,
    detail_scale: vec2<f32>,
    fog_color: vec4<f32>,
    fog_range: vec4<f32>,
    bicubic: f32,
    translucent: f32,
    envmap: f32,
    // 0 none, 1 normal-map alpha, 2 one minus base alpha, 3 mask texture.
    envmap_mask: f32,
    has_normal: f32,
    envmap_contrast: f32,
    envmap_saturation: f32,
    envmap_fresnel: f32,
    envmap_tint: vec4<f32>,
    water_fog_color: vec4<f32>,
    water_fog_range: vec4<f32>,
    base2_uv_u: vec4<f32>,
    base2_uv_v: vec4<f32>,
    detail_tint: vec4<f32>,
    selfillum: f32,
    selfillum_tint: vec4<f32>,
    unlit: f32,
    modulation: vec4<f32>,
    // A window's broken look: an edge piece's alpha x its jagged mask's
    // (tile: the vertex colour's blue; within it: red and green).
    pane_mask: f32,
    // How the second texture combines: 0 Source's linear mix, 1 LithTech's
    // dual texture (a = (1 - w) x first alpha), 2 its fullbright add.
    blend_mode: f32,
    // 1: the second texture's coordinates are the vertex colour's red and
    // green (map::MapMesh::uvs2).
    blend_uv2: f32,
    // The detail coordinates' rotation, radians.
    detail_rotation: f32,
    // A flat environment map: 0 none, 1 signed add, 2 multiply; weighted
    // by the base alpha; EnvScale.
    flat_envmap: f32,
    flat_envmap_alpha: f32,
    flat_envmap_scale: f32,
}

// Gamma space for the fixed-function blends of LithTech's second layers
// (the sRGB curve: the textures are sampled decoded).
fn to_gamma(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}

// A texture transform (map::MapUvTransform): rows . (u, v, 1), plus the
// TextureScroll translation, wrapped to [0, 1) as the proxy does.
fn transform_uv(uv: vec2<f32>, ru: vec4<f32>, rv: vec4<f32>) -> vec2<f32> {
    let scroll = fract(vec2<f32>(ru.w, rv.w) * globals.time);
    return vec2<f32>(dot(ru.xyz, vec3<f32>(uv, 1.0)), dot(rv.xyz, vec3<f32>(uv, 1.0))) + scroll;
}

// A lightmap page, bilinear or bicubic B-spline (4 bilinear taps; the
// sampler has already decoded sRGB, as the GPU does in CS:S).
fn sample_lightmap(t: texture_2d<f32>, uv: vec2<f32>) -> vec3<f32> {
    if params.bicubic < 0.5 {
        return textureSampleLevel(t, lightmap_sampler, uv, 0.0).rgb;
    }
    let size = vec2<f32>(textureDimensions(t));
    let p = uv * size - 0.5;
    let i = floor(p);
    let f = p - i;
    let f2 = f * f;
    let f3 = f2 * f;
    let w0 = (-f3 + 3.0 * f2 - 3.0 * f + 1.0) / 6.0;
    let w1 = (3.0 * f3 - 6.0 * f2 + 4.0) / 6.0;
    let w2 = (-3.0 * f3 + 3.0 * f2 + 3.0 * f + 1.0) / 6.0;
    let w3 = f3 / 6.0;
    let g0 = w0 + w1;
    let g1 = w2 + w3;
    let h0 = (i - 1.0 + w1 / g0 + 0.5) / size;
    let h1 = (i + 1.0 + w3 / g1 + 0.5) / size;
    let a = textureSampleLevel(t, lightmap_sampler, vec2<f32>(h0.x, h0.y), 0.0).rgb;
    let b = textureSampleLevel(t, lightmap_sampler, vec2<f32>(h1.x, h0.y), 0.0).rgb;
    let c = textureSampleLevel(t, lightmap_sampler, vec2<f32>(h0.x, h1.y), 0.0).rgb;
    let d = textureSampleLevel(t, lightmap_sampler, vec2<f32>(h1.x, h1.y), 0.0).rgb;
    return g0.y * (g0.x * a + g1.x * b) + g1.y * (g0.x * c + g1.x * d);
}

// Source range fog: toward the fog color by f^2, f from view depth.
// Opaque surfaces write alpha 1 (see `translucent`).
fn out_alpha(a: f32) -> f32 {
    // Additive (2): alpha 0 under premultiplied blending adds the colour.
    if params.translucent > 1.5 {
        return 0.0;
    }
    return select(1.0, a, params.translucent > 0.5);
}

fn apply_fog(color: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    // Under (or, at the surface, just below) water, what is below the
    // surface takes the water's fog.
    let f = source_fog(params.fog_color, params.fog_range, params.water_fog_color, params.water_fog_range, world);
    return mix(color, f.rgb, f.a);
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: WorldParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var base_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var base_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var normal_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var normal_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var lightmap: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var lightmap_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(7) var lightmap_b0: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(8) var lightmap_b1: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(9) var lightmap_b2: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(10) var base2_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(11) var normal2_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(12) var blend_mask: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(13) var detail_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(14) var envmap_texture: texture_cube<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(15) var envmap_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(16) var envmap_mask_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(17) var flat_envmap_texture: texture_2d<f32>;

const LUMA = vec3<f32>(0.299, 0.587, 0.114);

// $envmap (specs/cs_source/shaders.md): the cubemap in the reflected view
// direction, x mask x tint, then contrast, saturation and fresnel. The
// normal follows the normal map through a screen-space tangent frame
// (u and v directions), since world meshes carry no tangents.
fn envmap_term(in: VertexOutput, n_ts: vec3<f32>, base_alpha: f32, normal_alpha: f32) -> vec3<f32> {
    let p = in.world_position.xyz;
    var n = normalize(in.world_normal);
    if params.has_normal > 0.5 {
        let dp1 = dpdx(p);
        let dp2 = dpdy(p);
        let duv1 = dpdx(in.uv);
        let duv2 = dpdy(in.uv);
        let dp2perp = cross(dp2, n);
        let dp1perp = cross(n, dp1);
        let t = dp2perp * duv1.x + dp1perp * duv2.x;
        let b = dp2perp * duv1.y + dp1perp * duv2.y;
        let inv = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1e-12));
        n = normalize(n_ts.x * t * inv + n_ts.y * b * inv + n_ts.z * n);
    }
    let v = view.world_position - p;
    let r = 2.0 * dot(n, v) * n - dot(n, n) * v;
    // Source's cubemap axes: x, -z, y of ours (Z up).
    var spec = textureSample(envmap_texture, envmap_sampler, vec3<f32>(r.x, -r.z, r.y)).rgb;
    var mask = vec3<f32>(1.0);
    if params.envmap_mask > 0.5 && params.envmap_mask < 1.5 {
        mask = vec3<f32>(normal_alpha);
    } else if params.envmap_mask > 1.5 && params.envmap_mask < 2.5 {
        mask = vec3<f32>(1.0 - base_alpha);
    } else if params.envmap_mask > 2.5 {
        mask = textureSample(envmap_mask_texture, base_sampler, in.uv).rgb;
    }
    spec = spec * mask * params.envmap_tint.rgb;
    spec = mix(spec, spec * spec, params.envmap_contrast);
    spec = mix(vec3<f32>(dot(spec, LUMA)), spec, params.envmap_saturation);
    let r0 = params.envmap_fresnel;
    let ndv = saturate(dot(n, normalize(v)));
    spec = spec * (pow(1.0 - ndv, 5.0) * (1.0 - r0) + r0);
    return spec;
}

// Basis directions of the three directional lightmap pages, in order
// (specs/cs_source/shaders.md). Tangent space: x along texture u, y along
// v (image-down), z out of the surface.
const BASIS_0 = vec3<f32>(0.81649658, 0.0, 0.57735027);
const BASIS_1 = vec3<f32>(-0.40824829, 0.70710678, 0.57735027);
const BASIS_2 = vec3<f32>(-0.40824829, -0.70710678, 0.57735027);

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // WindowImposter: the cubemap seen through the surface in the view
    // direction (as if infinitely far), unlit and without fog.
    if params.envmap > 1.5 {
        let d = in.world_position.xyz - view.world_position;
        let c = textureSample(envmap_texture, envmap_sampler, vec3<f32>(d.x, -d.z, d.y)).rgb;
        return vec4<f32>(c * params.envmap_tint.rgb, out_alpha(1.0));
    }
    // $basetexturetransform (shaders.md 2): the base texture's
    // coordinates, and the detail's before $detailscale.
    let uv = transform_uv(in.uv, params.base_uv_u, params.base_uv_v);
    var albedo = textureSample(base_texture, base_sampler, uv) * params.base_color;
    // DecalModulate (shaders.md, DecalModulate): the surface times 2 x the
    // texel in gamma space, unlit; transparent texels (alpha 0) leave it,
    // partial alpha doesn't fade it. The blend is src x dst + dst x src,
    // so this returns half the factor (in our linear framebuffer the
    // factor raised to 2.2, as decal.wesl). Fog fades it to neutral by
    // the fog amount^0.4.
    if params.translucent > 2.5 {
        if albedo.a <= 0.0 {
            discard;
        }
        var factor = 2.0 * pow(max(albedo.rgb, vec3(0.0)), vec3(1.0 / 2.2));
        let f = source_fog(params.fog_color, params.fog_range, params.water_fog_color, params.water_fog_range, in.world_position.xyz);
        factor = mix(factor, vec3(1.0), pow(f.a, 0.4));
        return vec4<f32>(min(pow(factor, vec3(2.2)), vec3(2.0)) * 0.5, 1.0);
    }
    // Self-illumination masks by the base texture's own alpha (before the
    // second layer); the surface is then opaque (alpha = $alpha only).
    let base_alpha = albedo.a;
    if params.selfillum > 0.5 {
        albedo.a = params.base_color.a;
    }
    // WorldVertexTransition: blend toward the second texture by the vertex
    // alpha, optionally shaped by the mask (green: transition point, red:
    // softness). Colors blend in linear space; alpha stays the first's.
    var b = 0.0;
    @if(VERTEX_COLORS) {
        b = in.color.a;
    }
    if params.blend > 0.5 {
        if params.blend_masked > 0.5 {
            let m = textureSample(blend_mask, normal_sampler, in.uv);
            let lo = saturate(m.g - m.r);
            let hi = saturate(m.g + m.r);
            let t = select(step(lo, b), saturate((b - lo) / (hi - lo)), hi > lo);
            b = t * t * (3.0 - 2.0 * t);
        }
        var uv2 = transform_uv(in.uv, params.base2_uv_u, params.base2_uv_v);
        @if(VERTEX_COLORS) {
            if params.blend_uv2 > 0.5 {
                uv2 = in.color.xy;
            }
        }
        let second = textureSample(base2_texture, base_sampler, uv2) * params.base_color;
        if params.blend_mode < 0.5 {
            albedo = vec4<f32>(mix(albedo.rgb, second.rgb, b), albedo.a);
        } else {
            // LithTech's dual texture (map::material_fx::dual_texture): a
            // = vertex alpha (1 - w) x the first's alpha, in gamma space;
            // a fullbright second texture is added.
            let a = (1.0 - b) * albedo.a;
            let t0 = to_gamma(albedo.rgb);
            let t1 = to_gamma(second.rgb);
            var c = a * t0 + (1.0 - a) * t1;
            if params.blend_mode > 1.5 {
                c = min(t0 + a * t1, vec3<f32>(1.0));
            }
            albedo = vec4<f32>(to_linear(c), a);
        }
    }
    // $detail (shaders.md 2): sampled at $detailscale x the base
    // coordinates, times $detailtint, combined by $detailblendmode
    // (detail = 1 + mode; map::material_fx::detail_combine mirrors it).
    // Modes 5 and 6 add after lighting, which this shader never does.
    let two_texture = params.detail > 19.5 && params.detail < 29.5;
    if params.detail > 0.5 {
        var duv = uv * params.detail_scale;
        if params.detail_rotation != 0.0 {
            // LithTech's detail rotation (map::material_fx::detail_uv).
            let c = cos(params.detail_rotation);
            let s = sin(params.detail_rotation);
            duv = vec2<f32>(c * duv.x - s * duv.y, s * duv.x + c * duv.y);
        }
        var d = textureSample(detail_texture, base_sampler, duv);
        let f = params.detail_factor;
        if params.detail > 29.5 {
            // LithTech's detail texture: a signed add in gamma space, alpha
            // the base's (map::material_fx::signed_add).
            let g = saturate(to_gamma(albedo.rgb) + to_gamma(d.rgb) - 0.5);
            albedo = vec4<f32>(to_linear(g), albedo.a);
        } else if two_texture {
            if params.detail < 20.5 {
                // WorldTwoTextureBlend, detail over base: the raw detail texel.
                albedo = vec4<f32>(mix(albedo.rgb, d.rgb, d.a), albedo.a);
            } else {
                // WorldTwoTextureBlend's 2x grime mask: the detail is the
                // surface, darkened by twice the decoded base where the
                // detail's alpha says so. Alpha stays the base's.
                let k = saturate(2.0 * albedo.rgb);
                let m = saturate(k * d.a + (1.0 - d.a));
                albedo = vec4<f32>(m * d.rgb, albedo.a);
            }
        } else {
            d = vec4<f32>(d.rgb * params.detail_tint.rgb, d.a);
            let mode = params.detail - 1.0;
            if mode < 0.5 {
                albedo = vec4<f32>(albedo.rgb * mix(vec3<f32>(1.0), 2.0 * d.rgb, f), albedo.a);
            } else if mode < 1.5 {
                albedo = vec4<f32>(albedo.rgb + f * d.rgb, albedo.a);
            } else if mode < 2.5 {
                albedo = vec4<f32>(mix(albedo.rgb, d.rgb, f * d.a), albedo.a);
            } else if mode < 3.5 {
                albedo = mix(albedo, d, f);
            } else if mode < 4.5 {
                albedo = vec4<f32>(mix(albedo.rgb, d.rgb, f * (1.0 - albedo.a)), d.a);
            } else if mode < 6.5 {
                // 5, 6: post-lighting, not in the world shader.
            } else if mode < 7.5 {
                let c = mix(d.r, d.a, albedo.a);
                albedo = vec4<f32>(albedo.rgb * mix(1.0, 2.0 * c, f), albedo.a);
            } else if mode < 8.5 {
                albedo = mix(albedo, albedo * d, f);
            } else {
                albedo.a = mix(albedo.a, albedo.a * d.a, f);
            }
        }
    }
    // A flat environment map (LithTech's envmap link): sampled by the
    // view-space reflection vector, (r.x, r.y) x -0.5 / EnvScale + 0.5
    // (map::material_fx::flat_envmap_uv), combined in gamma space before
    // lighting; envmapalpha weights it by the base alpha.
    if params.flat_envmap > 0.5 {
        let p_view = (view.view_from_world * vec4<f32>(in.world_position.xyz, 1.0)).xyz;
        let n_view = (view.view_from_world * vec4<f32>(normalize(in.world_normal), 0.0)).xyz;
        let r = reflect(normalize(p_view), normalize(n_view));
        let euv = r.xy * (-0.5 / params.flat_envmap_scale) + 0.5;
        let e = to_gamma(textureSample(flat_envmap_texture, base_sampler, euv).rgb);
        let g = to_gamma(albedo.rgb);
        var c = saturate(g + e - 0.5);
        if params.flat_envmap > 1.5 {
            c = g * e;
        }
        if params.flat_envmap_alpha > 0.5 {
            c = mix(g, c, albedo.a);
        }
        albedo = vec4<f32>(to_linear(c), albedo.a);
    }
    // WorldTwoTextureBlend lights with one bilinear lightmap tap times a
    // fixed 2.0, not 2^2.2; its bump uses the detail coordinates and
    // unsquared weights.
    @if(VERTEX_COLORS) {
        if params.pane_mask > 0.5 {
            // Square tiles side by side, the blue picking one; clamped half a
            // texel inside it, so the solid edge doesn't wrap or bleed.
            let size = vec2<f32>(textureDimensions(blend_mask));
            let tiles = max(1.0, round(size.x / size.y));
            let m = clamp(in.color.xy, vec2<f32>(0.004), vec2<f32>(0.996));
            let at = vec2<f32>((in.color.z + m.x) / tiles, m.y);
            albedo.a = albedo.a * textureSample(blend_mask, normal_sampler, at).a;
        }
    }
    if params.alpha_cutoff > 0.0 && albedo.a < params.alpha_cutoff {
        discard;
    }
    var light = sample_lightmap(lightmap, in.uv_b);
    if two_texture {
        light = textureSampleLevel(lightmap, lightmap_sampler, in.uv_b, 0.0).rgb;
    }
    // The normal map (tangent space, signs fixed), for bump lighting and
    // reflections; its alpha can mask reflections.
    // The bump coordinates: the base's with a $detail (shaders.md 3),
    // else the vertex's ($bumptransform isn't read); WorldTwoTextureBlend
    // bumps at the detail's.
    var bump_uv = select(in.uv, uv, params.detail > 0.5);
    if two_texture {
        bump_uv = uv * params.detail_scale;
    }
    let normal_texel = textureSample(normal_texture, normal_sampler, bump_uv);
    var n_ts = normal_texel.xyz * 2.0 - 1.0;
    n_ts.x = n_ts.x * params.normal_x_sign;
    n_ts.y = n_ts.y * params.normal_g_sign;
    if params.bumped > 0.5 {
        var n = normal_texel.xyz * 2.0 - 1.0;
        if params.blend > 0.5 && params.blend_normal > 0.5 {
            let n2 = textureSample(normal2_texture, normal_sampler, bump_uv).xyz * 2.0 - 1.0;
            n = mix(n, n2, b);
        }
        n.x = n.x * params.normal_x_sign;
        n.y = n.y * params.normal_g_sign;
        var w = vec3<f32>(
            saturate(dot(n, BASIS_0)),
            saturate(dot(n, BASIS_1)),
            saturate(dot(n, BASIS_2)),
        );
        if !two_texture {
            w = w * w;
        }
        w = w / max(w.x + w.y + w.z, 1e-4);
        light = w.x * sample_lightmap(lightmap_b0, in.uv_b)
            + w.y * sample_lightmap(lightmap_b1, in.uv_b)
            + w.z * sample_lightmap(lightmap_b2, in.uv_b);
    }
    light = light * params.lightmap_scale;
    if two_texture {
        // The usual scale is 2^2.2 for Source's LDR lightmaps; this shader
        // uses 2.0.
        light = light * (2.0 / 4.594793);
    }
    if params.debug_view == 1.0 {
        albedo = vec4<f32>(1.0, 1.0, 1.0, albedo.a);
        light = light * 0.25;
        return vec4<f32>(albedo.rgb * light, out_alpha(albedo.a));
    }
    if params.debug_view == 2.0 {
        return albedo;
    }
    light = light + dynamic_light(in.world_position.xyz, normalize(in.world_normal), in.position.xy);
    // UnlitGeneric on a brush: the texture's own brightness x $color;
    // lit, the lighting x $color (shaders.md 2, modulation); between
    // (ShatteredGlass's $unlitfactor), the lighting that far toward 1.
    light = mix(light, vec3<f32>(1.0), params.unlit) * params.modulation.rgb;
    var color = albedo.rgb * light;
    // $selfillum: toward tint x albedo, unlit, by the base texture's alpha.
    if params.selfillum > 0.5 {
        color = mix(color, params.selfillum_tint.rgb * albedo.rgb, base_alpha);
    }
    color = color * params.light_scale;
    if params.envmap > 0.5 {
        color = color + envmap_term(in, n_ts, albedo.a, normal_texel.a);
    }
    return vec4<f32>(apply_fog(color, in.world_position.xyz), out_alpha(albedo.a));
}
