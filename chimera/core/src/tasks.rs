//! Tasks the simulation waits for (a navigation mesh, a body's model):
//! polled once a tick so the main thread never blocks, which makes the
//! tick their result lands on depend on how fast the task's thread ran,
//! so on the machine's load. With `WaitForTasks` in the world (the
//! headless harness's repeatable runs, `harness::Sim::repeatable`) `poll`
//! waits for the result instead, and the stepped map spawn takes no time
//! budget, so a scenario plays out the same on every run
//! (docs/performance.md, "Work counts").
//!
//! Pending work: `spawn` starts a task on a pool and counts it until it
//! has run (or is dropped); `Busy` counts a job on a thread of its own
//! the same way. `pending()` is the count, process-wide; `pending_here()`
//! counts only the work this thread started (and that work's own spawns),
//! so tests in one process (`cargo test`'s threads) don't wait on each
//! other's. The headless harness's `Sim::run_until_idle` ticks until this
//! thread's is zero, so a test
//! waits for a map load, a model build or a download instead of
//! sleeping. A loader that spawns work through `spawn` or holds a `Busy`
//! shows up there; one that doesn't is invisible to it.
//!
//! `ReadAhead`: a game's loads started ahead of need kept to a few at once
//! on the async compute pool, so what is wanted now does not queue behind
//! them.
//!
//! `Behind`: a value a system reads from an install, read on the async
//! compute pool instead of on its frame (the HUDs' pictures and the guns'
//! names at the start).

use bevy::{
    prelude::*,
    tasks::{Task, block_on, poll_once},
};

pub use crate::pending_work::{Busy, pending, pending_here, spawn, thread};

/// Wait for tasks' results on the tick they're first polled, and spawn a
/// map in one go: runs repeat tick for tick whatever the machine's load.
/// Measurement and determinism checks only: a game would stall on it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WaitForTasks;

/// The app runs under a test harness (`harness::Sim` inserts it before any
/// plugin): its clock is stepped by hand, and what only exists for a
/// player's eyes takes no time: the UI's animations snap
/// (`chimera_ui::motion::init`: `ui_motion_scale` 0). A test of an
/// animation itself sets `Motion` back.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct TestRun;

/// The task's result: now if it is done, or, with `wait` (the world holds
/// `WaitForTasks`), once it is done.
pub fn poll<T>(task: &mut Task<T>, wait: bool) -> Option<T> {
    if wait {
        Some(block_on(task))
    } else {
        block_on(poll_once(task))
    }
}

/// Whether `world` waits for tasks (`WaitForTasks`).
pub fn waits(world: &World) -> bool {
    world.contains_resource::<WaitForTasks>()
}

/// Tasks a game starts ahead of need (a match's ladder's models, read before
/// any rung gives them) kept to `cap` on the async compute pool at once, so
/// what is wanted now never waits behind them: a worn body's cook, a held
/// model's, a pipeline Bevy makes for a model shown for the first time
/// (docs/performance.md, "Late models shown whole"). Each loader says how
/// many of its own read-ahead tasks are on their way (`set`) and starts
/// another only while there is `room`; wanted-now loads start regardless.
/// Without this resource in the world, loaders start everything at once.
#[derive(Resource, Debug)]
pub struct ReadAhead {
    pub cap: usize,
    in_flight: std::collections::BTreeMap<&'static str, usize>,
}

impl Default for ReadAhead {
    /// Half the async compute pool's threads (at least one; Bevy's most,
    /// four, before the pool is made): the rest are for what is wanted now.
    fn default() -> Self {
        let threads = bevy::tasks::AsyncComputeTaskPool::try_get().map_or(4, |p| p.thread_num());
        Self::with_cap((threads / 2).max(1))
    }
}

impl ReadAhead {
    pub fn with_cap(cap: usize) -> Self {
        Self { cap, in_flight: Default::default() }
    }

    /// Loader `who` has `n` read-ahead tasks on their way.
    pub fn set(&mut self, who: &'static str, n: usize) {
        self.in_flight.insert(who, n);
    }

    /// Read-ahead tasks on their way, all loaders'.
    pub fn in_flight(&self) -> usize {
        self.in_flight.values().sum()
    }

    /// Whether another read-ahead task may start.
    pub fn room(&self) -> bool {
        self.in_flight() < self.cap
    }
}

/// Whether a read-ahead task may start (`ReadAhead::room`; none: always).
pub fn read_ahead_room(ahead: Option<&ReadAhead>) -> bool {
    ahead.is_none_or(ReadAhead::room)
}

/// A value read behind the frames: a system that would read an install
/// on the main thread (a HUD's pictures, the guns' names: opening a
/// Source game's mount for them cost 0.6 to 2.7 G instructions of the
/// first frames; docs/performance.md, "Start cost, fifth pass") starts
/// the read on the async
/// compute pool (`start`) and takes the value on the frame it is in
/// (`take`). Without the pool (a bare test app) the read runs at once, and
/// a world that waits for tasks (`WaitForTasks`) takes it on the frame
/// it started, so a test sees what a synchronous read gave.
pub struct Behind<T>(BehindState<T>);

enum BehindState<T> {
    Task(Task<T>),
    Ready(T),
    Taken,
}

impl<T: Send + 'static> Behind<T> {
    /// Start `read` (counted as pending work until it has run: `spawn`).
    pub fn start(read: impl FnOnce() -> T + Send + 'static) -> Self {
        Self(match bevy::tasks::AsyncComputeTaskPool::try_get() {
            Some(pool) => BehindState::Task(spawn(pool, async move { read() })),
            None => BehindState::Ready(read()),
        })
    }

    /// The value, once: now if the read is done, or, with `wait` (the
    /// world holds `WaitForTasks`), when it is; None while it reads and
    /// after it was taken.
    pub fn take(&mut self, wait: bool) -> Option<T> {
        match std::mem::replace(&mut self.0, BehindState::Taken) {
            BehindState::Ready(value) => Some(value),
            BehindState::Taken => None,
            BehindState::Task(mut task) => match poll(&mut task, wait) {
                Some(value) => Some(value),
                None => {
                    self.0 = BehindState::Task(task);
                    None
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A read behind the frames gives what the read gives, once: at once
    /// when waited for, else on a later poll.
    #[test]
    fn a_read_behind_gives_the_reads_value_once() {
        let read = || (1..=10u64).product::<u64>();
        // Whatever the process has (a pool or none): waited for.
        let mut waited = Behind::start(read);
        assert_eq!(waited.take(true), Some(read()));
        assert_eq!(waited.take(true), None, "taken once");
        // On the pool: in after some polls, the same value.
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        let mut polled = Behind::start(read);
        let value = loop {
            if let Some(v) = polled.take(false) {
                break v;
            }
            std::thread::yield_now();
        };
        assert_eq!(value, read());
        assert_eq!(polled.take(false), None);
    }
}
