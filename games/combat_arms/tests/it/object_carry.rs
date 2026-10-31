//! A client riding Combat Arms' moving world models (specs/combat_arms/
//! world_objects.md 10, Client): on a network game the client steps every
//! active world model to the tick it predicts as the server's object frame
//! does and carries its own player with it, so a player on a rising lift
//! is predicted where the server has it (no corrections while riding), and
//! a door closing onto a player stops above it on the client as on the
//! server. Keyframers are stepped too, and the platform a keyframer moves
//! carries its riders on the server and on the client alike (10: the
//! engine carries on any move).
//!
//! `cargo test -p combat_arms --features dev --test it object_carry:: --
//! --nocapture` prints the numbers.

use std::{collections::BTreeMap, time::Duration};

use bevy::prelude::*;
use chimera_core::core::{PredictedComponents, SimTick};
use chimera_net::{
    harness::NetSim,
    net::{
        memory::LinkConditions,
        predict::{NetGraph, PredictionHistory},
    },
};
use chimera_world::map::MapSimPlugin;
use combat_arms::{
    game::{
        TICK_INTERVAL, movement,
        objects::{
            CaObjectMessage, CaObjects,
            geometry::{from_engine, to_engine},
        },
        world,
    },
    harness::CaSim,
};
use lithtech_formats::world::{BlindChunk, Object, Value, World};

use crate::world_objects_play::{cuboid, prop, world_with};

pub(crate) fn link(latency_ms: u64, jitter_ms: u64) -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(latency_ms),
        jitter: Duration::from_millis(jitter_ms),
        loss: 0.0,
    }
}

pub(crate) fn graph(sim: &NetSim) -> NetGraph {
    sim.clients[0].app.world().resource::<NetGraph>().clone()
}

/// A 200 u square lift 10 u high at x 400 (`SlidingWorldModel` "Lift",
/// `ForceMove`) that rises 200 u in 2 s when switched on and comes down
/// when off.
fn lift_world() -> World {
    world_with(
        vec![cuboid("Lift", [300.0, 0.0, -100.0], [500.0, 10.0, 100.0], false)],
        vec![Object {
            class: "SlidingWorldModel".into(),
            properties: vec![
                prop("Name", Value::String("Lift".into())),
                prop("Pos", Value::Vector([400.0, 5.0, 0.0])),
                prop("MoveDir", Value::Vector([0.0, 1.0, 0.0])),
                prop("MoveDist", Value::Real(200.0)),
                prop("PowerOnTime", Value::Real(2.0)),
                prop("PowerOffTime", Value::Real(2.0)),
                // (Else the game's own test refuses to move into the
                // player standing on it: world_objects.md 6.)
                prop("ForceMove", Value::Bool(true)),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    )
}

/// A sliding door at z 300 (x ±100, 150 high) hanging 200 u up, that
/// closes down onto the floor in 1 s when switched on.
fn gate_world() -> World {
    world_with(
        vec![cuboid("Gate", [-100.0, 200.0, 295.0], [100.0, 350.0, 305.0], false)],
        vec![Object {
            class: "SlidingWorldModel".into(),
            properties: vec![
                prop("Name", Value::String("Gate".into())),
                prop("Pos", Value::Vector([0.0, 275.0, 300.0])),
                prop("MoveDir", Value::Vector([0.0, -1.0, 0.0])),
                prop("MoveDist", Value::Real(200.0)),
                prop("PowerOnTime", Value::Real(1.0)),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    )
}

fn node_y(app: &App, name: &str, s: f32) -> f32 {
    let o = app.world().resource::<CaObjects>();
    let id = o.named(name).unwrap();
    let node = *o.nodes.get(&id).expect("a node");
    from_engine(app.world().get::<Transform>(node).unwrap().translation, s).y
}

pub(crate) fn send(sim: &mut NetSim, target: &str, message: &str) {
    sim.server.app.world_mut().write_message(CaObjectMessage {
        target: target.into(),
        message: message.into(),
    });
}

/// The server puts client 0's character at `at` (file frame, u), still.
pub(crate) fn put(sim: &mut NetSim, at: Vec3, s: f32) {
    let e = sim.character_of(0).unwrap();
    let p = to_engine(at, s);
    let world = sim.server.app.world_mut();
    world.get_mut::<Transform>(e).unwrap().translation = p;
    if let Some(mut q) = world.get_mut::<avian3d::prelude::Position>(e) {
        q.0 = p;
    }
}

/// What a ride saw.
#[derive(Debug, Default)]
pub(crate) struct Ride {
    /// Ticks (from the start) with a new prediction error.
    pub errors: Vec<u32>,
    /// The client's first prediction of its player against the server's
    /// position at the same tick, the largest difference (m): over the
    /// whole ride, and of the predictions made after the first `window`
    /// steps (the start's round trip).
    pub worst_gap: f32,
    pub gap_after: f32,
    /// The client's node of the mover as drawn after each step (at the
    /// tick it predicts) against the server's node at that tick, the
    /// largest difference after the first `window` steps (m).
    pub node_gap_after: f32,
    pub ticks: u32,
}

/// Step until the server's `name` node has been still for 64 ticks.
pub(crate) fn ride(sim: &mut NetSim, name: &str, max: u32, window: u32) -> Ride {
    let theirs = sim.character_of(0).unwrap();
    let mut out = Ride::default();
    let mut last = graph(sim).errors;
    let mut server: BTreeMap<u64, (u32, Vec3)> = BTreeMap::new();
    // Each tick's first prediction, and the step it was made at.
    let mut first: BTreeMap<u64, (u32, Vec3)> = BTreeMap::new();
    let mut still = 0;
    let pose = |app: &App| {
        let o = app.world().resource::<CaObjects>();
        let node = *o.nodes.get(&o.named(name).unwrap()).expect("a node");
        let t = app.world().get::<Transform>(node).unwrap();
        (t.translation, t.rotation)
    };
    let mut prev = pose(&sim.server.app);
    let mut server_node: BTreeMap<u64, Vec3> = BTreeMap::new();
    let mut client_node: Vec<(u64, Vec3)> = Vec::new();
    for k in 0..max {
        sim.step();
        let g = graph(sim);
        if g.errors > last {
            out.errors.push(k);
            last = g.errors;
        }
        let world = sim.server.app.world();
        server.insert(
            world.resource::<SimTick>().0,
            (k, world.get::<Transform>(theirs).unwrap().translation),
        );
        for p in &sim.clients[0].app.world().resource::<PredictionHistory>().0 {
            first.entry(p.tick).or_insert((k, p.position));
        }
        let now = pose(&sim.server.app);
        server_node.insert(sim.server.app.world().resource::<SimTick>().0, now.0);
        if k > window {
            let c = &sim.clients[0].app;
            client_node.push((c.world().resource::<chimera_core::core::SimClock>().tick, pose(c).0));
        }
        still = if now == prev { still + 1 } else { 0 };
        prev = now;
        out.ticks = k;
        if still > 64 && k > 64 {
            break;
        }
    }
    for (t, p) in &client_node {
        if let Some(q) = server_node.get(t) {
            out.node_gap_after = out.node_gap_after.max(p.distance(*q));
        }
    }
    for (t, (k, p)) in &first {
        let Some((_, q)) = server.get(t) else { continue };
        let gap = p.distance(*q);
        out.worst_gap = out.worst_gap.max(gap);
        if *k > window {
            out.gap_after = out.gap_after.max(gap);
        }
    }
    out
}

/// The client's predicted states of the last 30 ticks are the server's,
/// bit for bit.
pub(crate) fn assert_agrees(sim: &mut NetSim, what: &str) {
    let theirs = sim.character_of(0).unwrap();
    let mut server = std::collections::HashMap::new();
    for _ in 0..30 {
        sim.step();
        let world = sim.server.app.world();
        let tick = world.resource::<SimTick>().0;
        server.insert(tick, world.resource::<PredictedComponents>().encode(world, theirs));
    }
    let history = &sim.clients[0].app.world().resource::<PredictionHistory>().0;
    let compared = history
        .iter()
        .filter(|p| !p.state.is_empty())
        .filter_map(|p| server.get(&p.tick).map(|s| (p, s)))
        .inspect(|(p, s)| {
            assert!(
                **s == p.state,
                "{what}: the prediction for tick {} isn't the server's",
                p.tick
            )
        })
        .count();
    assert!(compared > 0, "{what}: no predicted tick to compare");
}

/// A client standing on a lift rides it up 200 u and back down: its
/// prediction follows the server's (errors only in the round trip after
/// each start, none while riding or at the stops; the drawn position
/// never more than a few centimetres from the server's at the same tick).
#[test]
fn a_client_rides_a_lift_predicted() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&lift_world(), "lift", s, &|_| None);
    for (latency, jitter, seed) in [(50, 0, 71), (150, 20, 72)] {
        let m = map.clone();
        let mut sim = NetSim::new(CaSim, link(latency, jitter), seed, 1, move |app| {
            app.add_plugins(MapSimPlugin::new(m.clone()));
        });
        sim.set_frame(TICK_INTERVAL);
        sim.until_joined(600);
        sim.ticks(240);
        put(&mut sim, Vec3::new(400.0, 60.0, 0.0), s);
        sim.ticks(200);
        let theirs = sim.character_of(0).unwrap();
        let feet0 = from_engine(sim.server.app.world().get::<Transform>(theirs).unwrap().translation, s).y;
        assert!(
            (node_y(&sim.clients[0].app, "Lift", s) - 5.0).abs() < 1e-3,
            "the client has the lift down"
        );
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();

        // Only the starts mispredict (the client hears of them a round
        // trip late): errors in the first round trip of each ride, none
        // while riding, none at the stops.
        let window = ((2 * latency + 2 * jitter) as f64 / 1000.0 / TICK_INTERVAL) as u32 + 8;
        send(&mut sim, "Lift", "on");
        let up = ride(&mut sim, "Lift", 600, window);
        let top = from_engine(sim.server.app.world().get::<Transform>(theirs).unwrap().translation, s).y;
        let carried = graph(&sim).carried;
        send(&mut sim, "Lift", "off");
        let down = ride(&mut sim, "Lift", 600, window);
        let bottom = from_engine(sim.server.app.world().get::<Transform>(theirs).unwrap().translation, s).y;
        let g = graph(&sim);
        println!(
            "{latency}±{jitter} ms: up {:.1} u in {} ticks, errors at {:?}, worst gap {:.3} m ({:.4} after the start, \
             lift {:.4}); down to {:.1} u in {} ticks, errors at {:?}, worst gap {:.3} m ({:.4}, lift {:.4}); carried {carried} ticks, {} of {} states wrong, worst \
             correction {:.3} m",
            top - feet0,
            up.ticks,
            up.errors,
            up.worst_gap,
            up.gap_after,
            up.node_gap_after,
            bottom - feet0,
            down.ticks,
            down.errors,
            down.worst_gap,
            down.gap_after,
            down.node_gap_after,
            g.errors,
            g.checked,
            g.worst_error
        );
        let what = format!("{latency}±{jitter} ms");
        assert!((top - feet0 - 200.0).abs() < 1.0, "{what}: rode up {}", top - feet0);
        assert!((bottom - feet0).abs() < 1.0, "{what}: back at {}", bottom - feet0);
        assert!(carried > 30, "{what}: carried {carried} ticks");
        for (dir, r) in [("up", &up), ("down", &down)] {
            assert!(
                r.gap_after < 1e-4,
                "{what}: {dir}: {} m off the server after the start",
                r.gap_after
            );
            assert!(
                r.node_gap_after < 1e-4,
                "{what}: {dir}: the client's lift {} m off",
                r.node_gap_after
            );
            assert!(
                r.errors.iter().all(|k| *k <= window),
                "{what}: {dir}: errors after the start's round trip ({:?}, window {window})",
                r.errors
            );
            assert!(r.errors.len() <= 4, "{what}: {dir}: {:?}", r.errors);
        }
        // No correction bigger than the lift's travel over the round trip
        // (the start), and the drawn path near the server's.
        let rtt = (2 * latency + 2 * jitter) as f32 / 1000.0 + 0.1;
        let travel = 100.0 * s * rtt;
        assert!(
            g.worst_error < travel,
            "{what}: a correction of {} m (travel over the round trip {travel})",
            g.worst_error
        );
        sim.ticks(30);
        assert_agrees(&mut sim, &what);
    }
}

/// A gate closing down onto a client standing under it stops above the
/// player (world_objects.md 6: movers never push) on the server and on
/// the client alike, and the player's prediction stays the server's.
#[test]
fn a_gate_stops_on_a_client_as_on_the_server() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&gate_world(), "gate", s, &|_| None);
    let m = map.clone();
    let mut sim = NetSim::new(CaSim, link(80, 0), 73, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(m.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    put(&mut sim, Vec3::new(0.0, 60.0, 300.0), s);
    sim.ticks(200);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let theirs = sim.character_of(0).unwrap();
    let head = {
        let world = sim.server.app.world();
        let t = world.get::<Transform>(theirs).unwrap().translation;
        let m = world.get::<chimera_core::core::MovementState>(theirs).unwrap();
        from_engine(t + m.hull_max, s).y
    };
    send(&mut sim, "Gate", "on");
    let r = ride(&mut sim, "Gate", 400, 0);
    let server_gate = node_y(&sim.server.app, "Gate", s);
    let client_gate = node_y(&sim.clients[0].app, "Gate", s);
    let g = graph(&sim);
    println!(
        "gate: stopped at {server_gate:.2} u (bottom {:.2}, head at {head:.2}) on the server, {client_gate:.2} u on the \
         client; errors at {:?}, worst gap {:.3} m, {} of {} states wrong",
        server_gate - 75.0,
        r.errors,
        r.worst_gap,
        g.errors,
        g.checked
    );
    // (It stops a whole frame's step, 10 u, short at most.)
    assert!(
        server_gate - 75.0 > head - 1.0,
        "the gate stops above the head on the server"
    );
    assert!(
        server_gate - 75.0 < head + 11.0,
        "the gate came down to the head on the server"
    );
    assert!(
        (client_gate - server_gate).abs() < 0.01,
        "the client's gate stops where the server's does"
    );
    assert_eq!(g.errors, 0, "standing still under a gate isn't mispredicted");
    assert!(r.worst_gap < 1e-4, "the player stays put ({} m)", r.worst_gap);
    assert_agrees(&mut sim, "gate");
}

/// A door 100 u thick (x -400..-300) that slides 400 u along +X in 2 s.
fn slider_world() -> World {
    world_with(
        vec![cuboid("Slider", [-400.0, 0.0, -100.0], [-300.0, 150.0, 100.0], false)],
        vec![Object {
            class: "SlidingWorldModel".into(),
            properties: vec![
                prop("Name", Value::String("Slider".into())),
                prop("Pos", Value::Vector([-350.0, 75.0, 0.0])),
                prop("MoveDir", Value::Vector([1.0, 0.0, 0.0])),
                prop("MoveDist", Value::Real(400.0)),
                prop("PowerOnTime", Value::Real(2.0)),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    )
}

/// A client walking into a door that slides at it: the door stops at the
/// player (the game's test refuses a step into a player, 6) and the
/// player walks into the door where the server has it, both predicted
/// (errors only in the round trip after the door starts).
#[test]
fn a_client_walking_into_a_sliding_door_is_predicted() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&slider_world(), "slider", s, &|_| None);
    for (latency, seed) in [(50, 74), (150, 75)] {
        let m = map.clone();
        let mut sim = NetSim::new(CaSim, link(latency, 0), seed, 1, move |app| {
            app.add_plugins(MapSimPlugin::new(m.clone()));
        });
        sim.set_frame(TICK_INTERVAL);
        sim.until_joined(600);
        sim.until_looks_taken(600);
        sim.ticks(120);
        put(&mut sim, Vec3::new(-100.0, 60.0, 0.0), s);
        sim.ticks(200);
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        let theirs = sim.character_of(0).unwrap();
        let x0 = from_engine(sim.server.app.world().get::<Transform>(theirs).unwrap().translation, s).x;
        let me = sim.local_player(0).unwrap();
        {
            let world = sim.clients[0].app.world_mut();
            let mut i = world.get_mut::<chimera_core::core::Intent>(me).unwrap();
            i.yaw = 0.0;
            i.pitch = 0.0;
            i.move_axis = Vec2::new(-1.0, 0.0);
        }
        send(&mut sim, "Slider", "on");
        let window = ((2 * latency) as f64 / 1000.0 / TICK_INTERVAL) as u32 + 8;
        let r = ride(&mut sim, "Slider", 400, window);
        let x1 = from_engine(sim.server.app.world().get::<Transform>(theirs).unwrap().translation, s).x;
        let server_door = from_engine(
            {
                let o = sim.server.app.world().resource::<CaObjects>();
                let n = *o.nodes.get(&o.named("Slider").unwrap()).unwrap();
                sim.server.app.world().get::<Transform>(n).unwrap().translation
            },
            s,
        )
        .x;
        let client_door = {
            let app = &sim.clients[0].app;
            let o = app.world().resource::<CaObjects>();
            let n = *o.nodes.get(&o.named("Slider").unwrap()).unwrap();
            from_engine(app.world().get::<Transform>(n).unwrap().translation, s).x
        };
        let g = graph(&sim);
        println!(
            "{latency} ms: walked {x0:.1} -> {x1:.1} u, door stopped at {server_door:.1} (client {client_door:.1}); \
             errors at {:?}, worst gap {:.3} m ({:.4} after the start, door {:.4}), {} of {} states wrong",
            r.errors, r.worst_gap, r.gap_after, r.node_gap_after, g.errors, g.checked
        );
        let what = format!("{latency} ms");
        assert!(x1 < x0 - 20.0, "{what}: walked toward the door");
        assert!(
            server_door < -350.0 + 400.0 - 1.0,
            "{what}: the door was stopped by the player"
        );
        assert!(server_door + 50.0 <= x1, "{what}: the player isn't inside the door");
        assert!(
            (client_door - server_door).abs() < 0.01,
            "{what}: the client's door is the server's"
        );
        assert!(
            r.errors.iter().all(|k| *k <= window),
            "{what}: errors after the start ({:?})",
            r.errors
        );
        assert!(
            r.gap_after < 1e-4,
            "{what}: {} m off the server after the start",
            r.gap_after
        );
        assert!(
            r.node_gap_after < 1e-4,
            "{what}: the client's door {} m off",
            r.node_gap_after
        );
        sim.ticks(30);
        assert_agrees(&mut sim, &what);
    }
}

/// A door 200 u wide hinged at x 0 (z 300) that turns 90 degrees of yaw
/// in 1.5 s.
fn swing_world() -> World {
    world_with(
        vec![cuboid("Swing", [0.0, 0.0, 295.0], [200.0, 150.0, 305.0], false)],
        vec![Object {
            class: "RotatingWorldModel".into(),
            properties: vec![
                prop("Name", Value::String("Swing".into())),
                prop("Pos", Value::Vector([0.0, 75.0, 300.0])),
                prop("RotationAngles", Value::Vector([0.0, 90.0, 0.0])),
                prop("PowerOnTime", Value::Real(1.5)),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    )
}

/// A rotating world model swinging into a client standing in its way
/// stops at the player on the server and on the client alike (its turn is
/// stepped on the client too), and the player isn't mispredicted.
#[test]
fn a_swinging_door_stops_on_a_client_as_on_the_server() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&swing_world(), "swing", s, &|_| None);
    let m = map.clone();
    let mut sim = NetSim::new(CaSim, link(100, 0), 76, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(m.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    put(&mut sim, Vec3::new(150.0, 60.0, 240.0), s);
    sim.ticks(200);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    send(&mut sim, "Swing", "on");
    let r = ride(&mut sim, "Swing", 400, 0);
    let turn = |app: &App| {
        let o = app.world().resource::<CaObjects>();
        let n = *o.nodes.get(&o.named("Swing").unwrap()).unwrap();
        app.world().get::<Transform>(n).unwrap().rotation
    };
    let (server, client) = (turn(&sim.server.app), turn(&sim.clients[0].app));
    let g = graph(&sim);
    println!(
        "swing: stopped at {:.2} degrees on the server, {:.2} on the client; errors at {:?}, worst gap {:.4} m, {} of \
         {} states wrong",
        server.to_euler(EulerRot::YXZ).0.to_degrees(),
        client.to_euler(EulerRot::YXZ).0.to_degrees(),
        r.errors,
        r.worst_gap,
        g.errors,
        g.checked
    );
    let swung = server.to_euler(EulerRot::YXZ).0.to_degrees().abs();
    assert!(
        swung > 5.0 && swung < 85.0,
        "the door swung and stopped at the player ({swung})"
    );
    assert!(server.angle_between(client) < 1e-4, "the client's door is the server's");
    assert_eq!(g.errors, 0, "standing in a door's swing isn't mispredicted");
    assert_agrees(&mut sim, "swing");
}

/// A 200 u square platform 10 u high at x 400 (a solid `WorldModel`
/// "Plat") that a time-mode keyframer ("KF", keys `K0`, `K01`, `K02`)
/// lifts 200 u in 2 s, then slides 200 u along +X in 2 s.
fn keyframed_world() -> World {
    let mut w = world_with(
        vec![cuboid("Plat", [300.0, 0.0, -100.0], [500.0, 10.0, 100.0], false)],
        vec![
            Object {
                class: "WorldModel".into(),
                properties: vec![
                    prop("Name", Value::String("Plat".into())),
                    prop("Pos", Value::Vector([400.0, 5.0, 0.0])),
                    prop("Solid", Value::Bool(true)),
                    prop("Rayhit", Value::Bool(true)),
                ],
            },
            Object {
                class: "KeyFramer".into(),
                properties: vec![
                    prop("Name", Value::String("KF".into())),
                    prop("Pos", Value::Vector([0.0, 0.0, 0.0])),
                    prop("ObjectName", Value::String("Plat".into())),
                    prop("BaseKeyName", Value::String("K".into())),
                    prop("KeyDataIndex", Value::Real(0.1)),
                ],
            },
        ],
    );
    // The keys' blind data (world_objects.md 9): no tangents, no sound or
    // command.
    let mut b = Vec::new();
    b.extend(3u32.to_le_bytes());
    for ([x, y, z], stamp) in [([400.0f32, 5.0, 0.0], 0.0f32), ([400.0, 205.0, 0.0], 2.0), ([600.0, 205.0, 0.0], 2.0)] {
        b.extend(0u16.to_le_bytes());
        b.extend([0u8, 0u8]);
        for f in [x, y, z, 0.0, 0.0, 0.0, stamp, 300.0] {
            b.extend(f.to_le_bytes());
        }
    }
    w.blind_data.push(BlindChunk {
        id: combat_arms::game::objects::keyframer::KEY_CHUNK_ID,
        data: b,
    });
    w
}

/// A client standing on a keyframed platform rides it up 200 u and along
/// 200 u, then back: the server carries it (10), and its prediction
/// steps the keyframer as the server does, so it rides where the server
/// has it (errors only in the round trip after each start).
#[test]
fn a_client_rides_a_keyframed_platform_predicted() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&keyframed_world(), "keyframed", s, &|_| None);
    for (latency, jitter, seed) in [(50, 0, 77), (150, 20, 78)] {
        let m = map.clone();
        let mut sim = NetSim::new(CaSim, link(latency, jitter), seed, 1, move |app| {
            app.add_plugins(MapSimPlugin::new(m.clone()));
        });
        sim.set_frame(TICK_INTERVAL);
        sim.until_joined(600);
        sim.ticks(240);
        put(&mut sim, Vec3::new(400.0, 60.0, 0.0), s);
        sim.ticks(200);
        let theirs = sim.character_of(0).unwrap();
        let at = |sim: &NetSim| from_engine(sim.server.app.world().get::<Transform>(theirs).unwrap().translation, s);
        let start = at(&sim);
        assert!(
            (node_y(&sim.clients[0].app, "Plat", s) - 5.0).abs() < 1e-3,
            "the client has the platform down"
        );
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        let window = ((2 * latency + 2 * jitter) as f64 / 1000.0 / TICK_INTERVAL) as u32 + 8;
        send(&mut sim, "KF", "on");
        let there = ride(&mut sim, "Plat", 900, window);
        let top = at(&sim);
        let carried = graph(&sim).carried;
        send(&mut sim, "KF", "backward");
        send(&mut sim, "KF", "on");
        let back = ride(&mut sim, "Plat", 900, window);
        let end = at(&sim);
        let g = graph(&sim);
        println!(
            "keyframed {latency}±{jitter} ms: there {:.1} in {} ticks, errors at {:?}, worst gap {:.3} m ({:.4} after \
             the start, platform {:.4}); back to {:.1} in {} ticks, errors at {:?}, worst gap {:.3} m ({:.4}, \
             platform {:.4}); carried {carried} ticks, {} of {} states wrong, worst correction {:.3} m",
            top - start,
            there.ticks,
            there.errors,
            there.worst_gap,
            there.gap_after,
            there.node_gap_after,
            end - start,
            back.ticks,
            back.errors,
            back.worst_gap,
            back.gap_after,
            back.node_gap_after,
            g.errors,
            g.checked,
            g.worst_error
        );
        let what = format!("keyframed {latency}±{jitter} ms");
        assert!(
            (top - start - Vec3::new(200.0, 200.0, 0.0)).length() < 1.0,
            "{what}: rode up and along {}",
            top - start
        );
        assert!((end - start).length() < 1.0, "{what}: back at {}", end - start);
        // (It faces no target: its `TargetName` isn't its own `Name`.)
        let turn = {
            let o = sim.server.app.world().resource::<CaObjects>();
            o.world.objects[o.named("Plat").unwrap() as usize].pose.rot
        };
        assert!(turn.angle_between(Quat::IDENTITY) < 1e-4, "{what}: the platform didn't turn: {turn:?}");
        assert!(carried > 30, "{what}: carried {carried} ticks");
        for (dir, r) in [("there", &there), ("back", &back)] {
            assert!(r.gap_after < 1e-4, "{what}: {dir}: {} m off the server after the start", r.gap_after);
            assert!(r.node_gap_after < 1e-4, "{what}: {dir}: the client's platform {} m off", r.node_gap_after);
            assert!(
                r.errors.iter().all(|k| *k <= window),
                "{what}: {dir}: errors after the start's round trip ({:?}, window {window})",
                r.errors
            );
            assert!(r.errors.len() <= 4, "{what}: {dir}: {:?}", r.errors);
        }
        sim.ticks(30);
        assert_agrees(&mut sim, &what);
    }
}
