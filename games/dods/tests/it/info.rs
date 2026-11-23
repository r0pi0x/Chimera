//! What DoD:S tells the shell (`game::INFO`) and what it lends
//! (`game::parts::PARTS`): its id, folder, tick, Steam app, namespaces,
//! its spawn classes by team (specs/dods/install.md).

use chimera_core::core::Team;
use dods::game::{BUNDLE, GAME, INFO, maps, movement, parts::PARTS, weapons};

#[test]
fn info_is_the_spec_s() {
    assert_eq!(INFO.id, "dods");
    assert_eq!(INFO.content_dir, "dod");
    assert_eq!(INFO.tick_interval, 0.015);
    assert_eq!(INFO.steam.map(|s| s.app_id), Some(300));
    assert_eq!(INFO.console_namespaces, &["dods", "dod"]);
    assert_eq!(INFO.map_id("dod_avalanche"), "dods:dod_avalanche");
    assert!(std::ptr::eq(BUNDLE.info, &INFO));
}

#[test]
fn allies_and_axis_spawn_as_the_two_sides() {
    assert_eq!(maps::SPAWNS, &[("info_player_allies", Some(Team(1))), ("info_player_axis", Some(Team(2)))]);
}

#[test]
fn it_lends_a_gun_and_its_movement() {
    assert_eq!(PARTS.game, GAME);
    assert!(PARTS.playable());
    assert_eq!(PARTS.movement, Some(movement::ID));
    let set = weapons::dods_weapons();
    for g in weapons::GUNS {
        let w = set.weapons.iter().find(|w| w.id == *g).unwrap_or_else(|| panic!("{g} in the data"));
        assert!(w.models.view.is_some() && w.models.world.is_some(), "{g}'s models");
    }
}
