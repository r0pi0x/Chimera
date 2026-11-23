//! Slot keys hand out nothing of another game's (the user's bug: holding a
//! Halo gun, 3 gave Combat Arms' knife, and with it Combat Arms' body).
//! A rung's kit is its gun's game's alone (`auto::definition` and the
//! sandbox's: each game's own knife, none for a game without one), so on
//! every rung of every game (the sandbox's ladder: every playable game's
//! guns) the inventory holds that game's weapons only, and each slot key
//! leaves the player holding its game's weapon, as its game. On the
//! party's simulation, headless.

use bevy::prelude::*;
use chimera_core::{
    core::LocalPlayer,
    slots::GameIdentity,
};
use chimera_sim::{
    minigame::{self, Minigames, ladder::Ladder, rung_command},
    testkit,
    weapon::{Inventory, Weapon},
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::{MINIGAME, sandbox::SANDBOX};

use crate::parts::ugg;

fn game(id: &str) -> &str {
    id.split_once(':').map_or(id, |(g, _)| g)
}

fn carried(sim: &Sim, e: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    w.get::<Inventory>(e)
        .map(|i| i.weapons.iter().filter_map(|x| w.get::<Weapon>(*x).map(|x| x.id)).collect())
        .unwrap_or_default()
}

/// The automatic gun game's kit: each game's own knife or nothing.
#[test]
fn the_kit_by_game_is_each_games_own_melee_or_none() {
    let sim = ugg();
    let def = sim.app.world().resource::<Minigames>().get(MINIGAME).expect("the automatic gun game is made");
    for g in &def.slots.weapons {
        for id in def.spawn.kit.for_game(Some(g)) {
            assert_eq!(game(id), g, "{g}'s kit holds another game's {id}");
        }
    }
    assert!(def.spawn.kit.for_game(Some("halo_ce")).is_empty(), "Halo has no melee slot");
}

#[test]
fn slot_keys_never_give_another_games_weapon_or_body() {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let mut keyed: Vec<&str> = Vec::new();
    // Every rung but the sentinel (the last), by the sandbox's own move
    // (which holds the player there).
    for (r, id) in ladder.rungs.iter().enumerate().take(ladder.rungs.len() - 1) {
        let g = game(id);
        assert!(rung_command::move_to(sim.app.world_mut(), me, r, &ladder), "rung {r} ({id}) at once");
        sim.ticks(2);
        let have = carried(&sim, me);
        assert!(have.iter().all(|w| game(w) == g), "rung {r} ({id}) carries another game's: {have:?}");
        // Every slot key, once per game (on its first rung).
        if keyed.contains(&g) {
            continue;
        }
        keyed.push(g);
        sim.seconds(1.0);
        for slot in 0..6u8 {
            sim.intent(me).select = Some(slot);
            sim.ticks(1);
            sim.intent(me).select = None;
            sim.seconds(0.5);
            let w = sim.app.world();
            let held = testkit::active_id(w, me);
            assert!(held.is_some_and(|h| game(h) == g), "{id}, slot key {}: holding {held:?}", slot + 1);
            let is = w.get::<GameIdentity>(me).map(|i| i.0.as_str());
            assert_eq!(is, Some(g), "{id}, slot key {}: the body's game", slot + 1);
        }
    }
    for g in ["cs_source", "combat_arms", "hl2dm", "tf2", "halo_ce"] {
        assert!(keyed.contains(&g), "{g}'s guns in the sandbox: {keyed:?}");
    }
}
