//! Spectators (`chimera_net::spectators`, `chimera_sim::minigame::
//! spectators`) on `NetSim`, the engine's base, the greybox and the test
//! kit's guns: a client joining with `cl_spectator 1` watches (no team,
//! dead, out of the minigame), sees the players move, never spawns, sends
//! no inputs; `spectator 0|1` switches it as the host allows
//! (`mp_spectate_switch`, `mp_spectators_max`); the pre-match lobby lists
//! it apart and never waits for it; the map vote counts it only with
//! `sv_vote_allow_spectators 1`. Ticks only (64 a second).

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{Connecting, Dead, Spectating},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    minigame_net::{self, MinigameNetPlugin},
    net::{
        client::{Handshake, JoinProgress, JoinStage, Joined},
        memory::LinkConditions,
        server::CommandBuffer,
    },
    prematch::{self, NetLobby},
    spectators::JoinAsSpectator,
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{self, InMinigame, MinigameAppExt, MinigamePlugin, PreMatch, vote::MapVote},
    movement::placeholder,
    testkit,
    weapon::Inventory,
};
use chimera_world::greybox::GreyboxMapPlugin;

const DEF: &str = r#"
id = "kit:watch"
name = "Kit gun game"
players = { min = 1, max = 8 }
maps = ["chimera:greybox"]
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

const ID: &str = "kit:watch";
const HZ: u64 = 64;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout {
        movement: placeholder::ID,
    });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_watch.toml", DEF)]).expect("the definition reads");
}

fn sim(players: usize, lobby: bool) -> NetSim {
    let mut sim = NetSim::new(
        SimBasePlugins,
        LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 },
        11,
        players,
        move |app| {
            setup(app);
            if lobby {
                prematch::enable(app);
            }
        },
    );
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby_host_start 0");
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    sim
}

/// A client that asks to watch (`cl_spectator 1`).
fn add_spectator(sim: &mut NetSim) -> usize {
    sim.add_client(|w| w.resource_mut::<JoinAsSpectator>().0 = 1)
}

fn joined(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().contains_resource::<Joined>()
}

fn say(sim: &mut NetSim, i: usize, line: &str) {
    sim.clients[i].app.world_mut().resource_mut::<Console>().submit(line);
}

fn watching(sim: &mut NetSim, i: usize) -> bool {
    let e = sim.character_of(i).expect("its character");
    sim.server.app.world().get::<Spectating>(e).is_some()
}

#[test]
fn a_spectator_joins_watching_sees_the_players_and_never_spawns() {
    let mut sim = sim(1, false);
    sim.until_joined(900);
    let spec = add_spectator(&mut sim);
    assert!(sim.until(900, |s| joined(s, spec) && s.local_player(spec).is_some()), "the spectator is in");
    sim.ticks(5 * HZ);
    let me = sim.character_of(spec).unwrap();
    {
        let w = sim.server.app.world_mut();
        assert!(w.get::<Spectating>(me).is_some(), "watching");
        assert!(w.get::<InMinigame>(me).is_none() && w.get::<Dead>(me).is_some(), "out of play");
        assert!(w.get::<Connecting>(me).is_none(), "in the game as a watcher once its map is in");
        assert!(w.get::<Inventory>(me).is_none_or(|i| i.weapons.is_empty()), "no weapons");
        assert_eq!(minigame::runtime::players(w).len(), 1, "one player in the match");
        // No inputs from it: the server ran no command of its.
        assert!(w.get::<CommandBuffer>(me).is_none_or(|b| b.last.is_none()), "no commands");
    }
    // Its own copy says so (the client's camera follows `Spectating`).
    let mine = sim.local_player(spec).unwrap();
    assert!(sim.clients[spec].app.world().get::<Spectating>(mine).is_some(), "replicated");
    // It sees the player where the server has it.
    let p = sim.character_of(0).unwrap();
    let to = sim.server.position(p) + Vec3::new(2.0, 0.0, 1.0);
    sim.server.app.world_mut().get_mut::<Transform>(p).unwrap().translation = to;
    let id = sim.client_id(0);
    assert!(
        sim.until(HZ, |s| NetSim::owned_by(&mut s.clients[spec], Some(id)).is_some_and(|e| s.clients[spec].position(e).distance(s.server.position(p)) < 0.3)),
        "the player seen where it is"
    );
}

#[test]
fn spectator_switches_both_ways_as_the_host_allows() {
    let mut sim = sim(2, false);
    sim.until_joined(900);
    sim.ticks(HZ);
    // Playing to watching: out at once, its weapons gone.
    say(&mut sim, 1, "spectator 1");
    assert!(sim.until(HZ, |s| watching(s, 1)), "watching");
    let e = sim.character_of(1).unwrap();
    assert!(sim.server.app.world().get::<InMinigame>(e).is_none());
    sim.ticks(3 * HZ);
    assert!(sim.server.app.world().get::<Dead>(e).is_some(), "never respawns");
    // Back: the minigame takes it in and spawns it.
    say(&mut sim, 1, "spectator");
    assert!(sim.until(2 * HZ, |s| !watching(s, 1)), "playing");
    assert!(
        sim.until(2 * HZ, |s| {
            let w = s.server.app.world();
            w.get::<InMinigame>(e).is_some() && w.get::<Dead>(e).is_none()
        }),
        "spawned"
    );
    // The host forbids switching: refused.
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_spectate_switch 0");
    sim.ticks(HZ / 2);
    say(&mut sim, 1, "spectator 1");
    sim.ticks(HZ);
    assert!(!watching(&mut sim, 1), "not allowed");
    // No room: refused too.
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_spectate_switch 1");
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_spectators_max 1");
    sim.ticks(HZ / 2);
    say(&mut sim, 0, "spectator 1");
    assert!(sim.until(HZ, |s| watching(s, 0)), "the first fits");
    say(&mut sim, 1, "spectator 1");
    sim.ticks(HZ);
    assert!(!watching(&mut sim, 1), "the cap is 1");
}

/// A held client's handshake (as while its files download).
#[derive(Resource, Default)]
struct HeldJoin(Option<Handshake>);

fn take_handshake(world: &mut World) {
    if world.contains_resource::<HeldJoin>()
        && let Some(h) = world.remove_resource::<Handshake>()
    {
        world.resource_mut::<HeldJoin>().0 = Some(h);
    }
}

#[test]
fn a_spectator_join_is_refused_when_none_fit() {
    use chimera_net::spectators::{NONE_ALLOWED, NO_ROOM};
    let mut sim = sim(1, false);
    sim.until_joined(900);
    let refused = |sim: &NetSim, i: usize| sim.clients[i].app.world().resource::<JoinProgress>().failure.as_ref().map(|(_, why)| why.clone());
    // The host allows none: refused with why, and it did not play instead.
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_spectators_max 0");
    sim.ticks(HZ / 2);
    let none = add_spectator(&mut sim);
    assert!(sim.until(900, |s| refused(s, none).is_some()), "told why");
    assert_eq!(refused(&sim, none).as_deref(), Some(NONE_ALLOWED));
    assert!(sim.character_of(none).is_none(), "not in the game");
    // One place, taken: the next is refused for the room, the first in.
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_spectators_max 1");
    sim.ticks(HZ / 2);
    let first = add_spectator(&mut sim);
    assert!(sim.until(900, |s| joined(s, first) && s.character_of(first).is_some()), "the first fits");
    assert!(watching(&mut sim, first));
    let second = add_spectator(&mut sim);
    assert!(sim.until(900, |s| refused(s, second).is_some()), "told why");
    assert_eq!(refused(&sim, second).as_deref(), Some(NO_ROOM));
    // A player who doesn't ask to watch is not refused for it.
    let player = sim.add_client(|_| {});
    assert!(sim.until(900, |s| joined(s, player) && s.character_of(player).is_some()), "a player is let in");
    assert!(!watching(&mut sim, player));
}

#[test]
fn the_lobby_lists_spectators_apart_and_never_waits_for_them() {
    let mut sim = sim(1, true);
    // A spectator still downloading: the lobby doesn't wait for it.
    let spec = sim.add_client(|w| w.resource_mut::<JoinAsSpectator>().0 = 1);
    sim.clients[spec].app.init_resource::<HeldJoin>().add_systems(RunFixedMainLoop, take_handshake);
    assert!(sim.until(900, |s| s.clients[spec].app.world().resource::<HeldJoin>().0.is_some()), "welcomed");
    sim.clients[spec].app.world_mut().resource_mut::<JoinProgress>().stage = JoinStage::Downloading;
    let lobby = |s: &mut NetSim| {
        let w = s.server.app.world_mut();
        w.query::<&NetLobby>().iter(w).next().cloned()
    };
    let id = sim.client_id(spec);
    assert!(
        sim.until(10 * HZ, |s| lobby(s).is_some_and(|l| l.rows.iter().any(|r| r.id == Some(id) && r.spectator))),
        "listed as a spectator: {:?}",
        lobby(&mut sim)
    );
    assert!(sim.until(10 * HZ, |s| lobby(s).is_some_and(|l| l.countdown.is_some())), "counts down without it: {:?}", lobby(&mut sim));
    let l = lobby(&mut sim).unwrap();
    assert_eq!(l.waiting, 0, "{l:?}");
    assert!(sim.until(6 * HZ, |s| !s.server.app.world().contains_resource::<PreMatch>()), "the match starts");
    let e = sim.character_of(spec).unwrap();
    let w = sim.server.app.world();
    assert!(w.get::<Spectating>(e).is_some() && w.get::<InMinigame>(e).is_none(), "still watching, not let in");
}

#[test]
fn the_map_vote_counts_a_spectator_only_when_the_host_allows() {
    let mut sim = sim(2, false);
    sim.until_joined(900);
    say(&mut sim, 1, "spectator 1");
    assert!(sim.until(HZ, |s| watching(s, 1)));
    let (player, spec) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let w = sim.server.app.world_mut();
    let now = w.resource::<chimera_core::core::SimClock>().now;
    w.insert_resource(MapVote::new(vec!["chimera:greybox".into(), "test:other".into()], now + 30.0));
    assert!(minigame_net::cast_vote(w, player, 1).is_ok(), "a player votes");
    assert!(minigame_net::cast_vote(w, spec, 0).is_err(), "a spectator doesn't");
    w.resource_mut::<Console>().submit("sv_vote_allow_spectators 1");
    sim.ticks(1);
    let w = sim.server.app.world_mut();
    assert!(minigame_net::cast_vote(w, spec, 0).is_ok(), "unless the host allows");
}
