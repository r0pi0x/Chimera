//! Combat Arms' maps from the user's install (`game::world`): every
//! map converts and loads, a player landing at its first spawn (the 33
//! plain ones; with the V2 key the 72 of WORLDS, WORLDS4 and WORLDS5, a
//! test per archive), and a player spawned on WAREHOUSE stands on its
//! floor, walks, and stops at a wall. Passes with a note without
//! `[games.combat_arms] path` in chimera.local.toml.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::mount::config::LocalConfig;
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin},
};
use combat_arms::{
    game::{GAME, INFO, movement, world},
    harness::CaSim,
};

fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !install.join("Game").is_dir() {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// One map converted, with what every map must have: drawn meshes with
/// lightmap coordinates inside the atlas, a collision mesh, and spawns;
/// then loaded into a simulation, where a player at the first spawn
/// lands on a floor.
fn check_map(mount: &chimera_core::mount::Mount, name: &str) {
    let started = std::time::Instant::now();
    let map = world::load_world(mount, name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let lightmap = map.lightmap.as_ref().unwrap_or_else(|| panic!("{name}: no lightmap"));
    eprintln!(
        "{name}: {} meshes, {} triangles, {} collision triangles, {} spawns, lightmap {} x {}, {} warnings",
        map.meshes.len(),
        map.triangle_count(),
        map.collision_indices.len(),
        map.spawns.len(),
        lightmap.width,
        lightmap.height,
        map.warnings.len()
    );
    for w in &map.warnings {
        eprintln!("{name}: warning: {w}");
    }
    // TWINTOWER's file holds 220 polygons (564 render triangles, all
    // vertex-lit): a stub world, drawn as it is.
    if name != "twintower" {
        assert!(map.triangle_count() > 1000, "{name}");
        // Static collision, or world models' nodes (HELLGATE's rooms
        // are world models it shows and hides).
        let nodes: usize = map
            .collision_sets
            .iter()
            .filter(|s| s.name == world::NODE_FLOORS)
            .map(|s| s.indices.len())
            .sum();
        assert!(map.collision_indices.len() + nodes > 1000, "{name}");
    }
    assert!(lightmap.width <= world::ATLAS_MAX && lightmap.height <= world::ATLAS_MAX, "{name}");
    for m in &map.meshes {
        assert_eq!(m.lightmap_uvs.len(), m.positions.len(), "{name}");
        assert!(m.lightmap_uvs.iter().flatten().all(|c| (0.0..=1.0).contains(c)), "{name}");
        assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()), "{name}");
        assert!(m.positions.iter().flatten().all(|c| c.is_finite()), "{name}");
    }
    assert!(!map.spawns.is_empty() && map.spawn_yaws.len() == map.spawns.len(), "{name}");
    assert!(map.spawns.iter().all(|(p, _)| p.is_finite()), "{name}");
    let converted = started.elapsed();
    let (feet, _) = map.spawns[0];
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y * 0.5, movement::ID);
    sim.seconds(1.0);
    let at = sim.position(p);
    eprintln!("{name}: converted in {converted:.1?}, spawned at {feet}, standing at {at} ({:.1?} in all)", started.elapsed());
    if NO_FLOOR.contains(&name) {
        // Over water with nothing solid below (`hgoilrig_spawns_on_its_own_family`).
        assert!(at.y > feet.y - 20.0, "{name}: at {at}, spawn {feet}");
        return;
    }
    assert!(at.y > feet.y - 0.5 && at.y < feet.y + 2.0, "{name}: standing at {at}, spawn {feet}");
}

/// Maps whose spawns have no solid floor under them in the file: the
/// player lands on a liquid's top and floats there.
const NO_FLOOR: &[&str] = &["hgoilrig"];

/// Every map whose archive folder is `folder`, two at a time; how many.
fn check_maps_in(folder: &str) -> Option<usize> {
    let mount = mount()?;
    let names: Vec<String> = world::map_names(&mount)
        .into_iter()
        .filter(|n| world::find(&mount, n).is_some_and(|p| p.split('/').next() == Some(folder)))
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failed = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(name) = names.get(i) else { break };
                    let check = std::panic::AssertUnwindSafe(|| check_map(&mount, name));
                    if std::panic::catch_unwind(check).is_err() {
                        failed.lock().unwrap().push(name.clone());
                    }
                }
            });
        }
    });
    let failed = failed.into_inner().unwrap();
    assert!(failed.is_empty(), "{folder}: {failed:?} failed (above)");
    Some(names.len())
}

/// The keyed maps (V2) of one archive; skipped without the key.
fn keyed_maps(folder: &str, count: usize) {
    if !combat_arms::game::mount::keys().is_ok_and(|k| k.contains_key("V2")) {
        eprintln!("skipping: no V2 key in chimera.local.toml");
        return;
    }
    if let Some(n) = check_maps_in(folder) {
        assert_eq!(n, count, "maps in {folder}");
    }
}

/// The 33 plain maps (WORLDS2, WORLDS3) convert.
#[test]
fn every_plain_map_converts() {
    let Some(a) = check_maps_in("worlds2") else { return };
    let b = check_maps_in("worlds3").unwrap();
    assert_eq!(a + b, 33);
}

/// WORLDS.rez's maps (V2) convert.
#[test]
fn keyed_maps_worlds() {
    keyed_maps("worlds", 36);
}

/// WORLDS4.rez's maps (V2) convert.
#[test]
fn keyed_maps_worlds4() {
    keyed_maps("worlds4", 32);
}

/// WORLDS5.rez's maps (V2) convert.
#[test]
fn keyed_maps_worlds5() {
    keyed_maps("worlds5", 4);
}

fn warehouse() -> Option<MapData> {
    world::load_from(&mount()?, "warehouse").map(Some).unwrap()
}

/// WAREHOUSE's spawns: test case 2's 24 Team Death Match points, a team
/// each, and each over the floor.
#[test]
fn warehouse_spawns() {
    let Some(map) = warehouse() else { return };
    assert_eq!(map.name, "combat_arms:warehouse");
    assert_eq!(map.spawns.len(), 24);
    let teams = |t: u8| map.spawns.iter().filter(|(_, team)| team.map(|x| x.0) == Some(t)).count();
    assert_eq!((teams(1), teams(2)), (12, 12));
    // Object 92, GameStartPoint11 (JSF), at (618, −5546, −2271.0002) u in
    // file axes: Z mirrored, metres; its feet on the floor below.
    let s = movement::movement_data().meters_per_unit;
    let at = Vec3::new(618.0, -5546.0, 2271.0) * s;
    let (feet, team) = map
        .spawns
        .iter()
        .find(|(p, _)| p.xz().distance(at.xz()) < 0.01)
        .expect("GameStartPoint11");
    assert_eq!(team.map(|t| t.0), Some(1));
    assert!(feet.y < at.y && feet.y > at.y - 200.0 * s, "{feet} below {at}");
}

/// world_dat.md cases 9, 10, 13, 14 and 20 on WAREHOUSE: its non-solid
/// physics polygons stop traces only and its 50 collision-blocker
/// polygons players only; the sky camera follows the player through the
/// sky object's inner box; the fog is the world properties' (linear,
/// (23, 23, 19), 0 to 10000 u); no file hash.
#[test]
fn warehouse_collision_sky_and_fog() {
    let Some(map) = warehouse() else { return };
    let s = movement::movement_data().meters_per_unit;
    let tris = |players: bool, traces: bool| -> usize {
        map.collision_sets
            .iter()
            .filter(|c| c.characters == players && c.traces == traces)
            .map(|c| c.indices.len())
            .sum()
    };
    assert!(tris(false, true) >= 4_960, "non-solid polygons: {}", tris(false, true));
    assert!(tris(true, false) >= 50, "blocker polygons: {}", tris(true, false));
    // Sky: SkyPointerBox00 at (6623, 7009, -1471) u, SkyDims 256,
    // InnerPercent 0.001; extents +-(9952, 10080, 10112).
    let cam = map.sky_camera.as_ref().expect("a sky camera");
    let sky_at = |p: Vec3| cam.origin + p / cam.scale;
    let engine = |p: Vec3| Vec3::new(p.x, p.y, -p.z) * s;
    let close = |a: Vec3, b: Vec3| (a - b).abs().max_element() < 0.01 * s;
    assert!(close(sky_at(Vec3::ZERO), engine(Vec3::new(6623.0, 7009.0, -1471.0))), "{}", sky_at(Vec3::ZERO) / s);
    let corner = engine(Vec3::new(9952.0, 10080.0, 10112.0));
    let want = engine(Vec3::new(6623.256, 7009.256, -1470.744));
    assert!((sky_at(corner) - want).abs().max_element() < 0.02 * s, "{} vs {}", sky_at(corner) / s, want / s);
    assert!(map.meshes.iter().any(|m| m.skybox), "the sky's own world model is drawn by the sky camera");
    let fog = map.fog.as_ref().expect("fog");
    assert_eq!(fog.color.map(|c| (c * 255.0).round() as u8), [23, 23, 19]);
    assert!(fog.start.abs() < 1e-6 && (fog.end - 10_000.0 * s).abs() < 1e-3 && fog.linear, "{fog:?}");
    assert_eq!(map.file_hash, None);
}

/// world_dat.md case 12: SNOWSTORM's two light groups start off (their
/// objects' `StartOn` false); GRAVITYB's `Light_Grv_01` starts on and
/// `Light_Grv_02` off.
#[test]
fn light_groups_start_state() {
    let Some(mount) = mount() else { return };
    // Either may be in a keyed archive: then it is left out.
    let read = |name: &str| {
        let path = world::find(&mount, name)?;
        lithtech_formats::world::World::parse(&mount.read(&path).ok()?).ok()
    };
    let Some(snow) = read("snowstorm") else { return };
    let colours = world::light_group_colours(&snow);
    let names: Vec<String> = snow
        .render
        .all_blocks()
        .flat_map(|b| b.light_groups.iter().map(|g| g.name.clone()))
        .collect();
    assert!(!names.is_empty());
    for n in &names {
        assert_eq!(colours(n, [1.0, 0.968_627_5, 0.901_960_8]), [0.0; 3], "{n}");
    }
    let Some(grav) = read("gravityb") else {
        eprintln!("GRAVITYB is in a keyed archive: its part skipped");
        return;
    };
    let colours = world::light_group_colours(&grav);
    assert_ne!(colours("Light_Grv_01", [1.0; 3]), [0.0; 3]);
    assert_eq!(colours("Light_Grv_02", [1.0; 3]), [0.0; 3]);
}

/// A player spawned at a WAREHOUSE spawn stands on the floor, walks on
/// it, and a wall stops it.
#[test]
fn warehouse_walk_and_collide() {
    let Some(map) = warehouse() else { return };
    let (feet, _) = map.spawns[0];
    let yaw = map.spawn_yaws[0];
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(1.0);
    let rest = sim.position(p);
    assert!(sim.velocity(p).length() < 0.05, "at rest: {}", sim.velocity(p));
    assert!(rest.y > feet.y && rest.y < feet.y + 1.5, "standing at {rest}, feet {feet}");

    // Walk forward for a second: moves over the floor, stays on it.
    sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
    sim.seconds(1.0);
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(0.5);
    let walked = sim.position(p);
    assert!(walked.xz().distance(rest.xz()) > 1.0, "walked from {rest} to {walked}");
    assert!((walked.y - rest.y).abs() < 1.0, "still on the floor: {walked}");

    // The nearest wall around, by a ray at chest height; walk into it.
    let from = walked;
    let (dir, dist) = {
        let mut state = SystemState::<SpatialQuery>::new(sim.app.world_mut());
        let q = state.get(sim.app.world()).unwrap();
        // What players collide with (not the traces-only detail).
        let filter = SpatialQueryFilter::from_excluded_entities([p]).with_mask(chimera_core::core::CHARACTER_FILTER);
        (0..16)
            .filter_map(|k| {
                let a = k as f32 * std::f32::consts::TAU / 16.0;
                let d = Dir3::new(Vec3::new(a.sin(), 0.0, -a.cos())).unwrap();
                let hit = q.cast_ray(from, d, 30.0, true, &filter)?;
                Some((d, hit.distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a wall within 30 m")
    };
    // Face it (yaw 0 = −Z, positive turns toward −X) and run 3 s.
    sim.intent(p).yaw = (-dir.x).atan2(-dir.z);
    sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
    sim.seconds(3.0 + f64::from(dist) / 3.0);
    let stopped = sim.position(p);
    let moved = (stopped - from).dot(*dir);
    assert!(moved < dist, "stopped at {moved} m of the wall's {dist} m");
    assert!(moved > dist - 1.5, "reached the wall: {moved} m of {dist} m");
}

/// The second layers built from the install (world_dat.md "Second
/// textures"): WAREHOUSE's 12 dual-texture sections become meshes with a
/// second texture, coordinate set and weights; SNOWVALLEY's dual sections
/// carry a UV1 of their own; TOWERBRIDGE's car windows take a flat
/// environment map (`envmapalpha`) and its river the self-linked detail
/// texture.
#[test]
fn second_layers_from_the_install() {
    use chimera_world::map::{BlendMode, DetailMode};
    let Some(mount) = mount() else { return };
    let dual = |map: &MapData| -> Vec<usize> {
        (0..map.meshes.len())
            .filter(|&i| map.meshes[i].blend.is_some_and(|b| b.texture.is_some()))
            .collect()
    };
    let map = world::load_from(&mount, "warehouse").unwrap();
    let meshes = dual(&map);
    assert!(!meshes.is_empty(), "WAREHOUSE has dual-texture meshes");
    for &i in &meshes {
        let m = &map.meshes[i];
        assert_eq!(m.blend.unwrap().mode, BlendMode::BaseAlpha);
        assert_eq!((m.uvs2.len(), m.blend_weights.len()), (m.positions.len(), m.positions.len()));
        assert!(m.blend_weights.iter().all(|w| (0.0..=1.0).contains(w)));
    }
    let map = world::load_from(&mount, "snowvalley").unwrap();
    assert!(
        dual(&map).iter().any(|&i| {
            let m = &map.meshes[i];
            m.uvs.iter().zip(&m.uvs2).any(|(a, b)| a != b)
        }),
        "SNOWVALLEY's UV1 differs from UV0"
    );
    let map = world::load_from(&mount, "towerbridge").unwrap();
    let windows: Vec<_> = map.meshes.iter().filter_map(|m| m.flat_envmap).collect();
    assert!(windows.len() >= 10, "{} meshes with an environment map", windows.len());
    assert!(windows.iter().all(|e| e.by_base_alpha && e.signed_add));
    assert!(windows.iter().any(|e| map.textures[e.texture].name.eq_ignore_ascii_case("TEXFX\\SKY\\SKY20_RT.dtx")));
    let river = map
        .meshes
        .iter()
        .find(|m| m.material.eq_ignore_ascii_case("textures\\etc\\sprinkle_river.dtx"))
        .expect("the river");
    let d = river.detail.expect("its detail texture");
    assert_eq!((d.mode, d.scale, d.rotation), (DetailMode::SignedAdd, [1.0; 2], 0.0));
}

/// Census of the second texture layers on the plain maps (world_dat.md
/// "Second textures"): dual-texture sections, and base textures that link
/// a detail texture or an environment map.
#[test]
fn second_layer_census() {
    use lithtech_formats::{
        dtx::{Dtx, LinkKind, link_path},
        world::{World, shader},
    };
    let Some(mount) = mount() else { return };
    let dtx = |name: &str| mount.read(&link_path(name)).ok().and_then(|b| Dtx::parse(b).ok());
    for name in world::map_names(&mount) {
        let path = world::find(&mount, &name).unwrap();
        if !(path.starts_with("worlds2/") || path.starts_with("worlds3/")) {
            continue;
        }
        let w = World::parse(&mount.read(&path).unwrap()).unwrap();
        let (mut dual, mut uv_differs, mut tex1_fb, mut tex0_alpharef) = (0, 0, 0, 0);
        let mut links: std::collections::BTreeMap<String, (LinkKind, String, usize)> = Default::default();
        let renders = std::iter::once(&w.render).chain(w.render.models.iter().map(|(_, r)| r));
        for r in renders {
            for b in &r.blocks {
                for (s, tris) in b.section_triangles() {
                    let drawn = [
                        shader::GOURAUD,
                        shader::LIGHTMAP_TEXTURE,
                        shader::DUAL_TEXTURE,
                        shader::LIGHTMAP_DUAL_TEXTURE,
                    ];
                    if !drawn.contains(&s.shader) {
                        continue;
                    }
                    if s.shader == shader::DUAL_TEXTURE || s.shader == shader::LIGHTMAP_DUAL_TEXTURE {
                        dual += 1;
                        let differs = tris.iter().flat_map(|t| t.indices).any(|i| {
                            let v = &b.vertices[i as usize];
                            v.uv0 != v.uv1
                        });
                        uv_differs += differs as usize;
                        tex1_fb += dtx(&s.textures[1]).is_some_and(|d| d.flags & 1 != 0) as usize;
                        tex0_alpharef += dtx(&s.textures[0]).is_some_and(|d| d.alpha_ref().is_some()) as usize;
                    }
                    if let Some(l) = dtx(&s.textures[0]).and_then(|d| d.link()) {
                        let e = links
                            .entry(s.textures[0].to_lowercase())
                            .or_insert((l.kind, l.names[0].clone(), 0));
                        e.2 += 1;
                    }
                }
            }
        }
        eprintln!(
            "{name}: {dual} dual sections ({uv_differs} with UV1 != UV0, {tex1_fb} fullbright texture 1, \
             {tex0_alpharef} alpha-tested texture 0)"
        );
        for (t, (k, l, n)) in links {
            eprintln!("    {t}: {k:?} {l} ({n} sections, resolves: {})", dtx(&l).is_some());
        }
    }
}

/// HGOILRIG (a Hunted map, WORLDS4, V2): its one Team Death Match point
/// has no team, so the map's spawns are its own family's, the 16
/// unlocked `HungerGame` points (game_modes.md S4), not a fallback that
/// drops into the void: none of them sits on the world's outer box
/// (10240 u down). Nothing solid lies under them in the file, only the
/// sea (its sliding water's surface over a `Water` volume): a player
/// spawned at one starts at the water's top, not 190 m down (and floats
/// there: `heavy::movement_volumes`). The rules' spawn list under TDM is the same
/// family's.
#[test]
fn hgoilrig_spawns_on_its_own_family() {
    if !combat_arms::game::mount::keys().is_ok_and(|k| k.contains_key("V2")) {
        eprintln!("skipping: no V2 key in chimera.local.toml");
        return;
    }
    let Some(mount) = mount() else { return };
    let map = world::load_world(&mount, "hgoilrig").unwrap();
    let s = movement::movement_data().meters_per_unit;
    let starts: Vec<&chimera_world::map::MapEntity> =
        map.entities.iter().filter(|e| e.classname() == "GameStartPoint").collect();
    let family = |name: &str| starts.iter().filter(|e| e.get("GameType") == Some(name)).count();
    assert_eq!((family("HungerGame"), family("Team Death Match")), (32, 1));
    assert_eq!(map.spawns.len(), 16, "{:?}", map.spawns);
    for (feet, team) in &map.spawns {
        assert_eq!(*team, None);
        assert!(feet.y > -10000.0 * s, "{feet}: on the outer box");
        assert!(feet.y > -1.0 && feet.y < 10.0, "{feet}");
    }
    let (feet, _) = map.spawns[0];
    let entities = map.entities.clone();
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y * 0.5, movement::ID);
    sim.ticks(2);
    let at = sim.position(p);
    eprintln!("hgoilrig: spawn {feet}, two ticks later at {at}");
    assert!(at.y > feet.y - 2.5, "fell from {feet} to {at}");
    // The rules' list for a TDM player: the HungerGame points too.
    let objects = combat_arms::game::rules::objects::read(&entities, s);
    let own: Vec<Transform> =
        objects.starts.iter().filter(|p| p.qualifies("HungerGame", false)).map(|p| p.at).collect();
    assert_eq!(own.len(), 16);
    let world = sim.app.world_mut();
    world.insert_resource(objects);
    let (list, _) = combat_arms::game::rules::spawns::spawn_list(world, 1);
    assert_eq!(list.len(), 16);
    for t in &list {
        assert!(own.iter().any(|p| p.translation.distance(t.translation) < 1e-3));
    }
}

/// models_ltb.md Follow-up (2026-10-31) 7, R10: `grayhammer`'s fans
/// (`TEXTURES\Objects\sub_ctrlbox\FanSpin01.spr`) are sprite sections
/// that play their 8 frames, 19 a second (52 ms a frame, 416 ms long).
#[test]
fn r10_grayhammer_fans_animate() {
    let Some(mount) = mount() else { return };
    let map = world::load_from(&mount, "grayhammer").unwrap();
    let fans: Vec<_> = map
        .meshes
        .iter()
        .filter(|m| m.material.to_lowercase().ends_with("fanspin01.spr"))
        .collect();
    if fans.is_empty() {
        eprintln!("skipping: the map's world file isn't readable here");
        return;
    }
    for m in &fans {
        let f = m.skin_frames.as_ref().expect("the fan plays");
        assert_eq!((f.frames.len(), f.frame_ms), (8, 52), "8 frames, 19 a second");
    }
    let sprites = map.meshes.iter().filter(|m| m.material.to_lowercase().ends_with(".spr")).count();
    let playing = map.meshes.iter().filter(|m| m.skin_frames.is_some()).count();
    eprintln!("grayhammer: {sprites} sprite sections, {playing} playing");
    assert!(playing >= 1);
}
