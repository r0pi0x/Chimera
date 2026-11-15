//! The model reader on synthetic data: every case of
//! specs/halo_ce/models.md's test section (a) (the minimal model: two
//! nodes, one marker, one region, one part, the four-index strip, the
//! two-node vertex; the corrupt variants; the quaternion examples), built
//! in code with `map_fixture`.

use blam_formats::cache::{MapFile, class};
use blam_formats::model::{
    DETAIL_LEVELS, Model, Transform, rotation_axes, strip_triangles, unpack_coordinate, unpack_vector,
};

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

const S: f32 = 0.707_106_77;

/// What the corrupt cases change in the minimal model.
#[derive(Clone, Copy)]
struct Opts {
    /// Added to the part's vertex record address.
    vertex_record_delta: u32,
    /// Vertex 2's first node byte.
    node_byte: u8,
    /// The strip's last index.
    last_index: u16,
    /// Node 1's parent.
    node1_parent: i16,
}

const GOOD: Opts = Opts {
    vertex_record_delta: 0,
    node_byte: 0x03,
    last_index: 3,
    node1_parent: 0,
};

fn vertex(pos: [f32; 3], u: i16, v: i16, n0: u8, n1: u8, w: i16) -> Vec<u8> {
    Struct::new(32)
        .f32s(0x00, &pos)
        .u32(0x0C, 0x7FC0_0000)
        .u32(0x10, 0x001F_F800)
        .u32(0x14, 0x0000_03FF)
        .i16(0x18, u)
        .i16(0x1A, v)
        .u8(0x1C, n0)
        .u8(0x1D, n1)
        .i16(0x1E, w)
        .into()
}

#[allow(clippy::too_many_arguments)]
fn node(
    name: &str,
    sibling: i16,
    child: i16,
    parent: i16,
    t: [f32; 3],
    q: [f32; 4],
    dist: f32,
    pos: [f32; 3],
) -> Vec<u8> {
    Struct::new(0x9C)
        .str(0x00, name)
        .i16(0x20, sibling)
        .i16(0x22, child)
        .i16(0x24, parent)
        .f32s(0x28, &t)
        .f32s(0x34, &q)
        .f32(0x44, dist)
        .f32(0x68, 1.0)
        .f32s(0x6C, &[0.0, 1.0, 0.0])
        .f32s(0x78, &[-1.0, 0.0, 0.0])
        .f32s(0x84, &[0.0, 0.0, 1.0])
        .f32s(0x90, &pos)
        .into()
}

/// The spec's minimal model as a map whose tag 1 is the model.
fn model_map(o: Opts) -> MapFile {
    let mut td = TagData::new(&[("scnr", "levels\\tiny"), ("mode", "models\\tiny")]);
    let verts: Vec<u8> = [
        vertex([0.0, 0.0, 1.0], 0, 0, 0x00, 0xFD, 32767),
        vertex([0.0, 1.0, 1.0], 0, 32767, 0x00, 0xFD, 32767),
        vertex([1.0, 0.0, 1.0], 32767, 0, o.node_byte, 0xFD, 32767),
        vertex([1.0, 1.0, 1.0], 32767, 32767, 0x00, 0x03, 24575),
    ]
    .concat();
    let vdata = td.append(&verts);
    let idx: Vec<u8> = [0u16, 1, 2, o.last_index]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let idata = td.append(&idx);
    td.model_buffers(&[vdata], &[idata]);
    let header = |at: usize| u32::from_le_bytes(td.heap.bytes[at..at + 4].try_into().unwrap());
    let (varr, iarr) = (header(0x14), header(0x1C));

    let part: Vec<u8> = Struct::new(0x68)
        .i16(0x04, 0)
        .u8(0x06, 0xFF)
        .u8(0x07, 0xFF)
        .i16(0x08, 0)
        .i16(0x0A, 1)
        .f32(0x0C, 0.75)
        .f32(0x10, 0.25)
        .f32s(0x14, &[0.5, 0.5, 1.0])
        .i16(0x44, 1)
        .i32(0x48, 2)
        .u32(0x4C, idata)
        .u32(0x50, iarr)
        .i16(0x54, 5)
        .i32(0x58, 4)
        .u32(0x64, varr + o.vertex_record_delta)
        .into();
    let parts = td.block(&[part]);
    let geometry: Vec<u8> = Struct::new(0x30).put(0x24, &parts).into();
    let geometries = td.block(&[geometry]);
    let permutation: Vec<u8> = Struct::new(0x58).str(0x00, "__base").into();
    let perms = td.block(&[permutation]);
    let region: Vec<u8> = Struct::new(0x4C).str(0x00, "body").put(0x40, &perms).into();
    let regions = td.block(&[region]);
    let nodes = td.block(&[
        node(
            "root",
            -1,
            1,
            -1,
            [0.0, 0.0, 1.0],
            [0.0, 0.0, S, S],
            0.0,
            [0.0, 0.0, -1.0],
        ),
        node(
            "tip",
            -1,
            -1,
            o.node1_parent,
            [2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            2.0,
            [-2.0, 0.0, -1.0],
        ),
    ]);
    let instance: Vec<u8> = Struct::new(0x20)
        .u8(0x02, 1)
        .f32s(0x04, &[0.5, 0.0, 0.0])
        .f32s(0x10, &[0.0, 0.0, S, S])
        .into();
    let instances = td.block(&[instance]);
    let marker: Vec<u8> = Struct::new(0x40)
        .str(0x00, "primary trigger")
        .put(0x34, &instances)
        .into();
    let markers = td.block(&[marker]);
    let shader: Vec<u8> = Struct::new(0x20).put(0x00, &null_ref("soso")).into();
    let shaders = td.block(&[shader]);

    let mut m = Struct::new(0xE8)
        .u32(0x04, 0x1234_5678)
        .f32s(0x08, &[1.0, 10.0, 20.0, 40.0, 80.0]);
    for l in 0..5 {
        m = m.i16(0x1C + 2 * l, 1);
    }
    let m = m
        .f32(0x30, 2.0)
        .f32(0x34, 1.0)
        .put(0xAC, &markers)
        .put(0xB8, &nodes)
        .put(0xC4, &regions)
        .put(0xD0, &geometries)
        .put(0xDC, &shaders);
    td.set_struct(1, &m.0);
    MapFile::from_image(CacheFile::new("tiny", 1, td.finish()).image()).unwrap()
}

fn read(o: Opts) -> std::io::Result<Model> {
    let map = model_map(o);
    let tag = map.of_class(class::MODEL).next().unwrap().clone();
    Model::read(&map, &tag)
}

fn model() -> Model {
    read(GOOD).unwrap()
}

fn close(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

fn assert_close3(a: [f32; 3], b: [f32; 3], eps: f32, what: &str) {
    assert!(
        a.iter().zip(&b).all(|(x, y)| close(*x, *y, eps)),
        "{what}: {a:?} vs {b:?}"
    );
}

#[test]
fn counts_and_fields() {
    let m = model();
    assert_eq!(m.nodes.len(), 2);
    assert_eq!(m.markers.len(), 1);
    assert_eq!(m.regions.len(), 1);
    assert_eq!(m.regions[0].name, "body");
    assert_eq!(m.regions[0].permutations.len(), 1);
    assert_eq!(m.geometries.len(), 1);
    assert_eq!(m.geometries[0].parts.len(), 1);
    assert_eq!(m.shaders.len(), 1);
    assert!(m.shaders[0].shader.is_null());
    assert_eq!(
        m.shaders[0].shader.class,
        blam_formats::cache::ClassCode::from_text("soso")
    );
    assert_eq!(m.node_list_checksum, 0x1234_5678);
    assert_eq!(m.detail_cutoffs, [1.0, 10.0, 20.0, 40.0, 80.0]);
    assert_eq!(m.node_counts, [1; 5]);
    assert_eq!((m.base_map_u_scale, m.base_map_v_scale), (2.0, 1.0));
    let p = &m.regions[0].permutations[0];
    assert_eq!(p.name, "__base");
    assert_eq!(p.geometry_indices, [Some(0); DETAIL_LEVELS]);
    assert_eq!(p.highest_detail(), Some(0));
    let part = &m.geometries[0].parts[0];
    assert_eq!((part.centroid_primary_node, part.centroid_secondary_node), (0, 1));
    assert_eq!(
        (part.centroid_primary_weight, part.centroid_secondary_weight),
        (0.75, 0.25)
    );
    assert_eq!(part.centroid, [0.5, 0.5, 1.0]);
    assert_eq!(part.shader, Some(0));
    assert_eq!((part.previous_filthy_part, part.next_filthy_part), (None, None));
}

#[test]
fn decoded_vectors() {
    let part = &model().geometries[0].parts[0];
    for v in &part.vertices {
        assert_close3(v.normal, [0.00048852, 0.00048852, 1.0], 1e-6, "normal");
        assert_close3(v.binormal, [0.00048852, 1.0, 0.00097752], 1e-6, "binormal");
        assert_close3(v.tangent, [1.0, 0.00048852, 0.00097752], 1e-6, "tangent");
    }
    assert_eq!(unpack_vector(0), [1.0 / 2047.0, 1.0 / 2047.0, 1.0 / 1023.0]);
    // The extremes decode to exactly -1 and +1.
    assert_eq!(unpack_coordinate(32767), 1.0);
    assert_eq!(unpack_coordinate(-32768), -1.0);
}

#[test]
fn texcoords_get_the_base_scale() {
    let part = &model().geometries[0].parts[0];
    let t0 = part.vertices[0].texcoord;
    assert!(
        close(t0[0], 2.0 / 65535.0, 1e-9) && close(t0[1], 1.0 / 65535.0, 1e-9),
        "{t0:?}"
    );
    assert_eq!(part.vertices[3].texcoord, [2.0, 1.0]);
}

#[test]
fn skin_nodes_and_weights() {
    let v = &model().geometries[0].parts[0].vertices;
    assert_eq!((v[0].node0, v[0].node1, v[0].weight0), (0, None, 1.0));
    assert_eq!((v[2].node0, v[2].node1, v[2].weight0), (1, None, 1.0));
    assert_eq!((v[3].node0, v[3].node1), (0, Some(1)));
    // 24575 / 32767 (the spec writes 0.7499771 and 0.2500229: a slip, its own rule gives these).
    assert!(close(v[3].weight0, 24575.0 / 32767.0, 1e-7), "{}", v[3].weight0);
    assert!(close(v[3].weight0, 0.749_992_4, 1e-6) && close(1.0 - v[3].weight0, 0.250_007_6, 1e-6));
    assert_eq!(v[1].position, [0.0, 1.0, 1.0]);
}

#[test]
fn strip_and_triangles() {
    let part = &model().geometries[0].parts[0];
    assert_eq!(part.strip, [0, 1, 2, 3]);
    assert_eq!(part.triangles, [[0, 1, 2], [1, 3, 2]]);
    assert_eq!(part.triangles_ccw(), [[0, 2, 1], [1, 2, 3]]);
    // Clockwise seen from +Z (the normals' side): negative signed area.
    for t in &part.triangles {
        let p = |i: u16| part.vertices[i as usize].position;
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        assert!(area < 0.0, "{t:?} has area {area}");
    }
    for t in part.triangles_ccw() {
        let p = |i: u16| part.vertices[i as usize].position;
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        assert!((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.0);
    }
}

#[test]
fn strip_drops_degenerates_but_keeps_parity() {
    assert_eq!(
        strip_triangles(&[0, 1, 2, 3, 3, 4, 5]),
        [[0, 1, 2], [1, 3, 2], [3, 4, 5]]
    );
    assert!(strip_triangles(&[]).is_empty());
    assert!(strip_triangles(&[0, 1]).is_empty());
}

#[test]
fn quaternion_examples() {
    // Stored (0.5, 0.5, 0.5, 0.5): forward +Z, left +X, up +Y.
    let [f, l, u] = rotation_axes([0.5; 4]);
    assert_close3(f, [0.0, 0.0, 1.0], 1e-6, "forward");
    assert_close3(l, [1.0, 0.0, 0.0], 1e-6, "left");
    assert_close3(u, [0.0, 1.0, 0.0], 1e-6, "up");
    // The matrix with rows (0,1,0), (0,0,1), (1,0,0) sends +X to +Z.
    let t = Transform::from_quaternion([0.5; 4], [0.0; 3]);
    assert_close3(t.apply_vector([1.0, 0.0, 0.0]), [0.0, 0.0, 1.0], 1e-6, "x");
    // Stored (0, 0, sin 45, cos 45): +X to -Y.
    let [f, l, u] = rotation_axes([0.0, 0.0, S, S]);
    assert_close3(f, [0.0, -1.0, 0.0], 1e-6, "forward");
    assert_close3(l, [1.0, 0.0, 0.0], 1e-6, "left");
    assert_close3(u, [0.0, 0.0, 1.0], 1e-6, "up");
    // A zero quaternion has no rotation to apply (s = 0): the identity.
    assert_eq!(
        rotation_axes([0.0; 4]),
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );
}

#[test]
fn node_transforms() {
    let m = model();
    let w = m.bind_pose();
    assert_eq!(w.len(), 2);
    assert_close3(w[0].forward, [0.0, -1.0, 0.0], 1e-6, "n0 forward");
    assert_close3(w[0].left, [1.0, 0.0, 0.0], 1e-6, "n0 left");
    assert_close3(w[0].up, [0.0, 0.0, 1.0], 1e-6, "n0 up");
    assert_close3(w[0].position, [0.0, 0.0, 1.0], 1e-6, "n0 position");
    assert_close3(w[1].forward, w[0].forward, 1e-6, "n1 forward");
    assert_close3(w[1].position, [0.0, -2.0, 1.0], 1e-6, "n1 position");
    let d = (0..3)
        .map(|k| (w[1].position[k] - w[0].position[k]).powi(2))
        .sum::<f32>()
        .sqrt();
    assert!(close(d, 2.0, 1e-6) && m.nodes[1].distance_from_parent == 2.0);
    assert_eq!((m.nodes[0].parent, m.nodes[0].first_child), (None, Some(1)));
    assert_eq!((m.nodes[1].parent, m.nodes[1].first_child), (Some(0), None));
}

#[test]
fn inverse_matrices_and_bind_pose_skin() {
    let m = model();
    let w = m.bind_pose();
    for (n, node) in m.nodes.iter().enumerate() {
        // The stored inverse is the inverse of the bind pose.
        assert!(w[n].inverse().max_difference(&node.inverse) < 1e-6, "node {n}");
        // So every bind-pose skin matrix is the identity, and skinned
        // positions equal the stored ones.
        let skin = w[n].then(&node.inverse);
        assert!(skin.max_difference(&Transform::IDENTITY) < 1e-6, "skin {n}");
    }
    let v = &m.geometries[0].parts[0].vertices[2];
    let skinned = w[1].then(&m.nodes[1].inverse).apply_point(v.position);
    assert_close3(skinned, v.position, 1e-6, "skinned");
}

#[test]
fn markers() {
    let m = model();
    let marker = m.find_marker("PRIMARY TRIGGER").expect("case-insensitive");
    assert_eq!(marker.name, "primary trigger");
    assert_eq!(marker.instances.len(), 1);
    assert!(m.find_marker("nothing").is_none());
    let w = m.bind_pose();
    let t = m.marker_world(&marker.instances[0], &w).unwrap();
    assert_close3(t.position, [0.0, -2.5, 1.0], 1e-6, "position");
    assert_close3(t.forward, [-1.0, 0.0, 0.0], 1e-6, "forward");
    assert_close3(t.left, [0.0, -1.0, 0.0], 1e-6, "left");
    assert_close3(t.up, [0.0, 0.0, 1.0], 1e-6, "up");
}

#[test]
fn level_of_detail() {
    let m = model();
    assert_eq!(m.level_for_size(1000.0), Some(4));
    assert_eq!(m.level_for_size(15.0), Some(1));
    assert_eq!(m.level_for_size(20.0), Some(2));
    assert_eq!(m.level_for_size(0.5), None);
}

#[test]
fn parts_for_a_level_and_permutations() {
    let m = model();
    let parts = m.highest_detail_parts();
    assert_eq!(parts.len(), 1);
    assert_eq!(
        (
            parts[0].region,
            parts[0].permutation,
            parts[0].geometry,
            parts[0].part_index
        ),
        (0, 0, 0, 0)
    );
    assert!(parts[0].shader.is_some());
    assert_eq!(m.parts_at(0, &[0]).unwrap().len(), 1);
    assert!(m.parts_at(5, &[]).is_err());
    assert!(m.parts_at(4, &[1]).is_err());
}

fn error_of(o: Opts) -> String {
    read(o).expect_err("a corrupt model is refused").to_string()
}

#[test]
fn corrupt_vertex_record() {
    let e = error_of(Opts {
        vertex_record_delta: 4,
        ..GOOD
    });
    assert!(e.contains("multiple of 12") && e.contains("models\\tiny"), "{e}");
}

#[test]
fn corrupt_node_byte() {
    let e = error_of(Opts {
        node_byte: 0x04,
        ..GOOD
    });
    assert!(e.contains("vertex 2") && e.contains("node byte"), "{e}");
}

#[test]
fn corrupt_strip_index() {
    let e = error_of(Opts { last_index: 9, ..GOOD });
    assert!(e.contains("strip index 9") && e.contains("4 vertices"), "{e}");
}

#[test]
fn corrupt_node_parent() {
    let e = error_of(Opts {
        node1_parent: 1,
        ..GOOD
    });
    assert!(e.contains("not below its own index"), "{e}");
}

#[test]
fn not_a_model_is_refused() {
    let map = model_map(GOOD);
    let scenario = map.scenario().clone();
    assert!(Model::read(&map, &scenario).is_err());
}
