//! `map_fitness --write` from a sweep of some games (`party_roster::
//! fitness_data::write`): the games with rows in the CSV get their
//! `data/map_fitness.toml` written; a game the CSV has no rows of keeps
//! its file byte for byte (it was emptied to a header with no maps).

use party_roster::fitness_data::{self, WANDER_PLAYERS};

/// A sweep's CSV with one row of `game`'s map `map`.
fn csv(game: &str, map: &str) -> String {
    let wander: Vec<String> = WANDER_PLAYERS.iter().map(|n| format!("wander_{n}_s")).collect();
    let header = format!("game,map,loads,area_m2,sight_m,spot_area_m2,spawns,spots,spots_off,stuck,{}", wander.join(","));
    let values = vec!["30.0"; WANDER_PLAYERS.len()].join(",");
    format!("{header}\n{game},{map},true,1200,14.5,800,20,24,0,0,{values}\n")
}

#[test]
fn write_leaves_games_without_rows_as_they_are() {
    let repo = std::env::temp_dir().join(format!("chimera_fitness_write_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    let file = |game: &str| repo.join("games").join(game).join("data").join("map_fitness.toml");
    for game in ["cs_source", "combat_arms"] {
        std::fs::create_dir_all(file(game).parent().unwrap()).unwrap();
    }
    let kept = "speed_mps = 6.35\n\n[maps.warehouse]\narea_m2 = 900\n";
    std::fs::write(file("combat_arms"), kept).unwrap();
    std::fs::write(file("cs_source"), "speed_mps = 6.35\n").unwrap();

    let games = ["cs_source".to_string(), "combat_arms".to_string()];
    let written = fitness_data::write(&csv("cs_source", "de_dust2"), &repo, &games, 6.35).unwrap();
    let cs = std::fs::read_to_string(file("cs_source")).unwrap();
    let ca = std::fs::read_to_string(file("combat_arms")).unwrap();
    let _ = std::fs::remove_dir_all(&repo);

    assert_eq!(written.iter().map(|w| (w.game.as_str(), w.maps)).collect::<Vec<_>>(), [("cs_source", Some(1)), ("combat_arms", None)]);
    assert!(cs.contains("[maps.de_dust2]") && cs.contains("area_m2 = 1200"), "{cs}");
    assert_eq!(ca, kept, "the game the sweep didn't cover keeps its file");
}
