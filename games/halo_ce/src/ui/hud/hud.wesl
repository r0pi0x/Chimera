// Halo's HUD pictures (games/halo_ce/src/ui/hud/material.rs): a sprite
// drawn "alpha-multiply add" (specs/halo_ce/hud.md, section 3: the texel
// times the colour, the frame kept by 1 - its alpha), the meter shader's
// rule (section 4: the texel's blue channel is the place along the bar),
// the motion tracker's sweep, and the scope's darkening (section 9: the
// frame times 1 - the mask's grey level). The pipeline blends premultiplied: the
// output's rgb is added, the frame kept by 1 - its alpha. Texels and
// colours are the game's (gamma) values; the result is made linear last.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct HudParams {
    // x: mode (0 sprite, 1 meter, 2 sweep), y: meter value (a sprite's:
    // 1 a party crosshair), z: flash
    // extension, w: tint grey.
    mode: vec4<f32>,
    color: vec4<f32>,
    // The picture's texture rectangle: left, top, right, bottom.
    uv: vec4<f32>,
    // Gradient minimum (rgb), the frame kept behind a filled texel (a).
    lo: vec4<f32>,
    // Gradient maximum (rgb), the frame kept behind an empty texel (a).
    hi: vec4<f32>,
    // Flash colour (rgb), the sweep's window half size (a).
    flash: vec4<f32>,
    // Empty colour (rgb).
    empty: vec4<f32>,
}

@group(1) @binding(0) var<uniform> params: HudParams;
@group(1) @binding(1) var image: texture_2d<f32>;
@group(1) @binding(2) var image_sampler: sampler;
@group(1) @binding(3) var mask: texture_2d<f32>;
@group(1) @binding(4) var mask_sampler: sampler;

fn linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let mode = params.mode.x;
    if mode > 2.5 {
        // The scope's darkening: the frame kept by 1 - the texel's grey
        // level (its blue), nothing added.
        let t = mix(params.uv.xy, params.uv.zw, in.uv);
        let y = textureSample(image, image_sampler, t).b;
        return vec4<f32>(0.0, 0.0, 0.0, clamp(y, 0.0, 1.0));
    }
    if mode > 1.5 {
        // The sweep: the picture over a window of half size h about its
        // centre, in its colour, through the disc's mask.
        let h = params.flash.a;
        let t = vec2<f32>(0.5) + (in.uv - vec2<f32>(0.5)) * 2.0 * h;
        let s = textureSample(image, image_sampler, clamp(t, vec2<f32>(0.0), vec2<f32>(1.0)));
        let m = textureSample(mask, mask_sampler, in.uv);
        let a = s.a * m.a;
        return vec4<f32>(linear(params.color.rgb * s.rgb) * a, 0.0);
    }
    let t = mix(params.uv.xy, params.uv.zw, in.uv);
    let texel = textureSample(image, image_sampler, t);
    if mode > 0.5 {
        if texel.a <= 0.0 {
            discard;
        }
        let b = texel.b;
        var rgb: vec3<f32>;
        var keep: f32;
        if b < params.mode.y {
            rgb = mix(params.lo.rgb, params.hi.rgb, clamp(b, 0.0, 1.0)) * params.mode.w;
            keep = params.lo.a;
        } else if b < params.mode.z {
            rgb = params.flash.rgb;
            keep = params.lo.a;
        } else {
            rgb = params.empty.rgb;
            keep = params.hi.a;
        }
        return vec4<f32>(linear(rgb) * texel.a, (1.0 - keep) * texel.a);
    }
    let c = texel * params.color;
    // A party crosshair (mode.y 1): the frame also kept by 1 - its brightness.
    if params.mode.y > 0.5 {
        return vec4<f32>(linear(c.rgb), max(c.a, max(c.r, max(c.g, c.b))));
    }
    return vec4<f32>(linear(c.rgb), c.a);
}
