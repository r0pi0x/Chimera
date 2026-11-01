//! Each of HL2:DM's 8 stock maps converts (`game::maps`, Source's
//! conversion lifted from CS:S in M1; specs/hl2dm/source_deltas.md,
//! "Stock maps"): its faces (meshes, textures, collision), props, entities
//! (as many as its entity lump holds), lights (lightmap, ambient light
//! field) and spawn points (its `info_player_deathmatch` count), named
//! `hl2dm:<map>`, falling at HL2:DM's gravity. Converted fresh (no map
//! cache). Skips without an install (Steam, `[games.hl2dm] path`, or
//! `$CHIMERA_HL2DM_ROOT`).

use chimera_core::mount::{Mount, served};
use chimera_world::map::{MapData, MapLoadOptions};
use hl2dm::game::{GAME, INFO, STOCK_MAPS, maps, mount};
use source_engine::content::bsp;

pub(crate) fn install_mount() -> Option<Mount> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no HL2:DM install (Steam, [games.hl2dm] path, or $CHIMERA_HL2DM_ROOT)");
        return None;
    };
    if !served::usable(GAME, &path, "hl2mp") {
        eprintln!("skipping: no HL2:DM install at {}", path.display());
        return None;
    }
    Some(mount::open(&path).expect("the install mounts"))
}

/// `name` converted from `mount` with `options`, never from the cache.
pub(crate) fn convert(mount: &Mount, name: &str, options: &MapLoadOptions) -> MapData {
    let bytes = mount.read(&format!("maps/{name}.bsp")).expect("the map's file");
    bsp::load_level_bytes_in(&maps::HL2DM, mount, name, bytes, options, None)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .0
}

/// Each stock map's entities and deathmatch spawns (source_deltas.md's
/// table).
const STOCK: [(&str, usize, usize); 8] = [
    ("dm_lockdown", 648, 33),
    ("dm_overwatch", 364, 17),
    ("dm_powerhouse", 1400, 18),
    ("dm_resistance", 630, 20),
    ("dm_runoff", 608, 9),
    ("dm_steamlab", 1090, 24),
    ("dm_underpass", 878, 17),
    ("halls3", 337, 10),
];

#[test]
fn every_stock_map_converts() {
    let Some(mount) = install_mount() else { return };
    assert_eq!(STOCK_MAPS.len(), STOCK.len());
    for (name, entities, spawns) in STOCK {
        let map = convert(&mount, name, &MapLoadOptions::default());
        let lights = map
            .entities
            .iter()
            .filter(|e| e.classname().starts_with("light"))
            .count();
        let props = map.props.len();
        eprintln!(
            "{name}: {} triangles, {} meshes, {} textures, {} props, {} entities ({lights} lights), {} spawns, \
             {} warnings",
            map.triangle_count(),
            map.meshes.len(),
            map.textures.len(),
            props,
            map.entities.len(),
            map.spawns.len(),
            map.warnings.len()
        );
        for w in &map.warnings {
            eprintln!("  warning: {w}");
        }
        assert_eq!(map.name, format!("hl2dm:{name}"));
        // Faces: drawn, textured, solid.
        assert!(map.triangle_count() > 5_000, "{name}: {} triangles", map.triangle_count());
        assert!(map.meshes.iter().all(|m| !m.positions.is_empty()));
        let textured = map.meshes.iter().filter(|m| m.texture.is_some()).count();
        assert!(textured * 10 >= map.meshes.len() * 9, "{name}: {textured} of {} meshes textured", map.meshes.len());
        assert!(!map.collision_brushes.is_empty() && map.brush_tree.is_some(), "{name}: no collision");
        // Props: static props and prop entities with their models.
        assert!(props > 0 && !map.models.is_empty(), "{name}: no props");
        // Entities: the whole entity lump, worldspawn first.
        assert_eq!(map.entities.len(), entities, "{name}: entities");
        assert_eq!(map.entities[0].classname(), "worldspawn");
        // Lights: the baked lightmap, the light entities, the light field.
        assert!(map.lightmap.is_some(), "{name}: no lightmap");
        assert!(lights > 0, "{name}: no light entities");
        assert!(map.light_field.is_some(), "{name}: no light field");
        // Spawns: every info_player_deathmatch, for anyone.
        assert_eq!(map.spawns.len(), spawns, "{name}: spawns");
        assert!(map.spawns.iter().all(|(_, team)| team.is_none()));
        assert_eq!(map.spawn_yaws.len(), spawns);
        // HL2:DM's gravity (600 units/s²).
        let g = map.gravity.expect("a gravity");
        assert!((g - 600.0 * 0.0254).abs() < 1e-4, "{name}: gravity {g}");
        // Every material resolves (source_deltas.md 1.11: none missing).
        let missing: Vec<_> = map.warnings.iter().filter(|w| w.contains("missing")).collect();
        assert!(missing.is_empty(), "{name}: {missing:#?}");
    }
}

/// The dedicated server's load (no textures) gives the same world.
#[test]
fn a_lean_load_has_the_same_world() {
    let Some(mount) = install_mount() else { return };
    let full = convert(&mount, "halls3", &MapLoadOptions::default());
    let lean = convert(&mount, "halls3", &MapLoadOptions::lean());
    assert_eq!(full.triangle_count(), lean.triangle_count());
    assert_eq!(full.entities.len(), lean.entities.len());
    assert_eq!(full.spawns, lean.spawns);
    assert_eq!(full.props.len(), lean.props.len());
}
