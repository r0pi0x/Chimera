//! Halo's movement over the network (`harness::NetSim`, Chimera's server
//! and a client on a simulated link): a client predicts its own player's
//! movement (`HaloMovement` registered with `predicted_net`) while it
//! walks, jumps and crouches on the greybox, without a prediction error,
//! at 30 Hz and 50 ms.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::Intent;
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_world::greybox::GreyboxMapPlugin;
use halo_ce::{
    game::{TICK_INTERVAL, movement::HaloMovement},
    harness::HaloSim,
};

#[test]
fn movement_is_predicted_without_errors() {
    let link = LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(HaloSim, link, 31, 1, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(90);
    let me = sim.local_player(0).expect("a local player");
    assert!(sim.clients[0].app.world().get::<HaloMovement>(me).is_some(), "the client predicts its movement");
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let theirs = sim.character_of(0).expect("the server's character");
    let start = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    let (mut top, mut crouched) = (f32::MIN, false);
    for k in 0..150u32 {
        let mut i = sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap();
        i.move_axis = match k {
            0..=49 => Vec2::Y,
            50..=99 => Vec2::new(1.0, 1.0),
            _ => Vec2::ZERO,
        };
        i.yaw = k as f32 * 0.01;
        i.jump = (40..=42).contains(&k);
        i.crouch = (80..=110).contains(&k);
        sim.step();
        let world = sim.server.app.world();
        top = top.max(world.get::<Transform>(theirs).unwrap().translation.y);
        crouched |= world.get::<HaloMovement>(theirs).unwrap().crouch > 0.5;
    }
    sim.ticks(60);
    let end = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    assert!(end.distance(start) > 3.0, "moved {} m", end.distance(start));
    assert!(top > start.y + 1.0, "jumped to {top} from {}", start.y);
    assert!(crouched, "crouched");
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!("{} states checked, {} errors ({:?})", g.checked, g.errors, g.last_differing);
    assert!(g.checked > 0, "compared no state");
    assert_eq!(g.errors, 0, "mispredicted ({:?})", g.last_differing);
}
