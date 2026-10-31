//! Combat Arms' install mounted as the shell mounts it (`INFO.open`,
//! `game::mount`). Passes with a note without `[games.combat_arms] path`
//! in chimera.local.toml.

use std::io::ErrorKind;

use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{GAME, INFO, mount};

#[test]
fn install_mounts_its_archives() {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return;
    }
    if !install.join("Game").is_dir() {
        eprintln!("skipping: checks the install's own archives (served from the asset store here)");
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    // The install's own archives (Steam build 26236938: 108 `.rez`); the
    // shell's mount may have the asset store layered under them, one more
    // layer, when the store holds the install's build (`mount::served`).
    let own = mount::open_install(&install).unwrap().layers().count();
    assert_eq!(own, 108);
    let all = mount.layers().count();
    assert!(all == own || all == own + 1, "the install's {own} layers and at most the store: {all}");
    assert!(mount.read("worlds2/warehouse.dat").unwrap().len() > 1_000_000);
    let keyed = mount.read("worlds/junk_flea.dat");
    if !mount::keys().unwrap().contains_key("V2") {
        assert_eq!(keyed.unwrap_err().kind(), ErrorKind::PermissionDenied);
    }
}

/// The shell's map list (`GameInfo::map_names`) finds the install's maps.
#[test]
fn install_lists_its_maps_for_the_console() {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let names = (INFO.map_names)(&mount);
    assert!(names.iter().any(|n| n == "warehouse"), "{} maps, no warehouse", names.len());
}
