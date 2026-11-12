//! Network play, slice 1 (docs/plans/active/multiplayer.md): a server and
//! clients in one process (`harness::NetSim`, an in-memory link with
//! seeded latency, jitter and loss) on the greybox. Clients join, the
//! server spawns their characters, everyone sees everyone move; a
//! disconnect removes the character; another version is refused; a
//! different copy of the map is refused.

use std::time::Duration;

use bevy::prelude::*;
use crate::stack::{
    core::{Health, LocalPlayer, NetRole, Team},
    games::cs_source::movement::{self as source, SourceMovementPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, NetSim},
    map::{LoadedMapName, MapFile},
    net::{self, LastDisconnect, NetCharacter, NetSettings, NetVersion, client::Joined, memory::LinkConditions},
    slots::Loadout,
};

fn greybox(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, SourceMovementPlugin))
        .insert_resource(Loadout { movement: source::ID });
}

fn lan() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::from_millis(10),
        loss: 0.0,
    }
}

/// The listen server's own player, as the game's local player.
fn host(sim: &mut NetSim) -> Entity {
    let e = sim.server.spawn_character(Vec3::new(6.0, 1.0, 0.0), source::ID);
    // A counter-terrorist, as the game's player starts.
    sim.server.app.world_mut().entity_mut(e).insert((LocalPlayer, Team(2)));
    e
}

fn pos(world: &World, e: Entity) -> Vec3 {
    world.get::<Transform>(e).unwrap().translation
}

/// Client `viewer`'s copy of the character owned by `owner`.
fn seen_by(sim: &mut NetSim, viewer: usize, owner: u64) -> Option<Entity> {
    NetSim::owned_by(&mut sim.clients[viewer], Some(owner))
}

#[test]
fn clients_join_and_see_each_other_move() {
    let mut sim = NetSim::new(CsSim, lan(), 1, 2, greybox);
    let host = host(&mut sim);
    sim.until_joined(200);

    // The server made a character per client, on both teams (the host is
    // a counter-terrorist: the first client joins the terrorists).
    let a = sim.character_of(0).expect("server character for client 1");
    let b = sim.character_of(1).expect("server character for client 2");
    let teams = |e| *sim.server.app.world().get::<Team>(e).unwrap();
    assert_eq!(teams(a), Team(1));
    assert_eq!(teams(b), Team(2));
    // Respawned by the rules at a spawn point once its client said it has
    // the map (a round trip), and drawn there by the others (in the past).
    sim.ticks(40);
    assert!(sim.server.app.world().get::<crate::stack::rules::Dead>(a).is_none());

    // Each client sees all three, its own as its local player.
    for i in 0..2 {
        let world = sim.clients[i].app.world_mut();
        let n = world.query::<&NetCharacter>().iter(world).count();
        assert_eq!(n, 3, "client {i} sees {n} characters");
        let me = sim.local_player(i).unwrap();
        let owner = sim.clients[i].app.world().get::<NetCharacter>(me).unwrap().owner;
        assert_eq!(owner, Some(sim.client_id(i)));
    }
    // Team and health replicate.
    let a_on_2 = seen_by(&mut sim, 1, 1).unwrap();
    assert_eq!(*sim.clients[1].app.world().get::<Team>(a_on_2).unwrap(), Team(1));
    assert_eq!(sim.clients[1].app.world().get::<Health>(a_on_2).unwrap().current, 1.0);

    // Client 1 walks forward: the server moves its character, client 2
    // sees it move, and so does client 1 (drawn from the server's state).
    let a_local = sim.local_player(0).unwrap();
    let start_server = pos(sim.server.app.world(), a);
    let start_seen = pos(sim.clients[1].app.world(), a_on_2);
    assert!(
        start_server.distance(start_seen) < 0.01,
        "{start_server} vs {start_seen}"
    );
    sim.clients[0]
        .app
        .world_mut()
        .get_mut::<crate::stack::core::Intent>(a_local)
        .unwrap()
        .move_axis = Vec2::Y;
    sim.ticks(64);
    sim.clients[0]
        .app
        .world_mut()
        .get_mut::<crate::stack::core::Intent>(a_local)
        .unwrap()
        .move_axis = Vec2::ZERO;
    // Stopped (friction), and others are drawn 0.1 s and the latency in
    // the past (`net::interp`).
    sim.ticks(80);
    let moved = pos(sim.server.app.world(), a).distance(start_server);
    assert!(moved > 2.0, "the server moved client 1's character {moved} m");
    let end_server = pos(sim.server.app.world(), a);
    for (viewer, e) in [(0, a_local), (1, a_on_2)] {
        let seen = pos(sim.clients[viewer].app.world(), e);
        assert!(
            seen.distance(end_server) < 0.01,
            "client {} sees it at {seen}, the server has {end_server}",
            viewer + 1
        );
    }

    // The host walks: both clients see it.
    let host_start = pos(sim.server.app.world(), host);
    sim.server.intent(host).move_axis = Vec2::X;
    sim.ticks(64);
    sim.server.intent(host).move_axis = Vec2::ZERO;
    sim.ticks(80);
    let host_end = pos(sim.server.app.world(), host);
    assert!(host_end.distance(host_start) > 2.0);
    for viewer in 0..2 {
        let e = seen_by(&mut sim, viewer, net::HOST_ID).unwrap();
        assert!(pos(sim.clients[viewer].app.world(), e).distance(host_end) < 0.01);
    }
}

#[test]
fn looks_replicate_to_others() {
    let mut sim = NetSim::new(CsSim, lan(), 2, 2, greybox);
    sim.until_joined(200);
    // The server spawns the character after the client sees it, and its
    // spawn facing replaces the client's look when it arrives: script
    // ours after the client has taken it.
    sim.until_looks_taken(200);
    let me = sim.local_player(0).unwrap();
    {
        let world = sim.clients[0].app.world_mut();
        let mut i = world.get_mut::<crate::stack::core::Intent>(me).unwrap();
        i.yaw = 1.0;
        i.pitch = -0.25;
        i.crouch = true;
    }
    sim.ticks(30);
    let other = seen_by(&mut sim, 1, 1).unwrap();
    let world = sim.clients[1].app.world();
    let i = world.get::<crate::stack::core::Intent>(other).unwrap();
    // Within the network's 16-bit angles (`net::quant`).
    assert!(
        (i.yaw - 1.0).abs() < 1e-4 && (i.pitch + 0.25).abs() < 1e-4,
        "{} {}",
        i.yaw,
        i.pitch
    );
    assert!(world.get::<crate::stack::core::MovementState>(other).unwrap().crouching);
}

#[test]
fn a_disconnect_removes_the_character_everywhere() {
    let mut sim = NetSim::new(CsSim, lan(), 3, 2, greybox);
    sim.until_joined(200);
    assert!(sim.character_of(1).is_some());
    net::disconnect(sim.clients[1].app.world_mut(), "Disconnect by user.");
    sim.ticks(30);
    assert!(sim.character_of(1).is_none(), "the server dropped client 2's character");
    assert!(seen_by(&mut sim, 0, 2).is_none(), "client 1 no longer sees it");
    assert!(seen_by(&mut sim, 0, 1).is_some(), "client 1 still sees itself");
    // The leaver is back to single player with nothing from the server.
    let world = sim.clients[1].app.world_mut();
    assert_eq!(*world.resource::<NetRole>(), NetRole::Standalone);
    assert_eq!(world.query::<&NetCharacter>().iter(world).count(), 0);

    // The server stopping drops the rest.
    net::disconnect(sim.server.app.world_mut(), "Server shutting down.");
    sim.ticks(10);
    let world = sim.clients[0].app.world_mut();
    assert_eq!(*world.resource::<NetRole>(), NetRole::Standalone);
    assert_eq!(world.query::<&NetCharacter>().iter(world).count(), 0);
}

#[test]
fn another_version_is_refused_with_a_reason() {
    let mut sim = NetSim::new(CsSim, lan(), 4, 0, greybox);
    sim.add_client(|w| w.insert_resource(NetVersion("0.0.1/net0".into())));
    sim.ticks(60);
    let world = sim.clients[0].app.world();
    assert_eq!(*world.resource::<NetRole>(), NetRole::Standalone);
    let reason = &world.resource::<LastDisconnect>().0;
    assert!(reason.contains("version 0.0.1/net0"), "{reason}");
    assert!(!world.contains_resource::<Joined>());
    assert!(sim.character_of(0).is_none());
}

#[test]
fn a_full_server_refuses() {
    let mut sim = NetSim::new(CsSim, lan(), 5, 0, greybox);
    host(&mut sim);
    // The host and one more.
    sim.server.app.world_mut().resource_mut::<NetSettings>().maxplayers = 2;
    sim.add_client(|_| {});
    sim.until_joined(200);
    sim.add_client(|_| {});
    sim.ticks(60);
    let reason = &sim.clients[1].app.world().resource::<LastDisconnect>().0;
    assert_eq!(reason, "Server is full.");
    assert!(sim.character_of(0).is_some() && sim.character_of(1).is_none());
}

/// Both sides "loaded" `cs_source:test_map` with these file hashes.
fn on_map(world: &mut World, hash: u8) {
    world.insert_resource(LoadedMapName("cs_source:test_map".into()));
    world.insert_resource(MapFile {
        name: "test_map".into(),
        hash: Some([hash; 32]),
    });
}

#[test]
fn the_map_must_be_the_same_file() {
    let mut sim = NetSim::new(CsSim, lan(), 6, 0, greybox);
    on_map(sim.server.app.world_mut(), 7);
    sim.add_client(|w| on_map(w, 7));
    sim.add_client(|w| on_map(w, 8));
    sim.ticks(60);
    assert!(sim.clients[0].app.world().contains_resource::<Joined>());
    let world = sim.clients[1].app.world();
    assert_eq!(*world.resource::<NetRole>(), NetRole::Standalone);
    let reason = &world.resource::<LastDisconnect>().0;
    assert!(reason.contains("differs from the server's"), "{reason}");
    sim.ticks(10);
    assert!(sim.character_of(1).is_none());
}

#[test]
fn a_lossy_link_still_converges() {
    let conditions = LinkConditions {
        latency: Duration::from_millis(60),
        jitter: Duration::from_millis(30),
        loss: 0.2,
    };
    let mut sim = NetSim::new(CsSim, conditions, 7, 2, greybox);
    sim.until_joined(600);
    let me = sim.local_player(0).unwrap();
    sim.clients[0]
        .app
        .world_mut()
        .get_mut::<crate::stack::core::Intent>(me)
        .unwrap()
        .move_axis = Vec2::Y;
    sim.ticks(64);
    sim.clients[0]
        .app
        .world_mut()
        .get_mut::<crate::stack::core::Intent>(me)
        .unwrap()
        .move_axis = Vec2::ZERO;
    sim.ticks(100);
    let a = sim.character_of(0).unwrap();
    let truth = pos(sim.server.app.world(), a);
    let seen = seen_by(&mut sim, 1, 1).unwrap();
    let there = pos(sim.clients[1].app.world(), seen);
    assert!(there.distance(truth) < 0.01, "{there} vs {truth}");
    assert!(sim.link.lock().unwrap().lost > 0, "the link dropped packets");
}

/// The real transport: netcode over UDP on loopback (`ip`), on a port the
/// system picks (never 27015: a CS:S server may hold it).
#[test]
fn netcode_over_loopback() {
    let make = || {
        crate::stack::harness::Sim::with(CsSim, |app| {
            app.add_plugins(net::NetPlugin);
            greybox(app);
        })
    };
    let mut server = make();
    {
        let world = server.app.world_mut();
        let mut s = world.resource_mut::<NetSettings>();
        s.ip = std::net::Ipv4Addr::LOCALHOST.into();
        s.hostport = 0;
        s.maxplayers = 4;
    }
    let addr = net::server::listen(server.app.world_mut()).expect("listen");
    assert_ne!(addr.port(), 0);
    assert!(addr.ip().is_loopback(), "{addr}");
    let mut client = make();
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(client.app.world_mut(), to).expect("connect");
    let mut joined = false;
    for _ in 0..400 {
        server.app.update();
        client.app.update();
        std::thread::sleep(Duration::from_millis(1));
        let world = client.app.world_mut();
        let mine = world.query_filtered::<(), With<LocalPlayer>>().iter(world).count();
        if world.contains_resource::<Joined>() && mine == 1 {
            joined = true;
            break;
        }
    }
    assert!(joined, "joined over UDP");
    let status = net::status(server.app.world_mut());
    assert!(status.contains("players : 1 (4 max)"), "{status}");
    // Leaving reaches the server (netcode's disconnect packet).
    net::disconnect(client.app.world_mut(), "Disconnect by user.");
    let mut gone = false;
    for _ in 0..200 {
        server.app.update();
        std::thread::sleep(Duration::from_millis(1));
        let world = server.app.world_mut();
        if world.query::<&NetCharacter>().iter(world).count() == 0 {
            gone = true;
            break;
        }
    }
    assert!(gone, "the server dropped the character");
    net::disconnect(server.app.world_mut(), "done");
}

/// A UDP server and a client joined to it on loopback.
fn udp_pair() -> (crate::stack::harness::Sim, crate::stack::harness::Sim) {
    let make = || {
        crate::stack::harness::Sim::with(CsSim, |app| {
            app.add_plugins(net::NetPlugin);
            greybox(app);
        })
    };
    let mut server = make();
    {
        let world = server.app.world_mut();
        let mut s = world.resource_mut::<NetSettings>();
        s.ip = std::net::Ipv4Addr::LOCALHOST.into();
        s.hostport = 0;
        s.maxplayers = 4;
    }
    let addr = net::server::listen(server.app.world_mut()).expect("listen");
    let mut client = make();
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(client.app.world_mut(), to).expect("connect");
    for _ in 0..400 {
        server.app.update();
        client.app.update();
        std::thread::sleep(Duration::from_millis(1));
        if client.app.world().contains_resource::<Joined>() {
            return (server, client);
        }
    }
    panic!("didn't join over UDP");
}

/// Playtest 2026-10-10: when the server closed, clients waited for the
/// timeout. Every way a server ends (its `quit`, the host's `disconnect`
/// or menu, the app exiting: window closed, Ctrl+C) hangs up on its
/// clients at once, and they have CS:S's "Server shutting down": as the
/// reconnect reason, or (`cl_autoreconnect 0`) in the Disconnected dialog.
#[test]
fn clients_hear_at_once_that_the_server_shut_down() {
    use crate::stack::net::client::{JoinFailure, JoinProgress};
    let ends: [(&str, fn(&mut World)); 3] = [
        // The game's `quit` (and the dedicated server's).
        ("quit", |w| {
            net::disconnect(w, "Quit.");
            w.write_message(AppExit::Success);
        }),
        ("disconnect", |w| net::disconnect(w, "Disconnect by user.")),
        ("app exit", |w| {
            w.write_message(AppExit::Success);
        }),
    ];
    use crate::stack::net::reconnect::{ReconnectSettings, Reconnecting};
    for auto in [true, false] {
        for (how, end) in ends {
            let (mut server, mut client) = udp_pair();
            client.app.world_mut().resource_mut::<ReconnectSettings>().enabled = u32::from(auto);
            end(server.app.world_mut());
            server.app.update();
            let mut gone = false;
            for _ in 0..50 {
                client.app.update();
                std::thread::sleep(Duration::from_millis(2));
                if *client.app.world().resource::<crate::stack::core::NetRole>() == crate::stack::core::NetRole::Standalone {
                    gone = true;
                    break;
                }
            }
            let what = format!("{how}, cl_autoreconnect {}", u32::from(auto));
            assert!(gone, "{what}: the client was told at once (not after the 30 s timeout)");
            if auto {
                // Reconnecting instead of the failure box; the words are its reason.
                let r = client.app.world().get_resource::<Reconnecting>();
                assert_eq!(
                    r.map(|r| r.reason.as_str()),
                    Some("Server shutting down"),
                    "{what}: CS:S's words as the reconnect reason"
                );
            } else {
                assert!(client.app.world().get_resource::<Reconnecting>().is_none(), "{what}");
                let p = client.app.world().resource::<JoinProgress>().clone();
                assert_eq!(
                    p.failure,
                    Some((JoinFailure::Dropped, "Server shutting down".to_string())),
                    "{what}: CS:S's words in the Disconnected dialog"
                );
            }
        }
    }
}

/// A client that goes quiet for 20 s (putting a big map in on a slow or
/// busy machine) is still connected after: both ends wait 30 s
/// (`net::client::TIMEOUT_SECONDS`, Source's `cl_timeout`). With
/// netcode's fixed 15 s the live soak's clients were all dropped after
/// a `changelevel` that took one 17 s.
#[test]
fn a_client_quiet_for_20_s_stays_connected() {
    let make = || {
        crate::stack::harness::Sim::with(CsSim, |app| {
            app.add_plugins(net::NetPlugin);
            greybox(app);
        })
    };
    let mut server = make();
    {
        let world = server.app.world_mut();
        let mut s = world.resource_mut::<NetSettings>();
        s.ip = std::net::Ipv4Addr::LOCALHOST.into();
        s.hostport = 0;
        s.maxplayers = 4;
    }
    let addr = net::server::listen(server.app.world_mut()).expect("listen");
    let mut client = make();
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(client.app.world_mut(), to).expect("connect");
    let joined = |client: &mut crate::stack::harness::Sim| {
        let world = client.app.world_mut();
        world.contains_resource::<Joined>() && world.query_filtered::<(), With<LocalPlayer>>().iter(world).count() == 1
    };
    let mut ok = false;
    for _ in 0..400 {
        server.app.update();
        client.app.update();
        std::thread::sleep(Duration::from_millis(1));
        if joined(&mut client) {
            ok = true;
            break;
        }
    }
    assert!(ok, "joined over UDP");
    // 20 s of the server's time without a word from the client.
    let frames = (20.0 * crate::stack::DEFAULT_TICK_HZ) as u32;
    for _ in 0..frames {
        server.app.update();
    }
    for _ in 0..100 {
        server.app.update();
        client.app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(joined(&mut client), "still in the game: {:?}", client.app.world().get_resource::<LastDisconnect>());
    let status = net::status(server.app.world_mut());
    assert!(status.contains("players : 1 (4 max)"), "{status}");
    net::disconnect(client.app.world_mut(), "Disconnect by user.");
    net::disconnect(server.app.world_mut(), "done");
}

/// `net_fakelag` on a client's UDP transport: the round trip grows by it,
/// and the game still joins and predicts.
#[test]
fn fake_lag_delays_what_the_client_receives() {
    let make = || {
        crate::stack::harness::Sim::with(CsSim, |app| {
            app.add_plugins(net::NetPlugin);
            greybox(app);
        })
    };
    let mut server = make();
    {
        let world = server.app.world_mut();
        let mut s = world.resource_mut::<NetSettings>();
        s.ip = std::net::Ipv4Addr::LOCALHOST.into();
        s.hostport = 0;
        s.maxplayers = 4;
    }
    let addr = net::server::listen(server.app.world_mut()).expect("listen");
    let mut client = make();
    client.app.world_mut().resource_mut::<net::udp::FakeLag>().lag = 80.0;
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(client.app.world_mut(), to).expect("connect");
    let mut rtt = 0.0;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        server.app.update();
        client.app.update();
        std::thread::sleep(Duration::from_millis(2));
        let world = client.app.world();
        let predicting = world.resource::<net::predict::CommandClock>().tick.is_some();
        rtt = world
            .get_resource::<bevy_replicon_renet::RenetClient>()
            .map_or(0.0, |c| c.rtt());
        if predicting && rtt > 0.08 {
            break;
        }
    }
    assert!(rtt > 0.08, "round trip {rtt} s with 80 ms of fake lag");
    assert!(
        client
            .app
            .world()
            .resource::<net::predict::CommandClock>()
            .tick
            .is_some()
    );
    net::disconnect(client.app.world_mut(), "done");
    net::disconnect(server.app.world_mut(), "done");
}

/// A value that changes once on an entity whose other values change every
/// tick (a body) survives a lost update, on a lossy link. replicon
/// (0.44.3) holds back mutations until the update they follow arrives
/// (reliable, resent after a loss), acknowledging them on arrival; when
/// the update comes it applies the newest held-back mutation of each
/// entity and drops the older ones as outdated. The newer ones, built
/// after the acknowledgement came back, didn't carry the value: it was
/// lost for good. The soak (games/cs_source/tests/it/heavy/net_soak.rs) saw lossy clients
/// keep a score one kill short, a dead player's last health, the old
/// scores after a map change. The client now applies the older held-back
/// values too, each component in tick order (`net::ordered`). Each cycle
/// here: a shape change (an update),
/// a value change two ticks later, a value changing every tick all along.
#[test]
fn a_value_survives_held_back_mutations() {
    let link = LinkConditions {
        latency: Duration::from_millis(60),
        jitter: Duration::from_millis(10),
        loss: 0.1,
    };
    let mut sim = NetSim::new(CsSim, link, 4243, 1, greybox);
    sim.until_joined(600);
    let e = sim
        .server
        .app
        .world_mut()
        .spawn((
            bevy_replicon::prelude::Replicated,
            net::NetScore::default(),
            Health::default(),
            Team(1),
        ))
        .id();
    sim.ticks(30);
    let mut stale = Vec::new();
    for cycle in 0..40 {
        for t in 0..60 {
            {
                let w = sim.server.app.world_mut();
                // The body: a new value every tick.
                w.get_mut::<Health>(e).unwrap().current = (cycle * 60 + t) as f32;
                if t == 0 {
                    if w.get::<Team>(e).is_some() {
                        w.entity_mut(e).remove::<Team>();
                    } else {
                        w.entity_mut(e).insert(Team(1));
                    }
                }
                if t == 2 {
                    w.get_mut::<net::NetScore>(e).unwrap().kills += 1;
                }
            }
            sim.step();
        }
        let want = sim.server.app.world().get::<net::NetScore>(e).unwrap().kills;
        let w = sim.clients[0].app.world_mut();
        let got: Vec<i32> = w
            .query_filtered::<&net::NetScore, Without<NetCharacter>>()
            .iter(w)
            .map(|s| s.kills)
            .collect();
        if got != vec![want] {
            stale.push((cycle, want, got));
        }
    }
    println!("a cycle behind: {stale:?}");
    assert!(stale.is_empty(), "the client was left a value behind: {stale:?}");
}


/// Each client gets updates at its own `cl_updaterate` (within the
/// server's `sv_minupdaterate` and `sv_maxupdaterate`) and no more than
/// its `rate` allows (`net::server::UpdatePacing`): a client asking for
/// 20 hears a tick about every 50 ms, one asking for 66 every tick, one
/// whose `rate` is below what its updates take is choked (updates
/// skipped); all predict without errors and see the host where the
/// server has it.
#[test]
fn updates_follow_each_clients_rate() {
    let mut sim = NetSim::new(CsSim, lan(), 61, 0, greybox);
    let host = host(&mut sim);
    sim.server
        .app
        .world_mut()
        .resource_mut::<crate::stack::console::Console>()
        .submit("sv_minrate 0");
    for (updaterate, rate) in [(20.0, 30000.0), (66.0, 1e6), (66.0, 1500.0)] {
        sim.add_client(move |w| {
            *w.resource_mut::<net::predict::RateSettings>() = net::predict::RateSettings { updaterate, rate };
        });
    }
    sim.until_joined(600);
    // The host walks.
    let walk = |sim: &mut NetSim, axis: Vec2| {
        sim.server.app.world_mut().get_mut::<crate::stack::core::Intent>(host).unwrap().move_axis = axis;
    };
    walk(&mut sim, Vec2::Y);
    sim.ticks(320);
    let sent = |sim: &NetSim, i: usize| {
        let id = sim.client_id(i);
        sim.link.lock().unwrap().bytes_to.get(&id).copied().unwrap_or(0)
    };
    let before: Vec<u64> = (0..3).map(|i| sent(&sim, i)).collect();
    for c in &mut sim.clients {
        *c.app.world_mut().resource_mut::<net::predict::NetGraph>() = default();
    }
    sim.ticks(320);
    let world = sim.server.app.world_mut();
    let pacing: Vec<(u64, net::server::UpdatePacing)> = world
        .query::<(&net::server::UpdatePacing, &bevy_replicon::shared::backend::connected_client::NetworkId)>()
        .iter(world)
        .map(|(p, id)| (id.get(), p.clone()))
        .collect();
    for i in 0..3 {
        let g = sim.clients[i].app.world().resource::<net::predict::NetGraph>().clone();
        let bytes = sent(&sim, i) - before[i];
        let p = &pacing.iter().find(|(id, _)| *id == sim.client_id(i)).unwrap().1;
        println!(
            "client {i}: updates every {:.1} ms, {} sent, {} choked, {:.2} KB/s, prediction {} wrong of {}",
            g.update_ms,
            p.sent,
            p.choked,
            bytes as f64 / 5.0 / 1000.0,
            g.errors,
            g.checked
        );
        match i {
            0 => {
                // 20 for client 1's 64 (one a tick).
                let ratio = p.sent as f64 / pacing.iter().find(|(id, _)| *id == sim.client_id(1)).unwrap().1.sent as f64;
                assert!((ratio - 20.0 / 64.0).abs() < 0.02, "client 0: {ratio} of client 1's updates");
                assert!((g.update_ms - 50.0).abs() < 8.0, "client 0: every {} ms", g.update_ms);
            }
            1 => assert!(p.choked == 0 && g.update_ms < 25.0, "client 1: every {} ms", g.update_ms),
            _ => {
                assert!(p.choked > 0, "client 2 choked");
                // Fewer bytes than client 1 at the same update rate (the
                // rate counts the messages, not the transport's headers
                // and acknowledgements).
                let full = sent(&sim, 1) - before[1];
                assert!((bytes as f64) < full as f64 * 0.75, "client 2: {bytes} bytes in 5 s, client 1 {full}");
            }
        }
        assert_eq!(g.errors, 0, "client {i}: prediction errors ({:?})", g.last_differing);
        assert!(g.checked > 20, "client {i}: {} states compared", g.checked);
    }
    walk(&mut sim, Vec2::ZERO);
    sim.ticks(120);
    let end = sim.server.app.world().get::<Transform>(host).unwrap().translation;
    for i in 0..3 {
        let seen = seen_by(&mut sim, i, net::HOST_ID).unwrap();
        let at = sim.clients[i].app.world().get::<Transform>(seen).unwrap().translation;
        assert!(at.distance(end) < 1e-3, "client {i} draws the host at {at}, the server has {end}");
    }
}

#[test]
fn map_render_looks_replicate() {
    use crate::stack::map::{BrushTint, MapBrushEntity, RenderLook};
    let mut sim = NetSim::new(CsSim, lan(), 32, 2, greybox);
    sim.until_joined(200);
    sim.ticks(10);
    // The map logic makes client 0's player invisible and another one red
    // (AddOutput rendermode/renderamt, Color: `logic::bridge`).
    let ghost = RenderLook {
        color: [255; 3],
        alpha: 0,
        mode: 1,
    };
    let on_server = NetSim::owned_by(&mut sim.server, Some(1)).unwrap();
    sim.server.app.world_mut().entity_mut(on_server).insert(ghost);
    // A mover's colour: the server's node replicates with its NetMover;
    // the client's own copy of the node (same index) takes it.
    let red = BrushTint(
        RenderLook {
            color: [255, 0, 0],
            alpha: 255,
            mode: 0,
        },
        2,
    );
    sim.server.app.world_mut().spawn((
        bevy_replicon::prelude::Replicated,
        net::NetMover {
            index: 77,
            ..default()
        },
        red,
    ));
    let node = sim.clients[1].app.world_mut().spawn(MapBrushEntity(77)).id();
    sim.ticks(30);
    let other = seen_by(&mut sim, 1, 1).unwrap();
    assert_eq!(sim.clients[1].app.world().get::<RenderLook>(other), Some(&ghost), "others see it");
    let me = sim.local_player(0).unwrap();
    assert_eq!(sim.clients[0].app.world().get::<RenderLook>(me), Some(&ghost), "its own client too");
    assert_eq!(sim.clients[1].app.world().get::<BrushTint>(node), Some(&red), "the node's copy");
    // Back to normal.
    sim.server.app.world_mut().entity_mut(on_server).insert(RenderLook::default());
    sim.ticks(30);
    assert_eq!(
        sim.clients[1].app.world().get::<RenderLook>(other),
        Some(&RenderLook::default())
    );
}

#[test]
fn map_effects_reach_clients() {
    use bevy::ecs::message::Messages;
    use crate::stack::map::BeamStrike;
    let mut sim = NetSim::new(CsSim, lan(), 33, 1, greybox);
    sim.until_joined(200);
    sim.ticks(10);
    let strike = BeamStrike {
        entity: 12,
        start: Vec3::new(1.0, 2.0, 3.0),
        end: Vec3::new(4.0, 5.0, 6.0),
        life: 2.0,
    };
    sim.server.app.world_mut().write_message(strike);
    let mut seen = Vec::new();
    for _ in 0..30 {
        sim.ticks(1);
        let messages = sim.clients[0].app.world().resource::<Messages<BeamStrike>>();
        seen.extend(messages.iter_current_update_messages().copied());
    }
    assert!(seen.contains(&strike), "{seen:?}");
}

/// An automatic hint the server gives a remote player (`rules::hints`:
/// killing an enemy, which only the server sees) reaches that player's
/// client as its own player's hint (`game::net`'s `HintNews`), and no one
/// else's.
#[test]
fn hints_reach_a_remote_player() {
    use bevy::ecs::message::Messages;
    use crate::stack::{
        core::{Damage, DamageKind, Hitgroup},
        rules::hints::{AutoHint, HintRules},
    };
    const KILLED_ENEMY: &str = "Cstrike_TitlesTXT_Hint_win_round_by_killing_enemy";
    let mut sim = NetSim::new(CsSim, lan(), 34, 2, greybox);
    // No gap: the buy zone's hint comes first.
    sim.server.app.world_mut().resource_mut::<HintRules>().gap = 0.0;
    let host = host(&mut sim);
    sim.until_joined(200);
    sim.ticks(10);
    let killer = sim.character_of(0).expect("client 1's character");
    // The host's enemy, in the game.
    sim.server.app.world_mut().entity_mut(killer).insert(Team(1));
    let alive = sim.until(600, |s| {
        s.server.app.world().get::<crate::stack::core::Dead>(killer).is_none()
    });
    assert!(alive, "client 1's player spawns");
    sim.server.app.world_mut().write_message(Damage {
        target: host,
        attacker: Some(killer),
        amount: 5.0,
        point: Vec3::new(6.0, 1.0, 0.0),
        dir: Vec3::X,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    let mut seen: Vec<Vec<AutoHint>> = vec![Vec::new(); 2];
    let mut cursors: Vec<_> = sim
        .clients
        .iter()
        .map(|c| c.app.world().resource::<Messages<AutoHint>>().get_cursor_current())
        .collect();
    let mut given = Vec::new();
    let mut server = sim.server.app.world().resource::<Messages<AutoHint>>().get_cursor_current();
    for _ in 0..60 {
        sim.ticks(1);
        given.extend(server.read(sim.server.app.world().resource::<Messages<AutoHint>>()).cloned());
        for (i, c) in sim.clients.iter().enumerate() {
            seen[i].extend(cursors[i].read(c.app.world().resource::<Messages<AutoHint>>()).cloned());
        }
    }
    assert!(given.iter().any(|h| h.to == killer && h.token == KILLED_ENEMY), "the server's: {given:?}");
    let mine = sim.local_player(0).expect("client 1's player");
    let killed: Vec<_> = seen[0].iter().filter(|h| h.token == KILLED_ENEMY).collect();
    assert_eq!(killed.len(), 1, "{:?}", seen[0]);
    assert_eq!(killed[0].to, mine);
    assert!(seen[1].iter().all(|h| h.token != KILLED_ENEMY), "{:?}", seen[1]);
}

/// A server whose match movement is noclip still spawns its clients'
/// players walking: CS:S never spawns a player noclipping
/// (specs/cs_source/movement.md "Noclip and spawning").
#[test]
fn the_server_never_spawns_a_player_noclipping() {
    use crate::stack::{movement::noclip, slots::MovementSlot};
    let mut sim = NetSim::new(CsSim, lan(), 1, 1, |app: &mut App| {
        greybox(app);
        app.insert_resource(Loadout { movement: noclip::ID });
    });
    host(&mut sim);
    sim.until_joined(200);
    let a = sim.character_of(0).expect("server character for the client");
    let slot = sim.server.app.world().get::<MovementSlot>(a).unwrap().0;
    assert_ne!(slot, noclip::ID);
}
