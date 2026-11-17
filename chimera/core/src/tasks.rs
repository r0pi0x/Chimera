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
//! the same way. `pending()` is the count, process-wide (tests in one
//! process share it; nextest runs one test per process). The headless
//! harness's `Sim::run_until_idle` ticks until it is zero, so a test
//! waits for a map load, a model build or a download instead of
//! sleeping. A loader that spawns work through `spawn` or holds a `Busy`
//! shows up there; one that doesn't is invisible to it.
//!
//! `ReadAhead`: a game's loads started ahead of need kept to a few at once
//! on the async compute pool, so what is wanted now does not queue behind
//! them.

use bevy::{
    prelude::*,
    tasks::{Task, block_on, poll_once},
};

pub use crate::pending_work::{Busy, pending, spawn, thread};

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
