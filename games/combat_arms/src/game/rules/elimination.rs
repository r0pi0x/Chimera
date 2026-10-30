//! Elimination Pro (`combat_arms:elimination_pro`, room key `TSV`;
//! specs/combat_arms/game_modes.md M2, the brief's "Elimination"): Alpha
//! against Bravo in rounds without respawn, after a warm-up; G round wins
//! take the match.
//!
//! - Deaths (S7's Elimination Pro column): enemy kill: killer kills +1,
//!   victim deaths +1, the killer's team's round kills +1; team kill:
//!   killer friendly kills +1 (no death, Quirk 3); own damage: victim
//!   suicides and friendly kills +1 (no death); no killer: victim deaths
//!   +1; every death: the victim's team's round deaths +1. Assists as S7.
//! - Round win: the first death that leaves a team with nobody alive gives
//!   the round to the other team (both empty: the victim's enemy team).
//!   Once a winner is set, later events change nothing.
//! - Time-out (`TimeLimit` 180 s): more living players; else more round
//!   kills; else fewer round deaths; else no winner.
//! - Match end, after each real round: a team has G round wins, or a team
//!   is empty; the winner the non-empty team when one is empty, else more
//!   round wins, else Bravo (Quirk 4). After the warm-up too, minus the
//!   round-win goal (`Hooks::warmup_end`, Corrections 2026-10-22, 1).
//! - Leaving (`leaving`, in the warm-up too): the only player of a team
//!   leaving ends the round (the time-out rule picks the other team); a
//!   living leaver who was the last of its team alive gives the round to
//!   the other team.
//! - Warm-up (S9, `TrainingTimeLimit` 10 s): nothing counts, the dead
//!   revived every 5 s. Joiners are held until the next round when the
//!   round is live with 3 or more players in (S1). Spawns: the
//!   `Team Death Match` points, as TDM.
//!
//! - Spectating (Open question 35): the dead watch their living
//!   teammates, then anyone once none is left (`rules::spectator_rules`,
//!   Chimera's spectating with force 2; Last Man Standing and Hunted:
//!   anyone).
//!
//! Known shortcuts: the death camera follows Chimera's (its death cam on
//! the killer, then the first allowed player), not "the killer first while
//! alive with the enemy-view option on"; the enemy-view room option isn't
//! there (always on); no late-join closing.

use bevy::prelude::*;

use super::{
    CaMode, CaRuleSettings, ELIMINATION_PRO, Hooks, Leaver, NO_HOOKS, Panel, RoundTime, ScoreOrder, TEAM_POINTS, Teams,
    flow::alive,
    scoring::stats,
    state::{ALPHA, BRAVO, CaMatch, Death, DeathKind, Phase, enemy, side},
};

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: ELIMINATION_PRO,
        key: "TSV",
        shown_as: "Elimination Pro",
        description: "Elimination Pro: Alpha against Bravo in rounds without respawn after a warm-up, G round wins take the match",
        respawns: false,
        warmup: true,
        warmup_revives: true,
        forced_setup: None,
        teams: Teams::Two,
        on_death,
        time_out,
        match_end,
        hooks: Hooks {
            warmup_end: match_end,
            ..NO_HOOKS
        },
    },
    ff_scaled: true,
    assists: true,
    timer_shown: true,
    spawns: TEAM_POINTS,
    round_time: RoundTime::Mission,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::RoundWins,
    order: ScoreOrder::Kills,
    leaving,
};

fn on_death(world: &mut World, d: &Death) {
    match d.kind {
        DeathKind::Enemy => {
            if let Some(k) = d.killer {
                stats(world, k).kills += 1;
            }
            stats(world, d.victim).deaths += 1;
            if let Some(i) = side(enemy(d.victim_team)) {
                world.resource_mut::<CaMatch>().round_kills[i] += 1;
            }
        }
        DeathKind::TeamKill => {
            if let Some(k) = d.killer {
                stats(world, k).friendly_kills += 1;
            }
        }
        DeathKind::Suicide => {
            let mut s = stats(world, d.victim);
            s.suicides += 1;
            s.friendly_kills += 1;
        }
        DeathKind::NoKiller => stats(world, d.victim).deaths += 1,
    }
    if let Some(i) = side(d.victim_team) {
        world.resource_mut::<CaMatch>().round_deaths[i] += 1;
    }
    if world.resource::<CaMatch>().winner.is_some() {
        return;
    }
    let (a, b) = (alive(world, ALPHA), alive(world, BRAVO));
    let winner = match (a, b) {
        (0, 0) => Some(enemy(d.victim_team)),
        (0, _) => Some(BRAVO),
        (_, 0) => Some(ALPHA),
        _ => None,
    };
    if let Some(w) = winner {
        world.resource_mut::<CaMatch>().win(w);
    }
}

/// More living players; more round kills; fewer round deaths; else none.
pub fn time_out(world: &mut World) {
    let living = [alive(world, ALPHA), alive(world, BRAVO)];
    let mut m = world.resource_mut::<CaMatch>();
    let pick = |x: u32, y: u32| match x.cmp(&y) {
        std::cmp::Ordering::Greater => Some(ALPHA),
        std::cmp::Ordering::Less => Some(BRAVO),
        std::cmp::Ordering::Equal => None,
    };
    m.winner = pick(living[0] as u32, living[1] as u32)
        .or_else(|| pick(m.round_kills[0], m.round_kills[1]))
        .or_else(|| pick(m.round_deaths[1], m.round_deaths[0]));
}

/// G round wins, or an empty team.
fn match_end(world: &World) -> Option<Option<u8>> {
    let goal = world.resource::<CaRuleSettings>().goal();
    let m = world.resource::<CaMatch>();
    let counts = world.resource::<super::CaTeamCounts>().0;
    let empty = counts.iter().any(|c| *c == 0);
    if !empty && m.round_wins.iter().all(|w| *w < goal) {
        return None;
    }
    let winner = if counts[0] == 0 && counts[1] > 0 {
        BRAVO
    } else if counts[1] == 0 && counts[0] > 0 {
        ALPHA
    } else if m.round_wins[0] > m.round_wins[1] {
        ALPHA
    } else {
        BRAVO
    };
    Some(Some(winner))
}

/// The leaving rule (Corrections 2026-10-22, 2; no phase or warm-up test
/// beyond "live"; a request in the warm-up ends it early, no winner): the
/// only player of a team leaving, alive or dead, asks for the round's end
/// (the time-out rule picks the other team, which has living players);
/// otherwise a living leaver, the last of its team alive, gives the round
/// to the other team.
pub fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase != Phase::Live || m.winner.is_some() {
        return;
    }
    if l.team_players <= 1 {
        m.end_requested = true;
    } else if l.alive && l.team_alive <= 1 {
        m.win(enemy(l.team));
    }
}
