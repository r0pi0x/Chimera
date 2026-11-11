//! Values read from an install that every app in the process may share
//! (`shared`): Combat Arms' attribute data and mounted archives are the
//! same for each app, and take seconds to read, so a `NetSim`'s server and
//! clients (or a listen server's two worlds) read them once. The process
//! keeps only a weak reference: each app holds the value (a resource, its
//! map loads' `LoadContext`), and the value is freed when the last app
//! holding it goes; the next one to ask reads it again.
//!
//! `leaked` keeps, for the process's life, values the code holds as
//! `&'static` anyway (a game's weapon set), once per equal input.
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

/// The values leaked for the process's life (`leaked`), by type and key.
type Leaked = (TypeId, String, &'static (dyn Any + Send + Sync));
static LEAKED: Mutex<Vec<Leaked>> = Mutex::new(Vec::new());

/// `make(&input)`'s value leaked for the process's life, once per key and
/// equal input: a value made from an equal input before is given back
/// (with that input, leaked with it), so data read again from the install
/// (a client's startup map's install data, freed after, then its app's)
/// doesn't leak a second copy. For values the code holds as `&'static`
/// already (a game's weapon set). `make` runs without the register's
/// lock; two racing on an equal input keep the first stored. None when
/// `make` gives none (nothing kept).
pub fn leaked<I, T>(key: &str, input: I, make: impl FnOnce(&I) -> Option<T>) -> Option<(&'static I, &'static T)>
where
    I: PartialEq + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    let find = |input: &I| -> Option<(&'static I, &'static T)> {
        let leaked = LEAKED.lock().unwrap_or_else(|e| e.into_inner());
        leaked
            .iter()
            .filter(|(t, k, _)| *t == TypeId::of::<(I, T)>() && k == key)
            .filter_map(|(_, _, v)| v.downcast_ref::<(I, T)>())
            .find(|(i, _)| i == input)
            .map(|(i, v)| (i, v))
    };
    if let Some(found) = find(&input) {
        return Some(found);
    }
    let value = make(&input)?;
    let mut leaked = LEAKED.lock().unwrap_or_else(|e| e.into_inner());
    let stored = leaked
        .iter()
        .filter(|(t, k, _)| *t == TypeId::of::<(I, T)>() && k == key)
        .filter_map(|(_, _, v)| v.downcast_ref::<(I, T)>())
        .find(|(i, _)| *i == input);
    if let Some((i, v)) = stored {
        return Some((i, v));
    }
    let pair: &'static (I, T) = Box::leak(Box::new((input, value)));
    leaked.push((TypeId::of::<(I, T)>(), key.to_string(), pair));
    Some((&pair.0, &pair.1))
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

    #[test]
    fn a_leaked_value_is_made_once_per_equal_input() {
        let a = leaked("test:leaked", vec![1u8, 2], |i| Some(i.len())).unwrap();
        let b = leaked("test:leaked", vec![1u8, 2], |_| -> Option<usize> { panic!("made again") }).unwrap();
        assert!(std::ptr::eq(a.0, b.0) && std::ptr::eq(a.1, b.1), "one value for equal inputs");
        let c = leaked("test:leaked", vec![3u8], |i| Some(i.len())).unwrap();
        assert!(!std::ptr::eq(a.1, c.1) && *c.1 == 1, "another input, its own");
        assert!(leaked("test:leaked", vec![4u8], |_| None::<usize>).is_none(), "nothing kept");
        assert_eq!(*leaked("test:leaked", vec![4u8], |_| Some(9usize)).unwrap().1, 9);
    }
}
