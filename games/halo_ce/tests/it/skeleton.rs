//! The skeleton on the headless harness (`harness::HaloSim`, the
//! greybox): Halo registers, is the gameplay at its tick, and the world
//! steps.

use chimera_core::{game_registry::Games, gameplay::Gameplay};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use halo_ce::{
    game::{GAME, INFO},
    harness::HaloSim,
};

fn sim() -> Sim {
    Sim::new(HaloSim, GreyboxMapPlugin)
}

#[test]
fn the_game_registers_and_is_the_gameplay_at_its_tick() {
    let sim = sim();
    let w = sim.app.world();
    assert!(std::ptr::eq(w.resource::<Games>().get(GAME).unwrap(), &INFO));
    let g = w.resource::<Gameplay>();
    assert_eq!((g.rules, g.hud, g.binds), (GAME, GAME, GAME));
    assert!((sim.tick_hz() - 30.0).abs() < 0.01, "{}", sim.tick_hz());
}

#[test]
fn a_few_ticks_run_on_the_greybox() {
    let mut sim = sim();
    sim.ticks(10);
    sim.seconds(0.5);
}
