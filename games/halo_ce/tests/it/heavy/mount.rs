//! Halo's install mounted from the user's disc image (`game::mount`,
//! specs/halo_ce/disc_image.md). Each test passes with a note without an
//! install (`$CHIMERA_HALO_CE_ROOT`, or `[games.halo_ce] path`).

use std::path::PathBuf;

use chimera_core::mount::served;
use halo_ce::game::{GAME, INFO, MULTIPLAYER_MAPS};

/// The install (a disc image file or a folder), if there is one.
fn install() -> Option<PathBuf> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return None;
    };
    if !served::usable(GAME, &path, "") {
        eprintln!("skipping: no Halo CE install at {}", path.display());
        return None;
    }
    Some(path)
}

#[test]
fn the_install_mounts_and_lists_blood_gulch() {
    let Some(path) = install() else { return };
    let mount = (INFO.open)(&path).unwrap();
    let entry = mount
        .entries()
        .into_iter()
        .map(|(e, _)| e)
        .find(|e| e.path == "maps/bloodgulch.map")
        .expect("maps/bloodgulch.map is listed");
    assert_eq!(entry.size, 22_571_008);
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    assert_eq!(bytes.len() as u64, 22_571_008);
}

#[test]
fn every_multiplayer_map_is_there() {
    let Some(path) = install() else { return };
    if INFO.served_partial() {
        eprintln!("skipping: only some sets are served here");
        return;
    }
    let mount = (INFO.open)(&path).unwrap();
    assert_eq!((INFO.map_names)(&mount).len(), MULTIPLAYER_MAPS.len());
    assert_eq!((INFO.map_cycle)(&mount), MULTIPLAYER_MAPS);
}
