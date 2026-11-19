//! specs/combat_arms/game_modes.md's Corrections (2026-10-22) and
//! game_modes_2.md's: match flow, scoring and leaving. Test cases 92 to
//! 103 and one test for each correction they don't cover, on `CaSim` and
//! the greybox (`mode_kit`).

use bevy::prelude::*;
use chimera_core::core::{Connecting, DamageKind};
use chimera_world::harness::Sim;
use combat_arms::game::rules::{
    CaMatch, CaRuleSettings, CaStats, Phase, ScoreOrder, flow, free_for_all::CaKillsMade, objectives::BOMB, scoreboard,
    spawns,
};

use crate::mode_kit::*;

/// A player coming into a running game (S1): connecting, then entered.
fn late(sim: &mut Sim, team: u8) -> Entity {
    let p = join(sim, team);
    sim.app.world_mut().entity_mut(p).insert(Connecting);
    flow::enter_game(sim.app.world_mut(), p);
    sim.ticks(1);
    p
}

/// The mode live in its first real round (past the warm-up when it has
/// one), everyone unprotected.
fn live(sim: &mut Sim) {
    until_live(sim);
    if m(sim).warmup {
        until(sim, 40.0, |p| matches!(p, Phase::Setup { .. }));
        until_live(sim);
    }
    unprotect_all(sim);
}

fn site() -> Vec3 {
    Vec3::new(0.0, 1.0, 0.0)
}

/// S&D past its warm-up, site A placed.
fn sd(jsf: usize, creed: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = game("TMM", 12);
    objects(&mut sim).bomb_zones = vec![bomb_zone("Doobic", 0, CREED, site())];
    let a: Vec<Entity> = (0..jsf).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..creed).map(|_| join(&mut sim, CREED)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(3);
    (sim, a, b)
}

/// `p` plants the bomb at the site (S&D's JSF).
fn plant(sim: &mut Sim, p: Entity) {
    select_bomb(sim, p);
    assert!(wait_for(sim, 1.0, |s| in_hand(s, p) == Some(BOMB)));
    sim.intent(p).fire = true;
    hold_at(sim, p, site(), 4.0);
    sim.intent(p).fire = false;
    assert!(objectives(sim).planted.is_some(), "planted");
}

// --- Corrections 1, 2: the match end after a warm-up, leaving in it

/// 92. Elimination Pro 1 v 1; the Bravo player leaves at t = 4 s of the
/// warm-up: the warm-up is over at the next frame, round over until about
/// 3 s later, the intermission 3 s more, then the match is over, Alpha the
/// winner; round 1 never starts. S&D in the same case: Bravo, the round
/// wins being 0 - 0.
#[test]
fn case_92_empty_team_after_the_warm_up() {
    for (mode, winner) in [("TSV", JSF), ("TMM", CREED)] {
        let mut sim = game(mode, 12);
        join(&mut sim, JSF);
        let b = join(&mut sim, CREED);
        until_live(&mut sim);
        assert!(m(&sim).warmup);
        sim.seconds(4.0);
        sim.app.world_mut().despawn(b);
        let t = now(&sim);
        until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
        assert!(now(&sim) - t < 0.11, "{mode}: the warm-up over at the next frame");
        until(&mut sim, 4.0, |p| matches!(p, Phase::Intermission { .. }));
        assert!(
            (now(&sim) - t - 3.05).abs() < 0.15,
            "{mode}: round over until {:.2}",
            now(&sim) - t
        );
        assert_eq!(m(&sim).result, Some(Some(winner)), "{mode}");
        until(&mut sim, 4.0, |p| p == Phase::MatchOver);
        assert!(
            (now(&sim) - t - 6.05).abs() < 0.15,
            "{mode}: match over at {:.2}",
            now(&sim) - t
        );
        let s = m(&sim);
        assert_eq!(
            (s.round, s.results.len(), s.results[0].warmup),
            (0, 1, true),
            "{mode}: round 1 never starts"
        );
    }
}

/// 92 (the other rule): a warm-up that leaves both teams filled goes on to
/// round 1 (Last Man Standing: a lone player ends the match, the other
/// winning).
#[test]
fn a_full_warm_up_goes_on_to_the_round() {
    let mut sim = game("TSV", 12);
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    until(&mut sim, 40.0, |p| matches!(p, Phase::Setup { .. }));
    assert_eq!(m(&sim).result, None);
    assert_eq!(m(&sim).round, 0);
}

/// 93. S&D warm-up: Creed has 2 players, one dead (revive pending), the
/// living one leaves at t = 3 s: the warm-up ends at the next frame (no
/// winner), then the normal round over, intermission and round 1.
#[test]
fn case_93_warm_up_leaving() {
    let mut sim = game("TMM", 12);
    let a = join(&mut sim, JSF);
    let c1 = join(&mut sim, CREED);
    let c2 = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    sim.seconds(1.0);
    kill_now(&mut sim, c1, a);
    // c1 is dead, its revive pending.
    assert!(dead(&sim, c1));
    sim.seconds(2.0);
    sim.app.world_mut().despawn(c2);
    let t = now(&sim);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert!(now(&sim) - t < 0.11, "the next frame");
    let r = *m(&sim).results.last().unwrap();
    assert_eq!((r.warmup, r.winner), (true, None));
    until(&mut sim, 8.0, |p| matches!(p, Phase::Intermission { .. }));
    assert_eq!(m(&sim).result, None, "Creed still has a player");
    until(&mut sim, 8.0, |p| matches!(p, Phase::Setup { .. }));
    assert!(!m(&sim).warmup, "round 1");
}

/// 2. Elimination Pro: the only player of a team leaving asks for the
/// round's end at once, the other team (with living players) winning on
/// the time-out rule; S&D has only the "last of its team alive" part, and
/// a dead leaver gives nothing away.
#[test]
fn leaving_in_elimination_pro_and_s_and_d() {
    let mut sim = game("TSV", 12);
    let a = join(&mut sim, JSF);
    let a2 = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    live(&mut sim);
    let _ = (a, a2);
    sim.app.world_mut().despawn(b);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
    assert_eq!(m(&sim).round_wins, [1, 0]);
    // S&D: a dead Creed player leaving (a teammate lives): nothing.
    let (mut sim, _a, b) = sd(1, 2);
    kill_now(&mut sim, b[0], _a[0]);
    assert!(dead(&sim, b[0]));
    sim.app.world_mut().despawn(b[0]);
    sim.ticks(2);
    assert_eq!((phase(&sim), m(&sim).winner), (Phase::Live, None));
    // ... the living last one leaving gives the round away.
    sim.app.world_mut().despawn(b[1]);
    assert_eq!(m(&sim).winner, Some(JSF));
}

// --- Correction 3, 4: late joiners

/// 94. S&D live 30 s with 2 players in, a third joins: held (3 counted).
/// The count includes the joiner: with the wait raised to 4 the joiner
/// spawns at once. 5 players in: a joiner 0.6 s after going live spawns at
/// once, 1.2 s after is held. TDM, CTF, Bombing Run, Seize and Secure, Spy
/// Hunt and One Man Army never hold.
#[test]
fn case_94_late_join_count() {
    let (mut sim, ..) = sd(1, 1);
    sim.seconds(30.0);
    let p = late(&mut sim, JSF);
    assert!(dead(&sim, p), "3 counted: held");
    let (mut sim, ..) = sd(1, 1);
    settings(&mut sim).late_join_wait = 4;
    sim.seconds(30.0);
    let p = late(&mut sim, JSF);
    assert!(!dead(&sim, p), "2 in + the joiner = 3 < 4: at once");
    for (after, held) in [(0.6, false), (1.2, true)] {
        let (mut sim, ..) = sd(3, 2);
        let live_for = now(&sim) - m(&sim).live_since;
        sim.seconds(after - live_for);
        let p = late(&mut sim, CREED);
        assert_eq!(dead(&sim, p), held, "{after} s after going live");
    }
    for key in ["TDM", "CTF", "SBT", "HQM", "BNT", "FFA"] {
        let mut sim = game(key, 12);
        for i in 0..4 {
            join(&mut sim, if i % 2 == 0 { JSF } else { CREED });
        }
        live(&mut sim);
        sim.seconds(30.0);
        let p = late(&mut sim, JSF);
        assert!(!dead(&sim, p), "{key} never holds");
    }
}

/// 94 (the bomb): S&D holds while a bomb ticks, past the same count: 2 in
/// and the wait raised to 4, the bomb planted: the joiner spawns at once.
#[test]
fn a_ticking_bomb_holds_only_past_the_count() {
    for (wait, held) in [(3, true), (4, false)] {
        let (mut sim, a, _b) = sd(1, 1);
        settings(&mut sim).late_join_wait = wait;
        plant(&mut sim, a[0]);
        let p = late(&mut sim, JSF);
        assert_eq!(dead(&sim, p), held, "wait {wait}");
    }
}

/// 95. Quarantine: a real round live 10 s with 10 players, the joiner plays
/// at once as a human; live 25 s with 2 players, the joiner is held.
#[test]
fn case_95_quarantine_joiners() {
    let mut sim = game("QRT", 12);
    for _ in 0..10 {
        join(&mut sim, JSF);
    }
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.seconds(10.0);
    let p = late(&mut sim, JSF);
    assert!(!dead(&sim, p));
    assert_eq!(team(&sim, p), JSF, "a human");
    let mut sim = game("QRT", 12);
    for _ in 0..2 {
        join(&mut sim, JSF);
    }
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.seconds(25.0);
    let p = late(&mut sim, JSF);
    assert!(dead(&sim, p), "held until the next round");
}

// --- Correction 5: assists

/// 96. Elimination Pro: A deals 60 to B (max 100) in round 1, both
/// survive; round 2: C kills B untouched by A: no assist. TDM: A deals 60
/// to B, B dies by his own grenade, respawns, C kills him: A assist +1.
#[test]
fn case_96_assists_across_rounds() {
    let mut sim = game("TSV", 12);
    settings(&mut sim).modes.get_mut("TSV").unwrap().time_limit = Some(2.0);
    let a = join(&mut sim, JSF);
    let c = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    let _d = join(&mut sim, CREED);
    live(&mut sim);
    hit(
        &mut sim,
        b,
        Some(a),
        0.60,
        DamageKind::Bullet,
        Some(combat_arms::game::weapons::RIFLE),
    );
    sim.ticks(1);
    until(&mut sim, 6.0, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 8.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    unprotect_all(&mut sim);
    assert_eq!(m(&sim).round, 1);
    kill_now(&mut sim, b, c);
    assert_eq!(
        (stats(&sim, c).kills, stats(&sim, a).assists),
        (1, 0),
        "the round-1 damage is gone"
    );

    let mut sim = game("TDM", 12);
    let a = join(&mut sim, JSF);
    let c = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    hit(
        &mut sim,
        b,
        Some(a),
        0.60,
        DamageKind::Bullet,
        Some(combat_arms::game::weapons::RIFLE),
    );
    sim.ticks(1);
    hit(&mut sim, b, Some(b), 50.0, DamageKind::Blast, None);
    sim.ticks(1);
    assert!(dead(&sim, b));
    spawns::respawn(sim.app.world_mut(), b);
    sim.ticks(1);
    unprotect_all(&mut sim);
    kill_now(&mut sim, b, c);
    assert_eq!(stats(&sim, a).assists, 1, "the sums survive a suicide");
}

/// 5. A fall leaves a victim's sums too; a teammate's killing blow clears
/// them (and gives no assist).
#[test]
fn a_fall_keeps_the_sums_a_teammates_blow_clears_them() {
    for (by_teammate, assists) in [(false, 1), (true, 0)] {
        let mut sim = game("TDM", 12);
        let a = join(&mut sim, JSF);
        let c = join(&mut sim, JSF);
        let b = join(&mut sim, CREED);
        let b2 = join(&mut sim, CREED);
        settings(&mut sim).friendly_fire = 1;
        until_live(&mut sim);
        unprotect_all(&mut sim);
        hit(
            &mut sim,
            b,
            Some(a),
            0.60,
            DamageKind::Bullet,
            Some(combat_arms::game::weapons::RIFLE),
        );
        sim.ticks(1);
        if by_teammate {
            kill_now(&mut sim, b, b2);
        } else {
            hit(&mut sim, b, Some(b), 50.0, DamageKind::Crush, None);
            sim.ticks(1);
        }
        spawns::respawn(sim.app.world_mut(), b);
        sim.ticks(1);
        unprotect_all(&mut sim);
        kill_now(&mut sim, b, c);
        assert_eq!(stats(&sim, a).assists, assists, "teammate's blow {by_teammate}");
    }
}

// --- Correction 6, 7: Bombing Run

/// Bombing Run past its warm-up with `jsf` and `creed` players.
fn bombing(jsf: usize, creed: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = game("SBT", 12);
    {
        let mut o = objects(&mut sim);
        o.bomb_zones = vec![
            bomb_zone("Sabotage", 0, JSF, Vec3::new(-6.0, 1.0, 6.0)),
            bomb_zone("Sabotage", 1, CREED, Vec3::new(6.0, 1.0, -14.0)),
        ];
        o.drops = vec![drop_point("Sabotage", Vec3::new(0.0, 1.0, -4.0))];
    }
    let a: Vec<Entity> = (0..jsf).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..creed).map(|_| join(&mut sim, CREED)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    (sim, a, b)
}

/// 97. Bombing Run counts as S&D: a team kill gives the victim no death
/// and the killer a friendly kill; a fall, a death and no suicide; an own
/// grenade, a death and a suicide and no friendly kill.
#[test]
fn case_97_bombing_run_counts() {
    let (mut sim, a, _b) = bombing(4, 1);
    settings(&mut sim).friendly_fire = 1;
    kill_now(&mut sim, a[1], a[0]);
    let (killer, victim) = (stats(&sim, a[0]), stats(&sim, a[1]));
    assert_eq!((victim.deaths, killer.friendly_kills, killer.kills), (0, 1, 0));
    hit(&mut sim, a[2], Some(a[2]), 50.0, DamageKind::Crush, None);
    sim.ticks(1);
    let s = stats(&sim, a[2]);
    assert_eq!((s.deaths, s.suicides), (1, 0), "a fall");
    hit(&mut sim, a[3], Some(a[3]), 50.0, DamageKind::Blast, None);
    sim.ticks(1);
    let s = stats(&sim, a[3]);
    assert_eq!((s.deaths, s.suicides, s.friendly_kills), (1, 1, 0), "an own grenade");
}

/// 98. Bombing Run 3 v 3, two Bravo players dead awaiting respawn, the
/// third leaves: Alpha wins the round at the next frame.
#[test]
fn case_98_bombing_run_leaving() {
    let (mut sim, a, b) = bombing(3, 3);
    kill_now(&mut sim, b[0], a[0]);
    kill_now(&mut sim, b[1], a[0]);
    assert!(dead(&sim, b[0]) && dead(&sim, b[1]));
    assert_eq!(m(&sim).winner, None);
    sim.app.world_mut().despawn(b[2]);
    assert_eq!(m(&sim).winner, Some(JSF));
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).round_wins, [1, 0]);
}

/// 7. Bombing Run: the only player of a team leaving, dead or alive.
#[test]
fn bombing_run_only_player_leaving() {
    let (mut sim, _a, b) = bombing(1, 1);
    sim.app.world_mut().despawn(b[0]);
    assert_eq!(m(&sim).winner, Some(JSF));
}

// --- Correction 8: Seize and Secure

/// 99. Seize and Secure 1 v 2, the Alpha player leaves at t = 100 s:
/// Bravo wins at once, the match ends with it.
#[test]
fn case_99_seize_leaving() {
    let mut sim = game("HQM", 12);
    settings(&mut sim).goal = 10;
    objects(&mut sim).triggers = vec![trigger("ActivateTrigger_A", Vec3::new(-6.0, 1.0, 4.0))];
    let a = join(&mut sim, JSF);
    join(&mut sim, CREED);
    join(&mut sim, CREED);
    past_warmup(&mut sim);
    sim.seconds(100.0);
    assert_eq!(phase(&sim), Phase::Live);
    sim.app.world_mut().despawn(a);
    assert_eq!(m(&sim).winner, Some(CREED), "at once");
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(CREED)));
}

/// 1. Seize and Secure: an empty team after the warm-up ends the match
/// (the warm-up has no winner).
#[test]
fn seize_warm_up_with_an_empty_team() {
    let mut sim = game("HQM", 12);
    join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    sim.app.world_mut().despawn(b);
    until(&mut sim, 40.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(None));
}

// --- Correction 9: Last Man Standing and Hunted

/// 100. Last Man Standing, 4 players, A and B alive: A leaves, B's round
/// wins +1, the round over at the next frame. 2 players, one leaves: the
/// match over after round over and intermission, the other the winner.
#[test]
fn case_100_last_man_standing_leaving() {
    for key in ["LMS", "HG"] {
        let mut sim = game(key, 12);
        let p: Vec<Entity> = (0..4).map(|_| join(&mut sim, JSF)).collect();
        past_warmup(&mut sim);
        unprotect_all(&mut sim);
        kill_now(&mut sim, p[2], p[0]);
        kill_now(&mut sim, p[3], p[0]);
        assert_eq!(m(&sim).winner, None);
        let b_team = team(&sim, p[1]);
        sim.app.world_mut().despawn(p[0]);
        sim.ticks(1);
        assert_eq!(m(&sim).winner, Some(b_team), "{key}");
        until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
        assert_eq!(stats(&sim, p[1]).round_wins, 1, "{key}");

        let mut sim = game(key, 12);
        let p: Vec<Entity> = (0..2).map(|_| join(&mut sim, JSF)).collect();
        past_warmup(&mut sim);
        let stayer = team(&sim, p[1]);
        sim.app.world_mut().despawn(p[0]);
        until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
        assert_eq!(m(&sim).results.last().unwrap().winner, Some(stayer), "{key}");
        until(&mut sim, 8.0, |p| matches!(p, Phase::Intermission { .. }));
        assert_eq!(m(&sim).result, Some(Some(stayer)), "{key}");
        until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    }
}

/// 1. Last Man Standing with 2 players, one leaves in the warm-up: match
/// over after round over and intermission, the remaining player the
/// winner.
#[test]
fn last_man_standing_warm_up_leaving() {
    let mut sim = game("LMS", 12);
    let p: Vec<Entity> = (0..2).map(|_| join(&mut sim, JSF)).collect();
    until_live(&mut sim);
    assert!(m(&sim).warmup);
    let stayer = team(&sim, p[1]);
    sim.seconds(2.0);
    sim.app.world_mut().despawn(p[0]);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    let s = m(&sim);
    assert_eq!((s.result, s.round), (Some(Some(stayer)), 0), "no real round played");
}

// --- Correction 10: Quarantine

/// 10. Quarantine with 2 players in a real round, one leaves: round over
/// at the next frame (no side wins), match over after the intermission, no
/// winner. In the warm-up the same: the match over after it.
#[test]
fn quarantine_one_player_left() {
    let mut sim = game("QRT", 12);
    let p: Vec<Entity> = (0..2).map(|_| join(&mut sim, JSF)).collect();
    past_warmup(&mut sim);
    sim.app.world_mut().despawn(p[0]);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, None);
    until(&mut sim, 8.0, |p| matches!(p, Phase::Intermission { .. }));
    assert_eq!(m(&sim).result, Some(None));
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);

    let mut sim = game("QRT", 12);
    let p: Vec<Entity> = (0..2).map(|_| join(&mut sim, JSF)).collect();
    until_live(&mut sim);
    assert!(m(&sim).warmup);
    sim.app.world_mut().despawn(p[1]);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    assert_eq!((m(&sim).result, m(&sim).round), (Some(None), 0));
}

// --- Correction 11, 12: Spy Hunt

/// Spy Hunt in stage 2 with `n` players: p[0] holds all 5 intel (the final
/// spy), the rest hunters.
fn stage_two(n: usize) -> (Sim, Vec<Entity>) {
    let mut sim = game("BNT", 12);
    let spots: Vec<Vec3> = (0..5).map(|i| Vec3::new(-6.0 + 3.0 * i as f32, 1.0, 0.0)).collect();
    objects(&mut sim).drops = spots.iter().map(|p| drop_point("BountyHunter", *p)).collect();
    let p: Vec<Entity> = (0..n).map(|_| join(&mut sim, JSF)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    for s in &spots {
        hold_at(&mut sim, p[0], *s, 0.05);
    }
    assert_eq!(objectives(&sim).stage, 2);
    assert_eq!(team(&sim, p[0]), 2, "the final spy");
    assert_eq!(stats(&sim, p[0]).mission_points, 5);
    unprotect_all(&mut sim);
    (sim, p)
}

/// 101. Spy Hunt stage 2: the final spy kills two hunters (no kills or
/// deaths added), hunter H kills the spy: H's mission points 5, the spy's
/// 0, Alpha wins. Final spy against one hunter, the hunter leaves: Alpha
/// wins, the spy's points 0.
#[test]
fn case_101_spy_hunt_stage_two() {
    let (mut sim, p) = stage_two(4);
    kill_now(&mut sim, p[1], p[0]);
    kill_now(&mut sim, p[2], p[0]);
    assert_eq!(
        (
            stats(&sim, p[0]).kills,
            stats(&sim, p[1]).deaths,
            stats(&sim, p[2]).deaths
        ),
        (0, 0, 0)
    );
    kill_now(&mut sim, p[0], p[3]);
    assert_eq!(m(&sim).winner, Some(JSF));
    assert_eq!(
        (stats(&sim, p[3]).mission_points, stats(&sim, p[0]).mission_points),
        (5, 0)
    );
    assert_eq!((stats(&sim, p[3]).kills, stats(&sim, p[0]).deaths), (0, 0));

    let (mut sim, p) = stage_two(2);
    sim.app.world_mut().despawn(p[1]);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
    assert_eq!(stats(&sim, p[0]).mission_points, 0);
}

/// 12. Stage 1: a leaver who leaves one player or fewer ends the round,
/// the hunters winning.
#[test]
fn spy_hunt_stage_one_leaving() {
    let mut sim = game("BNT", 12);
    let p: Vec<Entity> = (0..2).map(|_| join(&mut sim, JSF)).collect();
    past_warmup(&mut sim);
    sim.app.world_mut().despawn(p[0]);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
}

// --- Correction 13: the Explode / Defuse column

/// 102. Plant at t = 100 s by P, the last Creed player dies at t = 110 s:
/// JSF wins, P's detonations +1 at the round result, no plant counted. A
/// defuse round: the planter gets nothing, the defuser a defuse. The row's
/// column shows both. Warm-up: nothing credited.
#[test]
fn case_102_planter_credit() {
    let (mut sim, a, b) = sd(2, 1);
    sim.seconds(95.0);
    plant(&mut sim, a[0]);
    sim.seconds(5.0);
    kill_now(&mut sim, b[0], a[1]);
    assert_eq!(m(&sim).winner, Some(JSF));
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    let s = stats(&sim, a[0]);
    assert_eq!((s.detonations, s.plants, s.defuses), (1, 0, 0));
    let rows = scoreboard::rows(sim.app.world_mut());
    let row = rows.iter().find(|r| r.player == a[0]).unwrap();
    assert_eq!(row.mode_cell(ScoreOrder::ExplodeDefuse).as_deref(), Some("1 / 0"));

    let (mut sim, a, b) = sd(1, 1);
    plant(&mut sim, a[0]);
    put(&mut sim, a[0], Vec3::new(10.0, 1.0, 10.0));
    sim.intent(b[0]).use_key = true;
    hold_at(&mut sim, b[0], site(), 7.5);
    assert_eq!(m(&sim).winner, Some(CREED));
    assert_eq!(stats(&sim, a[0]).detonations, 0);
    assert_eq!(stats(&sim, b[0]).defuses, 1);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(stats(&sim, a[0]).detonations, 0, "no credit for a defused bomb");

    // The planter left: not credited.
    let (mut sim, a, b) = sd(2, 1);
    plant(&mut sim, a[0]);
    sim.app.world_mut().despawn(a[0]);
    kill_now(&mut sim, b[0], a[1]);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));

    // The warm-up has no bomb and credits nothing.
    let mut sim = game("TMM", 12);
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    kill_now(&mut sim, b, a);
    assert_eq!(stats(&sim, a), CaStats::default());
}

/// 13. Bombing Run credits the planter alike and counts plants.
#[test]
fn bombing_run_counts_plants() {
    let (mut sim, a, _b) = bombing(1, 1);
    let bomb = Vec3::new(0.0, 1.0, -4.0);
    hold_at(&mut sim, a[0], bomb, 0.2);
    assert!(carry(&sim, a[0]).bomb, "the carrier");
    select_bomb(&mut sim, a[0]);
    let zone = Vec3::new(6.0, 1.0, -14.0);
    sim.intent(a[0]).fire = true;
    hold_at(&mut sim, a[0], zone, 4.2);
    sim.intent(a[0]).fire = false;
    assert_eq!(stats(&sim, a[0]).plants, 1);
}

// --- Correction 14: One Man Army and Arms Race

/// 103. One Man Army G = 30, A and B on 29: A's kill then B's kill in one
/// frame: the winner is B. A leaver's kills count in the leaving rule's
/// share: 5 kills of a player who left, 2 remain with none between them,
/// one leaves: the other wins (5 > round(0.1 x 30) = 3).
#[test]
fn case_103_two_kills_at_g_in_a_frame() {
    for key in ["FFA", "VWRM"] {
        let mut sim = game(key, 12);
        settings(&mut sim).goal = 30;
        let p: Vec<Entity> = (0..4).map(|_| join(&mut sim, JSF)).collect();
        until_live(&mut sim);
        unprotect_all(&mut sim);
        for e in [p[0], p[1]] {
            sim.app
                .world_mut()
                .entity_mut(e)
                .insert(CaStats { kills: 29, ..default() });
        }
        let rifle = Some(combat_arms::game::weapons::RIFLE);
        hit(&mut sim, p[2], Some(p[0]), 50.0, DamageKind::Bullet, rifle);
        hit(&mut sim, p[3], Some(p[1]), 50.0, DamageKind::Bullet, rifle);
        sim.ticks(1);
        assert_eq!(m(&sim).winner, Some(team(&sim, p[1])), "{key}: the later wins");
    }

    let mut sim = game("FFA", 12);
    settings(&mut sim).goal = 30;
    let p: Vec<Entity> = (0..8).map(|_| join(&mut sim, JSF)).collect();
    until_live(&mut sim);
    unprotect_all(&mut sim);
    for v in &p[0..5] {
        kill_now(&mut sim, *v, p[7]);
    }
    assert_eq!(sim.app.world().resource::<CaKillsMade>().0, 5);
    let stayer = team(&sim, p[6]);
    // The killer and the dead leave; the last two have no kills.
    for e in [p[7], p[0], p[1], p[2], p[3], p[4]] {
        sim.app.world_mut().despawn(e);
        sim.ticks(1);
        assert_eq!(m(&sim).winner, None);
    }
    sim.app.world_mut().despawn(p[5]);
    assert_eq!(m(&sim).winner, Some(stayer));
}

// --- Correction 15: Capture the Flag

/// 15. CTF G = 5: a time-out with Alpha 4, Bravo 4 decided Alpha on
/// returns; during the 3 s hold Bravo captures (5): the winner is Bravo.
#[test]
fn ctf_later_capture_changes_the_winner() {
    let home_a = Vec3::new(-8.0, 1.0, 8.0);
    let home_b = Vec3::new(8.0, 1.0, -8.0);
    let mut sim = game("CTF", 12);
    settings(&mut sim).goal = 5;
    let (fa, za) = flag_and_zone(JSF, home_a);
    let (fb, zb) = flag_and_zone(CREED, home_b);
    {
        let mut o = objects(&mut sim);
        o.flags = vec![fa, fb];
        o.flag_zones = vec![za, zb];
    }
    join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    live(&mut sim);
    {
        let mut s = sim.app.world_mut().resource_mut::<CaMatch>();
        s.score = [4, 4];
        s.returns = [1, 0];
    }
    hold_at(&mut sim, b, home_a, 0.2);
    assert_eq!(carry(&sim, b).flag, Some(JSF));
    let t = now(&sim);
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(t + 0.1);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, Some(JSF), "decided on returns");
    hold_at(&mut sim, b, home_b, 0.2);
    assert_eq!(m(&sim).score, [4, 5]);
    assert_eq!(m(&sim).winner, Some(CREED));
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(CREED)));
}

// --- Correction 16: Quarantine's setup

/// 16. `ca_object_start 5`: Quarantine's setup lasts one frame, S&D's 5 s.
#[test]
fn case_16_quarantine_setup_is_one_frame() {
    for (key, long) in [("QRT", false), ("TMM", true)] {
        let mut sim = game(key, 12);
        settings(&mut sim).object_start = 5.0;
        join(&mut sim, JSF);
        join(&mut sim, CREED);
        until(&mut sim, 1.0, |p| matches!(p, Phase::Setup { .. }));
        let t = now(&sim);
        until(&mut sim, 6.0, |p| p == Phase::Live);
        let took = now(&sim) - t;
        assert_eq!(took > 1.0, long, "{key}: {took:.2} s");
    }
}

// --- game_modes_2.md's corrections

/// GM2 1: VIP Escort and Hired Guns end the match after a warm-up that
/// left a team empty (Escort: more round wins, here Alpha 2 - 1; Hired
/// Guns the non-empty team).
#[test]
fn escort_ends_on_an_empty_team_after_the_warm_up() {
    let mut sim = game("ESC", 8);
    settings(&mut sim).goal = 7;
    join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    assert!(m(&sim).warmup);
    sim.app.world_mut().resource_mut::<CaMatch>().round_wins = [2, 1];
    sim.app.world_mut().despawn(b);
    until(&mut sim, 40.0, |p| p == Phase::MatchOver);
    let s = m(&sim);
    assert_eq!((s.result, s.round), (Some(Some(JSF)), 0));
}

/// GM2 1 (Hired Guns): the non-empty team wins.
#[test]
fn hired_guns_end_on_an_empty_team_after_the_warm_up() {
    let mut sim = game("SQM", 8);
    join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    assert!(m(&sim).warmup);
    sim.app.world_mut().despawn(b);
    until(&mut sim, 40.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
}

/// GM2 2: VIP Escort's join count includes the joiner: a real round live
/// 20 s with 2 players in, a third is held; 0.5 s after going live it
/// spawns at once.
#[test]
fn escort_late_join_count() {
    for (after, held) in [(20.0, true), (0.5, false)] {
        let mut sim = game("ESC", 8);
        settings(&mut sim).goal = 7;
        join(&mut sim, JSF);
        join(&mut sim, CREED);
        past_warmup(&mut sim);
        let live_for = now(&sim) - m(&sim).live_since;
        sim.seconds(after - live_for);
        let p = late(&mut sim, JSF);
        assert_eq!(dead(&sim, p), held, "{after} s");
    }
}

/// GM2 3: damage from an earlier round never earns an assist in VIP
/// Escort either (the mission start clears every sum).
#[test]
fn escort_assists_dont_cross_rounds() {
    let mut sim = game("ESC", 8);
    settings(&mut sim).goal = 7;
    settings(&mut sim).modes.get_mut("ESC").unwrap().time_limit = Some(2.0);
    let a = join(&mut sim, JSF);
    let c = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    join(&mut sim, CREED);
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    hit(
        &mut sim,
        b,
        Some(a),
        0.60,
        DamageKind::Bullet,
        Some(combat_arms::game::weapons::RIFLE),
    );
    sim.ticks(1);
    until(&mut sim, 6.0, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 8.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    unprotect_all(&mut sim);
    assert_eq!(sim.app.world().resource::<CaRuleSettings>().mode, "ESC");
    kill_now(&mut sim, b, c);
    assert_eq!(stats(&sim, a).assists, 0);
}
