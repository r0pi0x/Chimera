//! Records a foreground fill is fetching right now, process-wide
//! (`Remote::fill`: a join's sets, the startup's): an on-demand miss of
//! one (`CachedFiles`) waits for it to arrive instead of asking for it
//! again, and its group leaves out the others (the 2026-11-11 rehearsal:
//! a ladder's view models, loading while the join's sets downloaded,
//! fetched 1-2 groups a second time). The paced background prefetch
//! doesn't claim its records: it is held while a player joins or changes
//! map, so a load would wait on it.

use std::{
    collections::{HashMap, HashSet},
    sync::{Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

/// Record id -> how many claims hold it.
static PENDING: Mutex<Option<HashMap<String, usize>>> = Mutex::new(None);
static CHANGED: Condvar = Condvar::new();

fn pending() -> MutexGuard<'static, Option<HashMap<String, usize>>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Records being fetched; each is released as it arrives or fails, the
/// rest when the claim is dropped.
pub struct Claim {
    ids: Mutex<HashSet<String>>,
}

/// Claim `ids` (being fetched now).
pub fn claim<'a>(ids: impl IntoIterator<Item = &'a str>) -> Claim {
    let ids: HashSet<String> = ids.into_iter().map(String::from).collect();
    let mut p = pending();
    let map = p.get_or_insert_with(HashMap::new);
    for id in &ids {
        *map.entry(id.clone()).or_default() += 1;
    }
    Claim { ids: Mutex::new(ids) }
}

impl Claim {
    /// `id` arrived (or won't): no longer waited for.
    pub fn release(&self, id: &str) {
        if self.ids.lock().unwrap_or_else(|e| e.into_inner()).remove(id) {
            unclaim([id]);
        }
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let ids = std::mem::take(&mut *self.ids.lock().unwrap_or_else(|e| e.into_inner()));
        unclaim(ids.iter().map(String::as_str));
    }
}

fn unclaim<'a>(ids: impl IntoIterator<Item = &'a str>) {
    let mut p = pending();
    if let Some(map) = p.as_mut() {
        for id in ids {
            if let Some(n) = map.get_mut(id) {
                *n -= 1;
                if *n == 0 {
                    map.remove(id);
                }
            }
        }
    }
    CHANGED.notify_all();
}

/// Whether a fill is fetching `id` now.
pub fn is_pending(id: &str) -> bool {
    pending().as_ref().is_some_and(|m| m.contains_key(id))
}

/// Wait (at most `timeout`) while a fill is fetching `id`; whether it was.
pub fn wait(id: &str, timeout: Duration) -> bool {
    let end = Instant::now() + timeout;
    let mut p = pending();
    let mut was = false;
    while p.as_ref().is_some_and(|m| m.contains_key(id)) {
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

    #[test]
    fn a_claimed_record_is_waited_for_until_released() {
        let claim = claim(["pending-test-a", "pending-test-b"]);
        assert!(is_pending("pending-test-a") && !is_pending("pending-test-c"));
        std::thread::scope(|s| {
            let waiter = s.spawn(|| wait("pending-test-a", Duration::from_secs(10)));
            std::thread::sleep(Duration::from_millis(50));
            claim.release("pending-test-a");
            assert!(waiter.join().unwrap(), "it was pending");
        });
        assert!(!is_pending("pending-test-a") && is_pending("pending-test-b"));
        drop(claim);
        assert!(!is_pending("pending-test-b"), "the rest released with the claim");
        assert!(!wait("pending-test-b", Duration::from_secs(10)));
    }
}
