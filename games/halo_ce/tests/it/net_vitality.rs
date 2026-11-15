//! A Halo player's vitality over the network (`NetSim` on `HaloSim`, the
//! greybox): the server applies a hit (`HaloHit`) and the client sees its
//! own shield, body and stun as the server has them (`Vitality`
//! replicated, `Stun` predicted).

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::greybox::GreyboxMapPlugin;
use halo_ce::{
    game::{
        damage::HaloHit,
        vitality::{DamageEffects, Stun, Vitality, ids},
    },
    harness::HaloSim,
};

#[test]
fn a_client_sees_its_shield_after_a_hit() {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(HaloSim, link, 7, 1, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    // A frame a tick (Halo's 30 Hz), so a message written between frames
    // meets the server's next tick.
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(30);
    let theirs = sim.character_of(0).expect("the client's player on the server");
    let fx = sim.server.app.world().resource::<DamageEffects>().clone();
    let amount = fx.get(ids::PLASMA_RIFLE_BOLT).unwrap().amount_with(1.0, 12.0);
    sim.server.app.world_mut().write_message(HaloHit::direct(
        theirs,
        ids::PLASMA_RIFLE_BOLT,
        amount,
        1.0,
        Some(chimera_core::core::BodyRegion::Chest),
        Vec3::ZERO,
        Vec3::NEG_Z,
        None,
    ));
    sim.ticks(1);
    let server = sim.server.app.world().get::<Vitality>(theirs).unwrap().clone();
    assert!((server.shield - 0.68).abs() < 1e-6, "{server:?}");
    sim.ticks(20);
    let mine = sim.local_player(0).expect("the client's own player");
    let w = sim.clients[0].app.world();
    let v = w.get::<Vitality>(mine).expect("replicated to its client");
    assert_eq!((v.shield, v.body), (server.shield, server.body));
    assert!(v.delay < 180 && v.delay > 150, "{}", v.delay);
    assert_eq!(w.get::<Health>(mine).unwrap().current, 1.0);
    // The stun of the bolt ran out on both (4 ticks).
    assert_eq!(w.get::<Stun>(mine).copied(), Some(Stun::default()));
}
