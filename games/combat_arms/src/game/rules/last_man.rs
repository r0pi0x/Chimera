//! Last Man Standing (`combat_arms:last_man_standing`, room key `LMS`;
//! specs/combat_arms/game_modes.md M6): free-for-all rounds without
//! respawn after a warm-up; G counts rounds played, not wins.
//!
//! - Deaths: One Man Army's counts (`scoring::free_for_all`; S7 gives no
//!   column of its own).
//! - Round win: when a death leaves at most one player alive, that player
//!   (if any) wins the round (its round wins +1, its mission points with
//!   them: the scoreboard's column).
//! - Time-out: no winner.
//! - Match end: after G rounds played, or with one player or fewer left
//!   (also after the warm-up, `Hooks::warmup_end`; Corrections 2026-10-22,
//!   1 and 9); the winner by `winner`, which compares every other player
//!   with the first in the server's list only (Quirk 5); with one player
//!   or fewer left, that player.
//! - Leaving (Corrections 2026-10-22, 9): with 2 or fewer players on the
//!   server (the leaver counted) the round ends at once; a leaver's going
//!   that leaves one living player or none ends it with that player the
//!   winner (`leaving`, `left`).
//! - Spawns: the shuffled `FreeForAll` list; firing ends spawn protection;
//!   the warm-up revives the dead every 5 s (S9).
//!
//! Known shortcuts: the score panel shows the local player's round wins
//! and the leader's.

use bevy::prelude::*;

use super::{
    CaMode, CaRuleSettings, Hooks, LAST_MAN_STANDING, LeftPlayer, Leaver, NO_HOOKS, Panel, RoundTime, ScoreOrder, Teams,
    flow::alive_players,
    free_for_all::FFA_POINTS,
    scoring::free_for_all,
    state::{CaMatch, CaStats, Death, Phase},
};

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: LAST_MAN_STANDING,
        key: "LMS",
        shown_as: "Last Man Standing",
        description: "Last Man Standing: free-for-all rounds without respawn; the last player alive wins the round, G rounds \
                      are played",
        respawns: false,
        warmup: true,
        warmup_revives: true,
        forced_setup: None,
        teams: Teams::FreeForAll,
        on_death,
        time_out: |_| {},
        match_end,
        hooks: Hooks {
            warmup_end: players_left_end,
            ..NO_HOOKS
        },
    },
    ff_scaled: false,
    assists: false,
    timer_shown: true,
    spawns: FFA_POINTS,
    round_time: RoundTime::Mission,
    fire_ends_protection: true,
    death_drops: true,
    panel: Panel::PlayerRoundWins,
    order: ScoreOrder::MissionPoints,
    leaving,
};

/// A death leaving at most one player alive ends the round, that player
/// the winner.
pub fn on_death(world: &mut World, d: &Death) {
    free_for_all(world, d);
    if world.resource::<CaMatch>().winner.is_some() {
        return;
    }
    let left = alive_players(world);
    if left.len() > 1 {
        return;
    }
    match left.first() {
        Some((e, t)) => {
            super::scoring::stats(world, *e).mission_points += 1;
            world.resource_mut::<CaMatch>().win(*t);
        }
        None => world.resource_mut::<CaMatch>().end_without_winner(),
    }
}

/// Two or fewer players on the server (the leaver counted): the round ends
/// at once (Corrections 2026-10-22, 9).
pub fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase == Phase::Live && l.players <= 2 && !m.decided() {
        m.end_requested = true;
    }
}

/// The leaver's going left one living player or none (and no winner is
/// set): the round ends at once with that player, the first in the
/// server's list, its winner (its round wins +1 with the mission points).
/// A warm-up has no winner.
pub fn left(world: &mut World, l: &LeftPlayer) {
    let m = world.resource::<CaMatch>();
    if m.phase != Phase::Live || m.warmup || m.winner.is_some() {
        return;
    }
    let living: Vec<_> = alive_players(world).into_iter().filter(|(e, _)| *e != l.entity).collect();
    if living.len() > 1 {
        return;
    }
    match living.first() {
        Some((e, t)) => {
            super::scoring::stats(world, *e).mission_points += 1;
            world.resource_mut::<CaMatch>().win(*t);
        }
        None => world.resource_mut::<CaMatch>().end_without_winner(),
    }
}

/// After G rounds played, or with one player or fewer left (that player the
/// winner; Corrections 2026-10-22, 9): `winner`.
pub fn match_end(world: &World) -> Option<Option<u8>> {
    let goal = world.resource::<CaRuleSettings>().goal();
    if world.resource::<CaMatch>().round < goal && list(world).len() > 1 {
        return None;
    }
    Some(winner(world))
}

/// After the warm-up (and every real round, `match_end`): one player or
/// fewer left ends the match, that player the winner.
fn players_left_end(world: &World) -> Option<Option<u8>> {
    (list(world).len() <= 1).then(|| winner(world))
}

/// M6's match winner, as the game picks it (Quirk 5): every other player
/// compared with the first in the server's list only.
pub fn winner(world: &World) -> Option<u8> {
    let list = list(world);
    winner_of(&list)
}

/// The players in the server's list order with their statistics.
fn list(world: &World) -> Vec<(u8, CaStats)> {
    let mut all: Vec<(u64, u8, CaStats)> = Vec::new();
    // `flow::players` takes `&mut World`; the same filter here.
    for e in world.iter_entities() {
        let Some(t) = e.get::<crate::core::Team>() else { continue };
        if !super::state::plays(t.0) || e.get::<crate::core::Intent>().is_none() || e.contains::<crate::core::Npc>() {
            continue;
        }
        if e.contains::<crate::core::Connecting>() || e.contains::<crate::core::Spectating>() {
            continue;
        }
        let order = e.get::<super::flow::CaJoinOrder>().map_or(u64::MAX, |o| o.0);
        all.push((order, t.0, e.get::<CaStats>().copied().unwrap_or_default()));
    }
    all.sort_by_key(|(o, t, _)| (*o, *t));
    all.into_iter().map(|(_, t, s)| (t, s)).collect()
}

/// M6's comparison over `list` (team id, statistics) in list order: each
/// test walks the others, a player beating the first becoming the
/// candidate and clearing the tie mark, one equal to the first setting
/// it; round wins decide when no tie mark is left and the first has some,
/// then kills the same way, then more deaths when no tie mark is left;
/// else nobody. One player or fewer: that player.
pub fn winner_of(list: &[(u8, CaStats)]) -> Option<u8> {
    if list.len() <= 1 {
        return list.first().map(|(t, _)| *t);
    }
    let (first, f) = list[0];
    let test = |value: fn(&CaStats) -> u32| {
        let mut candidate = first;
        let mut tie = false;
        for (t, s) in &list[1..] {
            if value(s) > value(&f) {
                candidate = *t;
                tie = false;
            } else if value(s) == value(&f) {
                tie = true;
            }
        }
        (candidate, tie)
    };
    let (c, tie) = test(|s| s.round_wins);
    if !tie && f.round_wins > 0 {
        return Some(c);
    }
    let (c, tie) = test(|s| s.kills);
    if !tie && f.kills > 0 {
        return Some(c);
    }
    let (c, tie) = test(|s| s.deaths);
    if !tie {
        return Some(c);
    }
    None
}
