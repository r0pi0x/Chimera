// The integer HDR's exposure meter (specs/tf2/hdr_bloom.md 3.1-3.2; see
// hdr_integer.rs): every bin of the centre region's luminance histogram
// counted this frame (the CPU keeps one bin per frame,
// `render::source_hdr::Meter`). Bins have edges (b / 16)^1.5, both
// inclusive, the top one taking anything above 1; count 16 is every
// metered pixel; slot 17 gets the frame's number for the readback.

struct MeterParams {
    // x0, y0, x1, y1 (ends exclusive).
    region: vec4<u32>,
    frame: u32,
    _a: u32,
    _b: u32,
    _c: u32,
}

@group(0) @binding(0) var frame_texture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> counts: array<atomic<u32>, 18>;
@group(0) @binding(2) var<uniform> params: MeterParams;

var<workgroup> local_counts: array<atomic<u32>, 17>;

fn edge(b: u32) -> f32 {
    return pow(f32(b) / 16.0, 1.5);
}

@compute @workgroup_size(16, 16, 1)
fn meter(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_index) index: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    if index < 17u {
        atomicStore(&local_counts[index], 0u);
    }
    workgroupBarrier();
    let pixel = gid.xy + params.region.xy;
    if all(pixel < params.region.zw) {
        let c = clamp(textureLoad(frame_texture, pixel, 0).rgb, vec3<f32>(0.0), vec3<f32>(1.0));
        let y = dot(c, vec3<f32>(0.2125, 0.7154, 0.0721));
        for (var b = 0u; b < 16u; b++) {
            if y >= edge(b) && (y <= edge(b + 1u) || b == 15u) {
                atomicAdd(&local_counts[b], 1u);
            }
        }
        atomicAdd(&local_counts[16], 1u);
    }
    workgroupBarrier();
    if index < 17u {
        let n = atomicLoad(&local_counts[index]);
        if n > 0u {
            atomicAdd(&counts[index], n);
        }
    }
    if index == 0u && group.x == 0u && group.y == 0u {
        atomicStore(&counts[17], params.frame);
    }
}
