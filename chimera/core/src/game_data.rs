//! A crate's data documents (weapon TOMLs, rules, HUD and menu layouts,
//! minigame tables: our own text files beside its sources), read from the
//! checkout at run time in a dev build and compiled in otherwise
//! (docs/plans/active/architecture-for-iteration.md, stage 2).
//!
//! `game_data!("data/faces.toml")` names a file by its path from the
//! calling crate's folder (its `Cargo.toml`'s) and makes a `GameData`, a
//! `const` like the `include_str!` it replaces:
//!
//! - with `dev` (this crate's feature, which every crate's `dev` turns
//!   on): the macro keeps only the file's absolute path; the first
//!   `text()` reads it and keeps it for the process. Cargo doesn't know the
//!   file, so editing it rebuilds nothing: restart the app (or `data_reload`
//!   in the console, for the readers that ask again) and the tests that read
//!   it see the new text. A file that can't be read is a panic naming it.
//! - without (releases, `--profile playtest`, the servers as released):
//!   `include_str!`, the bytes in the binary, no path in it.
//!
//! The same text either way, so tests are as deterministic as before (the
//! file is in the repository); the module's tests check that the two
//! paths agree. Our own documents only: never another game's files (those
//! come from the player's install through the VFS, `mount`).
//!
//! `reload()` (the `data_reload` console command, `console`'s built-ins) reads every file read so
//! far again and bumps `generation()`; a reader that calls `text()` when it
//! needs the text (not once at start-up) picks the change up.
//!
//! Known shortcuts: a dev build reads from the checkout it was built in, so
//! a dev binary moved to another machine can't find its data (dev builds
//! aren't distributed). Each reload leaks the old text of a changed file
//! (dev only; files are kilobytes). Binary files (`include_bytes!`: the
//! party kit's icons, Ultimate Gun Game's UI sounds) are still compiled in.

use std::sync::atomic::{AtomicU64, Ordering};

/// One data document: its text, from the binary or the checkout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameData {
    /// The file: its absolute path in a dev build, the path given to the
    /// macro otherwise.
    path: &'static str,
    /// The text, when compiled in.
    embedded: Option<&'static str>,
}

impl GameData {
    /// A file compiled in (the macro without `dev`).
    #[doc(hidden)]
    pub const fn embedded(path: &'static str, text: &'static str) -> Self {
        Self {
            path,
            embedded: Some(text),
        }
    }

    /// A document given as text, from no file (a test's table).
    pub const fn from_text(text: &'static str) -> Self {
        Self {
            path: "",
            embedded: Some(text),
        }
    }

    /// A file read from `path` on first use (the macro with `dev`).
    #[doc(hidden)]
    pub const fn disk(path: &'static str) -> Self {
        Self { path, embedded: None }
    }

    /// The document's text.
    pub fn text(&self) -> &'static str {
        match self.embedded {
            Some(text) => text,
            None => disk::text(self.path),
        }
    }

    /// The file it came from (absolute in a dev build).
    pub fn path(&self) -> &'static str {
        self.path
    }

    /// Whether it is read from the checkout (a dev build) rather than
    /// compiled in.
    pub fn from_disk(&self) -> bool {
        self.embedded.is_none()
    }
}

impl std::ops::Deref for GameData {
    type Target = str;

    fn deref(&self) -> &str {
        self.text()
    }
}

/// Named documents' texts (a weapon set's files, in order), for an API that
/// takes `&[(&str, &str)]`.
pub fn texts(docs: &[(&'static str, GameData)]) -> Vec<(&'static str, &'static str)> {
    docs.iter().map(|(name, data)| (*name, data.text())).collect()
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

/// How many reloads changed something: a reader that keeps parsed data can
/// compare it with the one it parsed under.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Reads every document read so far again (a dev build; none otherwise)
/// and returns the paths whose text changed.
pub fn reload() -> Vec<&'static str> {
    let changed = disk::reload();
    if !changed.is_empty() {
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
    changed
}

mod disk {
    use std::{
        collections::BTreeMap,
        sync::{LazyLock, Mutex},
    };

    /// Path -> text, for the files read so far.
    static TEXTS: LazyLock<Mutex<BTreeMap<&'static str, &'static str>>> = LazyLock::new(Default::default);

    fn read(path: &str) -> &'static str {
        match std::fs::read_to_string(path) {
            Ok(text) => Box::leak(text.into_boxed_str()),
            Err(e) => panic!(
                "data file {path}: {e} (a dev build reads its data documents from the checkout it was built in: \
                 chimera_core::game_data)"
            ),
        }
    }

    pub(super) fn text(path: &'static str) -> &'static str {
        let mut texts = TEXTS.lock().unwrap_or_else(|e| e.into_inner());
        texts.entry(path).or_insert_with(|| read(path))
    }

    pub(super) fn reload() -> Vec<&'static str> {
        let mut texts = TEXTS.lock().unwrap_or_else(|e| e.into_inner());
        let mut changed = Vec::new();
        for (path, text) in texts.iter_mut() {
            let Ok(new) = std::fs::read_to_string(path) else { continue };
            if new != *text {
                *text = Box::leak(new.into_boxed_str());
                changed.push(*path);
            }
        }
        changed
    }
}

/// A data document of the calling crate, by its path from the crate's
/// folder (pieces are joined, as `concat!` does): `pub const DATA:
/// GameData = game_data!("data/faces.toml");`.
/// Read from the checkout in a dev build, compiled in otherwise (the
/// module's doc).
#[cfg(feature = "dev")]
#[macro_export]
macro_rules! game_data {
    ($($path:expr),+ $(,)?) => {
        $crate::game_data::GameData::disk(concat!(env!("CARGO_MANIFEST_DIR"), "/", $($path),+))
    };
}

/// A data document of the calling crate, by its path from the crate's
/// folder (pieces are joined, as `concat!` does): `pub const DATA:
/// GameData = game_data!("data/faces.toml");`.
/// Read from the checkout in a dev build, compiled in otherwise (the
/// module's doc).
#[cfg(not(feature = "dev"))]
#[macro_export]
macro_rules! game_data {
    ($($path:expr),+ $(,)?) => {
        $crate::game_data::GameData::embedded(
            concat!($($path),+),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $($path),+)),
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The macro's text is the file's, read or compiled in.
    #[test]
    fn both_paths_give_the_files_text() {
        const MANIFEST: GameData = game_data!("Cargo.toml");
        assert_eq!(MANIFEST.text(), include_str!("../Cargo.toml"));
        assert_eq!(MANIFEST.from_disk(), cfg!(feature = "dev"));
        assert!(MANIFEST.path().ends_with("Cargo.toml"));
        const PIECES: GameData = game_data!("src/", "game_data.rs");
        assert_eq!(&*PIECES, include_str!("game_data.rs"));
        assert_eq!(texts(&[("m", MANIFEST)]), [("m", include_str!("../Cargo.toml"))]);
    }

    /// `reload` picks up an edited file and only reports what changed.
    #[test]
    fn reload_reads_an_edited_file_again() {
        let dir = std::env::temp_dir().join(format!("chimera_game_data_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("doc.toml");
        std::fs::write(&file, "a = 1\n").unwrap();
        let data = GameData::disk(Box::leak(file.to_string_lossy().into_owned().into_boxed_str()));
        assert_eq!(data.text(), "a = 1\n");
        let before = generation();
        std::fs::write(&file, "a = 2\n").unwrap();
        assert!(reload().contains(&data.path()));
        assert_eq!(data.text(), "a = 2\n");
        assert!(generation() > before);
        assert!(!reload().contains(&data.path()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
