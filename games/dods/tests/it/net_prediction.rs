//! DoD:S's movement over the network (`NetSim` on `DodsSim`, the greybox):
//! a client predicts its player going prone and getting up while moving
//! and sprinting (the stance-change speed cap, `stance::stance_cap`, and
//! the stamina it reads) and running into physics props that turbo
//! physics makes walls (a light `prop_physics`, a settled multiplayer
//! prop), and every predicted tick matches the server's bit for bit: no
//! prediction errors.

use std::time::Duration;

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use chimera_core::core::{ActionRegistry, Intent, PredictedComponents, SimTick};
use chimera_net::{
    harness::NetSim,
    net::{
        memory::LinkConditions,
        predict::{CommandClock, NetGraph, PredictionHistory},
    },
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{MapPropCollider, PhysicsProp, PushAway},
};
use dods::{game::stance, harness::DodsSim};

const U: f32 = 0.0254;

fn joined() -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(100),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(DodsSim, link, 31, 1, |app: &mut App| {
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

/// The same still prop in the server's world and the client's (a map's
/// prop: each side loads its own), `at` its centre.
fn prop(sim: &mut NetSim, at: Vec3, push: PushAway, mass: f32) {
    let half = Vec3::new(24.0, 18.0, 24.0) * U;
    for world in std::iter::once(sim.server.app.world_mut()).chain(sim.clients.iter_mut().map(|c| c.app.world_mut())) {
        world.spawn((
            RigidBody::Static,
            Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z),
            Transform::from_translation(at),
            PhysicsProp { push, mass, bounds: (-half, half) },
            MapPropCollider,
        ));
    }
}

/// Run and sprint (from rest, so the sprint starts), go prone on the
/// way, get up sprinting, prone and up again, then on into the props.
fn script(k: u32, i: &mut Intent, prone: u32) {
    i.yaw = 0.0;
    i.move_axis = Vec2::Y;
    i.sprint = k >= 20;
    i.actions &= !prone;
    if matches!(k, 60 | 180 | 330 | 430) {
        i.actions |= prone;
    }
}

#[test]
fn going_prone_getting_up_and_walls_of_props_are_predicted() {
    let mut sim = joined();
    let theirs = sim.character_of(0).unwrap();
    // From the greybox's first spawn, looking down -Z (52 m to the north
    // wall); then settled.
    let start = chimera_world::greybox::SPAWNS[0];
    {
        let w = sim.server.app.world_mut();
        w.get_mut::<Transform>(theirs).unwrap().translation = start;
        if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(theirs) {
            pos.0 = start;
        }
    }
    *client_intent(&mut sim) = Intent::default();
    sim.ticks(120);
    // Two props 1000 units ahead along the run (-Z): a 10 kg
    // `prop_physics` (a wall under turbo physics) and beside it a settled
    // multiplayer crate (a wall too), the two across the way.
    let at = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    let wall_z = at.z - 1000.0 * U;
    prop(&mut sim, Vec3::new(at.x - 24.0 * U, at.y - 18.0 * U, wall_z), PushAway::Collide, 10.0);
    prop(&mut sim, Vec3::new(at.x + 24.0 * U, at.y - 18.0 * U, wall_z), PushAway::Solid, 30.0);
    let prone = sim.clients[0].app.world().resource::<ActionRegistry>().bit(stance::PRONE_ACTION);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let mut server = std::collections::HashMap::new();
    let mut went_prone = false;
    for k in 0..640 {
        script(k, &mut client_intent(&mut sim), prone);
        sim.step();
        let world = sim.server.app.world();
        let tick = world.resource::<SimTick>().0;
        server.insert(tick, world.resource::<PredictedComponents>().encode(world, theirs));
        went_prone |= world.get::<stance::DodsStance>(theirs).unwrap().prone();
    }
    // Stopped at the props' face (their half depth 24 and the box's 16).
    let world = sim.server.app.world();
    let z = world.get::<Transform>(theirs).unwrap().translation.z;
    let speed = world.get::<chimera_core::core::Velocity>(theirs).unwrap().0.xz().length() / U;
    assert!(((z - wall_z) / U - 40.0).abs() < 1.0 && speed < 1.0, "stopped by the props: {} units from them at {speed}", (z - wall_z) / U);
    assert!(went_prone, "went prone");
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
