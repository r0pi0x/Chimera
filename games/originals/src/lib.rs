//! Lucker Party's original minigames (games/originals, crate `originals`;
//! docs/plans/active/architecture-review.md D2, A5): their definitions,
//! our own data files in the repository's `minigames/` directory
//! (`MINIGAMES`, compiled in and parsed at run time), and the rules hooks
//! an original needs where Chimera's built-in modules (`chimera_sim::
//! minigame`: `chimera:ladder`) don't cover it, registered by name under
//! `originals:` (none yet: Gun Game is the ladder's).
//!
//! - `originals:gun_game_dust2` (minigames/gun_game_dust2.toml): Gun Game
//!   on de_dust2 (or Warehouse), a ladder of CS:S's and Combat Arms' guns,
//!   Combat Arms' soldiers, movement, HUD and tick.
//! - `originals:gun_game_2fort` (minigames/gun_game_2fort.toml): Gun Game
//!   on TF2's ctf_2fort (then de_dust2), a ladder mixing TF2's four guns
//!   with CS:S's, HL2:DM's and Combat Arms' (no Halo).
//!
//! It uses Chimera only: the games' parts a definition names (weapons,
//! movement, HUD) are ids the app's registered games fill
//! (checks/tests/it/architecture.rs). `OriginalsPlugin` adds them to an
//! app that has `chimera_sim::minigame::MinigamePlugin`.

use bevy::prelude::*;
use chimera_sim::minigame::MinigameAppExt;

/// The original minigames' definitions, `(file, text)`: every file in
/// `minigames/` (a test checks the list against the directory).
pub const MINIGAMES: &[(&str, &str)] = &[
    ("minigames/gun_game_dust2.toml", include_str!("../../../minigames/gun_game_dust2.toml")),
    ("minigames/gun_game_2fort.toml", include_str!("../../../minigames/gun_game_2fort.toml")),
];

/// Gun Game on de_dust2.
pub const GUN_GAME_DUST2: &str = "originals:gun_game_dust2";

/// Gun Game on 2Fort: TF2's guns among the others' (no Halo).
pub const GUN_GAME_2FORT: &str = "originals:gun_game_2fort";

/// The original minigames (and their rules hooks, when one needs its own).
pub struct OriginalsPlugin;

impl Plugin for OriginalsPlugin {
    fn build(&self, app: &mut App) {
        app.add_minigames(MINIGAMES)
            .unwrap_or_else(|e| panic!("an original minigame's definition: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_sim::minigame::MinigameDef;

    /// Every definition reads, its id is `originals:<file name>`, and every
    /// file in minigames/ is in `MINIGAMES`.
    #[test]
    fn every_definition_reads_and_is_listed() {
        let mut ids = Vec::new();
        for (file, text) in MINIGAMES {
            let def = MinigameDef::parse(file, text).unwrap();
            let stem = file.trim_start_matches("minigames/").trim_end_matches(".toml");
            assert_eq!(def.id, format!("originals:{stem}"), "{file}");
            ids.push(def.id);
        }
        assert!(ids.iter().any(|i| i == GUN_GAME_DUST2) && ids.iter().any(|i| i == GUN_GAME_2FORT));
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../minigames");
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| format!("minigames/{}", e.file_name().to_string_lossy()))
            .filter(|f| f.ends_with(".toml"))
            .collect();
        files.sort();
        let mut listed: Vec<String> = MINIGAMES.iter().map(|(f, _)| f.to_string()).collect();
        listed.sort();
        assert_eq!(files, listed, "minigames/ and MINIGAMES differ");
    }

    #[test]
    fn gun_game_is_the_ladder_on_dust2_mixing_both_games_guns() {
        let def = MinigameDef::parse(MINIGAMES[0].0, MINIGAMES[0].1).unwrap();
        assert_eq!(def.rules.module, "chimera:ladder");
        assert_eq!(def.maps[0], "cs_source:de_dust2");
        let ladder = def.rules.params["ladder"].as_array().unwrap();
        for game in ["cs_source:", "combat_arms:"] {
            assert!(ladder.iter().any(|w| w.as_str().unwrap().starts_with(game)), "{game} guns");
        }
    }

    /// TF2's four guns among CS:S's, HL2:DM's and Combat Arms', no Halo.
    #[test]
    fn gun_game_2fort_mixes_tf2_rungs_without_halo() {
        let def = MinigameDef::parse(MINIGAMES[1].0, MINIGAMES[1].1).unwrap();
        assert_eq!(def.id, GUN_GAME_2FORT);
        assert_eq!(def.maps[0], "tf2:ctf_2fort");
        let ladder: Vec<&str> = def.rules.params["ladder"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
        assert_eq!(ladder.iter().filter(|w| w.starts_with("tf2:")).count(), 4);
        for game in ["cs_source:", "combat_arms:", "hl2dm:"] {
            assert!(ladder.iter().any(|w| w.starts_with(game)), "{game} guns");
        }
        assert!(!ladder.iter().any(|w| w.starts_with("halo_ce:")), "no Halo");
    }
}
