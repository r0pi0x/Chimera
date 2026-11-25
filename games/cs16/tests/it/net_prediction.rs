//! CS 1.6's movement predicted (`cs16:movement` on `NetSim`'s in-memory
//! link, the greybox): with no loss a client's prediction of its own
//! player is never wrong through a run, strafes, jumps, a duck, a duck
//! tap's hop, a duck jump and the walk key, the jump penalty and the
//! command's milliseconds in its predicted state; every predicted tick is
//! the server's, bit for bit; with loss and jitter it ends where the
//! server has it.

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
use cs16::harness::Cs16Sim;

fn joined(latency_ms: u64, jitter_ms: u64, loss: f64, seed: u64) -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(latency_ms),
        jitter: Duration::from_millis(jitter_ms),
        loss,
    };
    let mut sim = NetSim::new(Cs16Sim, link, seed, 1, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.until_joined(900);
    // Respawn, the first states, the clock sync settling.
    sim.ticks(480);
    assert!(sim.clients[0].app.world().resource::<CommandClock>().tick.is_some(), "the client predicts");
    sim
}

/// The movement script for step `k`.
fn script(k: u32, i: &mut Intent) {
    i.move_axis = match k {
        0..=59 => Vec2::Y,
        60..=119 => Vec2::new(1.0, 1.0).normalize(),
        120..=179 => Vec2::new(-1.0, 0.0),
        180..=259 => Vec2::Y,
        _ => Vec2::ZERO,
    };
    i.yaw = 0.4 + k as f32 * 0.007;
    i.pitch = -0.1 + (k % 9) as f32 * 0.003;
    // A jump, a jump into the landing slowdown, a duck jump.
    i.jump = (30..=32).contains(&k) || (105..=106).contains(&k) || (195..=196).contains(&k);
    // A full duck, a duck tap (the hop), a duck in the air.
    i.crouch = (60..=110).contains(&k) || (150..=154).contains(&k) || (200..=215).contains(&k);
    i.walk = (125..=140).contains(&k);
}

fn client_intent(sim: &mut NetSim) -> Mut<'_, Intent> {
    let me = sim.local_player(0).unwrap();
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap()
}

/// The script run on the client, then idle until the last states are back;
/// the server character's positions.
fn run_script(sim: &mut NetSim, steps: u32) -> Vec<Vec3> {
    let theirs = sim.character_of(0).unwrap();
    let mut path = Vec::new();
    for k in 0..steps {
        script(k, &mut client_intent(sim));
        sim.step();
        path.push(sim.server.app.world().get::<Transform>(theirs).unwrap().translation);
    }
    *client_intent(sim) = Intent::default();
    sim.ticks(120);
    path
}

#[test]
fn without_loss_the_prediction_is_never_wrong() {
    let mut sim = joined(80, 0, 0.0, 16);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let theirs = sim.character_of(0).unwrap();
    let start = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    let path = run_script(&mut sim, 280);
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!("{} states checked, {} errors, {} resyncs", g.checked, g.errors, g.resyncs);
    assert_eq!(g.errors, 0, "prediction errors ({g:?})");
    assert_eq!(g.resyncs, 0, "restarts");
    assert!(g.checked > 50, "only {} states compared", g.checked);
    // It moved and jumped.
    let far = path.iter().map(|p| p.distance(start)).fold(0.0, f32::max);
    assert!(far > 3.0, "moved {far} m");
    let top = path.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(top > start.y + 0.5, "jumped to {top} from {}", start.y);
    // Every tick the client has predicted that the server has run is the
    // server's state for it, bit for bit (the movement's state with its
    // jump penalty and leftover milliseconds among it).
    script(40, &mut client_intent(&mut sim));
    let mut server = std::collections::HashMap::new();
    for _ in 0..40 {
        sim.step();
        let world = sim.server.app.world();
        server.insert(world.resource::<SimTick>().0, world.resource::<PredictedComponents>().encode(world, theirs));
    }
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

#[test]
fn with_loss_the_client_ends_where_the_server_has_it() {
    let mut sim = joined(60, 15, 0.05, 23);
    run_script(&mut sim, 280);
    sim.ticks(200);
    let theirs = sim.character_of(0).unwrap();
    let me = sim.local_player(0).unwrap();
    let server = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    let client = sim.clients[0].app.world().get::<Transform>(me).unwrap().translation;
    assert_eq!(server, client, "after the corrections the client stands where the server has it");
}
