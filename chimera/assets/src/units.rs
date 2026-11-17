//! Units of game files downloaded on demand, in the background
//! (docs/plans/active/ingest.md 3.5a, "Fetching"; the sandbox's
//! progressive downloads, docs/backlog.md): a game's set (`weapon/<id>`,
//! `players/<class>`, `character/<id>`, `voice/<id>`, `map/<name>`)
//! asked for with the sets it needs (its closure, which the caller
//! resolves: `chimera_net::net::units`), fetched one unit at a time on a
//! thread from the bound asset service (`sets::fetch`) at a capped rate
//! (`Pace`), never on the caller's frame.
//!
//! - **States** (`UnitState`): `Absent { size }` (the bytes a fetch would
//!   download now, the closure's records the store lacks, from the kept
//!   manifest before any record is fetched: `describe`), `Queued {
//!   position }` (1: next), `Fetching`, `Ready` (every record of the
//!   closure in the store: a mount reads them at once, `CachedFiles`
//!   reads the store at each read), `Failed { reason }` (`retry` asks
//!   again). `UnitStatus` adds files and bytes done and in all.
//! - **The queue** (`Priority`): a pick first (the newest pick before
//!   older ones), then pins, then the rest, each in the order asked.
//!   A unit asked for again at a higher priority moves up. The running
//!   fetch is never cut short.
//! - **The budget**: one `Pace` for every unit (`rate`, bytes a second;
//!   `pause` holds it while a foreground download runs; `stop` at quit).
//! - `poll` (once a frame, cheap: no file system work) notes a finished
//!   fetch, the running one's progress and the queue's positions, starts
//!   the next, and returns the units whose status changed.
//!
//! A set the kept manifest doesn't name counts as `Ready`: nothing of it
//! can be fetched as a unit (its files come on demand, as before), and a
//! dependency the manifest doesn't name is skipped (Halo's `common`).
//!
//! Known shortcuts: when the service can't be reached for a game's
//! manifest the store doesn't keep, a unit's size is unknown (`Absent {
//! size: 0 }`) until a fetch brings the manifest; no
//! cancel; a failed unit's dependencies fail with it (they were the same
//! download).

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    AssetSettings,
    pace::Pace,
    progress::Transfer,
    sets::{self, Wanted},
};

/// A unit: (game, set).
pub type UnitKey = (String, String);

/// `(game, set)` as a key.
pub fn key(game: &str, set: &str) -> UnitKey {
    (game.to_string(), set.to_string())
}

/// A unit's state (module doc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitState {
    /// Not here: `size` stored bytes to download (0: not known yet).
    Absent { size: u64 },
    /// Waiting its turn (1: next).
    Queued { position: usize },
    Fetching,
    Ready,
    Failed { reason: String },
}

impl Default for UnitState {
    fn default() -> Self {
        UnitState::Absent { size: 0 }
    }
}

/// A unit's state with its counts (bytes as transferred, compressed).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitStatus {
    pub state: UnitState,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl UnitStatus {
    fn of(state: UnitState) -> Self {
        Self { state, ..Default::default() }
    }
}

/// Which units go first (module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// A gun picked: what the player waits for.
    Pick,
    /// A pinned gun.
    Pin,
    /// Anything else ahead of need.
    Later,
}

#[derive(Clone, Debug)]
struct Want {
    key: UnitKey,
    closure: Vec<UnitKey>,
    priority: Priority,
    /// Order asked (picks: the newest first).
    seq: u64,
}

impl Want {
    fn order(&self) -> (Priority, i64) {
        let seq = self.seq as i64;
        (self.priority, if self.priority == Priority::Pick { -seq } else { seq })
    }
}

struct Job {
    want: Want,
    handle: JoinHandle<Result<(), String>>,
    transfer: Arc<Mutex<Option<Arc<Transfer>>>>,
}

/// The downloads (module doc).
pub struct Downloads {
    settings: AssetSettings,
    pace: Arc<Pace>,
    /// Waits before each new try at what didn't arrive, inside one fetch
    /// (`sets::Options::retry_delays`).
    pub retry_delays: Vec<Duration>,
    queue: Vec<Want>,
    running: Option<Job>,
    states: BTreeMap<UnitKey, UnitStatus>,
    closures: BTreeMap<UnitKey, Vec<UnitKey>>,
    to_describe: Vec<(UnitKey, Vec<UnitKey>)>,
    describing: Option<JoinHandle<Vec<(UnitKey, UnitStatus)>>>,
    seq: u64,
    changed: BTreeSet<UnitKey>,
}

/// One quick try again inside a fetch; the player's retry does the rest.
pub const RETRY_DELAYS: &[Duration] = &[Duration::from_secs(2)];

impl Downloads {
    /// Downloads into `settings`' store from its service, at most
    /// `rate` bytes a second (`u64::MAX`: no cap).
    pub fn new(settings: AssetSettings, rate: u64) -> Self {
        Self {
            settings,
            pace: Arc::new(Pace::new(rate)),
            retry_delays: RETRY_DELAYS.to_vec(),
            queue: Vec::new(),
            running: None,
            states: BTreeMap::new(),
            closures: BTreeMap::new(),
            to_describe: Vec::new(),
            describing: None,
            seq: 0,
            changed: BTreeSet::new(),
        }
    }

    pub fn settings(&self) -> &AssetSettings {
        &self.settings
    }

    /// The cap, bytes a second.
    pub fn set_rate(&self, rate: u64) {
        if self.pace.rate() != rate {
            self.pace.set_rate(rate);
        }
    }

    /// Hold (a foreground download runs) or go on.
    pub fn pause(&self, paused: bool) {
        if self.pace.paused() != paused {
            self.pace.pause(paused);
        }
    }

    /// End the running fetch (quitting); nothing more starts.
    pub fn stop(&self) {
        self.pace.stop();
    }

    pub fn status(&self, key: &UnitKey) -> Option<&UnitStatus> {
        self.states.get(key)
    }

    pub fn statuses(&self) -> &BTreeMap<UnitKey, UnitStatus> {
        &self.states
    }

    /// Whether every unit of `keys` is `Ready`.
    pub fn ready(&self, keys: &[UnitKey]) -> bool {
        keys.iter().all(|k| self.states.get(k).is_some_and(|s| s.state == UnitState::Ready))
    }

    /// The running fetch's unit.
    pub fn running(&self) -> Option<&UnitKey> {
        self.running.as_ref().map(|j| &j.want.key)
    }

    /// The units waiting, in the order they will go.
    pub fn queued(&self) -> Vec<UnitKey> {
        let mut q: Vec<&Want> = self.queue.iter().collect();
        q.sort_by_key(|w| w.order());
        q.into_iter().map(|w| w.key.clone()).collect()
    }

    fn set(&mut self, key: &UnitKey, status: UnitStatus) {
        if self.states.get(key) != Some(&status) {
            self.states.insert(key.clone(), status);
            self.changed.insert(key.clone());
        }
    }

    /// Ask for `key` with the units it needs (`closure`, without it or
    /// with it: it is fetched first either way), at `priority`. A unit
    /// fetching or ready stays so; one queued moves up when this asks
    /// higher (a pick again: to the front of the picks).
    pub fn request(&mut self, key: UnitKey, closure: Vec<UnitKey>, priority: Priority) {
        let mut all = vec![key.clone()];
        for k in closure {
            if !all.contains(&k) {
                all.push(k);
            }
        }
        self.closures.insert(key.clone(), all.clone());
        let state = self.states.get(&key).map(|s| s.state.clone());
        let in_job = self.running.as_ref().is_some_and(|j| j.want.closure.contains(&key));
        if matches!(state, Some(UnitState::Ready | UnitState::Fetching)) || in_job {
            return;
        }
        self.seq += 1;
        let seq = self.seq;
        if let Some(w) = self.queue.iter_mut().find(|w| w.key == key) {
            if priority <= w.priority {
                w.priority = priority;
                w.seq = seq;
            }
            w.closure = all;
            return;
        }
        let status = self.states.get(&key).cloned().unwrap_or_default();
        let size = match status.state {
            UnitState::Absent { size } => size,
            _ => status.bytes_total,
        };
        self.queue.push(Want { key: key.clone(), closure: all, priority, seq });
        self.set(&key, UnitStatus { state: UnitState::Queued { position: 0 }, bytes_total: size, ..Default::default() });
    }

    /// Unqueue every waiting unit `keep` doesn't name (a Want not yet
    /// started): back to `Absent` with its size. The running fetch is not
    /// cut (it ends on its own; nothing queued follows it). The units
    /// unqueued.
    pub fn cancel_queued(&mut self, keep: impl Fn(&UnitKey) -> bool) -> Vec<UnitKey> {
        let (kept, gone): (Vec<Want>, Vec<Want>) = std::mem::take(&mut self.queue).into_iter().partition(|w| keep(&w.key));
        self.queue = kept;
        let mut out = Vec::new();
        for w in gone {
            let size = self.states.get(&w.key).map_or(0, |s| s.bytes_total);
            self.set(&w.key, UnitStatus { state: UnitState::Absent { size }, bytes_total: size, ..Default::default() });
            out.push(w.key);
        }
        out
    }

    /// Ask again for a unit that failed (at a pick's priority); false when
    /// it hadn't failed.
    pub fn retry(&mut self, key: &UnitKey) -> bool {
        if !matches!(self.states.get(key).map(|s| &s.state), Some(UnitState::Failed { .. })) {
            return false;
        }
        let closure = self.closures.get(key).cloned().unwrap_or_else(|| vec![key.clone()]);
        let status = UnitStatus::default();
        self.states.insert(key.clone(), status);
        self.request(key.clone(), closure, Priority::Pick);
        true
    }

    /// Find out, on a thread, whether units not seen yet are here and
    /// what they would download (`Absent { size }`), each with its
    /// closure.
    pub fn describe(&mut self, units: Vec<(UnitKey, Vec<UnitKey>)>) {
        for (k, c) in units {
            if self.states.contains_key(&k) || self.to_describe.iter().any(|d| d.0 == k) {
                continue;
            }
            self.closures.entry(k.clone()).or_insert_with(|| c.clone());
            self.to_describe.push((k, c));
        }
    }

    /// Once a frame (module doc): the units whose status changed, with it.
    pub fn poll(&mut self) -> Vec<(UnitKey, UnitStatus)> {
        self.poll_described();
        self.poll_running();
        if self.running.is_none() && !self.queue.is_empty() && !self.pace.stopped() {
            self.start_next();
        }
        // The queue's positions.
        let order = self.queued();
        for (i, k) in order.iter().enumerate() {
            let mut s = self.states.get(k).cloned().unwrap_or_default();
            s.state = UnitState::Queued { position: i + 1 };
            self.set(k, s);
        }
        std::mem::take(&mut self.changed)
            .into_iter()
            .filter_map(|k| self.states.get(&k).cloned().map(|s| (k, s)))
            .collect()
    }

    fn poll_described(&mut self) {
        if self.describing.as_ref().is_some_and(|h| h.is_finished()) {
            let found = self.describing.take().expect("checked").join().unwrap_or_default();
            for (k, s) in found {
                // A request since then knows better.
                if self.states.get(&k).is_none_or(|old| matches!(old.state, UnitState::Absent { .. })) {
                    self.set(&k, s);
                }
            }
        }
        if self.describing.is_none() && !self.to_describe.is_empty() {
            let units = std::mem::take(&mut self.to_describe);
            let settings = self.settings.clone();
            self.describing = std::thread::Builder::new()
                .name("unit sizes".into())
                .spawn(move || describe(&settings, &units))
                .ok();
        }
    }

    fn poll_running(&mut self) {
        let Some(job) = self.running.as_ref() else { return };
        if job.handle.is_finished() {
            let job = self.running.take().expect("checked");
            let result = job.handle.join().unwrap_or(Err("the download panicked".into()));
            let (files, bytes, total, files_total) = counts(&job.transfer);
            for k in &job.want.closure {
                let done = matches!(self.states.get(k).map(|s| &s.state), Some(UnitState::Ready));
                let status = match &result {
                    Ok(()) => UnitStatus {
                        state: UnitState::Ready,
                        files_done: files,
                        files_total,
                        bytes_done: bytes,
                        bytes_total: total,
                    },
                    Err(_) if done => continue,
                    Err(reason) => UnitStatus {
                        state: UnitState::Failed { reason: reason.clone() },
                        files_done: files,
                        files_total,
                        bytes_done: bytes,
                        bytes_total: total,
                    },
                };
                self.set(k, status);
            }
            return;
        }
        let (files, bytes, total, files_total) = counts(&job.transfer);
        let closure = job.want.closure.clone();
        for k in &closure {
            if matches!(self.states.get(k).map(|s| &s.state), Some(UnitState::Ready)) {
                continue;
            }
            let status = UnitStatus { state: UnitState::Fetching, files_done: files, files_total, bytes_done: bytes, bytes_total: total };
            self.set(k, status);
        }
    }

    fn start_next(&mut self) {
        let order = self.queued();
        let Some(first) = order.first() else { return };
        let i = self.queue.iter().position(|w| w.key == *first).expect("queued");
        let want = self.queue.remove(i);
        let transfer: Arc<Mutex<Option<Arc<Transfer>>>> = Arc::default();
        let (settings, pace, delays, t) = (self.settings.clone(), self.pace.clone(), self.retry_delays.clone(), transfer.clone());
        let closure = want.closure.clone();
        let run = move || fetch(&settings, &closure, &pace, &delays, &t);
        let handle = match std::thread::Builder::new().name("unit download".into()).spawn(run) {
            Ok(h) => h,
            Err(e) => {
                let reason = format!("couldn't start the download: {e}");
                self.set(&want.key, UnitStatus::of(UnitState::Failed { reason }));
                return;
            }
        };
        for k in &want.closure {
            if !matches!(self.states.get(k).map(|s| &s.state), Some(UnitState::Ready)) {
                let s = self.states.get(k).cloned().unwrap_or_default();
                self.set(k, UnitStatus { state: UnitState::Fetching, files_done: 0, bytes_done: 0, ..s });
            }
        }
        self.running = Some(Job { want, handle, transfer });
    }
}

/// Files and bytes done, bytes in all, files in all, of a transfer.
fn counts(transfer: &Mutex<Option<Arc<Transfer>>>) -> (u64, u64, u64, u64) {
    transfer.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or((0, 0, 0, 0), |t| {
        let (f, b, total) = t.done();
        (f, b, total, t.files())
    })
}

/// The closure's sets per game, in order.
fn wanted(closure: &[UnitKey]) -> Vec<Wanted> {
    let mut out: Vec<Wanted> = Vec::new();
    for (game, set) in closure {
        match out.iter_mut().find(|w| w.game == *game) {
            Some(w) => w.sets.push(set.clone()),
            None => out.push(Wanted { game: game.clone(), sets: vec![set.clone()] }),
        }
    }
    out
}

/// A unit's fetch (on its thread): the closure's records, then a look
/// that every set the manifest names is whole.
fn fetch(
    settings: &AssetSettings,
    closure: &[UnitKey],
    pace: &Pace,
    delays: &[Duration],
    transfer: &Mutex<Option<Arc<Transfer>>>,
) -> Result<(), String> {
    let wants = wanted(closure);
    let options = sets::Options { pace: Some(pace), retry_delays: Some(delays), ..Default::default() };
    sets::fetch(settings, &wants, "the files asked for", transfer, options)?;
    for w in &wants {
        let names: Vec<&str> = w.sets.iter().map(String::as_str).collect();
        let lack = sets::lacking(settings, &w.game, &names).ok_or_else(|| format!("no manifest of {}", w.game))?;
        if lack.files > 0 {
            return Err(format!("{} files of {} didn't arrive; try again", lack.files, w.game));
        }
    }
    Ok(())
}

/// Each unit's state from the kept manifest (fetched first when the store
/// has none) and the store (module doc): `Ready` when its
/// closure lacks nothing (or the manifest doesn't name it), else `Absent`
/// with what it would download.
pub fn describe(settings: &AssetSettings, units: &[(UnitKey, Vec<UnitKey>)]) -> Vec<(UnitKey, UnitStatus)> {
    let mut manifests: BTreeMap<String, Option<(crate::Store, crate::Manifest)>> = BTreeMap::new();
    let mut out = Vec::new();
    for (key, closure) in units {
        let mut all = vec![key.clone()];
        all.extend(closure.iter().filter(|k| *k != key).cloned());
        let mut size = 0;
        let mut known = true;
        let mut named = true;
        for w in wanted(&all) {
            // The service's manifest when the store keeps none (only the
            // manifest: a few hundred kB at most).
            let kept = manifests.entry(w.game.clone()).or_insert_with(|| {
                sets::kept_manifest(settings, &w.game).or_else(|| settings.build(&w.game).and_then(|_| sets::kept_manifest(settings, &w.game)))
            });
            let Some((store, manifest)) = kept else {
                known = false;
                continue;
            };
            let names: Vec<&str> = w.sets.iter().map(String::as_str).collect();
            let lack = sets::lacking_in(store, manifest, &names);
            if w.game == key.0 && !lack.named.contains(&key.1) {
                named = false;
            }
            size += lack.bytes;
        }
        let state = match (known, named, size) {
            (true, false, _) | (true, true, 0) => UnitState::Ready,
            _ => UnitState::Absent { size },
        };
        out.push((key.clone(), UnitStatus { state, bytes_total: size, ..Default::default() }));
    }
    out
}
