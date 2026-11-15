//! The dedicated server (`ServerPlugin`, its command line) on the greybox:
//! its plugin builds without a window, ticks at Halo's rate, and a player
//! spawned on the stand-in movement walks.

use bevy::prelude::*;
use chimera_core::core::NetRole;
use chimera_world::{greybox, harness::Sim};
use halo_ce::game::movement;
use halo_ce_server::{Parsed, ServerPlugin, StartMap};

#[test]
fn the_server_builds_on_the_greybox_and_ticks() {
    let args = ["+map", "greybox", "+ip", "127.0.0.1", "-port", "0"].map(String::from);
    let Ok(Parsed::Run(options)) = halo_ce_server::parse(args) else { panic!("options") };
    let start = StartMap::load(&halo_ce_server::SERVER, &options).expect("the greybox loads");
    let mut sim = Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(&halo_ce_server::SERVER, start, &options));
    });
    sim.app.update();
    let addr = halo_ce_server::listen(sim.app.world_mut()).expect("listen");
    assert!(addr.ip().is_loopback() && addr.port() != 0, "{addr}");
    assert_eq!(*sim.app.world().resource::<NetRole>(), NetRole::Server);
    assert!((sim.tick_hz() - 30.0).abs() < 0.01, "{}", sim.tick_hz());
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    assert!(sim.state(p).on_ground);
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    assert!(sim.position(p).distance(start) > 1.0, "walks");
}
