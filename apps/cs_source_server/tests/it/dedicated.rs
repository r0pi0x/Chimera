//! The dedicated server (`ServerPlugin`, its command line) on the greybox
//! with a headless client of CS:S's simulation joined over UDP on
//! loopback; and what CS:S's server has beyond the others
//! (`ServerHooks`): the greybox at the engine's 64 Hz, the map cycle
//! following the map loaded, the map's file offered (`MapFiles`).

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::core::{DEFAULT_TICK_HZ, MapCycle, NetRole};
use chimera_core::slots::Loadout;
use chimera_net::net::{self, NetPlugin, client::Joined, maps::MapFiles};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use cs_source::{
    game::{CsSourcePlugins, movement},
    harness::CsSim,
};
use cs_source_server::{Parsed, ServerPlugin, StartMap};

fn frame(sim: &mut Sim) {
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            cs_source::game::TICK_INTERVAL,
        )));
}

/// The server from its command line (`args`), listening on loopback.
fn server(args: &[&str]) -> Option<(Sim, std::net::SocketAddr)> {
    let mut args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    args.extend(["+ip", "127.0.0.1", "-port", "0"].map(String::from));
    let Ok(Parsed::Run(options)) = cs_source_server::parse(args) else {
        panic!("options")
    };
    let start = match StartMap::load(&cs_source_server::SERVER, &options) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    let mut sim = Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(&cs_source_server::SERVER, start, &options));
    });
    frame(&mut sim);
    // The command line's console lines run.
    sim.app.update();
    let addr = cs_source_server::listen(sim.app.world_mut()).expect("listen");
    assert!(addr.ip().is_loopback() && addr.port() != 0, "{addr}");
    assert_eq!(*sim.app.world().resource::<NetRole>(), NetRole::Server);
    Some((sim, addr))
}

fn tick(sim: &Sim) -> Duration {
    sim.app.world().resource::<Time<Fixed>>().timestep()
}

#[test]
fn the_greybox_server_has_csss_hooks_and_a_client_joins() {
    let Some((mut server, addr)) = server(&["+map", "greybox", "+maxplayers", "4"]) else {
        return;
    };
    // The greybox at the engine's tick, not CS:S's 66.7 Hz.
    assert_eq!(tick(&server), Duration::from_secs_f64(1.0 / DEFAULT_TICK_HZ));
    // The map's file is offered: CS:S's maps from its install, no other game's.
    let files = server.app.world().resource::<MapFiles>().clone();
    assert!((files.read)("hl2dm:dm_lockdown").is_none());
    assert_eq!(files.cache, cs_source::game::INFO.content_cache());
    let console = server.app.world().resource::<chimera_core::console::Console>();
    assert!(
        console
            .output
            .iter()
            .any(|l| l.text.starts_with("Chimera CS:S dedicated server")),
        "the banner"
    );

    let mut c = Sim::with(CsSim, |app| {
        // As the game composes it (its parts with the network's: the same
        // protocol as the server's).
        app.add_plugins((NetPlugin, GreyboxMapPlugin, CsSourcePlugins))
            .insert_resource(Loadout { movement: movement::ID });
    });
    frame(&mut c);
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(c.app.world_mut(), to).expect("connect");
    let mut joined = false;
    for _ in 0..1000 {
        server.app.update();
        c.app.update();
        std::thread::sleep(Duration::from_millis(1));
        if c.app.world().contains_resource::<Joined>() {
            joined = true;
            break;
        }
    }
    let said = |sim: &Sim| {
        let lines = &sim.app.world().resource::<chimera_core::console::Console>().output;
        lines.iter().rev().take(12).map(|l| l.text.clone()).collect::<Vec<_>>()
    };
    let stage = |sim: &Sim| {
        format!(
            "{:?}",
            sim.app
                .world()
                .get_resource::<net::client::JoinProgress>()
                .map(|p| p.log.clone())
        )
    };
    assert!(
        joined,
        "joined over UDP: client {:?} {}, server {:?}",
        said(&c),
        stage(&c),
        said(&server)
    );
    let status = net::status(server.app.world_mut());
    assert!(status.contains("players : 1 (4 max)"), "{status}");

    // `changelevel greybox` keeps the greybox's tick and starts a new game.
    chimera_net::dedicated::change_level(server.app.world_mut(), "greybox").unwrap();
    server.app.update();
    assert_eq!(tick(&server), Duration::from_secs_f64(1.0 / DEFAULT_TICK_HZ));
    // The map cycle follows the map loaded.
    assert_eq!(server.app.world().resource::<MapCycle>().current, "greybox");
}
