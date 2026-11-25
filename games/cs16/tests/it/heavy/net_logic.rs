//! CS 1.6's map logic reaching a network client (`goldsrc_engine::logic::net`
//! on Chimera's `net::movers`), on the user's install, with a server and a
//! client over `NetSim`'s link: cs_office's `door1` opens on the client
//! tick for tick with the server once the client has heard of it; a
//! player riding as_oilrig's elevator up has no prediction errors after
//! the start's round trip; de_dust2's broken crates are passable and
//! hidden on the client. Passes with a note without an install.

use std::{collections::HashMap, time::Duration};

use avian3d::prelude::{ColliderDisabled, Position};
use bevy::prelude::*;
use chimera_core::core::{MovingSolid, SimClock, SimTick};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_world::map::{MapBrushEntity, MapData, MapSimPlugin, entities::FireEntityOutput};
use cs16::harness::Cs16Sim;
use goldsrc_engine::{
    content::{METERS_PER_UNIT, to_engine},
    logic::{GoldSrcLogic, LogicFire},
};

use super::maps::{convert, mount};

const LATENCY_MS: u64 = 60;

/// A server running `name` with one joined client.
fn net_of(name: &str) -> Option<(NetSim, MapData)> {
    let mount = mount()?;
    let map = convert(&mount, name, false);
    let m = map.clone();
    let link = LinkConditions { latency: Duration::from_millis(LATENCY_MS), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(Cs16Sim, link, 7, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(m.clone()));
    });
    sim.until_joined(1200);
    sim.ticks(60);
    assert!(sim.server.app.world().get_resource::<GoldSrcLogic>().is_some(), "{name}: no logic on the server");
    assert!(sim.clients[0].app.world().get_resource::<GoldSrcLogic>().is_none(), "{name}: logic on the client");
    Some((sim, map))
}

fn model(map: &MapData, model: &str) -> usize {
    map.entities.iter().position(|e| e.get("model") == Some(model)).unwrap_or_else(|| panic!("no {model}"))
}

fn named(map: &MapData, name: &str) -> Vec<usize> {
    (0..map.entities.len()).filter(|&i| map.entities[i].get("targetname") == Some(name)).collect()
}

fn node(world: &mut World, i: usize) -> Entity {
    let mut q = world.query::<(Entity, &MapBrushEntity)>();
    q.iter(world).find(|(_, n)| n.0 == i).map(|(e, _)| e).unwrap_or_else(|| panic!("no node for {i}"))
}

fn at(world: &World, e: Entity) -> Vec3 {
    world.get::<Transform>(e).unwrap().translation
}

fn tick(world: &World) -> u64 {
    world.resource::<SimTick>().0
}

fn graph(sim: &NetSim) -> NetGraph {
    sim.clients[0].app.world().resource::<NetGraph>().clone()
}

/// Ticks a round trip and the snapshot delay take, with room.
fn window(sim: &NetSim) -> u64 {
    let hz = 1.0 / sim.server.app.world().resource::<Time<Fixed>>().timestep().as_secs_f64();
    ((2 * LATENCY_MS) as f64 / 1000.0 * hz) as u64 + 12
}

#[test]
fn cs_office_door1_opens_on_the_client_with_the_server() {
    let Some((mut sim, map)) = net_of("cs_office") else { return };
    let door = model(&map, "*200");
    assert_eq!(named(&map, "door1"), vec![door]);
    let (sn, cn) = (node(sim.server.app.world_mut(), door), node(sim.clients[0].app.world_mut(), door));
    let closed = at(sim.server.app.world(), sn);
    assert_eq!(at(sim.clients[0].app.world(), cn), closed, "the client has door1 where the server does");
    let (lo, hi) = sim.server.app.world().resource::<GoldSrcLogic>().world.bounds(door);
    println!("door1 bounds {lo} {hi}");
    sim.server.app.world_mut().write_message(LogicFire("door1".into()));
    // The door rises 108 units in 2.7 s: both sides per tick until well
    // after it stops.
    let (mut server, mut client) = (HashMap::new(), Vec::new());
    let mut fired = None;
    for _ in 0..600 {
        sim.step();
        let s = sim.server.app.world();
        server.insert(tick(s), at(s, sn).y);
        if fired.is_none() && at(s, sn) != closed {
            fired = Some(tick(s));
        }
        let c = sim.clients[0].app.world();
        client.push((c.resource::<SimClock>().tick, at(c, cn).y));
    }
    let fired = fired.expect("the server's door never moved");
    let open = closed.y + 108.0 * METERS_PER_UNIT;
    assert!((server[&(fired + 400)] - open).abs() < 1e-3, "server door at {}", server[&(fired + 400)]);
    // The client's door at each tick it ran is the server's at that tick,
    // but for the ticks before it heard the door start (a round trip).
    let wrong: Vec<u64> = client
        .iter()
        .filter(|(t, y)| server.get(t).is_some_and(|s| (s - y).abs() > 1e-4))
        .map(|(t, _)| *t)
        .collect();
    println!("door started at server tick {fired}; client off it at ticks {wrong:?}");
    assert!(wrong.iter().all(|t| *t < fired + window(&sim)), "client off the server's door after the start: {wrong:?}");
    assert!(client.iter().any(|(t, y)| server.contains_key(t) && (y - open).abs() < 1e-3), "the client's door opened");
    assert!((client.last().unwrap().1 - open).abs() < 1e-3);
}

#[test]
fn riding_as_oilrig_elevator_has_no_prediction_errors() {
    let Some((mut sim, map)) = net_of("as_oilrig") else { return };
    let lift = model(&map, "*22");
    assert_eq!(named(&map, "elevator"), vec![lift]);
    // Stand the client's player on the elevator's top, in its middle.
    let (lo, hi) = sim.server.app.world().resource::<GoldSrcLogic>().world.bounds(lift);
    let top = Vec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, hi.z + 37.0);
    let p = sim.character_of(0).unwrap();
    let to = to_engine(top.to_array());
    {
        let w = sim.server.app.world_mut();
        w.get_mut::<Transform>(p).unwrap().translation = to;
        if let Some(mut q) = w.get_mut::<Position>(p) {
            q.0 = to;
        }
    }
    sim.ticks(200);
    let before = at(sim.server.app.world(), p);
    assert!(
        (before.x - to.x).abs() < 0.05 && (before.z - to.z).abs() < 0.05 && (before.y - to.y).abs() < 1.0,
        "the player didn't stay on the elevator: {before} vs {to}"
    );
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let sn = node(sim.server.app.world_mut(), lift);
    let lift0 = at(sim.server.app.world(), sn);
    sim.server.app.world_mut().write_message(LogicFire("elevator".into()));
    // Up 505 units in 5.05 s, then 5 s at the top: ride it up and stop
    // half a second into the wait (its start down mispredicts as its
    // start up does).
    let mut errors = Vec::new();
    let mut last = 0;
    let mut started = None;
    let mut stopped = None;
    let (mut prev, mut still) = (lift0, 0);
    for k in 0..1500u64 {
        sim.step();
        let g = graph(&sim);
        if g.errors > last {
            errors.push(k);
            last = g.errors;
        }
        let now = at(sim.server.app.world(), sn);
        if started.is_none() && now != lift0 {
            started = Some(k);
        }
        still = if now == prev { still + 1 } else { 0 };
        if started.is_some() && still == 1 && stopped.is_none() {
            stopped = Some(k - 1);
        }
        prev = now;
        if started.is_some() && still > 50 {
            break;
        }
    }
    let rose = at(sim.server.app.world(), sn).y - lift0.y;
    let carried = at(sim.server.app.world(), p).y - before.y;
    let g = graph(&sim);
    println!(
        "elevator rose {:.2} m, player {:.2} m; started at {started:?}; errors at {errors:?}; carried {} ticks, {} of {} states wrong",
        rose, carried, g.carried, g.errors, g.checked
    );
    let started = started.expect("the elevator never moved");
    assert!((rose - 505.0 * METERS_PER_UNIT).abs() < 0.01, "rose {rose}");
    assert!((carried - rose).abs() < 0.05, "the player rode {carried} of {rose}");
    assert!(g.carried > 100, "the client carried its player {} ticks", g.carried);
    // Its stop mispredicts too on CS 1.6's own movement (the rider is put
    // on its ground probe's end each tick, against where the client has
    // the lift on the tick it arrives): one state within the stop's round
    // trip, none during the ride.
    let stopped = stopped.expect("the elevator never stopped");
    let w = window(&sim);
    assert!(
        errors.iter().all(|k| *k <= started + w || (*k >= stopped && *k <= stopped + w)),
        "prediction errors outside the start's and the stop's round trips (started {started}, stopped {stopped}, window {w}): {errors:?}"
    );
    assert!(errors.len() <= 3, "{errors:?}");
}

#[test]
fn de_dust2_broken_crates_are_passable_on_the_client() {
    let Some((mut sim, map)) = net_of("de_dust2") else { return };
    let a = named(&map, "tgt_a");
    let target = model(&map, "*42");
    for &i in &a {
        let n = node(sim.clients[0].app.world_mut(), i);
        let w = sim.clients[0].app.world();
        assert!(w.get::<MovingSolid>(n).is_some_and(|m| m.solid), "crate {i} solid on the client before");
        assert!(w.get::<ColliderDisabled>(n).is_none());
    }
    sim.server.app.world_mut().write_message(FireEntityOutput { map_index: target, output: "BombExplode".into(), activator: None });
    sim.ticks(window(&sim) + 20);
    for &i in &a {
        assert!(sim.server.app.world().resource::<GoldSrcLogic>().world.ents[i].removed, "crate {i} not broken");
        let n = node(sim.clients[0].app.world_mut(), i);
        let w = sim.clients[0].app.world();
        assert!(w.get::<MovingSolid>(n).is_some_and(|m| !m.solid && m.brushes.is_empty()), "crate {i} solid on the client");
        assert!(w.get::<ColliderDisabled>(n).is_some(), "crate {i}'s collider on the client");
        assert_eq!(w.get::<Visibility>(n), Some(&Visibility::Hidden), "crate {i} drawn on the client");
    }
}
