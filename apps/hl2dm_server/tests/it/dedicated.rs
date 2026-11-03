//! The dedicated server (`ServerPlugin`, its command line) on the greybox
//! with a headless client of HL2:DM's simulation (as the app runs it,
//! without the window) joined over UDP on loopback: it comes in on
//! HL2:DM's movement with its loadout (the gravity gun in hand, the SMG1
//! its last weapon).

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::core::{LocalPlayer, NetRole};
use chimera_net::net::{self, NetCharacter, NetPlugin, client::Joined};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use hl2dm::{
    game::{TICK_INTERVAL, movement, weapons},
    harness::Hl2Sim,
};
use hl2dm_server::{Parsed, ServerPlugin, StartMap};

fn frame(sim: &mut Sim) {
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK_INTERVAL)));
}

/// The server from its command line (`args`), listening on loopback.
pub(crate) fn server(args: &[&str]) -> Option<(Sim, std::net::SocketAddr)> {
    let mut args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    args.extend(["+ip", "127.0.0.1", "-port", "0"].map(String::from));
    let Ok(Parsed::Run(options)) = hl2dm_server::parse(args) else { panic!("options") };
    let start = match StartMap::load(&options) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    let mut sim = Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(start, &options));
    });
    frame(&mut sim);
    // The command line's console lines run.
    sim.app.update();
    let addr = hl2dm_server::listen(sim.app.world_mut()).expect("listen");
    assert!(addr.ip().is_loopback() && addr.port() != 0, "{addr}");
    assert_eq!(*sim.app.world().resource::<NetRole>(), NetRole::Server);
    Some((sim, addr))
}

/// A client of HL2:DM's simulation on the greybox, connected to `addr`.
fn client(addr: std::net::SocketAddr) -> Sim {
    let mut sim = Sim::with(Hl2Sim, |app| {
        app.add_plugins((NetPlugin, GreyboxMapPlugin));
    });
    frame(&mut sim);
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(sim.app.world_mut(), to).expect("connect");
    sim
}

fn local(sim: &mut Sim) -> Option<Entity> {
    let w = sim.app.world_mut();
    w.query_filtered::<Entity, With<LocalPlayer>>().iter(w).next()
}

#[test]
fn a_client_joins_on_hl2dms_movement_with_its_loadout() {
    let Some((mut server, addr)) = server(&["+map", "greybox", "+maxplayers", "4"]) else { return };
    let mut c = client(addr);
    let mut joined = false;
    for _ in 0..1000 {
        server.app.update();
        c.app.update();
        std::thread::sleep(Duration::from_millis(1));
        if c.app.world().contains_resource::<Joined>() && local(&mut c).is_some() {
            joined = true;
            break;
        }
    }
    assert!(joined, "joined over UDP");
    let status = net::status(server.app.world_mut());
    assert!(status.contains("players : 1 (4 max)"), "{status}");
    // A second's play: the server's character settles with its kit.
    for _ in 0..(1.0 / TICK_INTERVAL) as usize {
        server.app.update();
        c.app.update();
    }
    let me = local(&mut c).unwrap();
    let id = c.app.world().get::<NetCharacter>(me).unwrap().owner;
    let w = server.app.world_mut();
    let theirs = w.query::<(Entity, &NetCharacter)>().iter(w).find(|(_, n)| n.owner == id).unwrap().0;
    let w = server.app.world();
    assert_eq!(w.get::<chimera_core::slots::MovementSlot>(theirs).map(|m| m.0), Some(movement::ID));
    // The spawn's loadout (specs/hl2dm/weapons.md 3, test 1): the gravity
    // gun in hand, the SMG1 the last weapon.
    assert_eq!(chimera_sim_active(w, theirs), Some(weapons::PHYSCANNON));
    let last = w.get::<chimera_sim::weapon::Inventory>(theirs).and_then(|i| i.last);
    assert_eq!(last.and_then(|g| w.get::<chimera_sim::weapon::Weapon>(g)).map(|g| g.id), Some(weapons::SMG1));
}

/// The weapon `p` holds (by id).
fn chimera_sim_active(w: &World, p: Entity) -> Option<&'static str> {
    let gun = w.get::<chimera_sim::weapon::Inventory>(p)?.active?;
    Some(w.get::<chimera_sim::weapon::Weapon>(gun)?.id)
}
