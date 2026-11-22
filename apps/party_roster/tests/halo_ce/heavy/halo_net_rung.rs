//! A Halo rung over the network (the playtest's check, 2026-11-15): the
//! party's server on the greybox and two clients that only connect
//! (`NetSim`, 40 ms each way), a ladder of Halo's assault rifle; client 0
//! fires at client 1 for 30 simulated seconds. Measured, with
//! `--nocapture`: client 0's prediction errors a second (and what
//! differed), the rounds client 1 was shown of client 0's (`HaloFired`
//! of the copy), that client 1's copy of client 0 is Halo's (identity,
//! the Spartan's body) and that client 1 saw its own death. Asserts all
//! but the error rate, printed.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{core::Dead, slots::GameIdentity};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_sim::{minigame::MinigameAppExt, testkit};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{BodyModel, CharacterModels, MapCharacterModel, MapSimPlugin},
};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use halo_ce::game::weapons::{self, HaloFired};
use party_roster::PartyPartsPlugin;

const BODIES: [&str; 3] = ["combat_arms", "cs_source", "halo_ce"];

fn bodies(app: &mut App) {
    let body = |game| MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    app.insert_resource(CharacterModels(std::sync::Arc::new(BODIES.iter().map(|g| body(*g)).collect())));
}

/// Shown `HaloFired` rounds by owner entity, on a client.
#[derive(Resource, Default)]
struct Shown(Vec<Entity>);

fn shown(mut fired: MessageReader<HaloFired>, mut s: ResMut<Shown>) {
    s.0.extend(fired.read().map(|f| f.owner));
}

#[test]
fn a_halo_rung_plays_over_the_network() {
    let text = crate::play::ladder_text(&[weapons::ASSAULT_RIFLE; 5], false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 11, 2, |app| {
        app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin));
        bodies(app);
        app.init_resource::<Shown>().add_systems(PostUpdate, shown);
    });
    crate::play::plain_rungs(sim.server.app.world_mut());
    crate::play::no_finale(sim.server.app.world_mut());
    sim.server.app.add_minigames(&[("halo_net.toml", &text)]).unwrap();
    chimera_sim::minigame::start(sim.server.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.ticks(300);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    crate::net_play::face_off(&mut sim, a, b);
    sim.ticks(60);
    for c in &mut sim.clients {
        *c.app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        c.app.world_mut().resource_mut::<Shown>().0.clear();
    }
    let me = sim.local_player(0).unwrap();
    let victim = sim.local_player(1).unwrap();
    let copy = {
        let id = sim.client_id(0);
        NetSim::owned_by(&mut sim.clients[1], Some(id)).expect("client 1's copy of client 0")
    };
    assert_eq!(crate::net_play::held(&mut sim, 0), Some(weapons::ASSAULT_RIFLE), "the Halo rung's gun on client 0");
    let w1 = sim.clients[1].app.world();
    assert_eq!(w1.get::<GameIdentity>(copy).map(|g| g.0.as_str()), Some("halo_ce"), "client 1 sees a Halo player");
    assert_eq!(w1.get::<BodyModel>(copy).map(|m| BODIES[m.0]), Some("halo_ce"), "client 1 sees the Spartan's body");
    let ticks = (30.0 / TICK_INTERVAL) as u64;
    let mut died_seen = false;
    for _ in 0..ticks {
        let target = sim.server.position(b);
        testkit::aim_at(&mut sim.clients[0], me, target);
        sim.clients[0].intent(me).fire = true;
        sim.step();
        died_seen |= sim.clients[1].app.world().get::<Dead>(victim).is_some();
    }
    sim.clients[0].intent(me).fire = false;
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    let seen = sim.clients[1].app.world().resource::<Shown>().0.iter().filter(|e| **e == copy).count();
    let own = sim.clients[0].app.world().resource::<Shown>().0.iter().filter(|e| **e == me).count();
    println!(
        "client 0 over 30 s: {} states checked, {} errors ({:.2}/s, last differing {:?}, worst {}), {} resyncs; rounds shown on client 0 {own}, of client 0 on client 1 {seen}; client 1 saw its death: {died_seen}",
        g.checked,
        g.errors,
        g.errors as f64 / 30.0,
        g.last_differing,
        g.worst_error,
        g.resyncs
    );
    assert!(seen > 0, "client 1 was shown none of client 0's Halo rounds");
    assert!(died_seen, "client 1 never saw itself dead");
}
