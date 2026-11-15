//! The model reader on the user's Halo disc image, against section (b) of
//! specs/halo_ce/models.md: the counts of the Spartan and the assault
//! rifle (nodes, parts, vertices, triangles), the Spartan's first vertex
//! and strip, its bind pose from the composed node transforms, the markers
//! and the winding, a sweep over every model tag of bloodgulch.map and
//! hangemhigh.map, and the per-map model and part counts. Each test passes
//! with a note when there is no image.

use std::{sync::OnceLock, time::Instant};

use blam_formats::cache::{MapFile, TagEntry, class};
use blam_formats::disc::DiscImage;
use blam_formats::model::{DETAIL_LEVELS, Model, Transform};

use super::disc_install::{SKIP, image_path};

fn load(name: &str) -> Option<MapFile> {
    let path = image_path().or_else(|| {
        eprintln!("{SKIP}");
        None
    })?;
    let disc = DiscImage::open(&path).unwrap();
    Some(MapFile::load(&disc.read_file(&format!("maps/{name}.map")).unwrap()).unwrap())
}

fn bloodgulch() -> Option<&'static MapFile> {
    static MAP: OnceLock<Option<MapFile>> = OnceLock::new();
    MAP.get_or_init(|| load("bloodgulch")).as_ref()
}

fn entry<'a>(map: &'a MapFile, path: &str) -> &'a TagEntry {
    map.find(class::MODEL, path)
        .unwrap_or_else(|| panic!("no model `{path}`"))
}

fn model(map: &MapFile, path: &str) -> Model {
    Model::read(map, entry(map, path)).unwrap()
}

const SPARTAN: &str = "characters\\cyborg\\cyborg";
const RIFLE: &str = "weapons\\assault rifle\\assault rifle";
const RIFLE_FP: &str = "weapons\\assault rifle\\fp\\fp";

fn close(a: f32, b: f32, eps: f32, what: &str) {
    assert!((a - b).abs() <= eps, "{what}: {a} vs {b}");
}

fn close3(a: [f32; 3], b: [f32; 3], eps: f32, what: &str) {
    for k in 0..3 {
        close(a[k], b[k], eps, what);
    }
}

/// The bounding box of the highest detail's vertices.
fn bounds(m: &Model) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in m.highest_detail_parts() {
        for v in &p.part.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v.position[k]);
                hi[k] = hi[k].max(v.position[k]);
            }
        }
    }
    (lo, hi)
}

#[test]
fn bloodgulch_counts() {
    let Some(map) = bloodgulch() else { return };
    let tags: Vec<_> = map.of_class(class::MODEL).collect();
    assert_eq!(tags.len(), 63);
    let parts: usize = tags
        .iter()
        .map(|t| {
            let m = Model::read(map, t).unwrap();
            m.geometries.iter().map(|g| g.parts.len()).sum::<usize>()
        })
        .sum();
    assert_eq!(parts, 359);
    assert_eq!(map.model_vertex_buffers().len(), 359);
    assert_eq!(map.model_index_buffers().len(), 359);
}

#[test]
fn spartan_fields() {
    let Some(map) = bloodgulch() else { return };
    let tag = entry(map, SPARTAN);
    assert_eq!((tag.index(), tag.address), (151, 0x8072_9B64));
    let m = Model::read(map, tag).unwrap();
    assert_eq!((m.flags, m.node_list_checksum), (0, 0x6CFA_6498));
    assert_eq!(m.detail_cutoffs, [5.0, 50.0, 120.0, 250.0, 400.0]);
    assert_eq!(m.node_counts, [18; 5]);
    assert_eq!((m.base_map_u_scale, m.base_map_v_scale), (1.0, 1.0));
    assert_eq!(
        (
            m.markers.len(),
            m.nodes.len(),
            m.regions.len(),
            m.geometries.len(),
            m.shaders.len()
        ),
        (8, 19, 1, 5, 4)
    );
    let r = &m.regions[0];
    assert_eq!((r.name.as_str(), r.permutations.len()), ("cyborg", 1));
    let p = &r.permutations[0];
    assert_eq!((p.name.as_str(), p.flags), ("__base", 0));
    assert_eq!(p.geometry_indices, [Some(4), Some(3), Some(2), Some(1), Some(0)]);
    assert_eq!(p.highest_detail(), Some(0));
    assert!(p.markers.is_empty());
    let shaders: Vec<_> = m.shaders.iter().map(|s| s.resolve(map).unwrap().path.clone()).collect();
    assert_eq!(shaders[0], "characters\\cyborg\\shaders\\light shield");
    assert_eq!(shaders[3], "characters\\cyborg\\shaders\\visor");
    for l in 0..DETAIL_LEVELS {
        let g = p.geometry_at(l).unwrap();
        let total: usize = m.geometries[g].parts.iter().map(|p| p.vertices.len()).sum();
        assert_eq!(total, [507, 636, 940, 1686, 2256][l], "level {l}");
    }
}

#[test]
fn spartan_geometry() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, SPARTAN);
    let g = &m.geometries[0];
    assert_eq!(g.parts.len(), 4);
    let v: Vec<_> = g.parts.iter().map(|p| p.vertices.len()).collect();
    let s: Vec<_> = g.parts.iter().map(|p| p.strip.len()).collect();
    let t: Vec<_> = g.parts.iter().map(|p| p.triangles.len()).collect();
    assert_eq!(
        (v, s, t),
        (vec![60, 4, 2123, 69], vec![88, 4, 3653, 122], vec![30, 2, 1880, 104])
    );
    assert_eq!(g.parts.iter().map(|p| p.vertices.len()).sum::<usize>(), 2256);
    assert_eq!(g.parts.iter().map(|p| p.strip.len()).sum::<usize>(), 3867);
    assert_eq!(g.parts.iter().map(|p| p.triangles.len()).sum::<usize>(), 2016);
    let shaders: Vec<_> = g.parts.iter().map(|p| p.shader).collect();
    assert_eq!(shaders, [Some(0), Some(1), Some(2), Some(3)]);
    // The default selection is the same four parts.
    let d = m.highest_detail_parts();
    assert_eq!(d.len(), 4);
    assert!(d.iter().all(|p| p.geometry == 0 && p.region == 0));
    // The centroids of the first and the last parts.
    let p0 = &g.parts[0];
    assert_eq!((p0.previous_filthy_part, p0.next_filthy_part), (None, None));
    assert_eq!((p0.centroid_primary_node, p0.centroid_secondary_node), (6, 1));
    close(p0.centroid_primary_weight, 0.8182, 1e-4, "w6");
    close(p0.centroid_secondary_weight, 0.1818, 1e-4, "w1");
    assert_eq!(
        (g.parts[3].centroid_primary_node, g.parts[3].centroid_secondary_node),
        (12, 0)
    );
    // The bounding box.
    let (lo, hi) = bounds(&m);
    close3(lo, [-0.1046, -0.1491, 0.0], 1e-4, "box low");
    close3(hi, [0.0796, 0.1491, 0.7036], 1e-4, "box high");
}

#[test]
fn spartan_first_vertex_and_strip() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, SPARTAN);
    let p0 = &m.geometries[0].parts[0];
    // The buffer records and the data they name.
    let th = map.tag_header();
    assert_eq!(th.vertex_buffer_address + 313 * 12, 0x803C_7220);
    assert_eq!(th.index_buffer_address + 313 * 12, 0x8064_952C);
    assert_eq!(map.model_vertex_buffers()[313], 0x8062_B260);
    assert_eq!(map.model_index_buffers()[313], 0x8068_8FF8);
    let raw = map.view(blam_formats::cache::SpaceId::Tags, 0x8062_B260, 32).unwrap();
    let want: [u8; 32] = [
        0x50, 0xFA, 0xC2, 0xBB, 0xC0, 0x0F, 0x04, 0x3E, 0x64, 0x5B, 0x08, 0x3F, 0xCF, 0xCF, 0xDE, 0x21, 0x7F, 0x98,
        0xC8, 0x85, 0x09, 0x9C, 0xFF, 0xEE, 0x64, 0x70, 0x30, 0x04, 0x27, 0xFD, 0xFF, 0x7F,
    ];
    assert_eq!(raw.bytes(), want);
    let v = &p0.vertices[0];
    close3(v.position, [-0.0059502, 0.1289663, 0.5326445], 1e-6, "position");
    close3(v.normal, [-0.047386, 0.962872, 0.264907], 1e-6, "normal");
    assert_eq!(i16::from_le_bytes([0x64, 0x70]), 28772);
    assert_eq!(i16::from_le_bytes([0x30, 0x04]), 1072);
    close(v.texcoord[0], 0.878080, 1e-6, "u");
    close(v.texcoord[1], 0.032731, 1e-6, "v");
    assert_eq!((v.node0, v.node1, v.weight0), (13, None, 1.0));
    assert_eq!(m.nodes[13].name, "bip01 l upperarm");
    assert_eq!(p0.strip.len(), 88);
    assert_eq!(p0.strip[..6], [0, 1, 2, 3, 3, 4]);
}

#[test]
fn spartan_skeleton() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, SPARTAN);
    let n0 = &m.nodes[0];
    assert_eq!(n0.name, "bip01 pelvis");
    assert_eq!((n0.next_sibling, n0.first_child, n0.parent), (None, Some(2), None));
    close3(n0.translation, [-0.0082, 0.0, 0.3944], 5e-5, "translation");
    for (g, w) in n0.rotation.iter().zip([0.5; 4]) {
        close(*g, w, 5e-5, "rotation");
    }
    let w = m.bind_pose();
    let by_name = |name: &str| w[m.nodes.iter().position(|n| n.name == name).unwrap()].position;
    close(by_name("bip01 l thigh")[1], 0.0422, 5e-4, "left thigh y");
    close3(by_name("bip01 head"), [-0.007, 0.0, 0.609], 5e-4, "head");
    close3(by_name("bip01 l hand"), [0.009, 0.133, 0.352], 5e-4, "left hand");
    assert!(by_name("bip01 l hand")[1] > 0.0 && by_name("bip01 r hand")[1] < 0.0);
    // Every child is at its stored distance from its parent, and the stored
    // inverse matrices are the inverses of the composed transforms.
    for (i, n) in m.nodes.iter().enumerate() {
        if let Some(p) = n.parent {
            let d = (0..3)
                .map(|k| (w[i].position[k] - w[p].position[k]).powi(2))
                .sum::<f32>()
                .sqrt();
            close(d, n.distance_from_parent, 1e-5, &n.name);
        }
        let diff = w[i].inverse().max_difference(&n.inverse);
        assert!(diff < 1e-5, "{}: inverse differs by {diff}", n.name);
        assert!(
            w[i].then(&n.inverse).max_difference(&Transform::IDENTITY) < 1e-5,
            "{}",
            n.name
        );
    }
}

#[test]
fn spartan_height_from_the_bind_pose() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, SPARTAN);
    let w = m.bind_pose();
    let (lo, hi) = bounds(&m);
    // 0.70 WU tall: feet on z = 0, the head node (0.609) well below the top.
    close(hi[2] - lo[2], 0.7036, 1e-4, "height");
    let head = w[m.nodes.iter().position(|n| n.name == "bip01 head").unwrap()].position[2];
    assert!(head < hi[2] && head > 0.5 * hi[2]);
    // Rigidly bound vertices lie near their node's bind-pose position (they
    // are in model space already): 0.05 to 0.12 WU on average.
    let mut sum = 0.0;
    let mut n = 0.0;
    for p in m.highest_detail_parts() {
        for v in &p.part.vertices {
            if v.node1.is_none() && v.node0 == 12 {
                let q = w[12].position;
                sum += (0..3).map(|k| (v.position[k] - q[k]).powi(2)).sum::<f32>().sqrt();
                n += 1.0;
            }
        }
    }
    assert!(
        n > 0.0 && sum / n < 0.2,
        "head vertices average {} from the head node",
        sum / n
    );
}

#[test]
fn spartan_markers() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, SPARTAN);
    let names: Vec<_> = m.markers.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "antenna",
            "body",
            "flashlight",
            "head",
            "left foot",
            "left hand",
            "right foot",
            "right hand"
        ]
    );
    let w = m.bind_pose();
    let at = |name: &str| {
        let k = m.find_marker(name).unwrap();
        assert_eq!(k.instances.len(), 1);
        m.marker_world(&k.instances[0], &w).unwrap()
    };
    // The game's convention: markers face +X and the feet are at +-Y.
    let head = at("HEAD");
    close3(head.position, [-0.0031, 0.0, 0.6486], 5e-4, "head");
    close3(head.forward, [1.0, 0.0, 0.0], 0.02, "head facing");
    close3(at("left foot").position, [0.0145, 0.063, 0.0037], 5e-4, "left foot");
    close3(at("right foot").position, [0.0145, -0.0626, 0.0037], 5e-4, "right foot");
    close3(at("left foot").forward, [1.0, 0.0, 0.0], 0.02, "foot facing");
    assert_eq!(
        m.markers[m.markers.iter().position(|k| k.name == "head").unwrap()].instances[0].node,
        12
    );
}

#[test]
fn assault_rifle() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, RIFLE);
    assert_eq!(
        (
            m.nodes.len(),
            m.markers.len(),
            m.regions.len(),
            m.geometries.len(),
            m.shaders.len()
        ),
        (3, 9, 1, 3, 9)
    );
    assert_eq!(m.detail_cutoffs, [5.0, 10.0, 20.0, 40.0, 80.0]);
    assert_eq!(m.node_counts, [0, 0, 0, 0, 2]);
    let r = &m.regions[0];
    assert_eq!(
        (r.name.as_str(), r.permutations[0].name.as_str()),
        ("__unnamed", "__base")
    );
    assert_eq!(
        r.permutations[0].geometry_indices,
        [Some(2), Some(2), Some(2), Some(1), Some(0)]
    );
    let g = &m.geometries[0];
    assert_eq!(g.parts.len(), 8);
    assert_eq!(g.parts.iter().map(|p| p.vertices.len()).sum::<usize>(), 819);
    assert_eq!(g.parts.iter().map(|p| p.strip.len()).sum::<usize>(), 1371);
    assert_eq!(g.parts.iter().map(|p| p.triangles.len()).sum::<usize>(), 764);
    let counts: Vec<_> = g.parts.iter().map(|p| p.vertices.len()).collect();
    assert_eq!(counts, [376, 408, 9, 6, 4, 6, 4, 6]);
    let (lo, hi) = bounds(&m);
    close3(lo, [-0.0895, -0.0147, -0.0252], 1e-4, "box low");
    close3(hi, [0.1991, 0.0147, 0.0821], 1e-4, "box high");
    // The muzzle marker.
    let k = m.find_marker("primary trigger").unwrap();
    let i = &k.instances[0];
    assert_eq!(i.node, 0);
    close3(i.translation, [0.2197, 0.0, 0.0525], 5e-5, "translation");
    close3(
        [i.rotation[0], i.rotation[1], i.rotation[2]],
        [0.0; 3],
        1e-4,
        "rotation",
    );
    close(i.rotation[3].abs(), 1.0, 1e-4, "rotation w");
    // The second `numbers` reference has the permutation 1.
    assert_eq!(m.shaders.iter().filter(|s| s.permutation == 1).count(), 1);
}

#[test]
fn assault_rifle_first_person() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, RIFLE_FP);
    assert_eq!(
        (m.nodes.len(), m.geometries.len(), m.geometries[0].parts.len()),
        (5, 1, 8)
    );
    assert_eq!(
        m.geometries[0].parts.iter().map(|p| p.vertices.len()).sum::<usize>(),
        1713
    );
    assert_eq!(m.geometries[0].parts.iter().map(|p| p.strip.len()).sum::<usize>(), 3116);
    assert_eq!(
        m.geometries[0].parts.iter().map(|p| p.triangles.len()).sum::<usize>(),
        1964
    );
    assert_eq!(m.detail_cutoffs, [0.0; 5]);
    assert_eq!(m.level_for_size(0.0), Some(4));
}

/// A model's non-degenerate triangles that agree and disagree with the
/// average of their vertex normals (clockwise seen from the front: the
/// cross product of the game's winding points to the back).
fn winding(m: &Model) -> (usize, usize) {
    let (mut agree, mut disagree) = (0, 0);
    for g in &m.geometries {
        for p in &g.parts {
            for t in &p.triangles {
                let v = |i: u16| &p.vertices[i as usize];
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                let e1: Vec<f32> = (0..3).map(|k| b.position[k] - a.position[k]).collect();
                let e2: Vec<f32> = (0..3).map(|k| c.position[k] - a.position[k]).collect();
                let cross = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                let dot: f32 = (0..3)
                    .map(|k| cross[k] * (a.normal[k] + b.normal[k] + c.normal[k]))
                    .sum();
                if dot.abs() < 1e-12 {
                    continue; // flat to rounding
                }
                if dot < 0.0 {
                    agree += 1;
                } else {
                    disagree += 1;
                }
            }
        }
    }
    (agree, disagree)
}

/// Every model of a map: reads, every index in range, unit normals, node
/// indices below the node count, weights in 0..1; returns (models, parts,
/// triangles agreeing with their normals, disagreeing).
fn sweep(map: &MapFile) -> (usize, usize, usize, usize) {
    let t0 = Instant::now();
    let (mut models, mut parts, mut vertices, mut agree, mut disagree) = (0, 0, 0usize, 0, 0);
    for tag in map.of_class(class::MODEL) {
        let m = Model::read(map, tag).unwrap_or_else(|e| panic!("{}: {e}", tag.path));
        models += 1;
        for g in &m.geometries {
            for (pi, p) in g.parts.iter().enumerate() {
                let what = format!("{} part {pi}", tag.path);
                parts += 1;
                vertices += p.vertices.len();
                assert!(
                    p.strip.iter().all(|&k| (k as usize) < p.vertices.len()),
                    "{what}: strip index"
                );
                assert!(
                    p.triangles.iter().flatten().all(|&k| (k as usize) < p.vertices.len()),
                    "{what}: triangle index"
                );
                assert!(p.shader.is_none_or(|s| s < m.shaders.len()), "{what}: shader");
                for v in &p.vertices {
                    // Normals are within 0.0011 of unit length (the binormal
                    // and tangent are packed the same way but some models
                    // leave them zero, which decodes to a tiny vector).
                    let len = v.normal.iter().map(|c| c * c).sum::<f32>().sqrt();
                    assert!((len - 1.0).abs() < 0.002, "{what}: normal of length {len}");
                    assert!(v.binormal.iter().chain(&v.tangent).all(|c| c.abs() <= 1.0), "{what}");
                    assert!(v.node0 < m.nodes.len(), "{what}: node0 {}", v.node0);
                    assert!(v.node1.is_none_or(|n| n < m.nodes.len()), "{what}: node1");
                    assert!((0.0..=1.0).contains(&v.weight0), "{what}: weight {}", v.weight0);
                    assert!(v.position.iter().all(|c| c.is_finite()), "{what}: position");
                    assert!(v.texcoord.iter().all(|c| c.is_finite()), "{what}: texcoord");
                }
            }
        }
        // Every region's every permutation names geometries that exist.
        for r in &m.regions {
            for p in &r.permutations {
                assert!(p.geometry_indices.iter().flatten().all(|&g| g < m.geometries.len()));
            }
        }
        let (a, d) = winding(&m);
        agree += a;
        disagree += d;
    }
    eprintln!(
        "{}: {models} models, {parts} parts, {vertices} vertices ({agree} triangles agree with their normals, {disagree} do not), swept in {:?}",
        map.header().name,
        t0.elapsed()
    );
    (models, parts, agree, disagree)
}

#[test]
fn sweep_bloodgulch() {
    let Some(map) = bloodgulch() else { return };
    let (models, parts, agree, disagree) = sweep(map);
    assert_eq!((models, parts), (63, 359));
    let share = agree as f64 / (agree + disagree) as f64;
    assert!(
        share >= 0.995,
        "{agree} of {} triangles agree with their normals",
        agree + disagree
    );
}

#[test]
fn sweep_hangemhigh() {
    let Some(map) = load("hangemhigh") else { return };
    let (models, parts, agree, disagree) = sweep(&map);
    assert_eq!((models, parts), (46, 323));
    assert!(agree as f64 / (agree + disagree) as f64 >= 0.995);
}

#[test]
fn models_and_parts_of_every_multiplayer_map() {
    let maps = [
        ("beavercreek", 53, 341),
        ("bloodgulch", 63, 359),
        ("boardingaction", 51, 336),
        ("carousel", 50, 328),
        ("chillout", 47, 324),
        ("damnation", 52, 335),
        ("hangemhigh", 46, 323),
        ("longest", 46, 324),
        ("prisoner", 45, 322),
        ("putput", 45, 318),
        ("ratrace", 47, 326),
        ("sidewinder", 56, 351),
        ("wizard", 48, 329),
    ];
    for (name, models, parts) in maps {
        let Some(map) = load(name) else { return };
        let tags: Vec<_> = map.of_class(class::MODEL).collect();
        let n: usize = tags
            .iter()
            .map(|t| {
                Model::read(&map, t)
                    .unwrap()
                    .geometries
                    .iter()
                    .map(|g| g.parts.len())
                    .sum::<usize>()
            })
            .sum();
        assert_eq!((tags.len(), n), (models, parts), "{name}");
        assert_eq!(
            (map.model_vertex_buffers().len(), map.model_index_buffers().len()),
            (parts, parts),
            "{name}"
        );
        // The Warthog is the largest model on every map.
        let warthog = Model::read(&map, entry(&map, "vehicles\\warthog\\warthog")).unwrap();
        let total: usize = warthog
            .highest_detail_parts()
            .iter()
            .map(|p| p.part.vertices.len())
            .sum();
        assert_eq!(total, 3690, "{name}: warthog vertices");
    }
}

#[test]
fn warthog_scales_regions_and_permutations() {
    let Some(map) = bloodgulch() else { return };
    let m = model(map, "vehicles\\warthog\\warthog");
    assert_eq!(m.nodes.len(), 18);
    assert!((m.base_map_u_scale - 1.999474).abs() < 1e-5 && (m.base_map_v_scale - 3.000816).abs() < 1e-5);
    assert_eq!(m.geometries.len(), 20);
    let names: Vec<_> = m.regions.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["barrels", "hull", "tire"]);
    assert_eq!(
        m.regions[1].permutations[0].geometry_indices,
        [Some(10), Some(10), Some(7), Some(4), Some(1)]
    );
    let blur = m.regions[2].permutations.iter().find(|p| p.name == "~blur").unwrap();
    assert_eq!(blur.flags & 1, 1);
    assert_eq!(blur.geometry_indices[4], Some(12));
    // Highest detail: barrels 205, hull 2853, tire 632 vertices.
    let per_region: Vec<usize> = (0..3)
        .map(|r| {
            m.highest_detail_parts()
                .iter()
                .filter(|p| p.region == r)
                .map(|p| p.part.vertices.len())
                .sum()
        })
        .collect();
    assert_eq!(per_region, [205, 2853, 632]);
    // A texture coordinate carries the scale: vertices decode to 0..1 first.
    let v = &m.highest_detail_parts()[1].part.vertices;
    assert!(v.iter().any(|v| v.texcoord[0] > 1.0 || v.texcoord[1] > 1.0));
}
