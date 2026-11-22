//! HL2:DM's guns in the gun game over the network (`net_gun_game`'s
//! harness, `net_play`).

use std::time::Duration;
use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::minigame::{self};
use combat_arms::{
    game::TICK_INTERVAL,
    harness::CaSim,
};
use crate::net_play::*;

/// A gun game with HL2:DM's guns over the network: the first rung,
/// HL2:DM's pistol, in client 0's hand with rounds in it (the client
/// predicting HL2:DM's weapon frame: its pistol hooks), the player
/// HL2:DM's on the server and on its client; its kill moves it to CS:S's
/// Glock on both.
#[test]
fn an_hl2dm_rung_plays_over_the_network() {
    use crate::mixed::{LADDER, mixed_text};
    let text = mixed_text(false);
    let setup_hl2 = move |app: &mut App| {
        setup(app);
        chimera_sim::minigame::MinigameAppExt::add_minigames(app, &[("hl2dm.toml", &text)]).unwrap();
    };
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        setup_hl2,
    );
    sim.set_frame(TICK_INTERVAL);
    crate::play::no_finale(sim.server.app.world_mut());
    minigame::start(sim.server.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    assert!(sim.until(300, |s| held(s, 0) == Some(LADDER[0])), "HL2:DM's pistol on the client");
    let identity = |w: &World, e: Entity| w.get::<chimera_core::slots::GameIdentity>(e).map(|g| g.0.clone());
    assert_eq!(identity(sim.server.app.world(), a).as_deref(), Some("hl2dm"));
    let me = sim.local_player(0).unwrap();
    assert_eq!(identity(sim.clients[0].app.world(), me).as_deref(), Some("hl2dm"), "replicated");
    kill_once(&mut sim, 0, a, b, &LADDER);
    assert!(sim.until(300, |s| held(s, 0) == Some(LADDER[1])), "the Glock on the client after the kill");
}
