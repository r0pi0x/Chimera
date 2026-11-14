//! Session wins over the network (`minigame::wins`, ugg-ui M6): a server
//! with the pre-match lobby, two clients (Ann, Bob) and a bot (Cy) on a
//! three-rung ladder. Ann wins the first match: the server counts it, the
//! results every client gets carry it (her row's `wins`, this match's +1
//! counted) and her character's `MatchesWon` reaches both clients. The
//! next match comes on another map: the lobby's rows carry the counts
//! (`LobbyRow::wins`) and the characters keep them. The bot wins the
//! second, counted by its name. Ann's client then leaves and joins again
//! (a new connection): her new character has her count back. Ticks only
//! (64 a second); the map change is the test's (as `prematch_lobby`'s).

use std::time::Duration;

use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::{
    character::character_bundle,
    console::{Console, ConsoleAppExt},
    core::{Bot, Damage, MapLoads, DamageKind, Dead, God, Hitgroup, Team},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    minigame_net::{self, MinigameNetPlugin},
    net::{NetEvent, NetSettings, client::Joined, maps::map_name, memory::LinkConditions},
    prematch::{self, NetLobby},
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{self, InMinigame, MatchResults, MinigameAppExt, MinigamePlugin, PreMatch, wins::{MatchWins, MatchesWon}},
    mode::{Match, Phase},
    movement::placeholder,
    testkit,
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{LoadedMapName, MapFile},
};

const DEF: &str = r#"
id = "kit:wins"
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

const ID: &str = "kit:wins";
/// Ticks a second.
const HZ: u64 = 64;
const BOT: &str = "Bot Cy";

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout {
        movement: placeholder::ID,
    });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_wins.toml", DEF)]).expect("the definition reads");
    prematch::enable(app);
}

fn named(name: &'static str) -> impl FnOnce(&mut World) {
    move |w| w.resource_mut::<NetSettings>().name = name.into()
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

/// A client loads what its handshake asks for, 30 frames later.
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
    let Some(FakeLoad(map, left)) = world.remove_resource::<FakeLoad>() else { return };
    if left > 0 {
        world.insert_resource(FakeLoad(map, left - 1));
        return;
    }
    loaded(world, &map);
    world.get_resource_or_init::<MapLoads>().0 += 1;
}

fn add_client(sim: &mut NetSim, name: &'static str) -> usize {
    let i = sim.add_client(named(name));
    sim.clients[i].app.add_systems(Update, client_loads);
    i
}

fn joined(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().contains_resource::<Joined>()
}

/// Every character in the match alive on the server and the round live.
fn playing(sim: &mut NetSim, n: usize) -> bool {
    let w = sim.server.app.world_mut();
    let alive = w.query_filtered::<Option<&Dead>, With<InMinigame>>().iter(w).filter(|d| d.is_none()).count();
    alive == n && w.resource::<Match>().phase == Phase::Live && !w.contains_resource::<PreMatch>()
}

/// The server's character named `name`.
fn by_name(sim: &mut NetSim, name: &str) -> Option<Entity> {
    let w = sim.server.app.world_mut();
    w.query::<(Entity, &Name)>().iter(w).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

/// The bot, spawned again (by its name) when a new game took it.
fn bot(sim: &mut NetSim) -> Entity {
    if let Some(e) = by_name(sim, BOT) {
        return e;
    }
    let w = sim.server.app.world_mut();
    let e = w.spawn(character_bundle(Transform::from_xyz(0.0, 1.0, 0.0), Team(1))).id();
    w.entity_mut(e).insert((Name::new(BOT), Bot));
    e
}

/// `a` wins the match: three kills climb the three rungs (the victims
/// taking turns).
fn win(sim: &mut NetSim, a: Entity, victims: [Entity; 2]) {
    for k in 0..3 {
        let b = victims[k % 2];
        let ready = |s: &mut NetSim| {
            let w = s.server.app.world();
            [a, b].iter().all(|e| w.get::<Dead>(*e).is_none() && w.get::<God>(*e).is_none())
        };
        assert!(sim.until(10 * HZ, ready), "both in play");
        let at = sim.server.position(b);
        sim.server.app.world_mut().write_message(Damage {
            target: b,
            attacker: Some(a),
            amount: 500.0,
            point: at,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: None,
            force: Vec3::ZERO,
        });
        sim.ticks(5);
    }
    let won = |s: &mut NetSim| s.server.app.world().get_resource::<MatchResults>().is_some_and(|r| r.winner().is_some());
    assert!(sim.until(10 * HZ, won), "the match is decided");
}

/// Client `i`'s copy of the character named `name`: its `MatchesWon`.
fn seen_wins(sim: &mut NetSim, i: usize, name: &str) -> Option<u32> {
    let w = sim.clients[i].app.world_mut();
    let e = w.query::<(Entity, &chimera_net::net::NetCharacter)>().iter(w).find(|(_, c)| c.name == name).map(|(e, _)| e)?;
    w.get::<MatchesWon>(e).map(|m| m.0)
}

fn client_lobby(sim: &mut NetSim, i: usize) -> Option<NetLobby> {
    let w = sim.clients[i].app.world_mut();
    w.query::<&NetLobby>().iter(w).next().cloned()
}

#[test]
fn session_wins_survive_a_map_change_and_a_rejoin() {
    let mut sim = NetSim::new(
        SimBasePlugins,
        LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 },
        51,
        0,
        setup,
    );
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby_max_wait 90");
    sim.server.app.add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        w.insert_resource(LoadedMapName(map.clone()));
        w.insert_resource(FakeLoad(map, 30));
        Ok(None)
    });
    let ann = add_client(&mut sim, "Ann");
    let bob = add_client(&mut sim, "Bob");
    bot(&mut sim);
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    assert!(sim.until(900, |s| joined(s, ann) && joined(s, bob)), "both in");
    assert!(sim.until(10 * HZ, |s| playing(s, 3)), "the first match plays");

    // Match one: Ann's.
    let (a, b, c) = (by_name(&mut sim, "Ann").unwrap(), by_name(&mut sim, "Bob").unwrap(), bot(&mut sim));
    win(&mut sim, a, [b, c]);
    assert_eq!(sim.server.app.world().resource::<MatchWins>().of("Ann"), 1, "the server counted it");
    // The results every client gets: Ann's row with this match counted.
    let row = |s: &mut NetSim, i: usize, who: &str| {
        s.clients[i].app.world().get_resource::<MatchResults>().and_then(|r| r.rows.iter().find(|r| r.name == who).map(|r| (r.winner, r.wins)))
    };
    assert!(sim.until(HZ, |s| row(s, bob, "Ann") == Some((true, 1))), "Bob's results: {:?}", row(&mut sim, bob, "Ann"));
    assert_eq!(row(&mut sim, ann, "Bob"), Some((false, 0)));
    assert!(sim.until(HZ, |s| (0..2).all(|i| seen_wins(s, i, "Ann") == Some(1) && seen_wins(s, i, "Bob") == Some(0))), "MatchesWon replicated");

    // The next match on another map: the lobby shows the counts.
    minigame_net::change_match(sim.server.app.world_mut(), ID, Some("test:next")).expect("the change");
    let lobby_wins = |s: &mut NetSim| {
        client_lobby(s, bob).map(|l| {
            let mut w: Vec<(String, u32)> = l.rows.iter().map(|r| (r.name.clone(), r.wins)).collect();
            w.sort();
            w
        })
    };
    let want = vec![("Ann".to_string(), 1), ("Bob".to_string(), 0), (BOT.to_string(), 0)];
    assert!(sim.until(600, |s| lobby_wins(s).as_ref() == Some(&want)), "the lobby's rows: {:?}", lobby_wins(&mut sim));
    bot(&mut sim);
    let state = |s: &mut NetSim| {
        let w = s.server.app.world_mut();
        let alive: Vec<(String, bool)> = w.query_filtered::<(&Name, Option<&Dead>), With<InMinigame>>().iter(w).map(|(n, d)| (n.to_string(), d.is_none())).collect();
        (alive, w.resource::<Match>().phase, w.contains_resource::<PreMatch>(), client_lobby(s, 1))
    };
    assert!(sim.until(20 * HZ, |s| playing(s, 3)), "the second match plays: {:?}", state(&mut sim));
    assert_eq!(sim.server.app.world().resource::<LoadedMapName>().0, "test:next");
    assert!(sim.until(HZ, |s| seen_wins(s, ann, "Ann") == Some(1)), "kept over the map change");

    // Match two: the bot's, counted by its name.
    let (a, b, c) = (by_name(&mut sim, "Ann").unwrap(), by_name(&mut sim, "Bob").unwrap(), bot(&mut sim));
    win(&mut sim, c, [a, b]);
    let tally = sim.server.app.world().resource::<MatchWins>().clone();
    assert_eq!((tally.of("Ann"), tally.of("Bob"), tally.of(BOT)), (1, 0, 1));
    assert!(sim.until(HZ, |s| row(s, ann, BOT) == Some((true, 1)) && row(s, ann, "Ann") == Some((false, 1))));

    // Ann leaves and comes back on a new connection: her count is hers.
    sim.remove_client(ann);
    sim.ticks(HZ / 2);
    assert!(by_name(&mut sim, "Ann").is_none(), "her character went");
    let back = add_client(&mut sim, "Ann");
    assert!(sim.until(900, |s| joined(s, back)), "she is back");
    let her = |s: &mut NetSim| by_name(s, "Ann").and_then(|e| s.server.app.world().get::<MatchesWon>(e).map(|m| m.0));
    assert!(sim.until(10 * HZ, |s| her(s) == Some(1)), "the server's: {:?}", her(&mut sim));
    assert!(sim.until(2 * HZ, |s| seen_wins(s, back, "Ann") == Some(1) && seen_wins(s, back, BOT) == Some(1)), "her client sees them");
}
