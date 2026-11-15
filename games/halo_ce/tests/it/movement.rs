//! The stand-in movement (`halo_ce:movement`, PLACEHOLDER numbers) on the
//! greybox: a player spawned with it stands on the floor and, pushed
//! forward, ends up displaced along its facing; the match's loadout names it.

use bevy::prelude::*;
use chimera_core::slots::{Loadout, MovementSlot};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{game::movement, harness::HaloSim};

#[test]
fn a_player_stands_and_walks_forward_on_the_greybox() {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    assert_eq!(sim.app.world().resource::<Loadout>().movement, movement::ID);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    assert_eq!(sim.app.world().get::<MovementSlot>(p).map(|m| m.0), Some(movement::ID));
    sim.seconds(1.5);
    assert!(sim.state(p).on_ground, "stands on the greybox");
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let moved = sim.position(p) - start;
    assert!(moved.xz().length() > 2.0, "walks forward: {moved}");
    assert!(moved.y.abs() < 0.5, "stays on the floor: {moved}");
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(1.0);
    assert!(sim.velocity(p).xz().length() < 0.1, "stops when released");
}
