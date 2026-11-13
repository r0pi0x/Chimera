//! Play starts for a player only once that player's own map load is done
//! (`map::LocalMapLoad`: from the load's start through its spawn and the
//! settling behind the loading screen; playtest 2026-11: a listen host
//! heard the round start under its loading panel). On `NetSim` with the
//! engine's base, the greybox and the test kit's guns, the pre-match lobby
//! off (a listen host's default):
//!
//! - a host's next match on another map waits for the host's own load,
//!   not only for the map to be in the world (spawned, still settling);
//! - a client's report that it has the map (`server::ClientMap`, which the
//!   next match waits for) comes once its own load is done, so the match
//!   doesn't start under its loading screen either.
//!
//! Ticks only (64 a second).

use std::time::Duration;

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::MapLoads,
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    minigame_net::{self, MapChange, MinigameNetPlugin},
    net::{NetEvent, client::Joined, maps::map_name, memory::LinkConditions, server::ClientMap},
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{self, InMinigame, MinigameAppExt, MinigamePlugin},
    mode::{Match, Phase},
    movement::placeholder,
    testkit,
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{LoadedMapName, LocalMapLoad, MapFile},
};

const DEF: &str = r#"
id = "kit:local_load"
name = "Kit gun game"
players = { min = 2, max = 4 }
maps = ["chimera:greybox", "test:next"]
teams = "free_for_all"
time_limit = "5m"
rotate = false

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
"#;

const ID: &str = "kit:local_load";
/// Ticks a second.
const HZ: u64 = 64;
/// Frames a fake load takes to bring its map in.
const LOAD_FRAMES: u32 = 30;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout {
        movement: placeholder::ID,
    });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_local_load.toml", DEF)]).expect("the definition reads");
}

fn sim() -> NetSim {
    NetSim::new(
        SimBasePlugins,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        11,
        2,
        setup,
    )
}

/// A test map loading here, as the shell's (`client::console`): its id,
/// the frames until it is in, then the frames it settles (`settle`) with
/// `LocalMapLoad` still on.
#[derive(Resource)]
struct FakeLoad {
    map: String,
    left: u32,
    settle: u32,
}

fn loaded(world: &mut World, map: &str) {
    world.insert_resource(LoadedMapName(map.to_string()));
    world.insert_resource(MapFile {
        name: map_name(map).to_string(),
        hash: None,
    });
}

/// The settling frames a load here takes (`FakeLoad::settle`).
#[derive(Resource, Clone, Copy)]
struct SettleFrames(u32);

/// A fake load's frame: the map in after `left` frames (a fresh game; a
/// client counts the load), then `settle` frames before the local load
/// is done.
fn step_load(world: &mut World, client: bool) {
    let Some(mut load) = world.remove_resource::<FakeLoad>() else { return };
    if load.left > 0 {
        load.left -= 1;
        if load.left == 0 {
            loaded(world, &load.map);
            if client {
                world.get_resource_or_init::<MapLoads>().0 += 1;
            } else {
                chimera_core::gameplay::new_game(world);
            }
        }
        world.insert_resource(load);
        return;
    }
    if load.settle > 0 {
        load.settle -= 1;
        world.insert_resource(load);
        return;
    }
    world.remove_resource::<LocalMapLoad>();
}

fn start_load(world: &mut World, map: &str) {
    let settle = world.get_resource::<SettleFrames>().map_or(0, |s| s.0);
    world.insert_resource(LoadedMapName(map.to_string()));
    world.insert_resource(LocalMapLoad { map: map.to_string() });
    world.insert_resource(FakeLoad {
        map: map.to_string(),
        left: LOAD_FRAMES,
        settle,
    });
}

fn server_loads(world: &mut World) {
    step_load(world, false);
}

/// A client loads what the server asks for (its join's `LoadMap`).
fn client_loads(world: &mut World, mut cursor: Local<MessageCursor<NetEvent>>) {
    let asked: Vec<String> = cursor
        .read(world.resource::<Messages<NetEvent>>())
        .filter_map(|e| match e {
            NetEvent::LoadMap { map, .. } => Some(map.clone()),
            _ => None,
        })
        .collect();
    if let Some(map) = asked.into_iter().last() {
        start_load(world, &map);
    }
    step_load(world, true);
}

/// Server and clients with their fake loads; the server's `changelevel`
/// is a fake load settling `host_settle` frames, each client's
/// `client_settle` frames.
fn with_loads(host_settle: u32, client_settle: u32) -> NetSim {
    let mut sim = sim();
    sim.server.app.insert_resource(SettleFrames(host_settle)).add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        start_load(w, &map);
        Ok(None)
    });
    for c in &mut sim.clients {
        c.app.insert_resource(SettleFrames(client_settle)).add_systems(Update, client_loads);
    }
    sim
}

fn joined(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().contains_resource::<Joined>()
}

/// The match on the server: live with `n` players in it.
fn playing(sim: &mut NetSim, n: usize) -> bool {
    let w = sim.server.app.world_mut();
    let players = w.query_filtered::<(), With<InMinigame>>().iter(w).count();
    players == n && w.resource::<Match>().phase == Phase::Live
}

/// Matches started on the server (`Minigames::starts`).
fn starts(sim: &NetSim) -> u64 {
    sim.server.app.world().resource::<minigame::Minigames>().starts
}

fn changing(sim: &NetSim) -> bool {
    sim.server.app.world().contains_resource::<MapChange>()
}

/// Both in and the first match playing, then the next match on
/// `test:next`.
fn to_next_map(sim: &mut NetSim) {
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    assert!(sim.until(900, |s| joined(s, 0) && joined(s, 1)), "both in");
    assert!(sim.until(8 * HZ, |s| playing(s, 2)), "the first match plays");
    minigame_net::change_match(sim.server.app.world_mut(), ID, Some("test:next")).expect("the change");
    assert!(changing(sim));
}

/// A listen host whose map is in (spawned) but still settling behind its
/// loading screen: its next match waits; it starts once the host's load is
/// done.
#[test]
fn a_hosts_next_match_waits_for_its_own_load() {
    let mut sim = with_loads(3 * HZ as u32, 0);
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby 0");
    to_next_map(&mut sim);
    // The map comes in here; the clients have it soon after.
    assert!(
        sim.until(600, |s| s.server.app.world().contains_resource::<MapFile>()
            && s.server.app.world().resource::<MapFile>().name == "next"),
        "the map is in on the server"
    );
    let clients_have = |s: &mut NetSim| {
        let w = s.server.app.world_mut();
        let n = w.query::<&ClientMap>().iter(w).filter(|m| m.0 == "test:next").count();
        n == 2
    };
    assert!(sim.until(600, clients_have), "both clients have it");
    // Still settling here: no match.
    let before = starts(&sim);
    while sim.server.app.world().contains_resource::<LocalMapLoad>() {
        assert!(changing(&sim), "the next match waits for the host's own load");
        assert_eq!(starts(&sim), before, "no match starts under the host's loading screen");
        sim.ticks(1);
    }
    assert!(sim.until(HZ, |s| !changing(s)), "the host's load done: the match starts");
    assert!(starts(&sim) > before);
    assert!(sim.until(2 * HZ, |s| playing(s, 2)), "and plays");
}

/// A client whose map is in but still settling: it hasn't reported the
/// map loaded (the server's `ClientMap`), so the next match waits for it;
/// the report, its join and the match come once its load is done.
#[test]
fn a_clients_report_waits_for_its_own_load() {
    let mut sim = with_loads(0, 3 * HZ as u32);
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby 0");
    to_next_map(&mut sim);
    let has = |s: &mut NetSim| {
        let w = s.server.app.world_mut();
        w.query::<&ClientMap>().iter(w).filter(|m| m.0 == "test:next").count()
    };
    // The clients' maps come in, then settle.
    let settling = |s: &NetSim, i: usize| {
        let w = s.clients[i].app.world();
        w.contains_resource::<LocalMapLoad>()
            && w.get_resource::<MapFile>().is_some_and(|f| f.name == "next")
    };
    assert!(sim.until(600, |s| settling(s, 0) && settling(s, 1)), "both clients settle the map");
    while settling(&sim, 0) || settling(&sim, 1) {
        assert!(
            !(settling(&sim, 0) && settling(&sim, 1)) || has(&mut sim) == 0,
            "no client reports the map before its load is done"
        );
        assert!(changing(&sim), "the next match waits");
        sim.ticks(1);
    }
    assert!(sim.until(HZ, |s| has(s) == 2), "both report it once done");
    assert!(sim.until(HZ, |s| !changing(s)), "the match starts");
    assert!(sim.until(2 * HZ, |s| playing(s, 2)), "and plays");
}
