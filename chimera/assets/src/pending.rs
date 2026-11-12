//! Records a foreground fill is fetching right now into a store
//! (`Remote::fill`: a join's sets, the startup's): an on-demand miss of
//! one in the same store (`CachedFiles`) waits for it to arrive instead of
//! asking for it again, and its group leaves out the others (the 2026-11-11 rehearsal:
//! a ladder's view models, loading while the join's sets downloaded,
//! fetched 1-2 groups a second time). The paced background prefetch
//! doesn't claim its records: it is held while a player joins or changes
//! map, so a load would wait on it.
//!
//! A fill knows its records only after a few answers from the service
//! (who the token is, its manifests): from the moment a join or level
//! change decides to fetch until the fill's claim, it is `starting`, and
//! an on-demand miss waits for the claim first (`wait_started`). Without
//! it the ladder's held models and bodies, which the client starts loading
//! as the new match's ladder arrives with the level change, missed in that
//! gap and came one group each (a seeded three-map session, 2026-11-12:
//! 39 of 62 on-demand requests, 17 of 32 MB, all of them in the offer).
//!
//! Both are kept per store (its root folder): a fill into one store never
//! holds a miss in another, which it wouldn't fill (two stores in one
//! process: a client and a server, or tests run in parallel, whose
//! fixtures' records are the same bytes).

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::Store;

/// Store root -> foreground fills into it started but not yet claiming
/// (`Starting`).
static STARTING: Mutex<Option<HashMap<PathBuf, usize>>> = Mutex::new(None);
static STARTED: Condvar = Condvar::new();

fn starting_now() -> MutexGuard<'static, Option<HashMap<PathBuf, usize>>> {
    STARTING.lock().unwrap_or_else(|e| e.into_inner())
}

/// A foreground fill between its start and its claim: on-demand misses in
/// its store wait (`wait_started`) until `started` is called or it is
/// dropped.
pub struct Starting {
    root: PathBuf,
    done: AtomicBool,
}

/// A foreground fill into `store` begins (before it asks the service
/// anything).
pub fn starting(store: &Store) -> Starting {
    *starting_now().get_or_insert_with(HashMap::new).entry(store.root.clone()).or_default() += 1;
    Starting { root: store.root.clone(), done: AtomicBool::new(false) }
}

impl Starting {
    /// The fill has claimed its records (or won't fetch): misses go on.
    pub fn started(&self) {
        if !self.done.swap(true, Ordering::AcqRel) {
            let mut map = starting_now();
            if let Some(m) = map.as_mut()
                && let Some(n) = m.get_mut(&self.root)
            {
                *n -= 1;
                if *n == 0 {
                    m.remove(&self.root);
                }
            }
            STARTED.notify_all();
        }
    }
}

impl Drop for Starting {
    fn drop(&mut self) {
        self.started();
    }
}

/// Wait (at most `timeout`) while a foreground fill into `store` is
/// starting; whether one was.
pub fn wait_started(store: &Store, timeout: Duration) -> bool {
    let end = Instant::now() + timeout;
    let mut map = starting_now();
    let mut was = false;
    while map.as_ref().is_some_and(|m| m.contains_key(&store.root)) {
        was = true;
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        map = STARTED.wait_timeout(map, left).unwrap_or_else(|e| e.into_inner()).0;
    }
    was
}

/// (Store root, record id) -> how many claims hold it.
type Key = (PathBuf, String);
static PENDING: Mutex<Option<HashMap<Key, usize>>> = Mutex::new(None);
static CHANGED: Condvar = Condvar::new();

fn pending() -> MutexGuard<'static, Option<HashMap<Key, usize>>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Records being fetched into a store; each is released as it arrives or
/// fails, the rest when the claim is dropped.
pub struct Claim {
    root: PathBuf,
    ids: Mutex<HashSet<String>>,
}

/// Claim `ids` (being fetched into `store` now).
pub fn claim<'a>(store: &Store, ids: impl IntoIterator<Item = &'a str>) -> Claim {
    let ids: HashSet<String> = ids.into_iter().map(String::from).collect();
    let mut p = pending();
    let map = p.get_or_insert_with(HashMap::new);
    for id in &ids {
        *map.entry((store.root.clone(), id.clone())).or_default() += 1;
    }
    Claim { root: store.root.clone(), ids: Mutex::new(ids) }
}

impl Claim {
    /// `id` arrived (or won't): no longer waited for.
    pub fn release(&self, id: &str) {
        if self.ids.lock().unwrap_or_else(|e| e.into_inner()).remove(id) {
            unclaim(&self.root, [id]);
        }
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let ids = std::mem::take(&mut *self.ids.lock().unwrap_or_else(|e| e.into_inner()));
        unclaim(&self.root, ids.iter().map(String::as_str));
    }
}

fn unclaim<'a>(root: &std::path::Path, ids: impl IntoIterator<Item = &'a str>) {
    let mut p = pending();
    if let Some(map) = p.as_mut() {
        for id in ids {
            let key = (root.to_path_buf(), id.to_string());
            if let Some(n) = map.get_mut(&key) {
                *n -= 1;
                if *n == 0 {
                    map.remove(&key);
                }
            }
        }
    }
    CHANGED.notify_all();
}

fn holds(map: &Option<HashMap<Key, usize>>, store: &Store, id: &str) -> bool {
    map.as_ref().is_some_and(|m| m.contains_key(&(store.root.clone(), id.to_string())))
}

/// Whether a fill is fetching `id` into `store` now.
pub fn is_pending(store: &Store, id: &str) -> bool {
    holds(&pending(), store, id)
}

/// Wait (at most `timeout`) while a fill is fetching `id` into `store`;
/// whether it was.
pub fn wait(store: &Store, id: &str, timeout: Duration) -> bool {
    let end = Instant::now() + timeout;
    let mut p = pending();
    let mut was = false;
    while holds(&p, store, id) {
        was = true;
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        p = CHANGED.wait_timeout(p, left).unwrap_or_else(|e| e.into_inner()).0;
    }
    was
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> Store {
        Store::new(std::env::temp_dir().join(format!("chimera-pending-test-{name}")))
    }

    #[test]
    fn a_claimed_record_is_waited_for_until_released() {
        let (s, other) = (store("claimed"), store("claimed-other"));
        let claim = claim(&s, ["pending-test-a", "pending-test-b"]);
        assert!(is_pending(&s, "pending-test-a") && !is_pending(&s, "pending-test-c"));
        assert!(!is_pending(&other, "pending-test-a"), "another store's miss isn't held");
        assert!(!wait(&other, "pending-test-a", Duration::from_secs(10)));
        std::thread::scope(|sc| {
            let waiter = sc.spawn(|| wait(&s, "pending-test-a", Duration::from_secs(10)));
            std::thread::sleep(Duration::from_millis(50));
            claim.release("pending-test-a");
            assert!(waiter.join().unwrap(), "it was pending");
        });
        assert!(!is_pending(&s, "pending-test-a") && is_pending(&s, "pending-test-b"));
        drop(claim);
        assert!(!is_pending(&s, "pending-test-b"), "the rest released with the claim");
        assert!(!wait(&s, "pending-test-b", Duration::from_secs(10)));
    }

    #[test]
    fn a_miss_waits_while_a_fill_is_starting() {
        let (s, other) = (store("starting"), store("starting-other"));
        assert!(!wait_started(&s, Duration::from_secs(10)), "nothing starting");
        let start = starting(&s);
        assert!(!wait_started(&other, Duration::from_secs(10)), "another store's miss goes on");
        std::thread::scope(|sc| {
            let waiter = sc.spawn(|| {
                let t = Instant::now();
                (wait_started(&s, Duration::from_secs(10)), t.elapsed())
            });
            std::thread::sleep(Duration::from_millis(50));
            start.started();
            let (was, took) = waiter.join().unwrap();
            assert!(was && took >= Duration::from_millis(40) && took < Duration::from_secs(5), "{took:?}");
        });
        // Called again, or dropped after: nothing more to release.
        start.started();
        drop(start);
        assert!(!wait_started(&s, Duration::from_secs(10)));
        // Dropped without `started`: released too.
        drop(starting(&s));
        assert!(!wait_started(&s, Duration::from_secs(10)));
    }
}
