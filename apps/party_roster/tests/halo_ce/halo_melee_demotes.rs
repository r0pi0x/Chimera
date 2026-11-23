//! Halo's melee (the held gun's bash, by Halo's melee button) is a melee
//! kill in the party's rules: with `demote_on = "melee_death"` it costs
//! the victim a rung, as a knife's does, whatever Halo gun bashed (the
//! bash's damage is melee: its effect's category). A Halo gun's bullet
//! kill costs none. Headless on the party's simulation (`halo_ladder`'s
//! written ladder: rung 1 Halo's assault rifle, rung 4 CS:S's Deagle).

use bevy::prelude::*;
use chimera_core::core::ActionRegistry;
use halo_ce::game::weapons::systems::MELEE_ACTION;

use crate::halo_ladder::party;
use crate::play::{alive, line_up, rung, set_rung, shoot_until_dead, soldier};

const RIFLE: usize = 1;
const VICTIM: usize = 4;

/// `a` on Halo's rifle, `b` on the Deagle's rung, `d` m in front.
fn ready(d: f32) -> (chimera_world::harness::Sim, Entity, Entity) {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, RIFLE);
    set_rung(&mut sim, b, VICTIM);
    line_up(&mut sim, a, b, d);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (RIFLE, VICTIM));
    (sim, a, b)
}

#[test]
fn a_halo_bash_kill_demotes_the_victim() {
    let (mut sim, a, b) = ready(1.0);
    let bit = sim.app.world().resource::<ActionRegistry>().bit(MELEE_ACTION);
    assert_ne!(bit, 0, "Halo's melee action is registered");
    for t in 0..(10.0 * sim.tick_hz()) as u32 {
        let at = sim.position(b);
        chimera_sim::testkit::aim_at(&mut sim, a, at);
        sim.intent(a).actions = if t % 40 == 0 { bit } else { 0 };
        sim.ticks(1);
        if !alive(&sim, b) {
            break;
        }
    }
    sim.intent(a).actions = 0;
    assert!(!alive(&sim, b), "the bashes killed b");
    sim.ticks(2);
    assert_eq!(rung(&sim, b), VICTIM - 1, "a melee death costs a rung");
}

#[test]
fn a_halo_bullet_kill_does_not_demote() {
    let (mut sim, a, b) = ready(3.0);
    assert!(shoot_until_dead(&mut sim, a, b, 12.0), "the rifle kills b");
    sim.ticks(2);
    assert_eq!(rung(&sim, b), VICTIM, "a bullet death costs none");
}
