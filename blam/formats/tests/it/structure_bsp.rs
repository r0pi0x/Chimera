//! The structure BSP reader on synthetic data: section (a) of
//! specs/halo_ce/structure_bsp.md ("Test cases", the synthetic structure
//! and the decoder unit tests), plus refusals of corrupt data.

use blam_formats::cache::MapFile;
use blam_formats::structure_bsp::{ClusterFog, StructureBsp, unpack_coordinate, unpack_vector};

use crate::bsp_fixture::{MAIN_SIZE, SpecParts, map_with_bsp, structure_map};
use crate::map_fixture::Struct;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn near3(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| near(a[i], b[i]))
}

#[test]
fn spec_synthetic_structure() {
    let map = structure_map(&SpecParts::spec());
    assert_eq!(map.bsps().unwrap().len(), 1);
    let bsp = StructureBsp::read(&map, 0).unwrap();
    assert_eq!(bsp.lightmap_groups.len(), 1);
    assert_eq!(bsp.lightmap_groups[0].bitmap_index, Some(0));
    assert_eq!(bsp.material_count(), 1);
    assert!(bsp.lightmap_bitmap.is_null());
    assert_eq!(bsp.triangle_count(), 2);
    let m = bsp.materials().next().unwrap();
    assert!(m.compressed);
    assert!(m.shader.is_null());
    assert_eq!((m.first_surface, m.surface_count), (0, 2));
    assert_eq!(m.breakable_surface, None);
    assert!(m.has_lightmap());
    assert!(!m.flags.coplanar() && !m.flags.fog_plane());
    assert_eq!((bsp.vertex_count(), bsp.lightmap_vertex_count()), (4, 4));
    let wb = bsp.world_bounds;
    assert_eq!((wb.x.lower, wb.x.upper, wb.y.upper, wb.z.upper), (0.0, 1.0, 1.0, 0.0));

    let g = bsp.material_geometry(0).unwrap();
    assert_eq!(g.vertices.len(), 4);
    for v in &g.vertices {
        assert!(near3(v.normal, [0.00048852, 0.00048852, 1.0]), "{:?}", v.normal);
        assert!(near3(v.binormal, [0.00048852, -1.0, 0.00097752]), "{:?}", v.binormal);
        assert!(near3(v.tangent, [1.0, 0.00048852, 0.00097752]), "{:?}", v.tangent);
        assert_eq!(v.packed, [0x7FC0_0000, 0x0020_0000, 0x0000_03FF]);
    }
    let pos: Vec<_> = g.vertices.iter().map(|v| v.position).collect();
    assert_eq!(pos, [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]);
    let uv: Vec<_> = g.vertices.iter().map(|v| v.uv).collect();
    assert_eq!(uv, [[0., 1.], [1., 1.], [1., 0.], [0., 0.]]);
    let lm: Vec<_> = g.lightmap_vertices.iter().map(|v| v.uv).collect();
    let want = [[0.0000153, 1.0], [1.0, 1.0], [1.0, 0.0000153], [0.0000153, 0.0000153]];
    for (a, b) in lm.iter().zip(want) {
        assert!(near(a[0], b[0]) && near(a[1], b[1]), "{lm:?}");
    }
    assert_eq!(g.lightmap_vertices[0].raw_uv, [0, 32767]);
    assert!(near3(
        g.lightmap_vertices[3].incident_direction,
        [0.00048852, 0.00048852, 1.0]
    ));
    assert_eq!(g.triangles, [[0, 3, 2], [0, 2, 1]]);

    // Triangles in world units and the facing the spec states: clockwise
    // from the front, so the cross product points opposite the normal.
    let tri = |i: usize| g.triangles[i].map(|k| g.vertices[k as usize].position);
    assert_eq!(tri(0), [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.]]);
    assert_eq!(tri(1), [[0., 0., 0.], [1., 1., 0.], [1., 0., 0.]]);
    for i in 0..2 {
        let [p0, p1, p2] = tri(i);
        let a = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let b = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        assert_eq!(cross, [0., 0., -1.]);
        assert!(cross[2] * g.vertices[0].normal[2] < 0.0);
    }
    assert_eq!(bsp.material_triangles(m), [[0, 3, 2], [0, 2, 1]]);
}

#[test]
fn decoders() {
    assert_eq!(unpack_vector(0x0000_0400)[0], -1.0);
    assert_eq!(unpack_vector(0x8000_0000)[2], -1.0);
    assert_eq!(unpack_vector(0x3FF)[0], 1.0);
    assert_eq!(unpack_vector(0x1FF << 22)[2], 1.0);
    let z = unpack_vector(0);
    assert!(near3(z, [0.00048852, 0.00048852, 0.00097752]), "{z:?}");
    assert_eq!(unpack_coordinate(-32768), -1.0);
    assert_eq!(unpack_coordinate(32767), 1.0);
    assert!(near(unpack_coordinate(0), 1.0 / 65535.0));
}

#[test]
fn uncompressed_forms_are_decoded() {
    let mut data = Vec::new();
    let vertex = |p: f32| {
        Struct::new(56)
            .f32s(0, &[p, 0.0, 0.0])
            .f32s(12, &[0.0, 0.0, 1.0])
            .f32s(24, &[0.0, 1.0, 0.0])
            .f32s(36, &[1.0, 0.0, 0.0])
            .f32s(48, &[0.25, 0.5])
            .0
    };
    for p in [0.0, 1.0, 2.0] {
        data.extend(vertex(p));
    }
    for k in 0..3 {
        data.extend(
            Struct::new(20)
                .f32s(0, &[0.0, 0.0, 1.0])
                .f32s(12, &[k as f32 / 4.0, 0.5])
                .0,
        );
    }
    let parts = SpecParts {
        data,
        rendered_form: 0,
        vertices: 3,
        lightmap_form: 2,
        lightmap_vertices: 3,
        surfaces: vec![[0, 1, 2]],
        material_surfaces: (0, 1),
        data_ref_at: 0xD8,
    };
    let bsp = StructureBsp::read(&structure_map(&parts), 0).unwrap();
    let m = bsp.materials().next().unwrap();
    assert!(!m.compressed);
    let g = bsp.decode(m).unwrap();
    assert_eq!(g.vertices[2].position, [2.0, 0.0, 0.0]);
    assert_eq!(g.vertices[1].normal, [0.0, 0.0, 1.0]);
    assert_eq!(g.vertices[0].uv, [0.25, 0.5]);
    assert_eq!(g.lightmap_vertices[2].uv, [0.5, 0.5]);
    assert_eq!(g.triangles, [[0, 1, 2]]);
}

#[test]
fn a_material_without_lightmap_vertices() {
    let mut p = SpecParts::spec();
    p.data.truncate(4 * 32);
    p.lightmap_vertices = 0;
    p.lightmap_form = 3;
    let bsp = StructureBsp::read(&structure_map(&p), 0).unwrap();
    let m = bsp.materials().next().unwrap();
    assert!(!m.has_lightmap());
    assert!(bsp.decode(m).unwrap().lightmap_vertices.is_empty());
}

fn read_err(p: &SpecParts) -> String {
    match StructureBsp::read(&structure_map(p), 0).and_then(|b| b.material_geometry(0)) {
        Ok(_) => panic!("accepted a corrupt BSP"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn corrupt_geometry_is_refused() {
    let ok = SpecParts::spec();
    let mut p = ok.clone();
    p.surfaces[0] = [0, 3, 4];
    assert!(read_err(&p).contains("triangle index 4"), "index past the vertices");
    let mut p = ok.clone();
    p.material_surfaces = (1, 5);
    assert!(read_err(&p).contains("surfaces 1+5"));
    let mut p = ok.clone();
    p.material_surfaces = (0, 1);
    assert!(read_err(&p).contains("own 1 surfaces"), "a surface no material owns");
    let mut p = ok.clone();
    p.data.truncate(100);
    assert!(read_err(&p).contains("vertex data of 100 bytes"));
    let mut p = ok.clone();
    p.lightmap_vertices = 3;
    assert!(read_err(&p).contains("lightmap vertices"));
    let mut p = ok.clone();
    p.rendered_form = 7;
    assert!(read_err(&p).contains("rendered vertex form 7"));
    let mut p = ok;
    p.vertices = -1;
    assert!(read_err(&p).contains("vertex counts"));
}

#[test]
fn an_empty_bsp_and_a_bad_index() {
    let map: MapFile = map_with_bsp(0x1000, |_| Struct::new(MAIN_SIZE));
    let b = StructureBsp::read(&map, 0).unwrap();
    assert_eq!((b.material_count(), b.triangle_count(), b.clusters.len()), (0, 0, 0));
    assert_eq!(b.visibility.words.len(), 0);
    assert!(!b.visibility.visible(0, 0));
    assert!(b.material_geometry(0).is_err());
    assert!(StructureBsp::read(&map, 1).is_err());
    fn is_send_sync<T: Send + Sync>() {}
    is_send_sync::<StructureBsp>();
}

#[test]
fn visibility_rows_and_sound_distances() {
    // Three clusters (one word per row) and the sound table's upper
    // triangle (3 bytes).
    let map = map_with_bsp(0x1000, |h| {
        let cluster = || {
            Struct::new(104)
                .i16(0, -1)
                .i16(2, -1)
                .i16(4, -1)
                .i16(6, -1)
                .i16(8, -1)
                .i16(0x0C, -1)
                .0
        };
        let clusters = h.block(&[cluster(), cluster(), cluster()]);
        let pvs: Vec<u8> = [0b111u32, 0b101, 0b111].iter().flat_map(|w| w.to_le_bytes()).collect();
        let pvs = h.data(&pvs);
        let sound = h.data(&[10, 20, 30, 0]);
        Struct::new(MAIN_SIZE)
            .put(0x134, &clusters)
            .put(0x140, &pvs)
            .put(0x214, &sound)
    });
    let b = StructureBsp::read(&map, 0).unwrap();
    assert_eq!(b.clusters.len(), 3);
    assert_eq!(b.clusters[0].sky, None);
    assert_eq!(b.visibility.words_per_row, 1);
    assert!(b.visibility.visible(0, 1) && !b.visibility.visible(1, 1) && b.visibility.visible(2, 2));
    assert!(!b.visibility.visible(3, 0));
    // i(N-1) - i(i+1)/2 + j - 1 for N = 3: (0,1) 0, (0,2) 1, (1,2) 2.
    assert_eq!(b.sound_distance(0, 1), Some(10));
    assert_eq!(b.sound_distance(2, 0), Some(20));
    assert_eq!(b.sound_distance(1, 2), Some(30));
    assert_eq!(b.sound_distance(1, 1), Some(0));
    assert_eq!(b.sound_distance(1, 3), None);
}

#[test]
fn clusters_portals_and_fog() {
    let map = map_with_bsp(0x2000, |h| {
        let verts = h.block(&[
            Struct::new(12).f32s(0, &[0., 0., 0.]).0,
            Struct::new(12).f32s(0, &[1., 0., 0.]).0,
            Struct::new(12).f32s(0, &[0., 1., 0.]).0,
        ]);
        let portal = Struct::new(64)
            .i16(0, 0)
            .i16(2, 1)
            .i32(4, 3)
            .f32s(8, &[0.5, 0.5, 0.0])
            .f32(0x14, 1.5)
            .put(0x34, &verts)
            .0;
        let portals = h.block(&[portal]);
        let portal_idx = h.block(&[Struct::new(2).i16(0, 0).0]);
        let cluster = |fog: i16, p: &[u8; 12]| {
            Struct::new(104)
                .i16(0, 0)
                .i16(2, fog)
                .i16(4, -1)
                .i16(6, -1)
                .i16(8, -1)
                .i16(0x0C, -1)
                .put(0x5C, p)
                .0
        };
        let none = [0u8; 12];
        let clusters = h.block(&[cluster(i16::MIN, &portal_idx), cluster(2, &none)]);
        let pvs: Vec<u8> = [3u32, 3].iter().flat_map(|w| w.to_le_bytes()).collect();
        let pvs = h.data(&pvs);
        let leaves = h.block(&[Struct::new(16).i16(8, 1).0, Struct::new(16).i16(8, 0).0]);
        Struct::new(MAIN_SIZE)
            .put(0x134, &clusters)
            .put(0x140, &pvs)
            .put(0x154, &portals)
            .put(0xE0, &leaves)
    });
    let b = StructureBsp::read(&map, 0).unwrap();
    assert_eq!(b.clusters[0].fog, ClusterFog::Plane(0));
    assert_eq!(b.clusters[1].fog, ClusterFog::Region(2));
    assert_eq!(b.clusters[0].portals, [0]);
    let p = &b.cluster_portals[0];
    assert_eq!((p.cluster_a, p.cluster_b, p.plane, p.bounding_radius), (0, 1, 3, 1.5));
    assert_eq!(p.vertices, [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]);
    assert_eq!(b.cluster_of_leaf(0), Some(1));
    assert_eq!(b.cluster_of_leaf(1), Some(0));
    assert_eq!(b.cluster_of_leaf(2), None);
}

#[test]
fn bad_cluster_references_are_refused() {
    let build = |portal_cluster: i16, leaf_cluster: i16| {
        map_with_bsp(0x2000, move |h| {
            let portals = h.block(&[Struct::new(64).i16(0, portal_cluster).i16(2, 0).0]);
            let clusters = h.block(&[Struct::new(104).0]);
            let pvs = h.data(&1u32.to_le_bytes());
            let leaves = h.block(&[Struct::new(16).i16(8, leaf_cluster).0]);
            Struct::new(MAIN_SIZE)
                .put(0x134, &clusters)
                .put(0x140, &pvs)
                .put(0x154, &portals)
                .put(0xE0, &leaves)
        })
    };
    assert!(StructureBsp::read(&build(0, 0), 0).is_ok());
    assert!(StructureBsp::read(&build(1, 0), 0).is_err());
    assert!(StructureBsp::read(&build(0, 1), 0).is_err());
}
