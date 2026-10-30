//! A match's results (the end-of-match panel): at the match's end (the
//! flow's `MatchEnd` step, observed: `on_step`; the intermission, then
//! match over) `record` collects `MatchResults` on the authoritative side: each player's name, kills,
//! deaths and, on a ladder, final rung, the winner first, then by rung,
//! kills and fewest deaths. It is data, not drawing: `hud::MinigameHud`
//! carries it to whichever game's HUD is up (`MatchResults::lines` words
//! it), and a network server replicates it (`chimera_net::minigame_net`'s
//! `NetMinigame`) so every client's HUD shows the same. Gone when the next
//! match starts (`minigame::restart`).
//!
//! Each row also has the final rung's gun (its weapon id) and the
//! player's matches won counting this one (`wins::MatchWins`), and the
//! character it was (mapped to a client's own entity over the network,
//! so a client finds its own row). With the results the match-end hold
//! begins (`NextMatch`: when the next match's map change comes, and
//! where once a server's rotation picked it), which the shell's
//! match-end screen counts down (`chimera_client::client::match_end`).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{ActiveMinigame, ladder, runtime};
use crate::{
    core::{NetRole, Playback, Score, authoritative},
    mode::{FlowEvent, Match, ModeStep},
};

/// One player's line of the results.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, bevy::ecs::entity::MapEntities)]
pub struct ResultRow {
    pub name: String,
    pub kills: i32,
    pub deaths: u32,
    /// On a ladder: the final rung, counted from 1.
    pub rung: Option<u32>,
    pub winner: bool,
    /// The character (replicated and mapped: a client's own entity for
    /// it, so it finds its own row).
    #[entities]
    pub player: Option<Entity>,
    /// On a ladder: the final rung's gun (a weapon id).
    pub gun: Option<String>,
    /// Matches won so far, this one counted (`wins::MatchWins`).
    pub wins: u32,
}

/// The finished match's results, best first.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize, bevy::ecs::entity::MapEntities)]
pub struct MatchResults {
    /// The minigame's name.
    pub title: String,
    /// On a ladder: how many rungs it has (0: not one).
    pub rungs: u32,
    #[entities]
    pub rows: Vec<ResultRow>,
}

/// The match-end hold: the match is decided with its results up (from
/// the round's end, after the win's own moment), and the next match's
/// map change comes at `at` (`SimClock`; `runtime::RESULTS_HOLD` s after
/// the match over) on `map` (`net::normalize_map`'s
/// form) once a server's rotation picked it (`chimera_net::minigame_net`,
/// which also replicates it). Begun by `record` on the authoritative side,
/// gone with the results (`minigame::restart`).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct NextMatch {
    pub map: Option<String>,
    pub at: f64,
}

impl NextMatch {
    /// The map's name as a player reads it: its id without the game.
    pub fn map_name(&self) -> Option<&str> {
        self.map.as_deref().map(|m| m.rsplit(':').next().unwrap_or(m))
    }
}

impl MatchResults {
    /// The match's winner, if it had one (a draw: none).
    pub fn winner(&self) -> Option<&ResultRow> {
        self.rows.iter().find(|r| r.winner)
    }

    /// The panel's text, top to bottom: who won, then a line per player
    /// (`name` is how the HUD words `row`).
    pub fn lines(&self, name: impl Fn(&ResultRow) -> String) -> Vec<String> {
        let mut out = vec![match self.winner() {
            Some(w) => format!("{} wins the match", name(w)),
            None => "Match over: a draw".to_string(),
        }];
        for (i, r) in self.rows.iter().enumerate() {
            let rung = match (r.rung, self.rungs) {
                (Some(n), total) if total > 0 => format!("  rung {n}/{total}"),
                _ => String::new(),
            };
            out.push(format!("{}. {}{rung}  {} kills  {} deaths", i + 1, name(r), r.kills, r.deaths));
        }
        out
    }
}

/// The results of the match as it stands.
pub fn collect(world: &mut World) -> Option<MatchResults> {
    let active = *world.get_resource::<ActiveMinigame>()?;
    let m = world.resource::<Match>().clone();
    let on_ladder = active.rules.mode.id == ladder::LADDER;
    let ladder_rungs: Vec<&'static str> =
        if on_ladder { world.get_resource::<ladder::Ladder>().map(|l| l.rungs.clone()).unwrap_or_default() } else { Vec::new() };
    let rungs = ladder_rungs.len() as u32;
    let wins = world.get_resource::<super::wins::MatchWins>().cloned().unwrap_or_default();
    let mut rows: Vec<(usize, ResultRow)> = runtime::players(world)
        .into_iter()
        .enumerate()
        .map(|(order, (e, team))| {
            let s = world.get::<Score>(e).copied().unwrap_or_default();
            let name = world.get::<Name>(e).map_or_else(|| format!("Player {}", order + 1), |n| n.as_str().to_string());
            let at = world.get::<ladder::Rung>(e).map_or(0, |r| r.0);
            let rung = on_ladder.then_some(at as u32 + 1);
            let gun = on_ladder
                .then(|| ladder_rungs.get(at.min(ladder_rungs.len().saturating_sub(1))).map(|g| g.to_string()))
                .flatten();
            (order, ResultRow {
                wins: wins.of(&name),
                gun,
                name,
                kills: s.kills,
                deaths: s.deaths,
                rung,
                winner: m.result.flatten() == Some(team),
                player: Some(e),
            })
        })
        .collect();
    rows.sort_by(|(oa, a), (ob, b)| {
        b.winner
            .cmp(&a.winner)
            .then(b.rung.cmp(&a.rung))
            .then(b.kills.cmp(&a.kills))
            .then(a.deaths.cmp(&b.deaths))
            .then(oa.cmp(ob))
    });
    Some(MatchResults {
        title: super::hud::shown_name(world, active.def),
        rungs,
        rows: rows.into_iter().map(|(_, r)| r).collect(),
    })
}

/// Keeps `MatchResults` in step with the match, from its flow's steps
/// (`mode::ModeStep`, observed at once inside the step, on the
/// authoritative side while a minigame is the gameplay): collected at the
/// match's end (`FlowEvent::MatchEnd`), with the match-end hold
/// (`NextMatch`); gone when a new match starts (`Prepare { first: true }`;
/// `minigame::restart` removes them itself).
pub(super) fn on_step(
    step: On<ModeStep>,
    active: Option<Res<ActiveMinigame>>,
    gameplay: Option<Res<chimera_core::gameplay::Gameplay>>,
    role: Option<Res<NetRole>>,
    playback: Option<Res<Playback>>,
    mut commands: Commands,
) {
    if active.is_none() || gameplay.is_some_and(|g| g.rules != super::MINIGAME) || !authoritative(role, playback) {
        return;
    }
    match step.0 {
        FlowEvent::MatchEnd => commands.queue(record),
        FlowEvent::Prepare { first: true } => commands.queue(|world: &mut World| {
            world.remove_resource::<MatchResults>();
            world.remove_resource::<NextMatch>();
        }),
        _ => {}
    }
}

/// The match's end: its results collected (the winner's match counted)
/// and the match-end hold begun: the change comes `RESULTS_HOLD` s after
/// the match over.
fn record(world: &mut World) {
    let decided = world.get_resource::<Match>().is_some_and(|m| m.result.is_some());
    if !decided {
        return;
    }
    if !world.contains_resource::<MatchResults>()
        && let Some(mut r) = collect(world)
    {
        if let (Some(w), Some(mut wins)) = (r.winner(), world.get_resource_mut::<super::wins::MatchWins>()) {
            wins.add(&w.name);
            // This match counted on the winner's row.
            for row in r.rows.iter_mut().filter(|row| row.winner) {
                row.wins += 1;
            }
        }
        world.insert_resource(r);
    }
    if world.contains_resource::<MatchResults>() && !world.contains_resource::<NextMatch>() {
        let now = world.get_resource::<crate::core::SimClock>().map_or(0.0, |c| c.now);
        let over_at = match world.resource::<Match>().phase {
            crate::mode::Phase::Intermission { until } => until,
            crate::mode::Phase::MatchOver => now,
            _ => now + runtime::INTERMISSION as f64,
        };
        world.insert_resource(NextMatch {
            map: None,
            at: over_at + runtime::RESULTS_HOLD,
        });
    }
}
