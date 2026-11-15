//! A session's diagnostics (docs/plans/active/feedback-reports.md, P3b):
//! the rolling log, the unclean-exit marker, crash bundles. No Bevy: the
//! game's glue is `chimera_core::diagnostics` (health lines, the console's
//! lines, the `tracing` layer) and `chimera_client::client::crash` (the
//! notice, `cl_crash_reports`); the dedicated server starts one too.
//!
//! - **The log** (`Session::line`): every line through the secret filter
//!   (`redact::line`) first, then into a ring of the last `RING_LINES`
//!   in memory and appended to `<root>/logs/<prefix>-<utc time>.log` with
//!   one write each (the OS has it the moment the call returns, so a
//!   crash of the process loses nothing); a thread syncs the file to disk
//!   every `SYNC_SECS` while lines come (a power loss loses at most that
//!   much). Past `FILE_CAP` bytes the file starts over from the ring (its
//!   latest lines). The last `KEEP` sessions' logs of a prefix are kept
//!   (`rotate`), a dead session's log until its report is settled.
//! - **The marker** (`<prefix>-<time>.running`, key = value lines: app,
//!   build, net, the log's and a minidump's names, pid, start): written
//!   at the start and held locked by the running process
//!   (`File::try_lock`: the OS lets go when the process dies, however it
//!   dies); removed at a clean exit (`Session::end`). At the next start a
//!   marker no live process holds is a session that died (a crash, a
//!   freeze killed, a power loss, a blue screen): `Session::start`
//!   returns it as a `Previous`, whose `report` is a crash bundle from its
//!   log's tail and health lines (and its minidump, its held bundle).
//! - **Crashes**: `install_panic_hook` (the process's binaries; the
//!   previous hook still runs after) writes the first panic's bundle
//!   (`Kind::Crash`: `backtrace.txt` with the message, place, thread and
//!   stack, the log's tail, the health lines, `game.txt` from the context
//!   the game keeps current, `system.txt`) per `policy`: `Always` into
//!   `<root>/feedback/outbox/` (sent at the next start, or at once by a
//!   running outbox), `Ask` into `<root>/feedback/held/` until the tester
//!   says (the next start's notice), `Never` nowhere; the marker names it
//!   (`crash = <id>`), so the next start offers that bundle rather than
//!   one from the log. `crash` does the same for a render device lost or
//!   out of memory. On Windows `native::install` adds an unhandled-exception
//!   filter writing `<log stem>.dmp` with `MiniDumpWriteDump` (stacks, thread
//!   info, modules: no heap) and a line saying the exception; the next
//!   start's report attaches it with home folders scrubbed (`scrub_dump`).
//! - Health lines (`Session::health`) are log lines kept in a ring of
//!   their own too (`HEALTH_LINES`): a crash bundle's `health.txt`.
//!
//! `CrashPolicy` is `cl_crash_reports` (ask, always, never; ask by
//! default), set by the game once its config is read (`set_policy`).
//!
//! Known shortcuts: a minidump's stacks may hold a secret that was on
//! them (a token being sent); its module paths are scrubbed of home
//! folders, nothing else. Linux has no native-crash handler (a segfault
//! leaves the marker and the log, no stack). Two instances of one app
//! share the prefix: each holds its own marker, so neither reports the
//! other.

use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{Kind, ReportState, bundle, civil, now_secs, redact, utc_text};
use crate::store::write_atomic;

/// Lines kept in memory (the crash bundle's tail).
pub const RING_LINES: usize = 5000;
/// Health lines kept in memory.
pub const HEALTH_LINES: usize = 120;
/// A session file's most bytes before it starts over from the ring.
pub const FILE_CAP: u64 = 4 * 1024 * 1024;
/// Sessions of a prefix kept on disk.
pub const KEEP: usize = 5;
/// A line's most characters.
pub const LINE_MAX: usize = 2000;
/// Seconds between syncs to disk while lines come.
pub const SYNC_SECS: u64 = 2;
/// Dead sessions offered at most (older ones are settled quietly).
pub const OFFER_MAX: usize = 3;

/// `cl_crash_reports`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CrashPolicy {
    /// Keep a crash's report until the tester says (the next start asks).
    #[default]
    Ask,
    /// Send without asking.
    Always,
    /// Keep nothing (the log stays on this PC as always).
    Never,
}

impl CrashPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "ask" | "" => Some(Self::Ask),
            "always" | "1" => Some(Self::Always),
            "never" | "0" => Some(Self::Never),
            _ => None,
        }
    }
}

static POLICY: AtomicU8 = AtomicU8::new(0);

/// The policy crashes from now on follow.
pub fn set_policy(p: CrashPolicy) {
    POLICY.store(p as u8, Ordering::Relaxed);
}

pub fn policy() -> CrashPolicy {
    match POLICY.load(Ordering::Relaxed) {
        1 => CrashPolicy::Always,
        2 => CrashPolicy::Never,
        _ => CrashPolicy::Ask,
    }
}

/// What a session is started with.
#[derive(Clone, Debug, Default)]
pub struct SessionOptions {
    /// The data folder (`<data_local>/chimera`): `logs/` and `feedback/`
    /// are in it.
    pub root: PathBuf,
    /// The files' prefix: `session` (a game), `server`, `offscreen`.
    pub prefix: String,
    pub app: String,
    pub build: String,
    pub net_version: String,
}

struct Ring {
    lines: VecDeque<String>,
    health: VecDeque<String>,
    file: Option<File>,
    bytes: u64,
}

/// A running session (one a process: `current`).
pub struct Session {
    logs: PathBuf,
    feedback: PathBuf,
    log_path: PathBuf,
    marker_path: PathBuf,
    dump_path: PathBuf,
    app: String,
    build: String,
    net_version: String,
    started: u64,
    ring: Mutex<Ring>,
    /// What the game is doing (`game.txt`): map, rung, position.
    context: Mutex<String>,
    /// The PC (`system.txt`): OS, CPU, GPU and driver.
    system: Mutex<String>,
    marker: Mutex<Option<File>>,
    dirty: Arc<AtomicBool>,
    /// A crash's bundle is written (the first only).
    crashed: AtomicBool,
    /// The session must be offered at the next start though it ends.
    keep_marker: AtomicBool,
}

static CURRENT: OnceLock<Arc<Session>> = OnceLock::new();

/// The process's session, once started (`Session::start` + `make_current`).
pub fn current() -> Option<&'static Arc<Session>> {
    CURRENT.get()
}

/// `YYYYMMDD-HHMMSS` (UTC).
fn stamp(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// `HH:MM:SS.mmm` now (UTC).
fn clock() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let (.., h, mi, s) = civil(now.as_secs());
    format!("{h:02}:{mi:02}:{s:02}.{:03}", now.subsec_millis())
}

fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut at = max;
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    &text[..at]
}

/// The marker's fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkerInfo {
    pub app: String,
    pub build: String,
    pub net_version: String,
    /// The log's file name (in the same folder).
    pub log: String,
    /// A minidump's file name, written only by a native crash.
    pub dump: String,
    pub started: u64,
    pub pid: u32,
    /// A crash bundle's id, when one was written.
    pub crash: Option<String>,
}

impl MarkerInfo {
    fn text(&self) -> String {
        format!(
            "app = {}\nbuild = {}\nnet = {}\nlog = {}\ndump = {}\nstarted = {}\npid = {}\n",
            self.app, self.build, self.net_version, self.log, self.dump, self.started, self.pid
        )
    }

    pub fn parse(text: &str) -> Self {
        let mut m = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim().to_string();
            match k.trim() {
                "app" => m.app = v,
                "build" => m.build = v,
                "net" => m.net_version = v,
                "log" => m.log = v,
                "dump" => m.dump = v,
                "started" => m.started = v.parse().unwrap_or(0),
                "pid" => m.pid = v.parse().unwrap_or(0),
                "crash" if super::is_report_id(&v) => m.crash = Some(v),
                _ => {}
            }
        }
        m
    }
}

/// A plain file name (no folders), or None.
fn plain_name(name: &str) -> Option<&str> {
    (!name.is_empty() && !name.contains(['/', '\\']) && name != ".." && name != ".").then_some(name)
}

/// A session that ended without a clean exit, found at a start.
#[derive(Clone, Debug)]
pub struct Previous {
    pub marker: PathBuf,
    pub info: MarkerInfo,
    logs: PathBuf,
    feedback: PathBuf,
}

impl Previous {
    /// Its log, when it is still there.
    pub fn log_path(&self) -> Option<PathBuf> {
        plain_name(&self.info.log).map(|n| self.logs.join(n)).filter(|p| p.exists())
    }

    /// Its minidump, when a native crash wrote one.
    pub fn dump_path(&self) -> Option<PathBuf> {
        plain_name(&self.info.dump).map(|n| self.logs.join(n)).filter(|p| p.exists())
    }

    /// Its held crash bundle (`feedback/held/<id>.zip`), when its policy
    /// was ask.
    pub fn held(&self) -> Option<PathBuf> {
        let id = self.info.crash.as_ref()?;
        Some(self.feedback.join("held").join(format!("{id}.zip"))).filter(|p| p.exists())
    }

    /// Whether the session crashed with a bundle, rather than just ended
    /// (a freeze, a power loss).
    pub fn crashed(&self) -> bool {
        self.info.crash.is_some() || self.dump_path().is_some()
    }

    /// The crash report of this session, from its log: the tail, the
    /// health lines, the last context and system lines, the minidump.
    pub fn report(&self) -> ReportState {
        let text = self.log_path().and_then(|p| fs::read(p).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let all: Vec<&str> = text.lines().collect();
        let log: Vec<String> = all[all.len().saturating_sub(bundle::CRASH_LOG_LINES)..].iter().map(|l| l.to_string()).collect();
        let health: Vec<String> = all.iter().filter(|l| l.contains(" health ")).map(|l| l.to_string()).collect();
        let last_with = |tag: &str| {
            all.iter().rev().find_map(|l| l.split_once(tag).map(|(_, rest)| rest.replace(" | ", "\n")))
        };
        let native = all.iter().rev().find(|l| l.contains("native crash:")).map(|l| l.to_string());
        let ended = all.last().map_or("nothing logged".to_string(), |l| format!("last line: {}", cut(l, 200)));
        let what = match &native {
            Some(n) => n.split_once("native crash:").map_or(n.clone(), |(_, r)| format!("native crash:{r}")),
            None => "The session ended unexpectedly (no clean exit: a crash, a freeze, a power loss)".into(),
        };
        let minidump = self.dump_path().and_then(|p| fs::read(p).ok()).map(|d| scrub_dump(&d));
        ReportState {
            kind: Kind::Crash,
            text: format!("{what}\nstarted {}; {ended}", utc_text(self.info.started)),
            build: self.info.build.clone(),
            net_version: self.info.net_version.clone(),
            app: self.info.app.clone(),
            install_id: super::install_id(&self.feedback),
            game: last_with(" context ").unwrap_or_default(),
            system: last_with(" system ").unwrap_or_default(),
            log,
            health,
            minidump,
            ..Default::default()
        }
    }

    /// Send it: its held bundle into the outbox, else a bundle from its
    /// log; then settled (`settle`). The report's id.
    pub fn send(self) -> Result<String, String> {
        let outbox = self.feedback.join("outbox");
        let id = match self.held() {
            Some(held) => {
                let id = self.info.crash.clone().expect("held has an id");
                fs::create_dir_all(&outbox).map_err(|e| e.to_string())?;
                fs::rename(&held, outbox.join(format!("{id}.zip"))).map_err(|e| e.to_string())?;
                id
            }
            None => {
                let b = bundle::build(&self.report())?;
                write_atomic(&outbox.join(format!("{}.zip", b.id)), &b.bytes).map_err(|e| e.to_string())?;
                b.id
            }
        };
        self.settle();
        Ok(id)
    }

    /// Done with it, sent or not: the marker and a held bundle go (the
    /// log stays until rotated out).
    pub fn settle(self) {
        if let Some(h) = self.held() {
            let _ = fs::remove_file(h);
        }
        let _ = fs::remove_file(&self.marker);
    }
}

/// Markers of `prefix` no live process holds, newest first.
fn dead_sessions(logs: &Path, feedback: &Path, prefix: &str) -> Vec<Previous> {
    let mut out = Vec::new();
    let Ok(dir) = fs::read_dir(logs) else { return out };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.starts_with(&format!("{prefix}-")) && name.ends_with(".running")) {
            continue;
        }
        let path = e.path();
        let Ok(mut file) = OpenOptions::new().read(true).write(true).open(&path) else { continue };
        if file.try_lock().is_err() {
            // A live process holds it (another instance).
            continue;
        }
        // Through the handle that holds the lock: on Windows the lock is
        // mandatory, so another handle's read of the file is refused.
        let mut text = String::new();
        let _ = file.read_to_string(&mut text);
        let _ = file.unlock();
        drop(file);
        out.push(Previous { marker: path, info: MarkerInfo::parse(&text), logs: logs.to_path_buf(), feedback: feedback.to_path_buf() });
    }
    out.sort_by(|a, b| b.info.started.cmp(&a.info.started).then_with(|| b.marker.cmp(&a.marker)));
    out
}

/// Keep the last `KEEP` logs of `prefix` (with their minidumps), and any
/// a dead session's marker still names.
fn rotate(logs: &Path, prefix: &str, keep_too: &[String]) {
    let Ok(dir) = fs::read_dir(logs) else { return };
    let mut names: Vec<String> = dir
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&format!("{prefix}-")) && n.ends_with(".log"))
        .collect();
    names.sort();
    let excess = names.len().saturating_sub(KEEP);
    for name in names.into_iter().take(excess) {
        if keep_too.contains(&name) {
            continue;
        }
        let _ = fs::remove_file(logs.join(&name));
        let _ = fs::remove_file(logs.join(name.replace(".log", ".dmp")));
    }
}

impl Session {
    /// Start a session: the dead sessions of `options.prefix` found (to
    /// offer: `OFFER_MAX` at most, the rest settled), a new log and
    /// marker, old logs rotated out.
    pub fn start(options: SessionOptions) -> std::io::Result<(Arc<Self>, Vec<Previous>)> {
        let logs = options.root.join("logs");
        let feedback = options.root.join("feedback");
        fs::create_dir_all(&logs)?;
        let mut previous = dead_sessions(&logs, &feedback, &options.prefix);
        for p in previous.split_off(previous.len().min(OFFER_MAX)) {
            p.settle();
        }
        let started = now_secs();
        let base = format!("{}-{}", options.prefix, stamp(started));
        let mut stem = base.clone();
        let mut n = 1;
        while logs.join(format!("{stem}.log")).exists() || logs.join(format!("{stem}.running")).exists() {
            n += 1;
            stem = format!("{base}-{n}");
        }
        let log_path = logs.join(format!("{stem}.log"));
        let marker_path = logs.join(format!("{stem}.running"));
        let dump_path = logs.join(format!("{stem}.dmp"));
        // Not `append`: on Windows an append-only handle can't be truncated
        // (`set_len`) when the file starts over. The file is new, so the
        // position is its end; the cap's start-over seeks back itself.
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(&log_path)?;
        let info = MarkerInfo {
            app: options.app.clone(),
            build: options.build.clone(),
            net_version: options.net_version.clone(),
            log: format!("{stem}.log"),
            dump: format!("{stem}.dmp"),
            started,
            pid: std::process::id(),
            crash: None,
        };
        let mut marker = OpenOptions::new().create(true).truncate(true).read(true).write(true).open(&marker_path)?;
        marker.write_all(info.text().as_bytes())?;
        marker.sync_data()?;
        let _ = marker.try_lock();
        let keep_too: Vec<String> = previous.iter().map(|p| p.info.log.clone()).collect();
        rotate(&logs, &options.prefix, &keep_too);
        let session = Arc::new(Self {
            logs,
            feedback,
            log_path,
            marker_path,
            dump_path,
            app: options.app,
            build: options.build,
            net_version: options.net_version,
            started,
            ring: Mutex::new(Ring { lines: VecDeque::new(), health: VecDeque::new(), file: Some(file), bytes: 0 }),
            context: Mutex::default(),
            system: Mutex::default(),
            marker: Mutex::new(Some(marker)),
            dirty: Arc::default(),
            crashed: AtomicBool::new(false),
            keep_marker: AtomicBool::new(false),
        });
        session.line(&format!(
            "session {} started: {} {} ({}), {} {}",
            stem,
            session.app,
            session.build,
            session.net_version,
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        for p in &previous {
            session.line(&format!(
                "the session {} ({} {}) ended without a clean exit{}",
                p.info.log,
                p.info.app,
                p.info.build,
                if p.crashed() { " (crashed)" } else { "" }
            ));
        }
        Self::sync_thread(&session);
        Ok((session, previous))
    }

    /// Make this the process's session (`current`; the panic hook's and
    /// the log layer's). False when one already is.
    pub fn make_current(self: &Arc<Self>) -> bool {
        CURRENT.set(self.clone()).is_ok()
    }

    /// Syncs the file to disk every `SYNC_SECS` while lines come.
    fn sync_thread(session: &Arc<Self>) {
        let weak = Arc::downgrade(session);
        let dirty = session.dirty.clone();
        let _ = std::thread::Builder::new().name("session-log-sync".into()).spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(SYNC_SECS));
                let Some(s) = weak.upgrade() else { return };
                if dirty.swap(false, Ordering::Relaxed) {
                    let file = s.ring.lock().ok().and_then(|r| r.file.as_ref().and_then(|f| f.try_clone().ok()));
                    drop(s);
                    if let Some(f) = file {
                        let _ = f.sync_data();
                    }
                }
            }
        });
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn marker_path(&self) -> &Path {
        &self.marker_path
    }

    /// Where a native crash writes its minidump.
    pub fn dump_path(&self) -> &Path {
        &self.dump_path
    }

    /// `<root>/feedback`.
    pub fn feedback_folder(&self) -> &Path {
        &self.feedback
    }

    pub fn logs_folder(&self) -> &Path {
        &self.logs
    }

    /// A line into the log: filtered, timed, kept, written.
    pub fn line(&self, text: &str) {
        let Ok(mut ring) = self.ring.lock() else { return };
        self.push(&mut ring, text, false);
    }

    /// A health line: a log line kept in the health ring too.
    pub fn health(&self, text: &str) {
        let Ok(mut ring) = self.ring.lock() else { return };
        self.push(&mut ring, text, true);
    }

    fn push(&self, ring: &mut Ring, text: &str, health: bool) {
        for part in text.lines().filter(|l| !l.trim().is_empty()) {
            let line = format!("{} {}", clock(), cut(&redact::line(part), LINE_MAX));
            if health {
                ring.health.push_back(line.clone());
                while ring.health.len() > HEALTH_LINES {
                    ring.health.pop_front();
                }
            }
            ring.bytes += line.len() as u64 + 1;
            let over = ring.bytes > FILE_CAP;
            if let Some(f) = ring.file.as_mut() {
                let _ = f.write_all(format!("{line}\n").as_bytes());
            }
            ring.lines.push_back(line);
            while ring.lines.len() > RING_LINES {
                ring.lines.pop_front();
            }
            if over {
                // Start over from the ring (the latest lines).
                let mut text = String::from("(earlier lines cut: the file's cap)\n");
                for l in &ring.lines {
                    text.push_str(l);
                    text.push('\n');
                }
                if let Some(f) = ring.file.as_mut() {
                    let _ = f.set_len(0);
                    let _ = f.seek(SeekFrom::Start(0));
                    let _ = f.write_all(text.as_bytes());
                }
                ring.bytes = text.len() as u64;
            }
        }
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// What the game is doing now (`game.txt` of a crash): logged when it
    /// changes, as a `context` line.
    pub fn set_context(&self, text: &str) {
        let changed = self.context.lock().is_ok_and(|mut c| {
            if *c == text {
                false
            } else {
                *c = text.to_string();
                true
            }
        });
        if changed {
            self.line(&format!("context {}", text.trim_end().replace('\n', " | ")));
        }
    }

    /// The PC (`system.txt` of a crash): logged, as a `system` line.
    pub fn set_system(&self, text: &str) {
        if let Ok(mut s) = self.system.lock() {
            *s = text.to_string();
        }
        self.line(&format!("system {}", text.trim_end().replace('\n', " | ")));
    }

    /// The ring's lines, oldest first.
    pub fn tail(&self) -> Vec<String> {
        self.ring.lock().map(|r| r.lines.iter().cloned().collect()).unwrap_or_default()
    }

    /// The health ring's lines, oldest first.
    pub fn health_lines(&self) -> Vec<String> {
        self.ring.lock().map(|r| r.health.iter().cloned().collect()).unwrap_or_default()
    }

    /// A crash bundle's state now: `what` the text, `backtrace`
    /// `backtrace.txt`. (`try_lock`s: a crash inside a lock still gets
    /// what it can.)
    pub fn crash_state(&self, what: &str, backtrace: &str) -> ReportState {
        let (log, health) = match self.ring.try_lock() {
            Ok(r) => (r.lines.iter().cloned().collect(), r.health.iter().cloned().collect()),
            Err(_) => (Vec::new(), Vec::new()),
        };
        let get = |m: &Mutex<String>| m.try_lock().map(|s| s.clone()).unwrap_or_default();
        ReportState {
            kind: Kind::Crash,
            text: format!("{what}\nstarted {}", utc_text(self.started)),
            build: self.build.clone(),
            net_version: self.net_version.clone(),
            app: self.app.clone(),
            install_id: super::install_id(&self.feedback),
            game: get(&self.context),
            system: get(&self.system),
            backtrace: backtrace.to_string(),
            log,
            health,
            ..Default::default()
        }
    }

    /// A crash: logged, and the first one's bundle written per `policy`
    /// (the marker names it and stays, so the next start offers it). The
    /// bundle's path, when one was written.
    pub fn crash(&self, what: &str, backtrace: &str) -> Option<PathBuf> {
        self.crash_with(what, backtrace, policy())
    }

    /// `crash` with a given policy.
    pub fn crash_with(&self, what: &str, backtrace: &str, policy: CrashPolicy) -> Option<PathBuf> {
        if let Ok(mut ring) = self.ring.try_lock() {
            self.push(&mut ring, &format!("CRASH {what}"), false);
        }
        self.keep_marker.store(true, Ordering::Relaxed);
        if self.crashed.swap(true, Ordering::SeqCst) || policy == CrashPolicy::Never {
            return None;
        }
        let state = self.crash_state(what, backtrace);
        let bundle = bundle::build(&state).ok()?;
        let folder = if policy == CrashPolicy::Always { "outbox" } else { "held" };
        let path = self.feedback.join(folder).join(format!("{}.zip", bundle.id));
        write_atomic(&path, &bundle.bytes).ok()?;
        if let Ok(mut m) = self.marker.try_lock()
            && let Some(f) = m.as_mut()
        {
            let _ = f.seek(SeekFrom::End(0));
            let _ = f.write_all(format!("crash = {}\n", bundle.id).as_bytes());
            let _ = f.sync_data();
        }
        if let Ok(mut ring) = self.ring.try_lock() {
            self.push(&mut ring, &format!("crash report {} written to feedback/{folder}", bundle.id), false);
            if let Some(f) = ring.file.as_ref() {
                let _ = f.sync_data();
            }
        }
        Some(path)
    }

    /// A clean exit: the marker goes (unless a crash wants it offered).
    pub fn end(&self) {
        self.line("session ended");
        if let Ok(r) = self.ring.lock()
            && let Some(f) = r.file.as_ref()
        {
            let _ = f.sync_data();
        }
        if let Ok(mut m) = self.marker.lock() {
            if let Some(f) = m.take() {
                let _ = f.unlock();
            }
            if !self.keep_marker.load(Ordering::Relaxed) {
                let _ = fs::remove_file(&self.marker_path);
            }
        }
    }

    /// Let the marker go as a dying process would (tests: a restart).
    pub fn abandon(&self) {
        if let Ok(mut m) = self.marker.lock()
            && let Some(f) = m.take()
        {
            let _ = f.unlock();
        }
    }
}

/// The panic hook: the first panic's crash bundle (`Session::crash`), the
/// others logged; then the hook that was there before.
pub fn install_panic_hook() {
    static ONCE: OnceLock<()> = OnceLock::new();
    if ONCE.set(()).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        static INSIDE: AtomicBool = AtomicBool::new(false);
        if let Some(s) = current()
            && !INSIDE.swap(true, Ordering::SeqCst)
        {
            let (what, backtrace) = panic_text(info);
            s.crash(&what, &backtrace);
            INSIDE.store(false, Ordering::SeqCst);
        }
        previous(info);
    }));
}

/// A panic's one-line text and its `backtrace.txt`.
pub fn panic_text(info: &std::panic::PanicHookInfo<'_>) -> (String, String) {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(not a string)".into());
    let place = info.location().map_or(String::new(), |l| format!(" at {}:{}", l.file(), l.line()));
    let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
    let what = format!("panic in thread '{thread}': {}{place}", cut(&message, 500));
    let backtrace = format!("{what}\n\n{}", std::backtrace::Backtrace::force_capture());
    (what, backtrace)
}

/// A minidump with home folders' names hidden: every `\Users\<name>`
/// and `/home/<name>` (and this machine's home), in UTF-8 and UTF-16, has
/// its name's characters replaced by `_`, the bytes' length kept (the
/// dump's offsets stay right).
pub fn scrub_dump(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let mut names: Vec<String> = Vec::new();
    if let Some(home) = dirs::home_dir()
        && let Some(n) = home.file_name()
    {
        names.push(n.to_string_lossy().into_owned());
    }
    // Names after a `Users\` or `home/` in the dump itself.
    for (prefix, wide) in [("\\Users\\", false), ("/home/", false), ("\\Users\\", true), ("/home/", true)] {
        let needle: Vec<u8> = if wide { prefix.encode_utf16().flat_map(u16::to_le_bytes).collect() } else { prefix.as_bytes().to_vec() };
        let step = if wide { 2 } else { 1 };
        let mut i = 0;
        while let Some(at) = find(&out[i..], &needle).map(|p| p + i) {
            let start = at + needle.len();
            let mut end = start;
            let mut name = String::new();
            while end + step <= out.len() {
                let c = if wide { u16::from_le_bytes([out[end], out[end + 1]]) as u32 } else { out[end] as u32 };
                let Some(ch) = char::from_u32(c).filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ')) else { break };
                name.push(ch);
                end += step;
            }
            let lower = name.to_ascii_lowercase();
            if !name.is_empty() && !matches!(lower.as_str(), "public" | "default" | "all users") {
                names.push(name);
            }
            i = start;
        }
    }
    names.sort();
    names.dedup();
    for name in names.iter().filter(|n| n.chars().count() >= 2) {
        let filler: String = "_".repeat(name.chars().count());
        for (from, to) in [
            (name.as_bytes().to_vec(), filler.as_bytes().to_vec()),
            (
                name.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>(),
                filler.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>(),
            ),
        ] {
            if from.len() != to.len() {
                continue;
            }
            let mut i = 0;
            while let Some(at) = find(&out[i..], &from).map(|p| p + i) {
                out[at..at + to.len()].copy_from_slice(&to);
                i = at + to.len();
            }
        }
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// A crash report's signature for grouping (the receiver's `inbox`): a
/// panic's top frames of our own code (`backtrace.txt`), a native crash's
/// exception, else what the text says it was.
pub fn signature(backtrace: Option<&str>, log: Option<&str>, text: &str) -> String {
    if let Some(bt) = backtrace.filter(|b| !b.trim().is_empty()) {
        let skip = |f: &str| {
            f.starts_with("std::")
                || f.starts_with("core::")
                || f.starts_with("alloc::")
                || f.starts_with("<alloc::")
                || f.starts_with("<core::")
                || f.starts_with("<std::")
                || f.starts_with("rust_begin_unwind")
                || f.starts_with("__rust")
                || f.starts_with("rust_panic")
                || f.contains("feedback::session")
                || f.starts_with("backtrace::")
                || f.starts_with("bevy_ecs::")
                || f.starts_with("<bevy_ecs::")
                || f.starts_with("<F as ")
                || f.starts_with("bevy_tasks::")
                || f == "main"
                || f.starts_with("__libc")
                || f.starts_with("_start")
                || f.starts_with("BaseThreadInitThunk")
                || f.starts_with("RtlUserThreadStart")
        };
        let frames: Vec<String> = bt
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                let (n, rest) = l.split_once(": ")?;
                n.parse::<u32>().ok()?;
                // `name::h0123456789abcdef` -> `name`.
                let name = match rest.rsplit_once("::h") {
                    Some((head, hash)) if hash.len() == 16 && hash.bytes().all(|c| c.is_ascii_hexdigit()) => head,
                    _ => rest,
                };
                (!skip(name)).then(|| name.to_string())
            })
            .take(3)
            .collect();
        let head = bt.lines().next().unwrap_or("");
        let place = head.rsplit_once(" at ").map(|(_, p)| p.to_string()).unwrap_or_default();
        if !frames.is_empty() {
            return format!("panic {}{}", frames.join(" < "), if place.is_empty() { String::new() } else { format!(" ({place})") });
        }
        if !place.is_empty() {
            return format!("panic at {place}");
        }
    }
    if let Some(code) = log
        .and_then(|l| l.lines().rev().find_map(|l| l.split_once("native crash: exception ").map(|(_, r)| r.to_string())))
    {
        return format!("native {}", code.split_whitespace().next().unwrap_or(""));
    }
    let first = text.lines().next().unwrap_or("").trim();
    if let Some(r) = first.strip_prefix("render error: ") {
        return format!("render {}", r.split([':', ' ']).next().unwrap_or(""));
    }
    if first.starts_with("The session ended unexpectedly") {
        return "unclean exit".into();
    }
    cut(first, 80).to_string()
}

/// A short key of a signature (12 hex digits of its SHA-256), for a group.
pub fn signature_key(signature: &str) -> String {
    use sha2::Digest;
    crate::store::hex(&sha2::Sha256::digest(signature.as_bytes()))[..12].to_string()
}

/// Native crashes on Windows: an unhandled-exception filter writing the
/// session's minidump (`Session::dump_path`) and a log line, then letting
/// Windows go on as it would (its error report, the process ending).
#[cfg(windows)]
pub mod native {
    use std::{os::windows::ffi::OsStrExt, sync::OnceLock};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{CREATE_ALWAYS, CreateFileW, FILE_ATTRIBUTE_NORMAL},
        System::{
            Diagnostics::Debug::{
                EXCEPTION_POINTERS, MINIDUMP_EXCEPTION_INFORMATION, MiniDumpNormal, MiniDumpWithThreadInfo,
                MiniDumpWithUnloadedModules, MiniDumpWriteDump, SetUnhandledExceptionFilter,
            },
            Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId},
        },
    };

    static DUMP: OnceLock<Vec<u16>> = OnceLock::new();

    /// The filter, for the current session's dump path.
    pub fn install(session: &super::Session) {
        let wide: Vec<u16> = session.dump_path().as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        if DUMP.set(wide).is_err() {
            return;
        }
        // SAFETY: the filter is a plain function living as long as the process.
        unsafe {
            SetUnhandledExceptionFilter(Some(filter));
        }
    }

    unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
        let (code, address) = unsafe {
            info.as_ref()
                .and_then(|p| p.ExceptionRecord.as_ref())
                .map_or((0u32, 0usize), |r| (r.ExceptionCode as u32, r.ExceptionAddress as usize))
        };
        let mut written = false;
        if let Some(path) = DUMP.get() {
            // SAFETY: Win32 calls with a NUL-terminated path and handles we own.
            unsafe {
                let file = CreateFileW(
                    path.as_ptr(),
                    GENERIC_WRITE,
                    0,
                    std::ptr::null(),
                    CREATE_ALWAYS,
                    FILE_ATTRIBUTE_NORMAL,
                    std::ptr::null_mut(),
                );
                if file != INVALID_HANDLE_VALUE {
                    let exception = MINIDUMP_EXCEPTION_INFORMATION {
                        ThreadId: GetCurrentThreadId(),
                        ExceptionPointers: info as *mut EXCEPTION_POINTERS,
                        ClientPointers: 0,
                    };
                    written = MiniDumpWriteDump(
                        GetCurrentProcess(),
                        GetCurrentProcessId(),
                        file,
                        MiniDumpNormal | MiniDumpWithThreadInfo | MiniDumpWithUnloadedModules,
                        &exception,
                        std::ptr::null(),
                        std::ptr::null(),
                    ) != 0;
                    CloseHandle(file);
                }
            }
        }
        if let Some(s) = super::current() {
            if let Ok(mut ring) = s.ring.try_lock() {
                s.push(
                    &mut ring,
                    &format!(
                        "native crash: exception 0x{code:08x} at 0x{address:x}{}",
                        if written { ", minidump written" } else { ", no minidump" }
                    ),
                    false,
                );
                if let Some(f) = ring.file.as_ref() {
                    let _ = f.sync_data();
                }
            }
            s.keep_marker.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // EXCEPTION_CONTINUE_SEARCH: Windows ends the process as it would.
        0
    }
}

/// No native-crash handler here (Linux: the marker and the log tell).
#[cfg(not(windows))]
pub mod native {
    pub fn install(_: &super::Session) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_read_back() {
        let m = MarkerInfo {
            app: "ultimate_gun_game".into(),
            build: "abc".into(),
            net_version: "net53".into(),
            log: "session-x.log".into(),
            dump: "session-x.dmp".into(),
            started: 5,
            pid: 7,
            crash: None,
        };
        let mut text = m.text();
        assert_eq!(MarkerInfo::parse(&text), m);
        text += "crash = ../../x\n";
        assert_eq!(MarkerInfo::parse(&text).crash, None);
        assert_eq!(plain_name("../x"), None);
        assert_eq!(plain_name("a.log"), Some("a.log"));
    }

    #[test]
    fn signatures() {
        let bt = "panic in thread 'main': boom at chimera/x.rs:3\n\n   0: std::backtrace::Backtrace::force_capture\n   1: chimera_assets::feedback::session::panic_text\n   2: core::panicking::panic_fmt::h0123456789abcdef\n   3: chimera_client::client::foo::bar::h0123456789abcdef\n   4: chimera_client::client::foo::baz\n   5: bevy_ecs::system::run\n   6: game::main\n";
        assert_eq!(
            signature(Some(bt), None, ""),
            "panic chimera_client::client::foo::bar < chimera_client::client::foo::baz < game::main (chimera/x.rs:3)"
        );
        assert_eq!(signature(None, Some("a\n12:00 native crash: exception 0xc0000005 at 0x1, minidump written\n"), "x"), "native 0xc0000005");
        assert_eq!(signature(None, None, "The session ended unexpectedly (no clean exit)\n..."), "unclean exit");
        assert_eq!(signature(None, None, "render error: DeviceLost: gone"), "render DeviceLost");
        assert_eq!(signature_key("x").len(), 12);
    }

    #[test]
    fn dumps_lose_home_names_and_keep_their_length() {
        let mut dump = b"MDMP....".to_vec();
        // Built in pieces: no literal home path in the source.
        dump.extend(concat!("C:\\", "Users", "\\Someone\\game\\ugg.exe").encode_utf16().flat_map(u16::to_le_bytes));
        dump.extend(b"..Someone..");
        let out = scrub_dump(&dump);
        assert_eq!(out.len(), dump.len());
        let text = String::from_utf8_lossy(&out).replace('\0', "");
        assert!(!text.contains("Someone"), "{text}");
        assert!(text.contains("Users\\_______\\game"), "{text}");
    }
}
