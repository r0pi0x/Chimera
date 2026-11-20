//! Counting background work, for waiting on it in tests and scenes: `spawn`
//! starts a task on a pool and counts it until it has run (or is dropped),
//! `thread` and `Busy` do the same for a job on a thread of its own;
//! `pending()` is the count (re-exported by `tasks`, which holds the
//! story). Measurement only: no simulation reads it, so it cannot make
//! two runs differ (checks/tests/it/determinism.rs lists this file as not
//! simulation).
//!
//! Two counts: `pending()`, the process's (spawn sites have no world), and
//! `pending_here()`, the work started from this thread and from the work
//! it started (a task's or a counted thread's own spawns count for the
//! thread that started it). Tests in one process (`cargo test`'s threads)
//! share the first; the harness waits on the second, so one test's loads
//! don't keep another waiting (a map test timed out on 23 tasks of other
//! tests under load).

use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

use bevy::tasks::{Task, TaskPool};

static PENDING: AtomicUsize = AtomicUsize::new(0);

/// The count work started on a thread goes to: the thread's own, or while
/// a counted task or thread runs, the one of the thread that started it.
type Scope = Arc<AtomicUsize>;

thread_local! {
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

fn current_scope() -> Scope {
    SCOPE.with(|s| s.borrow_mut().get_or_insert_with(Scope::default).clone())
}

/// Run `f` with `scope` as this thread's (restored after).
fn in_scope<R>(scope: &Scope, f: impl FnOnce() -> R) -> R {
    let before = SCOPE.with(|s| s.borrow_mut().replace(scope.clone()));
    struct Restore(Option<Scope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            SCOPE.with(|s| *s.borrow_mut() = before);
        }
    }
    let _restore = Restore(before);
    f()
}

/// One unit of background work counted in `pending()` (and in this
/// thread's `pending_here()`) while it lives: held by the future `spawn`
/// makes, or by a thread's closure.
#[must_use = "the work counts while this lives"]
pub struct Busy(Scope);

impl Busy {
    pub fn new() -> Busy {
        let scope = current_scope();
        PENDING.fetch_add(1, Ordering::SeqCst);
        scope.fetch_add(1, Ordering::SeqCst);
        Busy(scope)
    }
}

impl Default for Busy {
    fn default() -> Self {
        Busy::new()
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
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

/// `pending()` of the work this thread started (and that work's own
/// spawns): what a test on this thread waits for, whatever other tests in
/// the process are doing. Work started from a system on another thread
/// (Bevy's multi-threaded executor) isn't in it: wait on `pending()` then.
pub fn pending_here() -> usize {
    current_scope().load(Ordering::SeqCst)
}

/// A future polled with its starter's scope as the thread's.
struct Scoped<F> {
    scope: Scope,
    fut: Pin<Box<F>>,
}

impl<F: Future> Future for Scoped<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let this = &mut *self;
        in_scope(&this.scope, || this.fut.as_mut().poll(cx))
    }
}

/// `std::thread::spawn(job)`, counted in `pending()` until `job` returns
/// (or unwinds): for a download or other blocking job on a thread of its
/// own that a test waits for.
pub fn thread<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> std::thread::JoinHandle<T> {
    let busy = Busy::new();
    std::thread::spawn(move || {
        let scope = busy.0.clone();
        let _busy = busy;
        in_scope(&scope, job)
    })
}

/// `pool.spawn(fut)`, counted in `pending()` until `fut` has finished or
/// the task is dropped. Use it for every background task a test might
/// need to wait for.
pub fn spawn<T: Send + 'static>(pool: &TaskPool, fut: impl std::future::Future<Output = T> + Send + 'static) -> Task<T> {
    let busy = Busy::new();
    let scope = busy.0.clone();
    let fut = async move {
        let out = fut.await;
        drop(busy);
        out
    };
    pool.spawn(Scoped { scope, fut: Box::pin(fut) })
}
