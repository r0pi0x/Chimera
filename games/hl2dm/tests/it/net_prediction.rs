//! HL2:DM's movement and suit over the network (`NetSim` on `Hl2Sim`,
//! the greybox): a client predicts its player's walk, run and sprint, the
//! sprint's aux power and its stop cost, ducks and jumps, and every
//! predicted tick matches the server's bit for bit (the suit's state among
//! the predicted components); the server's aux power is what the client
//! predicted.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Intent, PredictedComponents, SimTick};
use chimera_net::{
    harness::NetSim,
    net::{
        memory::LinkConditions,
        predict::{CommandClock, NetGraph, PredictionHistory},
    },
};
use chimera_world::greybox::GreyboxMapPlugin;
use hl2dm::{game::suit::Suit, harness::Hl2Sim};

fn joined(latency_ms: u64) -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(latency_ms),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(Hl2Sim, link, 31, 1, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.until_joined(600);
    sim.ticks(320);
    assert!(sim.clients[0].app.world().resource::<CommandClock>().tick.is_some(), "the client predicts");
    sim
}

fn client_intent(sim: &mut NetSim) -> Mut<'_, Intent> {
    let me = sim.local_player(0).unwrap();
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap()
}

/// Walk, sprint (pressed, released, pressed again), walk key, duck, jump.
fn script(k: u32, i: &mut Intent) {
    i.move_axis = if k < 150 { Vec2::Y } else { Vec2::ZERO };
    i.yaw = 0.2 + k as f32 * 0.01;
    i.sprint = (10..60).contains(&k) || (70..120).contains(&k);
    i.walk = i.sprint || (125..140).contains(&k);
    i.crouch = (90..105).contains(&k);
    i.jump = (40..42).contains(&k) || (130..132).contains(&k);
}

#[test]
fn sprint_and_aux_power_are_predicted() {
    let mut sim = joined(100);
    let theirs = sim.character_of(0).unwrap();
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let mut server = std::collections::HashMap::new();
    let mut lowest: f32 = 100.0;
    for k in 0..200 {
        script(k, &mut client_intent(&mut sim));
        sim.step();
        let world = sim.server.app.world();
        let tick = world.resource::<SimTick>().0;
        server.insert(tick, world.resource::<PredictedComponents>().encode(world, theirs));
        lowest = lowest.min(world.get::<Suit>(theirs).unwrap().aux);
    }
    *client_intent(&mut sim) = Intent::default();
    for _ in 0..80 {
        sim.step();
        let world = sim.server.app.world();
        let tick = world.resource::<SimTick>().0;
        server.insert(tick, world.resource::<PredictedComponents>().encode(world, theirs));
    }
    assert!(lowest < 90.0, "the sprint drew aux power ({lowest})");
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    assert_eq!(g.errors, 0, "prediction errors ({g:?})");
    let history = &sim.clients[0].app.world().resource::<PredictionHistory>().0;
    let mut compared = 0;
    for p in history.iter().filter(|p| !p.state.is_empty()) {
        if let Some(s) = server.get(&p.tick) {
            assert!(*s == p.state, "the prediction for tick {} isn't the server's", p.tick);
            compared += 1;
        }
    }
    assert!(compared > 0, "no predicted tick to compare");
}

