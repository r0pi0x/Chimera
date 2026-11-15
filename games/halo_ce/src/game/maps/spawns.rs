//! The scenario's player starting locations as the map's spawns
//! (specs/halo_ce/scenario.md, "Player starting locations", "Game
//! types"): each start's position is where the player's feet appear
//! (`MapData::spawns`, metres through `space::point`), its facing a
//! heading (`MapData::spawn_yaws` through `space::yaw`).
//!
//! The map's spawns are the starts eligible for free-for-all slayer
//! (`DEFAULT_GAME`, the spec's eligibility rule: `GameTypes::eligible_for`),
//! in block order. A start's team counts only where the game type tests
//! it (capture the flag: team index 0 red is `Team(1)`, 1 blue
//! `Team(2)`, as Combat Arms numbers its two teams; other indices none);
//! slayer does not, so the default set has no teams. A map with no
//! eligible start falls back to every start, and one with none at all to
//! the origin, each with a warning.

use blam_formats::scenario::{GameType, PlayerStart, Scenario};

use super::space;
use crate::{core::Team, map::MapData};

/// The game type whose starts are the map's spawns.
pub const DEFAULT_GAME: GameType = GameType::Slayer;

/// A start's team in game type `game`: only capture the flag tests it.
pub fn team(start: &PlayerStart, game: GameType) -> Option<Team> {
    if game != GameType::CaptureTheFlag {
        return None;
    }
    match start.team_index {
        0 => Some(Team(1)),
        1 => Some(Team(2)),
        _ => None,
    }
}

/// The starts of `game`, as (feet in metres, team, engine yaw).
pub fn starts(scenario: &Scenario, game: GameType) -> Vec<(bevy::math::Vec3, Option<Team>, f32)> {
    scenario
        .eligible_starts(game)
        .into_iter()
        .map(|i| {
            let s = &scenario.player_starts[i];
            (space::point(s.position), team(s, game), space::yaw(s.facing))
        })
        .collect()
}

/// Fill `map`'s spawns from the scenario (module doc).
pub fn add(scenario: &Scenario, map: &mut MapData) {
    let mut chosen = starts(scenario, DEFAULT_GAME);
    if chosen.is_empty() && !scenario.player_starts.is_empty() {
        map.warnings.push(format!(
            "no player start is eligible for {DEFAULT_GAME:?}: every start ({}) spawns",
            scenario.player_starts.len()
        ));
        chosen = scenario
            .player_starts
            .iter()
            .map(|s| (space::point(s.position), None, space::yaw(s.facing)))
            .collect();
    }
    if chosen.is_empty() {
        map.warnings.push("no player starting location: spawning at the origin".into());
        chosen.push((bevy::math::Vec3::ZERO, None, 0.0));
    }
    for (p, team, yaw) in chosen {
        map.spawns.push((p, team));
        map.spawn_yaws.push(yaw);
    }
}
