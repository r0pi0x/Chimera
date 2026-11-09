// Props lit by a light probe: texture x the light baked into vertex
// colors (or the probe evaluated per pixel, for moving models), plus the
// scene's point lights, then Source's range fog (f^2), as for world
// surfaces. Face parts (map::face): eyeballs lit along their own normal
// with the iris projected on them, teeth darkened toward the back.
// VertexLitGeneric $phong materials: Source's Phong shading per pixel
// (phong_shade, specs/cs_source/shaders_phong.md).

import bevy_pbr::{
    render::forward_io::VertexOutput,
    render::mesh_view_bindings::{view, globals},
    render::mesh_view_bindings as view_bindings,
    render::clustered_forward as clustering,
    render::mesh_view_types::POINT_LIGHT_FLAGS_SPOT_LIGHT_Y_NEGATIVE,
    render::view_transformations::position_world_to_view,
};
import chimera_present::render::fog::source_fog;
import chimera_present::render::dither::dithered_out;

struct PropParams {
    base_color: vec4<f32>,
    alpha_cutoff: f32,
    base_uv_u: vec4<f32>,
    base_uv_v: vec4<f32>,
    fog_color: vec4<f32>,
    fog_range: vec4<f32>,
    translucent: f32,
    envmap: f32,
    envmap_mask: f32,
    envmap_contrast: f32,
    envmap_saturation: f32,
    envmap_tint: vec4<f32>,
    dynamic: f32,
    probe: f32,
    probe_cube: array<vec4<f32>, 6>,
    probe_light_dir: array<vec4<f32>, 4>,
    probe_light_color: array<vec4<f32>, 4>,
    water_fog_color: vec4<f32>,
    water_fog_range: vec4<f32>,
    detail: f32,
    detail_factor: f32,
    detail_scale: vec2<f32>,
    detail_tint: vec4<f32>,
    selfillum: f32,
    selfillum_tint: vec4<f32>,
    layer: f32,
    layer_coords: f32,
    layer_address: f32,
    layer_factor: f32,
    layer_light: f32,
    layer_lit: f32,
    layer_emissive: vec4<f32>,
    layer_ambient: vec4<f32>,
    layer_diffuse: vec4<f32>,
    layer_specular: vec4<f32>,
    face: f32,
    face_center: vec4<f32>,
    face_up: vec4<f32>,
    face_u: vec4<f32>,
    face_v: vec4<f32>,
    phong: f32,
    phong_fresnel: vec4<f32>,
    phong_exponent: vec4<f32>,
    phong_tint: vec4<f32>,
    phong_flags: vec4<f32>,
    phong_env: vec4<f32>,
    phong_warp: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: PropParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var base_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var base_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var envmap_texture: texture_cube<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var envmap_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var envmap_mask_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var detail_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(7) var selfillum_mask_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(8) var layer_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(9) var normal_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(10) var phong_exponent_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(11) var lightwarp_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(12) var phong_warp_texture: texture_2d<f32>;

// A texture transform (map::MapUvTransform): rows . (u, v, 1), plus the
// TextureScroll translation, wrapped to [0, 1) as the proxy does.
fn transform_uv(uv: vec2<f32>, ru: vec4<f32>, rv: vec4<f32>) -> vec2<f32> {
    let scroll = fract(vec2<f32>(ru.w, rv.w) * globals.time);
    return vec2<f32>(dot(ru.xyz, vec3<f32>(uv, 1.0)), dot(rv.xyz, vec3<f32>(uv, 1.0))) + scroll;
}

// The detail combine before lighting (shaders.md 2 and 4; mode = the
// Source $detailblendmode; map::material_fx::detail_combine mirrors it).
fn detail_combine(mode: f32, a: vec4<f32>, d: vec4<f32>, f: f32) -> vec4<f32> {
    if mode < 0.5 {
        return vec4<f32>(a.rgb * mix(vec3<f32>(1.0), 2.0 * d.rgb, f), a.a);
    } else if mode < 1.5 {
        return vec4<f32>(a.rgb + f * d.rgb, a.a);
    } else if mode < 2.5 {
        return vec4<f32>(mix(a.rgb, d.rgb, f * d.a), a.a);
    } else if mode < 3.5 {
        return mix(a, d, f);
    } else if mode < 4.5 {
        return vec4<f32>(mix(a.rgb, d.rgb, f * (1.0 - a.a)), d.a);
    } else if mode < 6.5 {
        return a;
    } else if mode < 7.5 {
        let c = mix(d.r, d.a, a.a);
        return vec4<f32>(a.rgb * mix(1.0, 2.0 * c, f), a.a);
    } else if mode < 8.5 {
        return mix(a, a * d, f);
    }
    return vec4<f32>(a.rgb, mix(a.a, a.a * d.a, f));
}

const LUMA = vec3<f32>(0.299, 0.587, 0.114);

// A layer's coordinates (map::MapLayer, LithTech's generated texture
// coordinates): x and y of the camera-space reflection (0) or normal
// (1), the camera-space reflection as a sphere map (2: 0.5 x + 0.5), or
// the world-space reflection (3) or the world-space normal (4); then
// wrapped, clamped (1) or mirrored (2).
fn layer_uv(n: vec3<f32>, p: vec3<f32>) -> vec2<f32> {
    let c = params.layer_coords;
    var uv: vec2<f32>;
    if c > 3.5 {
        uv = n.xy;
    } else if c > 2.5 {
        uv = reflect(normalize(p - view.world_position), n).xy;
    } else {
        let nv = normalize((view.view_from_world * vec4<f32>(n, 0.0)).xyz);
        if c > 0.5 && c < 1.5 {
            uv = nv.xy;
        } else {
            let pv = (view.view_from_world * vec4<f32>(p, 1.0)).xyz;
            uv = reflect(normalize(pv), nv).xy;
            if c > 1.5 {
                uv = 0.5 * uv + 0.5;
            }
        }
    }
    if params.layer_address > 1.5 {
        let m = fract(uv * 0.5) * 2.0;
        uv = 1.0 - abs(m - 1.0);
    } else if params.layer_address > 0.5 {
        uv = saturate(uv);
    }
    return uv;
}

// The probe on a surface with normal n (shaders.md 4): the ambient cube
// weighted by the squared normal, plus each light by its cosine.
fn probe_ambient(n: vec3<f32>) -> vec3<f32> {
    let n2 = n * n;
    return n2.x * select(params.probe_cube[1].rgb, params.probe_cube[0].rgb, n.x >= 0.0)
        + n2.y * select(params.probe_cube[3].rgb, params.probe_cube[2].rgb, n.y >= 0.0)
        + n2.z * select(params.probe_cube[5].rgb, params.probe_cube[4].rgb, n.z >= 0.0);
}

fn probe_direct(n: vec3<f32>) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        light = light + params.probe_light_color[i].rgb * max(dot(n, params.probe_light_dir[i].xyz), 0.0);
    }
    return light;
}

// The probe's lights with half-Lambert ((n.l / 2 + 1/2)^2: the Eyes
// shader, specs/hl2dm/player_models.md 11.4).
fn probe_direct_half(n: vec3<f32>) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        let h = dot(n, params.probe_light_dir[i].xyz) * 0.5 + 0.5;
        light = light + params.probe_light_color[i].rgb * h * h;
    }
    return light;
}

fn probe_light(n: vec3<f32>) -> vec3<f32> {
    return probe_ambient(n) + probe_direct(n);
}

// The layer's specular colour (a render style's material: M_s x sum over
// the probe's lights of C x max(0, N.H)^p), added after the texture stages.
fn layer_specular(n: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    let power = params.layer_specular.w;
    if power <= 0.0 || all(params.layer_specular.rgb == vec3<f32>(0.0)) {
        return vec3<f32>(0.0);
    }
    let to_eye = normalize(view.world_position - p);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        let h = normalize(params.probe_light_dir[i].xyz + to_eye);
        sum = sum + params.probe_light_color[i].rgb * pow(max(dot(n, h), 0.0), power);
    }
    return params.layer_specular.rgb * sum;
}

// A spot light's cone at `to`, the line from the point to the light (as
// Bevy's own: its direction packed in x, z and a flag for y's sign). Same as
// world.wesl.
fn spot_cone(custom: vec4<f32>, flags: u32, to: vec3<f32>) -> f32 {
    var spot_dir = vec3<f32>(custom.x, 0.0, custom.y);
    spot_dir.y = sqrt(max(0.0, 1.0 - spot_dir.x * spot_dir.x - spot_dir.z * spot_dir.z));
    if (flags & POINT_LIGHT_FLAGS_SPOT_LIGHT_Y_NEGATIVE) != 0u {
        spot_dir.y = -spot_dir.y;
    }
    let cone = saturate(dot(-spot_dir, normalize(to)) * custom.z + custom.w);
    return cone * cone;
}

// The scene's point lights (muzzle flashes; map::DynamicLight), in
// lightmap units: colour x falloff x cosine. The falloff is the model's
// game's (map::LightFalloff::share): Source's 1 - d^2/r^2 (as world.wesl),
// or with `lithtech` 1 / (1 + 19 d^2/r^2), none at or beyond r.
fn dynamic_light(p: vec3<f32>, n: vec3<f32>, frag_coord: vec2<f32>, lithtech: bool) -> vec3<f32> {
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
        let x = dot(to, to) * (*light).color_inverse_square_range.w;
        var falloff = saturate(1.0 - x);
        if lithtech {
            falloff = select(1.0 / (1.0 + 19.0 * x), 0.0, x >= 1.0);
        }
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

// Detail modes 5 and 6: added after lighting.
fn detail_after_lighting(rgb: vec3<f32>, d: vec4<f32>) -> vec3<f32> {
    let mode = params.detail - 1.0;
    if params.detail > 0.5 && mode > 4.5 && mode < 6.5 {
        let f = params.detail_factor;
        if mode < 5.5 {
            return rgb + f * d.rgb;
        }
        let m = select(4.0 * f, 1.0 / f, f >= 0.5);
        let a = select(-0.5 * m, 1.0 - m, f >= 0.5);
        return rgb + saturate(m * d.rgb + a);
    }
    return rgb;
}

// Source's Phong model shading (specs/cs_source/shaders_phong.md;
// map::phong mirrors it on the CPU for the spec's test cases).

// The ramp F of (1 - N.V)^2 through (min, mid, max), not clamped (7).
fn phong_fresnel_ramp(n_dot_v: f32) -> f32 {
    let r = params.phong_fresnel;
    let x = saturate(1.0 - n_dot_v);
    let f = x * x;
    if f < 0.5 {
        return r.x + (r.y - r.x) * 2.0 * f;
    }
    return r.y + (r.z - r.y) * (2.0 * f - 1.0);
}

// clamp(R.L)^k x clamp(N.L), 0 behind the light (0^0 counts as 0).
fn phong_lobe(r_dot_l: f32, n_dot_l: f32, k: f32) -> f32 {
    if r_dot_l <= 0.0 || n_dot_l <= 0.0 {
        return 0.0;
    }
    return pow(min(r_dot_l, 1.0), k) * min(n_dot_l, 1.0);
}

// A 1D ramp's texel at u on its first row (shaders_phong.md open
// question 6), or the phong warp's at (u, v), between texel centres.
fn ramp_uv(t: texture_2d<f32>, uv: vec2<f32>) -> vec2<f32> {
    let size = vec2<f32>(textureDimensions(t));
    let half = 0.5 / size;
    return clamp(uv, half, vec2<f32>(1.0) - half);
}

struct PhongSum {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
    rim: vec3<f32>,
}

// One light (toward it l, colour x attenuation c): half-Lambert or warped
// diffuse (4), the specular and rim lobes (7).
fn phong_light(sum: PhongSum, n: vec3<f32>, r: vec3<f32>, l: vec3<f32>, c: vec3<f32>, k: f32, fresnel: f32) -> PhongSum {
    var out = sum;
    let n_dot_l = dot(n, l);
    let h = saturate(0.5 * n_dot_l + 0.5);
    var w = vec3<f32>(h * h);
    if params.phong_warp.x > 0.5 {
        w = 2.0 * textureSampleLevel(lightwarp_texture, base_sampler, ramp_uv(lightwarp_texture, vec2<f32>(h, 0.0)), 0.0).rgb;
    }
    out.diffuse = out.diffuse + c * w;
    let r_dot_l = dot(r, l);
    let lobe = phong_lobe(r_dot_l, n_dot_l, k);
    var s = c * lobe;
    if params.phong_warp.y > 0.5 {
        s = s * textureSampleLevel(phong_warp_texture, base_sampler, ramp_uv(phong_warp_texture, vec2<f32>(lobe, fresnel)), 0.0).rgb;
    }
    out.specular = out.specular + s;
    if params.phong_flags.x > 0.5 {
        out.rim = out.rim + c * phong_lobe(r_dot_l, n_dot_l, params.phong_exponent.z);
    }
    return out;
}

// The scene's point lights as local lights (dynamic_light's falloff as
// the attenuation).
fn phong_dynamic(sum: PhongSum, p: vec3<f32>, n: vec3<f32>, r: vec3<f32>, k: f32, fresnel: f32, frag_coord: vec2<f32>, lithtech: bool) -> PhongSum {
    var out = sum;
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), vec4<f32>(p, 1.0));
    let cluster = clustering::view_fragment_cluster_index(frag_coord, view_z, false);
    let ranges = clustering::unpack_clusterable_object_index_ranges(cluster);
    for (var i = ranges.first_point_light_index_offset; i < ranges.first_spot_light_index_offset; i = i + 1u) {
        let light = &view_bindings::clustered_lights.data[clustering::get_clusterable_object_id(i)];
        let to = (*light).position_radius.xyz - p;
        let x = dot(to, to) * (*light).color_inverse_square_range.w;
        var falloff = saturate(1.0 - x);
        if lithtech {
            falloff = select(1.0 / (1.0 + 19.0 * x), 0.0, x >= 1.0);
        }
        out = phong_light(out, n, r, normalize(to), (*light).color_inverse_square_range.rgb * falloff, k, fresnel);
    }
    for (var i = ranges.first_spot_light_index_offset; i < ranges.first_rect_light_index_offset; i = i + 1u) {
        let light = &view_bindings::clustered_lights.data[clustering::get_clusterable_object_id(i)];
        let to = (*light).position_radius.xyz - p;
        let falloff = saturate(1.0 - dot(to, to) * (*light).color_inverse_square_range.w);
        out = phong_light(out, n, r, normalize(to), (*light).color_inverse_square_range.rgb * falloff * spot_cone((*light).light_custom_data, (*light).flags, to), k, fresnel);
    }
    return out;
}

// The shading normal from the normal map through a screen-space tangent
// frame (dP/du, dP/dv: x along u, y along v image-down, as Source's).
fn phong_normal(p: vec3<f32>, n_v: vec3<f32>, uv: vec2<f32>, n_t: vec3<f32>) -> vec3<f32> {
    let dp1 = dpdx(p);
    let dp2 = dpdy(p);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    let dp2perp = cross(dp2, n_v);
    let dp1perp = cross(n_v, dp1);
    let t = dp2perp * duv1.x + dp1perp * duv2.x;
    let b = dp2perp * duv1.y + dp1perp * duv2.y;
    let inv = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1e-12));
    let n = n_t.x * t * inv + n_t.y * b * inv + n_t.z * n_v;
    return select(n_v, normalize(n), dot(n, n) > 1e-12);
}

// The whole Phong pixel (shaders_phong.md, Per-pixel order): `base` the
// base texture after detail, `a` the albedo (x modulation), `n_v` the
// vertex normal, `vertex_light` the baked light of a prop without a probe
// (its diffuse only). Before fog.
fn phong_shade(in: VertexOutput, uv: vec2<f32>, base: vec4<f32>, a: vec3<f32>, n_v: vec3<f32>, vertex_light: vec3<f32>, d: vec4<f32>) -> vec3<f32> {
    let p = in.world_position.xyz;
    let cheap = params.phong > 1.5;
    var texel = vec4<f32>(0.5, 0.5, 1.0, 1.0);
    if params.phong_flags.w > 0.5 {
        texel = textureSample(normal_texture, base_sampler, uv);
    }
    var n_t = texel.xyz * 2.0 - 1.0;
    var mask = texel.a;
    if params.phong_flags.z > 0.5 || cheap {
        n_t = vec3<f32>(0.0, 0.0, 1.0);
        mask = base.a;
    }
    let n = phong_normal(p, n_v, uv, n_t);
    let v = normalize(view.world_position - p);
    let n_dot_v = dot(n, v);
    let fresnel = phong_fresnel_ramp(n_dot_v);
    let x = saturate(1.0 - n_dot_v);
    let rim_fresnel = x * x * x * x;
    let r = 2.0 * n * n_dot_v - v;
    let ex = textureSample(phong_exponent_texture, base_sampler, uv);
    let k = params.phong_exponent.x + params.phong_exponent.y * ex.r;
    var sum = PhongSum(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
    if params.probe > 0.5 {
        sum.diffuse = probe_ambient(n);
        for (var i = 0u; i < 4u; i = i + 1u) {
            sum = phong_light(sum, n, r, params.probe_light_dir[i].xyz, params.probe_light_color[i].rgb, k, fresnel);
        }
    } else {
        sum.diffuse = vertex_light;
    }
    if params.dynamic > 0.5 {
        sum = phong_dynamic(sum, p, n, r, k, fresnel, in.position.xy, params.dynamic > 1.5);
    }
    // The envmap (8): its own mask rules, the fresnel ramp by
    // $envmapfresnel, the tint raw.
    var env = vec3<f32>(0.0);
    if params.envmap > 0.5 {
        var m_e = base.a;
        if params.phong_env.x > 1.5 {
            m_e = params.phong_env.w;
        } else if params.phong_env.x > 0.5 {
            m_e = mask;
        }
        if params.phong_env.y > 0.5 {
            m_e = 1.0 - m_e;
        }
        let cube = textureSample(envmap_texture, envmap_sampler, vec3<f32>(r.x, -r.z, r.y)).rgb;
        env = cube * params.envmap_tint.rgb * mix(1.0, fresnel, params.phong_env.z) * m_e;
    }
    // The mask takes the fresnel unless a phong warp already did; boost.
    let masked = select(mask * fresnel, mask, params.phong_warp.y > 0.5);
    var s = sum.specular * masked * params.phong_fresnel.w;
    var diffuse = a * sum.diffuse;
    if params.selfillum > 0.5 {
        var m_s = vec3<f32>(base.a);
        if params.selfillum > 1.5 {
            m_s = textureSample(selfillum_mask_texture, base_sampler, uv).rgb;
        }
        diffuse = max(vec3<f32>(0.0), mix(diffuse, params.selfillum_tint.rgb * a, m_s));
    }
    diffuse = detail_after_lighting(diffuse, d);
    // The rim (7, 3): its lobe by (1 - N.V)^4 (and the mask), merged by
    // max, plus the ambient cube along the view on upward-facing pixels.
    if params.phong_flags.x > 0.5 && !cheap {
        let rho = select(1.0, ex.a, params.phong_flags.y > 0.5) * rim_fresnel;
        s = max(s, sum.rim * rho);
        if params.probe > 0.5 {
            s = s + probe_ambient(v) * params.phong_exponent.w * saturate(rho * n.y);
        }
    }
    let tint = select(params.phong_tint.rgb, mix(vec3<f32>(1.0), base.rgb, ex.g), params.phong_tint.w > 0.5);
    return s * tint + env + diffuse;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    @if(VISIBILITY_RANGE_DITHER) {
        // Inside the prop's fade band (map::vis::fade_band).
        if dithered_out(in.position, in.visibility_range_dither) {
            discard;
        }
    }
    // $basetexturetransform: the base's coordinates, and the detail's
    // before $detailscale.
    let uv = transform_uv(in.uv, params.base_uv_u, params.base_uv_v);
    var color = textureSample(base_texture, base_sampler, uv);
    let base_alpha = color.a;
    // $detail (shaders.md 4): times $detailtint, combined before the
    // material colour; modes 5 and 6 add after lighting (below).
    var d = vec4<f32>(0.0);
    let mode = params.detail - 1.0;
    if params.detail > 0.5 {
        d = textureSample(detail_texture, base_sampler, uv * params.detail_scale);
        d = vec4<f32>(d.rgb * params.detail_tint.rgb, d.a);
        color = detail_combine(mode, color, d, params.detail_factor);
    }
    let base = color;
    color = color * params.base_color;
    // Self-illuminated: the base alpha is the mask, not opacity; so is it
    // under $basemapalphaphongmask.
    if params.selfillum > 0.5 || (params.phong > 0.5 && params.phong_flags.z > 0.5) {
        color.a = params.base_color.a;
    }
    if params.alpha_cutoff > 0.0 && color.a < params.alpha_cutoff {
        discard;
    }
    var n = normalize(in.world_normal);
    // An eyeball: lit along the direction from its centre, flattened
    // along its up (11.4); the iris projected over the sclera.
    if params.face > 0.5 && params.face < 1.5 {
        let d = in.world_position.xyz - params.face_center.xyz;
        let u = params.face_up.xyz;
        n = normalize(d - 0.5 * dot(d, u) * u);
        let p = vec4<f32>(in.world_position.xyz, 1.0);
        let iris_uv = vec2<f32>(dot(params.face_u, p), dot(params.face_v, p));
        if all(iris_uv >= vec2<f32>(0.0)) && all(iris_uv <= vec2<f32>(1.0)) {
            let iris = textureSample(layer_texture, base_sampler, iris_uv);
            color = vec4<f32>(mix(color.rgb, iris.rgb * params.base_color.rgb, iris.a), color.a);
        }
    }
    var light = vec3<f32>(1.0);
    @if(VERTEX_COLORS) {
        light = in.color.rgb;
    }
    if params.phong > 0.5 {
        return prop_output(phong_shade(in, uv, base, color.rgb, n, light, d), color.a, in.world_position.xyz);
    }
    var direct_extra = vec3<f32>(0.0);
    if params.probe > 0.5 {
        light = probe_light(n);
        if params.face > 0.5 && params.face < 1.5 {
            light = probe_ambient(n) + probe_direct_half(n);
        }
    }
    // Teeth: darker as the mouth closes and away from its forward (11.5).
    if params.face > 1.5 {
        light = light * params.face_up.w * saturate(dot(n, params.face_up.xyz));
    }
    if params.dynamic > 0.5 {
        direct_extra = dynamic_light(in.world_position.xyz, n, in.position.xy, params.dynamic > 1.5);
        light = light + direct_extra;
    }
    var rgb = color.rgb * light;
    // A second texture on generated coordinates (map::LayerCombine), with
    // the source game's lit colour l: 0 T + T.a S l, 1 T l + f S,
    // 2 f l lerp(T, S, T.a), 3 (T l + T.a S)(1 + T.a^2).
    if params.layer > 0.5 {
        let s = textureSample(layer_texture, base_sampler, layer_uv(n, in.world_position.xyz)).rgb;
        var l = light * params.layer_light;
        var spec = vec3<f32>(0.0);
        // A lighting material (the render style's): L = clamp(M_e + M_a A +
        // M_d sum), and a specular colour added after the stages.
        if params.layer_lit > 0.5 && params.probe > 0.5 {
            l = clamp(
                params.layer_emissive.rgb + params.layer_ambient.rgb * probe_ambient(n)
                    + params.layer_diffuse.rgb * (probe_direct(n) + direct_extra),
                vec3<f32>(0.0),
                vec3<f32>(1.0),
            );
            spec = layer_specular(n, in.world_position.xyz);
        }
        let a = base_alpha;
        let combine = params.layer - 1.0;
        if combine < 0.5 {
            rgb = color.rgb + a * s * l;
        } else if combine < 1.5 {
            rgb = color.rgb * l + params.layer_factor * s;
        } else if combine < 2.5 {
            rgb = params.layer_factor * l * mix(color.rgb, s, a);
        } else if combine < 3.5 {
            rgb = (color.rgb * l + a * s) * (1.0 + a * a);
        } else {
            // pv_char_b (models_ltb.md Follow-up (2026-10-31) 4): pass 1,
            // the lit pass's dot3 grey of c = lerp(F, T, F.a) blended
            // toward L by T.a^2; pass 2 added, (F T)^2 + F.a T.a; pass 3
            // multiplied, the base's 2T made brighter by its alpha and
            // opened by S where the alpha is low.
            let f = 0.502;
            let t = color.rgb;
            let c = clamp(t * f + vec3<f32>(f * (1.0 - f)), vec3<f32>(0.0), vec3<f32>(1.0));
            let a2 = clamp(a * a, 0.0, 1.0);
            let m = clamp(c * a2 + l * (1.0 - a2), vec3<f32>(0.0), vec3<f32>(1.0));
            let d = clamp(4.0 * dot(m - vec3<f32>(0.5), m - vec3<f32>(0.5)), 0.0, 1.0);
            let p2 = clamp((t * f) * (t * f) + vec3<f32>(f * a), vec3<f32>(0.0), vec3<f32>(1.0));
            let s0 = clamp(2.0 * t, vec3<f32>(0.0), vec3<f32>(1.0));
            let s1 = clamp(s0 + (1.0 - a) * s0, vec3<f32>(0.0), vec3<f32>(1.0));
            let a1 = clamp(4.0 * a - 1.0, 0.0, 1.0);
            let s2 = clamp(s1 + (1.0 - a1) * s, vec3<f32>(0.0), vec3<f32>(1.0));
            rgb = clamp(vec3<f32>(d) + spec + p2, vec3<f32>(0.0), vec3<f32>(1.0)) * s2;
            spec = vec3<f32>(0.0);
        }
        rgb = rgb + spec;
    }
    rgb = detail_after_lighting(rgb, d);
    // $selfillum: toward tint x albedo, unlit, by the base alpha or the
    // $selfillummask texture.
    if params.selfillum > 0.5 {
        var mask = vec3<f32>(base_alpha);
        if params.selfillum > 1.5 {
            mask = textureSample(selfillum_mask_texture, base_sampler, uv).rgb;
        }
        rgb = mix(rgb, params.selfillum_tint.rgb * color.rgb, mask);
    }
    // $envmap (specs/cs_source/shaders.md 4): the cubemap in the reflected
    // view direction x mask x tint, contrast and saturation; no fresnel.
    if params.envmap > 0.5 {
        let v = view.world_position - in.world_position.xyz;
        let r = 2.0 * dot(n, v) * n - dot(n, n) * v;
        var spec = textureSample(envmap_texture, envmap_sampler, vec3<f32>(r.x, -r.z, r.y)).rgb;
        if params.envmap_mask > 0.5 && params.envmap_mask < 1.5 {
            // $normalmapalphaenvmapmask: the normal map's alpha (raw).
            spec = spec * textureSample(envmap_mask_texture, base_sampler, uv).a;
        } else if params.envmap_mask > 1.5 && params.envmap_mask < 2.5 {
            spec = spec * (1.0 - color.a);
        } else if params.envmap_mask > 2.5 {
            spec = spec * textureSample(envmap_mask_texture, base_sampler, uv).rgb;
        }
        spec = spec * params.envmap_tint.rgb;
        spec = mix(spec, spec * spec, params.envmap_contrast);
        spec = mix(vec3<f32>(dot(spec, LUMA)), spec, params.envmap_saturation);
        rgb = rgb + spec;
    }
    return prop_output(rgb, color.a, in.world_position.xyz);
}

// Fog and the output alpha.
fn prop_output(lit: vec3<f32>, alpha: f32, p: vec3<f32>) -> vec4<f32> {
    // Under (or, at the surface, just below) water, what is below the
    // surface takes the water's fog.
    let fog = source_fog(params.fog_color, params.fog_range, params.water_fog_color, params.water_fog_range, p);
    let rgb = mix(lit, fog.rgb, fog.a);
    // Additive (translucent 2): alpha 0 under premultiplied blending adds.
    if params.translucent > 1.5 {
        return vec4<f32>(rgb, 0.0);
    }
    return vec4<f32>(rgb, select(1.0, alpha, params.translucent > 0.5));
}
