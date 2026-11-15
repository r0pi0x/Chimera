//! The tester's side of feedback reports (docs/plans/active/feedback-reports.md,
//! 2.4 and 2.6): a queue on disk and a thread that sends it.
//!
//! `Feedback::start(<data_local>/chimera/feedback, targets, config)` opens
//! the folder (`outbox/`, `sent/`) and starts the sender thread (never the
//! game's). `submit(ReportState)` builds the bundle (`bundle::build`),
//! writes `outbox/<id>.zip` atomically and wakes the thread; it returns at
//! once. The thread sends what waits at once, then after a failure again
//! with backoff (`OutboxConfig`: 1, 2, 4 ... 15 minutes) while the game
//! runs, and everything left at the next start. A 2xx moves the zip to
//! `sent/` (the last `keep_sent` kept); a 4xx other than 429 is final: the
//! zip stays in `outbox/` with `<id>.failed` holding the reason, and is
//! not tried again. 429 waits for its `Retry-After`.
//!
//! Where a report goes (`Targets`, `resolve`, first that applies):
//! 1. the `chimera:feedback_url` cvars (`Targets::cvar`);
//! 2. `[feedback] url`/`token`/`fingerprint` in the local config
//!    (`Targets::local`);
//! 3. the current game server's asset service (its offer: URL and
//!    certificate fingerprint) with the tester's asset token, only when
//!    the token is bound to that same certificate or to none yet (a
//!    tester with their own install: trusted on first use, as fetching
//!    binds it) (`access`); else the
//!    service the token is bound to; else a developer's `[assets]`
//!    service (`Targets::assets`);
//! 4. none: the report waits in `outbox/` (the dialog says it is saved
//!    on this PC) and goes when a destination appears.
//!
//! The game polls `status()` (what waits, the destination) and drains
//! `events()` (sent, queued, failed) for its toasts; `set_targets` tells
//! the thread about a new server, cvar or token.
//!
//! Upload: `POST <url>/feedback`, `Authorization: Bearer <token>`,
//! `Content-Type: application/zip`, `X-Chimera-Report-Id`, over HTTPS with
//! the pinned certificate (`Remote`).
//!
//! Known shortcuts: the backoff state (attempts, next try) is in memory
//! only: a restart tries everything at once; a destination change doesn't
//! retry a `failed` report (`retry_failed` does).

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use super::{ReportState, bundle};
use crate::{access::Access, remote::Remote, remote::RemoteConfig, store::write_atomic};

/// A report service named by hand (cvars or the local config).
#[derive(Clone, PartialEq, Eq)]
pub struct Target {
    pub url: String,
    pub token: String,
    pub fingerprint: Option<String>,
}

impl std::fmt::Debug for Target {
    // Never the token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Target").field("url", &self.url).field("fingerprint", &self.fingerprint).finish()
    }
}

/// What the game knows about where reports may go.
#[derive(Clone, Debug, Default)]
pub struct Targets {
    /// `chimera:feedback_url`, `_token`, `_fingerprint` (all set).
    pub cvar: Option<Target>,
    /// `[feedback]` in the local config (`LocalFeedback::target`).
    pub local: Option<Target>,
    /// The current game server's asset offer: URL and fingerprint.
    pub offer: Option<(String, String)>,
    /// The current server's name, for the dialog ("to <name>'s report
    /// service").
    pub server_name: Option<String>,
    /// The tester's token and the service it is bound to
    /// (`asset-access.toml`, `Access::load`).
    pub access: Option<Access>,
    /// A developer's `[assets]` service (`AssetSettings::remote`).
    pub assets: Option<RemoteConfig>,
}

/// Which rule chose the destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Cvar,
    LocalConfig,
    /// The current server's asset service.
    Server,
    /// The service the asset token is bound to.
    BoundService,
    /// A developer's `[assets]` service.
    AssetService,
}

/// Where reports go now.
#[derive(Clone, PartialEq, Eq)]
pub struct Destination {
    pub remote: RemoteConfig,
    pub source: Source,
    /// For people: the server's name, else the service's URL.
    pub label: String,
}

impl std::fmt::Debug for Destination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Destination").field("source", &self.source).field("label", &self.label).finish()
    }
}

fn remote_of(t: &Target) -> RemoteConfig {
    RemoteConfig { url: t.url.clone(), token: t.token.clone(), key: None, fingerprint: t.fingerprint.clone() }
}

/// The destination `targets` give (section 2.6's order), if any.
pub fn resolve(targets: &Targets) -> Option<Destination> {
    let named = |t: &Target, source| Destination { remote: remote_of(t), source, label: t.url.clone() };
    if let Some(t) = &targets.cvar {
        return Some(named(t, Source::Cvar));
    }
    if let Some(t) = &targets.local {
        return Some(named(t, Source::LocalConfig));
    }
    if let Some(access) = &targets.access
        && let Some(token) = &access.token
    {
        // The offered service when the token is bound to its certificate,
        // or bound to none yet (a tester who plays from their own install
        // and never fetched: trusted on first use, as fetching binds it).
        let offered = targets.offer.as_ref().and_then(|(url, fingerprint)| {
            let offered = crate::tls::normalize_fingerprint(fingerprint)?;
            match &access.fingerprint {
                Some(bound) => (*bound == offered).then_some((url, offered)),
                None => Some((url, offered)),
            }
        });
        if let Some((url, bound)) = offered {
            return Some(Destination {
                remote: RemoteConfig {
                    url: url.trim_end_matches('/').to_string(),
                    token: token.clone(),
                    key: None,
                    fingerprint: Some(bound),
                },
                source: Source::Server,
                label: targets.server_name.clone().unwrap_or_else(|| "this server".into()),
            });
        }
        if let Some(remote) = access.remote() {
            return Some(Destination { label: remote.url.clone(), remote, source: Source::BoundService });
        }
    }
    targets
        .assets
        .clone()
        .map(|remote| Destination { label: remote.url.clone(), remote, source: Source::AssetService })
}

/// The sender's timing.
#[derive(Clone, Copy, Debug)]
pub struct OutboxConfig {
    /// The wait after the first failure; doubled after each.
    pub first_retry: Duration,
    pub max_retry: Duration,
    /// Sent zips kept in `sent/`.
    pub keep_sent: usize,
}

impl Default for OutboxConfig {
    fn default() -> Self {
        Self { first_retry: Duration::from_secs(60), max_retry: Duration::from_secs(15 * 60), keep_sent: 20 }
    }
}

/// `submit`'s answer.
#[derive(Clone, Debug)]
pub struct Submitted {
    pub id: String,
    /// The queued zip.
    pub path: PathBuf,
    pub bytes: usize,
    /// Where it will go (None: nowhere yet, saved on this PC).
    pub destination: Option<Destination>,
    /// The screenshot was left out (too large).
    pub screenshot_dropped: bool,
}

/// What the sender did, for the game's toasts and console.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboxEvent {
    /// The service took it (201 stored, 200 already had it, 202).
    Sent { id: String },
    /// Not sent yet; tried again later. `waiting`: reports in the outbox.
    Queued { id: String, waiting: usize, reason: String },
    /// Refused for good (a 4xx other than 429): stays in the outbox.
    Failed { id: String, reason: String },
}

/// One report in the outbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
    pub id: String,
    pub bytes: u64,
    pub attempts: u32,
    /// Seconds until the next try (0: due).
    pub next_try_secs: u64,
    pub last_error: Option<String>,
    /// Refused for good (not tried again).
    pub failed: bool,
}

/// The outbox now (`feedback_status`).
#[derive(Clone, Debug, Default)]
pub struct OutboxStatus {
    pub waiting: Vec<Waiting>,
    pub sent_this_session: usize,
    pub destination: Option<Destination>,
    /// The feedback folder.
    pub folder: PathBuf,
}

impl OutboxStatus {
    /// Reports that will still be sent (not failed).
    pub fn pending(&self) -> usize {
        self.waiting.iter().filter(|w| !w.failed).count()
    }
}

#[derive(Default)]
struct Attempt {
    count: u32,
    next: Option<Instant>,
    last_error: Option<String>,
}

#[derive(Default)]
struct State {
    targets: Targets,
    attempts: HashMap<String, Attempt>,
    events: Vec<OutboxEvent>,
    sent: usize,
    wake: bool,
    stop: bool,
}

struct Shared {
    folder: PathBuf,
    config: OutboxConfig,
    state: Mutex<State>,
    signal: Condvar,
}

/// The outbox and its sender thread; the thread ends when this is
/// dropped (an upload under way finishes first, in the background).
pub struct Feedback {
    shared: Arc<Shared>,
}

impl Drop for Feedback {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap_or_else(|e| e.into_inner()).stop = true;
        self.shared.signal.notify_all();
    }
}

fn outbox_dir(folder: &Path) -> PathBuf {
    folder.join("outbox")
}

fn sent_dir(folder: &Path) -> PathBuf {
    folder.join("sent")
}

/// The ids in `outbox/`, oldest first.
fn queued(folder: &Path) -> Vec<(String, u64, bool, Option<std::time::SystemTime>)> {
    let dir = outbox_dir(folder);
    let mut out: Vec<_> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".zip")?.to_string();
            if !super::is_report_id(&id) {
                return None;
            }
            let meta = e.metadata().ok();
            let failed = dir.join(format!("{id}.failed")).is_file();
            Some((id, meta.as_ref().map_or(0, |m| m.len()), failed, meta.and_then(|m| m.modified().ok())))
        })
        .collect();
    out.sort_by_key(|(id, _, _, t)| (*t, id.clone()));
    out
}

impl Feedback {
    /// Open `folder` (`<data_local>/chimera/feedback`) and start sending
    /// what waits there to `targets`' destination.
    pub fn start(folder: PathBuf, targets: Targets, config: OutboxConfig) -> std::io::Result<Self> {
        fs::create_dir_all(outbox_dir(&folder))?;
        fs::create_dir_all(sent_dir(&folder))?;
        let shared = Arc::new(Shared {
            folder,
            config,
            state: Mutex::new(State { targets, ..State::default() }),
            signal: Condvar::new(),
        });
        let thread_shared = shared.clone();
        std::thread::Builder::new().name("feedback-outbox".into()).spawn(move || sender(&thread_shared))?;
        Ok(Self { shared })
    }

    /// The feedback folder.
    pub fn folder(&self) -> &Path {
        &self.shared.folder
    }

    /// Build `state`'s bundle, queue it and wake the sender. Returns at
    /// once; the outcome arrives as an `OutboxEvent`.
    pub fn submit(&self, state: &ReportState) -> Result<Submitted, String> {
        let bundle = bundle::build(state)?;
        let path = outbox_dir(&self.shared.folder).join(format!("{}.zip", bundle.id));
        write_atomic(&path, &bundle.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut s = self.lock();
        s.wake = true;
        let destination = resolve(&s.targets);
        drop(s);
        self.shared.signal.notify_all();
        Ok(Submitted {
            id: bundle.id,
            path,
            bytes: bundle.bytes.len(),
            destination,
            screenshot_dropped: bundle.screenshot_dropped,
        })
    }

    /// Change what the sender knows of destinations (a server joined, a
    /// cvar set, a token entered); what waits is tried again at once.
    pub fn set_targets(&self, change: impl FnOnce(&mut Targets)) {
        let mut s = self.lock();
        let before = resolve(&s.targets);
        change(&mut s.targets);
        if resolve(&s.targets) != before {
            for a in s.attempts.values_mut() {
                a.next = None;
            }
            s.wake = true;
        }
        drop(s);
        self.shared.signal.notify_all();
    }

    /// The destination now.
    pub fn destination(&self) -> Option<Destination> {
        resolve(&self.lock().targets)
    }

    /// Try what waits now (not failed reports).
    pub fn retry_now(&self) {
        let mut s = self.lock();
        for a in s.attempts.values_mut() {
            a.next = None;
        }
        s.wake = true;
        drop(s);
        self.shared.signal.notify_all();
    }

    /// Forget every report's final refusal and try them again.
    pub fn retry_failed(&self) {
        for (id, ..) in queued(&self.shared.folder) {
            let _ = fs::remove_file(outbox_dir(&self.shared.folder).join(format!("{id}.failed")));
        }
        self.retry_now();
    }

    /// The outbox now.
    pub fn status(&self) -> OutboxStatus {
        let s = self.lock();
        let now = Instant::now();
        let waiting = queued(&self.shared.folder)
            .into_iter()
            .map(|(id, bytes, failed, _)| {
                let a = s.attempts.get(&id);
                let reason = failed
                    .then(|| fs::read_to_string(outbox_dir(&self.shared.folder).join(format!("{id}.failed"))).ok())
                    .flatten();
                Waiting {
                    bytes,
                    attempts: a.map_or(0, |a| a.count),
                    next_try_secs: a.and_then(|a| a.next).map_or(0, |t| t.saturating_duration_since(now).as_secs()),
                    last_error: reason.or_else(|| a.and_then(|a| a.last_error.clone())),
                    failed,
                    id,
                }
            })
            .collect();
        OutboxStatus {
            waiting,
            sent_this_session: s.sent,
            destination: resolve(&s.targets),
            folder: self.shared.folder.clone(),
        }
    }

    /// What happened since the last call.
    pub fn events(&self) -> Vec<OutboxEvent> {
        std::mem::take(&mut self.lock().events)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.shared.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// How one upload went.
enum Outcome {
    Sent,
    Later(String, Option<Duration>),
    Final(String),
}

fn upload(destination: &Destination, id: &str, bytes: &[u8]) -> Outcome {
    let remote = match Remote::new(destination.remote.clone()) {
        Ok(r) => r,
        Err(e) => return Outcome::Final(e),
    };
    match remote.post_raw("/feedback", "application/zip", &[("X-Chimera-Report-Id", id)], bytes) {
        Ok((200..=299, ..)) => Outcome::Sent,
        Ok((429, retry, body)) => Outcome::Later(
            format!("the service is limiting reports: {}", body.trim()),
            retry.and_then(|r| r.trim().parse::<u64>().ok()).map(Duration::from_secs),
        ),
        Ok((status @ 400..=499, _, body)) => Outcome::Final(format!("refused (HTTP {status}): {}", body.trim())),
        Ok((status, _, body)) => Outcome::Later(format!("HTTP {status}: {}", body.trim()), None),
        Err(e) => Outcome::Later(e.to_string(), None),
    }
}

fn sender(shared: &Shared) {
    loop {
        // What is due, and where it goes.
        let (due, destination, next_wake) = {
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            if s.stop {
                return;
            }
            s.wake = false;
            let now = Instant::now();
            let ids = queued(&shared.folder);
            s.attempts.retain(|id, _| ids.iter().any(|(i, ..)| i == id));
            let mut due = Vec::new();
            let mut next: Option<Instant> = None;
            for (id, _, failed, _) in &ids {
                if *failed {
                    continue;
                }
                match s.attempts.get(id).and_then(|a| a.next) {
                    Some(t) if t > now => next = Some(next.map_or(t, |n| n.min(t))),
                    _ => due.push(id.clone()),
                }
            }
            (due, resolve(&s.targets), next)
        };
        if let Some(destination) = destination.filter(|_| !due.is_empty()) {
            for id in due {
                let path = outbox_dir(&shared.folder).join(format!("{id}.zip"));
                let Ok(bytes) = fs::read(&path) else { continue };
                let outcome = upload(&destination, &id, &bytes);
                let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                match outcome {
                    Outcome::Sent => {
                        let to = sent_dir(&shared.folder).join(format!("{id}.zip"));
                        if fs::rename(&path, &to).is_err() {
                            let _ = fs::remove_file(&path);
                        }
                        s.attempts.remove(&id);
                        s.sent += 1;
                        s.events.push(OutboxEvent::Sent { id });
                        prune_sent(&shared.folder, shared.config.keep_sent);
                    }
                    Outcome::Final(reason) => {
                        let _ = write_atomic(&outbox_dir(&shared.folder).join(format!("{id}.failed")), reason.as_bytes());
                        s.attempts.remove(&id);
                        s.events.push(OutboxEvent::Failed { id, reason });
                    }
                    Outcome::Later(reason, after) => {
                        let a = s.attempts.entry(id.clone()).or_default();
                        a.count += 1;
                        let backoff = shared
                            .config
                            .first_retry
                            .saturating_mul(1u32 << (a.count - 1).min(16))
                            .min(shared.config.max_retry);
                        a.next = Some(Instant::now() + after.unwrap_or(backoff).max(Duration::from_millis(10)));
                        a.last_error = Some(reason.clone());
                        let waiting = queued(&shared.folder).iter().filter(|q| !q.2).count();
                        s.events.push(OutboxEvent::Queued { id, waiting, reason });
                    }
                }
                if s.stop {
                    return;
                }
            }
            continue;
        } else if !due.is_empty() {
            // Nowhere to send: say so once per report.
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            let waiting = due.len();
            for id in due {
                let a = s.attempts.entry(id.clone()).or_default();
                if a.last_error.is_none() {
                    a.last_error = Some("no report service: saved on this PC".into());
                    s.events.push(OutboxEvent::Queued {
                        id,
                        waiting,
                        reason: "no report service: saved on this PC".into(),
                    });
                }
            }
        }
        let s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.stop || s.wake {
            continue;
        }
        // Without a destination nothing is due until `set_targets` wakes us.
        let wait = next_wake.map_or(Duration::from_secs(3600), |t| t.saturating_duration_since(Instant::now()));
        let _ = shared.signal.wait_timeout(s, wait.max(Duration::from_millis(5)));
    }
}

/// Keep the newest `keep` zips in `sent/`.
fn prune_sent(folder: &Path, keep: usize) {
    let mut sent: Vec<_> = fs::read_dir(sent_dir(folder))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".zip"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).ok(), e.path()))
        .collect();
    sent.sort();
    let extra = sent.len().saturating_sub(keep);
    for (_, path) in sent.into_iter().take(extra) {
        let _ = fs::remove_file(path);
    }
}
