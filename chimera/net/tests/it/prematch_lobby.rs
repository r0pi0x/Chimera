//! The pre-match lobby (`chimera_net::prematch`) on `NetSim`, the engine's
//! base, the greybox and the test kit's guns, with `mp_lobby 1`: a new
//! match waits on its map while a client still downloads (its join held
//! after the welcome, its `JoinProgress` showing bytes, as the asset
//! fetch leaves it), every client hears each player's status and bytes,
//! then once everyone is ready a countdown and the round with everyone
//! spawned together; the longest wait starts it anyway (the straggler
//! joins late); bots are ready at once; `lobby_start` ends it; the next
//! match on another map (a rotation's map change) has its lobby again.
//! Ticks only (64 a second).

use std::time::Duration;

use bevy::prelude::*;
use bevy::ecs::message::MessageCursor;
use chimera_core::{
    character::character_bundle,
    console::{Console, ConsoleAppExt},
    core::{Dead, FreezeTime, MapLoads, Team},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    minigame_net::{self, MinigameNetPlugin},
    net::{
        NetEvent,
        client::{DownloadInfo, Handshake, JoinProgress, JoinStage, Joined},
        maps::map_name,
        memory::LinkConditions,
    },
    prematch::{self, NetLobby, status},
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{self, InMinigame, MinigameAppExt, MinigamePlugin, PreMatch},
    mode::{Match, Phase},
    movement::placeholder,
    testkit,
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{LoadedMapName, MapFile},
};

const DEF: &str = r#"
id = "kit:lobby"
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

const ID: &str = "kit:lobby";
/// Ticks a second.
const HZ: u64 = 64;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout {
        movement: placeholder::ID,
    });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_lobby.toml", DEF)]).expect("the definition reads");
    prematch::enable(app);
}

fn sim(clients: usize) -> NetSim {
    let mut sim = NetSim::new(
        SimBasePlugins,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        clients,
        setup,
    );
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby_max_wait 90");
    sim
}

fn server_lobby(sim: &mut NetSim) -> Option<NetLobby> {
    let w = sim.server.app.world_mut();
    w.query::<&NetLobby>().iter(w).next().cloned()
}

fn client_lobby(sim: &mut NetSim, i: usize) -> Option<NetLobby> {
    let w = sim.clients[i].app.world_mut();
    w.query::<&NetLobby>().iter(w).next().cloned()
}

fn in_lobby(sim: &NetSim) -> bool {
    sim.server.app.world().contains_resource::<PreMatch>()
}

/// A held client's handshake, taken as it comes (before the game
/// would load the map in the same frame).
#[derive(Resource, Default)]
struct HeldJoin(Option<Handshake>);

fn take_handshake(world: &mut World) {
    if world.contains_resource::<HeldJoin>()
        && let Some(h) = world.remove_resource::<Handshake>()
    {
        world.resource_mut::<HeldJoin>().0 = Some(h);
    }
}

/// Client `i`'s join stopped after the server's welcome (its handshake
/// taken away, as while the match's files download).
fn hold(sim: &mut NetSim, i: usize) -> Handshake {
    sim.clients[i].app.init_resource::<HeldJoin>().add_systems(RunFixedMainLoop, take_handshake);
    let ok = sim.until(900, |s| s.clients[i].app.world().resource::<HeldJoin>().0.is_some());
    assert!(ok, "client {i} heard the welcome");
    let w = sim.clients[i].app.world_mut();
    w.remove_resource::<HeldJoin>().unwrap().0.unwrap()
}

/// Client `i` downloading: `done` of `total` bytes on its join's progress.
fn downloading(sim: &mut NetSim, i: usize, done: u64, total: u64) {
    let mut p = sim.clients[i].app.world_mut().resource_mut::<JoinProgress>();
    p.stage = JoinStage::Downloading;
    p.download = Some(DownloadInfo {
        file: chimera_net::net::assets::FILES.into(),
        from: "asset service".into(),
        done,
        total,
    });
}

/// Client `i`'s download done: its join goes on.
fn release(sim: &mut NetSim, i: usize, h: Handshake) {
    let w = sim.clients[i].app.world_mut();
    w.resource_mut::<JoinProgress>().download = None;
    w.insert_resource(h);
}

fn joined(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().contains_resource::<Joined>()
}

/// Whether every character in the match on the server is alive and the
/// match runs.
fn playing(sim: &mut NetSim, n: usize) -> bool {
    let w = sim.server.app.world_mut();
    let alive = w.query_filtered::<Option<&Dead>, With<InMinigame>>().iter(w).filter(|d| d.is_none()).count();
    alive == n && w.resource::<Match>().phase == Phase::Live
}

fn row<'a>(l: &'a NetLobby, id: u64) -> &'a chimera_net::prematch::LobbyRow {
    l.rows.iter().find(|r| r.id == Some(id)).expect("the player's row")
}

#[test]
fn the_lobby_waits_for_a_downloading_player_then_counts_down_and_starts() {
    let mut sim = sim(2);
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    let h = hold(&mut sim, 1);
    downloading(&mut sim, 1, 10_000_000, 80_000_000);
    assert!(sim.until(600, |s| joined(s, 0)), "client 0 is in");
    // Client 0 stands frozen at its spawn; the match doesn't start for
    // client 1's download, however long.
    let a = sim.character_of(0).unwrap();
    sim.ticks(30 * HZ);
    assert!(in_lobby(&sim), "still in the lobby");
    let l = server_lobby(&mut sim).expect("the lobby");
    assert_eq!((l.countdown, l.waiting), (None, 1), "{l:?}");
    assert!(l.wait_left.is_some_and(|s| s > 0 && s <= 60), "{l:?}");
    assert_eq!(row(&l, sim.client_id(0)).status, status::READY);
    let r1 = row(&l, sim.client_id(1));
    assert_eq!((r1.status, r1.done, r1.total), (status::DOWNLOADING, 10_000_000, 80_000_000));
    assert!(row(&l, sim.client_id(0)).ping_ms.is_some(), "pings");
    {
        let w = sim.server.app.world();
        assert_eq!(w.resource::<Match>().phase, Phase::Waiting);
        assert!(w.get::<InMinigame>(a).is_some() && w.get::<Dead>(a).is_none(), "client 0 spawned");
        assert!(w.resource::<FreezeTime>().0, "frozen");
    }
    // The download ends: client 1 loads the map, then the countdown.
    release(&mut sim, 1, h);
    assert!(sim.until(600, |s| server_lobby(s).is_some_and(|l| l.countdown.is_some())), "the countdown");
    let at = sim.server.app.world().resource::<chimera_core::core::SimClock>().now;
    let l = server_lobby(&mut sim).unwrap();
    assert_eq!((l.countdown, l.waiting), (Some(5), 0), "{l:?}");
    assert!(sim.until(10 * HZ, |s| !in_lobby(s)), "the lobby ends");
    let took = sim.server.app.world().resource::<chimera_core::core::SimClock>().now - at;
    assert!((took - 5.0).abs() < 0.1, "5 s of countdown: {took}");
    assert!(server_lobby(&mut sim).is_none());
    assert!(sim.until(HZ, |s| playing(s, 2)), "both spawn and the round runs");
    assert!(!sim.server.app.world().resource::<FreezeTime>().0, "unfrozen");
    sim.ticks(10);
    assert!(client_lobby(&mut sim, 0).is_none() && client_lobby(&mut sim, 1).is_none(), "gone on the clients");
}

#[test]
fn each_players_progress_reaches_the_other_clients() {
    let mut sim = sim(2);
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    let _h = hold(&mut sim, 1);
    downloading(&mut sim, 1, 1_000_000, 50_000_000);
    assert!(sim.until(600, |s| joined(s, 0)), "client 0 is in");
    let id1 = sim.client_id(1);
    let seen = |s: &mut NetSim, i: usize, done: u64| {
        client_lobby(s, i).is_some_and(|l| l.rows.iter().any(|r| r.id == Some(id1) && r.done == done && r.total == 50_000_000))
    };
    assert!(sim.until(HZ, |s| seen(s, 0, 1_000_000) && seen(s, 1, 1_000_000)), "{:?}", client_lobby(&mut sim, 0));
    downloading(&mut sim, 1, 30_000_000, 50_000_000);
    assert!(sim.until(HZ, |s| seen(s, 0, 30_000_000) && seen(s, 1, 30_000_000)), "{:?}", client_lobby(&mut sim, 0));
    let l = client_lobby(&mut sim, 0).unwrap();
    assert_eq!(l.mode, "Kit gun game");
    assert_eq!(l.map, "greybox");
    assert_eq!(l.rows.len(), 2);
    assert_eq!(row(&l, sim.client_id(0)).status, status::READY);
    assert_eq!(row(&l, id1).status, status::DOWNLOADING);
}

#[test]
fn the_longest_wait_starts_the_match_and_the_straggler_joins_late() {
    let mut sim = sim(2);
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby_max_wait 10");
    sim.ticks(1);
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    let h = hold(&mut sim, 1);
    downloading(&mut sim, 1, 5, 100);
    assert!(sim.until(600, |s| joined(s, 0)), "client 0 is in");
    assert!(sim.until(12 * HZ, |s| server_lobby(s).is_some_and(|l| l.countdown.is_some())), "the wait ran out");
    assert!(sim.until(6 * HZ, |s| !in_lobby(s)), "the match starts without client 1");
    // Client 0 alone (min 2): the match waits for players as any does;
    // client 1 joins it late, with no lobby again.
    release(&mut sim, 1, h);
    assert!(sim.until(600, |s| joined(s, 1)), "client 1 is in");
    assert!(sim.until(2 * HZ, |s| playing(s, 2)), "both play");
    assert!(!in_lobby(&sim) && server_lobby(&mut sim).is_none(), "no lobby for a late joiner");
}

#[test]
fn bots_are_ready_at_once() {
    let mut sim = sim(1);
    let w = sim.server.app.world_mut();
    let bot = w.spawn(character_bundle(Transform::from_xyz(0.0, 1.0, 0.0), Team(1))).id();
    w.entity_mut(bot).insert(Name::new("Bot Bob"));
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    assert!(sim.until(600, |s| joined(s, 0)), "client 0 is in");
    assert!(sim.until(HZ, |s| server_lobby(s).is_some_and(|l| l.countdown.is_some())), "the countdown");
    let l = server_lobby(&mut sim).unwrap();
    let bot = l.rows.iter().find(|r| r.bot()).expect("the bot's row");
    assert_eq!((bot.name.as_str(), bot.status), ("Bot Bob", status::READY));
    assert!(sim.until(6 * HZ, |s| !in_lobby(s)), "the lobby ends");
    assert!(sim.until(HZ, |s| playing(s, 2)), "the player and the bot play");
}

#[test]
fn the_host_starts_it_now() {
    let mut sim = sim(2);
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    let _h = hold(&mut sim, 1);
    downloading(&mut sim, 1, 5, 100);
    assert!(sim.until(600, |s| joined(s, 0)), "client 0 is in");
    sim.ticks(HZ);
    assert!(in_lobby(&sim));
    sim.server.app.world_mut().resource_mut::<Console>().submit("lobby_start 0");
    sim.ticks(2);
    assert!(!in_lobby(&sim), "started at once");
    // Again on a fresh match, with the countdown.
    minigame::start(sim.server.app.world_mut(), ID).expect("again");
    sim.ticks(2);
    assert!(in_lobby(&sim));
    sim.server.app.world_mut().resource_mut::<Console>().submit("lobby_start");
    sim.ticks(2);
    assert_eq!(server_lobby(&mut sim).and_then(|l| l.countdown), Some(5));
    assert!(sim.until(6 * HZ, |s| !in_lobby(s)));
}

/// A test map loading here: its id and the frames left.
#[derive(Resource)]
struct FakeLoad(String, u32);

fn loaded(world: &mut World, map: &str) {
    world.insert_resource(LoadedMapName(map.to_string()));
    world.insert_resource(MapFile {
        name: map_name(map).to_string(),
        hash: None,
    });
}

/// The server's test `changelevel`: the map in 30 frames, a fresh game.
fn server_loads(world: &mut World) {
    let Some(FakeLoad(map, left)) = world.remove_resource::<FakeLoad>() else { return };
    if left > 0 {
        world.insert_resource(FakeLoad(map, left - 1));
        return;
    }
    loaded(world, &map);
    chimera_core::gameplay::new_game(world);
    minigame::restart(world);
}

/// A client loads what its handshake asks for `frames` frames later.
fn client_loads(world: &mut World, mut cursor: Local<MessageCursor<NetEvent>>) {
    let asked: Vec<String> = cursor
        .read(world.resource::<Messages<NetEvent>>())
        .filter_map(|e| match e {
            NetEvent::LoadMap { map, .. } => Some(map.clone()),
            _ => None,
        })
        .collect();
    if let Some(map) = asked.into_iter().last() {
        world.insert_resource(FakeLoad(map, 90));
    }
    let Some(FakeLoad(map, left)) = world.remove_resource::<FakeLoad>() else { return };
    if left > 0 {
        world.insert_resource(FakeLoad(map, left - 1));
        return;
    }
    loaded(world, &map);
    world.get_resource_or_init::<MapLoads>().0 += 1;
}

#[test]
fn a_map_change_runs_the_lobby_again() {
    let mut sim = sim(2);
    sim.server.app.add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        w.insert_resource(LoadedMapName(map.clone()));
        w.insert_resource(FakeLoad(map, 30));
        Ok(None)
    });
    for c in &mut sim.clients {
        c.app.add_systems(Update, client_loads);
    }
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    assert!(sim.until(900, |s| joined(s, 0) && joined(s, 1)), "both in");
    assert!(sim.until(8 * HZ, |s| playing(s, 2)), "the first match plays");
    // The next match on the pool's other map.
    minigame_net::change_match(sim.server.app.world_mut(), ID, Some("test:next")).expect("the change");
    assert!(sim.until(600, |s| in_lobby(s)), "a lobby on the new map");
    let l = server_lobby(&mut sim).unwrap();
    assert_eq!(l.map, "test:next");
    assert!(l.rows.iter().all(|r| r.status != status::READY), "the clients still load it: {l:?}");
    assert_eq!(l.countdown, None);
    assert!(sim.until(600, |s| server_lobby(s).is_some_and(|l| l.countdown.is_some())), "both have it: the countdown");
    assert!(sim.until(6 * HZ, |s| !in_lobby(s)));
    assert!(sim.until(HZ, |s| playing(s, 2)), "the next match plays");
    assert_eq!(sim.server.app.world().resource::<LoadedMapName>().0, "test:next");
}
