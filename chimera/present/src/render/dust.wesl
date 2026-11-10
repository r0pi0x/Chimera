// Dust motes (render::dust): each quad of the volume's mesh is one mote of
// the buffer (`motes`, far to near), facing the camera at its position with
// its half-width and colour; quads past the motes' count collapse to a
// point. The fragment stage is the volume's unlit StandardMaterial's.

import bevy_pbr::{
    render::forward_io::{Vertex, VertexOutput, decompress_vertex},
    render::view_transformations::position_world_to_clip,
    render::mesh_functions,
};

struct Mote {
    // Position, half-width.
    at: vec4<f32>,
    color: vec4<f32>,
}

struct Motes {
    // The camera's right; w: how many motes.
    right: vec4<f32>,
    up: vec4<f32>,
    // The quads' normal (toward the camera).
    normal: vec4<f32>,
    motes: array<Mote>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> dust: Motes;

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let v = decompress_vertex(vertex_in, vertex_in.instance_index);
    var q = 0u;
    @if(VERTEX_UVS_B) {
        q = u32(v.uv_b.x + 0.5);
    }
    // Corners from the coordinates: u 0 left, 1 right; v 0 top, 1 bottom.
    let cx = v.uv.x * 2.0 - 1.0;
    let cy = 1.0 - v.uv.y * 2.0;
    var world = vec3<f32>(0.0);
    var color = vec4<f32>(0.0);
    if f32(q) < dust.right.w {
        let m = dust.motes[q];
        world = m.at.xyz + (dust.right.xyz * cx + dust.up.xyz * cy) * m.at.w;
        color = m.color;
    }
    out.world_position = vec4<f32>(world, 1.0);
    out.position = position_world_to_clip(world);
    out.world_normal = dust.normal.xyz;
    out.uv = v.uv;
    @if(VERTEX_UVS_B) {
        out.uv_b = v.uv_b;
    }
    out.color = color;
    @if(VERTEX_OUTPUT_INSTANCE_INDEX) {
        out.instance_index = vertex_in.instance_index;
    }
    @if(VISIBILITY_RANGE_DITHER) {
        out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
            vertex_in.instance_index, mesh_functions::get_world_from_local(vertex_in.instance_index)[3]);
    }
    return out;
}
