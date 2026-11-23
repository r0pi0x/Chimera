//! DoD:S from the user's install (specs/dods/install.md): it mounts from
//! its `dod/gameinfo.txt` (the dedicated server's layout without
//! `dod_english.vpk` too), lists the nine stock maps, each a BSP 20 that
//! converts (lean: no textures) with the Allies' and the Axis' spawns
//! (dod_avalanche and dod_flash 20 each), and every class body loads (MDL
//! 44, its sequences from the shared model) with a head box; the Colt's
//! view and held models load. Converted fresh (no map cache). Each test
//! passes with a note without an install (Steam, `[games.dods] path`, or
//! `$CHIMERA_DODS_ROOT`).

use std::path::PathBuf;

use chimera_core::{
    core::{Hitgroup, Team},
    mount::{Mount, served},
};
use chimera_world::map::{GameKit, KitLoad, MapLoadOptions};
use dods::game::{GAME, INFO, STOCK_MAPS, bodies, maps, mount, weapons};
use source_engine::content::{bsp, material::MaterialLoader, surfaceprops::SurfaceProps};

/// The install, if usable.
fn install() -> Option<PathBuf> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no DoD:S install (Steam, [games.dods] path, or $CHIMERA_DODS_ROOT)");
        return None;
    };
    if !served::usable(GAME, &path, "dod") {
        eprintln!("skipping: no DoD:S install at {}", path.display());
        return None;
    }
    Some(path)
}

fn install_mount() -> Option<Mount> {
    Some(mount::open(&install()?).expect("the install mounts"))
}

#[test]
fn it_mounts_and_lists_the_nine_stock_maps() {
    let Some(mount) = install_mount() else { return };
    assert!(mount.read("scripts/weapon_garand.txt").is_ok(), "a script from dod_pak");
    assert!(mount.read("scripts/playerclass_us_garand.txt").is_ok(), "a class script");
    let names = (INFO.map_names)(&mount);
    for m in STOCK_MAPS {
        assert!(names.iter().any(|n| n == m), "{m} in {names:?}");
    }
}

#[test]
fn each_stock_map_converts_with_both_teams_spawns() {
    let Some(mount) = install_mount() else { return };
    for name in STOCK_MAPS {
        let bytes = mount.read(&format!("maps/{name}.bsp")).unwrap_or_else(|e| panic!("{name}'s file: {e}"));
        assert_eq!(&bytes[..4], b"VBSP", "{name}");
        assert_eq!(i32::from_le_bytes(bytes[4..8].try_into().unwrap()), 20, "{name}: BSP version");
        let map = bsp::load_level_bytes_in(&maps::DODS, &mount, name, bytes, &MapLoadOptions::lean(), None)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
            .0;
        let count = |t: u8| map.spawns.iter().filter(|(_, team)| *team == Some(Team(t))).count();
        let (allies, axis) = (count(1), count(2));
        eprintln!(
            "{name}: {} triangles, {} props, {} entities, spawns {allies} + {axis}, {} warnings",
            map.triangle_count(),
            map.props.len(),
            map.entities.len(),
            map.warnings.len()
        );
        for w in map.warnings.iter().take(4) {
            eprintln!("  {w}");
        }
        assert!(map.triangle_count() > 10_000, "{name}: a world");
        assert!(allies > 0 && axis > 0, "{name}: spawns {allies} + {axis}");
        if matches!(*name, "dod_avalanche" | "dod_flash") {
            assert_eq!((allies, axis), (20, 20), "{name}: install.md's spawns");
        }
        // The kit: the class bodies.
        assert!(map.kit.characters.iter().filter(|c| c.game == Some(GAME)).count() == bodies::CLASSES.len(), "{name}: bodies");
    }
}

#[test]
fn every_class_body_loads_with_sequences_and_a_head() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    for c in &bodies::CLASSES {
        let bytes = materials.read(c.model).unwrap_or_else(|| panic!("{}: not in the mount", c.model));
        assert_eq!(i32::from_le_bytes(bytes[4..8].try_into().unwrap()), 44, "{}: MDL version", c.model);
        let body = bodies::load(&mut materials, &surfaces, c.model).unwrap_or_else(|e| panic!("{}: {e}", c.model));
        assert_eq!(body.game, Some(GAME));
        assert_eq!(body.name.is_none(), c.model == bodies::DEFAULT_MODEL, "{}: named unless the default", c.model);
        assert!(!body.model.meshes.is_empty(), "{}: meshes", c.model);
        let set = body.animations.as_ref().unwrap_or_else(|| panic!("{}: no animations", c.model));
        assert!(set.sequences.len() > 10, "{}: {} sequences (the shared model's)", c.model, set.sequences.len());
        assert!(body.boxes.iter().any(|b| b.group == Hitgroup::Head), "{}: a head box", c.model);
    }
}

#[test]
fn the_colt_s_models_load_on_the_body() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    let (mut kit, mut warnings) = (GameKit::default(), Vec::new());
    let wants = KitLoad::Players(&["dods:"]);
    bodies::add_bodies(&mut materials, &surfaces, wants, &mut kit, &mut warnings);
    weapons::add_guns(&mut materials, &surfaces, &|g| wants.wants(g), &mut kit, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(kit.characters.len(), bodies::CLASSES.len());
    assert!(kit.view_models.iter().any(|v| v.key == weapons::COLT), "the Colt's view model");
    assert!(kit.held.iter().any(|h| h.key == weapons::COLT), "the Colt held");
}
