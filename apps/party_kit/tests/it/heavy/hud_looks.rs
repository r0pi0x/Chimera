//! The party's maps carry every game's HUD look (the installs'): each map's
//! own, with the other Source game's as a guest's beside it, so a gun's game
//! never draws its HUD in another game's look. Passes with a note without
//! the installs.

use chimera_core::mount::config::LocalConfig;

#[test]
fn every_map_carries_each_games_look() {
    let config = LocalConfig::load().ok();
    let usable = config.is_some_and(|c| {
        c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
            .is_some_and(|p| chimera_core::mount::served::usable("cs_source", &p, "cstrike"))
            && c.game_path(combat_arms::game::GAME).is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
    }) && hl2dm::game::INFO.install().is_some();
    if !usable {
        eprintln!("skipping: needs the CS:S, Combat Arms and HL2:DM installs");
        return;
    }
    for (map, own, guest) in [
        ("cs_source:de_dust2", cs_source::game::GAME, hl2dm::game::GAME),
        ("hl2dm:dm_lockdown", hl2dm::game::GAME, cs_source::game::GAME),
        ("combat_arms:warehouse", cs_source::game::GAME, hl2dm::game::GAME),
    ] {
        let data = party_kit::load_map(map, 0).unwrap();
        assert_eq!(data.kit.hud.as_ref().map(|h| h.game.as_str()), Some(own), "{map}");
        let guests: Vec<&str> = data.kit.guest_huds.iter().map(|h| h.game.as_str()).collect();
        assert_eq!(guests, [guest], "{map}");
    }
}
