//! A cap on how fast a background fetch reads from the asset service
//! (docs/plans/active/asset-service.md, "Playtest delivery"): the game's
//! prefetch of later maps shares the host's upload with every other
//! tester, so it reads at most `rate` bytes a second, holds while the
//! game fetches in the foreground (`pause`) and ends at `stop`.
//! `Remote::fill_paced` reads each batch answer through it in small
//! chunks (`Paced`), so the bytes leave the host's socket at that rate
//! (TCP's window holds the rest back), not in line-speed bursts.

use std::{
    io::{self, Read},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Bytes a paced read takes at once, at most: about 8 ms at 2 MB/s.
const CHUNK: usize = 16 * 1024;
/// How far ahead a pace may run after a quiet spell, in seconds of its
/// rate.
const BURST_SECS: f64 = 0.25;
/// How often a held fetch looks again.
const HOLD: Duration = Duration::from_millis(50);

/// A rate cap with a pause and a stop, shared by a fetch's threads.
pub struct Pace {
    /// Bytes a second; `u64::MAX`: no cap; 0: hold.
    rate: AtomicU64,
    paused: AtomicBool,
    stopped: AtomicBool,
    bucket: Mutex<Bucket>,
}

struct Bucket {
    /// Bytes that may go now (negative: owed).
    tokens: f64,
    last: Instant,
}

impl Default for Pace {
    fn default() -> Self {
        Self::new(u64::MAX)
    }
}

impl Pace {
    /// At most `bytes_per_sec` (`u64::MAX`: no cap; 0: hold).
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            rate: AtomicU64::new(bytes_per_sec),
            paused: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            bucket: Mutex::new(Bucket { tokens: 0.0, last: Instant::now() }),
        }
    }

    /// At most this many kilobytes (1000 bytes) a second; 0 holds.
    pub fn kb_per_sec(kb: u64) -> Self {
        Self::new(kb.saturating_mul(1000))
    }

    pub fn set_rate(&self, bytes_per_sec: u64) {
        self.rate.store(bytes_per_sec, Ordering::Relaxed);
    }

    pub fn rate(&self) -> u64 {
        self.rate.load(Ordering::Relaxed)
    }

    /// Hold (true) or go on (false).
    pub fn pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// End the fetch: every paced read fails from now on.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    /// Wait until `bytes` more may be read: while paused or held, and as
    /// long as the rate asks. Fails once stopped.
    pub fn take(&self, bytes: u64) -> io::Result<()> {
        let mut owed = bytes as f64;
        loop {
            if self.stopped() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "the fetch was stopped"));
            }
            let rate = self.rate();
            if self.paused() || rate == 0 {
                std::thread::sleep(HOLD);
                // What a pause saves isn't spent at once afterwards.
                let mut b = self.bucket.lock().unwrap_or_else(|e| e.into_inner());
                b.tokens = b.tokens.min(0.0);
                b.last = Instant::now();
                continue;
            }
            if rate == u64::MAX {
                return Ok(());
            }
            let rate = rate as f64;
            let mut b = self.bucket.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            b.tokens = (b.tokens + (now - b.last).as_secs_f64() * rate).min(rate * BURST_SECS);
            b.last = now;
            if b.tokens >= owed {
                b.tokens -= owed;
                return Ok(());
            }
            owed -= b.tokens.max(0.0);
            b.tokens = b.tokens.min(0.0);
            drop(b);
            std::thread::sleep(Duration::from_secs_f64((owed / rate).min(0.1)));
        }
    }
}

/// A reader that reads at `pace`'s rate, a chunk at a time.
pub struct Paced<'a, R> {
    pub inner: R,
    pub pace: &'a Pace,
}

impl<R: Read> Read for Paced<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len().min(CHUNK);
        let n = self.inner.read(&mut buf[..len])?;
        self.pace.take(n as u64)?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lower bounds only: sleeping is never faster than asked, however
    /// loaded the machine.
    #[test]
    fn a_pace_reads_no_faster_than_its_rate() {
        let pace = Pace::new(400_000);
        let data = vec![7u8; 200_000];
        let start = Instant::now();
        let mut out = Vec::new();
        Paced { inner: &data[..], pace: &pace }.read_to_end(&mut out).unwrap();
        assert_eq!(out.len(), data.len());
        // 200 kB at 400 kB/s with a quarter second's start: >= 0.25 s.
        assert!(start.elapsed() >= Duration::from_millis(240), "{:?}", start.elapsed());
    }

    #[test]
    fn a_paused_pace_holds_until_resumed_and_a_stop_ends_it() {
        let pace = std::sync::Arc::new(Pace::default());
        pace.pause(true);
        let p = pace.clone();
        let reader = std::thread::spawn(move || {
            let start = Instant::now();
            p.take(10).map(|()| start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(300));
        assert!(!reader.is_finished(), "held while paused");
        pace.pause(false);
        let waited = reader.join().unwrap().unwrap();
        assert!(waited >= Duration::from_millis(300), "{waited:?}");

        let held = Pace::new(0);
        held.stop();
        assert_eq!(held.take(1).unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert!(Pace::default().take(u64::MAX / 2).is_ok(), "no cap: at once");
    }
}
