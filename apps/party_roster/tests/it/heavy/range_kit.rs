//! The range's kit from the installs (`map::sources::CODE_MAP_KIT` in the
//! party's `MAP_SOURCES`): loading `chimera:range` dresses it with the
//! installed games' bodies, so targets and bots on the sandbox's range
//! are drawn (the greybox's too). Passes with a note without the CS:S and
//! Combat Arms installs.

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::MapLoadOptions;

#[test]
fn the_range_is_dressed_with_the_games_bodies() {
    let Ok(config) = LocalConfig::load() else {
        eprintln!("skipping: no local config");
        return;
    };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config
        .game_path(combat_arms::game::GAME)
        .filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    for id in ["chimera:range", "chimera:greybox"] {
        let kit = party_roster::load_map_with(id, &MapLoadOptions::lean()).unwrap_or_else(|e| panic!("{id}: {e}"));
        let games: Vec<&str> = kit.kit.characters.iter().filter_map(|c| c.game).collect();
        assert!(games.contains(&"combat_arms"), "{id}: Combat Arms' bodies, got {games:?} ({:?})", kit.warnings);
        assert!(games.contains(&"cs_source"), "{id}: CS:S's bodies, got {games:?} ({:?})", kit.warnings);
    }
}
