//! `Memos`: values worked out once on first use and kept by their owner,
//! one per type: what a game memoizes on a value it already has (its
//! install's data: Combat Arms' `CaData` keeps its converted character
//! models, view models and parsed effect file there) or on an app's map
//! loads (`map::LoadContext`, the app's context its loaders get).
//!
//! Cloning shares the store (an `Arc`): every clone sees what any fills,
//! and the values go when the last clone does. Each type's value is built
//! once (`get_or_init`; a second caller waits for the first), outside the
//! store's lock, so a value's `init` may fill another type's.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

type Cell = Arc<dyn Any + Send + Sync>;

/// One value per type, filled on first use (module doc).
#[derive(Clone, Default)]
pub struct Memos(Arc<Mutex<HashMap<TypeId, Cell>>>);

impl Memos {
    /// The cell of type `T`, made empty when missing.
    fn cell<T: Any + Send + Sync>(&self) -> Arc<OnceLock<Arc<T>>> {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let cell = map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Arc::new(OnceLock::<Arc<T>>::new()))
            .clone();
        cell.downcast().expect("keyed by its type")
    }

    /// The value of type `T`, built by `init` the first time.
    pub fn get_or_init<T: Any + Send + Sync>(&self, init: impl FnOnce() -> T) -> Arc<T> {
        self.cell::<T>().get_or_init(|| Arc::new(init())).clone()
    }

    /// The value of type `T`, if one was built or set.
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let cell = map.get(&TypeId::of::<T>())?.clone().downcast::<OnceLock<Arc<T>>>().ok()?;
        cell.get().cloned()
    }

    /// Set the value of type `T` (in place of any).
    pub fn insert<T: Any + Send + Sync>(&self, value: T) {
        let cell = OnceLock::new();
        let _ = cell.set(Arc::new(value));
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(TypeId::of::<T>(), Arc::new(cell));
    }

    /// Forget the value of type `T` (the next `get_or_init` builds anew).
    pub fn remove<T: Any + Send + Sync>(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(&TypeId::of::<T>());
    }

    /// Forget every value.
    pub fn clear(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Whether `self` and `other` are one store (clones of each other).
    pub fn same(&self, other: &Memos) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Memos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.0.lock().map_or(0, |m| m.len());
        write!(f, "Memos({n})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_built_once_and_shared_by_clones() {
        let memos = Memos::default();
        let built = std::sync::atomic::AtomicU32::new(0);
        let a = memos.get_or_init(|| {
            built.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            7u32
        });
        let b = memos.clone().get_or_init(|| 8u32);
        assert_eq!((*a, *b, built.into_inner()), (7, 7, 1));
        assert!(memos.get::<String>().is_none());
        memos.insert(String::from("x"));
        assert_eq!(memos.get::<String>().as_deref().map(String::as_str), Some("x"));
        memos.remove::<u32>();
        assert!(memos.get::<u32>().is_none());
        assert!(!memos.same(&Memos::default()) && memos.same(&memos.clone()));
    }

    #[test]
    fn an_init_may_fill_another_type() {
        let memos = Memos::default();
        let outer = memos.get_or_init(|| *memos.get_or_init(|| 2u8) as u16 + 1);
        assert_eq!((*outer, memos.get::<u8>().map(|v| *v)), (3, Some(2)));
    }
}
