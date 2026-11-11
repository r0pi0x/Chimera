//! A player who drops (a killed client) and joins again shows once, not
//! twice until the dead connection times out: the server drops the silent
//! connection of the same name from the same address at once
//! (`server::PeerTable`, `drop_stale_twins`). Addresses come from the UDP
//! transport; here the test fills the table.

use std::{net::IpAddr, time::Duration};

use bevy::prelude::*;
use chimera_core::slots::Loadout;
use chimera_net::{
    harness::NetSim,
    net::{NetSettings, memory::LinkConditions, server::{PeerTable, Player}},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout { movement: placeholder::ID });
}

fn names(sim: &mut NetSim) -> Vec<String> {
    let world = sim.server.app.world_mut();
    let mut names: Vec<String> = world.query::<&Player>().iter(world).map(|p| p.name.clone()).collect();
    names.sort();
    names
}

fn named(name: &'static str) -> impl FnOnce(&mut World) {
    move |w| w.resource_mut::<NetSettings>().name = name.into()
}

fn peer(sim: &mut NetSim, id: u64, ip: &str, silent_secs: u64) {
    sim.server
        .app
        .world_mut()
        .resource_mut::<PeerTable>()
        .0
        .insert(id, (ip.parse::<IpAddr>().unwrap(), Duration::from_secs(silent_secs)));
}

fn joined(first: &'static str, second: &'static str) -> NetSim {
    let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(SimBasePlugins, link, 3, 0, setup);
    sim.add_client(named(first));
    sim.add_client(named(second));
    sim.until_joined(600);
    sim
}

#[test]
fn a_player_who_rejoins_from_the_same_address_shows_once() {
    let mut sim = joined("Ann", "Bob");
    assert_eq!(names(&mut sim), ["Ann", "Bob"]);
    // Ann's client is killed: it stops being stepped, nothing hangs up.
    drop(sim.clients.remove(0));
    peer(&mut sim, 1, "10.0.0.5", 5);
    peer(&mut sim, 2, "10.0.0.6", 0);
    // She comes back from the same address (a new connection id).
    let back = sim.add_client(named("Ann"));
    peer(&mut sim, 3, "10.0.0.5", 0);
    sim.until(600, |s| s.clients[back].app.world().contains_resource::<chimera_net::net::client::Joined>());
    sim.ticks(10);
    assert_eq!(names(&mut sim), ["Ann", "Bob"], "the dead connection went at once");
}

#[test]
fn another_address_or_a_live_connection_is_not_a_twin() {
    // The same name from another address stays (two players, one name).
    let mut sim = joined("Ann", "Bob");
    peer(&mut sim, 1, "10.0.0.5", 5);
    let other = sim.add_client(named("Ann"));
    peer(&mut sim, 3, "10.0.0.9", 0);
    sim.until(600, |s| s.clients[other].app.world().contains_resource::<chimera_net::net::client::Joined>());
    sim.ticks(10);
    assert_eq!(names(&mut sim), ["Ann", "Ann", "Bob"]);

    // The same name and address while the first is alive stays too (two
    // clients on one machine).
    let mut sim = joined("Ann", "Bob");
    peer(&mut sim, 1, "10.0.0.5", 0);
    let again = sim.add_client(named("Ann"));
    peer(&mut sim, 3, "10.0.0.5", 0);
    sim.until(600, |s| s.clients[again].app.world().contains_resource::<chimera_net::net::client::Joined>());
    sim.ticks(10);
    assert_eq!(names(&mut sim), ["Ann", "Ann", "Bob"]);
}
