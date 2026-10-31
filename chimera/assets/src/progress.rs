//! Human-readable progress of fetches from the asset service.
//!
//! - `Transfer` (`chimera_assets fetch`): a known total of files and bytes.
//!   On a terminal, one live line rewritten in place (files and MB
//!   done/total, speed, time left); elsewhere (a log, CI, a pipe) a plain
//!   line every 5 s; `finish` prints the summary.
//! - `on_demand` (the game, tests and tools: `CachedFiles` fetching files a
//!   load asks for): fetches are counted into bursts; while a burst lasts,
//!   a line every 2 s says how much has arrived and how fast, and a burst's
//!   end (no fetch for 1.5 s) prints its summary, so a slow first load
//!   clearly shows it is downloading. Nothing is printed when nothing is
//!   fetched. Lines go to stderr, prefixed `assets:`.

use std::{
    io::{IsTerminal, Write},
    sync::{Mutex, Once},
    time::{Duration, Instant},
};

/// `bytes` as MB with one decimal.
pub fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

/// A duration as `1m05s` / `12s`.
pub fn clock(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

/// MB/s of `bytes` over `d`.
fn speed(bytes: u64, d: Duration) -> String {
    format!("{:.1} MB/s", bytes as f64 / 1e6 / d.as_secs_f64().max(1e-3))
}

/// A fetch of a known size, reported as it goes.
pub struct Transfer {
    what: String,
    files: u64,
    bytes: u64,
    started: Instant,
    state: Mutex<State>,
    live: bool,
}

#[derive(Default)]
struct State {
    files: u64,
    bytes: u64,
    failed: u64,
    last: Option<Instant>,
    /// The live line's width, to clear it.
    width: usize,
}

impl Transfer {
    /// `files` files of `bytes` (as transferred, compressed) to fetch for
    /// `what`. Live when stderr is a terminal.
    pub fn new(what: &str, files: u64, bytes: u64) -> Self {
        Self::with_live(what, files, bytes, std::io::stderr().is_terminal())
    }

    pub fn with_live(what: &str, files: u64, bytes: u64, live: bool) -> Self {
        Self { what: what.into(), files, bytes, started: Instant::now(), state: Mutex::new(State::default()), live }
    }

    /// The progress line as it stands.
    fn line(&self, s: &State) -> String {
        let took = self.started.elapsed();
        let left = if s.bytes > 0 && self.bytes > s.bytes {
            let rate = s.bytes as f64 / took.as_secs_f64().max(1e-3);
            format!(", {} left", clock(Duration::from_secs_f64((self.bytes - s.bytes) as f64 / rate)))
        } else {
            String::new()
        };
        format!(
            "{}: {}/{} records, {:.1}/{:.1} MB, {}{left}{}",
            self.what,
            s.files,
            self.files,
            s.bytes as f64 / 1e6,
            self.bytes as f64 / 1e6,
            speed(s.bytes, took),
            if s.failed > 0 { format!(", {} failed", s.failed) } else { String::new() },
        )
    }

    /// `files` more arrived (`failed` of them refused), `bytes` more
    /// transferred.
    pub fn add(&self, files: u64, failed: u64, bytes: u64) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.files += files;
        s.failed += failed;
        s.bytes += bytes;
        let now = Instant::now();
        let every = if self.live { Duration::from_millis(200) } else { Duration::from_secs(5) };
        let due = s.last.map_or_else(|| now - self.started >= every, |l| now - l >= every);
        if !due {
            return;
        }
        s.last = Some(now);
        let line = self.line(&s);
        let mut err = std::io::stderr().lock();
        if self.live {
            let pad = s.width.saturating_sub(line.len());
            let _ = write!(err, "\r{line}{:pad$}", "");
            let _ = err.flush();
            s.width = line.len();
        } else {
            let _ = writeln!(err, "{line}");
        }
    }

    /// The summary (the live line replaced by it).
    pub fn finish(&self) -> String {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let took = self.started.elapsed();
        let summary = format!(
            "{}: fetched {} records, {} in {} ({}){}",
            self.what,
            s.files,
            mb(s.bytes),
            if took < Duration::from_secs(60) { format!("{:.1}s", took.as_secs_f64()) } else { clock(took) },
            speed(s.bytes, took),
            if s.failed > 0 { format!(", {} failed", s.failed) } else { String::new() },
        );
        if self.live && s.width > 0 {
            let _ = write!(std::io::stderr(), "\r{:w$}\r", "", w = s.width);
        }
        summary
    }
}

/// Fetches made on demand (`CachedFiles`), in bursts.
#[derive(Default)]
struct Burst {
    from: String,
    started: Option<Instant>,
    last_fetch: Option<Instant>,
    last_print: Option<Instant>,
    files: u64,
    bytes: u64,
}

static BURST: Mutex<Burst> = Mutex::new(Burst { from: String::new(), started: None, last_fetch: None, last_print: None, files: 0, bytes: 0 });

/// A burst ends after this long without a fetch.
const QUIET: Duration = Duration::from_millis(1500);
/// A progress line while a burst lasts, this often.
const EVERY: Duration = Duration::from_secs(2);

/// `files` records (`bytes` transferred) arrived on demand from `from`
/// (the service's URL). Starts the reporter on first use.
pub fn on_demand(from: &str, files: u64, bytes: u64) {
    static REPORTER: Once = Once::new();
    {
        let mut b = BURST.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if b.started.is_none() {
            b.started = Some(now);
            b.from = from.to_string();
        }
        b.last_fetch = Some(now);
        b.files += files;
        b.bytes += bytes;
    }
    REPORTER.call_once(|| {
        let _ = std::thread::Builder::new().name("asset progress".into()).spawn(|| {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                if let Some(line) = tick(Instant::now()) {
                    eprintln!("{line}");
                }
            }
        });
    });
}

/// The line to print now, if any: progress while a burst lasts, its
/// summary once it is over.
fn tick(now: Instant) -> Option<String> {
    let mut b = BURST.lock().unwrap_or_else(|e| e.into_inner());
    let (Some(started), Some(last_fetch)) = (b.started, b.last_fetch) else {
        return None;
    };
    if now - last_fetch >= QUIET {
        let took = last_fetch - started;
        let line = format!(
            "assets: fetched {} files, {} from {} in {:.1} s ({})",
            b.files,
            mb(b.bytes),
            b.from,
            took.as_secs_f64(),
            speed(b.bytes, took)
        );
        *b = Burst::default();
        return Some(line);
    }
    // Only while fetches keep coming (a pause ends in the summary).
    if now - b.last_print.unwrap_or(started) >= EVERY && now - last_fetch < Duration::from_millis(500) {
        b.last_print = Some(now);
        return Some(format!(
            "assets: downloading from {}: {} files, {} so far, {}",
            b.from,
            b.files,
            mb(b.bytes),
            speed(b.bytes, last_fetch - started)
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transfer_line_says_done_total_speed_and_time_left() {
        let t = Transfer::with_live("cs_source", 10, 10_000_000, false);
        let s = State { files: 5, bytes: 5_000_000, ..Default::default() };
        let line = t.line(&s);
        assert!(line.starts_with("cs_source: 5/10 records, 5.0/10.0 MB, "), "{line}");
        assert!(line.contains("MB/s") && line.contains("left"), "{line}");
        assert!(t.finish().contains("fetched 0 records"));
    }

    #[test]
    fn clock_reads_well() {
        assert_eq!(clock(Duration::from_secs(5)), "5s");
        assert_eq!(clock(Duration::from_secs(65)), "1m05s");
        assert_eq!(clock(Duration::from_secs(3720)), "1h02m");
    }
}
