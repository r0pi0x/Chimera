//! The skeleton on the headless harness (`harness::DodsSim`, the greybox):
//! DoD:S registers and is the gameplay at its tick (0.015 s), and a player
//! on `dods:movement` (specs/dods/movement.md, "Constants") stands on the
//! greybox's floor with its eye 58 units over its feet, ducks to 34, runs
//! at 220 units/s; its envelope (the box its maps carry for other games'
//! characters) is the 32 x 72 box, 45 ducked, with those eyes.

use bevy::prelude::*;
use chimera_core::{envelope::MovementEnvelopes, game_registry::Games, gameplay::Gameplay, slots::MovementSlot};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use dods::{
    game::{GAME, INFO, movement},
    harness::DodsSim,
};

const UNIT: f32 = 0.0254;
/// Source movement's origin above the feet (half the standing box), metres.
const ORIGIN: f32 = 36.0 * UNIT;

fn sim() -> Sim {
    Sim::new(DodsSim, GreyboxMapPlugin)
}

/// A player at the greybox's first spawn, settled.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    p
}

/// The eye's height over the feet, units.
fn eye(sim: &Sim, p: Entity) -> f32 {
    (ORIGIN + sim.state(p).eye_offset.y) / UNIT
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
fn a_player_stands_with_its_eye_at_58_and_ducks_to_34() {
    let mut sim = sim();
    let p = player(&mut sim);
    assert_eq!(sim.app.world().get::<MovementSlot>(p).map(|m| m.0), Some(movement::ID));
    let feet = sim.position(p).y - ORIGIN;
    assert!(feet.abs() < 0.05, "feet at {feet}");
    assert!(sim.state(p).on_ground);
    let standing = eye(&sim, p);
    assert!((standing - 58.0).abs() < 0.5, "standing eye {standing} units");
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    assert!(sim.state(p).crouching);
    // Ducked, the origin is the ducked box's middle: the eye over the feet
    // is the feet's height plus the origin's plus the offset.
    let ducked = (sim.position(p).y + sim.state(p).eye_offset.y) / UNIT - feet / UNIT;
    assert!((ducked - 34.0).abs() < 0.5, "ducked eye {ducked} units");
}

#[test]
fn runs_at_220() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let speed = sim.velocity(p).xz().length() / UNIT;
    assert!((speed - 220.0).abs() < 3.0, "run speed {speed} units/s");
    sim.intent(p).move_axis = Vec2::NEG_Y;
    sim.seconds(1.0);
    let back = sim.velocity(p).xz().length() / UNIT;
    assert!((back - 220.0).abs() < 3.0, "no back-pedal penalty: {back} units/s");
}

#[test]
fn its_envelope_is_the_stances_box() {
    let sim = sim();
    let env = sim.app.world().resource::<MovementEnvelopes>().of(GAME).expect("DoD:S's envelope");
    let u = |v: f32| v * UNIT;
    assert!((env.radius - u(16.0)).abs() < 1e-4);
    assert!((env.height - u(72.0)).abs() < 1e-4);
    assert!((env.crouch_height - u(45.0)).abs() < 1e-4);
    assert!((env.eye - u(58.0)).abs() < 1e-4);
    assert!((env.crouch_eye - u(34.0)).abs() < 1e-4);
    assert!((env.step - u(18.0)).abs() < 1e-4);
}
