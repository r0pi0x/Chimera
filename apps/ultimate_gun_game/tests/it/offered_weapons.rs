//! Every weapon a character holds in an automatic gun game's match is in
//! the match's asset offer (`minigame_net::match_weapons`: its
//! `weapon/<id>` set comes with the join's download), bots' too: a held
//! gun's view-model keys play its sounds (a reload's), which a client
//! without an install would else fetch on demand (the 2026-11-12
//! rehearsal: Combat Arms' `guns_snd_ar/m4a1/reload3.wav` at a join: a
//! joiner holds the base game's starting M4A1 from its spawn until the
//! minigame arms it). A server with Combat Arms' bots, a client joining
//! during the first match, then a new ladder.

use std::{collections::BTreeSet, time::Duration};

use bevy::prelude::*;
use chimera_core::console::Console;
use chimera_net::{harness::NetSim, minigame_net, net::memory::LinkConditions};
use chimera_sim::weapon::{Inventory, Weapon};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()));
}

/// The weapons characters hold now that the offer doesn't name.
fn unoffered(world: &mut World) -> BTreeSet<String> {
    let offered: BTreeSet<String> = minigame_net::match_weapons(world).into_iter().collect();
    let held: Vec<Entity> = world
        .query::<&Inventory>()
        .iter(world)
        .flat_map(|i| i.weapons.clone())
        .collect();
    held.into_iter()
        .filter_map(|w| world.get::<Weapon>(w).map(|w| w.id.to_string()))
        .filter(|id| !offered.contains(id))
        .collect()
}

#[test]
fn every_held_weapon_is_offered() {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        13,
        0,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    sim.server
        .app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_lobby 1; MaxPlayer 3");
    minigame_net::change_match(sim.server.app.world_mut(), MINIGAME, Some("greybox")).unwrap();
    sim.ticks(60);
    sim.add_client(|_| {});
    let mut seen = BTreeSet::new();
    for _ in 0..1200 {
        sim.ticks(1);
        seen.extend(unoffered(sim.server.app.world_mut()));
    }
    // A next match (a rotation's new ladder).
    sim.server
        .app
        .world_mut()
        .resource_mut::<Console>()
        .submit("ugg_new 7 12 greybox");
    for _ in 0..1200 {
        sim.ticks(1);
        seen.extend(unoffered(sim.server.app.world_mut()));
    }
    assert!(seen.is_empty(), "held but not offered: {seen:?}");
}
