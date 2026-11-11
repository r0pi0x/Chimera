//! Contract tests of the network's prediction and replication
//! (`chimera_net`) on the engine's base alone (`SimBasePlugins`), the
//! greybox and the placeholder movement: nothing of a game. A lift of
//! network code into Chimera is checked here (`cargo test -p chimera_net
//! --test it`) before the games' suites.
//!
//! - With no loss a scripted run (walk, strafe, turn, jump, stop) is
//!   predicted with no error, and each predicted tick is the server's
//!   state for it, bit for bit.
//! - With loss and jitter mistakes are corrected: the client ends where the
//!   server has it.
//! - An own-state part (`NetAppExt::own_part`) reaches the client's player.
//! - Joining while the map settles behind the loading screen (a frame runs
//!   at most a tick, `map::MapSettling`) the clock doesn't jump: it starts
//!   once the map is in.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    core::{Intent, PredictedComponents, SimTick},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    net::{
        NetAppExt,
        memory::LinkConditions,
        predict::{CommandClock, NetGraph, PredictionHistory},
    },
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

fn link(latency_ms: u64, jitter_ms: u64, loss: f64) -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(latency_ms),
        jitter: Duration::from_millis(jitter_ms),
        loss,
    }
}

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout {
        movement: placeholder::ID,
    });
}

/// A server and `clients` clients, joined, respawned and predicting with
/// their clocks settled.
fn joined(conditions: LinkConditions, seed: u64, clients: usize, setup: fn(&mut App)) -> NetSim {
    let mut sim = NetSim::new(SimBasePlugins, conditions, seed, clients, setup);
    sim.until_joined(600);
    sim.ticks(320);
    for i in 0..clients {
        let world = sim.clients[i].app.world();
        assert!(world.resource::<CommandClock>().tick.is_some(), "client {i} predicts");
    }
    sim
}

fn script(k: u32, i: &mut Intent) {
    i.move_axis = match k {
        0..=39 => Vec2::Y,
        40..=79 => Vec2::new(1.0, 1.0).normalize(),
        80..=119 => Vec2::new(-1.0, 0.0),
        _ => Vec2::ZERO,
    };
    i.yaw = 0.4 + k as f32 * 0.011;
    i.jump = (20..=22).contains(&k) || (100..=101).contains(&k);
}

fn run_script(sim: &mut NetSim, i: usize, steps: u32) -> Vec<Vec3> {
    let theirs = sim.character_of(i).unwrap();
    let mut path = Vec::new();
    for k in 0..steps {
        let me = sim.local_player(i).unwrap();
        script(k, &mut sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap());
        sim.step();
        path.push(sim.server.app.world().get::<Transform>(theirs).unwrap().translation);
    }
    let me = sim.local_player(i).unwrap();
    *sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap() = Intent::default();
    sim.ticks(80);
    path
}

/// Step 30 more ticks; every tick the client predicted that the server ran
/// must be the server's state for it, bit for bit.
fn assert_agrees(sim: &mut NetSim, i: usize) {
    let theirs = sim.character_of(i).unwrap();
    let mut server = std::collections::BTreeMap::new();
    for _ in 0..30 {
        sim.step();
        let world = sim.server.app.world();
        let tick = world.resource::<SimTick>().0;
        server.insert(tick, world.resource::<PredictedComponents>().encode(world, theirs));
    }
    let history = &sim.clients[i].app.world().resource::<PredictionHistory>().0;
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
fn without_loss_the_prediction_is_never_wrong() {
    for (latency, seed) in [(50, 21), (100, 22), (150, 23)] {
        let mut sim = joined(link(latency, 0, 0.0), seed, 1, setup);
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        let theirs = sim.character_of(0).unwrap();
        let start = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
        let path = run_script(&mut sim, 0, 160);
        let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
        assert_eq!(g.errors, 0, "{latency} ms: prediction errors ({g:?})");
        assert_eq!(g.resyncs, 0, "{latency} ms: restarts");
        assert!(g.checked > 50, "{latency} ms: only {} states compared", g.checked);
        let far = path.iter().map(|p| p.distance(start)).fold(0.0, f32::max);
        assert!(far > 3.0, "{latency} ms: moved {far} m");
        assert_agrees(&mut sim, 0);
    }
}

#[test]
fn with_loss_and_jitter_the_client_ends_where_the_server_has_it() {
    let mut sim = joined(link(80, 20, 0.05), 24, 1, setup);
    run_script(&mut sim, 0, 160);
    sim.ticks(120);
    let theirs = sim.character_of(0).unwrap();
    let me = sim.local_player(0).unwrap();
    let server = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    let client = sim.clients[0].app.world().get::<Transform>(me).unwrap().translation;
    assert!(server.distance(client) < 0.05, "server {server}, client {client}");
}

#[derive(Component)]
struct Tag(u8);

fn with_tag_part(app: &mut App) {
    setup(app);
    app.own_part(
        "kit:tag",
        |w, e| w.get::<Tag>(e).map(|t| vec![t.0]),
        |w, e, bytes| {
            match bytes {
                Some(b) => w.entity_mut(e).insert(Tag(b[0])),
                None => w.entity_mut(e).remove::<Tag>(),
            };
        },
    );
}

#[test]
fn an_own_state_part_reaches_the_clients_player() {
    let mut sim = joined(link(40, 0, 0.0), 25, 2, with_tag_part);
    let theirs = sim.character_of(0).unwrap();
    sim.server.app.world_mut().entity_mut(theirs).insert(Tag(42));
    sim.ticks(40);
    let me = sim.local_player(0).unwrap();
    let got = sim.clients[0].app.world().get::<Tag>(me).map(|t| t.0);
    assert_eq!(got, Some(42), "the client's player got the part");
    // Only the owner's: the other client's player doesn't have it.
    let other = sim.local_player(1).unwrap();
    assert!(sim.clients[1].app.world().get::<Tag>(other).is_none());
    // Taken off on the server, taken off on the client.
    sim.server.app.world_mut().entity_mut(theirs).remove::<Tag>();
    sim.ticks(40);
    assert!(sim.clients[0].app.world().get::<Tag>(me).is_none());
}

/// A client joins while its map still settles behind the loading screen
/// (`map::MapSettling`, as after a map change): its frames are two ticks
/// long but run at most one (`Time<Virtual>`'s cap, `client::console`'s
/// `LoadCaps`). A clock started then falls a tick behind every frame and
/// jumped every few frames (25 jumps and 85 restarts per client after a
/// map change, 2026-11-11); it waits for the game instead and starts with
/// its first state.
#[test]
fn a_map_settling_behind_the_loading_screen_makes_no_clock_jumps() {
    let tick = Duration::from_secs_f64(1.0 / chimera_core::core::DEFAULT_TICK_HZ);
    let mut sim = NetSim::new(SimBasePlugins, link(30, 0, 0.0), 41, 0, setup);
    let c = sim.add_client(|w| {
        w.insert_resource(chimera_world::map::MapSettling);
    });
    fn client(sim: &mut NetSim) -> &mut World {
        sim.clients[0].app.world_mut()
    }
    client(&mut sim).resource_mut::<Time<Virtual>>().set_max_delta(tick);
    sim.clients[c]
        .app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(tick * 2));
    // A frame: two server ticks, one client frame.
    let frame = |sim: &mut NetSim| {
        for _ in 0..2 {
            sim.link.lock().unwrap().advance(tick);
            sim.server.app.update();
        }
        sim.clients[c].app.update();
    };
    let mut joined = false;
    for _ in 0..400 {
        frame(&mut sim);
        if client(&mut sim).contains_resource::<chimera_net::net::client::Joined>() {
            joined = true;
            break;
        }
    }
    assert!(joined, "the client didn't join");
    // Five seconds of settling.
    for _ in 0..(5.0 / (2.0 * tick.as_secs_f64())) as u32 {
        frame(&mut sim);
    }
    let w = client(&mut sim);
    let (clock, graph) = (w.resource::<CommandClock>().clone(), w.resource::<NetGraph>().clone());
    assert_eq!(clock.jumps, 0, "the clock jumped while the map settled ({graph:?})");
    assert_eq!(graph.resyncs, 0, "prediction restarted while the map settled");
    // In the game: frames uncapped, a tick each.
    let w = client(&mut sim);
    w.remove_resource::<chimera_world::map::MapSettling>();
    w.resource_mut::<Time<Virtual>>().set_max_delta(Duration::from_millis(250));
    sim.clients[c]
        .app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(tick));
    sim.ticks(320);
    let w = client(&mut sim);
    let (clock, graph) = (w.resource::<CommandClock>().clone(), w.resource::<NetGraph>().clone());
    assert!(clock.tick.is_some(), "the client predicts once in the game");
    assert_eq!(clock.jumps, 0, "the clock jumped after the map settled ({graph:?})");
    assert!(graph.resyncs <= 1, "{} prediction restarts after the map settled", graph.resyncs);
    assert_eq!(graph.errors, 0, "prediction errors after the map settled");
}
