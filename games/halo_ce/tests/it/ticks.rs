//! Halo's 30 Hz ticks counted out of another game's tick
//! (`game::ticks::halo_ticks`): over ten minutes of engine ticks at the
//! party apps' 0.015 s tick (and at 64 Hz, 60 Hz and Halo's own 30 Hz),
//! the Halo ticks counted up to each engine tick are exactly what its
//! elapsed time implies, never one dropped by rounding (the old float
//! count lost one at engine tick 101 at 30 Hz); the vitality's tick counts
//! the same Halo ticks (its shield delay runs down by them) on `HaloSim`
//! set to the party's tick.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::SimClock;
use halo_ce::game::{
    damage::HaloHit,
    ticks::{crossed, halo_ticks},
    vitality::{DamageEffects, ids},
};

use super::vitality::{sim, spartan, vit};

/// Ten minutes and a tick, in ticks of `secs`.
fn ten_minutes(secs: f64) -> u64 {
    (600.0 / secs).ceil() as u64 + 1
}

#[test]
fn the_halo_ticks_counted_are_the_elapsed_times_at_any_tick_rate() {
    // (the engine's tick, Halo ticks per engine tick as num / den)
    let rates = [
        (Duration::from_secs_f64(0.015), 45u128, 100u128),
        (Duration::from_secs_f64(1.0 / 64.0), 30, 64),
        (Duration::from_secs_f64(1.0 / 60.0), 1, 2),
        (Duration::from_secs_f64(1.0 / 30.0), 1, 1),
        (Duration::from_nanos(33_333_334), 1, 1),
    ];
    for (delta, num, den) in rates {
        let ticks = ten_minutes(delta.as_secs_f64());
        let mut counted: u128 = 0;
        for k in 1..=ticks {
            let clock = SimClock { tick: k, now: (delta * k as u32).as_secs_f64(), delta };
            let n = halo_ticks(&clock);
            assert!(n <= 1, "{delta:?}: at most one Halo tick an engine tick here (tick {k}: {n})");
            counted += u128::from(n);
            assert_eq!(counted, u128::from(k) * num / den, "{delta:?}: the count at engine tick {k}");
        }
        assert!(ticks as f64 * delta.as_secs_f64() >= 600.0);
    }
    // Halo's own tick: one on each, the 101st too.
    let delta = Duration::from_secs_f64(1.0 / 30.0);
    let at = |k: u64| halo_ticks(&SimClock { tick: k, now: (delta * k as u32).as_secs_f64(), delta });
    assert!((1..=ten_minutes(1.0 / 30.0)).all(|k| at(k) == 1));
    assert_eq!(at(0), 0, "before the first tick");
}

#[test]
fn the_shield_delay_runs_down_by_halo_ticks_on_the_partys_tick() {
    let mut sim = sim();
    sim.set_tick_interval(0.015);
    let e = spartan(&mut sim);
    let fx = sim.app.world().resource::<DamageEffects>().clone();
    let amount = fx.get(ids::PISTOL_BULLET).unwrap().amount(1.0, 0.0);
    sim.app
        .world_mut()
        .write_message(HaloHit::direct(e, ids::PISTOL_BULLET, amount, 1.0, None, Vec3::ZERO, Vec3::NEG_Z, None));
    sim.ticks(1);
    let (d0, t0) = (vit(&sim, e).delay, sim.tick());
    assert!(d0 > 170, "the delay restarted: {d0}");
    let delta = Duration::from_secs_f64(0.015);
    sim.seconds(2.0);
    let ran = crossed(sim.tick(), delta) - crossed(t0, delta);
    assert!((59..=61).contains(&ran), "two seconds are 60 Halo ticks: {ran}");
    assert_eq!(u64::from(d0 - vit(&sim, e).delay), ran, "the delay ran down once a Halo tick");
    // The rest of the 6 s delay and the 4 s recharge: full after 10 s.
    sim.seconds(4.1);
    assert!(vit(&sim, e).shield < 1.0, "still recharging at 6 s");
    sim.seconds(4.2);
    assert_eq!(vit(&sim, e).shield, 1.0, "full after 10 s");
}
