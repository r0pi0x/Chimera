//! The user's install (`~/games/cs16`, Steam, `[games.cs16] path` or
//! `$CHIMERA_CS16_ROOT`; `mount::install`): it
//! mounts as `cstrike` then `valve` (specs/goldsrc/mount.md), with file
//! names matched ignoring case. Passes with a note without one. The
//! formats themselves are tested on the install in `goldsrc_formats`.

use cs16::game::{INFO, mount};

#[test]
fn the_install_mounts_cstrike_then_valve() {
    let Some(install) = INFO.install() else {
        eprintln!("no Counter-Strike 1.6 install (~/games/cs16, Steam, config or CHIMERA_CS16_ROOT): skipped");
        return;
    };
    assert_eq!(mount::folder_chain(&install), ["cstrike", "valve"]);
    let m = mount::open_install(&install).unwrap_or_else(|e| panic!("{}: {e}", install.display()));
    let layer = |p: &str| m.layer_of(&chimera_core::mount::normalize(p)).map(|(i, _)| i);
    assert_eq!(layer("maps/de_dust2.bsp"), Some(0));
    assert_eq!(layer("gfx/env/desup.tga"), Some(0), "found ignoring case (Desup.tga)");
    assert_eq!(layer("decals.wad"), Some(0), "cstrike's shadows valve's");
    assert_eq!(layer("halflife.wad"), Some(1));
    assert_eq!(
        m.files_in("maps").iter().filter(|f| f.ends_with(".bsp")).count() >= 25,
        true
    );
}

/// An install in the games folder (`~/games/cs16`) is found with no
/// variable or config path: heavy tests in an agent's slot or a
/// post-push run must not skip for want of `CHIMERA_CS16_ROOT`.
#[test]
fn a_games_folder_install_is_found_without_a_variable() {
    use chimera_core::mount::install;
    let sg = INFO.steam.as_ref().unwrap();
    let Some(found) = install::find_by_convention(&install::games_dirs(), sg, false).install else {
        eprintln!("no Counter-Strike 1.6 in a games folder: skipped");
        return;
    };
    assert!(
        INFO.install().is_some(),
        "{} is in the games folder but no install resolved",
        found.path.display()
    );
}
