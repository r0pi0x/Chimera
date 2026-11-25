//! A client that connects during a dedicated server's start follows it into
//! its start map (`chimera_net::dedicated::StartLevel`, docs/performance.md,
//! "A join while the host starts"): the server listens on its first frame
//! and loads the start map as a level change, and a client that joined
//! meanwhile loads it behind its loading screen and plays there. Before,
//! the server loaded and spawned its start map whole before it listened,
//! and a client connecting then got no answer for that long.
//!
//! The Ultimate Gun Game's dedicated server in this process (as its binary
//! builds it, `StartMap::choose_in`) on Combat Arms' warehouse, and this
//! crate's client binary offscreen, started first and knocking (`+connect`)
//! when the server comes up, so it joins while the start map loads.
//! Needs Combat Arms (an install); passes with a note without one or
//! without a GPU adapter. About a minute.
//!
//! `cargo test -p ultimate_gun_game --features dev --test base heavy::server_start_join -- --nocapture`

use std::time::{Duration, Instant};

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_net::{
    dedicated::{self, GameServer, ServerHooks, ServerMinigame, StartLevel, StartMap},
    net,
};
use chimera_world::map::{self, LoadedMapName};
use party_roster::server::PARTY;

// Its process start and log reading (the rest is first_view's).
#[allow(dead_code)]
#[path = "../../common/first_view_pair.rs"]
mod pair;

const MAP: &str = "combat_arms:warehouse";

/// The dedicated server's own (apps/ultimate_gun_game_server).
static SERVER: GameServer = GameServer {
    title: "Ultimate Gun Game",
    a_title: "an Ultimate Gun Game",
    usage: "",
    hooks: ServerHooks {
        minigame: Some(ServerMinigame {
            default: ultimate_gun_game::MINIGAME,
            first_map: ultimate_gun_game::first_map,
        }),
        launch: Some(|app| {
            party_roster::server::launch(app);
            app.add_plugins(ultimate_gun_game::UltimateGunGamePlugin::launch());
        }),
        ..PARTY.hooks
    },
    ..PARTY
};

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

/// One server frame, at most `dedicated::FPS_MAX` a second.
fn step(app: &mut App) {
    app.update();
    std::thread::sleep(Duration::from_secs_f64(1.0 / dedicated::FPS_MAX));
}

#[test]
fn a_client_connecting_during_a_servers_start_follows_it_into_the_map() {
    if !combat_arms_installed() {
        eprintln!("skipping: needs the Combat Arms install");
        return;
    }
    // A port the system picks (never Source's 27015); the client is
    // started first and knocks until the server answers, so it connects
    // in the server's first frames, while the start map loads.
    let port = std::net::UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let dir = std::env::temp_dir().join(format!("chimera_ugg_server_start_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let client_args: Vec<String> =
        ["+connect", &format!("127.0.0.1:{port}"), "+after", "280", "quit"].iter().map(|s| s.to_string()).collect();
    let (mut client, log) = pair::start(&dir, "Client", &client_args);
    let knocking = Instant::now();
    while !pair::read(&log).contains("Connecting to") {
        assert!(knocking.elapsed() < pair::TIMEOUT, "the client never connected\n{}", pair::read(&log));
        if let Ok(Some(status)) = client.0.try_wait() {
            panic!("the client exited ({status})\n{}", pair::read(&log));
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let port = port.to_string();
    let args: Vec<String> = ["-port", &port, "+maxplayers", "4", "+map", MAP].iter().map(|s| s.to_string()).collect();
    let options = match SERVER.parse(args).unwrap() {
        dedicated::Parsed::Run(o) => o,
        dedicated::Parsed::Help => unreachable!(),
    };
    // As the binary's: no sound samples, no render meshes of the kit.
    let context = map::LoadContext::default();
    context.insert(map::sound::Silent);
    context.insert(map::undrawn::Undrawn);
    let start = StartMap::choose_in(&SERVER, &options, context).expect("a start");
    assert!(matches!(start, StartMap::Later { .. }), "the start map comes in as a level change");
    let mut app = dedicated::app(&SERVER, start, &options);
    chimera_world::harness::single_threaded_schedules(&mut app);
    app.finish();
    app.cleanup();
    // The first frame: the settings ran, the start map's change began.
    step(&mut app);
    dedicated::listen(app.world_mut()).expect("the server listens");
    assert!(app.world().contains_resource::<StartLevel>(), "it listens before the start map is in");
    assert!(app.world().get_resource::<map::MapFile>().is_none(), "no map in yet");

    let began = Instant::now();
    let mut joined_during_start = false;
    let mut log_text = String::new();
    let mut frames = 0u64;
    while began.elapsed() < pair::TIMEOUT {
        step(&mut app);
        frames += 1;
        let players = net::server::players(app.world_mut());
        if !players.is_empty() && app.world().contains_resource::<StartLevel>() {
            joined_during_start = true;
        }
        if let Some(exit) = app.should_exit() {
            panic!("the server exited ({exit:?})");
        }
        if let Ok(Some(status)) = client.0.try_wait() {
            log_text = pair::read(&log);
            panic!("the client exited ({status})\n{log_text}");
        }
        // The client's log a second at a time.
        if frames % 300 == 0 {
            log_text = pair::read(&log);
            let settled = log_text.contains("settled behind the loading screen");
            if settled && !app.world().contains_resource::<StartLevel>() && !players.is_empty() {
                break;
            }
        }
    }
    drop(client);
    let _ = std::fs::remove_dir_all(&dir);
    if log_text.contains("adapter") && log_text.contains("panicked") {
        eprintln!("skipping: no GPU adapter\n{log_text}");
        return;
    }
    assert!(!pair::join_timed_out(&log_text), "the client's join timed out\n{log_text}");
    assert_eq!(app.world().resource::<LoadedMapName>().0, MAP);
    assert!(!app.world().contains_resource::<StartLevel>(), "the server's start map came in\n{log_text}");
    assert!(
        log_text.contains("settled behind the loading screen"),
        "the client didn't load the server's map behind its loading screen\n{log_text}"
    );
    assert!(joined_during_start, "the client joined while the server's start map came in\n{log_text}");
    let players = net::server::players(app.world_mut());
    assert_eq!(players.len(), 1, "the client plays on the server");
}
