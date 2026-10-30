//! The free-for-all modes (specs/combat_arms/game_modes.md M5, M6, M7,
//! M14, M15): One Man Army's test cases 50-52 and case 10's One Man Army
//! half, Last Man Standing's 53-54, and Spy Hunt, Arms Race and Hunted
//! (no numbered cases) on `CaSim` and the greybox (`mode_kit`).

use bevy::prelude::*;
use chimera_core::core::DamageKind;
use chimera_world::harness::Sim;
use combat_arms::game::{
    grenades::M67,
    rules::{
        self, CaMatch, CaStats, Phase,
        free_for_all::CaKillsMade,
        last_man::winner_of,
        objects::StartPoint,
        spawns::{self, CaProtected, CaSpawnCursors},
        state::FFA_FIRST,
    },
    weapons::RIFLE,
};

use crate::mode_kit::*;

/// `n` players in a free-for-all `mode` (they join as Alpha; the mode
/// gives each a team of its own), live, unprotected.
fn ffa(mode: &str, n: usize) -> (Sim, Vec<Entity>) {
    let mut sim = game(mode, 12);
    let players: Vec<Entity> = (0..n).map(|_| join(&mut sim, JSF)).collect();
    until_live(&mut sim);
    unprotect_all(&mut sim);
    (sim, players)
}

/// Each player a team of their own (S1).
#[test]
fn every_player_has_a_team_of_their_own() {
    let (sim, p) = ffa("FFA", 4);
    let mut teams: Vec<u8> = p.iter().map(|e| team(&sim, *e)).collect();
    assert!(teams.iter().all(|t| *t >= FFA_FIRST), "{teams:?}");
    teams.sort();
    teams.dedup();
    assert_eq!(teams.len(), 4);
    assert_eq!(chimera_sim::rule_module::pick(sim.app.world()), Some(rules::ONE_MAN_ARMY));
}

/// 10 (One Man Army half). A protected player who fires at 1.0 s and is
/// hit at 1.1 s takes damage: firing ends protection in the free-for-all
/// modes.
#[test]
fn case_10_firing_ends_protection_in_one_man_army() {
    let mut sim = game("FFA", 12);
    let p = join(&mut sim, JSF);
    let enemy = join(&mut sim, JSF);
    until_live(&mut sim);
    assert!(sim.app.world().get::<CaProtected>(p).is_some());
    // The trigger pulled at 1.0 s: the M4A1's first round leaves once its
    // draw at the respawn is over (about 1.15 s in), well inside the 3 s.
    sim.seconds(1.0);
    sim.intent(p).fire = true;
    let ended = wait_for(&mut sim, 0.5, |s| s.app.world().get::<CaProtected>(p).is_none());
    sim.intent(p).fire = false;
    assert!(ended, "firing ended it");
    sim.seconds(0.1);
    assert!(sim.app.world().get::<CaProtected>(enemy).is_some(), "not the other's");
    hit(&mut sim, p, Some(enemy), 0.40, DamageKind::Bullet, Some(RIFLE));
    sim.ticks(1);
    assert!((health(&sim, p) - 0.6).abs() < 1e-5, "{}", health(&sim, p));
}

/// 50. G = 30: a player reaching 30 kills wins at once; one at 29 kills
/// who kills itself stays at 29, deaths +1.
#[test]
fn case_50_win_and_suicide() {
    let (mut sim, p) = ffa("FFA", 3);
    sim.app.world_mut().entity_mut(p[0]).insert(CaStats {
        kills: 29,
        ..default()
    });
    sim.app.world_mut().entity_mut(p[1]).insert(CaStats {
        kills: 29,
        ..default()
    });
    hit(&mut sim, p[1], Some(p[1]), 5.0, DamageKind::Blast, Some(M67));
    sim.ticks(1);
    let s = stats(&sim, p[1]);
    assert_eq!((s.kills, s.deaths, s.suicides), (29, 1, 1));
    assert_eq!(m(&sim).winner, None);
    kill_now(&mut sim, p[2], p[0]);
    assert_eq!(stats(&sim, p[0]).kills, 30);
    assert_eq!(m(&sim).winner, Some(team(&sim, p[0])), "wins at once");
    until(&mut sim, 7.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(team(&sim, p[0]))));
}

/// The mode's flow steps reach observers (`mode::ModeStep`), each once and
/// in order, inside the step: the win's round over, its end and the
/// match's end (`MatchEnd`, the result set and the intermission on).
#[test]
fn a_matchs_end_is_triggered_as_its_flows_steps() {
    use chimera_sim::mode::{FlowEvent, ModeStep};
    #[derive(Resource, Default)]
    struct Steps(Vec<(FlowEvent, Phase, Option<Option<u8>>)>);
    let (mut sim, p) = ffa("FFA", 2);
    sim.app.init_resource::<Steps>().add_observer(|s: On<ModeStep>, m: Res<CaMatch>, mut steps: ResMut<Steps>| {
        steps.0.push((s.0, m.phase, m.result));
    });
    sim.app.world_mut().entity_mut(p[0]).insert(CaStats {
        kills: 29,
        ..default()
    });
    kill_now(&mut sim, p[1], p[0]);
    until(&mut sim, 7.0, |p| p == Phase::MatchOver);
    let won = team(&sim, p[0]);
    let steps = std::mem::take(&mut sim.app.world_mut().resource_mut::<Steps>().0);
    let events: Vec<FlowEvent> = steps.iter().map(|s| s.0).collect();
    assert_eq!(
        events,
        [
            FlowEvent::RoundWonBy(p[0]),
            FlowEvent::RoundOver,
            FlowEvent::RoundEnd,
            FlowEvent::RoundEnded,
            FlowEvent::MatchEnd
        ]
    );
    let (_, phase, result) = steps[4];
    assert!(matches!(phase, Phase::Intermission { .. }), "{phase:?}");
    assert_eq!(result, Some(Some(won)));
}

/// 51. 22 `FreeForAll` points, shuffled once at the map's load: one list,
/// whatever their team, consecutive spawns take consecutive shuffled
/// points with one shared cursor.
#[test]
fn case_51_spawns() {
    let mut sim = game("FFA", 12);
    let pts: Vec<Vec3> = (0..22).map(|i| Vec3::new(-15.0 + 1.4 * i as f32, 1.0, -2.0)).collect();
    {
        let mut o = objects(&mut sim);
        o.starts = pts.iter().map(|p| start("FreeForAll", None, *p)).collect();
        // Other families' points aren't used.
        o.starts.push(start("Team Death Match", Some(JSF), Vec3::new(0.0, 1.0, 12.0)));
        o.starts.push(StartPoint {
            spawn: 2,
            ..start("FreeForAll", None, Vec3::new(0.0, 1.0, 14.0))
        });
    }
    let players: Vec<Entity> = (0..4).map(|_| join(&mut sim, JSF)).collect();
    let (list, cursor) = spawns::spawn_list(sim.app.world_mut(), FFA_FIRST);
    assert_eq!((list.len(), cursor), (22, 0));
    let order: Vec<usize> = list
        .iter()
        .map(|t| pts.iter().position(|p| (*p - t.translation).length() < 1e-4).unwrap())
        .collect();
    assert_ne!(order, (0..22).collect::<Vec<_>>(), "shuffled");
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(sorted, (0..22).collect::<Vec<_>>(), "every point once");
    until_live(&mut sim);
    for (k, p) in players.iter().enumerate() {
        let want = list[k + 1].translation;
        assert!(at(&sim, *p).xz().distance(want.xz()) < 0.05, "player {k}: {} want {want}", at(&sim, *p));
    }
    assert_eq!(sim.app.world().resource::<CaSpawnCursors>().0[0], 4, "one shared cursor");
    // The same order for the whole map load.
    let (again, _) = spawns::spawn_list(sim.app.world_mut(), FFA_FIRST + 2);
    assert_eq!(again, list);
}

/// 52. Leaving, G = 30: 3 players, one leaves: nothing (3 counted, the
/// leaver too). 2 players with 10 kills made in total, one leaves: the
/// round is over, the remaining player the winner (10 > round(0.1 x 30)
/// = 3); with 3 kills in total: no winner.
#[test]
fn case_52_leaving() {
    let (mut sim, p) = ffa("FFA", 3);
    sim.app.world_mut().despawn(p[2]);
    sim.ticks(2);
    assert_eq!((phase(&sim), m(&sim).winner), (Phase::Live, None));
    for (kills, winner) in [(10, true), (3, false)] {
        let (mut sim, p) = ffa("FFA", 2);
        // The kills made in the round, the leaver's too (Corrections
        // 2026-10-22, 14).
        sim.app.world_mut().resource_mut::<CaKillsMade>().0 = kills;
        let stayer = team(&sim, p[0]);
        sim.app.world_mut().despawn(p[1]);
        until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
        let want = winner.then_some(stayer);
        assert_eq!(m(&sim).results.last().unwrap().winner, want, "{kills} kills");
    }
}

/// 53. Last Man Standing: 5 players, deaths leave one alive: that player's
/// round wins +1, the round over; G = 5: the match is over after round 5
/// (rounds played), its winner by M6's comparison.
#[test]
fn case_53_last_man_standing_round() {
    let mut sim = game("LMS", 12);
    let p: Vec<Entity> = (0..5).map(|_| join(&mut sim, JSF)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    for v in &p[1..] {
        kill_now(&mut sim, *v, p[0]);
    }
    assert_eq!(m(&sim).winner, Some(team(&sim, p[0])));
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(stats(&sim, p[0]).round_wins, 1);
    assert_eq!(stats(&sim, p[0]).mission_points, 1);
    // Round 5 of 5.
    sim.app.world_mut().resource_mut::<CaMatch>().round = 4;
    until(&mut sim, 8.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).round, 5);
    // A (the first in the list) has the only round win: nobody beats or
    // ties it, so the round-win test picks A.
    assert_eq!(m(&sim).result, Some(Some(team(&sim, p[0]))));
}

fn st(round_wins: u32, kills: u32, deaths: u32) -> CaStats {
    CaStats {
        round_wins,
        kills,
        deaths,
        ..default()
    }
}

/// 54. M6's winner quirk: A (2 round wins), B (3), C (3): each compared
/// with A only, the pick the last with more round wins than A: C. A (2),
/// B (3), C (2): C ties A, which blocks the round-win test; kills decide.
/// A (0), B (4): A has none, so the round-win test is skipped; kills
/// decide.
#[test]
fn case_54_winner_quirk() {
    assert_eq!(winner_of(&[(3, st(2, 0, 0)), (4, st(3, 0, 0)), (5, st(3, 0, 0))]), Some(5));
    assert_eq!(winner_of(&[(3, st(2, 5, 0)), (4, st(3, 9, 0)), (5, st(2, 1, 0))]), Some(4), "kills decide");
    assert_eq!(winner_of(&[(3, st(0, 5, 0)), (4, st(4, 2, 0))]), Some(3), "kills decide: A's 5");
    // Kills tied too: more deaths decide; tied again: nobody.
    assert_eq!(winner_of(&[(3, st(1, 1, 1)), (4, st(1, 1, 4))]), Some(4));
    assert_eq!(winner_of(&[(3, st(1, 1, 1)), (4, st(1, 1, 1))]), None);
    assert_eq!(winner_of(&[(7, st(0, 0, 0))]), Some(7), "one player");
}

/// M7: Spy Hunt's intel makes a spy (a team of its own); all of it makes
/// the final spy (Bravo), everyone else a hunter, the dead revived and
/// respawning stopped; killing the final spy wins for the hunters.
#[test]
fn spy_hunt_stages() {
    let mut sim = game("BNT", 12);
    settings(&mut sim).objectives.intel_count = 2;
    let spots = [Vec3::new(-3.0, 1.0, 0.0), Vec3::new(3.0, 1.0, 0.0)];
    objects(&mut sim).drops = spots.iter().map(|p| drop_point("BountyHunter", *p)).collect();
    let p: Vec<Entity> = (0..3).map(|_| join(&mut sim, JSF)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    assert!(p.iter().all(|e| team(&sim, *e) == JSF), "everyone a hunter");
    assert_eq!(objectives(&sim).intel.len(), 2);
    hold_at(&mut sim, p[0], spots[0], 0.05);
    assert_eq!(carry(&sim, p[0]).intel, 1);
    assert!(team(&sim, p[0]) >= FFA_FIRST, "a spy: {}", team(&sim, p[0]));
    // A hunter kills the spy: the intel drops, the spy is a hunter again.
    kill_now(&mut sim, p[0], p[1]);
    assert_eq!(stats(&sim, p[1]).kills, 1);
    assert_eq!(objectives(&sim).intel.len(), 2);
    assert_eq!(team(&sim, p[0]), JSF);
    let items = objectives(&sim).intel;
    for i in items {
        hold_at(&mut sim, p[1], i, 0.05);
    }
    assert_eq!(objectives(&sim).stage, 2);
    assert_eq!(team(&sim, p[1]), 2, "the final spy");
    assert!(!dead(&sim, p[0]), "the dead revived");
    unprotect_all(&mut sim);
    // Respawning stops: a hunter the spy kills stays dead.
    kill_now(&mut sim, p[2], p[1]);
    sim.seconds(8.0);
    assert!(dead(&sim, p[2]), "no respawn in stage 2");
    assert_eq!(m(&sim).winner, None);
    kill_now(&mut sim, p[1], p[0]);
    assert_eq!(m(&sim).winner, Some(JSF), "the hunters win");
}

/// Arms Race with `n` players, the default ladder's `Percent` at
/// `percent` from the start.
fn arms_race(n: usize, percent: i32) -> (Sim, Vec<Entity>) {
    let mut sim = game("VWRM", 12);
    settings(&mut sim).objectives.ladder_percent = percent;
    let players: Vec<Entity> = (0..n).map(|_| join(&mut sim, JSF)).collect();
    until_live(&mut sim);
    sim.ticks(3);
    unprotect_all(&mut sim);
    (sim, players)
}

/// M14 (missions.md F4.8): Arms Race's ladder at G 15 (the stock
/// default ladder: 0 kills ACR, 4 the Kriss; our M4A1 and MP5A4 stand in
/// for them without the install): the step's weapon at spawn and at the
/// kill that reaches a step, drawn; no death drops. With the roll always
/// below `Percent` the table's weapon, never its own.
#[test]
fn arms_race_ladder() {
    let (mut sim, p) = arms_race(5, 101);
    assert!(!sim.app.world().resource::<combat_arms::game::inventory::CaDeathDrops>().0);
    assert!(wait_for(&mut sim, 1.0, |s| weapons(s, p[0]).contains(&"combat_arms:m4a1")), "{:?}", weapons(&sim, p[0]));
    for (i, v) in [1, 2, 3].into_iter().enumerate() {
        kill_now(&mut sim, p[v], p[0]);
        sim.ticks(1);
        assert!(!weapons(&sim, p[0]).contains(&"combat_arms:mp5a4"), "not after {} kills", i + 1);
    }
    kill_now(&mut sim, p[4], p[0]);
    sim.ticks(1);
    assert!(weapons(&sim, p[0]).contains(&"combat_arms:mp5a4"), "{:?}", weapons(&sim, p[0]));
    assert!(wait_for(&mut sim, 2.0, |s| in_hand(s, p[0]) == Some("combat_arms:mp5a4")), "drawn");
    let l = sim.app.world().get::<combat_arms::game::rules::arms_race::CaLadder>(p[0]).cloned().unwrap();
    assert_eq!((l.next, l.last_stage), (Some(7), false));
}

/// M14: a roll at or above `Percent` keeps the player's own weapon.
#[test]
fn arms_race_roll_keeps_your_own() {
    let (mut sim, p) = arms_race(5, 0);
    for v in [1, 2, 3, 4] {
        kill_now(&mut sim, p[v], p[0]);
    }
    sim.ticks(2);
    assert!(!weapons(&sim, p[0]).contains(&"combat_arms:mp5a4"), "{:?}", weapons(&sim, p[0]));
    assert!(weapons(&sim, p[0]).contains(&RIFLE), "own primary kept: {:?}", weapons(&sim, p[0]));
}

/// M15: Hunted plays on the unlocked `HungerGame` points in the warm-up
/// and the locked ones in the rounds.
#[test]
fn hunted_spawns() {
    let mut sim = game("HG", 12);
    let warm = Vec3::new(-6.0, 1.0, 0.0);
    let round = Vec3::new(6.0, 1.0, 0.0);
    {
        let mut o = objects(&mut sim);
        o.starts = vec![
            start("HungerGame", None, warm),
            StartPoint {
                locked: true,
                ..start("HungerGame", None, round)
            },
        ];
    }
    let p = join(&mut sim, JSF);
    join(&mut sim, JSF);
    until_live(&mut sim);
    assert!(at(&sim, p).xz().distance(warm.xz()) < 0.05, "warm-up: {}", at(&sim, p));
    until(&mut sim, 30.0, |p| matches!(p, Phase::Setup { .. }));
    sim.ticks(1);
    assert!(at(&sim, p).xz().distance(round.xz()) < 0.05, "round: {}", at(&sim, p));
}


/// A mode's per-tick system runs only in its mode (`mode::mode_is`):
/// Hunted's turns a fist kit's loadout into the fist within a tick; in
/// Team Death Match the same mark stays and the loadout with it.
#[test]
fn a_modes_tick_runs_only_in_its_mode() {
    use combat_arms::game::rules::hunted::CaFistKit;
    for (mode, runs) in [("HG", true), ("TDM", false)] {
        let mut sim = game(mode, 12);
        let p = join(&mut sim, JSF);
        join(&mut sim, CREED);
        until_live(&mut sim);
        sim.ticks(2);
        let before = weapons(&sim, p);
        assert!(!before.is_empty(), "{mode}: a loadout");
        sim.app.world_mut().entity_mut(p).insert(CaFistKit);
        sim.ticks(2);
        let kept = sim.app.world().get::<CaFistKit>(p).is_some();
        assert_eq!(!kept, runs, "{mode}: the kit mark {}", if kept { "kept" } else { "taken" });
        if !runs {
            assert_eq!(weapons(&sim, p), before, "{mode}: the loadout left alone");
        }
    }
}
