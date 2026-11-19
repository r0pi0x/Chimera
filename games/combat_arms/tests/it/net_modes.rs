//! Combat Arms' game modes over the network (specs/combat_arms/
//! game_modes.md; docs/plans/active/combat-arms.md, "Network part"): a
//! server and two clients on `NetSim`'s in-memory link playing Team Death
//! Match on the greybox with a team's row of start points each. The two
//! join on opposite teams, a kill scores on every machine (the team
//! counter and the killer's statistics), the dead player's client asks to
//! respawn after its 7 s and comes back at its team's point, and the
//! deciding kill ends the match on every machine. Then one test for each
//! other mode: its objectives (`NetCaObjectives`), what players carry
//! (`CaCarry`), the action gauges (`CaAction`), free-for-all teams and the
//! statistics as every client sees them.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Dead, Health, Hitgroup, SimClock, SpawnPoint, Team};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        inventory::UNIT,
        rules::{
            CaMatch, CaMatchView, CaRuleSettings, CaStats, Phase,
            objectives::{ActionKind, BOMB, CaAction, CaCarry, CaObjectives, FlagAt},
            objects::{BombZone, CaMapObjects, DropPoint, FlagObject, FlagZone, ObjBox, UseTrigger},
            spawns::{self, CaProtected},
        },
        weapons::RIFLE,
    },
    harness::CaSim,
};

pub(crate) fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin);
    let w = app.world_mut();
    {
        let mut s = w.resource_mut::<CaRuleSettings>();
        s.mode = "TDM".into();
        s.goal = 2;
    }
    for (k, team) in [1u8, 2].into_iter().enumerate() {
        for i in 0..4 {
            w.spawn((
                Name::new(format!("Spawn {}", k * 4 + i)),
                SpawnPoint { team: Some(Team(team)) },
                Transform::from_translation(Vec3::new(
                    -6.0 + 3.0 * i as f32,
                    1.0,
                    if team == 1 { 12.0 } else { -20.0 },
                )),
            ));
        }
    }
}

pub(crate) fn view(sim: &NetSim, i: usize) -> CaMatchView {
    sim.clients[i].app.world().resource::<CaMatchView>().clone()
}

pub(crate) fn server_match(sim: &NetSim) -> CaMatch {
    sim.server.app.world().resource::<CaMatch>().clone()
}

/// `killer` kills `victim` on the server.
pub(crate) fn kill(sim: &mut NetSim, victim: Entity, killer: Entity) {
    sim.server.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(killer),
        amount: 50.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: Some(RIFLE),
        force: Vec3::ZERO,
    });
}

#[test]
fn two_clients_play_team_death_match() {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 7, 2, setup);
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    let ok = sim.until(400, |s| {
        server_match(s).phase == Phase::Live && view(s, 0).phase == Phase::Live
    });
    assert!(ok, "live on the server and the client: {:?}", server_match(&sim).phase);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let team = |sim: &NetSim, e| sim.server.app.world().get::<Team>(e).unwrap().0;
    let (ta, tb) = (team(&sim, a), team(&sim, b));
    assert!(
        ta != tb && [ta, tb].iter().all(|t| *t == 1 || *t == 2),
        "opposite teams: {ta} {tb}"
    );
    for i in 0..2 {
        assert_eq!(view(&sim, i).team_counts, [1, 1], "client {i} sees one player a team");
        assert_eq!(view(&sim, i).mode, "TDM");
        assert_eq!(view(&sim, i).goal, 2);
    }
    // Past the spawn protection: a kills b.
    sim.ticks((4.2 / TICK_INTERVAL) as u64);
    kill(&mut sim, b, a);
    let side = |t: u8| t as usize - 1;
    let scored = sim.until(60, |s| (0..2).all(|i| view(s, i).score[side(ta)] == 1));
    assert!(scored, "the kill scores on every client: {:?}", view(&sim, 0).score);
    let me = sim.local_player(0).unwrap();
    assert_eq!(
        sim.clients[0].app.world().get::<CaStats>(me).map(|s| s.kills),
        Some(1),
        "the killer's stats"
    );
    let dead_b = sim.local_player(1).unwrap();
    assert!(
        sim.clients[1].app.world().get::<Dead>(dead_b).is_some(),
        "b's client sees itself dead"
    );
    // b's client asks after its 7 s; the server respawns it.
    let died = sim.server.app.world().resource::<SimClock>().now;
    let back = sim.until(700, |s| s.server.app.world().get::<Dead>(b).is_none());
    assert!(back, "respawned");
    let after = sim.server.app.world().resource::<SimClock>().now - died;
    assert!((7.0..8.0).contains(&after), "respawned {after:.2} s after the death");
    let health = sim.server.app.world().get::<Health>(b).unwrap();
    assert_eq!(health.current, health.max);
    let alive = sim.until(60, |s| {
        let me = s.local_player(1).unwrap();
        s.clients[1].app.world().get::<Dead>(me).is_none()
    });
    assert!(alive, "b's client sees itself alive");
    // The deciding kill: the match ends everywhere.
    sim.ticks((4.2 / TICK_INTERVAL) as u64);
    kill(&mut sim, b, a);
    let over = sim.until(800, |s| (0..2).all(|i| view(s, i).phase == Phase::MatchOver));
    assert!(over, "match over on every client: {:?}", view(&sim, 0).phase);
    for i in 0..2 {
        let v = view(&sim, i);
        assert_eq!(v.result, Some(Some(ta)), "client {i}: {v:?}");
        assert_eq!(v.score[side(ta)], 2);
    }
}

// --- The other modes: one test each (a server and two clients on
// opposite teams, or two free-for-all players), the mode's state seen on
// every client.

/// A server and two clients playing `mode` (G `goal`) on the greybox,
/// with short warm-ups and phases (1 s warm-up, 0.5 s round-over and
/// intermission) so the tests reach the first real round soon.
pub(crate) fn mode_sim(mode: &'static str, goal: u32) -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 11, 2, move |app| {
        setup(app);
        let mut s = app.world_mut().resource_mut::<CaRuleSettings>();
        s.mode = mode.into();
        s.goal = goal;
        s.object_end = 0.5;
        s.mission_end = 0.5;
        for d in s.modes.values_mut() {
            if d.training_time_limit.is_some() {
                d.training_time_limit = Some(1.0);
            }
        }
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim
}

/// The match from its start on the server (after its objects were put
/// in), until its first real round is live everywhere.
pub(crate) fn restart_and_play(sim: &mut NetSim) {
    combat_arms::game::rules::flow::restart(sim.server.app.world_mut());
    let ok = sim.until(1200, |s| {
        let m = server_match(s);
        m.phase == Phase::Live && !m.warmup && (0..2).all(|i| view(s, i).phase == Phase::Live && !view(s, i).warmup)
    });
    assert!(ok, "the first round live everywhere: {:?}", server_match(sim).phase);
    let w = sim.server.app.world_mut();
    let all: Vec<Entity> = w.query_filtered::<Entity, With<CaProtected>>().iter(w).collect();
    for e in all {
        spawns::unprotect(w, e);
    }
}

pub(crate) fn client_objectives(sim: &NetSim, i: usize) -> CaObjectives {
    sim.clients[i].app.world().resource::<CaObjectives>().clone()
}

/// Keep the server's `e` at `at` for `n` ticks.
pub(crate) fn hold_server(sim: &mut NetSim, e: Entity, at: Vec3, n: u64) {
    for _ in 0..n {
        let w = sim.server.app.world_mut();
        w.get_mut::<Transform>(e).unwrap().translation = at;
        if let Some(mut p) = w.get_mut::<avian3d::prelude::Position>(e) {
            p.0 = at;
        }
        sim.step();
    }
}

/// The server's character of client `i`, and its team.
pub(crate) fn player(sim: &mut NetSim, i: usize) -> (Entity, u8) {
    let e = sim.character_of(i).unwrap();
    (e, sim.server.app.world().get::<Team>(e).unwrap().0)
}

/// Client `i`'s client and server entities by team: (alpha's, bravo's).
pub(crate) fn by_team(sim: &mut NetSim) -> [(usize, Entity); 2] {
    let (a, ta) = player(sim, 0);
    let (b, _) = player(sim, 1);
    if ta == 1 { [(0, a), (1, b)] } else { [(1, b), (0, a)] }
}

/// Search & Destroy: the JSF client selects the bomb and plants it (its
/// client sees the plant gauge); every client sees the planted bomb and
/// its fuse; the Creed client defuses it and every client sees Creed's
/// round.
#[test]
fn search_and_destroy_over_the_network() {
    let mut sim = mode_sim("TMM", 5);
    let site = Vec3::new(0.0, 1.0, -4.0);
    sim.server.app.world_mut().resource_mut::<CaMapObjects>().bomb_zones = vec![BombZone {
        family: "Doobic".into(),
        number: 0,
        defusers: 2,
        can_defuse: true,
        defuse_time: 7.0,
        detonate_time: 40.0,
        area: ObjBox {
            centre: site,
            half: Vec3::new(130.0, 64.0, 125.0) * UNIT,
        },
    }];
    restart_and_play(&mut sim);
    let [(ia, a), (ib, b)] = by_team(&mut sim);
    let got = sim.until(120, |s| weapons_of(s, a).contains(&BOMB));
    assert!(got, "JSF's bomb");
    client_intent(&mut sim, ia).select = Some(5);
    sim.ticks(2);
    client_intent(&mut sim, ia).select = None;
    let drawn = sim.until(120, |s| held(s, a) == Some(BOMB));
    assert!(drawn, "the bomb in hand on the server");
    client_intent(&mut sim, ia).fire = true;
    hold_server(&mut sim, a, site, 120);
    let me = sim.local_player(ia).unwrap();
    let gauge = sim.clients[ia].app.world().get::<CaAction>(me).copied();
    assert_eq!(
        gauge.map(|g| g.kind),
        Some(ActionKind::Plant),
        "the planter's gauge on its client"
    );
    hold_server(&mut sim, a, site, 160);
    client_intent(&mut sim, ia).fire = false;
    assert!(
        sim.server.app.world().resource::<CaObjectives>().planted.is_some(),
        "planted"
    );
    let seen = sim.until(60, |s| (0..2).all(|i| client_objectives(s, i).planted.is_some()));
    assert!(seen, "every client sees the bomb");
    let now = sim.clients[0].app.world().resource::<SimClock>().now;
    let fuse = client_objectives(&sim, 0).planted.unwrap().detonates - now;
    assert!(
        (35.0..41.0).contains(&fuse),
        "the fuse on the client's clock: {fuse:.2}"
    );
    client_intent(&mut sim, ib).use_key = true;
    hold_server(&mut sim, b, site, (7.3 / TICK_INTERVAL) as u64);
    let won = sim.until(120, |s| (0..2).all(|i| view(s, i).round_wins == [0, 1]));
    assert!(won, "Creed's round everywhere: {:?}", view(&sim, 0).round_wins);
}

/// Capture the Flag: the Alpha player takes Bravo's flag (every client
/// sees it taken and who carries it) and captures it at its zone (every
/// client sees Alpha's capture and the flag home).
#[test]
fn capture_the_flag_over_the_network() {
    let mut sim = mode_sim("CTF", 3);
    let (alpha_home, bravo_home) = (Vec3::new(-4.0, 1.0, 6.0), Vec3::new(4.0, 1.0, -14.0));
    {
        let mut o = sim.server.app.world_mut().resource_mut::<CaMapObjects>();
        for (team, home) in [(1u8, alpha_home), (2, bravo_home)] {
            o.flags.push(FlagObject {
                name: format!("Flag{team}"),
                family: "Capturetheflag".into(),
                team,
                area: ObjBox {
                    centre: home,
                    half: Vec3::new(10.0, 25.0, 10.0) * UNIT,
                },
            });
            o.flag_zones.push(FlagZone {
                team,
                linked_flag: format!("Flag{team}"),
                area: ObjBox {
                    centre: home,
                    half: Vec3::new(64.0, 50.0, 64.0) * UNIT,
                },
            });
        }
    }
    restart_and_play(&mut sim);
    let [(ia, a), _] = by_team(&mut sim);
    hold_server(&mut sim, a, bravo_home, 2);
    let taken = sim.until(60, |s| {
        (0..2).all(|i| client_objectives(s, i).flag(2).is_some_and(|f| f.at == FlagAt::Carried))
    });
    assert!(taken, "every client sees Bravo's flag taken");
    let me = sim.local_player(ia).unwrap();
    let carried = sim.until(60, |s| {
        s.clients[ia].app.world().get::<CaCarry>(me).and_then(|c| c.flag) == Some(2)
    });
    assert!(carried, "the carrier's client sees it carries the flag");
    hold_server(&mut sim, a, alpha_home, 2);
    let scored = sim.until(60, |s| {
        (0..2).all(|i| {
            view(s, i).score == [1, 0] && client_objectives(s, i).flag(2).is_some_and(|f| f.at == FlagAt::Home)
        })
    });
    assert!(scored, "Alpha's capture everywhere: {:?}", view(&sim, 0).score);
}

/// One Man Army: the two players get teams of their own on every
/// machine; the deciding kill ends the match with the killer's own team
/// id the winner.
#[test]
fn one_man_army_over_the_network() {
    let mut sim = mode_sim("FFA", 1);
    let ok = sim.until(400, |s| server_match(s).phase == Phase::Live);
    assert!(ok);
    let (a, ta) = player(&mut sim, 0);
    let (b, tb) = player(&mut sim, 1);
    assert!(ta >= 3 && tb >= 3 && ta != tb, "teams of their own: {ta} {tb}");
    let me = sim.local_player(1).unwrap();
    let seen = sim.until(60, |s| {
        s.clients[1].app.world().get::<Team>(me).map(|t| t.0) == Some(tb)
    });
    assert!(seen, "the client sees its own team");
    sim.ticks((4.2 / TICK_INTERVAL) as u64);
    kill(&mut sim, b, a);
    let over = sim.until(800, |s| (0..2).all(|i| view(s, i).phase == Phase::MatchOver));
    assert!(over);
    assert_eq!(view(&sim, 1).result, Some(Some(ta)));
}

/// Last Man Standing: the last player alive's round win reaches every
/// client (the result and its statistics).
#[test]
fn last_man_standing_over_the_network() {
    let mut sim = mode_sim("LMS", 3);
    restart_and_play(&mut sim);
    let (a, ta) = player(&mut sim, 0);
    let (b, _) = player(&mut sim, 1);
    kill(&mut sim, b, a);
    let won = sim.until(120, |s| (0..2).all(|i| view(s, i).winner == Some(ta)));
    assert!(won, "the round's winner everywhere");
    let me = sim.local_player(0).unwrap();
    let stat = sim.until(120, |s| {
        s.clients[0].app.world().get::<CaStats>(me).map(|s| s.round_wins) == Some(1)
    });
    assert!(stat, "the round win in the client's statistics");
}

/// Quarantine Regen: 20 s in, one of the two humans becomes the infected
/// host (Bravo) on every client.
#[test]
fn quarantine_over_the_network() {
    let mut sim = mode_sim("QRT", 5);
    restart_and_play(&mut sim);
    let (a, _) = player(&mut sim, 0);
    let (b, _) = player(&mut sim, 1);
    let team_of = |sim: &NetSim, e: Entity| sim.server.app.world().get::<Team>(e).unwrap().0;
    assert_eq!((team_of(&sim, a), team_of(&sim, b)), (1, 1), "all human");
    sim.ticks((20.3 / TICK_INTERVAL) as u64);
    let infected = [a, b].into_iter().filter(|e| team_of(&sim, *e) == 2).count();
    assert_eq!(infected, 1, "one host for 2 players");
    let seen = sim.until(60, |s| {
        (0..2).all(|i| {
            let w = s.clients[i].app.world_mut();
            w.query::<&Team>().iter(w).filter(|t| t.0 == 2).count() == 1
        })
    });
    assert!(seen, "every client sees the host");
}

/// Bombing Run: the bomb on its drop point is seen on every client; its
/// pick-up shows on the carrier's client.
#[test]
fn bombing_run_over_the_network() {
    let mut sim = mode_sim("SBT", 5);
    let spot = Vec3::new(0.0, 1.0, -4.0);
    sim.server.app.world_mut().resource_mut::<CaMapObjects>().drops = vec![DropPoint {
        family: "Sabotage".into(),
        at: spot,
    }];
    restart_and_play(&mut sim);
    let seen = sim.until(60, |s| (0..2).all(|i| client_objectives(s, i).loose_bomb.is_some()));
    assert!(seen, "the bomb on the ground everywhere");
    let (a, _) = player(&mut sim, 0);
    hold_server(&mut sim, a, spot, 3);
    let me = sim.local_player(0).unwrap();
    let carried = sim.until(60, |s| {
        s.clients[0].app.world().get::<CaCarry>(me).is_some_and(|c| c.bomb)
            && (0..2).all(|i| client_objectives(s, i).loose_bomb.is_none())
    });
    assert!(carried, "taken");
}

/// Seize and Secure: the Alpha client holds use at its trigger; every
/// client sees Alpha's flag rising and its points.
#[test]
fn seize_and_secure_over_the_network() {
    let mut sim = mode_sim("HQM", 8);
    let trigger = Vec3::new(-6.0, 1.0, 4.0);
    sim.server.app.world_mut().resource_mut::<CaMapObjects>().triggers = vec![UseTrigger {
        name: "ActivateTrigger_A".into(),
        area: ObjBox {
            centre: trigger,
            half: Vec3::splat(100.0) * UNIT,
        },
    }];
    restart_and_play(&mut sim);
    let [(ia, a), _] = by_team(&mut sim);
    client_intent(&mut sim, ia).use_key = true;
    hold_server(&mut sim, a, trigger, (5.3 / TICK_INTERVAL) as u64);
    client_intent(&mut sim, ia).use_key = false;
    sim.ticks((2.2 / TICK_INTERVAL) as u64);
    let seen = sim.until(60, |s| {
        (0..2).all(|i| client_objectives(s, i).pole.is_some_and(|p| p.team == Some(1)) && view(s, i).score[0] >= 2)
    });
    assert!(seen, "Alpha's flag rising everywhere: {:?}", view(&sim, 0).score);
}

/// Spy Hunt: the intel on the ground reaches every client; a pick-up makes
/// the player a spy (a team of its own) on every client.
#[test]
fn spy_hunt_over_the_network() {
    let mut sim = mode_sim("BNT", 8);
    let spots = [Vec3::new(-3.0, 1.0, -4.0), Vec3::new(3.0, 1.0, -4.0)];
    sim.server.app.world_mut().resource_mut::<CaMapObjects>().drops = spots
        .iter()
        .map(|p| DropPoint {
            family: "BountyHunter".into(),
            at: *p,
        })
        .collect();
    sim.server
        .app
        .world_mut()
        .resource_mut::<CaRuleSettings>()
        .objectives
        .intel_count = 2;
    restart_and_play(&mut sim);
    let seen = sim.until(60, |s| {
        (0..2).all(|i| client_objectives(s, i).intel.len() == 2 && client_objectives(s, i).stage == 1)
    });
    assert!(seen, "the intel everywhere");
    let (a, _) = player(&mut sim, 0);
    hold_server(&mut sim, a, spots[0], 3);
    let spy = sim.server.app.world().get::<Team>(a).unwrap().0;
    assert!(spy >= 3, "a spy: {spy}");
    let me = sim.local_player(0).unwrap();
    let mine = sim.until(60, |s| {
        let w = s.clients[0].app.world();
        w.get::<Team>(me).map(|t| t.0) == Some(spy) && w.get::<CaCarry>(me).is_some_and(|c| c.intel == 1)
    });
    assert!(mine, "the spy's client sees its team and intel");
}

/// The modes that are another's with a twist (VIP Elimination, Arms Race,
/// Hunted, the Go4CA Cup, Squad Match 2): each plays its first round over
/// the network with its room key and rule module on every client.
#[test]
fn the_variants_over_the_network() {
    for key in ["VTDM", "VWRM", "HG", "ESL", "SQM2"] {
        let mut sim = mode_sim(key, 5);
        restart_and_play(&mut sim);
        for i in 0..2 {
            assert_eq!(view(&sim, i).mode, key);
        }
        let (a, _) = player(&mut sim, 0);
        let (b, _) = player(&mut sim, 1);
        kill(&mut sim, b, a);
        let counted = sim.until(120, |s| {
            let me = s.local_player(0).unwrap();
            s.clients[0].app.world().get::<CaStats>(me).map(|s| s.kills) == Some(1)
        });
        assert!(counted, "{key}: the kill on the killer's client");
    }
}

/// The server's weapons of `e`.
pub(crate) fn weapons_of(sim: &NetSim, e: Entity) -> Vec<&'static str> {
    let w = sim.server.app.world();
    w.get::<chimera_sim::weapon::Inventory>(e)
        .map(|i| {
            i.weapons
                .iter()
                .filter_map(|x| w.get::<chimera_sim::weapon::Weapon>(*x))
                .map(|x| x.id)
                .collect()
        })
        .unwrap_or_default()
}

/// The server's weapon in `e`'s hand.
fn held(sim: &NetSim, e: Entity) -> Option<&'static str> {
    let w = sim.server.app.world();
    w.get::<chimera_sim::weapon::Inventory>(e)
        .and_then(|i| i.active)
        .and_then(|x| w.get::<chimera_sim::weapon::Weapon>(x))
        .map(|x| x.id)
}

pub(crate) fn client_intent(sim: &mut NetSim, i: usize) -> Mut<'_, chimera_core::core::Intent> {
    let me = sim.local_player(i).unwrap();
    sim.clients[i]
        .app
        .world_mut()
        .get_mut::<chimera_core::core::Intent>(me)
        .unwrap()
}
