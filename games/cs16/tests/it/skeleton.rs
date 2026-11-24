//! The skeleton on the headless harness (`harness::Cs16Sim`, the
//! greybox): the game registers and is the gameplay at its tick, its data
//! parses, and a body on `cs16:movement` spawns, stands on the floor and
//! walks. Replace the stand-in numbers with the spec's as the parts land.

use bevy::prelude::*;
use chimera_core::{game_registry::Games, gameplay::Gameplay, slots::MovementSlot};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use cs16::{
    game::{BUNDLE, GAME, INFO, TICK_INTERVAL, movement, parts::PARTS, weapons},
    harness::Cs16Sim,
};

fn sim() -> Sim {
    Sim::new(Cs16Sim, GreyboxMapPlugin)
}

#[test]
fn the_game_registers_and_is_the_gameplay_at_its_tick() {
    let sim = sim();
    let w = sim.app.world();
    assert!(std::ptr::eq(w.resource::<Games>().get(GAME).unwrap(), &INFO));
    assert!(std::ptr::eq(BUNDLE.info, &INFO));
    let g = w.resource::<Gameplay>();
    assert_eq!((g.rules, g.hud, g.binds), (GAME, GAME, GAME));
    assert!((sim.tick_hz() - 1.0 / TICK_INTERVAL).abs() < 0.01, "{}", sim.tick_hz());
}

#[test]
fn it_lends_its_guns_and_movement() {
    assert_eq!(PARTS.game, GAME);
    // A scaffold: its stand-ins are reported but dealt into no ladder
    // until its first real gun (`scaffold: false` then).
    assert!(PARTS.lends_guns_and_movement() && PARTS.scaffold && !PARTS.playable());
    // The party's roster (apps/party_roster) reads both of every guest.
    assert!(BUNDLE.guest.is_some() && !BUNDLE.kept.is_empty(), "a guest dressing and a kept prefix");
    assert_eq!(PARTS.movement, Some(movement::ID));
    let set = weapons::weapons();
    for g in weapons::GUNS {
        assert!(set.weapons.iter().any(|w| w.id == *g), "{g} in the data");
    }
}

#[test]
fn a_body_spawns_stands_and_walks() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<MovementSlot>(p).map(|m| m.0), Some(movement::ID));
    assert!(sim.state(p).on_ground, "standing on the greybox's floor");
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let speed = sim.velocity(p).xz().length();
    let walk = movement::tuning().walk;
    assert!((speed - walk).abs() < 0.2, "walks at {speed} m/s, data says {walk}");
}
