//! A bot's chat line over the network (specs/combat_arms/bots.md 10): the
//! server's `CaBotChat` reaches every remote client as the same message,
//! for its own chat box (`bots::chat::forward`, `hear`).

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use combat_arms::{
    game::{TICK_INTERVAL, bots::chat::CaBotChat},
    harness::CaSim,
};

use super::net_modes::setup;

/// What a machine's chat box was told, in order.
#[derive(Resource, Default)]
struct Heard(Vec<String>);

fn collect(mut lines: MessageReader<CaBotChat>, mut heard: ResMut<Heard>) {
    heard.0.extend(lines.read().map(|l| l.key.clone()));
}

fn collecting(app: &mut App) {
    setup(app);
    app.init_resource::<Heard>().add_systems(Update, collect);
}

#[test]
fn a_bots_line_reaches_every_client() {
    let link = LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(CaSim, link, 7, 2, collecting);
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    let someone = sim.character_of(0).unwrap();
    sim.server.app.world_mut().write_message(CaBotChat { bot: someone, key: "$Message1_Kill_4".into() });
    let said = |sim: &NetSim, i: usize| sim.clients[i].app.world().resource::<Heard>().0.clone();
    let ok = sim.until(120, |s| (0..2).all(|i| !said(s, i).is_empty()));
    assert!(ok, "client 0 {:?}, client 1 {:?}", said(&sim, 0), said(&sim, 1));
    for i in 0..2 {
        assert_eq!(said(&sim, i), vec!["$Message1_Kill_4".to_string()], "client {i}");
    }
}
