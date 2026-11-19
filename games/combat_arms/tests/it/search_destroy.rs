//! Search & Destroy (specs/combat_arms/game_modes.md M3): its test cases
//! 34-42 and case 17 as S&D has it, on `CaSim` and the greybox with the
//! common setup (`mode_kit`), site A (`Doobic`, defended by Creed) at the
//! origin with the stock box, and S&D for the Go4CA Cup's fixed round
//! total (M17).

use bevy::prelude::*;
use chimera_core::core::DamageKind;
use combat_arms::game::{
    grenades::M67,
    rules::{
        CaMatch, Phase,
        objectives::{ActionKind, BOMB},
        objects::ObjBox,
    },
};

use crate::mode_kit::*;

/// Site A's centre.
fn site() -> Vec3 {
    Vec3::new(0.0, 1.0, 0.0)
}

/// S&D past its warm-up with `jsf` and `creed` players, site A placed,
/// everyone unprotected and the bombs handed out.
fn game_sd(jsf: usize, creed: usize) -> (chimera_world::harness::Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = game("TMM", 12);
    objects(&mut sim).bomb_zones = vec![bomb_zone("Doobic", 0, CREED, site())];
    let a: Vec<Entity> = (0..jsf).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..creed).map(|_| join(&mut sim, CREED)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(3);
    (sim, a, b)
}

/// `p` selects the bomb, stands on site A and holds fire for `secs`.
fn plant(sim: &mut chimera_world::harness::Sim, p: Entity, secs: f64) {
    select_bomb(sim, p);
    assert!(wait_for(sim, 1.0, |s| in_hand(s, p) == Some(BOMB)), "the bomb in hand");
    sim.intent(p).fire = true;
    hold_at(sim, p, site(), secs);
    sim.intent(p).fire = false;
}

/// 34. Round 1's setup with 4 living JSF players: all 4 hold the bomb; the
/// warm-up's setup gives nobody one.
#[test]
fn case_34_bomb_hand_out() {
    let mut sim = game("TMM", 12);
    let jsf: Vec<Entity> = (0..4).map(|_| join(&mut sim, JSF)).collect();
    let creed = join(&mut sim, CREED);
    until_live(&mut sim);
    sim.ticks(3);
    assert!(m(&sim).warmup);
    for p in jsf.iter().chain([&creed]) {
        assert!(!weapons(&sim, *p).contains(&BOMB), "no bomb in the warm-up");
    }
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    sim.ticks(3);
    for p in &jsf {
        assert!(weapons(&sim, *p).contains(&BOMB), "every JSF player has the bomb");
        assert_ne!(in_hand(&sim, *p), Some(BOMB), "not drawn");
    }
    assert!(!weapons(&sim, creed).contains(&BOMB));
}

/// 35. A plant completing with 80 s left: the round timer then has 120 s
/// left, the bomb 40 s; at its end it detonates and JSF wins.
#[test]
fn case_35_plant_and_detonate() {
    let (mut sim, a, _b) = game_sd(1, 1);
    let t = now(&sim);
    // So that 80 s are left when the plant (3.9 s: the bomb's fire key)
    // completes.
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(t + 84.0 + 0.5);
    plant(&mut sim, a[0], 4.0);
    let bomb = objectives(&sim).planted.expect("planted");
    let n = now(&sim);
    let left = m(&sim).round_ends.unwrap() - n;
    assert!((left - 120.0).abs() < 0.6, "round timer {left:.2} s left");
    assert!(
        ((bomb.detonates - n) - 40.0).abs() < 0.6,
        "bomb {:.2}",
        bomb.detonates - n
    );
    assert!(!weapons(&sim, a[0]).contains(&BOMB), "the planter's bomb is gone");
    assert_eq!(stats(&sim, a[0]).plants, 0, "S&D keeps no plant statistic");
    assert_eq!(phase(&sim), Phase::Live);
    let to_go = bomb.detonates - now(&sim);
    sim.seconds(to_go - 0.2);
    assert_eq!(phase(&sim), Phase::Live, "not before the fuse");
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
    assert_eq!(stats(&sim, a[0]).detonations, 1);
}

/// 36. A Creed player defusing for 7 s: defused, Creed wins.
#[test]
fn case_36_defuse() {
    let (mut sim, a, b) = game_sd(1, 1);
    plant(&mut sim, a[0], 4.2);
    assert!(objectives(&sim).planted.is_some());
    put(&mut sim, a[0], point(JSF, 0));
    sim.intent(b[0]).use_key = true;
    hold_at(&mut sim, b[0], site(), 6.9);
    assert!(objectives(&sim).planted.is_some(), "not before 7 s");
    assert_eq!(action(&sim, b[0]).map(|a| a.kind), Some(ActionKind::Defuse));
    hold_at(&mut sim, b[0], site(), 0.2);
    assert!(objectives(&sim).planted.is_none(), "defused");
    assert_eq!(m(&sim).winner, Some(CREED));
    assert_eq!(stats(&sim, b[0]).defuses, 1);
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(CREED));
}

/// 37. A defuse held 3 s and let go: not defused (nothing kept); the bomb
/// still detonates, JSF wins.
#[test]
fn case_37_interrupted_defuse() {
    let (mut sim, a, b) = game_sd(1, 1);
    plant(&mut sim, a[0], 4.2);
    sim.intent(b[0]).use_key = true;
    hold_at(&mut sim, b[0], site(), 3.0);
    sim.intent(b[0]).use_key = false;
    hold_at(&mut sim, b[0], site(), 0.1);
    assert!(action(&sim, b[0]).is_none(), "the gauge emptied");
    // Again for 5 s: not enough from nothing.
    sim.intent(b[0]).use_key = true;
    hold_at(&mut sim, b[0], site(), 5.0);
    assert!(objectives(&sim).planted.is_some());
    sim.intent(b[0]).use_key = false;
    until(&mut sim, 40.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
}

/// 38. The last JSF player dies after the plant: the round goes on; the
/// detonation gives it to JSF.
#[test]
fn case_38_attackers_wiped_after_plant() {
    let (mut sim, a, b) = game_sd(1, 1);
    plant(&mut sim, a[0], 4.2);
    sim.seconds(1.0);
    kill_now(&mut sim, a[0], b[0]);
    assert!(dead(&sim, a[0]));
    sim.ticks(5);
    assert_eq!(phase(&sim), Phase::Live, "the round continues");
    assert_eq!(m(&sim).winner, None);
    until(&mut sim, 40.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
}

/// 39. The last Creed player dies after the plant: JSF wins at once.
#[test]
fn case_39_defenders_wiped_after_plant() {
    let (mut sim, a, b) = game_sd(1, 1);
    plant(&mut sim, a[0], 4.2);
    sim.seconds(1.0);
    kill_now(&mut sim, b[0], a[0]);
    assert_eq!(m(&sim).winner, Some(JSF));
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
}

/// 40. No plant, the round's 180 s pass: Creed wins.
#[test]
fn case_40_time_out() {
    let (mut sim, ..) = game_sd(1, 1);
    let left = m(&sim).round_ends.unwrap() - now(&sim);
    assert!((179.0..=180.0).contains(&left), "{left}");
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(now(&sim) + 1.0);
    until(&mut sim, 2.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(CREED));
}

/// 41. The plant zone's margin: site at the origin, half-extents (130, 64,
/// 125): a planter at x = 175 u is inside (<= 180), at 181 u outside, no
/// plant.
#[test]
fn case_41_plant_zone_margin() {
    let zone = ObjBox {
        centre: Vec3::ZERO,
        half: Vec3::new(u(130.0), u(64.0), u(125.0)),
    };
    assert!(zone.contains(Vec3::new(u(175.0), 0.0, 0.0), u(50.0)));
    assert!(!zone.contains(Vec3::new(u(181.0), 0.0, 0.0), u(50.0)));
    for (x, plants) in [(175.0, true), (181.0, false)] {
        let (mut sim, a, _b) = game_sd(1, 1);
        select_bomb(&mut sim, a[0]);
        assert!(wait_for(&mut sim, 1.0, |s| in_hand(s, a[0]) == Some(BOMB)));
        sim.intent(a[0]).fire = true;
        hold_at(&mut sim, a[0], site() + Vec3::X * u(x), 1.0);
        let gauge = action(&sim, a[0]).filter(|g| g.kind == ActionKind::Plant);
        assert_eq!(gauge.is_some(), plants, "{x} u: {gauge:?}");
        hold_at(&mut sim, a[0], site() + Vec3::X * u(x), 3.2);
        assert_eq!(objectives(&sim).planted.is_some(), plants, "{x} u");
    }
}

/// 42. G = 5, round wins 3 - 3 after a round, then every Creed player
/// leaves: the match ends after the round and S&D picks by wins only,
/// 3 - 3: Bravo, the empty team. (Elimination Pro gives Alpha, the
/// non-empty team: game_modes.rs.)
#[test]
fn case_42_match_end_tie() {
    let (mut sim, a, b) = game_sd(1, 1);
    kill_now(&mut sim, b[0], a[0]);
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    sim.app.world_mut().resource_mut::<CaMatch>().round_wins = [3, 3];
    sim.app.world_mut().despawn(b[0]);
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).round_wins, [3, 3]);
    assert_eq!(m(&sim).result, Some(Some(CREED)));
}

/// 17 (S&D). A round decided at t = 0; a grenade thrown before explodes at
/// t = 1.0 s and kills a player: no kill, no death, no score; the round's
/// result unchanged.
#[test]
fn case_17_grenade_after_round_over() {
    let (mut sim, a, b) = game_sd(2, 1);
    // A teammate's grenade, so friendly fire on (blasts at full damage).
    settings(&mut sim).friendly_fire = 1;
    kill_now(&mut sim, b[0], a[0]);
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    let before = (stats(&sim, a[0]), stats(&sim, a[1]), m(&sim).round_wins);
    sim.seconds(1.0);
    hit(&mut sim, a[1], Some(a[0]), 5.0, DamageKind::Blast, Some(M67));
    sim.ticks(1);
    assert!(dead(&sim, a[1]), "the blast kills");
    assert_eq!((stats(&sim, a[0]), stats(&sim, a[1]), m(&sim).round_wins), before);
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
}

/// S&D's deaths (S7): a team kill costs the victim no death; a suicide
/// costs a death and a suicide.
#[test]
fn deaths_count_as_s7_has_them() {
    let (mut sim, a, _b) = game_sd(3, 1);
    settings(&mut sim).friendly_fire = 1;
    kill_now(&mut sim, a[1], a[0]);
    assert_eq!((stats(&sim, a[0]).friendly_kills, stats(&sim, a[1]).deaths), (1, 0));
    hit(&mut sim, a[2], Some(a[2]), 5.0, DamageKind::Blast, Some(M67));
    sim.ticks(1);
    let s = stats(&sim, a[2]);
    assert_eq!((s.deaths, s.suicides), (1, 1));
}

/// M17: the Go4CA Cup ends when both teams' wins add up to G (5).
#[test]
fn go4ca_cup_plays_a_fixed_number_of_rounds() {
    let mut sim = game("ESL", 12);
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.app.world_mut().resource_mut::<CaMatch>().round_wins = [2, 2];
    kill_now(&mut sim, b, a);
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).round_wins, [3, 2]);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
}
