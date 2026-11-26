//! `CHIMERA_BOOT_LOG=1`: where a process's start goes before its first
//! frame, a line on stderr per stage (the log isn't up yet) with the
//! instructions of the whole process since the last stage (the starting
//! thread's and those of every thread started since: the same from run to
//! run, whatever the box's load), the starting thread's own among them
//! (what the stage itself ran, without what other threads still load or
//! fetch beside it) and the wall time (for reading only).
//! A client app logs its start map and each part it composes
//! (`chimera_client::client::app`), a party each game's simulation
//! (`party_kit::PartyPartsPlugin`), then past the first frame when a host
//! listens (`listening`) and each map settles (`a map settled`, a hosting
//! run's start map first); the start map's own phases are
//! `chimera_world::map::load_report`'s, the frames after are
//! `--frame-log`'s; a costly step inside a stage says its own count
//! (`within`: the first frame's reads of the installs).
//! docs/performance.md, "Every game installed".

use std::sync::Mutex;

use super::InstructionCounter;

struct Boot {
    counter: InstructionCounter,
    at: Option<u64>,
    /// The starting thread alone (what the stage itself ran, without the
    /// loads and fetches other threads still do beside it).
    own: InstructionCounter,
    own_at: Option<u64>,
    began: std::time::Instant,
}

/// The process's boot log (`start`).
static BOOT: Mutex<Option<Boot>> = Mutex::new(None);

/// Start the boot log if `CHIMERA_BOOT_LOG=1` (once, on the thread that
/// starts the app, before it starts others).
pub fn start() {
    if !std::env::var("CHIMERA_BOOT_LOG").is_ok_and(|v| v == "1") {
        return;
    }
    let mut counter = InstructionCounter::current_with_children();
    let at = counter.read();
    let mut own = InstructionCounter::current();
    let own_at = own.read();
    *BOOT.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(Boot { counter, at, own, own_at, began: std::time::Instant::now() });
}

/// The start's stage `name` ended (nothing without `start`).
pub fn stage(name: &str) {
    let mut boot = BOOT.lock().unwrap_or_else(|e| e.into_inner());
    let Some(log) = boot.as_mut() else { return };
    let now = log.counter.read();
    let own_now = log.own.read();
    let own = log.own_at.zip(own_now).map_or(String::new(), |(a, b)| format!(" ({} M this thread's)", b.saturating_sub(a) / 1_000_000));
    let instructions = log
        .at
        .zip(now)
        .map_or(String::new(), |(a, b)| format!("{} M instructions{own}, ", b.saturating_sub(a) / 1_000_000));
    eprintln!("boot: {name}: {instructions}{:.0} ms", log.began.elapsed().as_secs_f64() * 1000.0);
    log.at = now;
    log.own_at = own_now;
    log.began = std::time::Instant::now();
}

/// Run `f` and say what it ran on the starting thread, as a line of its
/// own inside the stage it is part of (`boot:   within <name>: N M this
/// thread's`; the stages' counts are unchanged): a costly step of a
/// frame's systems (the first frame's reads of the installs), which a
/// stage would lump with the systems run before it. For the starting
/// thread only (the counter is that thread's); nothing without `start`.
pub fn within<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let before = {
        let mut boot = BOOT.lock().unwrap_or_else(|e| e.into_inner());
        match boot.as_mut() {
            Some(log) => log.own.read(),
            None => return f(),
        }
    };
    let out = f();
    let mut boot = BOOT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(log) = boot.as_mut()
        && let Some((a, b)) = before.zip(log.own.read())
    {
        eprintln!("boot:   within {name}: {} M this thread's", b.saturating_sub(a) / 1_000_000);
    }
    out
}
