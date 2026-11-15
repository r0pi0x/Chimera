//! How many of Halo's 30 Hz ticks an engine tick advances (`halo_ticks`),
//! exactly, at any engine tick rate and for any run length: the count is
//! integer arithmetic on the engine's tick index (`SimClock::tick`), never
//! accumulated or rounded floats. Engine tick k ends at k engine tick
//! lengths of simulated time, and advances the 30 Hz boundaries it crosses:
//! floor(30 k T) - floor(30 (k - 1) T), with T the tick's length as a
//! fraction of a second. A tick length within a nanosecond of 1/n second
//! (Halo's own 1/30 s, a 64 Hz tick) is exactly 1/n; any other is its
//! nanoseconds over 10^9 (0.015 s: 0.45 Halo ticks an engine tick). Over a
//! run of one tick length the counts add up to floor(30 x the elapsed
//! time) exactly; at Halo's own tick it is 1 on every tick.
//!
//! Who counts with it: the weapons (`weapons::systems`), the view machine
//! (`view::drive`), the shots' effects (`fx::shots`), the Spartan's body
//! (`body::animate`) and the vitality's tick (`vitality::update`). The
//! movement does not count ticks: it scales one step per engine tick by
//! the tick's length (`movement::tick_scale`).
//!
//! A client predicting a command uses the server's tick index for it, so
//! both count the same Halo ticks on the same engine tick. A change of tick
//! length (another map's game) keeps the count's rule: each tick's count
//! uses its own length.

use std::time::Duration;

use crate::core::SimClock;

const NANOS: u128 = 1_000_000_000;

/// Halo ticks per engine tick of `delta` as a fraction (numerator,
/// denominator): 30 / n for a tick within a nanosecond of 1/n s, else
/// 30 x its nanoseconds over 10^9.
pub fn ratio(delta: Duration) -> (u128, u128) {
    let ns = delta.as_nanos().max(1);
    let hz = NANOS.div_ceil(ns).max(1);
    for n in [hz.saturating_sub(1).max(1), hz] {
        // 1/n s in nanoseconds, rounded either way, is `ns` give or take one.
        if (n * ns).abs_diff(NANOS) <= n {
            return (30, n);
        }
    }
    (30 * ns, NANOS)
}

/// The 30 Hz boundaries crossed by the end of engine tick `tick` of length
/// `delta` (from the run's start).
pub fn crossed(tick: u64, delta: Duration) -> u64 {
    let (num, den) = ratio(delta);
    (u128::from(tick) * num / den) as u64
}

/// The Halo ticks engine tick `clock.tick` advances (module doc): 1 at
/// Halo's own tick; 0 for the tick before the first.
pub fn halo_ticks(clock: &SimClock) -> u32 {
    if clock.tick == 0 {
        return 0;
    }
    (crossed(clock.tick, clock.delta) - crossed(clock.tick - 1, clock.delta)) as u32
}
