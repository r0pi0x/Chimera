//! Tasks the simulation waits for (a navigation mesh, a body's model):
//! polled once a tick so the main thread never blocks, which makes the
//! tick their result lands on depend on how fast the task's thread ran,
//! so on the machine's load. With `WaitForTasks` in the world (the
//! headless harness's repeatable runs, `harness::Sim::repeatable`) `poll`
//! waits for the result instead, and the stepped map spawn takes no time
//! budget, so a scenario plays out the same on every run
//! (docs/performance.md, "Work counts").

use bevy::{
    prelude::*,
    tasks::{Task, block_on, poll_once},
};

/// Wait for tasks' results on the tick they're first polled, and spawn a
/// map in one go: runs repeat tick for tick whatever the machine's load.
/// Measurement and determinism checks only: a game would stall on it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WaitForTasks;

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
