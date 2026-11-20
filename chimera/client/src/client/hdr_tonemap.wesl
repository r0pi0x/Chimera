// Source-style tone mapping for mat_hdr_level 1 and 2 (see hdr.rs): the
// scene's linear light times one scale, then clamped as an 8-bit
// framebuffer would. No curve: Source's HDR keeps the LDR look in range.
// The stand-in model takes its scale from Bevy's auto exposure within the
// map's tone-map controller bounds; the integer model (TF2,
// specs/tf2/hdr_bloom.md) is given it (`render::source_hdr::Exposure`) and
// stores the result as 8-bit sRGB bytes would hold it.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;
import bevy_render::view::View;

struct SourceTonemap {
    min_scale: f32,
    max_scale: f32,
    // 1: scale by auto exposure; 0: scale 1 (bloom only).
    auto_exposure: f32,
    // 1: the integer model: scale by `scale`, quantize to 8-bit sRGB.
    integer: f32,
    scale: f32,
    bloom: f32,
    frame: u32,
    _pad: u32,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: SourceTonemap;
// Auto exposure writes its result into the view's colour grading exposure
// (in stops) before this pass runs.
@group(0) @binding(3) var<uniform> view: View;

fn srgb_encode(l: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(l, vec3<f32>(1.0 / 2.4)) - 0.055, l * 12.92, l <= vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(screen, screen_sampler, in.uv);
    if settings.integer > 0.5 {
        let v = clamp(c.rgb * settings.scale, vec3<f32>(0.0), vec3<f32>(1.0));
        return vec4<f32>(srgb_decode(round(srgb_encode(v) * 255.0) / 255.0), 1.0);
    }
    var scale = 1.0;
    if settings.auto_exposure > 0.5 {
        scale = clamp(exp2(view.color_grading.exposure), settings.min_scale, settings.max_scale);
    }
    return vec4<f32>(clamp(c.rgb * scale, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
