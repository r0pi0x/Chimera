//! The lobby over the network and in a game of one (specs/combat_arms/
//! lobby.md F, S, G, X, M on the engine): a server with the lobby on and
//! two clients on `NetSim`'s in-memory link, Combat Arms' simulation on
//! the greybox. A client's requests reach the server's lobby, its notices
//! come back, the players wait held out of the match until their room's
//! "go", go in on their room team with the kit the lobby built, and are
//! back in the room when the match is over.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Connecting, LocalPlayer, Team as MatchTeam};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::weapon::{Inventory, Weapon};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        lobby::{
            CaLobbyClient, CaLobbyHost,
            data::LobbyData,
            host::{self, CaInMatch},
            room::*,
        },
        rules::{CaMatch, CaRuleSettings, Phase},
    },
    harness::CaSim,
};

/// A server with the lobby on (a quick tick and a short result) and two
/// clients, joined.
fn joined() -> NetSim {
    let link = LinkConditions {
        latency: Duration::ZERO,
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 21, 2, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(TICK_INTERVAL);
    {
        let mut host = sim.server.app.world_mut().resource_mut::<CaLobbyHost>();
        host.enabled = true;
        host.tick_secs = 0.1;
        host.result_secs = 0.3;
    }
    sim.until_joined(600);
    sim.ticks(120);
    sim
}

fn client(sim: &mut NetSim, i: usize) -> Mut<'_, CaLobbyClient> {
    sim.clients[i].app.world_mut().resource_mut::<CaLobbyClient>()
}

fn server_host(sim: &mut NetSim) -> Mut<'_, CaLobbyHost> {
    sim.server.app.world_mut().resource_mut::<CaLobbyHost>()
}

fn create_spec(mode: &str, players: u32, arms: [u8; 2]) -> CreateSpec {
    let data = LobbyData::built_in();
    let m = data.mode_by_key(mode).unwrap();
    CreateSpec {
        name: "Net room".into(),
        password: String::new(),
        mode: m.id,
        pure_combat: false,
        map: None,
        goal: m.goal_default(),
        players,
        arms,
    }
}

/// Client 0 hosts a room; client 1 enters it; both are in.
fn room(sim: &mut NetSim, mode: &str, arms: [u8; 2]) -> RoomId {
    client(sim, 0).0.send(Request::Create(create_spec(mode, 16, arms)));
    sim.ticks(30);
    let room = client(sim, 0).0.room.as_ref().expect("client 0 is in its room").id;
    assert!(
        client(sim, 1).0.list.find(room).is_some(),
        "the other client's list has the new room"
    );
    client(sim, 1).0.enter(room, "");
    sim.ticks(30);
    room
}

/// Both players' characters on the server.
fn characters(sim: &mut NetSim) -> [Entity; 2] {
    [sim.character_of(0).unwrap(), sim.character_of(1).unwrap()]
}

fn start_match(sim: &mut NetSim) {
    client(sim, 1).0.ready(Ok(()));
    sim.ticks(10);
    client(sim, 0).0.start(Ok(()));
    let started = sim.until(600, |s| {
        s.server
            .app
            .world()
            .resource::<CaLobbyHost>()
            .active
            .as_ref()
            .is_some_and(|a| a.went)
    });
    assert!(started, "the countdown ran and the room went in game");
    sim.ticks(10);
}

#[test]
fn clients_say_hello_and_get_the_room_list() {
    let mut sim = joined();
    for i in 0..2 {
        let c = client(&mut sim, i);
        assert!(c.0.active, "client {i} heard the lobby");
    }
    let ids = [client(&mut sim, 0).0.me, client(&mut sim, 1).0.me];
    assert_ne!(ids[0], ids[1]);
    assert!(
        ids.iter().all(|i| *i != 0),
        "remote players are told their transport id"
    );
    assert_eq!(server_host(&mut sim).lobby.players().len(), 2);
}

#[test]
fn players_wait_out_of_the_match_until_go() {
    let mut sim = joined();
    for c in characters(&mut sim) {
        let w = sim.server.app.world();
        assert!(w.get::<Connecting>(c).is_some(), "held out of the match");
        assert!(w.get::<CaInMatch>(c).is_none());
    }
}

#[test]
fn browse_create_join_ready_start_go_and_back_to_the_room() {
    let mut sim = joined();
    let room = room(&mut sim, "TDM", [0, 0]);
    // Both see the room, the creator as host on Alpha, the joiner on
    // Bravo.
    let (a, b) = (client(&mut sim, 0).0.me, client(&mut sim, 1).0.me);
    {
        let c0 = client(&mut sim, 0);
        let r = c0.0.room.as_ref().unwrap();
        assert_eq!(r.host, a);
        assert_eq!(r.seat(a).unwrap().team, Team::Alpha);
        assert_eq!(r.seat(b).unwrap().team, Team::Bravo);
    }
    // The list on both clients shows 2/16.
    assert_eq!(client(&mut sim, 1).0.list.rows()[0].players, "2/16");
    // A chat line reaches the room.
    client(&mut sim, 1).0.say("hello room");
    sim.ticks(10);
    assert!(
        client(&mut sim, 0)
            .0
            .chat
            .iter()
            .any(|l| l.text.ends_with("hello room"))
    );

    start_match(&mut sim);
    // The room's settings are the match's.
    let rules = sim.server.app.world().resource::<CaRuleSettings>().clone();
    assert_eq!(rules.mode, "TDM");
    // Both characters are in the match on their room teams, not held.
    let [c0, c1] = characters(&mut sim);
    let w = sim.server.app.world();
    assert!(w.get::<CaInMatch>(c0).is_some() && w.get::<CaInMatch>(c1).is_some());
    assert!(w.get::<Connecting>(c0).is_none() && w.get::<Connecting>(c1).is_none());
    assert_eq!(w.get::<MatchTeam>(c0).unwrap().0, 1);
    assert_eq!(w.get::<MatchTeam>(c1).unwrap().0, 2);
    // Each client got its go; the room is in game on both.
    for i in 0..2 {
        let c = client(&mut sim, i);
        assert!(c.0.go.is_some(), "client {i} got go");
        assert_eq!(c.0.room.as_ref().unwrap().state, RoomState::InGame);
        assert_eq!(c.0.seat().unwrap().state, PlayerState::InGame);
    }
    assert_eq!(client(&mut sim, 0).0.list.rows()[0].status, "In game");

    // The match ends: the room closes, then everyone is back in the same
    // room, waiting and held out again.
    sim.server.app.world_mut().resource_mut::<CaMatch>().phase = Phase::MatchOver;
    let back = sim.until(600, |s| {
        s.clients[0]
            .app
            .world()
            .resource::<CaLobbyClient>()
            .0
            .room
            .as_ref()
            .is_some_and(|r| r.state == RoomState::Waiting)
    });
    assert!(back, "the room is waiting again");
    sim.ticks(10);
    for i in 0..2 {
        let c = client(&mut sim, i);
        let r = c.0.room.as_ref().unwrap();
        assert_eq!(r.id, room, "the same room");
        assert!(
            r.seats.iter().all(|s| s.state == PlayerState::Waiting),
            "ready states are lost"
        );
    }
    let [c0, c1] = characters(&mut sim);
    let w = sim.server.app.world();
    assert!(w.get::<CaInMatch>(c0).is_none() && w.get::<CaInMatch>(c1).is_none());
    assert!(w.get::<Connecting>(c0).is_some() && w.get::<Connecting>(c1).is_some());
    assert!(server_host(&mut sim).active.is_none());
}

#[test]
fn the_room_arms_option_filters_the_kit_the_player_carries() {
    let mut sim = joined();
    // Pistols Only.
    room(&mut sim, "TDM", [1, 0]);
    start_match(&mut sim);
    sim.ticks(10);
    let [c0, c1] = characters(&mut sim);
    for c in [c0, c1] {
        let w = sim.server.app.world();
        let inv = w.get::<Inventory>(c).expect("their inventory was given");
        let mut ids: Vec<String> = inv
            .weapons
            .iter()
            .filter_map(|e| w.get::<Weapon>(*e))
            .map(|wp| wp.id.to_string())
            .collect();
        ids.sort();
        assert_eq!(
            ids,
            ["combat_arms:beretta_m92fs", "combat_arms:m9_knife"],
            "the pistol and the knife only"
        );
    }
}

#[test]
fn a_player_leaving_the_room_and_a_dropped_connection() {
    let mut sim = joined();
    let room = room(&mut sim, "TDM", [0, 0]);
    client(&mut sim, 1).0.send(Request::Leave);
    sim.ticks(20);
    assert!(client(&mut sim, 1).0.room.is_none());
    assert_eq!(client(&mut sim, 0).0.room.as_ref().unwrap().seats.len(), 1);
    assert_eq!(client(&mut sim, 1).0.list.find(room).unwrap().users, 1);
    // The host's connection drops: the room is gone from the lobby.
    sim.remove_client(0);
    sim.ticks(30);
    assert!(server_host(&mut sim).lobby.room(room).is_none());
}

#[test]
fn a_second_room_waits_while_one_match_runs() {
    let mut sim = joined();
    room(&mut sim, "TDM", [0, 0]);
    start_match(&mut sim);
    // A third party's room cannot get a game server on this process: its
    // launch is not served, and it times out as the spec's does.
    let mut host = server_host(&mut sim);
    host.drop_leavers = false;
    host.lobby.request(900, Request::Hello { name: "X".into() }).unwrap();
    host.lobby.request(901, Request::Hello { name: "Y".into() }).unwrap();
    let data = LobbyData::built_in();
    let m = data.mode_by_key("FFA").unwrap();
    host.lobby
        .request(
            900,
            Request::Create(CreateSpec {
                name: "second".into(),
                password: String::new(),
                mode: m.id,
                pure_combat: false,
                map: None,
                goal: 30,
                players: 8,
                arms: [0, 0],
            }),
        )
        .unwrap();
    let r2 = host.lobby.room_of(900).unwrap();
    host.lobby
        .request(
            901,
            Request::Enter {
                room: r2,
                password: String::new(),
            },
        )
        .unwrap();
    host.lobby.request(901, Request::Ready(true)).unwrap();
    host.lobby.request(900, Request::Start).unwrap();
    drop(host);
    sim.ticks(40);
    assert_eq!(
        server_host(&mut sim).lobby.room(r2).unwrap().state,
        RoomState::Starting,
        "still waiting for a game server"
    );
    assert_eq!(server_host(&mut sim).active.as_ref().unwrap().room != r2, true);
}

#[test]
fn a_game_of_one_serves_its_own_player_as_player_zero() {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    {
        let mut host = sim.app.world_mut().resource_mut::<CaLobbyHost>();
        host.enabled = true;
        host.tick_secs = 0.1;
        host.drop_leavers = false;
    }
    let me = sim.spawn_character(chimera_world::greybox::SPAWNS[0], combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(0.5);
    assert!(
        sim.app.world().resource::<CaLobbyClient>().0.active,
        "it heard its own lobby"
    );
    assert_eq!(sim.app.world().resource::<CaLobbyClient>().0.me, 0);
    assert!(
        sim.app.world().get::<Connecting>(me).is_some(),
        "held out until its room starts"
    );
    // Another player (no character) is in the lobby.
    {
        let mut host = sim.app.world_mut().resource_mut::<CaLobbyHost>();
        host.lobby.request(7, Request::Hello { name: "Other".into() }).unwrap();
    }
    sim.app
        .world_mut()
        .resource_mut::<CaLobbyClient>()
        .0
        .send(Request::Create(create_spec("TDM", 16, [0, 0])));
    sim.seconds(0.5);
    let room = sim.app.world().resource::<CaLobbyClient>().0.room.as_ref().unwrap().id;
    {
        let mut host = sim.app.world_mut().resource_mut::<CaLobbyHost>();
        host.lobby
            .request(
                7,
                Request::Enter {
                    room,
                    password: String::new(),
                },
            )
            .unwrap();
        host.lobby.request(7, Request::Ready(true)).unwrap();
    }
    sim.seconds(0.3);
    sim.app.world_mut().resource_mut::<CaLobbyClient>().0.start(Ok(()));
    sim.seconds(2.0);
    let w = sim.app.world();
    assert!(w.get::<CaInMatch>(me).is_some(), "went in");
    assert!(w.get::<Connecting>(me).is_none());
    assert_eq!(w.get::<MatchTeam>(me).unwrap().0, 1);
    assert!(w.resource::<CaLobbyClient>().0.go.is_some());
    assert_eq!(host::in_match(sim.app.world_mut()).len(), 1);
}
