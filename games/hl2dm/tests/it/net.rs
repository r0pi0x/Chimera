//! HL2:DM's rules over the network (`NetSim` on `Hl2Sim`, the greybox,
//! the rules started on the server as a map load starts them): a client
//! joins and is on a team (TDM: Combine or Rebels), changes team with
//! `jointeam` (sent to the server, which runs it for its player, a
//! suicide charged to the new team), and comes back after its death by
//! pressing a button more than 3 s on.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{Damage, DamageKind, Dead, Hitgroup, Intent, Score},
};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::greybox::GreyboxMapPlugin;
use hl2dm::{
    game::rules::{self, COMBINE, Hl2Player, Hl2Rules, REBELS},
    harness::Hl2Sim,
};

fn tdm() -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(Hl2Sim, link, 7, 1, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    {
        let w = sim.server.app.world_mut();
        w.resource_mut::<Hl2Rules>().teamplay = 1;
        rules::start(w);
    }
    sim.until_joined(600);
    sim.ticks(60);
    sim
}

#[test]
fn join_team_and_respawn() {
    let mut sim = tdm();
    let theirs = sim.character_of(0).expect("the client's player on the server");
    let team = sim.server.app.world().get::<Hl2Player>(theirs).expect("in HL2:DM's game").team;
    assert!(team == COMBINE || team == REBELS, "joined a team: {team}");
    let other = if team == COMBINE { REBELS } else { COMBINE };

    // `jointeam` from the client's console, run by the server.
    sim.ticks(400);
    let line = format!("hl2dm:jointeam {other}");
    sim.clients[0].app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(30);
    let w = sim.server.app.world();
    assert_eq!(w.get::<Hl2Player>(theirs).unwrap().team, other, "changed team");
    assert!(w.get::<Dead>(theirs).is_some(), "a suicide");
    assert_eq!(w.get::<Score>(theirs).map(|s| (s.kills, s.deaths)), Some((-1, 1)));

    // Back after more than 3 s, on a press from the client.
    sim.ticks(240);
    let me = sim.local_player(0).unwrap();
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = true;
    sim.ticks(20);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = false;
    assert!(sim.server.app.world().get::<Dead>(theirs).is_none(), "respawned");

    // Killed by the world: dead, back again on a press.
    sim.server.app.world_mut().write_message(Damage {
        target: theirs,
        attacker: None,
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(10);
    assert!(sim.server.app.world().get::<Dead>(theirs).is_some());
    sim.ticks(100);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = true;
    sim.ticks(20);
    assert!(sim.server.app.world().get::<Dead>(theirs).is_some(), "not before 3 s");
    sim.ticks(120);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = false;
    assert!(sim.server.app.world().get::<Dead>(theirs).is_none(), "respawned after 3 s");
}
