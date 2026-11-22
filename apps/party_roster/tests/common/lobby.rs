//! The party over the network: a server and clients on `NetSim`'s link.

#![allow(dead_code, unused_imports)]

use crate::play::*;
use std::time::Duration;
use bevy::prelude::*;
use chimera_core::{console::Console, core::Score};
use chimera_net::{
    harness::NetSim,
    minigame_net::PartyOffer,
    net::{
        LastDisconnect, NetHooks,
        client::{JoinFailure, JoinProgress, Joined},
        manifest::{GameMount, LocalMounts, MountReport, ServerManifest},
        maps::sha256,
        memory::LinkConditions,
    },
};
use chimera_sim::{
    minigame::{self, ActiveMinigame, MinigameAppExt, Minigames, Modifier, ladder::{Ladder, Rung}},
    testkit,
};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

/// A short ladder of two CS:S guns on the greybox, which only the server
/// has (a test's own: `minigame_load` on a real server).
pub const SHORT: &str = r#"
id = "test:short_ladder"
name = "Short ladder"
players = { min = 2, max = 8 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "5m"

[slots]
weapons = ["cs_source"]

[spawn]
respawn = { delay = "1s", protection = "0s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["cs_source:weapon_deagle", "cs_source:weapon_ak47"]
"#;

/// A server playing `minigame` (none: the gun game) and `clients` clients,
/// not yet joined.
pub fn party(clients: usize, minigame: Option<&str>) -> NetSim {
    party_and(clients, minigame, |_| {})
}

/// `party` with `extra` added to every app (the server's and the
/// clients': a client shell plugin under test).
pub fn party_and(clients: usize, minigame: Option<&str>, extra: fn(&mut App)) -> NetSim {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        11,
        clients,
        move |app: &mut App| {
            setup(app);
            extra(app);
        },
    );
    sim.set_frame(TICK_INTERVAL);
    let id = match minigame {
        Some(text) => minigame::add(sim.server.app.world_mut(), "test", text).expect("the test minigame reads").id.clone(),
        None => DEFAULT_MINIGAME.to_string(),
    };
    minigame::start(sim.server.app.world_mut(), &id).expect("the minigame starts");
    sim
}

/// A mount doctor's report: these games installed, the others not.
pub fn mounts(installed: &[&str]) -> LocalMounts {
    LocalMounts(MountReport {
        games: ["cs_source", "combat_arms"]
            .iter()
            .map(|g| GameMount {
                game: g.to_string(),
                installed: installed.contains(g),
                build: None,
                notes: Vec::new(),
            })
            .collect(),
    })
}

/// Why client `i`'s game ended, and its kind.
pub fn failure(sim: &NetSim, i: usize) -> Option<(JoinFailure, String)> {
    sim.clients[i].app.world().resource::<JoinProgress>().failure.clone()
}

pub(crate) fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
}
