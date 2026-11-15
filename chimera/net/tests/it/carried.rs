//! A character carried by something of the server's (`core::Carried`: a
//! vehicle's seat in a game) on the engine's base alone (`SimBasePlugins`,
//! the greybox, the placeholder movement): its own movement is suspended
//! (walking moves it nowhere on its own), the server's carrier moves it,
//! and the client predicting it takes each state as it comes (no
//! prediction error, `NetGraph::taken_carried`); set down, it walks and
//! is predicted again without error.

use std::time::Duration;

use avian3d::prelude::Position;
use bevy::prelude::*;
use chimera_core::{
    core::{Carried, Intent, SimSet, authoritative},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

/// The server's carrier: every carried character slides along +X at 3 m/s.
fn carrier(mut q: Query<(&mut Transform, Option<&mut Position>), With<Carried>>, time: Res<Time>) {
    for (mut t, p) in &mut q {
        t.translation.x += 3.0 * time.delta_secs();
        if let Some(mut p) = p {
            p.0 = t.translation;
        }
    }
}

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin)
        .insert_resource(Loadout {
            movement: placeholder::ID,
        })
        .add_systems(FixedUpdate, carrier.after(SimSet::Movement).run_if(authoritative));
}

#[test]
fn a_carried_player_is_not_predicted_and_walks_again_when_set_down() {
    let link = LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(SimBasePlugins, link, 41, 1, setup);
    sim.until_joined(600);
    sim.ticks(320);
    let theirs = sim.character_of(0).unwrap();
    let me = sim.local_player(0).unwrap();
    // Picked up on both sides (a game replicates its seat; here by hand).
    let anchor = sim.server.app.world_mut().spawn_empty().id();
    sim.server.app.world_mut().entity_mut(theirs).insert(Carried(anchor));
    chimera_core::core::rebase(sim.server.app.world_mut(), theirs);
    let anchor_here = sim.clients[0].app.world_mut().spawn_empty().id();
    sim.clients[0]
        .app
        .world_mut()
        .entity_mut(me)
        .insert(Carried(anchor_here));
    sim.ticks(10);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let start = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    // Walking sideways (along Z): the movement is suspended, only the
    // carrier moves it (along X).
    for _ in 0..64 {
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().move_axis = Vec2::Y;
        sim.step();
    }
    let end = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    eprintln!(
        "carried {:?}; {} checked, {} errors, {} taken as they came",
        end - start,
        g.checked,
        g.errors,
        g.taken_carried
    );
    assert!((end.x - start.x) > 2.0, "the carrier moved it {} m", end.x - start.x);
    assert!(
        (end.z - start.z).abs() < 1e-3,
        "its own movement moved it {} m",
        end.z - start.z
    );
    assert_eq!(
        g.errors, 0,
        "carried, its prediction fought the server ({:?})",
        g.last_differing
    );
    assert!(
        g.taken_carried > 10,
        "the server's states taken as they came: {}",
        g.taken_carried
    );
    // Set down: walking again, predicted without error.
    sim.server.app.world_mut().entity_mut(theirs).remove::<Carried>();
    chimera_core::core::rebase(sim.server.app.world_mut(), theirs);
    sim.clients[0].app.world_mut().entity_mut(me).remove::<Carried>();
    sim.ticks(20);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let start = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    for _ in 0..64 {
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().move_axis = Vec2::Y;
        sim.step();
    }
    let walked = sim
        .server
        .app
        .world()
        .get::<Transform>(theirs)
        .unwrap()
        .translation
        .distance(start);
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    assert!(walked > 2.0, "walked {walked} m set down");
    assert_eq!(g.errors, 0, "set down, mispredicted ({:?})", g.last_differing);
    assert!(g.checked > 10, "predicted again: {} compared", g.checked);
}
