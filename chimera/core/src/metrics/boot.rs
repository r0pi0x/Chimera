//! `CHIMERA_BOOT_LOG=1`: where a process's start goes before its first
//! frame, a line on stderr per stage (the log isn't up yet) with the
//! instructions of the whole process since the last stage (the starting
//! thread's and those of every thread started since: the same from run to
//! run, whatever the box's load) and the wall time (for reading only).
//! A client app logs its start map and each part it composes
//! (`chimera_client::client::app`), a party each game's simulation
//! (`party_kit::PartyPartsPlugin`); the start map's own phases are
//! `chimera_world::map::load_report`'s, the frames after are
//! `--frame-log`'s. docs/performance.md, "Every game installed".

use std::sync::Mutex;

use super::InstructionCounter;

struct Boot {
    counter: InstructionCounter,
    at: Option<u64>,
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
    *BOOT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Boot { counter, at, began: std::time::Instant::now() });
}

/// The start's stage `name` ended (nothing without `start`).
pub fn stage(name: &str) {
    let mut boot = BOOT.lock().unwrap_or_else(|e| e.into_inner());
    let Some(log) = boot.as_mut() else { return };
    let now = log.counter.read();
    let instructions =
        log.at.zip(now).map_or(String::new(), |(a, b)| format!("{} M instructions, ", b.saturating_sub(a) / 1_000_000));
    eprintln!("boot: {name}: {instructions}{:.0} ms", log.began.elapsed().as_secs_f64() * 1000.0);
    log.at = now;
    log.began = std::time::Instant::now();
}
