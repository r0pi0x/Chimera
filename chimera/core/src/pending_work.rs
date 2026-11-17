//! Counting background work, for waiting on it in tests and scenes: `spawn`
//! starts a task on a pool and counts it until it has run (or is dropped),
//! `thread` and `Busy` do the same for a job on a thread of its own;
//! `pending()` is the count (re-exported by `tasks`, which holds the
//! story). Process-wide, because spawn sites have no world: tests in one
//! process share it. Measurement only: no simulation reads it, so it
//! cannot make two runs differ (checks/tests/it/determinism.rs lists this
//! file as not simulation).

use std::sync::atomic::{AtomicUsize, Ordering};

use bevy::tasks::{Task, TaskPool};

static PENDING: AtomicUsize = AtomicUsize::new(0);

/// One unit of background work counted in `pending()` while it lives: held
/// by the future `spawn` makes, or by a thread's closure.
#[must_use = "the work counts while this lives"]
pub struct Busy(());

impl Busy {
    pub fn new() -> Busy {
        PENDING.fetch_add(1, Ordering::SeqCst);
        Busy(())
    }
}

impl Default for Busy {
    fn default() -> Self {
        Busy::new()
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        PENDING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Background work running or queued now (`spawn`ed tasks that haven't
/// finished, live `Busy` guards). A finished task's result may still wait
/// for the system that polls it: tick a few more times after this is 0
/// (`Sim::run_until_idle` does).
pub fn pending() -> usize {
    PENDING.load(Ordering::SeqCst)
}

/// `std::thread::spawn(job)`, counted in `pending()` until `job` returns
/// (or unwinds): for a download or other blocking job on a thread of its
/// own that a test waits for.
pub fn thread<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> std::thread::JoinHandle<T> {
    let busy = Busy::new();
    std::thread::spawn(move || {
        let _busy = busy;
        job()
    })
}

/// `pool.spawn(fut)`, counted in `pending()` until `fut` has finished or
/// the task is dropped. Use it for every background task a test might
/// need to wait for.
pub fn spawn<T: Send + 'static>(pool: &TaskPool, fut: impl std::future::Future<Output = T> + Send + 'static) -> Task<T> {
    let busy = Busy::new();
    pool.spawn(async move {
        let out = fut.await;
        drop(busy);
        out
    })
}
