//! The skeleton on the headless harness (`harness::Tf2Sim`, the greybox):
//! TF2 registers and is the gameplay at its tick (0.015 s), and a player
//! on `tf2:movement` (Source's with TF2's config; with no TF2 gun in hand
//! the Soldier's numbers: run 240, a jump of 50 units) stands on the
//! greybox's floor, runs and jumps.

use bevy::prelude::*;
use chimera_core::{game_registry::Games, gameplay::Gameplay, slots::MovementSlot};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use tf2::{
    game::{GAME, INFO, movement},
    harness::Tf2Sim,
};

const UNIT: f32 = 0.0254;
/// Source movement's origin above the feet, metres.
const ORIGIN: f32 = 36.0 * UNIT;

fn sim() -> Sim {
    Sim::new(Tf2Sim, GreyboxMapPlugin)
}

/// A player at the greybox's first spawn, settled.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    p
}

#[test]
fn the_game_registers_and_is_the_gameplay_at_its_tick() {
    let sim = sim();
    let w = sim.app.world();
    assert!(std::ptr::eq(w.resource::<Games>().get(GAME).unwrap(), &INFO));
    let g = w.resource::<Gameplay>();
    assert_eq!((g.rules, g.hud, g.binds), (GAME, GAME, GAME));
    assert!((sim.tick_hz() - 1.0 / 0.015).abs() < 0.01, "{}", sim.tick_hz());
}

#[test]
fn a_player_stands_on_the_greybox() {
    let mut sim = sim();
    let p = player(&mut sim);
    assert_eq!(sim.app.world().get::<MovementSlot>(p).map(|m| m.0), Some(movement::ID));
    let feet = sim.position(p).y - ORIGIN;
    let v = sim.velocity(p);
    assert!(v.length() < 0.05, "still moving at {v}");
    // Standing on the greybox's floor (its top at y 0) under the spawn,
    // not falling through.
    assert!(feet.abs() < 0.05, "feet at {feet}, spawn {}", greybox::SPAWNS[0]);
    sim.seconds(1.0);
    assert!((sim.position(p).y - ORIGIN - feet).abs() < 0.01, "it stays");
}

#[test]
fn runs_at_240_and_jumps_50_units() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let speed = sim.velocity(p).xz().length() / UNIT;
    assert!((speed - 240.0).abs() < 5.0, "run speed {speed} units/s");
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(1.0);
    // A standing jump: 289 units/s against 800 units/s² in Source's split
    // gravity peaks at 50.04 units (movement.md test 5).
    let start = sim.position(p).y;
    sim.intent(p).jump = true;
    let mut peak = start;
    for _ in 0..60 {
        sim.ticks(1);
        peak = peak.max(sim.position(p).y);
    }
    sim.intent(p).jump = false;
    let height = (peak - start) / UNIT;
    assert!((height - 50.0).abs() < 1.0, "jump height {height} units");
}
