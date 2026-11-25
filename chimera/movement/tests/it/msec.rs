//! Commands of whole milliseconds (`QuakeConfig::command_msec`;
//! docs/plans/active/shared-movement.md, S8): a tick of whole milliseconds
//! steps as it did; at 1/66 s the steps are 15 and 16 ms and what a step
//! leaves over is in the predicted state, so a replayed tick steps the
//! same.

use bevy::prelude::*;
use chimera_core::core::Velocity;
use chimera_movement::{QuakeConfig, QuakeMovement, command_msec};

use crate::course::{self, World};

fn msec() -> QuakeConfig {
    QuakeConfig { command_msec: true, ..QuakeConfig::reference() }
}

#[test]
fn the_tick_cut_to_whole_milliseconds() {
    // Whole already: nothing over.
    assert_eq!(command_msec(0.01, 0.0), (10.0, 0.0));
    assert_eq!(command_msec(0.015, 0.0), (15.0, 0.0));
    // 1/66 s: 15 ms and 0.15 over; a 16 ms step when it adds up.
    let (mut carry, mut steps) = (0.0, Vec::new());
    for _ in 0..66 {
        let (ms, left) = command_msec(1.0 / 66.0, carry);
        assert!((0.0..1.0).contains(&left), "{left}");
        carry = left;
        steps.push(ms);
    }
    assert_eq!(steps[..7], [15.0, 15.0, 15.0, 15.0, 15.0, 15.0, 16.0]);
    let total: f32 = steps.iter().sum();
    assert!((total + carry - 1000.0).abs() < 0.01, "a second of ticks is a second of steps: {total} + {carry}");
    // Less than a millisecond: no command, all of it carried; a long tick
    // is one command of at most 255 ms.
    assert_eq!(command_msec(0.0004, 0.0).0, 0.0);
    assert!((command_msec(0.0004, 0.0).1 - 0.4).abs() < 1e-4);
    assert_eq!(command_msec(0.0004, 0.7).0, 1.0);
    assert_eq!(command_msec(1.0, 0.0).0, 255.0);
}

/// One run of the course's script at the sim's own tick: each tick's feet.
fn run(config: QuakeConfig, ticks: u64) -> Vec<Vec3> {
    let (mut sim, p) = course::sim(config, World::Brushes);
    sim.set_tick_interval(0.01);
    (0..ticks)
        .map(|t| {
            let at = course::feet(&sim, p);
            course::script(t, at, &mut sim.intent(p));
            sim.ticks(1);
            course::feet(&sim, p)
        })
        .collect()
}

#[test]
fn a_tick_of_whole_milliseconds_steps_as_it_did() {
    assert_eq!(run(QuakeConfig::reference(), 500), run(msec(), 500), "10 ms ticks: bit for bit");
}

#[test]
fn the_remainder_is_carried_and_a_replayed_tick_sees_it() {
    let (mut sim, p) = course::sim(msec(), World::Brushes);
    sim.set_tick_interval(1.0 / 66.0);
    sim.ticks(1);
    let dt = sim.app.world().resource::<chimera_core::core::SimClock>().dt();
    // A free fall: each tick's gravity is its step's, 0.8 units/s a ms.
    course::place(&mut sim, p, Vec3::new(0.0, 0.0, 1500.0));
    let (mut long, mut short) = (0, 0);
    for t in 0..66 {
        let before = course::state(&sim, p);
        let vz = chimera_movement::to_units(sim.velocity(p)).z;
        let (ms, left) = command_msec(dt, before.msec_carry);
        sim.ticks(1);
        let after = course::state(&sim, p);
        let fell = vz - chimera_movement::to_units(sim.velocity(p)).z;
        assert!((fell - 0.8 * ms).abs() < 1e-2, "tick {t}: fell {fell} in a {ms} ms step");
        assert!((after.msec_carry - left).abs() < 1e-5, "tick {t}: {} left over, not {left}", after.msec_carry);
        if ms == 16.0 {
            long += 1;
        } else {
            assert_eq!(ms, 15.0);
            short += 1;
        }
    }
    assert!(long >= 9 && short >= 55, "{short} steps of 15 ms, {long} of 16");
    // A replay: the predicted state put back (the transform, the velocity,
    // the movement's own with its remainder) and the tick run again lands
    // on the same bits; with the remainder lost it would not (the next
    // 16 ms step comes at another tick).
    let w = sim.app.world_mut();
    let saved = (*w.get::<Transform>(p).unwrap(), *w.get::<Velocity>(p).unwrap(), w.get::<QuakeMovement>(p).unwrap().clone());
    let restore = |sim: &mut chimera_world::harness::Sim, me: QuakeMovement| {
        let w = sim.app.world_mut();
        *w.get_mut::<Transform>(p).unwrap() = saved.0;
        if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
            pos.0 = saved.0.translation;
        }
        *w.get_mut::<Velocity>(p).unwrap() = saved.1;
        *w.get_mut::<QuakeMovement>(p).unwrap() = me;
    };
    let ticks_to_long = |sim: &mut chimera_world::harness::Sim| {
        let mut n = 0;
        loop {
            let (ms, _) = command_msec(dt, course::state(sim, p).msec_carry);
            sim.ticks(1);
            n += 1;
            if ms == 16.0 || n > 20 {
                return (n, sim.position(p), sim.velocity(p));
            }
        }
    };
    let first = ticks_to_long(&mut sim);
    restore(&mut sim, saved.2.clone());
    let replayed = ticks_to_long(&mut sim);
    assert_eq!(first, replayed, "the replay steps the same milliseconds");
    let mut other = saved.2.clone();
    other.msec_carry = (other.msec_carry + 0.5).fract();
    restore(&mut sim, other);
    let lost = ticks_to_long(&mut sim);
    assert_ne!(first.0, lost.0, "another remainder, another tick for the 16 ms step");
}
