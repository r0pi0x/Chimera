//! Seize and Secure (`combat_arms:seize_and_secure`, room key `HQM`;
//! specs/combat_arms/game_modes.md M10) and Squad Match 2
//! (`combat_arms:squad_match_2`, `SQM2`, M11: the same rules, no room
//! entry): Alpha against Bravo, respawning; a warm-up, then one round of G
//! minutes around a flag pole.
//!
//! - Seizing: a living player holding use inside its own team's
//!   `ActivateTrigger` (`ActivateTrigger_A` JSF, `_B` Creed) for
//!   `seize_time` (5 s; `objectives::CaAction`) while the round counts:
//!   its team's flag starts from the bottom of the pole, the other flag
//!   gone (`objectives::Pole`); the seizer's mission points +1 (flag
//!   raises). A team whose flag already rises can't seize again.
//! - Score: the holding team gains 1 point (the team counter) per whole
//!   second its flag rises.
//! - Win: a flag that reaches the top (`flag_move_time`, 120 s of holding)
//!   wins at once.
//! - Time-out: a flag still rising gets `last_occupy_bonus` (50) points;
//!   then more points; else more enemy kills, counted over every player
//!   but the first in the server's list (Quirk 6); else fewer deaths; else
//!   a draw.
//! - Deaths: S&D's counts (`scoring::round_mode`, Corrections 2026-10-22,
//!   6), the victim's team's deaths +1. Friendly fire on is full damage
//!   (S8, test case 13).
//! - Leaving: the only player of a team leaving gives the other team the
//!   win at once (Corrections 2026-10-22, 8); an empty team after the
//!   warm-up ends the match (no winner, Corrections 1).
//!
//! - Spawns: the `HeadQuaters` points per team (Open question 19; Squad
//!   Match 2 its `SquadMatch2`). The use time of each trigger, the rise
//!   time and the bonus are the mission's (`missions::activate_time`,
//!   `missions::seize`: `ActivateTriggerTimeAmount` 5 s,
//!   `FlagMoveTime` 120 s, `LastOccupyBonus` 50), else our data's.
//!
//! Known shortcuts: the pole is a state only: the map's `FlagPolePos`
//! is known (`CaMapObjects::flag_pole`, Open question 24) but no pole or
//! flag models are placed (the prop table's `HQ_Flag_Pole` isn't read);
//! the anti-abuse timers aren't kept; no late-join closing.

use bevy::prelude::*;

use super::{
    CaMode, CaRuleSettings, Hooks, Leaver, NO_HOOKS, Panel, RoundTime, SEIZE_AND_SECURE, SQUAD_MATCH_2, ScoreOrder, Spawns,
    TEAM_POINTS, Teams,
    ctf::pick,
    flow::players,
    objectives::{ActionKind, CaAction, CaObjectives, Edits, Pole, Roster, add_stats, hold_action_in},
    objects::CaMapObjects,
    scoring::round_mode,
    state::{ALPHA, BRAVO, CaMatch, CaStats, Phase, enemy, side},
};
use std::ops::DerefMut;

use super::super::missions::{self, CaMission};
use crate::core::SimClock;

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: SEIZE_AND_SECURE,
        key: "HQM",
        shown_as: "Seize and Secure",
        description: "Seize and Secure: raise your team's flag on the pole (a point a second), 120 s of holding wins; one \
                      round of G minutes",
        respawns: true,
        warmup: true,
        warmup_revives: false,
        forced_setup: None,
        teams: Teams::Two,
        on_death: |w, d| {
            round_mode(w, d);
            if let Some(i) = side(d.victim_team) {
                w.resource_mut::<CaMatch>().deaths[i] += 1;
            }
        },
        time_out,
        match_end: |w| Some(w.resource::<CaMatch>().winner),
        hooks: Hooks {
            setup: |w| {
                let real = !w.resource::<CaMatch>().warmup;
                w.resource_mut::<CaObjectives>().pole = real.then(Pole::default);
            },
            warmup_end,
            ..NO_HOOKS
        },
    },
    ff_scaled: false,
    assists: false,
    timer_shown: true,
    spawns: Spawns {
        family: Some(FAMILY),
        ..TEAM_POINTS
    },
    round_time: RoundTime::GoalMinutes,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::TeamScore,
    order: ScoreOrder::MissionPoints,
    leaving,
};

/// Squad Match 2 (M11): Seize and Secure's rules on its own points.
pub const SQUAD_MATCH: CaMode = CaMode {
    rules: super::Mode {
        id: SQUAD_MATCH_2,
        key: "SQM2",
        shown_as: "Squad Match 2",
        description: "Squad Match 2: Seize and Secure's rules (reached through Hired Guns 2's mission file)",
        ..MODE.rules
    },
    spawns: Spawns {
        family: Some("SquadMatch2"),
        ..TEAM_POINTS
    },
    ..MODE
};

/// The only player of a team leaving gives the other team the win at once
/// (the match ends with it: one round; Corrections 2026-10-22, 8).
fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase == Phase::Live && m.winner.is_none() && l.team_players <= 1 {
        m.win(enemy(l.team));
    }
}

/// After the warm-up an empty team ends the match (Corrections
/// 2026-10-22, 1); the warm-up has no winner.
fn warmup_end(world: &World) -> Option<Option<u8>> {
    world
        .resource::<super::CaTeamCounts>()
        .0
        .contains(&0)
        .then(|| world.resource::<CaMatch>().winner)
}

/// Seize and Secure's start points' game type (spelled so).
pub const FAMILY: &str = "HeadQuaters";

/// Whether trigger `name` is `team`'s (`ActivateTrigger_A` JSF, `_B`
/// Creed).
pub fn trigger_of(name: &str, team: u8) -> bool {
    let n = name.to_ascii_uppercase();
    match team {
        ALPHA => n.ends_with("_A"),
        BRAVO => n.ends_with("_B"),
        _ => false,
    }
}

/// The points of the rising flag brought up to now.
fn score_pole(world: &mut World, now: f64) {
    world.resource_scope::<CaObjectives, _>(|world, mut o| {
        score_pole_in(&mut world.resource_mut::<CaMatch>(), &mut o, now);
    });
}

/// `score_pole` on the resources (only what changes is written).
fn score_pole_in(m: &mut impl DerefMut<Target = CaMatch>, o: &mut impl DerefMut<Target = CaObjectives>, now: f64) {
    let Some(pole) = o.pole else { return };
    let Some(team) = pole.team else { return };
    let whole = (now - pole.since + 1e-9).floor().max(0.0) as u32;
    if whole > pole.scored {
        let add = (whole - pole.scored) as i32;
        if let Some(i) = side(team) {
            m.score[i] += add;
        }
        if let Some(p) = &mut o.pole {
            p.scored = whole;
        }
    }
}

/// Seizing, the rising flag's points and the win (the mode's per-tick
/// system, `rules::mode_updates`).
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    clock: Res<SimClock>,
    mission: Option<Res<CaMission>>,
    settings: Res<CaRuleSettings>,
    objects: Res<CaMapObjects>,
    mut m: ResMut<CaMatch>,
    mut o: ResMut<CaObjectives>,
    roster: Roster,
    mut actions: Query<&mut CaAction>,
    mut stats: Query<&mut CaStats>,
    mut commands: Commands,
) {
    let now = clock.now;
    let mission = mission.as_deref();
    let (move_time, _) = missions::seize_in(mission, &settings);
    let counting = m.counting() && m.winner.is_none();
    let Some(pole) = o.pole else { return };
    let dt = clock.dt() as f32;
    let mut gauges = Edits::default();
    for p in roster.takers() {
        let inside = objects
            .triggers
            .iter()
            .find(|t| trigger_of(&t.name, p.team) && t.area.contains(p.at, 0.0));
        let on = counting && p.intent.use_key && inside.is_some() && pole.team != Some(p.team);
        // The use time the mission sets on that trigger (5 s stock).
        let time = inside.map_or(0.0, |t| missions::activate_time_in(mission, &settings, &t.name));
        if hold_action_in(&mut gauges, &actions, dt, p.e, ActionKind::Seize, time, on) {
            score_pole_in(&mut m, &mut o, now);
            o.pole = Some(Pole {
                team: Some(p.team),
                since: now,
                scored: 0,
            });
            add_stats(&mut stats, &mut commands, p.e, |s| s.mission_points += 1);
            info!("pole seized by team {}", p.team);
            gauges.apply(&mut actions, &mut commands);
            return;
        }
    }
    gauges.apply(&mut actions, &mut commands);
    if !counting {
        return;
    }
    score_pole_in(&mut m, &mut o, now);
    if let Some(Pole { team: Some(t), since, .. }) = o.pole
        && now + 1e-9 >= since + move_time as f64
    {
        m.win(t);
    }
}

/// The rising flag's bonus, then more points; more enemy kills (the first
/// player skipped); fewer deaths; else none.
fn time_out(world: &mut World) {
    let now = world.resource::<SimClock>().now;
    score_pole(world, now);
    let (_, bonus) = super::super::missions::seize(world);
    if let Some(Pole { team: Some(t), .. }) = world.resource::<CaObjectives>().pole
        && let Some(i) = side(t)
    {
        world.resource_mut::<CaMatch>().score[i] += bonus;
    }
    let mut kills = [0i64; 2];
    for (e, t) in players(world).into_iter().skip(1) {
        if let Some(i) = side(t) {
            kills[i] += world.get::<CaStats>(e).map_or(0, |s| s.kills) as i64;
        }
    }
    let mut m = world.resource_mut::<CaMatch>();
    m.winner = pick(m.score[0] as i64, m.score[1] as i64)
        .or_else(|| pick(kills[0], kills[1]))
        .or_else(|| pick(m.deaths[1] as i64, m.deaths[0] as i64));
}
