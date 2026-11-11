//! Per-thread CPU time and hardware counters.
//!
//! CPU time: `clock_gettime(CLOCK_THREAD_CPUTIME_ID)` on Unix,
//! `GetThreadTimes` on Windows (which only advances in scheduler quanta,
//! ~15.6 ms: read it over many frames). Hardware counters (user-space
//! instructions retired, cycles, cache misses, branch misses): Linux
//! `perf_event_open`, one counter group per thread, so a reading is one
//! `read` syscall; elsewhere, or where the kernel refuses
//! (`perf_event_paranoid` above 2, containers, VMs without a PMU), `None`
//! and `hw_status` says why.
//!
//! Covers: per-thread CPU time: `clock_gettime(CLOCK_THREAD_CPUTIME_ID)`,
//! Windows `GetThreadTimes`; Linux hardware counters through `perf-event2`:
//! user-space instructions, cycles, cache and branch misses as one counter
//! group per thread (`ThreadCounters`), `AllThreads` summing every thread of
//! the process, `hw_status` saying why they're unavailable.

use std::{ops::Sub, sync::OnceLock, thread::ThreadId};

/// What a thread has used so far (or between two readings, as a
/// difference).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    /// CPU time, nanoseconds (0 where it can't be read).
    pub cpu_ns: u64,
    /// Hardware counters, when they could be read.
    pub hw: Option<Hw>,
}

/// User-space hardware counter values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hw {
    pub instructions: u64,
    pub cycles: u64,
    pub cache_misses: u64,
    pub branch_misses: u64,
}

impl Sub for Hw {
    type Output = Hw;
    fn sub(self, o: Hw) -> Hw {
        Hw {
            instructions: self.instructions.saturating_sub(o.instructions),
            cycles: self.cycles.saturating_sub(o.cycles),
            cache_misses: self.cache_misses.saturating_sub(o.cache_misses),
            branch_misses: self.branch_misses.saturating_sub(o.branch_misses),
        }
    }
}

impl std::ops::Add for Hw {
    type Output = Hw;
    fn add(self, o: Hw) -> Hw {
        Hw {
            instructions: self.instructions + o.instructions,
            cycles: self.cycles + o.cycles,
            cache_misses: self.cache_misses + o.cache_misses,
            branch_misses: self.branch_misses + o.branch_misses,
        }
    }
}

impl Sub for Counts {
    type Output = Counts;
    fn sub(self, o: Counts) -> Counts {
        Counts {
            cpu_ns: self.cpu_ns.saturating_sub(o.cpu_ns),
            hw: self.hw.zip(o.hw).map(|(a, b)| a - b),
        }
    }
}

impl std::ops::Add for Counts {
    type Output = Counts;
    fn add(self, o: Counts) -> Counts {
        Counts {
            cpu_ns: self.cpu_ns + o.cpu_ns,
            hw: self.hw.zip(o.hw).map(|(a, b)| a + b),
        }
    }
}

static HW_STATUS: OnceLock<String> = OnceLock::new();

/// "ok", or why hardware counters are unavailable (the first attempt's
/// error), or "not tried" before any thread opened them.
pub fn hw_status() -> &'static str {
    HW_STATUS.get().map_or("not tried", |s| s.as_str())
}

/// Counters of one thread: the thread that first reads them (`current`),
/// or a given thread of this process (`thread`, Linux).
pub struct ThreadCounters {
    owner: Option<ThreadId>,
    /// The observed thread's id for `thread`; None: the reading thread.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    tid: Option<i32>,
    #[cfg(target_os = "linux")]
    hw: Option<linux::Group>,
}

impl Default for ThreadCounters {
    fn default() -> Self {
        Self::current()
    }
}

impl ThreadCounters {
    /// Counters of whichever thread reads them (opened on the first read,
    /// and again if another thread reads them later).
    pub fn current() -> Self {
        Self {
            owner: None,
            tid: None,
            #[cfg(target_os = "linux")]
            hw: None,
        }
    }

    /// Counters of thread `tid` of this process (Linux).
    #[cfg(target_os = "linux")]
    pub fn thread(tid: i32) -> Self {
        Self {
            owner: None,
            tid: Some(tid),
            hw: linux::Group::open(tid),
        }
    }

    /// Whether hardware counters are being read.
    pub fn has_hw(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.hw.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// The thread's totals so far: one `read` for the counter group and one
    /// clock read.
    pub fn read(&mut self) -> Counts {
        #[cfg(target_os = "linux")]
        if self.tid.is_some() {
            let tid = self.tid.unwrap_or(0);
            return Counts {
                cpu_ns: linux::thread_cpu_ns(tid).unwrap_or(0),
                hw: self.hw.as_mut().and_then(|g| g.read()),
            };
        }
        let me = std::thread::current().id();
        if self.owner != Some(me) {
            self.owner = Some(me);
            #[cfg(target_os = "linux")]
            {
                self.hw = linux::Group::open(0);
            }
        }
        Counts {
            cpu_ns: thread_cpu_ns().unwrap_or(0),
            #[cfg(target_os = "linux")]
            hw: self.hw.as_mut().and_then(|g| g.read()),
            #[cfg(not(target_os = "linux"))]
            hw: {
                let _ = HW_STATUS.set("not supported on this platform (Linux only)".into());
                None
            },
        }
    }
}

/// One thread's user-space instructions alone: a single counter, read
/// with one small `read` and no group bookkeeping (`metrics::work` reads
/// one at every system's start and end). None where `ThreadCounters` has
/// none.
pub struct InstructionCounter {
    #[cfg(target_os = "linux")]
    counter: Option<perf_event::Counter>,
    /// Whether a read was scaled up: the kernel multiplexed the counter
    /// (more events wanted on the thread than the PMU has counters), so
    /// that count was an estimate.
    scaled: bool,
}

impl InstructionCounter {
    /// The calling thread's (Linux; elsewhere always None).
    pub fn current() -> Self {
        Self::open(0)
    }

    /// Thread `tid` of this process (Linux; 0: the calling thread).
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
    pub fn open(tid: i32) -> Self {
        #[cfg(target_os = "linux")]
        {
            let mut b = perf_event::Builder::new(perf_event::events::Hardware::INSTRUCTIONS);
            if tid != 0 {
                b.observe_pid(tid);
            }
            b.exclude_kernel(true).exclude_hv(true).read_format(
                perf_event::ReadFormat::TOTAL_TIME_ENABLED | perf_event::ReadFormat::TOTAL_TIME_RUNNING,
            );
            let counter = match b.build() {
                Ok(mut c) => c.enable().ok().map(|_| c),
                Err(e) => {
                    let _ = HW_STATUS.set(format!("unavailable ({e})"));
                    None
                }
            };
            Self { counter, scaled: false }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = HW_STATUS.set("not supported on this platform (Linux only)".into());
            Self { scaled: false }
        }
    }

    /// Whether any read so far was an estimate (multiplexed).
    pub fn scaled(&self) -> bool {
        self.scaled
    }

    /// Instructions so far (scaled up if the kernel multiplexed the
    /// counter), or None.
    pub fn read(&mut self) -> Option<u64> {
        #[cfg(target_os = "linux")]
        {
            let c = self.counter.as_mut()?.read_count_and_time().ok()?;
            if c.time_running == 0 {
                return (c.time_enabled == 0).then_some(c.count);
            }
            Some(if c.time_running < c.time_enabled {
                self.scaled = true;
                (c.count as u128 * c.time_enabled as u128 / c.time_running as u128) as u64
            } else {
                c.count
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }
}

/// `CLOCK_MONOTONIC` in ns (Unix): what `perf record -k CLOCK_MONOTONIC`
/// stamps its samples with, so a frame log's frames can be cut out of a
/// profile (`client::frame_metrics`, `CHIMERA_FRAME_LOG`).
pub fn monotonic_ns() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: `ts` is a valid, writable timespec.
        let r = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
        (r == 0).then(|| ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// The process's CPU time so far (every thread), nanoseconds; Unix.
pub fn process_cpu_ns() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: a valid clock id and a pointer to a live timespec.
        let r = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
        (r == 0).then(|| ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// The calling thread's CPU time so far, nanoseconds.
pub fn thread_cpu_ns() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: a valid clock id and a pointer to a live timespec.
        let r = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
        (r == 0).then(|| ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64)
    }
    #[cfg(windows)]
    {
        windows::thread_cpu_ns()
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Every thread of this process (Linux): counters summed over all of
/// them, for work spread over the task pools. Threads are found in
/// `/proc/self/task` at each read; a thread's counts start when it is
/// first seen, and stay in the sum after it exits.
#[derive(Default)]
pub struct AllThreads {
    #[cfg(target_os = "linux")]
    threads: Vec<(i32, ThreadCounters)>,
}

impl AllThreads {
    /// Sum over the process's threads (only the calling thread outside
    /// Linux).
    pub fn read(&mut self) -> Counts {
        #[cfg(target_os = "linux")]
        {
            if let Ok(dir) = std::fs::read_dir("/proc/self/task") {
                for e in dir.flatten() {
                    if let Some(tid) = e.file_name().to_str().and_then(|s| s.parse::<i32>().ok())
                        && !self.threads.iter().any(|(t, _)| *t == tid)
                    {
                        self.threads.push((tid, ThreadCounters::thread(tid)));
                    }
                }
            }
            let mut sum = Counts {
                cpu_ns: 0,
                hw: Some(Hw::default()),
            };
            for (_, t) in &mut self.threads {
                sum = sum + t.read();
            }
            sum
        }
        #[cfg(not(target_os = "linux"))]
        {
            Counts {
                cpu_ns: thread_cpu_ns().unwrap_or(0),
                hw: None,
            }
        }
    }

    /// Threads counted so far.
    pub fn threads(&self) -> usize {
        #[cfg(target_os = "linux")]
        {
            self.threads.len()
        }
        #[cfg(not(target_os = "linux"))]
        {
            1
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use perf_event::{Builder, Counter, events::Hardware};

    use super::{HW_STATUS, Hw};

    pub struct Group {
        group: perf_event::Group,
        counters: [Counter; 4],
    }

    impl Group {
        /// User-space counters of thread `tid` (0: the calling thread).
        pub fn open(tid: i32) -> Option<Group> {
            match Self::try_open(tid) {
                Ok(g) => {
                    let _ = HW_STATUS.set("ok".into());
                    Some(g)
                }
                Err(e) => {
                    let _ = HW_STATUS.set(format!("unavailable ({e})"));
                    None
                }
            }
        }

        fn try_open(tid: i32) -> std::io::Result<Group> {
            let mut builder = perf_event::Group::builder();
            if tid != 0 {
                builder.observe_pid(tid);
            }
            let mut group = builder.build_group()?;
            let mut add = |event: Hardware| {
                let mut b = Builder::new(event);
                if tid != 0 {
                    b.observe_pid(tid);
                }
                // User space only: what `perf_event_paranoid` 2 allows,
                // and what our code does (syscalls' kernel time depends on
                // the machine's state).
                b.exclude_kernel(true).exclude_hv(true);
                group.add(&b)
            };
            let counters = [
                add(Hardware::INSTRUCTIONS)?,
                add(Hardware::CPU_CYCLES)?,
                add(Hardware::CACHE_MISSES)?,
                add(Hardware::BRANCH_MISSES)?,
            ];
            group.enable()?;
            Ok(Group { group, counters })
        }

        /// Current totals, scaled up when the kernel multiplexed the group
        /// (other counters wanted the PMU); None if it never ran.
        pub fn read(&mut self) -> Option<Hw> {
            let data = self.group.read().ok()?;
            let scale = match (data.time_enabled(), data.time_running()) {
                (Some(e), Some(r)) if r.is_zero() && !e.is_zero() => return None,
                (Some(e), Some(r)) if r < e => e.as_secs_f64() / r.as_secs_f64(),
                _ => 1.0,
            };
            let v = |c: &Counter| (data.get(c).map_or(0, |e| e.value()) as f64 * scale) as u64;
            Some(Hw {
                instructions: v(&self.counters[0]),
                cycles: v(&self.counters[1]),
                cache_misses: v(&self.counters[2]),
                branch_misses: v(&self.counters[3]),
            })
        }
    }

    /// Thread `tid`'s CPU time: its per-thread CPU clock
    /// (`MAKE_THREAD_CPUCLOCK(tid, CPUCLOCK_SCHED)` in the kernel).
    pub fn thread_cpu_ns(tid: i32) -> Option<u64> {
        let clock = ((!tid) << 3) | 4 | 2;
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: a pointer to a live timespec; a bad clock id only fails.
        let r = unsafe { libc::clock_gettime(clock as libc::clockid_t, &mut ts) };
        (r == 0).then(|| ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64)
    }
}

#[cfg(windows)]
mod windows {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> isize;
        fn GetThreadTimes(
            thread: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }

    /// User plus kernel time of the calling thread (100 ns units).
    pub fn thread_cpu_ns() -> Option<u64> {
        let mut t = [FileTime::default(); 4];
        let [c, e, k, u] = &mut t;
        // SAFETY: the pseudo-handle of the calling thread and four live
        // FILETIMEs.
        let ok = unsafe { GetThreadTimes(GetCurrentThread(), c, e, k, u) };
        let v = |f: &FileTime| ((f.high as u64) << 32 | f.low as u64) * 100;
        (ok != 0).then(|| v(&t[2]) + v(&t[3]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_grow_with_work() {
        let mut c = ThreadCounters::current();
        let a = c.read();
        // Burn work at least once, then until the thread's CPU clock moves.
        // One round is enough where the clock is fine (Linux: nanoseconds),
        // but Windows reads it from GetThreadTimes in 15.6 ms steps
        // (docs/performance.md), so how much more it takes is the clock's
        // business, not the test's. (Checking the clock before the first
        // round skipped the work on Linux, where two reads always differ.)
        let started = std::time::Instant::now();
        let mut x = 0u64;
        loop {
            for i in 0..2_000_000u64 {
                x = x.wrapping_mul(31).wrapping_add(std::hint::black_box(i));
            }
            std::hint::black_box(x);
            if c.read().cpu_ns != a.cpu_ns {
                break;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "the thread's CPU clock did not move in 10 s: {a:?}"
            );
        }
        let d = c.read() - a;
        assert!(d.cpu_ns > 0, "{d:?}");
        if let Some(hw) = d.hw {
            // At least the loop's adds and multiplies.
            assert!(hw.instructions > 4_000_000, "{hw:?}");
            assert!(hw.cycles > 0, "{hw:?}");
        } else {
            eprintln!("hardware counters: {}", hw_status());
        }
    }
}
