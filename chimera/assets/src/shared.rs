//! Values read from an install that every app in the process may share
//! (`shared`): Combat Arms' attribute data and mounted archives are the
//! same for each app, and take seconds to read, so a `NetSim`'s server and
//! clients (or a listen server's two worlds) read them once. The process
//! keeps only a weak reference: each app holds the value (a resource, its
//! map loads' `LoadContext`), and the value is freed when the last app
//! holding it goes; the next one to ask reads it again.
//!
//! The register is process-wide by nature (it is the machine's install, as
//! the asset store is), so it lives here, under the mounts, rather than in
//! a simulation crate (`checks/tests/it/determinism.rs`).

use std::{
    any::{Any, TypeId},
    sync::{Arc, Mutex, Weak},
};

type Entry = (TypeId, String, Weak<dyn Any + Send + Sync>);

/// The live values by type and key (dead entries are dropped on insert).
static LIVE: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

fn find<T: Any + Send + Sync>(key: &str) -> Option<Arc<T>> {
    let live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    live.iter()
        .filter(|(t, k, _)| *t == TypeId::of::<T>() && k == key)
        .find_map(|(_, _, w)| w.upgrade())
        .and_then(|v| v.downcast::<T>().ok())
}

/// The value of type `T` under `key` that an app holds, else `init`'s
/// (held from now on by the caller only). `init` runs without the
/// register's lock (it may ask for other shared values) and should be
/// cheap: a value that takes long to read fills itself lazily. Two
/// callers racing on a missing value may both run `init`; the first one
/// stored wins.
pub fn shared<T: Any + Send + Sync>(key: &str, init: impl FnOnce() -> T) -> Arc<T> {
    shared_opt(key, || Some(init())).expect("init always gives a value")
}

/// `shared` whose `init` may give nothing (no install): nothing is
/// stored then, and the next call tries again.
pub fn shared_opt<T: Any + Send + Sync>(key: &str, init: impl FnOnce() -> Option<T>) -> Option<Arc<T>> {
    if let Some(v) = find::<T>(key) {
        return Some(v);
    }
    let made = Arc::new(init()?);
    let mut live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    live.retain(|(_, _, w)| w.strong_count() > 0);
    let stored = live
        .iter()
        .filter(|(t, k, _)| *t == TypeId::of::<T>() && k == key)
        .find_map(|(_, _, w)| w.upgrade())
        .and_then(|v| v.downcast::<T>().ok());
    if let Some(v) = stored {
        return Some(v);
    }
    let erased: Arc<dyn Any + Send + Sync> = made.clone();
    live.push((TypeId::of::<T>(), key.to_string(), Arc::downgrade(&erased)));
    Some(made)
}

/// Whether a value of type `T` under `key` is held by anyone (tests).
pub fn is_live<T: Any + Send + Sync>(key: &str) -> bool {
    find::<T>(key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_shared_while_held_and_freed_after() {
        struct Data(u32);
        let a = shared("test:data", || Data(1));
        let b = shared("test:data", || Data(2));
        assert!(Arc::ptr_eq(&a, &b) && b.0 == 1, "the held value is shared");
        assert_eq!(shared("test:other", || Data(3)).0, 3, "by key");
        drop((a, b));
        assert!(!is_live::<Data>("test:data"), "freed with its last holder");
        assert_eq!(shared("test:data", || Data(4)).0, 4, "read again");
        assert!(shared_opt::<Data>("test:none", || None).is_none());
    }
}
