//! A soldier holding a rung's gun; a shot on the centred command.

#![allow(dead_code, unused_imports)]

use crate::play::*;
use avian3d::prelude::{Collider, LinearVelocity, Mass, RigidBody};
use bevy::prelude::*;
use chimera_sim::{
    minigame::ladder::{self, Ladder},
    testkit,
    weapon::{ViewPunch, impact::RecoilGame},
};
use chimera_world::{harness::Sim, map::PropSurface};
use combat_arms::game::weapons::CaKick;
use crate::play::{held, party, place, soldier};

/// A soldier at the greybox's middle holding the ladder's rung `r`.
pub(crate) fn holding(sim: &mut Sim, r: usize, id: &str) -> Entity {
    let a = soldier(sim);
    let _b = soldier(sim);
    sim.seconds(1.5);
    place(sim, a, Vec3::new(0.0, 0.1, 0.0));
    set_rung(sim, a, r);
    sim.seconds(1.5);
    assert_eq!(held(sim, a), Some(id));
    a
}

/// `a`'s next tick runs on `chimera_sim::weapon::centred_command`: its
/// `Seed` set so that tick's command number is that one.
pub(crate) fn centre_next_shot(sim: &mut Sim, a: Entity) {
    let next = sim.app.world().resource::<chimera_core::core::SimTick>().0 + 1;
    sim.app
        .world_mut()
        .entity_mut(a)
        .insert(chimera_core::core::Seed(chimera_sim::weapon::centred_command().wrapping_sub(next as u32) as u64));
}
