//! Joining a server: the lobby first, the loading screen when the game is
//! on (docs/plans/active/lobby-between-maps.md, step 5). The load's state
//! (`chimera_net::net::load_stages`: the download, `LoadWork`) is the one
//! source every screen reads and none owns:
//!
//! - a player joining a server in its lobby lands in the lobby (Ultimate
//!   Gun Game's theme), loading behind it, its own row showing its stage
//!   and share, the match's settings told, a Leave button;
//! - the host starts while it still loads: the lobby goes, the loading
//!   screen is up at once, the same load goes on (not restarted or
//!   duplicated), and the player spawns when it is done;
//! - a player joining a match already in play sees the loading screen
//!   directly, never a lobby.

use bevy::prelude::*;
use chimera_client::client::{
    prematch_lobby::LobbyView,
};
use chimera_core::console::Console;
use chimera_net::{
    harness::NetSim,
    net::{
        client::{Joined, JoinProgress, JoinStage},
        load_stages::{self, LoadStep, LoadWork},
    },
    prematch::{LobbySettings, NetLobby},
};
use chimera_sim::minigame::{InMinigame, Staged};
use chimera_core::core::Dead;
use chimera_world::map::LocalMapLoad;
use party_kit::main_menu::{PartyMenu, Screen, ShownScreen};

use crate::lobby_staging::{hold_join, lobby_owns, lobby_view, release_join, staged_signal_party};

/// Client `i` loading its map (its join held after the welcome): the
/// load's own state as the client shell leaves it while the spawn runs.
fn loading(sim: &mut NetSim, i: usize, step: LoadStep, done: u64, total: u64) {
    let w = sim.clients[i].app.world_mut();
    w.resource_mut::<JoinProgress>().stage = JoinStage::LoadingMap;
    if !w.contains_resource::<LocalMapLoad>() {
        w.insert_resource(LocalMapLoad { map: "chimera:greybox".into() });
    }
    w.insert_resource(LoadWork { step, done, total, colliders: 0 });
    let mut menu = w.resource_mut::<PartyMenu>();
    menu.screen = Screen::Joining;
}

fn own_stage(sim: &NetSim, i: usize) -> Option<(LoadStep, Option<f32>)> {
    load_stages::current(sim.clients[i].app.world())
}

fn loading_screen(sim: &NetSim, i: usize) -> bool {
    matches!(sim.clients[i].app.world().resource::<ShownScreen>().screen, Screen::Joining | Screen::Loading)
}

fn net_lobbies(sim: &mut NetSim, i: usize) -> usize {
    let w = sim.clients[i].app.world_mut();
    w.query::<&NetLobby>().iter(w).count()
}

/// When the load's marker resource was put in (a restart would move it).
fn load_added(sim: &NetSim, i: usize) -> Option<u32> {
    sim.clients[i].app.world().get_resource_ref::<LocalMapLoad>().map(|r| r.added().get())
}

#[test]
fn a_joiner_in_the_lobby_sees_it_loading_with_its_row_and_the_settings() {
    let mut sim = staged_signal_party();
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_balance 0");
    hold_join(&mut sim, 1);
    assert!(sim.until(900, |s| lobby_view(s, 1) && s.character_of(1).is_some()));
    assert!(!sim.clients[1].app.world().contains_resource::<Joined>(), "client 1 still loads");
    lobby_owns(&mut sim, 1).unwrap();
    loading(&mut sim, 1, LoadStep::Models, 5, 10);
    // Its own row shows the load, live, from the load's one source.
    assert!(sim.until(30, |s| {
        s.clients[1].app.world().resource::<LobbyView>().rows.iter().any(|r| r.local && r.step == Some(LoadStep::Models) && r.fraction == Some(0.5))
    }), "{:?}", sim.clients[1].app.world().resource::<LobbyView>().rows);
    sim.clients[1].app.insert_resource(LoadWork { step: LoadStep::Physics, done: 9, total: 10, colliders: 4 });
    assert!(sim.until(30, |s| {
        s.clients[1].app.world().resource::<LobbyView>().rows.iter().any(|r| r.local && r.step == Some(LoadStep::Physics) && r.fraction == Some(0.9))
    }));
    // The other client sees the same row from the server.
    let id1 = sim.client_id(1);
    assert!(sim.until(120, |s| {
        s.clients[0].app.world().resource::<LobbyView>().rows.iter().any(|r| !r.local && r.step == Some(LoadStep::Physics) && r.fraction == Some(0.9))
    }), "{id1}");
    // The settings: the mode's length, then the host's.
    let v = sim.clients[1].app.world().resource::<LobbyView>().clone();
    let get = |k: &str| v.settings.iter().find(|(l, _)| l == k).map(|(_, v)| v.clone());
    assert_eq!(get("Length").as_deref(), Some("5 min"), "{:?}", v.settings);
    assert_eq!(get("Cross-game balance").as_deref(), Some("Raw"), "{:?}", v.settings);
    assert!(!v.mode.is_empty() && !v.map.is_empty());
}

#[test]
fn the_host_starting_while_a_joiner_loads_shows_the_loading_screen_and_the_same_load_goes_on() {
    let mut sim = staged_signal_party();
    hold_join(&mut sim, 1);
    assert!(sim.until(900, |s| lobby_view(s, 1) && s.character_of(1).is_some()));
    loading(&mut sim, 1, LoadStep::Models, 5, 10);
    sim.ticks(10);
    assert!(lobby_view(&sim, 1) && !loading_screen(&sim, 1) || lobby_view(&sim, 1), "the lobby is up over its load");
    let added = load_added(&sim, 1);
    let before = own_stage(&sim, 1);
    assert_eq!(before, Some((LoadStep::Models, Some(0.5))));
    // The host starts (everyone else is on the map; the lobby's end lets
    // the loaded in, not this one).
    sim.server.app.world_mut().resource_mut::<Console>().submit("lobby_start 0");
    assert!(sim.until(300, |s| !lobby_view(s, 1)), "the lobby goes on client 1");
    // The loading screen at once, the load unchanged: not restarted, not
    // duplicated.
    sim.ticks(2);
    assert!(loading_screen(&sim, 1), "{:?}", sim.clients[1].app.world().resource::<ShownScreen>());
    assert_eq!(net_lobbies(&mut sim, 1), 0);
    assert_eq!(load_added(&sim, 1), added, "the load wasn't put in again");
    assert_eq!(own_stage(&sim, 1), before);
    assert_eq!(sim.clients[1].app.world().resource::<LoadWork>().done, 5);
    // The load goes on: more models built, the loading screen's stages
    // read the same state.
    sim.clients[1].app.insert_resource(LoadWork { step: LoadStep::Models, done: 8, total: 10, colliders: 0 });
    sim.ticks(2);
    assert_eq!(own_stage(&sim, 1), Some((LoadStep::Models, Some(0.8))));
    assert!(loading_screen(&sim, 1));
    // Not in play meanwhile; the load done, it spawns.
    let e = sim.character_of(1).unwrap();
    assert!(sim.server.app.world().get::<InMinigame>(e).is_none());
    sim.clients[1].app.world_mut().remove_resource::<LocalMapLoad>();
    sim.clients[1].app.world_mut().remove_resource::<LoadWork>();
    release_join(&mut sim, 1);
    assert!(sim.until(900, |s| s.clients[1].app.world().contains_resource::<Joined>()), "client 1 joined");
    assert!(sim.until(300, |s| {
        let w = s.server.app.world();
        w.get::<InMinigame>(e).is_some() && w.get::<Dead>(e).is_none() && w.get::<Staged>(e).is_none()
    }), "client 1 spawned");
    assert!(sim.until(120, |s| !loading_screen(s, 1)));
}

#[test]
fn a_late_joiner_into_a_running_match_gets_the_loading_screen_directly() {
    let mut sim = staged_signal_party();
    // No lobby on this server: the match is simply in play.
    sim.server.app.world_mut().resource_mut::<LobbySettings>().enabled = 0;
    hold_join(&mut sim, 1);
    assert!(sim.until(900, |s| s.clients[0].app.world().contains_resource::<Joined>() && s.character_of(1).is_some()));
    loading(&mut sim, 1, LoadStep::Map, 0, 0);
    for _ in 0..90 {
        sim.step();
        assert!(!lobby_view(&sim, 1), "no lobby for a match in play");
        assert_eq!(net_lobbies(&mut sim, 1), 0);
    }
    assert!(loading_screen(&sim, 1));
    let added = load_added(&sim, 1);
    sim.clients[1].app.insert_resource(LoadWork { step: LoadStep::Physics, done: 1, total: 4, colliders: 1 });
    sim.ticks(5);
    assert_eq!(own_stage(&sim, 1), Some((LoadStep::Physics, Some(0.25))));
    assert_eq!(load_added(&sim, 1), added);
    let e = sim.character_of(1).unwrap();
    sim.clients[1].app.world_mut().remove_resource::<LocalMapLoad>();
    sim.clients[1].app.world_mut().remove_resource::<LoadWork>();
    release_join(&mut sim, 1);
    assert!(sim.until(900, |s| s.clients[1].app.world().contains_resource::<Joined>()), "client 1 joined");
    assert!(sim.until(300, |s| s.server.app.world().get::<InMinigame>(e).is_some_and(|_| s.server.app.world().get::<Dead>(e).is_none())), "client 1 spawned");
}
