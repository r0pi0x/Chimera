//! The gun game over the network: a server and two clients, aim and kills.

#![allow(dead_code, unused_imports)]

use crate::play::*;
use std::time::Duration;
use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::{
    console::Console,
    core::{Dead, God, Score},
    gameplay::Gameplay,
};
use chimera_net::{harness::NetSim, minigame_net::NetMinigame, net::memory::LinkConditions};
use chimera_sim::{
    minigame::{self, MINIGAME, hud::MinigameHud, ladder::Rung, runtime::InMinigame},
    mode::{Match, Phase},
    testkit,
    weapon::{Magazine, WeaponEvent, WeaponEventKind, WeaponSounds},
};
use chimera_world::{greybox::GreyboxMapPlugin, map::PlaySound};
use combat_arms::{
    game::TICK_INTERVAL,
    harness::CaSim,
};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};
use crate::play;

pub(crate) fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
}

/// A server playing the gun game and two clients that came in with no
/// minigame of their own.
pub(crate) fn joined() -> NetSim {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    crate::play::no_finale(sim.server.app.world_mut());
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.until_joined(900);
    sim.until(300, |s| (0..2).all(|i| held(s, i).is_some() && clip(s, i).is_some()));
    sim.ticks(60);
    sim
}

pub(crate) fn hud_lines(sim: &NetSim, i: usize) -> Vec<String> {
    let hud = sim.clients[i].app.world().resource::<MinigameHud>();
    hud.lines.iter().map(|l| l.text.clone()).collect()
}

pub(crate) fn held(sim: &mut NetSim, i: usize) -> Option<&'static str> {
    let me = sim.local_player(i)?;
    testkit::active_id(sim.clients[i].app.world(), me)
}

/// Client `i`'s own rounds in the clip of the gun in hand.
pub(crate) fn clip(sim: &mut NetSim, i: usize) -> Option<u32> {
    let me = sim.local_player(i)?;
    let w = sim.clients[i].app.world();
    w.get::<Magazine>(testkit::active(w, me)?).map(|m| m.clip)
}

pub(crate) fn server_alive(sim: &NetSim, e: Entity) -> bool {
    sim.server.app.world().get::<Dead>(e).is_none()
}

/// `a` on the greybox's open middle and `b` 4 m in front of it (-Z), on
/// the server: wherever their spawns put them (a spawn's spot is the
/// picker's to choose, and one may have a crate 4 m ahead).
pub(crate) fn face_off(sim: &mut NetSim, a: Entity, b: Entity) {
    let w = sim.server.app.world_mut();
    for (e, at) in [(a, play::OPEN), (b, play::OPEN + Vec3::new(0.0, 0.0, -4.0))] {
        w.get_mut::<Transform>(e).unwrap().translation = at;
        w.get_mut::<avian3d::prelude::Position>(e).unwrap().0 = at;
    }
}

/// Client 0 (character `a`) shoots client 1 (`b`) dead with rung `kill`'s
/// gun, which each client sees in its owner's hand with rounds in it.
pub(crate) fn kill_once(sim: &mut NetSim, kill: usize, a: Entity, b: Entity, ladder: &[&'static str]) {
    assert_eq!(held(sim, 0), Some(ladder[kill]), "rung {kill}'s gun in client 0's hand");
    assert!(clip(sim, 0).is_some_and(|c| c > 0), "rung {kill}: {} has rounds on its client", ladder[kill]);
    // The victim back in play and unprotected, 4 m in front of the shooter.
    assert!(
        sim.until(600, |s| server_alive(s, b) && s.server.app.world().get::<God>(b).is_none()),
        "the victim is back"
    );
    sim.ticks(20);
    assert_eq!(held(sim, 1), Some(ladder[0]), "the victim, back, has the first rung's gun on its client");
    face_off(sim, a, b);
    sim.ticks(30);
    let me = sim.local_player(0).unwrap();
    let mut dead = false;
    for t in 0..(10.0 / TICK_INTERVAL) as u64 {
        let target = sim.server.position(b);
        testkit::aim_at(&mut sim.clients[0], me, target);
        sim.clients[0].intent(me).fire = t % 2 == 0;
        sim.step();
        if !server_alive(sim, b) {
            dead = true;
            break;
        }
    }
    sim.clients[0].intent(me).fire = false;
    assert!(dead, "kill {kill}: {} killed in 10 s", ladder[kill]);
    sim.ticks(25);
}
