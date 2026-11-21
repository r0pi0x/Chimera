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
//! Where a report goes (`Targets`, `resolve`, first that applies). A hub
//! takes its accounts' tickets only (docs/plans/active/hub.md, 6), so a
//! signed-in player's reports go with the account's current ticket
//! (`Targets::account`, asked by the sender just before each upload) and
//! a token is never sent to the account's hub:
//! 1. the `chimera:feedback_url` cvars (`Targets::cvar`);
//! 2. the current game server's asset service (its offer: URL and
//!    certificate fingerprint) when it is the account's hub, with the
//!    ticket;
//! 3. that offered service with the tester's asset token when it is not
//!    the account's hub (a service with no hub), only when the token is
//!    bound to that same certificate or to none yet (a tester with their
//!    own install: trusted on first use, as fetching binds it)
//!    (`access`);
//! 4. the account's own hub (`Source::Account`), with the ticket;
//! 5. without an account: `[feedback] url`/`token`/`fingerprint` in the
//!    local config (`Targets::local`), the service the token is bound
//!    to, a developer's `[assets]` service (`Targets::assets`);
//! 6. none: the report waits in `outbox/` and the dialog says to sign in
//!    (`SIGN_IN`); it goes once a destination appears (signing in).
//!
//! A 401 (a token the service doesn't know, as a hub answers any token)
//! is not final: the report stays, says `SIGN_IN` (or, refused with the
//! account's ticket, `SIGN_IN_AGAIN`), and goes when the destination
//! changes (signing in) or at the next start. Reports an older build
//! marked failed with a 401 are tried again at start
//! (`clear_auth_failures`).
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
use crate::{access::Access, account::Session, remote::Remote, remote::RemoteConfig, store::write_atomic};

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
    /// The signed-in account (`account::user_session`): its hub takes the
    /// reports, with its current ticket.
    pub account: Option<Arc<Session>>,
}

/// Why a report waits when sending needs an account (none, or a service
/// that refused the token as unknown).
pub const SIGN_IN: &str = "sign in to send reports; kept on this PC, sent once you're signed in";

/// Why a report waits when the hub refused the account's ticket.
pub const SIGN_IN_AGAIN: &str =
    "the hub refused this account's sign-in: sign in again; kept on this PC, sent once you're signed in";

/// Which rule chose the destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Cvar,
    LocalConfig,
    /// The current server's asset service.
    Server,
    /// The service the asset token is bound to.
    BoundService,
    /// The signed-in account's hub.
    Account,
    /// A developer's `[assets]` service.
    AssetService,
}

/// Where reports go now.
#[derive(Clone, PartialEq, Eq)]
pub struct Destination {
    /// The service; for `account` its token is the ticket cached when
    /// resolved (the sender asks the session's current one).
    pub remote: RemoteConfig,
    pub source: Source,
    /// Sent with the account's ticket.
    pub account: bool,
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

/// The destination `targets` give (the module doc's order), if any.
pub fn resolve(targets: &Targets) -> Option<Destination> {
    let named = |t: &Target, source| Destination { remote: remote_of(t), source, account: false, label: t.url.clone() };
    if let Some(t) = &targets.cvar {
        return Some(named(t, Source::Cvar));
    }
    let file = targets.account.as_ref().map(|s| s.file());
    let ticket = || file.as_ref().and_then(|f| f.ticket.clone()).unwrap_or_default();
    let server_label = || targets.server_name.clone().unwrap_or_else(|| "this server".into());
    let offer = targets
        .offer
        .as_ref()
        .and_then(|(url, fingerprint)| Some((url.trim_end_matches('/').to_string(), crate::tls::normalize_fingerprint(fingerprint)?)));
    if let Some((url, fingerprint)) = &offer {
        if file.as_ref().is_some_and(|f| f.serves(url, Some(fingerprint))) {
            return Some(Destination {
                remote: RemoteConfig { url: url.clone(), token: ticket(), key: None, fingerprint: Some(fingerprint.clone()) },
                source: Source::Server,
                account: true,
                label: server_label(),
            });
        }
        // The offered service with the token when the token is bound to
        // its certificate, or bound to none yet (a tester who plays from
        // their own install and never fetched: trusted on first use, as
        // fetching binds it).
        if let Some(access) = &targets.access
            && let Some(token) = &access.token
            && access.fingerprint.as_ref().is_none_or(|bound| bound == fingerprint)
        {
            return Some(Destination {
                remote: RemoteConfig { url: url.clone(), token: token.clone(), key: None, fingerprint: Some(fingerprint.clone()) },
                source: Source::Server,
                account: false,
                label: server_label(),
            });
        }
    }
    if let Some(file) = &file {
        let mut remote = file.remote().unwrap_or_else(|| RemoteConfig {
            url: file.assets_url.clone().unwrap_or_else(|| file.url.clone()),
            token: String::new(),
            key: None,
            fingerprint: file.fingerprint.clone(),
        });
        remote.url = remote.url.trim_end_matches('/').to_string();
        return Some(Destination { label: remote.url.clone(), remote, source: Source::Account, account: true });
    }
    if let Some(t) = &targets.local {
        return Some(named(t, Source::LocalConfig));
    }
    if let Some(remote) = targets.access.as_ref().and_then(Access::remote) {
        return Some(Destination { label: remote.url.clone(), remote, source: Source::BoundService, account: false });
    }
    targets
        .assets
        .clone()
        .map(|remote| Destination { label: remote.url.clone(), remote, source: Source::AssetService, account: false })
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
        clear_auth_failures(&folder);
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
    /// Needs a (new) sign-in: kept, tried when the destination changes
    /// (signing in) or after the longest wait.
    Auth(String),
    Final(String),
}

/// Reports an older build marked refused for good with a 401 (a token
/// the service didn't know, as a hub answers any token since it takes
/// tickets only) are tried again: their `.failed` marks go.
fn clear_auth_failures(folder: &Path) {
    for (id, _, failed, _) in queued(folder) {
        let mark = outbox_dir(folder).join(format!("{id}.failed"));
        if failed && fs::read_to_string(&mark).is_ok_and(|r| r.starts_with("refused (HTTP 401)")) {
            let _ = fs::remove_file(mark);
        }
    }
}

fn upload(destination: &Destination, session: Option<&Session>, id: &str, bytes: &[u8]) -> Outcome {
    let mut config = destination.remote.clone();
    if destination.account {
        match session.map(Session::ticket) {
            Some(Ok(ticket)) => config.token = ticket,
            Some(Err(e)) => return Outcome::Later(format!("no ticket from your hub's accounts service yet ({e})"), None),
            None => return Outcome::Auth(SIGN_IN.into()),
        }
    }
    let remote = match Remote::new(config) {
        Ok(r) => r,
        Err(e) => return Outcome::Final(e),
    };
    match remote.post_raw("/feedback", "application/zip", &[("X-Chimera-Report-Id", id)], bytes) {
        Ok((200..=299, ..)) => Outcome::Sent,
        Ok((429, retry, body)) => Outcome::Later(
            format!("the service is limiting reports: {}", body.trim()),
            retry.and_then(|r| r.trim().parse::<u64>().ok()).map(Duration::from_secs),
        ),
        Ok((401, ..)) => Outcome::Auth(if destination.account { SIGN_IN_AGAIN } else { SIGN_IN }.into()),
        Ok((status @ 400..=499, _, body)) => Outcome::Final(format!("refused (HTTP {status}): {}", body.trim())),
        Ok((status, _, body)) => Outcome::Later(format!("HTTP {status}: {}", body.trim()), None),
        Err(e) => Outcome::Later(e.to_string(), None),
    }
}

fn sender(shared: &Shared) {
    loop {
        // What is due, and where it goes.
        let (due, destination, session, next_wake) = {
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
            (due, resolve(&s.targets), s.targets.account.clone(), next)
        };
        if let Some(destination) = destination.filter(|_| !due.is_empty()) {
            for id in due {
                let path = outbox_dir(&shared.folder).join(format!("{id}.zip"));
                let Ok(bytes) = fs::read(&path) else { continue };
                let outcome = upload(&destination, session.as_deref(), &id, &bytes);
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
                    Outcome::Later(..) | Outcome::Auth(_) => {
                        let (reason, after) = match outcome {
                            Outcome::Later(reason, after) => (reason, after),
                            Outcome::Auth(reason) => (reason, Some(shared.config.max_retry)),
                            _ => unreachable!(),
                        };
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
                    a.last_error = Some(SIGN_IN.into());
                    s.events.push(OutboxEvent::Queued { id, waiting, reason: SIGN_IN.into() });
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
