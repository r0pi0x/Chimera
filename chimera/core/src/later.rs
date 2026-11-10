//! `Later<T>`: a value made the first time something asks for it, from
//! what its maker captured when it was set (docs/performance.md, "Poses
//! on demand"). A game that can say what a value will be but not whether
//! anyone will want it (a character's posed skeleton, its hitboxes, a
//! body's bone pose for drawing) hands out a `Later` instead of the value:
//! readers call `get` (or deref) and the first one makes it; a value
//! nobody reads costs only the capture.
//!
//! Cloning shares the cell (an `Arc`). The maker runs at most once, on
//! whichever thread asks first (another asking meanwhile waits), so the
//! value is the same whoever asks and whenever: a maker must depend only
//! on what it captured, never on the world at the time it runs.

use std::{
    fmt,
    ops::Deref,
    sync::{Arc, Mutex, OnceLock},
};

type Maker<T> = Box<dyn FnOnce() -> T + Send>;

struct Cell<T> {
    value: OnceLock<T>,
    make: Mutex<Option<Maker<T>>>,
}

/// A value made on first use (module doc).
pub struct Later<T>(Arc<Cell<T>>);

impl<T> Later<T> {
    /// A value `make` makes when first asked for.
    pub fn new(make: impl FnOnce() -> T + Send + 'static) -> Self {
        Self(Arc::new(Cell {
            value: OnceLock::new(),
            make: Mutex::new(Some(Box::new(make))),
        }))
    }

    /// A value already made.
    pub fn ready(value: T) -> Self {
        Self(Arc::new(Cell {
            value: OnceLock::from(value),
            make: Mutex::new(None),
        }))
    }

    /// The value, made now if nothing asked before.
    pub fn get(&self) -> &T {
        self.0.value.get_or_init(|| {
            let make = self.0.make.lock().unwrap_or_else(|e| e.into_inner()).take();
            make.expect("a Later's maker runs once")()
        })
    }

    /// Whether it has been made (tests, statistics).
    pub fn is_made(&self) -> bool {
        self.0.value.get().is_some()
    }

    /// Whether both are clones of one cell.
    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
}

impl<T> Clone for Later<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Later<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}

impl<T: Default> Default for Later<T> {
    fn default() -> Self {
        Self::ready(T::default())
    }
}

impl<T: fmt::Debug> fmt::Debug for Later<T> {
    /// Shows the value only once made (printing never makes it).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.value.get() {
            Some(v) => f.debug_tuple("Later").field(v).finish(),
            None => f.write_str("Later(<not made>)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn made_once_on_first_use_and_shared_by_clones() {
        let runs = Arc::new(AtomicU32::new(0));
        let r = runs.clone();
        let later = Later::new(move || {
            r.fetch_add(1, Ordering::Relaxed);
            41 + 1
        });
        let other = later.clone();
        assert!(!later.is_made());
        assert_eq!(runs.load(Ordering::Relaxed), 0);
        assert_eq!(*other, 42);
        assert_eq!(*later.get(), 42);
        assert!(later.is_made() && Later::ptr_eq(&later, &other));
        assert_eq!(runs.load(Ordering::Relaxed), 1);
        assert!(Later::ready(1).is_made());
        assert_eq!(format!("{:?}", Later::<u8>::new(|| 1)), "Later(<not made>)");
    }

    #[test]
    fn asked_from_many_threads_made_once() {
        let runs = Arc::new(AtomicU32::new(0));
        let r = runs.clone();
        let later = Later::new(move || {
            r.fetch_add(1, Ordering::Relaxed);
            7u64
        });
        std::thread::scope(|s| {
            for _ in 0..8 {
                let l = later.clone();
                s.spawn(move || assert_eq!(*l, 7));
            }
        });
        assert_eq!(runs.load(Ordering::Relaxed), 1);
    }
}
