//! The user's install (`[games.cs16] path` or `$CHIMERA_CS16_ROOT`): it
//! mounts as `cstrike` then `valve` (specs/goldsrc/mount.md), with file
//! names matched ignoring case. Passes with a note without one. The
//! formats themselves are tested on the install in `goldsrc_formats`.

use cs16::game::{INFO, mount};

#[test]
fn the_install_mounts_cstrike_then_valve() {
    let Some(install) = INFO.install() else {
        eprintln!("no Counter-Strike 1.6 install ([games.cs16] path or CHIMERA_CS16_ROOT): skipped");
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
