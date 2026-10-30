//! One Man Army (`combat_arms:one_man_army`, room key `FFA`;
//! specs/combat_arms/game_modes.md M5): everyone for themselves, each
//! player a team of their own (S1: ids from `state::FFA_FIRST`),
//! respawning, one round with no time limit; the first player to G kills
//! wins.
//!
//! - Deaths (S7's One Man Army column, `scoring::free_for_all`): an enemy
//!   kill, killer kills +1 and victim deaths +1; own damage or no killer,
//!   victim deaths and suicides +1 (never a negative kill).
//! - Win: a kill that brings a player to G kills ends the round, that
//!   player (its team id) the winner (the later of two such kills in one
//!   frame, Corrections 2026-10-22, 14); the match ends with it.
//! - Leaving: with 2 or fewer players left (the leaver counted) the round
//!   ends; the remaining player wins if the kills made in total (every
//!   kill of the round, the departed players' too: `CaKillsMade`) are
//!   above `leave_kill_share` (10 %) of G, rounded, else nobody.
//! - Spawns: the `FreeForAll` points in one list, shuffled once a map
//!   load, one cursor (S4). Firing ends spawn protection (S6).
//!
//! Known shortcuts: no mutation boxes (the room's option, S12); the
//! score panel shows the local player's kills and the leader's; no
//! late-join closing.

use bevy::prelude::*;

use super::{
    CaMode, CaRuleSettings, Leaver, NO_HOOKS, ONE_MAN_ARMY, Panel, RoundTime, ScoreOrder, Spawns, Teams,
    scoring::free_for_all,
    state::{CaMatch, Death, DeathKind, Phase},
};
use crate::core::Team;

/// The free-for-all modes' points: one list, shuffled.
pub const FFA_POINTS: Spawns = Spawns {
    family: Some("FreeForAll"),
    one_list: true,
    shuffled: true,
    locked_rounds: false,
};

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: ONE_MAN_ARMY,
        key: "FFA",
        shown_as: "One Man Army",
        description: "One Man Army: everyone for themselves, respawning; the first player to G kills wins",
        respawns: true,
        warmup: false,
        warmup_revives: false,
        forced_setup: None,
        teams: Teams::FreeForAll,
        on_death,
        time_out: |_| {},
        match_end: |w| Some(w.resource::<CaMatch>().winner),
        hooks: NO_HOOKS,
    },
    ff_scaled: false,
    assists: false,
    timer_shown: false,
    spawns: FFA_POINTS,
    round_time: RoundTime::Mission,
    fire_ends_protection: true,
    death_drops: true,
    panel: Panel::Kills,
    order: ScoreOrder::Kills,
    leaving,
};

fn on_death(world: &mut World, d: &Death) {
    free_for_all(world, d);
    if d.kind != DeathKind::Enemy {
        return;
    }
    let Some(k) = d.killer else { return };
    test_win(world, k);
}

/// Every enemy kill of the round, including those of players who already
/// left (the leaving rule's "kills made", Corrections 2026-10-22, 14);
/// back to 0 at each round's setup.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaKillsMade(pub u32);

/// A kill that brings `k` to G kills or more (re)sets the winner, so of two
/// such kills in one frame the later wins (Corrections 2026-10-22, 14).
pub fn test_win(world: &mut World, k: Entity) {
    let goal = world.resource::<CaRuleSettings>().goal();
    let kills = world.get::<super::CaStats>(k).map_or(0, |s| s.kills);
    let Some(team) = world.get::<Team>(k).map(|t| t.0) else { return };
    let mut m = world.resource_mut::<CaMatch>();
    if kills >= goal {
        m.win(team);
    }
}

/// Two or fewer players left: the round ends, the remaining player the
/// winner when the kills passed the share of G.
pub fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase != Phase::Live || m.winner.is_some() || l.players > 2 {
        return;
    }
    let floor = (l.leave_kill_share * l.goal as f32).round() as u32;
    match l.others.first() {
        Some(t) if l.total_kills > floor => m.win(*t),
        _ => m.end_without_winner(),
    }
}
