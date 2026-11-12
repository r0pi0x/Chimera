//! A client whose server goes away joins it again on its own once it is
//! back (`net::reconnect`); leaving by choice, a refusal or
//! `cl_autoreconnect 0` don't retry.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{core::NetRole, slots::Loadout};
use chimera_net::{
    harness::NetSim,
    net::{
        self, NetVersion,
        client::{BY_USER, JoinFailure, JoinProgress, Joined},
        memory::LinkConditions,
        reconnect::{RETRY_EVERY, ReconnectSettings, Reconnecting},
        server::Player,
    },
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout { movement: placeholder::ID });
}

fn sim() -> NetSim {
    let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
    NetSim::new(SimBasePlugins, link, 7, 0, setup)
}

fn role(sim: &NetSim, i: usize) -> NetRole {
    *sim.clients[i].app.world().resource::<NetRole>()
}

fn reconnecting(sim: &NetSim, i: usize) -> Option<(u32, String)> {
    sim.clients[i]
        .app
        .world()
        .get_resource::<Reconnecting>()
        .map(|r| (r.attempt, r.status()))
}

fn joined(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().contains_resource::<Joined>()
}

fn players(sim: &mut NetSim) -> usize {
    let w = sim.server.app.world_mut();
    w.query::<&Player>().iter(w).count()
}

/// Ticks in `secs` seconds.
fn ticks(secs: f64) -> u64 {
    (secs * chimera_core::core::DEFAULT_TICK_HZ).ceil() as u64
}

#[test]
fn a_client_joins_its_server_again_once_it_is_back() {
    let mut sim = sim();
    sim.add_client(|_| {});
    sim.until_joined(600);
    assert_eq!(players(&mut sim), 1);

    // The server quits: the client waits on the menu, asking.
    sim.stop_server();
    assert!(sim.until(ticks(2.0), |s| role(s, 0) == NetRole::Standalone), "the client heard the server go");
    sim.ticks(ticks(RETRY_EVERY * 2.5));
    let (attempt, text) = reconnecting(&sim, 0).expect("reconnecting");
    assert!(attempt >= 2, "asked again every {RETRY_EVERY} s: {attempt}");
    assert!(text.starts_with("Connection lost - reconnecting... (attempt"), "{text}");
    assert_eq!(role(&sim, 0), NetRole::Standalone, "no join while the server is down");
    let progress = sim.clients[0].app.world().resource::<JoinProgress>();
    assert!(progress.failure.is_none(), "the reconnect box, not the failure box: {:?}", progress.failure);

    // A new server on the same address: the client joins it on its own.
    sim.start_server();
    assert!(
        sim.until(ticks(RETRY_EVERY + 10.0), |s| joined(s, 0) && s.local_player(0).is_some()),
        "rejoined: {:?}",
        reconnecting(&sim, 0)
    );
    assert!(reconnecting(&sim, 0).is_none(), "done reconnecting once in");
    sim.ticks(10);
    assert_eq!(players(&mut sim), 1);
}

#[test]
fn leaving_by_choice_does_not_reconnect() {
    let mut sim = sim();
    sim.add_client(|_| {});
    sim.until_joined(600);
    net::disconnect(sim.clients[0].app.world_mut(), BY_USER);
    sim.ticks(ticks(RETRY_EVERY * 3.0));
    assert_eq!(role(&sim, 0), NetRole::Standalone);
    assert!(reconnecting(&sim, 0).is_none());
    assert_eq!(players(&mut sim), 0);

    // Cancelling while waiting stops it too, as does `cl_autoreconnect 0`.
    let mut sim = self::sim();
    sim.add_client(|_| {});
    sim.until_joined(600);
    sim.stop_server();
    assert!(sim.until(ticks(2.0), |s| reconnecting(s, 0).is_some()));
    net::disconnect(sim.clients[0].app.world_mut(), BY_USER);
    assert!(reconnecting(&sim, 0).is_none(), "Cancel (disconnect) while waiting");
    sim.start_server();
    sim.ticks(ticks(RETRY_EVERY * 2.0));
    assert_eq!(role(&sim, 0), NetRole::Standalone);

    let mut sim = self::sim();
    sim.add_client(|w| w.resource_mut::<ReconnectSettings>().enabled = 0);
    sim.until_joined(600);
    sim.stop_server();
    assert!(sim.until(ticks(2.0), |s| role(s, 0) == NetRole::Standalone));
    sim.ticks(10);
    assert!(reconnecting(&sim, 0).is_none(), "cl_autoreconnect 0");
    let failure = sim.clients[0].app.world().resource::<JoinProgress>().failure.clone();
    assert_eq!(failure.map(|f| f.0), Some(JoinFailure::Dropped), "the failure box as before");
}

#[test]
fn a_refused_client_does_not_reconnect() {
    let mut sim = sim();
    sim.add_client(|w| w.insert_resource(NetVersion("0.0.1/net1".into())));
    assert!(
        sim.until(ticks(5.0), |s| s.clients[0].app.world().resource::<JoinProgress>().failure.is_some()),
        "refused"
    );
    let (kind, reason) = sim.clients[0].app.world().resource::<JoinProgress>().failure.clone().unwrap();
    assert_eq!(kind, JoinFailure::NewServer, "{reason}");
    sim.ticks(ticks(RETRY_EVERY * 3.0));
    assert!(reconnecting(&sim, 0).is_none());
    assert_eq!(role(&sim, 0), NetRole::Standalone);
    assert_eq!(players(&mut sim), 0);
}
