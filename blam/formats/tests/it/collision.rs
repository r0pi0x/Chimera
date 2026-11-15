//! The collision BSP reader on synthetic data: section (a) of
//! specs/halo_ce/collision.md (the closed box room: polygons, fan
//! triangles, point tests, line traces) and refusals and panic-freedom on
//! corrupt data.

use blam_formats::cache::Plane3d;
use blam_formats::collision::{
    Child, CollisionBsp, Edge, Leaf, MaterialType, Node3d, Reference2d, Surface, SurfaceFlags, TraceFlags, Vertex,
    flipped, read_level, read_model, split_plane,
};

use crate::bsp_fixture::{MAIN_SIZE, box_room_map, map_with_bsp};
use crate::map_fixture::Struct;

fn room() -> CollisionBsp {
    read_level(&box_room_map(), 0).unwrap().bsp
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[test]
fn box_room_counts_and_materials() {
    let level = read_level(&box_room_map(), 0).unwrap();
    let b = &level.bsp;
    assert_eq!(
        (
            b.nodes.len(),
            b.planes.len(),
            b.leaves.len(),
            b.references.len(),
            b.nodes_2d.len(),
            b.surfaces.len(),
            b.edges.len(),
            b.vertices.len()
        ),
        (6, 6, 1, 6, 0, 6, 12, 8)
    );
    assert_eq!(level.materials.len(), 1);
    assert_eq!(level.materials[0].material_type(), Some(MaterialType::Stone));
    assert_eq!(level.surface_material_type(0), Some(MaterialType::Stone));
    assert_eq!(level.surface_material_type(99), None);
    assert!(b.surfaces.iter().all(|s| s.flags.0 == 0 && s.material == 0));
}

#[test]
fn box_room_polygons() {
    let b = room();
    let expected: [[[f32; 3]; 4]; 6] = [
        [[0., 0., 0.], [4., 0., 0.], [4., 4., 0.], [0., 4., 0.]],
        [[0., 0., 3.], [0., 4., 3.], [4., 4., 3.], [4., 0., 3.]],
        [[0., 0., 0.], [0., 4., 0.], [0., 4., 3.], [0., 0., 3.]],
        [[4., 0., 0.], [4., 0., 3.], [4., 4., 3.], [4., 4., 0.]],
        [[0., 0., 0.], [0., 0., 3.], [4., 0., 3.], [4., 0., 0.]],
        [[0., 4., 0.], [4., 4., 0.], [4., 4., 3.], [0., 4., 3.]],
    ];
    for (s, want) in expected.iter().enumerate() {
        assert_eq!(b.polygon_points(s).unwrap(), want, "surface {s}");
        // Counter-clockwise from inside: the cross product is parallel to
        // the plane's normal.
        let p = b.polygon_points(s).unwrap();
        let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        let n = b.surface_plane(s).unwrap().normal;
        let k = c[0] * n[0] + c[1] * n[1] + c[2] * n[2];
        assert!(k > 0.0 && (cross(c, n).iter().all(|x| x.abs() < 1e-6)), "surface {s}");
    }
    let tris = b.triangulate().unwrap();
    assert_eq!(tris.len(), 12);
    assert_eq!(tris[0].vertices, [0, 1, 2]);
    assert_eq!(tris[1].vertices, [0, 2, 3]);
    assert_eq!(tris[2].surface, 1);
    assert!(b.polygon(6).is_err());
}

#[test]
fn box_room_point_tests() {
    let b = room();
    assert_eq!(b.point_leaf([1., 1., 1.]), Some(0));
    assert_eq!(b.point_leaf([5., 1., 1.]), None);
    assert_eq!(b.point_leaf([1., 1., -0.001]), None);
    // A point exactly on a plane is in front.
    assert_eq!(b.point_leaf([0., 0., 0.]), Some(0));
    // A NaN point never panics and is not in any leaf (every test fails).
    let _ = b.point_leaf([f32::NAN, 0., 0.]);
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn box_room_traces() {
    let b = room();
    let both = TraceFlags::FRONT_AND_BACK;

    let r = b.trace_ray([1., 1., 2.], [0., 0., -3.], both);
    let h = r.hit.expect("a hit");
    assert!(close(r.t, 2.0 / 3.0), "{}", r.t);
    assert_eq!(h.surface, 0);
    assert_eq!(h.plane.normal, [0., 0., 1.]);
    assert!(
        h.point.iter().zip([1., 1., 0.]).all(|(a, b)| close(*a, b)),
        "{:?}",
        h.point
    );
    assert_eq!(h.material, 0);
    assert_eq!(r.leaves, [0]);

    let r = b.trace_ray([1., 1., 1.], [2., 2., 1.], both);
    assert!(r.hit.is_none());
    assert_eq!(r.t, 1.0);

    let r = b.trace_ray([1., 1., -1.], [0., 0., 2.], TraceFlags::FRONT_FACING);
    assert!(r.hit.is_none(), "front only, entering from solid");

    let r = b.trace_ray([1., 1., -1.], [0., 0., 2.], both);
    let h = r.hit.expect("a hit");
    assert!(close(r.t, 0.5));
    assert_eq!((h.surface, h.plane.normal), (0, [0., 0., 1.]));

    let r = b.trace_ray([2., 2., 1.], [4., 0., 0.], both);
    let h = r.hit.expect("a hit");
    assert!(close(r.t, 0.5));
    assert_eq!((h.surface, h.plane.normal), (3, [-1., 0., 0.]));

    let r = b.trace_ray([5., 5., 5.], [1., 0., 0.], both);
    assert!(r.hit.is_none(), "all solid");

    // Neither facing bit set: both are used.
    let r = b.trace_ray([1., 1., 2.], [0., 0., -3.], TraceFlags(0));
    assert_eq!(r.hit.map(|h| h.surface), Some(0));
    // A maximum t shorter than the hit: no hit.
    let r = b.trace([1., 1., 2.], [0., 0., -3.], both, 0.5, &|_| false);
    assert!(r.hit.is_none());
    let r = b.trace([1., 1., 2.], [0., 0., -3.], both, 0.9, &|_| false);
    assert!(r.hit.is_some());
}

#[test]
fn plane_helpers() {
    assert_eq!(split_plane(5), (5, false));
    assert_eq!(split_plane(0x8000_0003u32 as i32), (3, true));
    assert_eq!(Child::from_raw(-1), Child::None);
    assert_eq!(Child::from_raw(0x8000_0002u32 as i32), Child::Terminal(2));
    assert_eq!(Child::from_raw(7), Child::Node(7));
    let p = Plane3d {
        normal: [0., 0., 1.],
        d: 2.0,
    };
    assert_eq!(flipped(p, true).d, -2.0);
    assert_eq!(MaterialType::from_code(7), Some(MaterialType::MetalThick));
    assert_eq!(MaterialType::from_code(28), Some(MaterialType::Water));
    assert_eq!(MaterialType::from_code(32), Some(MaterialType::HunterShield));
    assert_eq!(MaterialType::from_code(33), None);
    assert_eq!(MaterialType::from_code(-1), None);
    assert_eq!(MaterialType::ALL.len(), 33);
    assert!(MaterialType::ALL.iter().enumerate().all(|(i, t)| t.code() == i as i16));
}

#[test]
fn a_two_sided_sheet_is_hit_from_both_sides() {
    // A single vertical sheet at x = 2 in a larger room, built as two
    // leaves (x < 2 and x > 2 inside y, z in 0..2), the sheet a two-sided
    // pair of twins on plane (1, 0, 0, 2) with the leaves flagged bit 0.
    let mut b = CollisionBsp {
        planes: vec![Plane3d {
            normal: [1., 0., 0.],
            d: 2.0,
        }],
        ..Default::default()
    };
    b.nodes = vec![Node3d {
        plane: 0,
        back: 0x8000_0000u32 as i32,
        front: 0x8000_0001u32 as i32,
    }];
    b.leaves = vec![
        Leaf {
            flags: 1,
            reference_count: 1,
            first_reference: 0,
        },
        Leaf {
            flags: 1,
            reference_count: 1,
            first_reference: 1,
        },
    ];
    // Leaf 0 (behind the plane) uses the flipped plane, leaf 1 the plane.
    b.references = vec![
        Reference2d {
            plane: 0x8000_0000u32 as i32,
            root: 0x8000_0000u32 as i32,
        },
        Reference2d {
            plane: 0,
            root: 0x8000_0001u32 as i32,
        },
    ];
    b.vertices = [[2., 0., 0.], [2., 2., 0.], [2., 2., 2.], [2., 0., 2.]]
        .iter()
        .map(|p| Vertex {
            point: *p,
            first_edge: 0,
        })
        .collect();
    // Surface 1 (front plane, ccw seen from +x): v0 v1 v2 v3. Surface 0 is
    // its twin with the flipped plane.
    b.surfaces = vec![
        Surface {
            plane: 0x8000_0000u32 as i32,
            first_edge: 0,
            flags: SurfaceFlags(1),
            breakable_surface: 0,
            material: -1,
        },
        Surface {
            plane: 0,
            first_edge: 0,
            flags: SurfaceFlags(1),
            breakable_surface: 0,
            material: -1,
        },
    ];
    // Ring of surface 1: edges 0..3 run start -> end with it on the left;
    // surface 0 is on the right of the same edges (reverse ring 4..7 not
    // needed: it walks the right side through `reverse`).
    let e = |s: i32, t: i32, f: i32, r: i32| Edge {
        start: s,
        end: t,
        forward: f,
        reverse: r,
        left: 1,
        right: 0,
    };
    b.edges = vec![e(0, 1, 1, 3), e(1, 2, 2, 0), e(2, 3, 3, 1), e(3, 0, 0, 2)];
    assert_eq!(b.polygon(1).unwrap(), [0, 1, 2, 3]);
    assert_eq!(b.polygon(0).unwrap(), [1, 0, 3, 2]);

    let from_left = b.trace_ray([1., 1., 1.], [2., 0., 0.], TraceFlags::FRONT_AND_BACK);
    let h = from_left.hit.expect("hit from the back side");
    assert!(close(from_left.t, 0.5));
    // Both leaves are open (two-sided): case 3 with front enabled tests the
    // previous leaf (0), whose reference leads to surface 0, the twin facing it.
    assert_eq!((h.surface, h.plane.normal), (0, [-1., 0., 0.]));

    let from_right = b.trace_ray([3., 1., 1.], [-2., 0., 0.], TraceFlags::FRONT_AND_BACK);
    let h = from_right.hit.expect("hit from the front side");
    assert_eq!((h.surface, h.plane.normal), (1, [1., 0., 0.]));

    let ignoring = b.trace_ray(
        [1., 1., 1.],
        [2., 0., 0.],
        TraceFlags::FRONT_AND_BACK | TraceFlags::IGNORE_TWO_SIDED,
    );
    assert!(ignoring.hit.is_none());

    // Outside the polygon (z = 5): no hit.
    let miss = b.trace_ray([1., 1., 5.], [2., 0., 0.], TraceFlags::FRONT_AND_BACK);
    assert!(miss.hit.is_none());

    // A broken breakable sheet is skipped.
    for s in &mut b.surfaces {
        s.flags = SurfaceFlags(1 | 8);
    }
    let r = b.trace(
        [1., 1., 1.],
        [2., 0., 0.],
        TraceFlags::FRONT_AND_BACK,
        f32::INFINITY,
        &|_| false,
    );
    assert!(r.hit.is_some());
    let r = b.trace(
        [1., 1., 1.],
        [2., 0., 0.],
        TraceFlags::FRONT_AND_BACK,
        f32::INFINITY,
        &|i| i == 0,
    );
    assert!(r.hit.is_none());
    let r = b.trace_ray(
        [1., 1., 1.],
        [2., 0., 0.],
        TraceFlags::FRONT_AND_BACK | TraceFlags::IGNORE_BREAKABLE,
    );
    assert!(r.hit.is_none());
}

#[test]
fn queries_on_damaged_fields_do_not_panic() {
    // A BSP with damaged fields (built directly, skipping the reader's
    // checks) never panics in its queries.
    let good = room();
    let mut seed = 0x1234_5678u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for round in 0..400 {
        let mut b = good.clone();
        let junk = [0x7FFF_FFFF, -2, 99, 0x8000_0063u32 as i32, i32::MIN, 0, -1][(rnd() % 7) as usize];
        match rnd() % 9 {
            0 => b.nodes[(rnd() % 6) as usize].plane = junk,
            1 => b.nodes[(rnd() % 6) as usize].back = junk,
            2 => b.nodes[(rnd() % 6) as usize].front = junk,
            3 => b.references[(rnd() % 6) as usize].root = junk,
            4 => b.references[(rnd() % 6) as usize].plane = junk,
            5 => b.surfaces[(rnd() % 6) as usize].first_edge = junk,
            6 => b.edges[(rnd() % 12) as usize].forward = junk,
            7 => b.edges[(rnd() % 12) as usize].left = junk,
            _ => b.leaves[0].first_reference = junk,
        }
        let start = [rnd() as f32 / 1e9 - 2.0, 1.0, 1.0];
        let r = b.trace_ray(start, [2., 1., 1.], TraceFlags::FRONT_AND_BACK);
        assert!(r.leaves.len() <= 256, "round {round}");
        let _ = b.point_leaf([1., 1., 1.]);
        let _ = b.triangulate();
        for s in 0..8 {
            let _ = b.polygon_points(s);
        }
    }

    // A cyclic tree: node 0's front child is node 0.
    let mut b = good.clone();
    b.nodes[0].front = 0;
    b.nodes[0].back = 0;
    let r = b.trace_ray([1., 1., 1.], [1., 1., 1.], TraceFlags::FRONT_AND_BACK);
    assert!(r.hit.is_none());
    assert_eq!(b.point_leaf([1., 1., 1.]), None);
}

#[test]
fn bad_indices_in_the_blocks_are_refused() {
    let build = |edit: &dyn Fn(&mut [Vec<u8>; 3])| {
        map_with_bsp(0x2000, |h| {
            // nodes, leaf, surface elements with one damaged field each.
            let mut parts = [
                Struct::new(12).i32(0, 0).i32(4, -1).i32(8, 0x8000_0000u32 as i32).0,
                Struct::new(8).u16(0, 0).i16(2, 0).i32(4, 0).0,
                Struct::new(12).i32(0, 0).i32(4, 0).i16(10, -1).0,
            ];
            edit(&mut parts);
            let planes = h.block(&[Struct::new(16).f32s(0, &[0., 0., 1., 0.]).0]);
            let edge = h.block(&[Struct::new(24)
                .i32(0, 0)
                .i32(4, 0)
                .i32(8, 0)
                .i32(12, 0)
                .i32(16, 0)
                .i32(20, 0)
                .0]);
            let vert = h.block(&[Struct::new(16).0]);
            let bsp = Struct::new(0x60)
                .put(0x00, &h.block(&[parts[0].clone()]))
                .put(0x0C, &planes)
                .put(0x18, &h.block(&[parts[1].clone()]))
                .put(0x3C, &h.block(&[parts[2].clone()]))
                .put(0x48, &edge)
                .put(0x54, &vert);
            let coll = h.block(&[bsp.0]);
            Struct::new(MAIN_SIZE).put(0xB0, &coll)
        })
    };
    let ok = |m| read_level(&m, 0);
    assert!(ok(build(&|_| {})).is_ok());
    let e = ok(build(&|p| p[0][0..4].copy_from_slice(&5i32.to_le_bytes())))
        .unwrap_err()
        .to_string();
    assert!(e.contains("3d node 0: plane 5"), "{e}");
    let e = ok(build(&|p| p[0][4..8].copy_from_slice(&9i32.to_le_bytes())))
        .unwrap_err()
        .to_string();
    assert!(e.contains("child"), "{e}");
    let e = ok(build(&|p| {
        p[1][2..4].copy_from_slice(&1i16.to_le_bytes());
        p[1][4..8].copy_from_slice(&3i32.to_le_bytes())
    }))
    .unwrap_err()
    .to_string();
    assert!(e.contains("leaf 0: references"), "{e}");
    let e = ok(build(&|p| p[2][4..8].copy_from_slice(&4i32.to_le_bytes())))
        .unwrap_err()
        .to_string();
    assert!(e.contains("first edge"), "{e}");
    let e = ok(build(&|p| p[2][10..12].copy_from_slice(&3i16.to_le_bytes())))
        .unwrap_err()
        .to_string();
    assert!(e.contains("material 3 of 0"), "{e}");
}

#[test]
fn model_reading_refuses_other_classes() {
    let map = box_room_map();
    let scenario = map.scenario().clone();
    let e = read_model(&map, &scenario).unwrap_err().to_string();
    assert!(e.contains("not a collision model"), "{e}");
}
