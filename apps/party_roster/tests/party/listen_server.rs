//! A listen server (playtest 2026-10-28: hosting from inside the app
//! with `+maxplayers 4` panicked in bevy_replicon's `collect_changes` on
//! its first send): the gun game served by a process with its own local
//! player, two bots and two network clients, the presentation's plugins
//! in the server's app before the network's as the apps add them
//! (`IdleLightsPlugin` there built `PostUpdate` in its `finish`, before
//! `NetPlugin::finish` registered the parts' replicated components, so
//! replicon's query lacked access to them). Runs a few seconds with a
//! rung swap for the host, a client and a bot.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_present::render::idle_lights::IdleLightsPlugin;
use chimera_sim::minigame::{
    self,
    ladder::{self, Ladder, Rung},
};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{
    game::{TICK_INTERVAL, bots::roster, movement},
    harness::CaSim,
};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
}

fn swap(sim: &mut NetSim, e: Entity, rung: usize) {
    let w = sim.server.app.world_mut();
    let ladder = w.resource::<Ladder>().clone();
    ladder::set_rung(w, e, rung, &ladder);
}

#[test]
fn a_listen_server_with_its_own_player_bots_and_two_clients_runs() {
    let mut sim = NetSim::new(
        (IdleLightsPlugin, CaSim),
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        21,
        2,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    // The host's own player, then two bots.
    let host = sim.server.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.server.app.world_mut().entity_mut(host).insert(LocalPlayer);
    let bots: Vec<Entity> =
        (0..2).map(|_| roster::add_bot(sim.server.app.world_mut(), None).expect("a bot")).collect();
    sim.until_joined(900);
    sim.ticks(120);
    // A rung swap each for the host, a client's player and a bot.
    let client = sim.character_of(0).expect("client 0 plays");
    for e in [host, client, bots[0]] {
        swap(&mut sim, e, 1);
    }
    sim.ticks((3.0 / TICK_INTERVAL) as u64);
    let w = sim.server.app.world();
    for e in [host, client, bots[0]] {
        assert_eq!(w.get::<Rung>(e).map(|r| r.0), Some(1), "{e} swapped rung");
    }
    // The clients see the swap on their own player.
    let me = sim.local_player(0).expect("client 0's own player");
    assert_eq!(sim.clients[0].app.world().get::<Rung>(me).map(|r| r.0), Some(1));
    // Both clients see the host and the bots.
    for i in 0..2 {
        let seen = NetSim::owned_by(&mut sim.clients[i], None);
        assert!(seen.is_some(), "client {i} sees the host or a bot");
    }
}
