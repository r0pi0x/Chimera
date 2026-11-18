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
//! `CHIMERA_SIM_INSTALLS=1` turns it off everywhere.
//!
//! Known shortcuts:
//! - A mixed `cargo test` process is only guarded until its first heavy
//!   test (module doc).

use std::sync::atomic::{AtomicU8, Ordering};

const UNSET: u8 = 0;
const FORBIDDEN: u8 = 1;
const ALLOWED: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(UNSET);

fn heavy_thread() -> bool {
    std::thread::current().name().is_some_and(|n| n.contains("heavy::"))
        || crate::env::var("CHIMERA_SIM_INSTALLS").as_deref() == Ok("1")
}

/// Installs and the asset store are off for this process from now on,
/// unless the caller is a heavy test or installs were allowed already.
pub fn forbid_installs() {
    if heavy_thread() {
        allow_installs();
    } else {
        let _ = STATE.compare_exchange(UNSET, FORBIDDEN, Ordering::SeqCst, Ordering::SeqCst);
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
