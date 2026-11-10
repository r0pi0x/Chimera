//! What the server's tests share: the server from a command line, a
//! scripted soldier, its aim and trigger.

use std::time::Duration;

use avian3d::prelude::Position;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::core::Dead;
use chimera_sim::testkit;
use chimera_world::harness::Sim;
use combat_arms::game::{TICK_INTERVAL, movement};
use party_kit::server::{Parsed, ServerPlugin, StartMap};

/// The server from its command line (`args`), its console lines run (the
/// minigame started); None when its map can't load (an install missing).
pub fn server(args: &[&str]) -> Option<Sim> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let Ok(Parsed::Run(options)) = party_kit::server::parse(args) else { panic!("options") };
    let start = match StartMap::load(&party_kit::server::SERVER, &options) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    let mut sim = Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(&party_kit::server::SERVER, start, &options));
    });
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK_INTERVAL)));
    sim.app.update();
    Some(sim)
}

/// A Combat Arms soldier, not yet in the game (the minigame takes it in).
pub fn soldier(sim: &mut Sim, at: Vec3) -> Entity {
    sim.spawn_character(at, movement::ID)
}

pub fn place(sim: &mut Sim, e: Entity, at: Vec3) {
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = at;
    w.get_mut::<Position>(e).unwrap().0 = at;
}

/// `shooter` aims at `target`'s centre and works the trigger until
/// `target` dies or `secs` pass; whether it died.
pub fn shoot_until_dead(sim: &mut Sim, shooter: Entity, target: Entity, secs: f64) -> bool {
    let ticks = (secs * sim.tick_hz()) as u64;
    for t in 0..ticks {
        let at = sim.position(target);
        testkit::aim_at(sim, shooter, at);
        sim.intent(shooter).fire = t % 2 == 0;
        sim.ticks(1);
        if sim.app.world().get::<Dead>(target).is_some() {
            sim.intent(shooter).fire = false;
            return true;
        }
    }
    sim.intent(shooter).fire = false;
    false
}
