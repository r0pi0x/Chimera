//! A campaign body over the network (`NetSim` on `HaloSim` with
//! `campaign::CampaignPlugin`, the greybox): the server gives a player a
//! body kind and its client sees it, and names its model (`BodyName`), as
//! every machine does; taken off on the server, it goes on the client.

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::{greybox::GreyboxMapPlugin, map::BodyName};
use halo_ce::{game::campaign::{BodyKind, CampaignPlugin}, harness::HaloSim};

#[test]
fn a_client_sees_the_body_kind_the_server_gives() {
    let link = LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 5, 1, |app: &mut App| {
        app.add_plugins((GreyboxMapPlugin, CampaignPlugin));
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(10);
    let theirs = sim.character_of(0).expect("the client's player on the server");
    sim.server.app.world_mut().entity_mut(theirs).insert(BodyKind::Jackal);
    sim.ticks(20);
    let mine = sim.local_player(0).expect("the client's own player");
    let w = sim.clients[0].app.world();
    assert_eq!(w.get::<BodyKind>(mine), Some(&BodyKind::Jackal));
    assert_eq!(w.get::<BodyName>(mine).map(|n| n.0.as_str()), Some("halo_ce:jackal"));
    sim.server.app.world_mut().entity_mut(theirs).insert(BodyKind::Spartan);
    sim.ticks(20);
    let w = sim.clients[0].app.world();
    assert_eq!(w.get::<BodyKind>(mine), Some(&BodyKind::Spartan));
    // Emptied, not taken off (`BodyName::none`).
    assert!(BodyName::named(w.get::<BodyName>(mine)).is_none());
}
