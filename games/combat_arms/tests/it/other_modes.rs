//! The other team modes (specs/combat_arms/game_modes.md M8, M9, M10,
//! M12): Quarantine Regen's test cases 55-57, Bombing Run's 58-59, Seize
//! and Secure's 60-62 and case 13, and VIP Elimination, on `CaSim` and
//! the greybox (`mode_kit`).

use bevy::prelude::*;
use chimera_core::core::DamageKind;
use chimera_world::harness::Sim;
use combat_arms::game::{
    movement::CaInfected,
    rules::{
        CaMatch, Phase,
        objectives::{ActionKind, BOMB, CaObjectives, Pole},
        quarantine::hosts,
        vip::CaVipRank,
    },
    weapons::RIFLE,
};

use crate::mode_kit::*;

// --- M8 Quarantine Regen

/// Quarantine past its warm-up with `n` players (all humans: Alpha).
fn quarantine(n: usize) -> (Sim, Vec<Entity>) {
    let mut sim = game("QRT", 12);
    let p: Vec<Entity> = (0..n)
        .map(|_| join(&mut sim, if n % 2 == 0 { CREED } else { JSF }))
        .collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    (sim, p)
}

/// 55. Hosts: 8 players, round(1.6) = 2; 12, round(2.4) = 2; 13,
/// round(2.6) = 3; 3 players, 1; 20, 4.
#[test]
fn case_55_hosts() {
    for (n, h) in [(8, 2), (12, 2), (13, 3), (3, 1), (20, 4), (4, 2)] {
        assert_eq!(hosts(n, 0.2, 2), h, "{n} players");
    }
}

/// Hosts picked 20 s in: everyone human before, H infected after (Bravo,
/// knife only); an infected killing a human infects it.
#[test]
fn hosts_are_picked_and_spread_it() {
    let (mut sim, p) = quarantine(8);
    assert!(p.iter().all(|e| team(&sim, *e) == JSF), "all human");
    sim.seconds(19.9);
    assert!(p.iter().all(|e| team(&sim, *e) == JSF));
    sim.seconds(0.2);
    let infected: Vec<Entity> = p.iter().copied().filter(|e| team(&sim, *e) == CREED).collect();
    assert_eq!(infected.len(), 2);
    sim.ticks(3);
    for e in &infected {
        assert!(sim.app.world().get::<CaInfected>(*e).is_some());
        assert_eq!(
            weapons(&sim, *e),
            [combat_arms::game::inventory::KNIFE],
            "the knife only"
        );
    }
    unprotect_all(&mut sim);
    let human = p.iter().copied().find(|e| team(&sim, *e) == JSF).unwrap();
    kill_now(&mut sim, human, infected[0]);
    assert_eq!(team(&sim, human), CREED, "infected");
    assert!(!dead(&sim, human), "back as infected");
    assert_eq!(stats(&sim, infected[0]).mission_points, 2);
    unprotect_all(&mut sim);
    let other = p.iter().copied().find(|e| team(&sim, *e) == JSF).unwrap();
    kill_now(&mut sim, infected[1], other);
    assert_eq!(
        (
            stats(&sim, other).mission_points,
            stats(&sim, infected[1]).mission_points
        ),
        (3, -1)
    );
}

/// 56. Three minutes pass with humans alive: the humans win; the living
/// humans +10 points each.
#[test]
fn case_56_time_out() {
    let (mut sim, p) = quarantine(4);
    sim.seconds(20.2);
    let humans: Vec<Entity> = p.iter().copied().filter(|e| team(&sim, *e) == JSF).collect();
    assert_eq!(humans.len(), 2);
    let left = m(&sim).round_ends.unwrap() - now(&sim);
    assert!((159.0..=160.0).contains(&left), "3 minutes a round: {left}");
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(now(&sim));
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
    for h in humans {
        assert_eq!(stats(&sim, h).mission_points, 10);
    }
}

/// 57. 4 players, 2 hosts needed, only 2 humans alive at 20 s: the round
/// is void, no winner.
#[test]
fn case_57_void_round() {
    let (mut sim, p) = quarantine(4);
    settings(&mut sim).friendly_fire = 1;
    kill_now(&mut sim, p[2], p[0]);
    kill_now(&mut sim, p[3], p[0]);
    sim.seconds(20.2);
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, None);
    assert_eq!(m(&sim).round_wins, [0, 0]);
}

// --- M9 Bombing Run

/// Bombing Run past its warm-up, Alpha's site (defended by Alpha) and
/// Bravo's, the bomb's drop point.
fn bombing_run() -> (Sim, Entity, Entity) {
    let mut sim = game("SBT", 12);
    {
        let mut o = objects(&mut sim);
        o.bomb_zones = vec![
            bomb_zone("Sabotage", 0, JSF, Vec3::new(-6.0, 1.0, 6.0)),
            bomb_zone("Sabotage", 1, CREED, Vec3::new(6.0, 1.0, -14.0)),
        ];
        o.drops = vec![drop_point("Sabotage", Vec3::new(0.0, 1.0, -4.0))];
    }
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    (sim, a, b)
}

/// 58. 300 s pass without a detonation: Alpha and Bravo each +1 round
/// win.
#[test]
fn case_58_time_out() {
    let (mut sim, ..) = bombing_run();
    let left = m(&sim).round_ends.unwrap() - now(&sim);
    assert!((299.0..=300.0).contains(&left), "{left}");
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(now(&sim));
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).round_wins, [1, 1]);
    assert_eq!(m(&sim).results.last().unwrap().winner, None);
}

/// 59. Bravo picks the bomb up, plants it in Alpha's zone; it detonates:
/// Bravo wins the round.
#[test]
fn case_59_detonation() {
    let (mut sim, a, b) = bombing_run();
    let bomb = objectives(&sim).loose_bomb.expect("the bomb on its drop point");
    hold_at(&mut sim, b, bomb, 0.05);
    assert!(carry(&sim, b).bomb, "taken");
    assert!(weapons(&sim, b).contains(&BOMB));
    // Carried to Bravo's own site: no plant there.
    select_bomb(&mut sim, b);
    assert!(wait_for(&mut sim, 1.0, |s| in_hand(s, b) == Some(BOMB)));
    sim.intent(b).fire = true;
    hold_at(&mut sim, b, Vec3::new(6.0, 1.0, -14.0), 1.0);
    assert!(action(&sim, b).is_none(), "not at your own site");
    hold_at(&mut sim, b, Vec3::new(-6.0, 1.0, 6.0), 4.2);
    sim.intent(b).fire = false;
    let planted = objectives(&sim).planted.expect("planted at Alpha's site");
    assert_eq!(planted.team, CREED);
    put(&mut sim, a, Vec3::new(0.0, 1.0, 0.0));
    until(&mut sim, 41.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(CREED));
}

/// M9: the carrier's death drops the bomb where it was.
#[test]
fn the_carrier_drops_the_bomb() {
    let (mut sim, a, b) = bombing_run();
    let bomb = objectives(&sim).loose_bomb.unwrap();
    hold_at(&mut sim, b, bomb, 0.05);
    let there = Vec3::new(3.0, 1.0, 3.0);
    put(&mut sim, b, there);
    kill_now(&mut sim, b, a);
    let on_ground = objectives(&sim).loose_bomb.expect("dropped");
    assert!(on_ground.xz().distance(there.xz()) < 1e-3);
    assert!(!carry(&sim, b).bomb && !weapons(&sim, b).contains(&BOMB));
}

// --- M10 Seize and Secure

const TRIGGER_A: Vec3 = Vec3::new(-6.0, 1.0, 4.0);
const TRIGGER_B: Vec3 = Vec3::new(6.0, 1.0, -12.0);

/// Seize and Secure past its warm-up, its two triggers placed.
fn seize(goal: u32) -> (Sim, Entity, Entity) {
    let mut sim = game("HQM", 12);
    settings(&mut sim).goal = goal;
    objects(&mut sim).triggers = vec![
        trigger("ActivateTrigger_A", TRIGGER_A),
        trigger("ActivateTrigger_B", TRIGGER_B),
    ];
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    (sim, a, b)
}

/// `p` uses its trigger at `at` for the seize time.
fn seize_with(sim: &mut Sim, p: Entity, at: Vec3) {
    sim.intent(p).use_key = true;
    hold_at(sim, p, at, 5.05);
    sim.intent(p).use_key = false;
}

fn pole(sim: &Sim) -> Pole {
    objectives(sim).pole.expect("a pole")
}

/// 13. Friendly fire on in Seize and Secure: a teammate's rifle 40 → 40.
#[test]
fn case_13_friendly_fire_in_seize_and_secure() {
    let mut sim = game("HQM", 12);
    settings(&mut sim).friendly_fire = 1;
    let a = join(&mut sim, JSF);
    let mate = join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    hit(&mut sim, a, Some(mate), 0.40, DamageKind::Bullet, Some(RIFLE));
    sim.ticks(1);
    assert!((health(&sim, a) - 0.6).abs() < 1e-5, "{}", health(&sim, a));
}

/// 60. JSF seizes at t = 0 and holds 30 s, Creed seizes at 30 s: JSF 30
/// points; JSF's flag gone; Creed's starts from the bottom.
#[test]
fn case_60_hold() {
    let (mut sim, a, b) = seize(8);
    seize_with(&mut sim, a, TRIGGER_A);
    assert_eq!(pole(&sim).team, Some(JSF));
    let t0 = pole(&sim).since;
    // Creed seizes, its 5 s ending at 30 s.
    let start = t0 + 25.0 - now(&sim);
    sim.seconds(start);
    seize_with(&mut sim, b, TRIGGER_B);
    let p = pole(&sim);
    assert_eq!(p.team, Some(CREED), "Creed's flag rises, JSF's is gone");
    assert!(((p.since - t0) - 30.0).abs() < 0.1, "at {:.2}", p.since - t0);
    assert_eq!(m(&sim).score[0], 30);
    assert_eq!(p.scored, 0, "from the bottom");
    assert_eq!(stats(&sim, b).mission_points, 1);
}

/// 61. A team holding 120 s uninterrupted wins at once.
#[test]
fn case_61_top() {
    let (mut sim, a, _b) = seize(8);
    seize_with(&mut sim, a, TRIGGER_A);
    let since = pole(&sim).since;
    sim.seconds(since + 119.8 - now(&sim));
    assert_eq!(m(&sim).winner, None);
    sim.seconds(0.3);
    assert_eq!(m(&sim).winner, Some(JSF));
    assert!(m(&sim).score[0] >= 120);
}

/// 62. G = 8: a round of 480 s; at its end JSF's flag rising with JSF 200,
/// Creed 220: JSF +50 = 250, JSF wins.
#[test]
fn case_62_time_out_bonus() {
    let (mut sim, a, _b) = seize(8);
    let left = m(&sim).round_ends.unwrap() - now(&sim);
    assert!((479.0..=480.0).contains(&left), "{left}");
    seize_with(&mut sim, a, TRIGGER_A);
    let n = now(&sim);
    {
        let w = sim.app.world_mut();
        let mut o = w.resource_mut::<CaObjectives>();
        o.pole = Some(Pole {
            team: Some(JSF),
            since: n,
            scored: 0,
        });
        let mut m = w.resource_mut::<CaMatch>();
        m.score = [200, 220];
        m.round_ends = Some(n);
    }
    until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).score[0], 250);
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
}

/// Seizing needs the player's own trigger, held for the use time.
#[test]
fn seizing_needs_your_own_trigger() {
    let (mut sim, a, _b) = seize(8);
    sim.intent(a).use_key = true;
    hold_at(&mut sim, a, TRIGGER_B, 1.0);
    assert!(action(&sim, a).is_none(), "the enemy's trigger");
    hold_at(&mut sim, a, TRIGGER_A, 1.0);
    assert_eq!(action(&sim, a).map(|g| g.kind), Some(ActionKind::Seize));
    assert_eq!(pole(&sim).team, None);
}

// --- M12 VIP Elimination

/// M12 (cases 71, 72; missions.md 23): G 140 and a team of 5 has 2
/// VIPs: its top player (here by name, no one has VIP points yet) silver
/// (5 points), the next bronze (3), the rest 1. Killing the silver VIP
/// adds 5 to the killer's team and the killer's VIP points; the ranks
/// follow the points (the killer's team re-ranked as its score rose).
#[test]
fn vip_elimination_weights_deaths() {
    let mut sim = game("VTDM", 12);
    settings(&mut sim).goal = 140;
    let a: Vec<Entity> = (0..5).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..5).map(|_| join(&mut sim, CREED)).collect();
    for (i, e) in a.iter().chain(b.iter()).enumerate() {
        sim.app.world_mut().entity_mut(*e).insert(Name::new(format!("p{i:02}")));
    }
    until_live(&mut sim);
    sim.ticks(2);
    unprotect_all(&mut sim);
    let rank = |sim: &Sim, e: Entity| sim.app.world().get::<CaVipRank>(e).map_or(0, |r| r.0);
    assert_eq!(
        b.iter().map(|e| rank(&sim, *e)).collect::<Vec<_>>(),
        vec![2, 1, 0, 0, 0]
    );
    kill_now(&mut sim, b[0], a[3]);
    assert_eq!(m(&sim).score, [5, 0]);
    assert_eq!(stats(&sim, a[3]).mission_points, 5);
    assert_eq!(rank(&sim, a[3]), 2, "the killer leads its team now");
    kill_now(&mut sim, b[2], a[3]);
    assert_eq!(m(&sim).score, [6, 0], "a rank-0 victim is worth 1");
    assert_eq!(m(&sim).round_ends, None, "no timer, as TDM");
}
