//! The receiver of feedback reports (docs/plans/active/feedback-reports.md,
//! 3.3 and 3.4), behind the asset service's `POST /feedback` (`server`):
//! the tester's asset token says who sent it.
//!
//! - **Answers:** 201 stored, 200 already stored (the same report id: a
//!   client's retry), 202 taken (a muted account: it isn't stored, and the
//!   sender doesn't learn it), 400 not a valid bundle (`bundle::open`: a
//!   zip of allow-listed entries under their caps, `report.json` with the
//!   id the `X-Chimera-Report-Id` header names), 413 over
//!   `FeedbackLimits::max_body`, 429 past a limit with `Retry-After`.
//! - **Limits** (`FeedbackLimits`): per account 10 an hour and 40 a day,
//!   per address 30 an hour; stored and refused-as-invalid reports count.
//! - **Storage:** `<server store>/server/feedback/<yyyy-mm-dd>/<id>/`: the
//!   zip's entries unpacked (the zip itself isn't kept: one form only),
//!   `meta.toml` (`Meta`: account,
//!   address, time, size, build, app, kind, map, a summary line, status,
//!   group, reason, fixed_in; a crash's `signature` and `signature_key`,
//!   `session::signature`, for grouping: `crash_groups`) and
//!   `history.log` (one line per change:
//!   time, who, from, to, why). A report is moved into place whole (a
//!   rename), so a half-written one is never listed.
//! - **Statuses** (`Status`): `new`, `triaged`, `accepted`, `rejected`,
//!   `needs-info`, `duplicate` (with a group), `fixed` (with the commit);
//!   `set_status` is what the CLI and the page call.
//! - **Retention** (`FeedbackLimits::keep_days`, `keep_bytes`): after each
//!   stored report, reports older than `keep_days` go, then the oldest
//!   until all of them together take at most `keep_bytes` (`prune`; the
//!   new one always stays). A `report.zip` an older receiver kept beside
//!   the unpacked entries goes too.
//! - **Mute** (`muted.toml`): a muted account's reports get 202 and are
//!   dropped.
//! - **on_report** (`--on-report`, `[feedback] on_report`): a command of
//!   the host's run for each stored report through the shell, with
//!   `CHIMERA_REPORT_DIR`, `_ID`, `_KIND`, `_ACCOUNT` and `_SUMMARY` (the
//!   tester's first line, untrusted: `@`, backticks and control characters
//!   taken out, at most 100 characters) in its environment. A Discord ping
//!   lives there, its ids in the host's config.
//!
//! Report text is untrusted data: shown, never run or followed.
//!
//! Known shortcuts: the limit windows are in memory (a restart forgets
//! them); `list` reads every report's `meta.toml` each time; retention
//! walks every report after each one stored.

use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use super::{MAX_BUNDLE, bundle, is_report_id, now_secs, utc_day, utc_text};
use crate::store::write_atomic;

/// What one account or address may send.
#[derive(Clone, Copy, Debug)]
pub struct FeedbackLimits {
    /// The most bytes a report may have (413 past it).
    pub max_body: usize,
    pub per_account_hour: usize,
    pub per_account_day: usize,
    pub per_address_hour: usize,
    /// Reports older than this many days are deleted (0: kept).
    pub keep_days: u64,
    /// The most bytes all stored reports may take; the oldest go first
    /// (0: no cap).
    pub keep_bytes: u64,
}

impl Default for FeedbackLimits {
    fn default() -> Self {
        Self {
            max_body: MAX_BUNDLE,
            per_account_hour: 10,
            per_account_day: 40,
            per_address_hour: 30,
            keep_days: 60,
            keep_bytes: 2 << 30,
        }
    }
}

/// Bytes of the files in `dir` (not below).
fn dir_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()
}

/// A report's triage status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    #[default]
    New,
    Triaged,
    Accepted,
    Rejected,
    NeedsInfo,
    Duplicate,
    Fixed,
}

impl Status {
    pub const ALL: [Status; 7] = [
        Status::New,
        Status::Triaged,
        Status::Accepted,
        Status::Rejected,
        Status::NeedsInfo,
        Status::Duplicate,
        Status::Fixed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::New => "new",
            Status::Triaged => "triaged",
            Status::Accepted => "accepted",
            Status::Rejected => "rejected",
            Status::NeedsInfo => "needs-info",
            Status::Duplicate => "duplicate",
            Status::Fixed => "fixed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s.trim())
    }
}

/// `meta.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    /// `kind:name#token` of the sender's asset token.
    pub account: String,
    #[serde(default)]
    pub address: String,
    /// Unix seconds.
    pub received: u64,
    #[serde(default)]
    pub received_utc: String,
    pub bytes: u64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub map: String,
    /// The text's first line (at most 120 characters).
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub install_id: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub fixed_in: String,
    /// The day folder (`yyyy-mm-dd`).
    #[serde(default)]
    pub day: String,
    /// A crash's signature (`session::signature`: a panic's top frames, a
    /// native crash's exception, "unclean exit") and its short key: crash
    /// reports with one key are one problem.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_key: String,
}

/// An answer to an upload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: String,
    /// Seconds (429).
    pub retry_after: Option<u64>,
}

impl Answer {
    fn new(status: u16, body: impl Into<String>) -> Self {
        Self { status, body: body.into(), retry_after: None }
    }
}

/// A status change.
#[derive(Clone, Debug, Default)]
pub struct Change {
    pub status: Option<Status>,
    pub group: Option<String>,
    pub reason: Option<String>,
    pub fixed_in: Option<String>,
}

#[derive(Default)]
struct Windows {
    accounts: HashMap<String, VecDeque<u64>>,
    addresses: HashMap<IpAddr, VecDeque<u64>>,
}

/// The reports folder and its limits.
pub struct Inbox {
    root: PathBuf,
    limits: FeedbackLimits,
    on_report: Option<String>,
    windows: Mutex<Windows>,
    /// One status change at a time.
    edit: Mutex<()>,
}

#[derive(Default, Serialize, Deserialize)]
struct Muted {
    #[serde(default)]
    accounts: Vec<String>,
}

/// One line, printable, at most `max` characters.
fn clean(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// The account part of a label (`kind:name#token` -> `kind:name`).
fn account_of(label: &str) -> &str {
    label.split('#').next().unwrap_or(label)
}

impl Inbox {
    /// The reports in `root` (`<server store>/server/feedback`).
    pub fn open(root: PathBuf) -> Self {
        Self { root, limits: FeedbackLimits::default(), on_report: None, windows: Mutex::default(), edit: Mutex::default() }
    }

    pub fn with_limits(mut self, limits: FeedbackLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Run `command` for each stored report (see the module's doc).
    pub fn with_on_report(mut self, command: Option<String>) -> Self {
        self.on_report = command.filter(|c| !c.trim().is_empty());
        self
    }

    pub fn limits(&self) -> FeedbackLimits {
        self.limits
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder of report `id`, if stored.
    pub fn find(&self, id: &str) -> Option<PathBuf> {
        if !is_report_id(id) {
            return None;
        }
        fs::read_dir(&self.root)
            .ok()?
            .flatten()
            .map(|day| day.path().join(id))
            .find(|p| p.join("meta.toml").is_file())
    }

    /// Every report, newest first.
    pub fn list(&self) -> Vec<Meta> {
        let mut out: Vec<Meta> = fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|d| d.path().is_dir())
            .flat_map(|day| fs::read_dir(day.path()).into_iter().flatten().flatten())
            .filter_map(|r| toml::from_str(&fs::read_to_string(r.path().join("meta.toml")).ok()?).ok())
            .collect();
        out.sort_by(|a: &Meta, b: &Meta| b.received.cmp(&a.received).then(a.id.cmp(&b.id)));
        out
    }

    /// Crash reports by signature: (key, signature, reports newest
    /// first), the group with the most reports first.
    pub fn crash_groups(&self) -> Vec<(String, String, Vec<Meta>)> {
        let mut groups: Vec<(String, String, Vec<Meta>)> = Vec::new();
        for m in self.list().into_iter().filter(|m| m.kind == "crash") {
            match groups.iter_mut().find(|g| g.0 == m.signature_key) {
                Some(g) => g.2.push(m),
                None => groups.push((m.signature_key.clone(), m.signature.clone(), vec![m])),
            }
        }
        groups.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then(b.2[0].received.cmp(&a.2[0].received)));
        groups
    }

    pub fn meta(&self, id: &str) -> Option<Meta> {
        toml::from_str(&fs::read_to_string(self.find(id)?.join("meta.toml")).ok()?).ok()
    }

    /// Report `id`'s `history.log`.
    pub fn history(&self, id: &str) -> String {
        self.find(id).and_then(|d| fs::read_to_string(d.join("history.log")).ok()).unwrap_or_default()
    }

    fn muted_path(&self) -> PathBuf {
        self.root.join("muted.toml")
    }

    /// Muted accounts (`kind:name`).
    pub fn muted(&self) -> Vec<String> {
        fs::read_to_string(self.muted_path())
            .ok()
            .and_then(|t| toml::from_str::<Muted>(&t).ok())
            .map(|m| m.accounts)
            .unwrap_or_default()
    }

    /// Mute (`true`) or unmute an account (`name` or `kind:name`).
    pub fn set_muted(&self, account: &str, mute: bool) -> Result<(), String> {
        let account = account.trim();
        let full = if account.contains(':') { account.to_string() } else { format!("internal:{account}") };
        if !crate::accounts::is_account_name(full.split_once(':').map_or("", |(_, n)| n)) {
            return Err(format!("{account:?} isn't an account name"));
        }
        let mut list = self.muted();
        list.retain(|a| *a != full);
        if mute {
            list.push(full);
        }
        let text = toml::to_string(&Muted { accounts: list }).map_err(|e| e.to_string())?;
        write_atomic(&self.muted_path(), text.as_bytes()).map_err(|e| e.to_string())
    }

    /// Count one report from `account` and `ip` if within the limits; the
    /// seconds to wait if not.
    fn admit(&self, account: &str, ip: Option<IpAddr>, now: u64) -> Result<(), u64> {
        let mut w = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        let wait = |q: &VecDeque<u64>, window: u64, max: usize| -> Option<u64> {
            let inside: Vec<u64> = q.iter().copied().filter(|t| *t + window > now).collect();
            (max > 0 && inside.len() >= max).then(|| inside[inside.len() - max] + window - now)
        };
        let acc = w.accounts.entry(account.to_string()).or_default();
        acc.retain(|t| *t + 86_400 > now);
        let mut retry = wait(acc, 3_600, self.limits.per_account_hour)
            .into_iter()
            .chain(wait(acc, 86_400, self.limits.per_account_day))
            .max();
        if let Some(ip) = ip {
            let q = w.addresses.entry(ip).or_default();
            q.retain(|t| *t + 3_600 > now);
            retry = retry.into_iter().chain(wait(q, 3_600, self.limits.per_address_hour)).max();
        }
        if let Some(r) = retry {
            return Err(r.max(1));
        }
        w.accounts.entry(account.to_string()).or_default().push_back(now);
        if let Some(ip) = ip {
            w.addresses.entry(ip).or_default().push_back(now);
        }
        Ok(())
    }

    /// Take an upload from `account` (`kind:name#token`) at `ip`.
    pub fn receive(&self, account: &str, ip: Option<IpAddr>, header_id: Option<&str>, body: &[u8]) -> Answer {
        if body.len() > self.limits.max_body {
            return Answer::new(413, format!("a report is at most {} bytes\n", self.limits.max_body));
        }
        let Some(id) = header_id.map(str::trim).filter(|i| is_report_id(i)) else {
            return Answer::new(400, "X-Chimera-Report-Id: a report id (a lowercase UUID) is needed\n");
        };
        if self.find(id).is_some() {
            return Answer::new(200, "already stored\n");
        }
        if self.muted().iter().any(|m| m == account_of(account)) {
            return Answer::new(202, "thanks\n");
        }
        let now = now_secs();
        if let Err(wait) = self.admit(account_of(account), ip, now) {
            return Answer {
                status: 429,
                body: "too many reports from this account or address: try again later\n".into(),
                retry_after: Some(wait),
            };
        }
        let opened = match bundle::open(body) {
            Ok(o) if o.info.id == id => o,
            Ok(_) => return Answer::new(400, "report.json's id isn't the header's\n"),
            Err(e) => return Answer::new(400, format!("not a report bundle: {e}\n")),
        };
        let text = |name: &str| {
            opened.entries.iter().find(|e| e.name == name).map(|e| String::from_utf8_lossy(&e.data).into_owned())
        };
        let map = text("game.txt")
            .and_then(|g| g.lines().find_map(|l| l.strip_prefix("map:").map(|m| clean(m, 100))))
            .unwrap_or_default();
        let info = &opened.info;
        let day = utc_day(now);
        let signature = if info.kind == bundle::Kind::Crash {
            clean(&super::session::signature(text("backtrace.txt").as_deref(), text("log.txt").as_deref(), &info.text), 300)
        } else {
            String::new()
        };
        let signature_key = if signature.is_empty() { String::new() } else { super::session::signature_key(&signature) };
        let meta = Meta {
            id: id.to_string(),
            account: clean(account, 100),
            address: ip.map_or_else(String::new, |i| i.to_string()),
            received: now,
            received_utc: utc_text(now),
            bytes: body.len() as u64,
            kind: info.kind.as_str().into(),
            build: clean(&info.build, 200),
            app: clean(&info.app, 100),
            map,
            summary: clean(info.text.lines().find(|l| !l.trim().is_empty()).unwrap_or(""), 120),
            install_id: clean(&info.install_id, 64),
            status: Status::New,
            day: day.clone(),
            signature,
            signature_key,
            ..Meta::default()
        };
        match self.store(&day, &meta, &opened) {
            Ok(dir) => {
                let pruned = self.prune(now, id);
                if pruned > 0 {
                    eprintln!("feedback: {pruned} old reports deleted (retention)");
                }
                self.ping(&dir, &meta);
                Answer::new(201, format!("stored {id}\n"))
            }
            Err(e) if self.find(id).is_some() => {
                let _ = e;
                Answer::new(200, "already stored\n")
            }
            Err(e) => {
                eprintln!("feedback: can't store {id}: {e}");
                Answer::new(500, "the report couldn't be stored\n")
            }
        }
    }

    fn store(&self, day: &str, meta: &Meta, opened: &bundle::Opened) -> std::io::Result<PathBuf> {
        let day_dir = self.root.join(day);
        let tmp = day_dir.join(format!(".{}.{}.tmp", meta.id, std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;
        let result = (|| {
            for e in &opened.entries {
                // Names are allow-listed (`bundle::ENTRIES`): flat, no `..`.
                fs::write(tmp.join(&e.name), &e.data)?;
            }
            fs::write(tmp.join("meta.toml"), toml::to_string(meta).map_err(std::io::Error::other)?)?;
            let mut history = format!("{} received from {} ({})\n", meta.received_utc, meta.account, meta.kind);
            if !opened.dropped.is_empty() {
                let names: Vec<String> = opened.dropped.iter().map(|n| clean(n, 100)).collect();
                history.push_str(&format!("{} dropped unknown entries: {}\n", meta.received_utc, names.join(", ")));
            }
            fs::write(tmp.join("history.log"), history)?;
            let dir = day_dir.join(&meta.id);
            fs::rename(&tmp, &dir)?;
            Ok(dir)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&tmp);
        }
        result
    }

    /// Retention (`FeedbackLimits::keep_days`, `keep_bytes`): delete
    /// reports past the age, then the oldest until the rest fit, never
    /// `keep` (the one just stored); old receivers' `report.zip`s go
    /// too. Returns how many reports were deleted.
    pub fn prune(&self, now: u64, keep: &str) -> usize {
        let mut reports: Vec<(u64, String, PathBuf, u64)> = Vec::new();
        for day in fs::read_dir(&self.root).into_iter().flatten().flatten().filter(|d| d.path().is_dir()) {
            for r in fs::read_dir(day.path()).into_iter().flatten().flatten() {
                let dir = r.path();
                let Some(meta) = fs::read_to_string(dir.join("meta.toml")).ok().and_then(|t| toml::from_str::<Meta>(&t).ok())
                else {
                    continue;
                };
                let _ = fs::remove_file(dir.join("report.zip"));
                reports.push((meta.received, meta.id, dir.clone(), dir_bytes(&dir)));
            }
        }
        reports.sort();
        let mut total: u64 = reports.iter().map(|r| r.3).sum();
        let mut deleted = 0;
        for (received, id, dir, bytes) in reports {
            let old = self.limits.keep_days > 0 && received + self.limits.keep_days * 86_400 < now;
            let over = self.limits.keep_bytes > 0 && total > self.limits.keep_bytes;
            if id == keep || !(old || over) {
                continue;
            }
            if fs::remove_dir_all(&dir).is_ok() {
                total = total.saturating_sub(bytes);
                deleted += 1;
                if let Some(day) = dir.parent() {
                    let _ = fs::remove_dir(day); // Only when empty.
                }
            }
        }
        deleted
    }

    /// Run `on_report` for a stored report (in the background).
    fn ping(&self, dir: &Path, meta: &Meta) {
        let Some(command) = self.on_report.clone() else { return };
        let summary: String = meta
            .summary
            .chars()
            .filter(|c| !c.is_control() && !matches!(c, '@' | '`' | '<' | '>'))
            .take(100)
            .collect();
        let env = [
            ("CHIMERA_REPORT_DIR", dir.to_string_lossy().into_owned()),
            ("CHIMERA_REPORT_ID", meta.id.clone()),
            ("CHIMERA_REPORT_KIND", meta.kind.clone()),
            ("CHIMERA_REPORT_ACCOUNT", account_of(&meta.account).to_string()),
            ("CHIMERA_REPORT_SUMMARY", summary),
        ];
        std::thread::spawn(move || {
            #[cfg(windows)]
            let mut cmd = {
                let mut c = std::process::Command::new("cmd");
                c.args(["/C", &command]);
                c
            };
            #[cfg(not(windows))]
            let mut cmd = {
                let mut c = std::process::Command::new("sh");
                c.args(["-c", &command]);
                c
            };
            cmd.envs(env).stdin(std::process::Stdio::null());
            match cmd.status() {
                Ok(s) if s.success() => {}
                Ok(s) => eprintln!("feedback: on_report ended with {s}"),
                Err(e) => eprintln!("feedback: on_report: {e}"),
            }
        });
    }

    /// Change report `id`'s status (and group, reason, commit) as `who`
    /// (`cli`, `page`), with a line in its history.
    pub fn set_status(&self, id: &str, who: &str, change: &Change) -> Result<Meta, String> {
        let _one = self.edit.lock().unwrap_or_else(|e| e.into_inner());
        let dir = self.find(id).ok_or_else(|| format!("no report {id}"))?;
        let mut meta = self.meta(id).ok_or_else(|| format!("{id}: meta.toml unreadable"))?;
        let from = meta.status;
        if let Some(s) = change.status {
            meta.status = s;
        }
        let set = |field: &mut String, v: &Option<String>| {
            if let Some(v) = v {
                *field = clean(v, 200);
            }
        };
        set(&mut meta.group, &change.group);
        set(&mut meta.reason, &change.reason);
        set(&mut meta.fixed_in, &change.fixed_in);
        if meta.status == Status::Duplicate && meta.group.is_empty() {
            return Err("duplicate: name the group (--group <id>)".into());
        }
        let text = toml::to_string(&meta).map_err(|e| e.to_string())?;
        write_atomic(&dir.join("meta.toml"), text.as_bytes()).map_err(|e| e.to_string())?;
        let mut line = format!("{} {} {} -> {}", utc_text(now_secs()), clean(who, 40), from.as_str(), meta.status.as_str());
        for (k, v) in [("group", &change.group), ("reason", &change.reason), ("fixed_in", &change.fixed_in)] {
            if let Some(v) = v {
                line += &format!(" {k}={:?}", clean(v, 200));
            }
        }
        line.push('\n');
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("history.log"))
            .and_then(|mut f| f.write_all(line.as_bytes()))
            .map_err(|e| e.to_string())?;
        Ok(meta)
    }
}
