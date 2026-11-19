//! The clan modes (specs/combat_arms/game_modes_2.md M18): test cases
//! 36-41 on `CaSim` and the greybox (`mode_kit`), 1 v 1 or 2 v 2 (a clan
//! a team), counters set where the case starts from them.

use bevy::prelude::*;
use chimera_core::core::DamageKind;
use chimera_world::harness::Sim;
use combat_arms::game::rules::{CaMatch, CaStats, Phase, clan::CaClan, objectives::BOMB};

use crate::mode_kit::*;

fn clan_game(mode: &str, per_team: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = game(mode, 8);
    let a: Vec<Entity> = (0..per_team).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..per_team).map(|_| join(&mut sim, CREED)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(2);
    (sim, a, b)
}

fn set(sim: &mut Sim, f: impl FnOnce(&mut CaMatch)) {
    f(&mut sim.app.world_mut().resource_mut::<CaMatch>());
}

/// Through the half time's round over, intermission and warm-up to the
/// second half's live phase; checks the warm-up came.
fn to_second_half(sim: &mut Sim) {
    until(sim, 10.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(sim);
    assert!(m(sim).warmup, "a warm-up before the second half");
    assert!(sim.app.world().resource::<CaClan>().second_half);
}

/// 36. CDM, G 100: clan X (Alpha) at 50 with clan Y at 41 → round over
/// (no winner), then X on team 2 with 50, Y on team 1 with 41, everyone
/// respawned, a 10 s warm-up counting nothing; Y first to 100 → team 1
/// wins.
#[test]
fn case_36_clan_elimination() {
    let (mut sim, x, y) = clan_game("CDM", 1);
    set(&mut sim, |m| m.score = [49, 41]);
    kill_now(&mut sim, y[0], x[0]);
    assert_eq!(m(&sim).score, [50, 41]);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, None);
    assert_eq!(m(&sim).result, None, "the match goes on");
    to_second_half(&mut sim);
    assert_eq!((team(&sim, x[0]), team(&sim, y[0])), (CREED, JSF));
    assert_eq!(m(&sim).score, [41, 50], "each clan keeps its kills");
    assert!(!dead(&sim, y[0]), "respawned");
    unprotect_all(&mut sim);
    kill_now(&mut sim, x[0], y[0]);
    assert_eq!(m(&sim).score, [41, 50], "the warm-up counts nothing");
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    assert!(!m(&sim).warmup);
    unprotect_all(&mut sim);
    set(&mut sim, |m| m.score = [99, 60]);
    kill_now(&mut sim, x[0], y[0]);
    until(&mut sim, 10.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(JSF)), "clan Y on team 1");
}

/// 37. CDM half time by a suicide: team 1 at 49, a team 2 player kills
/// themselves → team 1 50 → half time.
#[test]
fn case_37_half_by_suicide() {
    let (mut sim, _x, y) = clan_game("CDM", 1);
    set(&mut sim, |m| m.score = [49, 10]);
    hit(&mut sim, y[0], Some(y[0]), 5.0, DamageKind::Blast, None);
    sim.ticks(1);
    assert_eq!(m(&sim).score, [50, 10]);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    to_second_half(&mut sim);
}

/// 38. CCTF, G 6: team 1 at 3 captures at 400 s → half time; captures
/// 3 - 1 and returns 2 - 0 swap with the clans; the second half 600 s;
/// its time-out at 5 - 5 captures, 4 - 4 returns, 30 - 28 kills → the
/// 30-kill clan.
#[test]
fn case_38_clan_ctf() {
    let (mut sim, x, _y) = clan_game("CCTF", 1);
    let ends = m(&sim).round_ends.unwrap();
    assert!((ends - now(&sim) - 600.0).abs() < 0.2, "a flat 600 s half");
    set(&mut sim, |m| {
        m.score = [3, 1];
        m.returns = [2, 0];
    });
    sim.ticks(1);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, None);
    to_second_half(&mut sim);
    let mm = m(&sim);
    assert_eq!((mm.score, mm.returns), ([1, 3], [0, 2]));
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    let ends = m(&sim).round_ends.unwrap();
    assert!((ends - now(&sim) - 600.0).abs() < 0.2);
    // Clan X is on team 2 now.
    assert_eq!(team(&sim, x[0]), CREED);
    let t = now(&sim);
    set(&mut sim, |m| {
        m.score = [5, 5];
        m.returns = [4, 4];
        m.kills = [28, 30];
        m.round_ends = Some(t + 0.05);
    });
    until(&mut sim, 10.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(CREED)), "clan X's 30 kills");
}

/// 39. CCTF's first half running out at 600 s with 2 - 1: half time
/// anyway; the second half from 2 - 1 by clan.
#[test]
fn case_39_ctf_first_half_time_out() {
    let (mut sim, _x, _y) = clan_game("CCTF", 1);
    let t = now(&sim);
    set(&mut sim, |m| {
        m.score = [2, 1];
        m.round_ends = Some(t + 0.05);
    });
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, None);
    to_second_half(&mut sim);
    assert_eq!(m(&sim).score, [1, 2]);
}

/// 40. CMM, G 6: after round 3 (clan X attacking, 2 - 1 up) a warm-up,
/// X on team 2 defending with its 2 wins; round 4's bombs go to clan Y on
/// team 1. After round 6 at 3 - 3, mission completions 2 - 1 → the clan
/// with 2; all equal → a draw.
#[test]
fn case_40_clan_search_destroy() {
    for equal in [false, true] {
        let (mut sim, x, y) = clan_game("CMM", 2);
        set(&mut sim, |m| {
            m.round = 2;
            m.round_wins = [1, 1];
        });
        for v in &y {
            kill_now(&mut sim, *v, x[0]);
        }
        until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
        assert_eq!(m(&sim).round_wins, [2, 1]);
        to_second_half(&mut sim);
        assert!(x.iter().all(|p| team(&sim, *p) == CREED), "X defends");
        assert_eq!(m(&sim).round_wins, [1, 2], "X keeps 2");
        until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
        until_live(&mut sim);
        sim.ticks(3);
        assert!(y.iter().all(|p| weapons(&sim, *p).contains(&BOMB)), "Y's bombs");
        assert!(x.iter().all(|p| !weapons(&sim, *p).contains(&BOMB)));
        // Round 6: X wins it to 3 - 3.
        unprotect_all(&mut sim);
        set(&mut sim, |m| {
            m.round = 5;
            m.round_wins = [3, 2];
        });
        sim.app.world_mut().resource_mut::<CaClan>().completions = if equal { [1, 1] } else { [1, 2] };
        for v in &y {
            kill_now(&mut sim, *v, x[0]);
        }
        until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
        if equal {
            for p in x.iter().chain(&y) {
                sim.app.world_mut().entity_mut(*p).insert(CaStats::default());
            }
        }
        until(&mut sim, 10.0, |p| p == Phase::MatchOver);
        assert_eq!(m(&sim).round, 6, "exactly 6 rounds");
        assert_eq!(m(&sim).round_wins, [3, 3]);
        assert_eq!(m(&sim).result, Some(if equal { None } else { Some(CREED) }));
    }
}

/// 41. CTSV, G 6: 3 - 3 after round 6, clan kills 25 - 25, deaths
/// 24 - 26 → the clan with 24 deaths.
#[test]
fn case_41_clan_elimination_pro() {
    let (mut sim, x, y) = clan_game("CTSV", 1);
    set(&mut sim, |m| {
        m.round = 5;
        m.round_wins = [2, 3];
    });
    kill_now(&mut sim, y[0], x[0]);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    let stat = |kills, deaths| CaStats {
        kills,
        deaths,
        ..Default::default()
    };
    sim.app.world_mut().entity_mut(x[0]).insert(stat(25, 24));
    sim.app.world_mut().entity_mut(y[0]).insert(stat(25, 26));
    until(&mut sim, 10.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).round_wins, [3, 3]);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
}

/// M18.1: the last player of a clan leaving in the warm-up after the swap
/// ends the match for the other clan (Edge cases).
#[test]
fn leaving_in_the_second_warmup() {
    let (mut sim, x, y) = clan_game("CDM", 1);
    set(&mut sim, |m| m.score = [49, 0]);
    kill_now(&mut sim, y[0], x[0]);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    to_second_half(&mut sim);
    sim.app.world_mut().despawn(y[0]);
    until(&mut sim, 20.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(CREED)), "clan X, now team 2");
}
