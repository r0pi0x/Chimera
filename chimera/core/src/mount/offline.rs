//! The "no installs" switch for fast-tier tests: while it is on,
//! `LocalConfig::game_path_in` (so `GameInfo::install`, `mount_for` and
//! every loader behind them) finds no install and no asset store, so a
//! scenario test on `harness::Sim` never reads the machine's game files
//! (a campaign body, a map, a guest dressing) and gives the same result
//! on every machine.
//!
//! Process-wide, since loaders read installs on pool threads. `Sim` turns
//! it on (`forbid_installs`) unless its test is a heavy one (the test
//! thread's name has `heavy::` in it) or `allow_installs` was called
//! (a test outside `heavy` that needs the files): the first of those
//! makes it stay off for the rest of the process. Under nextest (a
//! process per test) and `cargo test -- --skip heavy::` that is exact;
//! heavy tests sharing a `cargo test` process with fast ones leave the
//! fast ones unguarded after the first heavy one runs.
//! `CHIMERA_SIM_INSTALLS=1` turns it off everywhere. A `Sim` made on a
//! process's main thread (a development tool's `main`: `map_fitness`,
//! `perfgate`; libtest runs every test on a thread of its own, named after
//! it) leaves it as it is, so a tool that loads a map after making a
//! `Sim` still finds the install (`map_fitness` sweeping several maps in
//! one process found none after the first).
//!
//! Known shortcuts:
//! - A mixed `cargo test` process is only guarded until its first heavy
//!   test (module doc).

use std::sync::atomic::{AtomicU8, Ordering};

const UNSET: u8 = 0;
const FORBIDDEN: u8 = 1;
const ALLOWED: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(UNSET);

/// What `forbid_installs` does on a thread named `thread`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Guard {
    /// A fast test: installs off (unless allowed already).
    Forbid,
    /// A heavy test (or `CHIMERA_SIM_INSTALLS=1`): installs on for good.
    Allow,
    /// A process's main thread (a tool, never a libtest test): nothing.
    Leave,
}

fn guard(thread: Option<&str>, installs_env: bool) -> Guard {
    if thread.is_some_and(|n| n.contains("heavy::")) || installs_env {
        Guard::Allow
    } else if thread == Some("main") {
        Guard::Leave
    } else {
        Guard::Forbid
    }
}

/// Installs and the asset store are off for this process from now on,
/// unless the caller is a heavy test, a tool's main thread or installs
/// were allowed already.
pub fn forbid_installs() {
    let env = crate::env::var("CHIMERA_SIM_INSTALLS").as_deref() == Ok("1");
    match guard(std::thread::current().name(), env) {
        Guard::Allow => allow_installs(),
        Guard::Leave => {}
        Guard::Forbid => {
            let _ = STATE.compare_exchange(UNSET, FORBIDDEN, Ordering::SeqCst, Ordering::SeqCst);
        }
    }
}

/// Installs and the asset store are reachable (for good, in this process).
pub fn allow_installs() {
    STATE.store(ALLOWED, Ordering::SeqCst);
}

/// Whether installs are off (`forbid_installs`).
pub fn installs_forbidden() -> bool {
    STATE.load(Ordering::SeqCst) == FORBIDDEN
}

#[cfg(test)]
mod tests {
    use super::{Guard, guard};

    #[test]
    fn a_tools_main_thread_leaves_installs_on_and_a_tests_thread_forbids_them() {
        assert_eq!(guard(Some("main"), false), Guard::Leave);
        assert_eq!(guard(Some("weapons::a_fast_test"), false), Guard::Forbid);
        assert_eq!(guard(None, false), Guard::Forbid);
        assert_eq!(guard(Some("heavy::served::a_test"), false), Guard::Allow);
        assert_eq!(guard(Some("main"), true), Guard::Allow);
        // This test's own thread is libtest's, named after it.
        assert_ne!(std::thread::current().name(), Some("main"));
    }
}
