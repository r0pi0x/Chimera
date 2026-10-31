//! Map rotation and the end-of-match results (architecture review L2/L3
//! polish): a two-rung ladder on a pool of two maps, played to its end on
//! `NetSim`'s in-memory link. The results (winner first, each player's
//! rung, kills and deaths) reach every client through the intermission;
//! after the hold the server moves to the pool's next map by
//! `change_match` (the clients stay connected, the scores anew, the
//! results gone), or restarts on this map with `rotate = false`. The map
//! change is ordered: the match held on the old map, the clients'
//! loading state (`JoinProgress`'s `ChangingLevel`, the loading dialog's)
//! while it loads, and the next match only once the server and every
//! client have the new map (the test's maps load in a few frames, the
//! clients' later than the server's). The win pauses everyone (the
//! server's end freeze, held in the clients' prediction too) and every
//! client shows the match-end screen (`client::match_end`): the
//! standings with its own row, the next map and the countdown.

use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::{
    console::ConsoleAppExt,
    core::{self, Damage, DamageKind, Dead, Hitgroup, MapLoads, Score},
};
use chimera_net::{
    harness::NetSim,
    minigame_net::MapChange,
    net::{
        NetEvent,
        client::{JoinProgress, JoinStage, Joined},
        maps::map_name,
    },
};
use chimera_sim::{
    minigame::{self, ActiveMinigame, MatchHold, MatchResults, hud::MinigameHud, ladder::Rung, wins::MatchesWon},
    mode::{Match, Phase},
};
use chimera_world::map::{LoadedMapName, MapFile};

use crate::net_party::{failure, party_and};

/// A two-rung ladder on a pool of two maps, short enough to finish in a
/// test.
pub(crate) const ROTATING: &str = r#"
id = "test:rotating"
name = "Rotating ladder"
players = { min = 2, max = 8 }
maps = ["chimera:greybox", "test:next"]
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

/// The levels the server was asked to change to (the test's
/// `changelevel`).
#[derive(Resource, Default)]
pub(crate) struct Levels(pub Vec<String>);

/// Frames a test map takes to load on the server and on a client.
pub(crate) const SERVER_LOAD: u32 = 30;
const CLIENT_LOAD: u32 = 90;

/// A test map loading here: its id and the frames left.
#[derive(Resource)]
pub(crate) struct FakeLoad(pub String, pub u32);

/// A map's load done (`MapFile` for the handshake, a map load counted),
/// as the game's loader leaves it (no world: the test's maps are empty).
fn loaded(world: &mut World, map: &str) {
    world.insert_resource(LoadedMapName(map.to_string()));
    world.insert_resource(MapFile {
        name: map_name(map).to_string(),
        hash: None,
    });
}

/// The server's `changelevel` as the party server's: the map's name at
/// once (its clients hear "changing level"), the map in `SERVER_LOAD`
/// frames later with a fresh game (`party_kit::server::swap_map`).
pub(crate) fn server_loads(world: &mut World) {
    let Some(FakeLoad(map, left)) = world.remove_resource::<FakeLoad>() else { return };
    if left > 0 {
        world.insert_resource(FakeLoad(map, left - 1));
        return;
    }
    loaded(world, &map);
    chimera_core::gameplay::new_game(world);
    if world.contains_resource::<ActiveMinigame>() {
        minigame::restart(world);
    }
}

/// A client loads what its handshake asks for (`NetEvent::LoadMap`)
/// `CLIENT_LOAD` frames later.
pub(crate) fn client_loads(world: &mut World, mut cursor: Local<MessageCursor<NetEvent>>) {
    let asked: Vec<String> = cursor
        .read(world.resource::<Messages<NetEvent>>())
        .filter_map(|e| match e {
            NetEvent::LoadMap { map, .. } => Some(map.clone()),
            _ => None,
        })
        .collect();
    if let Some(map) = asked.into_iter().last() {
        world.insert_resource(FakeLoad(map, CLIENT_LOAD));
    }
    let Some(FakeLoad(map, left)) = world.remove_resource::<FakeLoad>() else { return };
    if left > 0 {
        world.insert_resource(FakeLoad(map, left - 1));
        return;
    }
    loaded(world, &map);
    world.get_resource_or_init::<MapLoads>().0 += 1;
}

/// `by` kills `victim` on the server with one hit.
fn hit(sim: &mut NetSim, victim: Entity, by: Entity) {
    let at = sim.server.position(victim);
    sim.server.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(by),
        amount: 5.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(3);
}

/// Two clients joined to `text`'s ladder, and the first one's character
/// wins it (two kills on the second's).
fn a_won_match(text: &str) -> (NetSim, Entity) {
    a_won_match_and(text, |_| {})
}

/// `a_won_match` with `extra` on every app.
pub(crate) fn a_won_match_and(text: &str, extra: fn(&mut App)) -> (NetSim, Entity) {
    let mut sim = party_and(2, Some(text), extra);
    sim.server.app.init_resource::<Levels>().add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        w.resource_mut::<Levels>().0.push(map.clone());
        w.insert_resource(LoadedMapName(map.clone()));
        w.insert_resource(FakeLoad(map, SERVER_LOAD));
        Ok(None)
    });
    for c in &mut sim.clients {
        c.app.add_systems(Update, client_loads);
    }
    sim.until_joined(900);
    sim.ticks(120);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    // Players are told apart by name (matches won are counted by it).
    for (e, name) in [(a, "Ann"), (b, "Ben")] {
        let w = sim.server.app.world_mut();
        w.entity_mut(e).insert(Name::new(name));
        w.get_mut::<chimera_net::net::NetCharacter>(e).unwrap().name = name.into();
    }
    for _ in 0..2 {
        assert!(sim.until(600, |s| s.server.app.world().get::<Dead>(b).is_none()), "the victim is back");
        sim.ticks(30);
        hit(&mut sim, b, a);
    }
    (sim, a)
}

#[test]
fn the_results_reach_every_client_and_the_party_moves_to_the_next_map() {
    let (mut sim, a) = a_won_match(ROTATING);
    // The match is decided: the results (winner first) are on the server
    // and, replicated, on both clients, through the intermission.
    let ok = sim.until(1500, |s| s.clients.iter().all(|c| c.app.world().get_resource::<MatchResults>().is_some()));
    assert!(ok, "both clients get the results");
    let server = sim.server.app.world().resource::<MatchResults>().clone();
    assert_eq!(server.rungs, 2);
    assert_eq!(server.rows.len(), 2);
    assert!(server.rows[0].winner && server.rows[0].player == Some(a), "{server:?}");
    assert_eq!((server.rows[0].rung, server.rows[0].kills), (Some(2), 2), "{server:?}");
    assert_eq!((server.rows[1].rung, server.rows[1].deaths), (Some(1), 2), "{server:?}");
    for i in 0..2 {
        let got = sim.clients[i].app.world().resource::<MatchResults>().clone();
        assert_eq!(got.rows.len(), 2, "client {i}");
        assert_eq!(got.title, server.title);
        for (g, s) in got.rows.iter().zip(&server.rows) {
            assert_eq!((g.winner, g.rung, g.kills, g.deaths), (s.winner, s.rung, s.kills, s.deaths), "client {i}");
            assert!(!g.name.is_empty(), "client {i}: players are named");
        }
        assert!(got.rows[0].winner && !got.rows[1].winner, "client {i}: the winner first");
        let hud = sim.clients[i].app.world().resource::<MinigameHud>();
        assert_eq!(hud.results.as_ref().map(|r| r.rows.len()), Some(2), "client {i}'s HUD data");
    }
    assert!(sim.server.app.world().resource::<Levels>().0.is_empty(), "not yet: the results are up first");
    let rung_before = sim.server.app.world().get::<Rung>(a).map(|r| r.0);
    // After the hold the server rotates: the pool's next map, the match
    // held there until everyone has it.
    let ok = sim.until(3000, |s| !s.server.app.world().resource::<Levels>().0.is_empty());
    assert!(ok, "the server moves on");
    assert_eq!(sim.server.app.world().resource::<Levels>().0, ["test:next"], "the pool's second map");
    // Each frame until the new match runs: nothing starts on the old map
    // or before both clients are on the new one; the clients show the
    // loading state while the server loads.
    let mut told = [false; 2];
    let mut problems: Vec<String> = Vec::new();
    let ok = sim.until(3000, |s| {
        let w = s.server.app.world();
        let phase = w.resource::<Match>().phase;
        let changing = w.get_resource::<MapChange>().cloned();
        let on_new = w.resource::<LoadedMapName>().0 == "test:next" && w.contains_resource::<MapFile>();
        let started = !matches!(phase, Phase::MatchOver | Phase::Waiting);
        if changing.is_some() && !w.contains_resource::<MatchHold>() {
            problems.push("a map change without the match held".into());
        }
        if !on_new {
            if phase != Phase::MatchOver {
                problems.push(format!("the match left match over on the old map: {phase:?}"));
            }
            if !w.contains_resource::<MatchResults>() {
                problems.push("the results went before the map changed".into());
            }
            if w.get::<Rung>(a).map(|r| r.0) != rung_before {
                problems.push("the ladder reset on the old map".into());
            }
        }
        for (i, t) in told.iter_mut().enumerate() {
            let c = s.clients[i].app.world();
            if !on_new && c.resource::<JoinProgress>().stage == JoinStage::ChangingLevel {
                *t = true;
            }
            let on_it = c.get_resource::<Joined>().is_some_and(|j| j.map == "test:next");
            if started && !on_it {
                problems.push(format!("the match started ({phase:?}) before client {i} had the new map"));
            }
        }
        on_new && phase == Phase::Live
    });
    assert!(ok, "the next match starts on the new map");
    assert!(problems.is_empty(), "{:?}", &problems[..problems.len().min(5)]);
    assert_eq!(told, [true, true], "both clients show the loading state while the server loads");
    assert!(!sim.server.app.world().contains_resource::<MatchHold>(), "the hold lifted");
    sim.ticks(60);
    assert!(sim.server.app.world().get_resource::<MatchResults>().is_none(), "a new match, no results");
    for i in 0..2 {
        assert!(sim.clients[i].app.world().contains_resource::<Joined>(), "client {i} still in");
        assert!(failure(&sim, i).is_none(), "client {i} never left");
        assert!(sim.clients[i].app.world().get_resource::<MatchResults>().is_none(), "client {i}: the results are gone");
        let me = sim.local_player(i).unwrap();
        let w = sim.clients[i].app.world();
        assert_eq!(w.resource::<Match>().phase, Phase::Live, "client {i}: the match runs");
        assert_eq!(w.get::<Score>(me).map(|s| s.kills), Some(0), "client {i}: scores anew");
        assert_eq!(w.get::<Rung>(me).map(|r| r.0), Some(0), "client {i}: the first rung");
    }
}

#[test]
fn without_rotation_the_same_map_is_played_again() {
    let (mut sim, _a) = a_won_match(&ROTATING.replace("time_limit", "rotate = false\ntime_limit"));
    assert!(sim.until(1500, |s| s.server.app.world().get_resource::<MatchResults>().is_some()));
    assert!(sim.until(3000, |s| s.server.app.world().get_resource::<MatchResults>().is_none()), "the match restarts");
    sim.ticks(240);
    assert!(sim.server.app.world().resource::<Levels>().0.is_empty(), "no level change");
    assert_eq!(sim.server.app.world().resource::<Match>().phase, Phase::Live, "a new match on the same map");
    assert!((0..2).all(|i| sim.clients[i].app.world().contains_resource::<Joined>()));
}

/// Every `MatchesWon` a client has, sorted.
fn tallies(sim: &mut NetSim, client: usize) -> Vec<u32> {
    let w = sim.clients[client].app.world_mut();
    let mut v: Vec<u32> = w.query::<&MatchesWon>().iter(w).map(|m| m.0).collect();
    v.sort();
    v
}

#[test]
fn matches_won_reach_every_client_and_survive_the_rotation() {
    let (mut sim, _a) = a_won_match(ROTATING);
    // The winner has one, the other none, on both clients.
    let ok = sim.until(1500, |s| (0..2).all(|i| tallies(s, i) == [0, 1]));
    assert!(ok, "both clients see one match won: {:?} {:?}", tallies(&mut sim, 0), tallies(&mut sim, 1));
    // The party moves to the next map: a new match, the count kept.
    assert!(sim.until(3000, |s| s.server.app.world().get_resource::<MatchResults>().is_none()));
    sim.ticks(240);
    for i in 0..2 {
        assert_eq!(tallies(&mut sim, i), [0, 1], "client {i}: kept over the rotation");
    }
}

/// The match-end screen on every client (`client::match_end`): during the
/// hold each client has the server's standings (final gun, matches won),
/// its own row found by its character, the next map the server picked
/// and the countdown to the change, in step with the server's.
#[test]
fn every_client_shows_the_match_end_screen_with_its_own_row_and_the_next_map() {
    use chimera_client::client::match_end::{MatchEndPlugin, MatchEndView};
    use chimera_sim::minigame::NextMatch;
    let (mut sim, a) = a_won_match_and(ROTATING, |app| {
        app.add_plugins(MatchEndPlugin);
    });
    // The win pauses everyone: both clients press forward, jump and fire,
    // and nobody moves on the server or in a client's prediction, the
    // winner's weapon silent (the server's freeze, `core::FreezeTime`).
    assert!(matches!(sim.server.app.world().resource::<Match>().phase, Phase::RoundOver { .. }));
    sim.ticks(10);
    let me: Vec<Entity> = (0..2).map(|i| sim.local_player(i).unwrap()).collect();
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let server_at = flat(sim.server.position(a));
    let client_at: Vec<Vec2> = (0..2).map(|i| flat(sim.clients[i].position(me[i]))).collect();
    let fired = |s: &NetSim| {
        let w = s.server.app.world();
        let gun = w.get::<chimera_sim::weapon::Inventory>(a).and_then(|inv| inv.active);
        gun.and_then(|g| w.get::<chimera_sim::weapon::WeaponState>(g)).map(|st| (st.burst, st.next_primary.to_bits()))
    };
    let before = fired(&sim);
    for _ in 0..90 {
        for i in 0..2 {
            let mut intent = sim.clients[i].app.world_mut().get_mut::<chimera_core::core::Intent>(me[i]).unwrap();
            intent.move_axis = Vec2::Y;
            intent.jump = true;
            intent.fire = true;
        }
        sim.step();
    }
    assert!(sim.server.app.world().resource::<chimera_core::core::FreezeTime>().0, "frozen on the server");
    let moved = flat(sim.server.position(a)).distance(server_at);
    assert!(moved < 0.01, "the winner stands on the server: moved {moved}");
    for i in 0..2 {
        let moved = flat(sim.clients[i].position(me[i])).distance(client_at[i]);
        assert!(moved < 0.01, "client {i} stands in its prediction: moved {moved}");
    }
    assert_eq!(fired(&sim), before, "no shot fired");
    // The win's callout has the frozen window to itself: up on both
    // clients, the match-end screen not yet.
    for i in 0..2 {
        let w = sim.clients[i].app.world();
        assert!(w.resource::<MinigameHud>().callout.is_some(), "client {i}: the win's callout shows");
        assert!(!w.resource::<MatchEndView>().shown, "client {i}: the screen comes after it");
    }
    // The screen with the results, at the round's end (after the win's
    // own moment).
    let ok = sim.until(1500, |s| s.clients.iter().all(|c| c.app.world().get_resource::<MatchResults>().is_some()));
    assert!(ok);
    // The server's next match, then the screen on both clients.
    let ok = sim.until(1500, |s| {
        s.server.app.world().get_resource::<NextMatch>().is_some_and(|n| n.map.is_some())
            && s.clients.iter().all(|c| c.app.world().resource::<MatchEndView>().shown)
    });
    assert!(ok, "the screen on every client");
    for i in 0..2 {
        assert!(sim.clients[i].app.world().resource::<MinigameHud>().callout.is_none(), "client {i}: the callout over");
    }
    let server = sim.server.app.world().resource::<MatchResults>().clone();
    let guns: Vec<(Option<&str>, u32)> = server.rows.iter().map(|r| (r.gun.as_deref(), r.wins)).collect();
    assert_eq!(guns, [(Some("cs_source:weapon_ak47"), 1), (Some("cs_source:weapon_deagle"), 0)]);
    let next = sim.server.app.world().resource::<NextMatch>().clone();
    assert_eq!(next.map.as_deref(), Some("test:next"), "the pool's next map, picked at the hold's start");
    let server_left = next.at - sim.server.app.world().resource::<chimera_core::core::SimClock>().now;
    for i in 0..2 {
        let me = sim.local_player(i).unwrap();
        let w = sim.clients[i].app.world();
        let r = w.resource::<MatchResults>();
        let mine: Vec<bool> = r.rows.iter().map(|row| row.player == Some(me)).collect();
        assert_eq!(mine.iter().filter(|m| **m).count(), 1, "client {i}: its own character in the results: {r:?}");
        let n = w.resource::<NextMatch>();
        assert_eq!(n.map, next.map, "client {i}");
        let left = n.at - w.resource::<chimera_core::core::SimClock>().now;
        assert!((left - server_left).abs() < 0.5, "client {i}: {left} s left, the server {server_left}");
        let v = w.resource::<MatchEndView>();
        assert_eq!(v.rows.len(), 2);
        assert_eq!(v.rows.iter().filter(|r| r.local).count(), 1, "client {i}: one row marked");
        let my_row = v.rows.iter().position(|r| r.local).unwrap();
        assert_eq!(mine.iter().position(|m| *m), Some(my_row), "client {i}: the marked row is its own");
        assert!(!v.rows[0].gun.is_empty() && !v.rows[0].gun.contains(':'), "client {i}: the final gun by name: {:?}", v.rows[0]);
        assert_eq!((v.rows[0].wins, v.rows[0].winner), (1, true));
        assert_eq!(v.winner.as_deref(), Some("Ann"), "client {i}");
        assert_eq!(v.you_won, v.rows[my_row].winner, "client {i}");
        assert!(v.next.starts_with("Next map: next in "), "client {i}: {}", v.next);
    }
    // Through the change: the screen says so, then goes with the new match.
    let ok = sim.until(3000, |s| s.clients.iter().all(|c| c.app.world().resource::<MatchEndView>().next == "Changing map to next"));
    assert!(ok, "the countdown reaches the change");
    let ok = sim.until(3000, |s| s.server.app.world().resource::<Match>().phase == Phase::Live);
    assert!(ok);
    sim.ticks(60);
    assert!(sim.clients.iter().all(|c| !c.app.world().resource::<MatchEndView>().shown), "gone with the new match");
    assert!(!sim.server.app.world().resource::<chimera_core::core::FreezeTime>().0, "the freeze lifted");
}
