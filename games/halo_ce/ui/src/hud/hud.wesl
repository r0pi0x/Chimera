// Halo's HUD pictures (games/halo_ce/ui/src/hud/material.rs): a sprite
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
    // x: scaling (0 linear, 1 sharp, 2 smooth and sharpened).
    style: vec4<f32>,
}

@group(1) @binding(0) var<uniform> params: HudParams;
@group(1) @binding(1) var image: texture_2d<f32>;
@group(1) @binding(2) var image_sampler: sampler;
@group(1) @binding(3) var mask: texture_2d<f32>;
@group(1) @binding(4) var mask_sampler: sampler;

fn linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// "Sharp" sampling (material.rs `sharp_coord`): nearest inside a texel,
// linear over one pixel's width at its edge; the sampler is linear.
fn sharp(t: vec2<f32>, size: vec2<f32>, pixels_per_texel: vec2<f32>) -> vec2<f32> {
    let p = t * size;
    let s = max(pixels_per_texel, vec2<f32>(1.0));
    let base = floor(p);
    let c = p - base - vec2<f32>(0.5);
    let flat_ = vec2<f32>(0.5) - vec2<f32>(0.5) / s;
    return (base + (c - clamp(c, -flat_, flat_)) * s + vec2<f32>(0.5)) / size;
}

// A sprite's texel by the setting.
fn sprite_texel(t: vec2<f32>) -> vec4<f32> {
    let style = params.style.x;
    let size = vec2<f32>(textureDimensions(image));
    if style > 1.5 {
        // Unsharp mask: the linear sample, plus its difference from the
        // mean of the four neighbours a screen pixel away.
        let d = fwidth(t);
        let c = textureSample(image, image_sampler, t);
        let n = (textureSample(image, image_sampler, t + vec2<f32>(d.x, 0.0))
            + textureSample(image, image_sampler, t - vec2<f32>(d.x, 0.0))
            + textureSample(image, image_sampler, t + vec2<f32>(0.0, d.y))
            + textureSample(image, image_sampler, t - vec2<f32>(0.0, d.y))) * 0.25;
        let s = c + (c - n) * 0.6;
        return vec4<f32>(clamp(s.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), c.a);
    }
    if style > 0.5 {
        // Screen pixels per texel: the reciprocal of the texels one pixel spans.
        let per_pixel = fwidth(t * size);
        return textureSample(image, image_sampler, sharp(t, size, vec2<f32>(1.0) / max(per_pixel, vec2<f32>(1e-4))));
    }
    return textureSample(image, image_sampler, t);
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
    // A sprite turned clockwise by z quarter turns (the damage arrows).
    var local = in.uv;
    let quarters = select(0.0, params.mode.z, params.mode.x < 0.5);
    if quarters > 2.5 {
        local = vec2<f32>(1.0 - in.uv.y, in.uv.x);
    } else if quarters > 1.5 {
        local = vec2<f32>(1.0) - in.uv;
    } else if quarters > 0.5 {
        local = vec2<f32>(in.uv.y, 1.0 - in.uv.x);
    }
    let t = mix(params.uv.xy, params.uv.zw, local);
    // Meters keep their nearest sampling; the sprites take the setting.
    var texel: vec4<f32>;
    if mode > 0.5 {
        texel = textureSample(image, image_sampler, t);
    } else {
        texel = sprite_texel(t);
    }
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
