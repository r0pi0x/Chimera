//! The scenario's player starting locations as the map's spawns
//! (specs/halo_ce/scenario.md, "Player starting locations", "Game
//! types"; multiplayer_rules.md 4.3): each start's position is where the
//! player's feet appear (`MapData::spawns`, metres through
//! `space::point`), its facing a heading (`MapData::spawn_yaws` through
//! `space::yaw`).
//!
//! The map's spawns are, in block order, each start that serves free-for-all
//! slayer (`DEFAULT_GAME`, the spec's eligibility rule:
//! `GameTypes::eligible_for`) or a team in capture the flag
//! (`TEAM_GAME`), once each:
//!
//! - a capture the flag start of team index 0 (red) is `Team(1)`, of 1
//!   (blue) `Team(2)`, as Combat Arms numbers its two teams (and as an app
//!   composing Halo's maps maps them, `map::SpawnTeams`), unless the game's
//!   flag check drops it (4.3: nearer its enemy's flag spot than its own);
//!   indices 2 and 3 are never a capture the flag start;
//! - every other start (slayer's, whose team the game ignores) has no
//!   team.
//!
//! So a free-for-all gameplay takes every spawn (the slayer starts and the
//! team bases'), a two-team one each team's own (`Team`), as the disc's
//! capture the flag does. The slayer starts alone are `starts(..,
//! DEFAULT_GAME)`. A map with no eligible start falls back to every start,
//! and one with none at all to the origin, each with a warning.

use blam_formats::scenario::{GameType, NetgameFlag, NetgameFlagType, PlayerStart, Scenario};

use super::space;
use crate::{core::Team, map::MapData};

/// The free-for-all game type whose starts are among the map's spawns.
pub const DEFAULT_GAME: GameType = GameType::Slayer;

/// The team game type whose starts are the map's team spawns.
pub const TEAM_GAME: GameType = GameType::CaptureTheFlag;

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

/// The first capture flag spot of team index `team` (world units), if any.
fn flag_spot(flags: &[NetgameFlag], team: i16) -> Option<[f32; 3]> {
    flags
        .iter()
        .find(|f| f.flag_type == NetgameFlagType::CaptureTheFlag && f.team_index == team)
        .map(|f| f.position)
}

fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

/// Start `start`'s team in capture the flag after the game's flag check
/// (multiplayer_rules.md 4.3): None when it is not a capture the flag
/// start, its team index is not 0 or 1, or it is nearer (squared 3D
/// distance) to the enemy's flag spot than to its own team's. A map
/// without both flag spots keeps every start.
pub fn ctf_team(flags: &[NetgameFlag], start: &PlayerStart) -> Option<Team> {
    if !start.game_types.eligible_for(TEAM_GAME) {
        return None;
    }
    let team = team(start, TEAM_GAME)?;
    let own = flag_spot(flags, start.team_index);
    let enemy = flag_spot(flags, 1 - start.team_index);
    if let (Some(own), Some(enemy)) = (own, enemy)
        && distance2(start.position, enemy) < distance2(start.position, own)
    {
        return None;
    }
    Some(team)
}

/// The map's spawns (module doc) from its player starts and netgame
/// flags, as (feet in metres, team, engine yaw).
pub fn spawns(starts: &[PlayerStart], flags: &[NetgameFlag]) -> Vec<(bevy::math::Vec3, Option<Team>, f32)> {
    starts
        .iter()
        .filter_map(|s| {
            let team = ctf_team(flags, s);
            (team.is_some() || s.game_types.eligible_for(DEFAULT_GAME))
                .then(|| (space::point(s.position), team, space::yaw(s.facing)))
        })
        .collect()
}

/// Fill `map`'s spawns from the scenario (module doc).
pub fn add(scenario: &Scenario, map: &mut MapData) {
    let mut chosen = spawns(&scenario.player_starts, &scenario.netgame_flags);
    if chosen.is_empty() && !scenario.player_starts.is_empty() {
        map.warnings.push(format!(
            "no player start is eligible for {DEFAULT_GAME:?} or {TEAM_GAME:?}: every start ({}) spawns",
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
