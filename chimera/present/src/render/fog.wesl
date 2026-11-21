// Source's fog at a world point, shared by the map's shaders
// (specs/cs_source/shaders.md section 7, water.md sections 8 and 9).
//
// `color`/`range`: the world's range fog (linear colour, w = 1 when on;
// start, end in meters, max density). `water_color`/`water_range`: the
// water's fog for points below `water_range.w` (the surface plus the
// 2-unit fudge): `water_color.w` 1 = range fog (the eye is under water),
// 2 = the water's height fog (the eye is just above the surface and the
// near plane crosses it: Source's intersection view).
//
// Returns the fog colour (rgb) and how much of it to apply (a): range fog
// is f^2 (f with `range.w` 1, a map's linear fog), height fog is f (not
// squared).

import bevy_pbr::{
    render::mesh_view_bindings::view,
    render::view_transformations::position_world_to_view,
};
@if(DISTANCE_FOG)
import bevy_pbr::render::mesh_view_bindings::fog;

// A view with Source fog of its own (a monitor's camera: map::monitor,
// `SOURCE_VIEW_FOG`) carries it as Bevy's linear distance fog, marked by
// the directional light exponent: colour (alpha 1 when on), start and end
// meters, max density in the directional light colour's red.
const SOURCE_VIEW_FOG: f32 = -7.0;

fn view_fog() -> bool {
    @if(DISTANCE_FOG) {
        return fog.mode == 1u && fog.directional_light_exponent == SOURCE_VIEW_FOG;
    } @else {
        return false;
    }
}

fn range_amount(range: vec4<f32>, depth: f32) -> f32 {
    let f = clamp(min(range.z, (depth - range.x) / (range.y - range.x)), 0.0, 1.0);
    return f * f;
}

// Height fog (water.md section 8): the share of the eye's path to the
// point that is under the plane, times the point's depth, times one over
// the fog range.
fn height_amount(range: vec4<f32>, world: vec3<f32>, depth: f32) -> f32 {
    let plane = range.w;
    let path = view.world_position.y - world.y;
    let h = select(1.0, clamp((plane - world.y) / path, 0.0, 1.0), path > 0.0);
    let k = 1.0 / max(range.y - range.x, 1e-4);
    return clamp(h * depth * k, 0.0, 1.0);
}

fn source_fog(
    color: vec4<f32>,
    range: vec4<f32>,
    water_color: vec4<f32>,
    water_range: vec4<f32>,
    world: vec3<f32>,
) -> vec4<f32> {
    let depth = -position_world_to_view(world).z;
    // The view's own fog replaces the scene's, water fog included (that
    // follows the player's eye).
    @if(DISTANCE_FOG) {
        if view_fog() {
            if fog.base_color.a < 0.5 {
                return vec4<f32>(0.0);
            }
            let r = vec4<f32>(fog.be.x, fog.be.y, fog.directional_light_color.r, 0.0);
            return vec4<f32>(fog.base_color.rgb, range_amount(r, depth));
        }
    }
    if water_color.w > 0.5 && world.y < water_range.w {
        if water_color.w > 1.5 {
            return vec4<f32>(water_color.rgb, height_amount(water_range, world, depth));
        }
        return vec4<f32>(water_color.rgb, range_amount(water_range, depth));
    }
    if color.w < 0.5 {
        return vec4<f32>(0.0);
    }
    // range.w 1: linear fog (map::MapFog::linear), toward the colour by f.
    if range.w > 0.5 {
        return vec4<f32>(color.rgb, sqrt(range_amount(range, depth)));
    }
    return vec4<f32>(color.rgb, range_amount(range, depth));
}

fn to_gamma(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}

// A surface's colour (linear) after `source_fog`'s `f`, toward the fog
// colour. With the world's fog linear (`range.w` 1: LithTech's and
// Halo's fixed-function table fog) the blend is of the stored, gamma
// values, as those engines' framebuffers hold them
// (specs/combat_arms/world_dat.md test case 14: 0.6 at f = 0.5 toward
// 23/255 is 0.3451); Source's blends in linear light (shaders.md 1).
fn fog_mix(c: vec3<f32>, f: vec4<f32>, range: vec4<f32>) -> vec3<f32> {
    if range.w > 0.5 && f.a > 0.0 {
        return to_linear(mix(to_gamma(c), to_gamma(f.rgb), f.a));
    }
    return mix(c, f.rgb, f.a);
}
