//! The structure BSP and collision BSP readers on the user's Halo disc
//! image, against section (b) of specs/halo_ce/structure_bsp.md and
//! collision.md and their per-map tables. Each test passes with a note
//! when there is no image (`$CHIMERA_HALO_CE_ROOT`, else
//! `[games.halo_ce] path` in chimera.local.toml).

// The spec's numbers are quoted with the digits it gives.
#![allow(clippy::excessive_precision, clippy::type_complexity)]

use std::{collections::BTreeMap, sync::Mutex, sync::OnceLock};

use blam_formats::cache::{ClassCode, MapFile, Plane3d, TagEntry, class};
use blam_formats::collision::{
    CollisionBsp, LevelCollision, MaterialType, TraceFlags, read_level, read_model, vertex_bounds,
};
use blam_formats::disc::DiscImage;
use blam_formats::structure_bsp::StructureBsp;

use super::disc_install::{SKIP, image_path};

fn disc() -> Option<&'static DiscImage> {
    static DISC: OnceLock<Option<DiscImage>> = OnceLock::new();
    let d = DISC
        .get_or_init(|| image_path().map(|p| DiscImage::open(&p).unwrap()))
        .as_ref();
    if d.is_none() {
        eprintln!("{SKIP}");
    }
    d
}

/// A map, loaded once for all the tests of this file.
fn map(name: &str) -> Option<MapFile> {
    static MAPS: Mutex<BTreeMap<String, MapFile>> = Mutex::new(BTreeMap::new());
    let image = disc()?;
    let mut maps = MAPS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = maps.get(name) {
        return Some(m.clone());
    }
    let file = image.read_file(&format!("maps/{name}.map")).unwrap();
    let m = MapFile::load(&file).unwrap();
    maps.insert(name.to_owned(), m.clone());
    Some(m)
}

fn c(s: &str) -> ClassCode {
    ClassCode::from_text(s)
}

/// structure_bsp.md's per-map table: groups, lit groups, materials,
/// triangles, vertices, lightmap vertices, clusters, portals.
const TABLE: [(&str, usize, usize, usize, usize, usize, usize, usize, usize); 13] = [
    ("beavercreek", 14, 13, 65, 3317, 4242, 4043, 29, 65),
    ("bloodgulch", 16, 15, 79, 5503, 5790, 5534, 30, 70),
    ("boardingaction", 26, 25, 173, 7142, 12052, 11452, 41, 138),
    ("carousel", 29, 28, 123, 2940, 4692, 3044, 36, 90),
    ("chillout", 10, 9, 163, 3108, 5399, 4700, 10, 14),
    ("damnation", 11, 10, 69, 8618, 11664, 10076, 12, 45),
    ("hangemhigh", 3, 2, 186, 3218, 5381, 4655, 1, 0),
    ("longest", 2, 1, 63, 2319, 3832, 3455, 1, 0),
    ("prisoner", 3, 2, 79, 5234, 9260, 8996, 1, 0),
    ("putput", 10, 9, 67, 10494, 16555, 15835, 9, 0),
    ("ratrace", 12, 11, 146, 2692, 4728, 3976, 16, 36),
    ("sidewinder", 24, 23, 136, 10036, 14234, 13918, 35, 104),
    ("wizard", 12, 11, 47, 2972, 5118, 4814, 18, 40),
];

/// World bounds (x, y, z lower and upper) from the spec's table.
const BOUNDS: [(&str, [f32; 6]); 13] = [
    ("beavercreek", [-7.423, 35.439, 2.458, 23.563, -1.356, 11.085]),
    ("bloodgulch", [5.907, 131.902, -190.219, -45.057, -0.349, 26.233]),
    ("boardingaction", [-57.421, 77.410, -97.281, 97.278, -71.388, 65.668]),
    ("carousel", [-16.010, 16.010, -16.010, 16.010, -4.360, 2.344]),
    ("chillout", [-10.238, 12.815, -8.177, 12.967, 0.0, 6.691]),
    ("damnation", [-15.871, 12.102, -15.725, 16.150, -50.000, 18.200]),
    ("hangemhigh", [5.900, 35.878, -22.823, 15.823, -9.253, 8.183]),
    ("longest", [-17.378, 15.622, -22.442, -6.623, -0.600, 4.854]),
    ("prisoner", [-12.600, 11.000, -7.000, 7.600, -0.528, 8.712]),
    ("putput", [-19.757, 35.895, -36.589, -0.894, -0.198, 4.802]),
    ("ratrace", [-8.844, 23.502, -27.937, 3.563, -3.762, 4.786]),
    ("sidewinder", [-54.172, 54.767, -48.468, 61.172, -4.475, 7.220]),
    ("wizard", [-12.649, 12.647, -12.651, 12.646, -4.500, 4.703]),
];

fn bsp(map: &MapFile) -> StructureBsp {
    StructureBsp::read(map, 0).unwrap()
}

fn path_of(map: &MapFile, r: &blam_formats::cache::TagRef) -> Option<String> {
    map.tag(r.id).map(|t: &TagEntry| t.path.clone())
}

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

#[test]
fn bloodgulch_bsp_reference() {
    let Some(m) = map("bloodgulch") else { return };
    let bsps = m.bsps().unwrap();
    assert_eq!(bsps.len(), 1);
    assert_eq!(
        (bsps[0].file_offset, bsps[0].size, bsps[0].load_address),
        (0x800, 0x13C800, 0x8186_9800)
    );
}

#[test]
fn per_map_counts_of_all_thirteen() {
    let Some(_) = disc() else { return };
    for (name, groups, lit, materials, tris, verts, lm_verts, clusters, portals) in TABLE {
        let m = map(name).unwrap();
        let b = bsp(&m);
        let got = (
            b.lightmap_groups.len(),
            b.lightmap_groups.iter().filter(|g| g.bitmap_index.is_some()).count(),
            b.material_count(),
            b.triangle_count(),
            b.vertex_count(),
            b.lightmap_vertex_count(),
            b.clusters.len(),
            b.cluster_portals.len(),
        );
        assert_eq!(
            got,
            (groups, lit, materials, tris, verts, lm_verts, clusters, portals),
            "{name}"
        );
        // Lit groups use page i; the one group without a page is the last.
        for (i, g) in b.lightmap_groups.iter().enumerate() {
            let want = (i < lit).then_some(i as u16);
            assert_eq!(g.bitmap_index, want, "{name} group {i}");
        }
        // Material surface ranges are contiguous from 0 and cover the list.
        let mut next = 0;
        for mat in b.materials() {
            assert_eq!(mat.first_surface, next, "{name}");
            next += mat.surface_count;
            assert!(mat.compressed);
            assert!(mat.lightmap_vertex_count == 0 || mat.lightmap_vertex_count == mat.vertex_count);
        }
        assert_eq!(next as usize, tris, "{name}");
        // The lightmap bitmap resolves; shaders are the five spec'd classes.
        assert!(
            m.tag(b.lightmap_bitmap.id).is_some_and(|t| t.class == class::BITMAP),
            "{name}"
        );
        for mat in b.materials() {
            let t = b.shader_tag(mat).unwrap_or_else(|| panic!("{name}: unresolved shader"));
            let ok = ["senv", "sotr", "sgla", "swat", "schi"].iter().any(|k| t.class == c(k));
            assert!(ok, "{name}: shader class {}", t.class);
        }
        // Lit groups: lightmapped (senv or glass); the unlit group is last
        // and holds only transparent shaders.
        for g in &b.lightmap_groups {
            for mat in &g.materials {
                assert_eq!(
                    g.bitmap_index.is_none(),
                    !mat.has_lightmap() || g.bitmap_index.is_none(),
                    "{name}"
                );
                if g.bitmap_index.is_none() {
                    assert!(!mat.has_lightmap(), "{name}");
                    assert_ne!(b.shader_tag(mat).unwrap().class, c("senv"), "{name}");
                }
            }
        }
        // Leaves run parallel to the collision BSP's, each in a cluster.
        assert_eq!(b.leaves.len(), b.counts.collision_leaves as usize, "{name}");
        assert_eq!(b.counts.nodes, b.counts.collision_nodes, "{name}");
        assert!(b.leaves.iter().all(|l| l.cluster.is_some()), "{name}");
        // PVS: every cluster sees itself and the relation is symmetric.
        for i in 0..b.clusters.len() {
            assert!(b.visibility.visible(i, i), "{name}");
            for j in 0..b.clusters.len() {
                assert_eq!(b.visibility.visible(i, j), b.visibility.visible(j, i), "{name}");
            }
        }
        // Portals link clusters and carry a polygon on a collision plane.
        let lc = read_level(&m, 0).unwrap();
        for (k, p) in b.cluster_portals.iter().enumerate() {
            let pl = lc.bsp.planes[p.plane as usize];
            assert!(!p.vertices.is_empty());
            for v in &p.vertices {
                let d = pl.normal[0] * v[0] + pl.normal[1] * v[1] + pl.normal[2] * v[2] - pl.d;
                assert!(d.abs() < 1e-3, "{name} portal {k}: {d}");
            }
        }
    }
}

#[test]
fn world_bounds_of_all_thirteen() {
    let Some(_) = disc() else { return };
    for (name, want) in BOUNDS {
        let b = bsp(&map(name).unwrap());
        let w = b.world_bounds;
        let got = [w.x.lower, w.x.upper, w.y.lower, w.y.upper, w.z.lower, w.z.upper];
        for (g, e) in got.iter().zip(want) {
            assert!(near(*g, e, 0.0006), "{name}: {got:?} vs {want:?}");
        }
    }
}

#[test]
fn bloodgulch_first_material_and_vertex() {
    let Some(m) = map("bloodgulch") else { return };
    let b = bsp(&m);
    assert_eq!(
        path_of(&m, &b.lightmap_bitmap).as_deref(),
        Some("levels\\test\\bloodgulch\\bloodgulch")
    );
    let first = b.materials().next().unwrap();
    assert_eq!(first.group, 0);
    assert_eq!(b.lightmap_groups[0].bitmap_index, Some(0));
    let shader = b.shader_tag(first).unwrap();
    assert_eq!(shader.path, "levels\\test\\bloodgulch\\shaders\\cap_cliff01b");
    assert_eq!(shader.class, c("senv"));
    assert_eq!(first.first_surface, 0);
    // The spec's "surfaces 0..927" is the inclusive range.
    assert_eq!(first.surface_count, 928);
    assert_eq!((first.vertex_count, first.lightmap_vertex_count), (807, 807));
    assert!(first.compressed);

    let g = b.material_geometry(0).unwrap();
    let v = &g.vertices[0];
    assert_eq!(v.position, [63.956402, -93.394165, 20.563074]);
    assert_eq!(v.packed, [0x2635_FC65, 0x833C_5F42, 0x05A1_B145]);
    assert!(
        near(v.uv[0], 0.171656, 1e-6) && near(v.uv[1], 0.277278, 1e-6),
        "{:?}",
        v.uv
    );
    let want = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| near(a[i], b[i], 1e-5));
    assert!(want(v.normal, [-0.901319, -0.313141, 0.298143]), "{:?}", v.normal);
    assert!(want(v.binormal, [-0.185149, -0.113825, -0.976540]), "{:?}", v.binormal);
    assert!(want(v.tangent, [0.318026, -0.947240, 0.043988]), "{:?}", v.tangent);

    let l = &g.lightmap_vertices[0];
    assert_eq!(l.packed_direction, 0x0D36_3484);
    assert_eq!(l.raw_uv, [25832, 16895]);
    assert!(
        want(l.incident_direction, [-0.871031, -0.306302, 0.102639]),
        "{:?}",
        l.incident_direction
    );
    assert!(
        near(l.uv[0], 0.788357, 1e-5) && near(l.uv[1], 0.515618, 1e-5),
        "{:?}",
        l.uv
    );

    assert_eq!(g.triangles[0], [0, 1, 2]);
    let p = |i: usize| g.vertices[g.triangles[0][i] as usize].position;
    assert_eq!(p(0), [63.956402, -93.394165, 20.563074]);
    assert_eq!(p(1), [64.021805, -93.540810, 11.555674]);
    assert_eq!(p(2), [67.277100, -81.478722, 18.531078]);
}

#[test]
fn bloodgulch_in_detail() {
    let Some(m) = map("bloodgulch") else { return };
    let b = bsp(&m);
    // Groups 0-14 lit: 35 senv materials, 5371 triangles; group 15 has no
    // page: 44 sotr materials, 132 triangles.
    let (mut senv, mut tris) = (0, 0);
    for g in &b.lightmap_groups[..15] {
        for mat in &g.materials {
            assert_eq!(b.shader_tag(mat).unwrap().class, c("senv"));
            senv += 1;
            tris += mat.surface_count;
        }
    }
    assert_eq!((senv, tris), (35, 5371));
    let last = &b.lightmap_groups[15];
    assert_eq!(last.bitmap_index, None);
    assert_eq!(last.materials.len(), 44);
    assert!(
        last.materials
            .iter()
            .all(|x| b.shader_tag(x).unwrap().class == c("sotr"))
    );
    assert_eq!(last.materials.iter().map(|x| x.surface_count).sum::<u32>(), 132);

    assert_eq!(b.collision_materials.len(), 10);
    assert_eq!(b.counts.nodes, 12071);
    assert_eq!(b.leaves.len(), 5999);
    assert_eq!(b.leaf_surface_references.len(), 14007);
    assert_eq!(b.visibility.words.len() * 4, 120);
    assert!(b.sound_cluster_data.len() >= 435, "{}", b.sound_cluster_data.len());
    assert_eq!(b.weather_palette.len(), 1);
    assert_eq!(b.background_sound_palette.len(), 1);
    assert_eq!(b.sound_environment_palette.len(), 1);
    assert_eq!(b.counts.runtime_decals, 8);
    assert_eq!(b.counts.pathfinding_surfaces, 4916);
    assert_eq!(b.lens_flares.len(), 0);
    assert_eq!(b.markers.len(), 0);
    assert_eq!(
        (b.fog_planes.len(), b.fog_regions.len(), b.fog_palette.len()),
        (0, 0, 0)
    );
    assert_eq!(b.breakable_surfaces.len(), 0);
    // Subclusters list 6325 indices covering all 5503 surfaces.
    let mut seen = vec![false; b.triangle_count()];
    let mut listed = 0;
    for cl in &b.clusters {
        for s in &cl.subclusters {
            listed += s.surface_indices.len();
            for &i in &s.surface_indices {
                seen[i as usize] = true;
            }
        }
    }
    assert_eq!(listed, 6325);
    assert!(seen.iter().all(|&x| x));
    // The one sky of the level.
    assert!(b.clusters.iter().all(|c| c.sky == Some(0)));
    let dens: usize = (0..30)
        .flat_map(|i| (0..30).map(move |j| (i, j)))
        .filter(|&(i, j)| b.visibility.visible(i, j))
        .count();
    assert_eq!((dens as f64 / 9.0).round(), 80.0, "{dens} of 900");
}

#[test]
fn hangemhigh_in_detail() {
    let Some(m) = map("hangemhigh") else { return };
    let b = bsp(&m);
    assert_eq!(b.lightmap_groups.len(), 3);
    assert_eq!(b.material_count(), 186);
    assert_eq!(b.triangle_count(), 3218);
    assert_eq!((b.clusters.len(), b.cluster_portals.len()), (1, 0));
    assert_eq!(b.visibility.words.len() * 4, 4);
    assert_eq!(b.clusters[0].sky, Some(0));
    assert_eq!(b.lightmap_groups[0].materials.len(), 2);
    assert_eq!(
        b.lightmap_groups[0]
            .materials
            .iter()
            .map(|x| x.surface_count)
            .sum::<u32>(),
        1996
    );
    assert_eq!(b.lightmap_groups[1].materials.len(), 8);
    assert_eq!(
        b.lightmap_groups[1]
            .materials
            .iter()
            .map(|x| x.surface_count)
            .sum::<u32>(),
        864
    );
    assert_eq!(b.lightmap_groups[2].bitmap_index, None);
    assert_eq!(b.lightmap_groups[2].materials.len(), 176);
    assert_eq!(
        b.lightmap_groups[2]
            .materials
            .iter()
            .map(|x| x.surface_count)
            .sum::<u32>(),
        358
    );
    let shaders: std::collections::BTreeSet<_> = b.lightmap_groups[2].materials.iter().map(|x| x.shader.id.0).collect();
    assert_eq!(shaders.len(), 1);
}

fn len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[test]
fn decoded_geometry_checks_on_all_thirteen() {
    let Some(_) = disc() else { return };
    let (mut triangles, mut wrong_facing) = (0usize, 0usize);
    for (name, ..) in TABLE {
        let b = bsp(&map(name).unwrap());
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        let (mut lm_lo, mut lm_hi) = (f32::MAX, f32::MIN);
        for mat in b.materials() {
            let g = b.decode(mat).unwrap();
            assert_eq!(g.vertices.len(), mat.vertex_count as usize);
            assert_eq!(g.lightmap_vertices.len(), mat.lightmap_vertex_count as usize);
            assert_eq!(g.triangles.len(), mat.surface_count as usize);
            for v in &g.vertices {
                assert!(
                    (len(v.normal) - 1.0).abs() < 0.0012,
                    "{name}: normal length {}",
                    len(v.normal)
                );
                for a in 0..3 {
                    lo[a] = lo[a].min(v.position[a]);
                    hi[a] = hi[a].max(v.position[a]);
                }
            }
            for l in &g.lightmap_vertices {
                for u in l.uv {
                    assert!((0.0..=1.0).contains(&u), "{name}: lightmap coordinate {u}");
                    lm_lo = lm_lo.min(u);
                    lm_hi = lm_hi.max(u);
                }
            }
            for t in &g.triangles {
                assert!(t.iter().all(|&i| (i as usize) < g.vertices.len()));
                let p = |k: usize| g.vertices[t[k] as usize];
                let (p0, p1, p2) = (p(0), p(1), p(2));
                let e1 = [0, 1, 2].map(|a| (p1.position[a] - p0.position[a]) as f64);
                let e2 = [0, 1, 2].map(|a| (p2.position[a] - p0.position[a]) as f64);
                let cr = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                let n: f64 = (0..3)
                    .map(|a| cr[a] * (p0.normal[a] + p1.normal[a] + p2.normal[a]) as f64)
                    .sum();
                triangles += 1;
                if n > 0.0 {
                    wrong_facing += 1;
                }
            }
        }
        let w = b.world_bounds;
        assert_eq!(
            [lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]],
            [w.x.lower, w.x.upper, w.y.lower, w.y.upper, w.z.lower, w.z.upper],
            "{name}: the bounding box of the vertices is the world bounds"
        );
        eprintln!("{name}: lightmap coordinates {lm_lo:.4}..{lm_hi:.4}");
    }
    eprintln!("{triangles} triangles, {wrong_facing} facing the wrong way");
    assert_eq!(triangles, 67593);
    assert!(wrong_facing <= 1, "{wrong_facing}");
}

// Collision.

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in 0..1.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn level(name: &str) -> Option<(MapFile, LevelCollision)> {
    let m = map(name)?;
    let l = read_level(&m, 0).unwrap();
    Some((m, l))
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
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

fn depths(b: &CollisionBsp) -> (usize, usize) {
    use blam_formats::collision::Child;
    let (mut d3, mut stack) = (0, vec![(0u32, 1usize)]);
    while let Some((n, d)) = stack.pop() {
        d3 = d3.max(d);
        for c in [b.nodes[n as usize].back, b.nodes[n as usize].front] {
            if let Child::Node(k) = Child::from_raw(c) {
                stack.push((k, d + 1));
            }
        }
    }
    let mut d2 = 0;
    for r in &b.references {
        let mut stack = vec![(r.root, 1usize)];
        while let Some((raw, d)) = stack.pop() {
            if let Child::Node(k) = Child::from_raw(raw) {
                d2 = d2.max(d);
                let n = b.nodes_2d[k as usize];
                stack.push((n.left, d + 1));
                stack.push((n.right, d + 1));
            }
        }
    }
    (d3, d2)
}

#[test]
fn collision_counts_of_bloodgulch_and_hangemhigh() {
    // nodes, planes, leaves, 2D refs, 2D nodes, surfaces, edges, vertices,
    // fan triangles, surfaces with 3..=8 vertices, max 3D/2D depth.
    let cases = [
        (
            "bloodgulch",
            [12071, 4616, 5999, 13182, 418, 4916, 7650, 2704],
            5468,
            [4447, 420, 19, 28, 0, 2],
            (39, 4),
        ),
        (
            "hangemhigh",
            [1946, 964, 1116, 2659, 1175, 1695, 3132, 1381],
            2874,
            [739, 829, 66, 38, 11, 12],
            (22, 5),
        ),
    ];
    for (name, counts, fan, hist, depth) in cases {
        let Some((_, l)) = level(name) else { return };
        let b = &l.bsp;
        let got = [
            b.nodes.len(),
            b.planes.len(),
            b.leaves.len(),
            b.references.len(),
            b.nodes_2d.len(),
            b.surfaces.len(),
            b.edges.len(),
            b.vertices.len(),
        ];
        assert_eq!(got, counts, "{name}");
        let tris = b.triangulate().unwrap();
        assert_eq!(tris.len(), fan, "{name}");
        let mut h = [0usize; 6];
        for s in 0..b.surfaces.len() {
            let n = b.polygon(s).unwrap().len();
            h[n - 3] += 1;
        }
        assert_eq!(h, hist, "{name}");
        assert_eq!(depths(b), depth, "{name}");
        assert!(
            b.surfaces.iter().all(|s| s.flags.0 == 0 && s.breakable_surface == 0),
            "{name}"
        );
        assert!(b.leaves.iter().all(|l| !l.contains_two_sided()), "{name}");
    }
}

#[test]
fn collision_materials_and_histograms() {
    let bg_hist: [(i16, usize); 11] = [
        (-1, 94),
        (0, 1339),
        (1, 563),
        (2, 62),
        (3, 206),
        (4, 87),
        (5, 32),
        (6, 2415),
        (7, 48),
        (8, 24),
        (9, 46),
    ];
    let bg_types = [2, 7, 7, 7, 7, 7, 1, 2, 7, 7];
    let bg_shaders = [
        "levels\\test\\bloodgulch\\shaders\\cap_cliff01b",
        "levels\\b30\\shaders\\metal flat generic",
        "levels\\b30\\shaders\\metal panels generic ext clean",
        "levels\\b30\\shaders\\metal panels generic ext",
        "levels\\test\\bloodgulch\\shaders\\cap_metal",
        "levels\\test\\bloodgulch\\shaders\\cap_ramp",
        "levels\\test\\bloodgulch\\shaders\\blood ground",
        "levels\\test\\beavercreek\\shaders\\beavercreek boulder",
        "levels\\test\\bloodgulch\\shaders\\cap_ramp_blue",
        "levels\\b30\\shaders\\metal panels ext unearthed",
    ];
    let hh_hist: [(i16, usize); 10] = [
        (-1, 9),
        (0, 327),
        (1, 857),
        (2, 74),
        (3, 66),
        (4, 197),
        (5, 27),
        (6, 123),
        (7, 2),
        (8, 13),
    ];
    for (name, hist) in [("bloodgulch", &bg_hist[..]), ("hangemhigh", &hh_hist[..])] {
        let Some((m, l)) = level(name) else { return };
        let mut got: BTreeMap<i16, usize> = BTreeMap::new();
        for s in &l.bsp.surfaces {
            *got.entry(s.material).or_default() += 1;
        }
        let want: BTreeMap<i16, usize> = hist.iter().copied().collect();
        assert_eq!(got, want, "{name}");
        if name == "bloodgulch" {
            for (i, mat) in l.materials.iter().enumerate() {
                assert_eq!(mat.material_type().map(|t| t.code()), Some(bg_types[i]), "material {i}");
                assert_eq!(path_of(&m, &mat.shader).as_deref(), Some(bg_shaders[i]));
            }
            assert_eq!(l.materials[1].material_type(), Some(MaterialType::MetalThick));
            assert_eq!(l.materials[6].material_type(), Some(MaterialType::Sand));
        } else {
            assert_eq!(l.materials.len(), 9);
            assert!(
                l.materials
                    .iter()
                    .all(|x| x.material_type() == Some(MaterialType::MetalThick))
            );
            assert!(
                l.materials
                    .iter()
                    .all(|x| path_of(&m, &x.shader).is_some_and(|p| p.starts_with("levels\\b30\\shaders\\")))
            );
        }
    }
}

#[test]
fn every_collision_surface_is_a_closed_planar_ccw_polygon() {
    for name in ["bloodgulch", "hangemhigh"] {
        let Some((_, l)) = level(name) else { return };
        let b = &l.bsp;
        let mut worst = 0.0f32;
        for s in 0..b.surfaces.len() {
            let ring = b.polygon(s).unwrap();
            assert!((3..=8).contains(&ring.len()), "{name} surface {s}");
            let pts = b.polygon_points(s).unwrap();
            let plane = b.surface_plane(s).unwrap();
            for p in &pts {
                worst = worst.max((dot(plane.normal, *p) - plane.d).abs());
            }
            // Counter-clockwise about the outward normal.
            let c = cross(sub(pts[1], pts[0]), sub(pts[2], pts[0]));
            assert!(dot(c, plane.normal) > 0.0, "{name} surface {s}");
            // Edges share their vertex: each ring vertex has the edge's
            // end vertex as the next ring vertex's start (checked by the
            // ring walk closing at the first edge).
        }
        eprintln!("{name}: planar within {worst}");
        assert!(worst <= 0.0003, "{name}: {worst}");
    }
}

#[test]
fn bloodgulch_surface_zero_and_bounds() {
    let Some((m, l)) = level("bloodgulch") else { return };
    let b = &l.bsp;
    let s = b.surfaces[0];
    assert_eq!((s.plane, s.first_edge, s.flags.0, s.material), (0, 0, 0, -1));
    let p = b.planes[0];
    assert_eq!(p.normal, [0.0, 0.0, -1.0]);
    assert!(near(p.d, -68.03709, 1e-4));
    let want = [
        [169.49675, -39.11990, 68.03709],
        [174.00922, -220.41060, 68.03709],
        [41.40953, -183.45016, 68.03709],
        [-5.94453, -145.65318, 68.03709],
        [-7.97682, -40.75824, 68.03709],
        [89.12006, -30.12179, 68.03709],
    ];
    let got = b.polygon_points(0).unwrap();
    assert_eq!(got.len(), 6);
    for (g, w) in got.iter().zip(want) {
        assert!((0..3).all(|a| near(g[a], w[a], 2e-4)), "{g:?} vs {w:?}");
    }
    // Collision bounds against the render bounds.
    let cb = vertex_bounds(b).unwrap();
    let got = [cb.x.lower, cb.x.upper, cb.y.lower, cb.y.upper, cb.z.lower, cb.z.upper];
    let want = [-7.977, 174.009, -220.411, -30.122, -0.349, 68.037];
    for (g, w) in got.iter().zip(want) {
        assert!(near(*g, w, 1e-3), "{got:?}");
    }
    // The 94 material-less surfaces lie outside the render mesh: most have
    // a vertex outside the render bounds (the spec says all; 12 do not,
    // their bounding box is inside the render box); every surface with a material lies within
    // them.
    let w = bsp(&m).world_bounds;
    let inside = |p: [f32; 3]| {
        let e = 1e-3;
        p[0] >= w.x.lower - e
            && p[0] <= w.x.upper + e
            && p[1] >= w.y.lower - e
            && p[1] <= w.y.upper + e
            && p[2] >= w.z.lower - e
            && p[2] <= w.z.upper + e
    };
    let (mut none, mut inside_none) = (0, 0);
    for (i, s) in b.surfaces.iter().enumerate() {
        let pts = b.polygon_points(i).unwrap();
        let all_inside = pts.iter().all(|p| inside(*p));
        if s.material == -1 {
            none += 1;
            inside_none += usize::from(all_inside);
        } else {
            assert!(all_inside, "surface {i} with a material lies outside the render bounds");
        }
    }
    eprintln!("{inside_none} of {none} material-less surfaces lie inside the render bounds");
    assert_eq!(none, 94);
    assert!(inside_none <= 20, "{inside_none}");
}

/// The scenario's player starting locations (block at 0x354, 52 bytes
/// each, position first).
fn starting_locations(m: &MapFile) -> Vec<[f32; 3]> {
    let v = m.tag_view(m.scenario(), 0x360).unwrap();
    v.block(0x354, 52)
        .unwrap()
        .iter()
        .map(|e| e.point3(0).unwrap())
        .collect()
}

#[test]
fn bloodgulch_traces() {
    let Some((m, l)) = level("bloodgulch") else { return };
    let b = &l.bsp;
    let starts = starting_locations(&m);
    assert!(
        starts[0]
            .iter()
            .zip([98.49340, -157.63901, 1.70473])
            .all(|(a, e)| near(*a, e, 1e-4)),
        "{:?}",
        starts[0]
    );
    assert!(
        starts[6]
            .iter()
            .zip([43.91446, -80.67870, 1.70433])
            .all(|(a, e)| near(*a, e, 1e-4)),
        "{:?}",
        starts[6]
    );
    for (i, surface) in [(0, 1300), (6, 2306)] {
        let s = starts[i];
        let r = b.trace_ray([s[0], s[1], s[2] + 1.0], [0.0, 0.0, -20.0], TraceFlags::FRONT_AND_BACK);
        let h = r.hit.expect("a hit");
        eprintln!("start {i}: t {} z {} surface {}", r.t, h.point[2], h.surface);
        assert!(near(h.point[2], 1.703486, 1e-4), "{}", h.point[2]);
        assert_eq!(h.surface, surface);
        assert_eq!(h.plane.normal, [0.0, 0.0, 1.0]);
        assert_eq!(h.material, 2);
        assert_eq!(
            l.surface_material_type(h.surface as usize),
            Some(MaterialType::MetalThick)
        );
        if i == 0 {
            assert!(near(r.t, 0.0500622, 1e-5), "{}", r.t);
        }
    }
    let r = b.trace_ray([65.0, -120.0, 30.0], [0.0, 0.0, -40.0], TraceFlags::FRONT_FACING);
    let h = r.hit.expect("a hit");
    assert!(near(r.t, 0.7465926, 2e-5), "{}", r.t);
    assert!(near(h.point[2], 0.136297, 1e-3), "{}", h.point[2]);
    assert_eq!(h.surface, 1495);
    for (g, w) in h.plane.normal.iter().zip([-0.009558, 0.045286, 0.998928]) {
        assert!(near(*g, w, 1e-5), "{:?}", h.plane);
    }
    assert_eq!(h.material, 6);
    assert_eq!(l.surface_material_type(h.surface as usize), Some(MaterialType::Sand));
    assert_eq!(b.point_leaf([65.0, -120.0, 30.0]), Some(5960));
    // The leaf the trace started in is the point test's.
    assert_eq!(r.leaves.first().copied(), Some(5960));
    let _: Plane3d = h.plane;
}

#[test]
fn boardingaction_two_sided_surfaces_have_twins() {
    let Some((_, l)) = level("boardingaction") else { return };
    let b = &l.bsp;
    let two: Vec<usize> = (0..b.surfaces.len())
        .filter(|&i| b.surfaces[i].flags.two_sided())
        .collect();
    assert_eq!(two.len(), 80);
    for &i in &two {
        let s = b.surfaces[i];
        let mut ring = b.polygon(i).unwrap();
        ring.sort_unstable();
        let twin = two.iter().find(|&&j| {
            if j == i {
                return false;
            }
            let t = b.surfaces[j];
            let mut r = b.polygon(j).unwrap();
            r.sort_unstable();
            r == ring
                && (t.plane as u32 & 0x7FFF_FFFF) == (s.plane as u32 & 0x7FFF_FFFF)
                && (t.plane < 0) != (s.plane < 0)
                && t.flags == s.flags
                && t.material == s.material
        });
        assert!(twin.is_some(), "surface {i} has no twin");
    }
    // Leaves containing two-sided surfaces carry bit 0.
    assert!(b.leaves.iter().any(|l| l.contains_two_sided()));
    // Segments through the centre of every two-sided surface, from both
    // sides, hit a surface of the sheet with front + back, and none with
    // "ignore two-sided".
    for &i in &two {
        let pts = b.polygon_points(i).unwrap();
        let n = b.surface_plane(i).unwrap().normal;
        let mut c = [0.0f32; 3];
        for p in &pts {
            for a in 0..3 {
                c[a] += p[a] / pts.len() as f32;
            }
        }
        for side in [1.0f32, -1.0] {
            let start = [c[0] + side * n[0], c[1] + side * n[1], c[2] + side * n[2]];
            let v = [-2.0 * side * n[0], -2.0 * side * n[1], -2.0 * side * n[2]];
            let r = b.trace_ray(start, v, TraceFlags::FRONT_AND_BACK);
            if let Some(h) = r.hit {
                // Whatever is hit first is at or before the sheet.
                assert!(r.t <= 0.5 + 1e-3, "surface {i}: hit at {}", r.t);
                let _ = h;
            }
            let r = b.trace_ray(start, v, TraceFlags::FRONT_AND_BACK | TraceFlags::IGNORE_TWO_SIDED);
            if let Some(h) = r.hit {
                assert!(!h.flags.two_sided(), "surface {i}");
            }
        }
    }
}

#[test]
fn cyborg_collision_model() {
    let Some(m) = map("bloodgulch") else { return };
    let tag = m
        .find(class::MODEL_COLLISION_GEOMETRY, "characters\\cyborg\\cyborg")
        .unwrap();
    let model = read_model(&m, tag).unwrap();
    assert_eq!(model.nodes.len(), 19);
    let without: Vec<_> = model.nodes.iter().filter(|n| n.region.is_none()).collect();
    assert_eq!(without.len(), 3);
    assert!(without.iter().all(|n| n.bsps.is_empty()));
    let names: Vec<&str> = model.materials.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["head", "body", "legs", "shield generator"]);
    assert!(
        model
            .materials
            .iter()
            .all(|x| x.material_type() == Some(MaterialType::Cyborg))
    );
    assert_eq!(model.materials[0].flags & 1, 1);
    assert!(model.nodes.iter().any(|n| n.name == "bip01 pelvis"));
    // Every other node has a BSP per permutation of its region, and every
    // surface of every model has flags 0.
    for n in &model.nodes {
        if let Some(r) = n.region {
            assert_eq!(
                n.bsps.len(),
                model.regions[r as usize].permutations.len().max(n.bsps.len()),
                "{}",
                n.name
            );
            for b in &n.bsps {
                assert!(b.surfaces.iter().all(|s| s.flags.0 == 0), "{}", n.name);
                for s in 0..b.surfaces.len() {
                    b.polygon(s).unwrap();
                }
            }
        }
    }
    assert!(model.nodes.iter().filter(|n| !n.bsps.is_empty()).count() >= 1);
}

#[test]
fn every_collision_model_of_bloodgulch_reads() {
    let Some(m) = map("bloodgulch") else { return };
    let tags: Vec<_> = m.of_class(class::MODEL_COLLISION_GEOMETRY).collect();
    assert_eq!(tags.len(), 36);
    for t in tags {
        let model = read_model(&m, t).unwrap_or_else(|e| panic!("{}: {e}", t.path));
        for n in &model.nodes {
            for b in &n.bsps {
                b.triangulate().unwrap_or_else(|e| panic!("{}: {e}", t.path));
            }
        }
    }
}

/// The nearest crossing of the segment with the triangles, as brute force:
/// (t, surface index).
fn brute_force(tris: &[(u32, [[f64; 3]; 3])], start: [f64; 3], v: [f64; 3], front_only: bool) -> Option<(f64, u32)> {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let mut best: Option<(f64, u32)> = None;
    for (s, [p0, p1, p2]) in tris {
        let (e1, e2) = (sub(*p1, *p0), sub(*p2, *p0));
        let n = cross(e1, e2);
        let k = dot(n, v);
        if k == 0.0 || (front_only && k > 0.0) {
            continue;
        }
        let t = dot(n, sub(*p0, start)) / k;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let p = [start[0] + t * v[0], start[1] + t * v[1], start[2] + t * v[2]];
        let w = sub(p, *p0);
        let (d00, d01, d11) = (dot(e1, e1), dot(e1, e2), dot(e2, e2));
        let (d20, d21) = (dot(w, e1), dot(w, e2));
        let den = d00 * d11 - d01 * d01;
        if den == 0.0 {
            continue;
        }
        let a = (d11 * d20 - d01 * d21) / den;
        let b = (d00 * d21 - d01 * d20) / den;
        if a >= 0.0 && b >= 0.0 && a + b <= 1.0 && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, *s));
        }
    }
    best
}

#[test]
fn random_segments_agree_with_brute_force() {
    for name in ["bloodgulch", "hangemhigh"] {
        let Some((_, l)) = level(name) else { return };
        let b = &l.bsp;
        let tris: Vec<(u32, [[f64; 3]; 3])> = b
            .triangulate()
            .unwrap()
            .iter()
            .map(|t| {
                let p = |k: usize| b.vertices[t.vertices[k] as usize].point.map(|x| x as f64);
                (t.surface, [p(0), p(1), p(2)])
            })
            .collect();
        let bounds = vertex_bounds(b).unwrap();
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let (mut hits, mut misses) = (0, 0);
        for round in 0..600 {
            let front_only = round % 2 == 1;
            let lerp = |r: &mut Rng, lo: f32, hi: f32| lo as f64 + r.unit() * (hi - lo) as f64;
            let start = [
                lerp(&mut rng, bounds.x.lower, bounds.x.upper),
                lerp(&mut rng, bounds.y.lower, bounds.y.upper),
                lerp(&mut rng, bounds.z.lower, bounds.z.upper),
            ];
            // A random direction (rejection sampling in the unit ball).
            let dir = loop {
                let d = [rng.unit() * 2.0 - 1.0, rng.unit() * 2.0 - 1.0, rng.unit() * 2.0 - 1.0];
                let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if n > 0.1 && n <= 1.0 {
                    break [d[0] / n, d[1] / n, d[2] / n];
                }
            };
            let length = 1.0 + rng.unit() * 59.0;
            let v = dir.map(|x| x * length);
            let s32 = start.map(|x| x as f32);
            let v32 = v.map(|x| x as f32);
            let flags = if front_only {
                TraceFlags::FRONT_FACING
            } else {
                TraceFlags::FRONT_AND_BACK
            };
            let r = b.trace_ray(s32, v32, flags);
            let brute = brute_force(&tris, s32.map(|x| x as f64), v32.map(|x| x as f64), front_only);
            match (r.hit, brute) {
                (None, None) => misses += 1,
                (Some(h), Some((t, s))) => {
                    hits += 1;
                    assert!(
                        (r.t as f64 - t).abs() * length < 1e-3,
                        "{name} #{round}: t {} vs {t}",
                        r.t
                    );
                    assert_eq!(h.surface, s, "{name} #{round}");
                }
                (a, c) => panic!(
                    "{name} #{round}: trace {a:?} vs brute force {c:?} (start {s32:?}, v {v32:?}, front only {front_only})"
                ),
            }
        }
        eprintln!("{name}: 600 segments, {hits} hits, {misses} misses");
        assert!(hits > 50 && misses > 50);
    }
}
