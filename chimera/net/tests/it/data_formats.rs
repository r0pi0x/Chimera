//! A client whose build reads a game's data in another format than the
//! server's (a weapon hook's parameters with a field more, as at the
//! 2026-11-12 rehearsal: Combat Arms' `view_model` with `hidden_pieces`)
//! is refused at the join with the data's name, before it gets a map or
//! data (`net::formats`); the same build joins.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bevy::prelude::*;
use chimera_core::{core::NetRole, slots::Loadout};
use chimera_net::{
    harness::NetSim,
    net::{
        client::{JoinFailure, JoinProgress, Joined},
        formats::FORMAT,
        memory::LinkConditions,
        server::Player,
    },
};
use chimera_sim::{SimBasePlugins, movement::placeholder, weapon::data::WeaponDataAppExt};
use chimera_world::greybox::GreyboxMapPlugin;
use serde::Deserialize;

const HOOK: &str = "a_game:view_model";

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldView {
    #[serde(default)]
    pieces: Vec<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewView {
    #[serde(default)]
    pieces: Vec<String>,
    #[serde(default)]
    hidden_pieces: Vec<String>,
}

/// A server and clients; the apps built after the first `same` register
/// the hook with the old parameters, the server with the new.
fn sim(same: usize) -> NetSim {
    let built = Arc::new(AtomicUsize::new(0));
    let link = LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    NetSim::new(SimBasePlugins, link, 5, 0, move |app| {
        app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout {
            movement: placeholder::ID,
        });
        if built.fetch_add(1, Ordering::SeqCst) < same {
            app.register_weapon_hook::<NewView>(HOOK, |_, _, _| {});
        } else {
            app.register_weapon_hook::<OldView>(HOOK, |_, _, _| {});
        }
    })
}

#[test]
fn a_client_reading_another_data_format_is_refused_at_the_join() {
    // The server (built first) alone has the new format.
    let mut sim = sim(1);
    sim.add_client(|_| {});
    assert!(
        sim.until(300, |s| s.clients[0]
            .app
            .world()
            .resource::<JoinProgress>()
            .failure
            .is_some()),
        "refused"
    );
    let (kind, reason) = sim.clients[0]
        .app
        .world()
        .resource::<JoinProgress>()
        .failure
        .clone()
        .unwrap();
    assert_eq!(kind, JoinFailure::Protocol, "{reason}");
    assert!(reason.starts_with(FORMAT) && reason.contains(HOOK), "{reason}");
    let client = sim.clients[0].app.world();
    assert!(!client.contains_resource::<Joined>());
    assert_eq!(*client.resource::<NetRole>(), NetRole::Standalone);
    let w = sim.server.app.world_mut();
    assert_eq!(w.query::<&Player>().iter(w).count(), 0, "no player made");
}

#[test]
fn the_same_build_joins() {
    let mut sim = sim(usize::MAX);
    sim.add_client(|_| {});
    sim.until_joined(600);
}
