//! What TF2 tells the shell (`game::INFO`), with no install.

use chimera_core::mount::install;
use tf2::game::{GAME, INFO};

#[test]
fn names_its_game_folder_app_and_namespaces() {
    assert_eq!(INFO.id, GAME);
    assert_eq!(GAME, "tf2");
    assert_eq!(INFO.short_name, "TF2");
    assert_eq!(INFO.content_dir, "tf");
    let steam = INFO.steam.expect("a Steam app");
    assert_eq!((steam.app_id, steam.name), (440, "Team Fortress 2"));
    assert_eq!(steam.required, ["tf/tf2_misc_dir.vpk", "hl2/hl2_misc_dir.vpk"]);
    assert_eq!(INFO.namespaces(), ["tf2", "tf"]);
    assert_eq!(INFO.map_id("ctf_2fort"), "tf2:ctf_2fort");
    assert_eq!((INFO.map_file)("cp_badlands").as_deref(), Some("maps/cp_badlands.bsp"));
    assert_eq!(install::root_env(GAME), "CHIMERA_TF2_ROOT");
    assert!((INFO.meters_per_unit - 0.0254).abs() < 1e-9);
    assert!((INFO.tick_interval - 0.015).abs() < 1e-12);
    assert!(!INFO.movement_cvars.is_empty(), "Source's movement cvars");
}

#[test]
fn the_bundle_is_its_maps_only() {
    let b = &tf2::game::BUNDLE;
    assert!(std::ptr::eq(b.info, &INFO));
    assert_eq!(b.loader.game, GAME);
    assert!(b.parts.guns.is_empty() && b.parts.maps.is_empty(), "no rungs or pool maps yet (M2)");
    assert!(b.guest.is_none() && b.kept.is_empty());
}
