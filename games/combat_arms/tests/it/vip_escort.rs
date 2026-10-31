//! VIP Escort (specs/combat_arms/game_modes_2.md M13): its test cases
//! 20-35 (23, the VIP's cell, needs the map's doors: not yet) on `CaSim`
//! and the greybox with the common setup (`mode_kit`): a row of start
//! points per team (Alpha's are the map's `JSF` side), the VIP's teleport
//! point and the exit (`EscapeZoneTrigger`, 128 x 80 x 128 u).

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, FreezeTime, Hitgroup, Intent};
use chimera_world::harness::Sim;
use combat_arms::game::{
    TICK_INTERVAL,
    inventory::NoPickups,
    movement::{CaMovement, VipMovement},
    rules::{
        CaMatch, Phase,
        escort::{self, CaEscort, VipEntry, choose_vip, rounds_to_swap, swap_period},
        objects::{EscapeZone, NamedPoint},
    },
    weapons::{RIFLE},
};

use crate::mode_kit::*;

/// The VIP's teleport point (its yaw 90 degrees) and the exit's centre.
const VIP_AT: Vec3 = Vec3::new(20.0, 1.0, 28.0);
const EXIT: Vec3 = Vec3::new(-10.0, 1.0, 28.0);

/// The greybox with VIP Escort's objects, playing `mode` (R `goal`).
fn esc_game(mode: &str, goal: u32) -> Sim {
    let mut sim = game(mode, 8);
    settings(&mut sim).goal = goal;
    let mut o = objects(&mut sim);
    o.points = vec![NamedPoint {
        class: "TeleportPoint".into(),
        name: "VipTeleportPoint".into(),
        at: Transform::from_translation(VIP_AT).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
    }];
    o.escape_zones = vec![EscapeZone {
        name: "EscapeZoneTrigger".into(),
        family: "VIPEscort".into(),
        area: obj_box(EXIT, Vec3::new(128.0, 80.0, 128.0)),
    }];
    sim
}

/// VIP Escort past its warm-up with `alpha` and `bravo` players.
fn esc(alpha: usize, bravo: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = esc_game("ESC", 7);
    let a: Vec<Entity> = (0..alpha).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..bravo).map(|_| join(&mut sim, CREED)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(2);
    (sim, a, b)
}

fn vip(sim: &mut Sim) -> Option<Entity> {
    escort::vip(sim.app.world_mut())
}

/// Run until `t` s after `t0`.
fn run_to(sim: &mut Sim, t0: f64, t: f64) {
    while now(sim) < t0 + t - 1e-9 {
        sim.ticks(1);
    }
}

/// When the round over started (its end minus `ObjectEnd`).
fn round_over_at(sim: &Sim) -> Option<f64> {
    match phase(sim) {
        Phase::RoundOver { until } => Some(until - 3.0),
        _ => None,
    }
}

/// 20. The VIP choice (US): escort players P1 (score 5), P2 and P3 (9,
/// P3 lower by chance): round 1 P2, round 2 P3, round 3 P1; after the
/// swap Bravo's best never-VIP player.
#[test]
fn case_20_vip_choice() {
    let entry = |escort, previous, turns, score, rank| VipEntry {
        escort,
        previous,
        turns,
        score,
        rank,
    };
    let bravo = entry(false, false, 0, 50, 9);
    // Round 1.
    let r1 = [entry(true, false, 0, 5, 1), entry(true, false, 0, 9, 2), entry(true, false, 0, 9, 0), bravo];
    assert_eq!(choose_vip(&r1), Some(1));
    // Round 2: P2 the previous VIP.
    let r2 = [entry(true, false, 0, 5, 2), entry(true, true, 1, 9, 1), entry(true, false, 0, 9, 0), bravo];
    assert_eq!(choose_vip(&r2), Some(2));
    // Round 3: P1 the only one never VIP.
    let r3 = [entry(true, false, 0, 5, 0), entry(true, false, 1, 9, 2), entry(true, true, 1, 9, 1), bravo];
    assert_eq!(choose_vip(&r3), Some(0));
    // Round 4, Bravo escorting: its best never-VIP player.
    let r4 = [
        entry(false, true, 1, 5, 0),
        entry(true, false, 0, 3, 2),
        entry(true, false, 0, 7, 1),
        entry(true, false, 1, 20, 3),
    ];
    assert_eq!(choose_vip(&r4), Some(2));

    // In play: three Alpha players take turns, nobody twice in a row.
    let mut sim = esc_game("ESC", 7);
    let a: Vec<Entity> = (0..3).map(|_| join(&mut sim, JSF)).collect();
    let _b: Vec<Entity> = (0..3).map(|_| join(&mut sim, CREED)).collect();
    until_live(&mut sim);
    assert!(vip(&mut sim).is_none(), "no VIP in the warm-up");
    let mut seen = Vec::new();
    for _ in 0..3 {
        until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
        until_live(&mut sim);
        let v = vip(&mut sim).expect("a VIP");
        assert!(a.contains(&v), "on the escort team");
        seen.push(v);
        let t = now(&sim);
        sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(t + 0.1);
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 3, "each had a turn");
}

/// 21. The VIP at the teleport point facing its yaw; the other escort
/// players at the escort (JSF) points, the elimination team at Creed's.
#[test]
fn case_21_vip_spawn() {
    let (mut sim, a, b) = esc(3, 3);
    let v = vip(&mut sim).unwrap();
    let at = sim.position(v);
    assert!((at.xz() - VIP_AT.xz()).length() < 0.05, "at the teleport point: {at}");
    let yaw = sim.app.world().get::<Intent>(v).unwrap().yaw;
    assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    for p in a.iter().filter(|p| **p != v) {
        assert!((sim.position(*p).z - point(JSF, 0).z).abs() < 0.5, "an escort point");
    }
    for p in &b {
        assert!((sim.position(*p).z - point(CREED, 0).z).abs() < 0.5, "a Creed point");
    }
}

/// 22. The VIP's loadout: its weapon only, 100 health and armour, P 0.44
/// whatever it wears: 285 x 0.56 = 159.6 u/s; no pick-ups.
#[test]
fn case_22_vip_loadout() {
    let (mut sim, _a, _b) = esc(2, 2);
    let v = vip(&mut sim).unwrap();
    sim.ticks(3);
    assert_eq!(weapons(&sim, v), vec!["combat_arms:beretta_m92fs"]);
    assert_eq!(health(&sim, v), 1.0);
    assert_eq!(sim.app.world().get::<chimera_sim::weapon::Armor>(v).map(combat_arms::game::weapons::points).unwrap(), 100.0);
    assert_eq!(sim.app.world().get::<VipMovement>(v).unwrap().penalty, 0.44);
    assert!(sim.app.world().get::<NoPickups>(v).is_some());
    sim.intent(v).move_axis = Vec2::Y;
    sim.ticks(10);
    let limit = sim.app.world().get::<CaMovement>(v).unwrap().speed_limit;
    assert!((limit - 159.6).abs() < 0.5, "{limit} u/s");
}

/// 24, as game_modes_2.md Follow-up (2026-10-31b) corrects it. The exit
/// opens 61 s after the setup (the exit timer's 60 s + its 1 s run-out):
/// the VIP in the zone at 60.95 s → nothing; at 75 s → the escort wins,
/// everyone frozen and immune; the round over at 78 s; Alpha's round wins
/// +1.
#[test]
fn case_24_exit_opens() {
    let (mut sim, _a, b) = esc(2, 2);
    let t0 = match m(&sim).round_ends {
        Some(e) => e - 180.0,
        None => panic!("a timer"),
    };
    let v = vip(&mut sim).unwrap();
    run_to(&mut sim, t0, 60.9);
    assert!(!objectives(&sim).exit.unwrap().open);
    hold_at(&mut sim, v, EXIT, 0.04);
    assert!(m(&sim).winner.is_none(), "shut until 61 s: {:?} at {}", m(&sim), now(&sim) - t0);
    put(&mut sim, v, VIP_AT);
    run_to(&mut sim, t0, 61.05);
    assert!(objectives(&sim).exit.unwrap().open, "open at 61 s");
    run_to(&mut sim, t0, 75.0);
    hold_at(&mut sim, v, EXIT, TICK_INTERVAL);
    let mm = m(&sim);
    assert_eq!(mm.winner, Some(JSF));
    assert!(mm.immune && sim.app.world().resource::<FreezeTime>().0, "frozen and immune");
    hit(&mut sim, b[0], Some(v), 0.5, DamageKind::Bullet, Some(RIFLE));
    sim.ticks(1);
    assert_eq!(health(&sim, b[0]), 1.0, "immune");
    until(&mut sim, 4.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert!((round_over_at(&sim).unwrap() - (t0 + 78.0)).abs() < 0.1);
    assert_eq!(m(&sim).round_wins, [1, 0]);
    assert_eq!(stats(&sim, v).vip_escapes, 1);
}

/// game_modes_2.md 54. Exit at 61 s: the round live at t = 0 with the
/// exit timer started then: the helicopter coming at t = 54.0; the zone
/// still shut at t = 60.95; open at t = 61.0.
#[test]
fn case_54_exit_at_61_s() {
    let (mut sim, _a, _b) = esc(2, 2);
    let t0 = m(&sim).round_ends.expect("a timer") - 180.0;
    run_to(&mut sim, t0, 53.95);
    assert!(!objectives(&sim).exit.unwrap().arriving, "not yet at 53.95 s");
    run_to(&mut sim, t0, 54.0);
    assert!(objectives(&sim).exit.unwrap().arriving, "the helicopter at 54 s");
    run_to(&mut sim, t0, 60.95);
    assert!(!objectives(&sim).exit.unwrap().open, "shut at 60.95 s");
    run_to(&mut sim, t0, 61.0);
    let x = objectives(&sim).exit.unwrap();
    assert!(x.open, "open at 61 s");
    assert!((x.opens - (t0 + 61.0)).abs() < 1e-6, "opens at {}", x.opens - t0);
}

/// 25. The VIP killed at 100 s → the elimination team; frozen; the round
/// over at 103 s. Killed by a fall → the same.
#[test]
fn case_25_vip_killed() {
    for fall in [false, true] {
        let (mut sim, _a, b) = esc(2, 2);
        let t0 = m(&sim).round_ends.unwrap() - 180.0;
        let v = vip(&mut sim).unwrap();
        run_to(&mut sim, t0, 100.0);
        if fall {
            sim.app.world_mut().write_message(Damage {
                target: v,
                attacker: Some(v),
                amount: 5.0,
                point: Vec3::ZERO,
                dir: Vec3::NEG_Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Crush,
                weapon: None,
                force: Vec3::ZERO,
            });
            sim.ticks(1);
        } else {
            kill_now(&mut sim, v, b[0]);
            assert_eq!(stats(&sim, b[0]).vip_kills, 1);
        }
        assert_eq!(m(&sim).winner, Some(CREED));
        assert!(m(&sim).immune);
        until(&mut sim, 4.0, |p| matches!(p, Phase::RoundOver { .. }));
        assert!((round_over_at(&sim).unwrap() - (t0 + 103.0)).abs() < 0.1);
    }
}

/// 26. The last elimination player dies at 90 s, the VIP alive: the
/// escort wins; the round over at 93 s.
#[test]
fn case_26_elimination_wiped() {
    let (mut sim, a, b) = esc(2, 2);
    let t0 = m(&sim).round_ends.unwrap() - 180.0;
    let v = vip(&mut sim).unwrap();
    let other = *a.iter().find(|p| **p != v).unwrap();
    kill_now(&mut sim, b[0], other);
    run_to(&mut sim, t0, 90.0);
    kill_now(&mut sim, b[1], other);
    assert_eq!(m(&sim).winner, Some(JSF));
    until(&mut sim, 4.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert!((round_over_at(&sim).unwrap() - (t0 + 93.0)).abs() < 0.1);
}

/// 27. The clock at 0 (180 s), the VIP alive: the elimination team,
/// unfrozen; the round over at 183 s; a kill at 181 s counts, the winner
/// stays.
#[test]
fn case_27_time_out() {
    let (mut sim, a, b) = esc(2, 2);
    let t0 = m(&sim).round_ends.unwrap() - 180.0;
    let v = vip(&mut sim).unwrap();
    let other = *a.iter().find(|p| **p != v).unwrap();
    run_to(&mut sim, t0, 180.02);
    assert_eq!(m(&sim).winner, Some(CREED));
    assert!(!m(&sim).immune && !sim.app.world().resource::<FreezeTime>().0, "not frozen");
    run_to(&mut sim, t0, 181.0);
    kill_now(&mut sim, b[0], other);
    assert_eq!(stats(&sim, other).kills, 1, "the kill counts");
    assert_eq!(m(&sim).winner, Some(CREED));
    until(&mut sim, 3.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert!((round_over_at(&sim).unwrap() - (t0 + 183.0)).abs() < 0.1);
    assert_eq!(m(&sim).round_wins, [0, 1]);
}

/// 28. The VIP touching the open exit on the frame the clock runs out:
/// refused, the elimination team wins by the time-out; a frame earlier:
/// the escort wins.
#[test]
fn case_28_last_frame() {
    for (early, want) in [(false, CREED), (true, JSF)] {
        let (mut sim, _a, _b) = esc(2, 2);
        let v = vip(&mut sim).unwrap();
        let t = now(&sim);
        {
            let w = sim.app.world_mut();
            let mut o = w.resource_mut::<combat_arms::game::rules::objectives::CaObjectives>();
            o.exit = Some(combat_arms::game::rules::objectives::Exit { opens: t, open: true, ..Default::default() });
        }
        // The round ends at the next tick (or the one after).
        let dt = TICK_INTERVAL;
        let ends = t + dt * if early { 2.0 } else { 1.0 } - 1e-6;
        sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(ends);
        put(&mut sim, v, EXIT);
        sim.ticks(1);
        assert_eq!(m(&sim).winner, Some(want), "early {early}");
    }
}

/// 29. The VIP leaves alive at 30 s: the elimination team wins; the round
/// over at 33 s.
#[test]
fn case_29_vip_leaves() {
    let (mut sim, _a, _b) = esc(2, 2);
    let t0 = m(&sim).round_ends.unwrap() - 180.0;
    let v = vip(&mut sim).unwrap();
    run_to(&mut sim, t0, 30.0);
    sim.app.world_mut().despawn(v);
    sim.ticks(1);
    assert_eq!(m(&sim).winner, Some(CREED));
    until(&mut sim, 4.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert!((round_over_at(&sim).unwrap() - (t0 + 33.0)).abs() < 0.1);
}

/// 30. The swap after round 3 (R 7, T 3): the escape at 0 s, the hold to
/// 3 s, round over 3-6 s, intermission 6-9 s, the wait to 12 s, the swap
/// with a 3 s notice, the warm-up live from 15 s; round 4 Bravo escorts
/// from the JSF points, Alpha's round wins still Alpha's.
#[test]
fn case_30_swap_timing() {
    let (mut sim, a, b) = esc(2, 2);
    sim.app.world_mut().resource_mut::<CaMatch>().round = 2;
    let v = vip(&mut sim).unwrap();
    let now0 = now(&sim);
    {
        let mut o = sim.app.world_mut().resource_mut::<combat_arms::game::rules::objectives::CaObjectives>();
        o.exit = Some(combat_arms::game::rules::objectives::Exit { opens: now0, open: true, ..Default::default() });
    }
    hold_at(&mut sim, v, EXIT, TICK_INTERVAL);
    let t0 = now(&sim) - TICK_INTERVAL;
    assert_eq!(m(&sim).winner, Some(JSF));
    until(&mut sim, 4.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert!((round_over_at(&sim).unwrap() - (t0 + 3.0)).abs() < 0.1);
    assert_eq!(objectives(&sim).rounds_to_swap, 1);
    until(&mut sim, 4.0, |p| matches!(p, Phase::Intermission { .. }));
    let Phase::Intermission { until: end } = phase(&sim) else { unreachable!() };
    assert!((end - (t0 + 15.0)).abs() < 0.1, "intermission and the wait and notice to 15 s");
    run_to(&mut sim, t0, 12.1);
    assert!(sim.app.world().resource::<CaEscort>().swapped, "swapped at 12 s");
    assert!(objectives(&sim).swap_notice.is_some_and(|n| (n - (t0 + 15.0)).abs() < 0.1));
    until(&mut sim, 4.0, |p| p == Phase::Live);
    assert!(m(&sim).warmup, "a warm-up after the swap");
    assert!((now(&sim) - (t0 + 15.0)).abs() < 0.2);
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    assert!(!m(&sim).warmup);
    assert!((now(&sim) - (t0 + 31.0)).abs() < 0.3, "round 4 live at about 31 s");
    let v = vip(&mut sim).unwrap();
    assert!(b.contains(&v), "Bravo escorts");
    for p in b.iter().filter(|p| **p != v) {
        assert!((sim.position(*p).z - point(JSF, 0).z).abs() < 0.5, "Bravo at the JSF points");
    }
    for p in &a {
        assert!((sim.position(*p).z - point(CREED, 0).z).abs() < 0.5, "Alpha at the Creed points");
    }
    assert_eq!(m(&sim).round_wins, [1, 0], "Alpha keeps its win");
}

/// 31. The swap period: R 5 → after rounds 2, 4, 6, 8; R 9 → 4, 8, 12,
/// 16; R 11 → 5, 10, 15, 20; R 7 ends at most after round 13.
#[test]
fn case_31_swap_period() {
    assert_eq!([5, 7, 9, 11].map(swap_period), [2, 3, 4, 5]);
    for (r, swaps) in [(5u32, [2u32, 4, 6, 8]), (9, [4, 8, 12, 16]), (11, [5, 10, 15, 20])] {
        let t = swap_period(r);
        let got: Vec<u32> = (1..=2 * r - 1).filter(|k| k % t == 0).collect();
        assert_eq!(got, swaps);
    }
    // R 7: 7 - 6 is the longest match.
    assert_eq!(7 + 6, 2 * 7 - 1);
}

/// 32. The rounds until the swap, R 7: 3, 2, 1, 3 after rounds 1-4; none
/// (255) after the last swap.
#[test]
fn case_32_swap_hud() {
    assert_eq!([1, 2, 3, 4].map(|k| rounds_to_swap(k, 7)), [3, 2, 1, 3]);
    assert_eq!(rounds_to_swap(13, 7), 255);
}

/// 33. Alpha's 7th round win at round 12 (a multiple of T): the match is
/// over, Alpha wins, no swap.
#[test]
fn case_33_match_end() {
    let (mut sim, _a, b) = esc(2, 2);
    {
        let mut mm = sim.app.world_mut().resource_mut::<CaMatch>();
        mm.round = 11;
        mm.round_wins = [6, 5];
    }
    // Alpha (escort) wins by wiping Bravo.
    let v = vip(&mut sim).unwrap();
    kill_now(&mut sim, b[0], v);
    kill_now(&mut sim, b[1], v);
    until(&mut sim, 10.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
    assert!(sim.app.world().resource::<CaEscort>().swap_at.is_none(), "no swap");
}

/// 34. A 9th player 20 s into a live round is held until the next round's
/// setup; one joining in the warm-up spawns at once.
#[test]
fn case_34_join() {
    let mut sim = esc_game("ESC", 7);
    let _a: Vec<Entity> = (0..4).map(|_| join(&mut sim, JSF)).collect();
    let _b: Vec<Entity> = (0..4).map(|_| join(&mut sim, CREED)).collect();
    until_live(&mut sim);
    let early = join(&mut sim, JSF);
    combat_arms::game::rules::flow::enter_game(sim.app.world_mut(), early);
    sim.ticks(1);
    assert!(!dead(&sim, early), "the warm-up: in at once");
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    sim.ticks((20.0 / TICK_INTERVAL) as u64);
    let late = join(&mut sim, CREED);
    combat_arms::game::rules::flow::enter_game(sim.app.world_mut(), late);
    sim.ticks(1);
    assert!(dead(&sim, late), "held");
}

/// 35. Clan VIP Escort, R 7: exactly 7 rounds (every one won by the
/// elimination team on time), wins 4 - 3, swaps after rounds 3 and 6.
#[test]
fn case_35_clan_vip_escort() {
    let mut sim = esc_game("CESC", 7);
    settings(&mut sim).modes.get_mut("CESC").unwrap().time_limit = Some(1.0);
    let _a: Vec<Entity> = (0..2).map(|_| join(&mut sim, JSF)).collect();
    let _b: Vec<Entity> = (0..2).map(|_| join(&mut sim, CREED)).collect();
    let mut swaps = Vec::new();
    let mut was = false;
    for _ in 0..3000 {
        sim.ticks(10);
        let s = sim.app.world().resource::<CaEscort>().swapped;
        if s != was {
            swaps.push(m(&sim).round);
            was = s;
        }
        if phase(&sim) == Phase::MatchOver {
            break;
        }
    }
    let mm = m(&sim);
    assert_eq!(phase(&sim), Phase::MatchOver);
    assert_eq!(mm.round, 7, "exactly 7 rounds");
    assert_eq!(mm.round_wins, [3, 4]);
    assert_eq!(mm.result, Some(Some(CREED)));
    assert_eq!(swaps, [3, 6]);
}
