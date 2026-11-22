//! The sandbox entered from a match (`sandbox.rs`, its definition's
//! `lobby = false`; `chimera_net::prematch::lobby_on`,
//! `minigame_net::change_match`): straight onto the range, with no
//! match-end screen, no pre-match lobby, no download and no gun given
//! before the player is in; and its server names the match's sets only
//! when what they are made of changes (`net::server_assets::MatchInputs`:
//! every second, it froze a sandbox host's frame for ~100 ms, the
//! 2026-11-19 report's "Update watch" spikes). A server with one client on
//! the in-memory link; the maps load as the test's `changelevel` and the
//! client's load say (code-built maps, a few frames each).

use std::collections::BTreeSet;

use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::{MapLoads, SimClock},
    metrics::work,
};
use chimera_net::{
    harness::NetSim,
    minigame_net,
    net::{
        NetEvent,
        asset_startup::{AssetSettings, RemoteConfig, Store},
        client::{JoinProgress, JoinStage},
        memory::LinkConditions,
        server_assets::{self, ServerFiles, Via},
    },
    prematch::NetLobby,
};
use chimera_sim::{
    minigame::{ActiveMinigame, MatchResults, NextMatch, PreMatch},
    weapon::{Inventory, Weapon},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    map::{LoadedMapName, MapFile},
};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin, sandbox::SANDBOX};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()));
}

/// A map load `frames` frames from now (the test's `changelevel`, a
/// client's load of what its server asks for).
#[derive(Resource)]
struct FakeLoad(String, u32);

fn finish_load(world: &mut World, server: bool) {
    let Some(FakeLoad(map, left)) = world.remove_resource::<FakeLoad>() else { return };
    if left > 0 {
        world.insert_resource(FakeLoad(map, left - 1));
        return;
    }
    let id = chimera_net::net::normalize_map(&map);
    greybox::unload(world);
    greybox::select(world, &id);
    greybox::respawn(world);
    world.insert_resource(LoadedMapName(map));
    world.insert_resource(MapFile { name: id, hash: None });
    if server {
        chimera_core::gameplay::new_game(world);
    } else {
        world.get_resource_or_init::<MapLoads>().0 += 1;
    }
}

fn server_loads(world: &mut World) {
    finish_load(world, true);
}

fn client_loads(world: &mut World, mut cursor: Local<MessageCursor<NetEvent>>) {
    let asked: Vec<String> = cursor
        .read(world.resource::<Messages<NetEvent>>())
        .filter_map(|e| match e {
            NetEvent::LoadMap { map, .. } => Some(map.clone()),
            _ => None,
        })
        .collect();
    if let Some(map) = asked.into_iter().last() {
        world.insert_resource(FakeLoad(map, 30));
    }
    finish_load(world, false);
}

/// A server and one client (`clients`), the server's `changelevel` the
/// test's.
fn sim(clients: usize) -> NetSim {
    let mut sim = NetSim::new(CaSim, LinkConditions::default(), 5, clients, setup);
    sim.set_frame(TICK_INTERVAL);
    sim.server.app.add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        w.insert_resource(LoadedMapName(map.clone()));
        w.insert_resource(FakeLoad(map, 20));
        Ok(None)
    });
    for c in &mut sim.clients {
        c.app.add_systems(Update, client_loads);
    }
    sim
}

fn submit(sim: &mut NetSim, line: &str) {
    sim.server.app.world_mut().resource_mut::<Console>().submit(line);
}

/// The weapons characters hold (by id and entity).
fn held(w: &mut World) -> BTreeSet<String> {
    let inv: Vec<Entity> = w.query::<&Inventory>().iter(w).flat_map(|i| i.weapons.clone()).collect();
    inv.into_iter().filter_map(|e| w.get::<Weapon>(e).map(|w| format!("{}#{e:?}", w.id))).collect()
}

fn playing(w: &World, id: &str) -> bool {
    w.get_resource::<ActiveMinigame>().is_some_and(|a| a.def.id == id)
}

#[test]
fn the_sandbox_from_a_finished_match_is_entered_at_once() {
    let mut sim = sim(1);
    // A host's match as the party apps' Play page makes it: the lobby on.
    submit(&mut sim, "mp_lobby 1; mp_lobby_min_players 1; MaxPlayer 0");
    minigame_net::change_match(sim.server.app.world_mut(), MINIGAME, Some("greybox")).unwrap();
    sim.ticks(200);
    submit(&mut sim, "lobby_start 0");
    assert!(sim.until(2000, |s| !s.server.app.world().contains_resource::<PreMatch>()), "the match plays");
    sim.ticks(100);
    // The match decided: its end screen up, a next match counted down.
    let w = sim.server.app.world_mut();
    let now = w.resource::<SimClock>().now;
    w.insert_resource(MatchResults::default());
    w.insert_resource(NextMatch { map: Some("greybox".into()), at: now + 10.0 });
    sim.ticks(20);
    let end_screen = |w: &World| w.contains_resource::<MatchResults>() && w.contains_resource::<NextMatch>();
    assert!(end_screen(sim.clients[0].app.world()), "the end screen is up");
    assert!(sim.clients[0].app.world().contains_resource::<chimera_net::net::client::Joined>(), "the player is in");

    submit(&mut sim, "sandbox range");
    let mut before = held(sim.clients[0].app.world_mut());
    let mut entered = None;
    for t in 0..1200 {
        sim.ticks(1);
        assert!(!sim.server.app.world().contains_resource::<PreMatch>(), "tick {t}: no pre-match lobby on the server");
        // A listen host's own screen is the server's world: gone at once
        // (it stayed up until the sandbox started, behind the lobby).
        assert!(!end_screen(sim.server.app.world()), "tick {t}: the server's end screen gone at once");
        let c = sim.clients[0].app.world_mut();
        assert_eq!(c.query::<&NetLobby>().iter(c).count(), 0, "tick {t}: no lobby shown");
        // A client's: as soon as the server's match says so.
        if t >= 30 {
            assert!(!end_screen(c), "tick {t}: no end screen through the change");
        }
        let stage = c.get_resource::<JoinProgress>().map(|p| p.stage);
        assert_ne!(stage, Some(JoinStage::Downloading), "tick {t}: no download for the range");
        let now = held(c);
        let given: Vec<String> = now.difference(&before).cloned().collect();
        let on_range = chimera_net::net::current_map(c) == "range" && playing(c, SANDBOX);
        if !given.is_empty() && entered.is_none() {
            assert!(on_range, "tick {t}: {given:?} given before the player is on the range in the sandbox");
            entered = Some(t);
        }
        before = now;
        if entered.is_some() {
            break;
        }
    }
    let t = entered.expect("in the sandbox with its first gun");
    // The range is in after the server's load (20 frames) and the
    // client's (30): the sandbox starts at once, no lobby's countdown.
    assert!(t < 200, "in after {t} ticks");
    assert!(playing(sim.server.app.world(), SANDBOX));
}

/// The server's own game files as a listen host with an account has them
/// (`ServerFiles`), every game read from the store; the service is
/// nowhere (its fetches fail at once), the store a scratch one.
fn server_files(world: &mut World, dir: &std::path::Path) -> BTreeSet<String> {
    let games: BTreeSet<String> =
        world.resource::<chimera_core::game_registry::Games>().all().map(|g| g.id.to_string()).collect();
    world.insert_resource(ServerFiles {
        settings: AssetSettings {
            store: Some(Store::new(dir.to_path_buf())),
            use_install: false,
            service_first: false,
            builds: Default::default(),
            remote: Some(RemoteConfig { url: "http://127.0.0.1:9".into(), token: "none".into(), key: None, fingerprint: None }),
            lock: None,
        },
        served: games.clone(),
        via: Via::Config,
    });
    games
}

#[test]
fn a_sandbox_server_names_its_matchs_sets_only_when_they_change() {
    let mut sim = sim(0);
    let dir = std::env::temp_dir().join(format!("ugg_sandbox_sets_{}", std::process::id()));
    let games = server_files(sim.server.app.world_mut(), &dir);
    submit(&mut sim, "sandbox range");
    assert!(
        sim.until(600, |s| playing(s.server.app.world(), SANDBOX) && chimera_net::net::current_map(s.server.app.world()) == "range"),
        "the sandbox plays on the range"
    );
    sim.ticks(60);
    // One look's cost (the measure in docs/performance.md, "The sandbox's
    // jitter"): every game's guns, the parts' names of them.
    if work::now().is_some() {
        let w = sim.server.app.world_mut();
        let a = work::now().unwrap();
        let wanted = server_assets::match_wanted(w, &games);
        let b = work::now().unwrap();
        eprintln!(
            "a look at the sandbox's sets: {:.1} M instructions ({} games, {} sets)",
            (b - a) as f64 / 1e6,
            wanted.len(),
            wanted.iter().map(|g| g.sets.len()).sum::<usize>()
        );
    }
    // Nothing changes: the sets aren't named again (they were every
    // second).
    let looks = server_assets::match_looks(sim.server.app.world());
    assert!(looks > 0, "the match's sets were named");
    sim.ticks(600);
    assert_eq!(server_assets::match_looks(sim.server.app.world()), looks, "named again with nothing changed");
    // A change of what they are made of (the map): named again, once.
    submit(&mut sim, "sandbox greybox");
    assert!(sim.until(600, |s| chimera_net::net::current_map(s.server.app.world()) == "greybox" && playing(s.server.app.world(), SANDBOX)));
    sim.ticks(60);
    let after = server_assets::match_looks(sim.server.app.world());
    assert!(after > looks, "named again for the new map");
    sim.ticks(300);
    assert_eq!(server_assets::match_looks(sim.server.app.world()), after, "and not again after");
    let _ = std::fs::remove_dir_all(dir);
}
