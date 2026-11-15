//! ctf_2fort and cp_badlands convert (`game::maps`, Source's conversion
//! with TF2's data; specs/tf2/install.md 2.1 to 2.3): LZMA lumps, the
//! LZMA static-prop lump (version 10) and the LZMA pak read; faces
//! (meshes, textures, collision), displacements (drawn with their blend,
//! collided as triangles), the lightmap, the 2D and 3D sky, props, every
//! entity of the lump, spawns from `info_player_teamspawn` for both
//! teams, TF2's gravity, and on the client install no missing material.
//! Converted fresh (no map cache). Skips without an install (Steam,
//! `[games.tf2] path`, or `$CHIMERA_TF2_ROOT`).
//!
//! The dedicated server's layout (no texture archives) is checked when
//! `$CHIMERA_TF2_SERVER_ROOT` names one: the same world, its stock
//! textures missing (stand-ins).

use std::path::PathBuf;

use chimera_core::{
    core::Team,
    mount::{Mount, served},
};
use chimera_world::map::{MapData, MapLoadOptions};
use source_engine::content::bsp;
use tf2::game::{GAME, INFO, maps, mount};

/// The install, if usable.
pub(crate) fn install() -> Option<PathBuf> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no TF2 install (Steam, [games.tf2] path, or $CHIMERA_TF2_ROOT)");
        return None;
    };
    if !served::usable(GAME, &path, "tf") {
        eprintln!("skipping: no TF2 install at {}", path.display());
        return None;
    }
    Some(path)
}

pub(crate) fn install_mount() -> Option<Mount> {
    Some(mount::open(&install()?).expect("the install mounts"))
}

/// `name` converted from `mount` with `options`, never from the cache.
pub(crate) fn convert(mount: &Mount, name: &str, options: &MapLoadOptions) -> MapData {
    let bytes = mount.read(&format!("maps/{name}.bsp")).expect("the map's file");
    bsp::load_level_bytes_in(&maps::TF2, mount, name, bytes, options, None)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .0
}

/// Each map's entities and RED and BLU spawns at round start
/// (install.md 2.2, 2.3).
const MAPS: [(&str, usize, usize, usize); 2] = [("ctf_2fort", 1097, 15, 15), ("cp_badlands", 674, 16, 16)];

fn summary(name: &str, map: &MapData) {
    let blended = map.meshes.iter().filter(|m| m.blend.is_some()).count();
    eprintln!(
        "{name}: {} triangles, {} meshes ({blended} blended), {} textures, {} props, {} entities, {} spawns, \
         {} collision triangles, {} hulls, {} warnings",
        map.triangle_count(),
        map.meshes.len(),
        map.textures.len(),
        map.props.len(),
        map.entities.len(),
        map.spawns.len(),
        map.collision_indices.len(),
        map.collision_hulls.len(),
        map.warnings.len()
    );
}

/// The world of `map` as both layouts give it: faces, collision,
/// displacements, lightmap, the 3D sky's camera, props, entities, spawns,
/// gravity.
fn check_world(name: &str, map: &MapData, entities: usize, red: usize, blu: usize) {
    assert_eq!(map.name, format!("tf2:{name}"));
    // Faces: drawn and solid.
    assert!(map.triangle_count() > 10_000, "{name}: {} triangles", map.triangle_count());
    assert!(map.meshes.iter().all(|m| !m.positions.is_empty()));
    assert!(!map.collision_brushes.is_empty() && map.brush_tree.is_some(), "{name}: no collision");
    assert!(map.collision_hulls.len() > 500, "{name}: {} hulls", map.collision_hulls.len());
    // Displacements: collided as triangles, drawn with their blend
    // (WorldVertexTransition, install.md 3.1).
    assert!(!map.collision_indices.is_empty(), "{name}: displacements don't collide");
    assert!(map.meshes.iter().any(|m| m.blend.is_some()), "{name}: no blended displacement");
    // Light: the baked lightmap and the light field.
    assert!(map.lightmap.is_some(), "{name}: no lightmap");
    assert!(map.light_field.is_some(), "{name}: no light field");
    // The 3D skybox's camera (scale 16).
    assert!(map.sky_camera.is_some(), "{name}: no 3D sky camera");
    // Static props (version 10, LZMA) and their models.
    assert!(map.props.len() > 500 && !map.models.is_empty(), "{name}: {} props", map.props.len());
    // Entities: the whole (LZMA) entity lump, worldspawn first.
    assert_eq!(map.entities.len(), entities, "{name}: entities");
    assert_eq!(map.entities[0].classname(), "worldspawn");
    // Spawns: RED as our team 1, BLU as 2, nobody's else.
    let count = |t: u8| map.spawns.iter().filter(|(_, team)| *team == Some(Team(t))).count();
    assert_eq!((count(1), count(2)), (red, blu), "{name}: spawns");
    assert_eq!(map.spawns.len(), red + blu, "{name}: only team spawns");
    assert_eq!(map.spawn_yaws.len(), map.spawns.len());
    // TF2's gravity (800 units/s²).
    let g = map.gravity.expect("a gravity");
    assert!((g - 800.0 * 0.0254).abs() < 1e-4, "{name}: gravity {g}");
}

#[test]
fn ctf_2fort_and_cp_badlands_convert() {
    let Some(path) = install() else { return };
    let mount = mount::open(&path).expect("the install mounts");
    let client = mount::full_content(&path) || !path.join("tf").is_dir();
    for (name, entities, red, blu) in MAPS {
        let map = convert(&mount, name, &MapLoadOptions::default());
        summary(name, &map);
        for w in &map.warnings {
            eprintln!("  warning: {w}");
        }
        check_world(name, &map, entities, red, blu);
        // The 2D sky (its textures from tf2_textures, or the store).
        assert!(map.sky.is_some(), "{name}: no sky");
        // Self-shadowed bump maps (`$ssbump`: TF2's concrete and rock).
        assert!(map.meshes.iter().any(|m| m.ssbump && m.normal_map.is_some()), "{name}: no $ssbump surface");
        // Textured (the map's pak, tf2_misc and hl2_misc: install.md 3.1).
        let textured = map.meshes.iter().filter(|m| m.texture.is_some()).count();
        assert!(textured * 10 >= map.meshes.len() * 9, "{name}: {textured} of {} meshes textured", map.meshes.len());
        if client {
            // Every material and texture resolves on the client's install
            // (install.md 3.1: none missing).
            let missing: Vec<_> =
                map.warnings.iter().filter(|w| w.contains("missing") || w.contains("not found")).collect();
            assert!(missing.is_empty(), "{name}: {missing:#?}");
        }
    }
}

/// The dedicated server's load (no textures) gives the same world.
#[test]
fn a_lean_load_has_the_same_world() {
    let Some(mount) = install_mount() else { return };
    let full = convert(&mount, "ctf_2fort", &MapLoadOptions::default());
    let lean = convert(&mount, "ctf_2fort", &MapLoadOptions::lean());
    assert_eq!(full.triangle_count(), lean.triangle_count());
    assert_eq!(full.entities.len(), lean.entities.len());
    assert_eq!(full.spawns, lean.spawns);
    assert_eq!(full.props.len(), lean.props.len());
    assert_eq!(full.collision_indices.len(), lean.collision_indices.len());
}

/// The dedicated server's layout, if `$CHIMERA_TF2_SERVER_ROOT` names
/// one: its texture archives are missing, so it mounts without them and
/// ctf_2fort converts with the same world, the stock textures missing
/// (stand-ins) and with them the 2D sky.
#[test]
fn the_server_layout_converts_with_stand_ins() {
    let Some(path) = std::env::var_os("CHIMERA_TF2_SERVER_ROOT").map(PathBuf::from) else {
        eprintln!("skipping: no $CHIMERA_TF2_SERVER_ROOT (a dedicated server's layout)");
        return;
    };
    assert!(!mount::full_content(&path), "{}: not the server's layout", path.display());
    let mount = mount::open_install(&path).expect("the server's layout mounts");
    let (name, entities, red, blu) = MAPS[0];
    let map = convert(&mount, name, &MapLoadOptions::default());
    summary(name, &map);
    check_world(name, &map, entities, red, blu);
    let missing = map.warnings.iter().filter(|w| w.contains("missing") || w.contains("not found")).count();
    eprintln!("{name}: {missing} missing on the server's layout");
    assert!(missing > 0, "the server has no stock textures");
}
