//! Team Death Match, shown as "Elimination" (`combat_arms:tdm`, room key
//! `TDM`; specs/combat_arms/game_modes.md M1): Alpha against Bravo,
//! respawning, one round with no time limit, won by the first team whose
//! kill counter reaches the goal G.
//!
//! - Score (S7's TDM column): every death adds 1 to the victim's enemy
//!   team, whoever caused it (team kills, suicides and falls feed the
//!   other side; Quirk 1). Enemy kill: killer kills +1, victim deaths +1.
//!   Team kill: killer friendly kills +1, victim deaths +1. Own damage:
//!   victim deaths, suicides and friendly kills +1 (Quirk 2). No killer:
//!   victim deaths and suicides +1. Assists as S7.
//! - Win: the first death that brings a counter to G or more asks for the
//!   round's end with the higher counter's team; a later death in the same
//!   frame (still live) counts and changes the winner only if the other
//!   counter is now strictly higher (Edge cases).
//! - No time-out (no timer); the match ends after the one round, the
//!   round's winner its winner.
//! - Leaving: the last player of a team leaving gives the round (and the
//!   match) to the other team at once.
//! - Setup: 0 s, forced (everyone frozen one frame). Friendly fire on:
//!   bullets and melee at a quarter.
//!
//! Known shortcuts: no revenge buff (the event customisation) and no
//! mutation boxes (the room's option, S12): neither exists yet.

use bevy::prelude::*;

use super::{
    CaMode, Leaver, NO_HOOKS, Panel, RoundTime, ScoreOrder, TDM, TEAM_POINTS, Teams,
    scoring::personal,
    state::{CaMatch, Death, Phase, enemy, side},
};

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: TDM,
        key: "TDM",
        shown_as: "Elimination",
        description: "Team Death Match (shown as Elimination): Alpha against Bravo, respawning, the first team to G kills wins",
        respawns: true,
        warmup: false,
        warmup_revives: false,
        forced_setup: Some(0.0),
        teams: Teams::Two,
        on_death,
        time_out: |_| {},
        match_end: |w| Some(w.resource::<CaMatch>().winner),
        hooks: NO_HOOKS,
    },
    ff_scaled: true,
    assists: true,
    timer_shown: false,
    spawns: TEAM_POINTS,
    round_time: RoundTime::Mission,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::TeamScore,
    order: ScoreOrder::Kills,
    leaving,
};

fn on_death(world: &mut World, d: &Death) {
    personal(world, d);
    add_and_test(world, d.victim_team, 1);
}

/// S7's TDM score: `points` to the victim's enemy team; the first death
/// that brings a counter to G or more asks for the round's end with the
/// higher counter's team, a later one changing the winner only when the
/// other counter is now strictly higher (VIP Elimination's too).
pub fn add_and_test(world: &mut World, victim_team: u8, points: i32) {
    let goal = world.resource::<super::CaRuleSettings>().goal() as i32;
    let scorer = enemy(victim_team);
    let mut m = world.resource_mut::<CaMatch>();
    let Some(i) = side(scorer) else { return };
    m.score[i] += points;
    if m.score.iter().all(|s| *s < goal) {
        return;
    }
    let leader = if m.score[0] > m.score[1] { 0 } else { 1 };
    let take = match m.winner.and_then(side) {
        None => true,
        Some(w) => m.score[leader] > m.score[w],
    };
    if take {
        m.win(leader as u8 + 1);
    }
}

/// The last player of a team leaving gives the round to the other team.
pub fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase == Phase::Live && l.team_players <= 1 {
        m.win(enemy(l.team));
    }
}
