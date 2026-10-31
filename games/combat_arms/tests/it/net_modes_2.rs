//! The modes of specs/combat_arms/game_modes_2.md over the network: a
//! server and two clients on `NetSim`'s in-memory link (`net_modes`'
//! helpers), one test a mode: Hired Guns (the bullion, the pocket, the
//! bank and the banked bars, a hire and an order sent by a client, the
//! mercenary replicated as a character), VIP Escort (the VIP's mark, the
//! roles, the exit and the escape's round), the clan modes (the half-time
//! swap of teams and counters on every client).

use bevy::prelude::*;
use combat_arms::game::{
    TICK_INTERVAL,
    inventory::UNIT,
    net::{CaHireRequest, CaOrderRequest},
    rules::{
        CaStats,
        escort::CaVip,
        hired_guns::CaBank,
        mercs::{CaMerc, CaSquadOrder, MercOrder},
        objectives::{CaCarry, Exit},
        objects::{BankTrigger, CaMapObjects, DropPoint, EscapeZone, NamedPoint, ObjBox},
    },
};

use crate::net_modes::*;

fn named(class: &str, name: &str, at: Vec3) -> NamedPoint {
    NamedPoint {
        class: class.into(),
        name: name.into(),
        at: Transform::from_translation(at),
    }
}

/// Hired Guns: every client sees the bars placed; a client hires a unit
/// (its bank 1000 → 800 on its machine), the unit comes to every client
/// as a character with its hirer; the client's order reaches the server;
/// a pocket and its banking show on every client.
#[test]
fn hired_guns_over_the_network() {
    let mut sim = mode_sim("SQM", 3);
    let bank_a = Vec3::new(8.0, 1.0, 20.0);
    {
        let mut o = sim.server.app.world_mut().resource_mut::<CaMapObjects>();
        o.banks = vec![BankTrigger {
            name: "SaveZone_A".into(),
            team: Some(1),
            area: ObjBox {
                centre: bank_a,
                half: Vec3::new(128.0, 160.0, 128.0) * UNIT,
            },
        }];
        o.points = vec![
            named("BaseClass", "UNITSPAWN_ALPHA", Vec3::new(8.0, 1.0, 15.0)),
            named("BaseClass", "UNITSPAWN_BRAVO", Vec3::new(-8.0, 1.0, 15.0)),
        ];
        o.drops = (0..6)
            .map(|i| DropPoint {
                family: "SquadMatch".into(),
                at: Vec3::new(-4.0 + i as f32, 0.2, 25.0),
            })
            .collect();
    }
    restart_and_play(&mut sim);
    let seen = sim.until(60, |s| (0..2).all(|i| client_objectives(s, i).bullion.len() == 6));
    assert!(seen, "every client sees the 6 bars");
    let [(ia, a), _] = by_team(&mut sim);
    let me = sim.local_player(ia).unwrap();
    let bank = |sim: &chimera_net::harness::NetSim| sim.clients[ia].app.world().get::<CaBank>(me).map_or(0, |b| b.0);
    assert!(sim.until(60, |s| bank(s) == 1000), "the bank on the client");
    sim.clients[ia].app.world_mut().write_message(CaHireRequest { unit: 0 });
    let hired = sim.until(120, |s| bank(s) == 800);
    assert!(hired, "hired: bank {}", bank(&sim));
    let mercs_seen = sim.until(120, |s| {
        (0..2).all(|i| {
            let w = s.clients[i].app.world_mut();
            w.query::<&CaMerc>().iter(w).count() == 1
        })
    });
    assert!(mercs_seen, "every client sees the unit");
    {
        let w = sim.clients[ia].app.world_mut();
        let m = *w.query::<&CaMerc>().iter(w).next().unwrap();
        assert_eq!(m.leader, me, "its hirer, mapped to the client's player");
    }
    sim.clients[ia]
        .app
        .world_mut()
        .write_message(CaOrderRequest { order: MercOrder::FollowMe });
    let ordered = sim.until(120, |s| {
        s.server.app.world().get::<CaSquadOrder>(a).is_some_and(|o| o.0 == MercOrder::FollowMe)
            && s.clients[ia].app.world().get::<CaSquadOrder>(me).is_some_and(|o| o.0 == MercOrder::FollowMe)
    });
    assert!(ordered, "the order on the server and back");
    // A pocket, then the bank.
    sim.server.app.world_mut().entity_mut(a).insert(CaCarry {
        bars: 3,
        money: 300,
        ..default()
    });
    let pocket = sim.until(60, |s| s.clients[ia].app.world().get::<CaCarry>(me).is_some_and(|c| c.bars == 3));
    assert!(pocket, "the pocket on the client");
    hold_server(&mut sim, a, bank_a, 4);
    let banked = sim.until(60, |s| (0..2).all(|i| client_objectives(s, i).banked == [3, 0]) && bank(s) == 1100);
    assert!(banked, "banked everywhere: {:?}", client_objectives(&sim, 0).banked);
}

/// VIP Escort: every client sees who the VIP is, the escort team and the
/// exit's timer; the VIP's escape gives Alpha the round on every client.
#[test]
fn vip_escort_over_the_network() {
    let mut sim = mode_sim("ESC", 5);
    let exit = Vec3::new(-8.0, 1.0, 25.0);
    {
        let mut o = sim.server.app.world_mut().resource_mut::<CaMapObjects>();
        o.points = vec![named("TeleportPoint", "VipTeleportPoint", Vec3::new(8.0, 1.0, 25.0))];
        o.escape_zones = vec![EscapeZone {
            name: "EscapeZoneTrigger".into(),
            family: "VIPEscort".into(),
            area: ObjBox {
                centre: exit,
                half: Vec3::new(128.0, 80.0, 128.0) * UNIT,
            },
        }];
    }
    restart_and_play(&mut sim);
    let [(ia, a), _] = by_team(&mut sim);
    assert!(sim.server.app.world().get::<CaVip>(a).is_some(), "the Alpha player is the VIP");
    let me = sim.local_player(ia).unwrap();
    let marked = sim.until(60, |s| {
        (0..2).all(|i| {
            let w = s.clients[i].app.world_mut();
            w.query_filtered::<(), With<CaVip>>().iter(w).count() == 1
        }) && s.clients[ia].app.world().get::<CaVip>(me).is_some()
    });
    assert!(marked, "every client sees the VIP");
    let o = client_objectives(&sim, 1);
    assert_eq!(o.escort, 1);
    let now = sim.clients[1].app.world().resource::<chimera_core::core::SimClock>().now;
    let opens = o.exit.unwrap().opens - now;
    assert!((56.0..62.0).contains(&opens), "the exit opens in {opens:.1} s");
    // Open it now; the VIP walks in.
    let t = sim.server.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.server
        .app
        .world_mut()
        .resource_mut::<combat_arms::game::rules::objectives::CaObjectives>()
        .exit = Some(Exit { opens: t, open: true, ..Default::default() });
    hold_server(&mut sim, a, exit, 3);
    let won = sim.until(300, |s| (0..2).all(|i| view(s, i).round_wins == [1, 0]));
    assert!(won, "Alpha's round everywhere: {:?}", view(&sim, 0).round_wins);
    let escapes = sim.clients[ia].app.world().get::<CaStats>(me).map(|s| s.vip_escapes);
    assert_eq!(escapes, Some(1));
}

/// Clan Elimination with G 4: two kills make half time; every client sees
/// the teams swapped (its own player too), the counters with them and
/// the second half.
#[test]
fn clan_modes_over_the_network() {
    let mut sim = mode_sim("CDM", 4);
    restart_and_play(&mut sim);
    let [(ia, a), (ib, b)] = by_team(&mut sim);
    kill(&mut sim, b, a);
    sim.ticks((4.5 / TICK_INTERVAL) as u64);
    let back = sim.until(700, |s| s.server.app.world().get::<chimera_core::core::Dead>(b).is_none());
    assert!(back);
    sim.ticks((4.2 / TICK_INTERVAL) as u64);
    kill(&mut sim, b, a);
    let swapped = sim.until(800, |s| {
        let ma = s.local_player(ia).unwrap();
        let mb = s.local_player(ib).unwrap();
        s.clients[ia].app.world().get::<chimera_core::core::Team>(ma).map(|t| t.0) == Some(2)
            && s.clients[ib].app.world().get::<chimera_core::core::Team>(mb).map(|t| t.0) == Some(1)
            && (0..2).all(|i| view(s, i).score == [0, 2] && client_objectives(s, i).half == 2)
    });
    assert!(swapped, "swapped everywhere: {:?} {}", view(&sim, 0).score, client_objectives(&sim, 0).half);
}

/// The 2026-10-22 corrections over the network (game_modes.md Corrections
/// 2, 3): in S&D a third client joining a round that has been live for a
/// second is held (it counts itself); then the Creed client leaves and
/// the round goes to JSF on the server.
#[test]
fn late_joining_and_leaving_over_the_network() {
    let mut sim = mode_sim("TMM", 5);
    restart_and_play(&mut sim);
    sim.ticks((1.5 / TICK_INTERVAL) as u64);
    let third = sim.add_client(|_| {});
    let came = sim.until(600, |s| s.character_of(third).is_some());
    assert!(came, "the third client's character");
    let e = sim.character_of(third).unwrap();
    sim.ticks(5);
    let held = {
        let w = sim.server.app.world();
        w.get::<combat_arms::game::rules::spawns::CaHeld>(e).is_some() || w.get::<chimera_core::core::Dead>(e).is_some()
    };
    assert!(held, "held until the next setup");
    // Creed's only player leaves: JSF wins the round.
    let creed = if player(&mut sim, 0).1 == 2 { 0 } else { 1 };
    sim.remove_client(creed);
    let won = sim.until(300, |s| server_match(s).winner == Some(1));
    assert!(won, "JSF's round: {:?}", server_match(&sim).winner);
}
