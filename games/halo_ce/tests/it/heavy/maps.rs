//! Halo's maps converted from the user's disc image (`game::maps`),
//! against the numbers the specs state (structure_bsp.md, "Measured
//! facts"; collision.md, "Measured facts"; scenario.md). Each test passes
//! with a note without an install.

use std::{path::PathBuf, sync::Arc};

use bevy::math::Vec3;
use chimera_core::mount::{Mount, served};
use chimera_world::map::{
    MapData, MapLoadOptions,
    cache::{MapCache, Outcome},
};
use halo_ce::game::{
    GAME, INFO, MULTIPLAYER_MAPS,
    maps::{self, collision, lightmap::UNWRITTEN, space},
};

/// The install's mount, if there is one.
pub fn mount() -> Option<Arc<Mount>> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return None;
    };
    if !served::usable(GAME, &path, "") {
        eprintln!("skipping: no Halo CE install at {}", path.display());
        return None;
    }
    Some(Arc::new((INFO.open)(&path).unwrap()))
}

/// Map `name` converted (no cache), its textures decoded or not.
pub fn load(mount: &Mount, name: &str, textures: bool) -> MapData {
    let options = MapLoadOptions {
        textures,
        ..Default::default()
    };
    let start = std::time::Instant::now();
    let (map, outcome) = maps::load_in(mount, name, &options, None).unwrap();
    assert_eq!(outcome, Outcome::Off);
    eprintln!(
        "{name} (textures {textures}): converted in {:.2} s",
        start.elapsed().as_secs_f32()
    );
    map
}

/// The spec's counts and bounds of a map (structure_bsp.md "Per map",
/// collision.md "Measured facts").
struct Expected {
    name: &'static str,
    triangles: usize,
    vertices: usize,
    collision_triangles: usize,
    bounds: [[f32; 2]; 3],
    /// Spawns without a team (slayer's), red's and blue's (capture the
    /// flag's after the flag check): multiplayer_rules.md 11.2.
    spawns: [usize; 3],
}

const HANG_EM_HIGH: Expected = Expected {
    name: "hangemhigh",
    triangles: 3218,
    vertices: 5381,
    collision_triangles: 2874,
    bounds: [[5.900, 35.878], [-22.823, 15.823], [-9.253, 8.183]],
    spawns: [28, 19, 20],
};

const BLOOD_GULCH: Expected = Expected {
    name: "bloodgulch",
    triangles: 5503,
    vertices: 5790,
    collision_triangles: 5468,
    bounds: [[5.907, 131.902], [-190.219, -45.057], [-0.349, 26.233]],
    spawns: [16, 17, 21],
};

/// The level's own meshes (not the sky's).
fn level_meshes(map: &MapData) -> impl Iterator<Item = &chimera_world::map::MapMesh> {
    map.meshes.iter().filter(|m| !m.skybox)
}

fn bounds_of(map: &MapData) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for p in level_meshes(map).flat_map(|m| m.positions.iter()) {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
    }
    (lo, hi)
}

/// The nearest hit of a ray from `o` along `d` (unit) on any collision
/// triangle, either side (Möller-Trumbore, brute force).
fn ray_hit(map: &MapData, o: Vec3, d: Vec3) -> Option<f32> {
    ray_hit_in(collision::all_triangles(map), o, d)
}

/// The same over the given triangles.
fn ray_hit_in(triangles: impl Iterator<Item = [[f32; 3]; 3]>, o: Vec3, d: Vec3) -> Option<f32> {
    triangles
        .filter_map(|t| {
            let [a, b, c] = t.map(Vec3::from);
            let (e1, e2) = (b - a, c - a);
            let p = d.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-9 {
                return None;
            }
            let s = o - a;
            let u = s.dot(p) / det;
            let q = s.cross(e1);
            let v = d.dot(q) / det;
            let t = e2.dot(q) / det;
            (u >= -1e-5 && v >= -1e-5 && u + v <= 1.0 + 1e-5 && t >= 0.0).then_some(t)
        })
        .min_by(f32::total_cmp)
}

fn check(mount: &Mount, e: &Expected) -> MapData {
    let map = load(mount, e.name, true);
    assert_eq!(map.name, format!("halo_ce:{}", e.name));
    assert!(map.file_hash.is_some());
    // Geometry: the spec's totals.
    let triangles: usize = level_meshes(&map).map(|m| m.indices.len() / 3).sum();
    assert_eq!(triangles, e.triangles, "triangles");
    let vertices: usize = level_meshes(&map).map(|m| m.positions.len()).sum();
    assert_eq!(vertices, e.vertices, "vertices");
    for m in &map.meshes {
        assert_eq!(m.normals.len(), m.positions.len());
        assert_eq!(m.uvs.len(), m.positions.len());
        assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
    }
    // World bounds in metres: the spec's bounds through the axes.
    let [x, y, z] = e.bounds;
    let (want_lo, want_hi) = space::bounds(x, y, z);
    let (lo, hi) = bounds_of(&map);
    assert!((lo - want_lo).abs().max_element() < 0.01, "{lo} vs {want_lo}");
    assert!((hi - want_hi).abs().max_element() < 0.01, "{hi} vs {want_hi}");
    // Every mesh has a texture, or a warning names its shader.
    for m in &map.meshes {
        assert!(
            m.texture.is_some() || map.warnings.iter().any(|w| w.contains(&m.material)),
            "{} has no texture and no warning",
            m.material
        );
        assert!(!m.material.is_empty());
    }
    // Lightmap coordinates inside the atlas, and no triangle's bilinear
    // footprint reads an unwritten texel.
    let lm = map.lightmap.as_ref().expect("a lightmap");
    let (w, h) = (lm.width as f32, lm.height as f32);
    let mut lit = 0;
    for m in level_meshes(&map).filter(|m| !m.lightmap_uvs.is_empty()) {
        lit += 1;
        assert_eq!(m.lightmap_uvs.len(), m.positions.len());
        for t in m.indices.chunks(3) {
            let uv: Vec<[f32; 2]> = t.iter().map(|&i| m.lightmap_uvs[i as usize]).collect();
            for p in &uv {
                assert!(p.iter().all(|c| *c > 0.0 && *c < 1.0), "{p:?}");
            }
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for p in &uv {
                (x0, x1) = (x0.min(p[0] * w), x1.max(p[0] * w));
                (y0, y1) = (y0.min(p[1] * h), y1.max(p[1] * h));
            }
            let (tx0, ty0) = ((x0 - 0.5).floor() as u32, (y0 - 0.5).floor() as u32);
            let (tx1, ty1) = ((x1 - 0.5).floor() as u32 + 1, (y1 - 0.5).floor() as u32 + 1);
            for ty in ty0..=ty1.min(lm.height - 1) {
                for tx in tx0..=tx1.min(lm.width - 1) {
                    assert_ne!(
                        lm.rgb[(ty * lm.width + tx) as usize],
                        UNWRITTEN,
                        "{} at {tx}, {ty}",
                        m.material
                    );
                }
            }
        }
    }
    assert!(lit > 0);
    // Collision: the spec's fan triangle count, everything solid kept.
    // The level's own triangles come first; the scenery's follow.
    let collision_triangles = collision::all_triangles(&map).count();
    assert!(
        collision_triangles > e.collision_triangles,
        "the scenery adds collision triangles"
    );
    assert_eq!(
        level_collision_triangles(mount, e.name),
        e.collision_triangles,
        "collision triangles"
    );
    // Spawns: inside the world bounds (feet on the floor, which may be
    // the lowest point), and ground under each within a body height.
    assert!(!map.spawns.is_empty());
    assert_eq!(map.spawns.len(), map.spawn_yaws.len());
    // Slayer's starts without a team, capture the flag's red and blue as
    // Team 1 and 2 (`maps::spawns`).
    let of = |t: Option<u8>| map.spawns.iter().filter(|(_, team)| team.map(|t| t.0) == t).count();
    assert_eq!([of(None), of(Some(1)), of(Some(2))], e.spawns, "slayer, red and blue spawns");
    for (p, _) in &map.spawns {
        let slack = Vec3::splat(0.05);
        assert!(
            p.cmpge(want_lo - slack).all() && p.cmple(want_hi + slack).all(),
            "{p} outside the world"
        );
        let up = 0.3;
        let hit = ray_hit(&map, *p + Vec3::Y * up, Vec3::NEG_Y).unwrap_or_else(|| panic!("no ground under {p}"));
        assert!(hit - up < 2.13, "ground {:.2} m below the spawn at {p}", hit - up);
    }
    for w in &map.warnings {
        eprintln!("{}: {w}", e.name);
    }
    map
}

#[test]
fn hang_em_high_converts_as_the_specs_say() {
    let Some(mount) = mount() else { return };
    let map = check(&mount, &HANG_EM_HIGH);
    // Collision: nine metal materials and nine material-less surfaces,
    // no glass, no flags (collision.md).
    assert!(map.collision_sets.is_empty(), "no glass among the scenery either");
    // Two pages packed (256 x 256 and 512 x 512).
    let lm = map.lightmap.as_ref().unwrap();
    assert!(lm.width >= 514 && lm.height >= 514);
    // The opaque surfaces are metal (their shaders' type 7).
    assert!(
        level_meshes(&map)
            .filter(|m| !m.lightmap_uvs.is_empty())
            .all(|m| m.surface.as_deref() == Some("metal"))
    );
    // The light cards (one shader, 176 materials) are one mesh.
    let unlit = level_meshes(&map).filter(|m| m.lightmap_uvs.is_empty()).count();
    assert_eq!(unlit, 1);
    // The sky's outdoor fog (scenario.md: density 0.2, start 0, opaque 32).
    let fog = map.fog.as_ref().expect("fog");
    assert!(fog.linear && (fog.max_density - 0.2).abs() < 1e-6 && fog.start == 0.0);
    assert!(map.sky.is_some(), "a background");
}

#[test]
fn blood_gulch_converts_as_the_specs_say() {
    let Some(mount) = mount() else { return };
    let map = check(&mount, &BLOOD_GULCH);
    // Player starting location 0 (98.49340, -157.63901, 1.70473) is
    // the first spawn if eligible for slayer; ground 0.0012 WU under it.
    let start0 = space::point([98.4934, -157.639, 1.70473]);
    if let Some((p, _)) = map.spawns.iter().find(|(p, _)| (*p - start0).length() < 1e-3) {
        let hit = ray_hit(&map, *p + Vec3::Y, Vec3::NEG_Y).unwrap();
        assert!((hit - 1.0 - (1.70473 - 1.703486) * 3.048).abs() < 0.01, "{hit}");
    }
    // The 94 material-less boundary surfaces are in the collision: it
    // reaches the lid at z = 68.037 WU, far above the render bounds.
    let top = map.collision_positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!((top - 68.03709 * 3.048).abs() < 0.01, "{top}");
    assert!(
        level_meshes(&map).any(|m| m.detail.is_some()),
        "the terrain's detail map"
    );
}

/// What differs only in texels: textures' texels and blocks.
fn same_but_texels(a: &MapData, b: &MapData) {
    assert_eq!(a.name, b.name);
    assert_eq!(a.file_hash, b.file_hash);
    assert_eq!(format!("{:?}", a.meshes), format!("{:?}", b.meshes));
    assert_eq!(a.textures.len(), b.textures.len());
    for (x, y) in a.textures.iter().zip(&b.textures) {
        assert_eq!(
            (&x.name, x.width, x.height, x.srgb),
            (&y.name, y.width, y.height, y.srgb)
        );
    }
    let (la, lb) = (a.lightmap.as_ref().unwrap(), b.lightmap.as_ref().unwrap());
    assert_eq!((la.width, la.height), (lb.width, lb.height));
    assert!(la.rgb == lb.rgb);
    assert_eq!(format!("{:?}", a.models), format!("{:?}", b.models));
    assert_eq!(format!("{:?}", a.props), format!("{:?}", b.props));
    assert_eq!(format!("{:?}", a.sky_camera), format!("{:?}", b.sky_camera));
    assert_eq!(a.collision_positions, b.collision_positions);
    assert_eq!(a.collision_indices, b.collision_indices);
    assert_eq!(format!("{:?}", a.collision_sets), format!("{:?}", b.collision_sets));
    assert_eq!(a.spawns, b.spawns);
    assert_eq!(a.spawn_yaws, b.spawn_yaws);
    assert_eq!(format!("{:?}", a.fog), format!("{:?}", b.fog));
    assert_eq!(format!("{:?}", a.sky), format!("{:?}", b.sky));
    assert_eq!(format!("{:?}", a.look), format!("{:?}", b.look));
    assert_eq!(a.warnings, b.warnings);
}

#[test]
fn the_lean_load_differs_only_in_texels() {
    let Some(mount) = mount() else { return };
    let full = load(&mount, "hangemhigh", true);
    let lean = load(&mount, "hangemhigh", false);
    same_but_texels(&full, &lean);
    assert!(
        lean.textures
            .iter()
            .all(|t| t.rgba8.is_empty() && t.mips.is_empty() && t.bc.is_none())
    );
    assert!(
        full.textures
            .iter()
            .all(|t| t.rgba8.len() == (t.width * t.height * 4) as usize)
    );
    assert!(
        full.textures.iter().any(|t| t.bc.is_some()),
        "DXT textures keep their blocks"
    );
}

#[test]
fn a_second_load_comes_from_the_map_cache() {
    let Some(mount) = mount() else { return };
    let dir = std::env::temp_dir().join(format!("halo_ce_map_cache_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cache = MapCache::at(PathBuf::from(&dir), 10_000_000_000);
    for name in ["hangemhigh", "bloodgulch"] {
        let options = MapLoadOptions::default();
        let t = std::time::Instant::now();
        let (first, outcome) = maps::load_in(&mount, name, &options, Some(&cache)).unwrap();
        let converted = t.elapsed().as_secs_f32();
        assert_eq!(outcome, Outcome::Miss);
        let t = std::time::Instant::now();
        let (second, outcome) = maps::load_in(&mount, name, &options, Some(&cache)).unwrap();
        let cached = t.elapsed().as_secs_f32();
        assert_eq!(outcome, Outcome::Hit);
        eprintln!("{name}: converted (and written) in {converted:.2} s, from the cache in {cached:.2} s");
        same_but_texels(&first, &second);
        for (x, y) in first.textures.iter().zip(&second.textures) {
            assert!(x.decoded().rgba8 == y.decoded().rgba8, "{}", x.name);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_multiplayer_map_loads() {
    let Some(mount) = mount() else { return };
    for name in MULTIPLAYER_MAPS {
        let map = load(&mount, name, true);
        assert!(map.triangle_count() > 0 && !map.spawns.is_empty() && !map.collision_indices.is_empty());
        eprintln!(
            "{name}: {} meshes, {} textures, lightmap {:?}, {} spawns, {} collision triangles, {} warnings",
            map.meshes.len(),
            map.textures.len(),
            map.lightmap.as_ref().map(|l| (l.width, l.height)),
            map.spawns.len(),
            collision::all_triangles(&map).count(),
            map.warnings.len()
        );
        for w in &map.warnings {
            eprintln!("  {name}: {w}");
        }
    }
}

/// The level's collision triangles alone (the BSP's, before the scenery's).
fn level_collision_triangles(mount: &Mount, name: &str) -> usize {
    let (file, _, _) = scenario_of(mount, name);
    let level = blam_formats::collision::read_level(&file, 0).unwrap();
    let mut fresh = MapData::default();
    collision::add(&level, &mut fresh).unwrap();
    collision::all_triangles(&fresh).count()
}

/// A map's file and scenario (read from the mount).
fn scenario_of(mount: &Mount, name: &str) -> (blam_formats::cache::MapFile, blam_formats::scenario::Scenario, Vec<u8>) {
    let bytes = mount.read(&maps::map_path(name)).unwrap();
    let file = blam_formats::cache::MapFile::load(&bytes).unwrap();
    let scenario = blam_formats::scenario::Scenario::read(&file).unwrap();
    (file, scenario, bytes)
}

/// scenario.md, "Measured facts": scenery placements per map.
const SCENERY: [(&str, usize); 13] = [
    ("beavercreek", 30),
    ("bloodgulch", 50),
    ("boardingaction", 29),
    ("carousel", 40),
    ("chillout", 8),
    ("damnation", 12),
    ("hangemhigh", 8),
    ("longest", 4),
    ("prisoner", 2),
    ("putput", 32),
    ("ratrace", 8),
    ("sidewinder", 35),
    ("wizard", 6),
];

#[test]
fn every_created_scenery_placement_is_a_prop_inside_the_level() {
    let Some(mount) = mount() else { return };
    let mut total = 0;
    for (name, placements) in SCENERY {
        let map = load(&mount, name, true);
        // Created placements (scenario.md: a palette index in range and
        // the BSP's on-BSP bit); the table counts placements, and some maps
        // have placements with no BSP bit (Beaver Creek's fern).
        let (_, scenario, _) = scenario_of(&mount, name);
        let created = scenario
            .scenery
            .iter()
            .filter(|p| p.header.is_created(scenario.scenery_palette.len(), Some(0)))
            .count();
        assert_eq!(scenario.scenery.len(), placements, "{name}: placements");
        assert_eq!(map.props.len(), created, "{name}: props");
        eprintln!("{name}: {created} of {placements} scenery placements created");
        assert!(created <= placements);
        // Blood Gulch's table says 50 placements; four of them (palette 11
        // and 13) are in no structure BSP, so the creation rule skips them.
        match name {
            "bloodgulch" => assert_eq!(created, 46),
            "hangemhigh" => assert_eq!(created, 8),
            _ => {}
        }
        total += map.props.len();
        let (lo, hi) = bounds_of(&map);
        // The collision reaches past the render geometry to the boundary
        // walls; scenery stands in the playable level (a small margin).
        let slack = Vec3::splat(3.0);
        for p in &map.props {
            assert!(
                p.translation.cmpge(lo - slack).all() && p.translation.cmple(hi + slack).all(),
                "{name}: a prop at {} outside {lo} .. {hi}",
                p.translation
            );
            assert!(p.rotation.is_normalized() && p.model < map.models.len());
            assert!(!p.skybox && p.lighting.is_some());
        }
        for (i, m) in map.models.iter().enumerate() {
            assert!(
                m.meshes.iter().any(|x| x.texture.is_some()) || !map.warnings.is_empty(),
                "{name}: model {i} has no textured mesh and no warning"
            );
            for mesh in &m.meshes {
                assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()));
                assert_eq!(mesh.normals.len(), mesh.positions.len());
                assert!(!mesh.skybox);
                assert!(m.bounds.1.cmpge(m.bounds.0).all());
            }
        }
        assert!(!map.warnings.iter().any(|w| w.contains("scenery placements not drawn")));
        for w in &map.warnings {
            eprintln!("{name}: {w}");
        }
    }
    assert!((250..=264).contains(&total), "{total} created of the disc's 264");
}

/// A model of Blood Gulch stands at its placement at a sensible size:
/// the boulder placement's props sit on the ground within a metre or two.
#[test]
fn blood_gulch_scenery_stands_on_the_ground() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", true);
    let level = level_collision_triangles(&mount, "bloodgulch");
    let ground: Vec<[[f32; 3]; 3]> = collision::all_triangles(&map).take(level).collect();
    let mut on_ground = 0;
    for p in &map.props {
        let model = &map.models[p.model];
        let size = model.bounds.1 - model.bounds.0;
        assert!(
            size.max_element() > 0.05 && size.max_element() < 80.0,
            "model size {size}"
        );
        if let Some(h) = ray_hit_in(ground.iter().copied(), p.translation + Vec3::Y * 30.0, Vec3::NEG_Y) {
            let below = p.translation.y - (p.translation.y + 30.0 - h);
            if below.abs() < 6.0 {
                on_ground += 1;
            }
        }
    }
    assert!(
        on_ground * 10 >= map.props.len() * 8,
        "{on_ground} of {} on the ground",
        map.props.len()
    );
}

#[test]
fn scenery_collision_is_solid_above_the_ground() {
    let Some(mount) = mount() else { return };
    for name in ["bloodgulch", "hangemhigh"] {
        let map = load(&mount, name, true);
        let (file, scenario, _) = scenario_of(&mount, name);
        let level = level_collision_triangles(&mount, name);
        let all: Vec<[[f32; 3]; 3]> = collision::all_triangles(&map).collect();
        assert!(all.len() > level, "{name}: the scenery adds collision");
        let ground = &all[..level];
        // Placements whose object has a collision model, in the file's order.
        let mut with = 0;
        let mut hits = 0;
        let mut first_hit = None;
        for p in &scenario.scenery {
            let h = &p.header;
            if !h.is_created(scenario.scenery_palette.len(), Some(0)) {
                continue;
            }
            let entry = &scenario.scenery_palette[h.palette_entry(scenario.scenery_palette.len()).unwrap()];
            let header = blam_formats::scenario::ObjectHeader::read(&file, file.tag(entry.object.id).unwrap()).unwrap();
            if header.collision_model.id.is_none() {
                continue;
            }
            with += 1;
            let start = space::point(h.position) + Vec3::Y * 40.0;
            let (Some(top), Some(floor)) = (
                ray_hit_in(all.iter().copied(), start, Vec3::NEG_Y),
                ray_hit_in(ground.iter().copied(), start, Vec3::NEG_Y),
            ) else {
                continue;
            };
            let solid_above = floor - top;
            if solid_above > 0.2 {
                hits += 1;
                first_hit.get_or_insert((h.position, solid_above));
            }
        }
        eprintln!(
            "{name}: {with} placements with a collision model, {hits} solid above the ground, first {first_hit:?}"
        );
        assert!(with > 0, "{name}: scenery with collision");
        // Blood Gulch's rocks and trees stand above the ground (Hang 'Em
        // High's two are flat, under a ceiling the ray starts above).
        if name == "bloodgulch" {
            assert!(hits * 2 > with, "{name}: {hits} of {with} solid above the ground");
            // The first placement with a collision model (a boulder).
            assert!(first_hit.is_some());
        }
    }
}

#[test]
fn blood_gulch_has_a_sky_that_encloses_the_level() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", true);
    let cam = map.sky_camera.as_ref().expect("a sky camera");
    assert_eq!(cam.scale, halo_ce::game::maps::sky::SCALE);
    assert_eq!(cam.origin, Vec3::ZERO);
    assert!(cam.fog.is_none(), "the world's fog stays the world's");
    let sky: Vec<_> = map.meshes.iter().filter(|m| m.skybox).collect();
    assert_eq!(sky.len(), 5, "the five regions of the sky model");
    for m in &sky {
        assert!(m.texture.is_some(), "{} has a texture", m.material);
        assert!(m.unlit, "{} is unlit", m.material);
        assert!(!m.indices.is_empty());
    }
    // The meshes, scaled by the camera's scale, enclose the level.
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for p in sky.iter().flat_map(|m| m.positions.iter()) {
        lo = lo.min(Vec3::from(*p) * cam.scale);
        hi = hi.max(Vec3::from(*p) * cam.scale);
    }
    let (level_lo, level_hi) = bounds_of(&map);
    assert!(
        lo.x < level_lo.x && lo.z < level_lo.z && hi.x > level_hi.x && hi.z > level_hi.z,
        "{lo} {hi}"
    );
    assert!(hi.y > level_hi.y);
    // The sky model's size (models.md): about 425 km across after scaling
    // (y -53461.3..86000.4 units).
    // (engine x is Halo's y: -53461.3..86000.4 units).
    assert!(
        ((hi.x - lo.x) / 3.048 - 139461.7).abs() < 100.0,
        "{}",
        (hi.x - lo.x) / 3.048
    );
    // The flat background stays behind it, and the level keeps a fog.
    assert!(map.sky.is_some() && map.fog.is_some());
    assert!(!map.warnings.iter().any(|w| w.contains("sky model")));
}

/// Maps without a sky model keep the flat background: Chill Out's sky has
/// no model, Chiron TL-34 and Rat Race have no sky at all.
#[test]
fn maps_without_a_sky_model_have_no_sky_camera() {
    let Some(mount) = mount() else { return };
    for name in ["chillout", "putput", "ratrace"] {
        let map = load(&mount, name, true);
        assert!(map.sky_camera.is_none(), "{name}");
        assert!(map.meshes.iter().all(|m| !m.skybox), "{name}");
    }
    // Chill Out has a sky tag, so its flat background (the fog colour).
    assert!(load(&mount, "chillout", false).sky.is_some());
    // Every other map's sky has a model drawn as a skybox.
    for name in [
        "beavercreek",
        "boardingaction",
        "carousel",
        "damnation",
        "hangemhigh",
        "longest",
        "prisoner",
        "sidewinder",
        "wizard",
    ] {
        let map = load(&mount, name, false);
        assert!(
            map.sky_camera.is_some() && map.meshes.iter().any(|m| m.skybox),
            "{name}"
        );
    }
}
