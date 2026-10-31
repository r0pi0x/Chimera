//! What a load reads, recorded: `Recording` wraps a mount layer and tells a
//! `Recorder` each file read from it (or found by `contains`), once per
//! game and path per process, with its size and SHA-256. With
//! `CHIMERA_VFS_LOG=<file>` every game mount does this (Chimera's
//! `mount::served`) and the recorder appends one line per file to that
//! file: `<game>\t<path>\t<size>\t<sha256>`. Several processes may append
//! to one log (a test run's binaries): each line is one write. A set for
//! `chimera_pack` is the lines of one game (`read_log`).

use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

use chimera_vfs::{Entry, FileSource};

use crate::sha256_hex;

pub const LOG_ENV: &str = "CHIMERA_VFS_LOG";

/// One file a load read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub game: String,
    pub path: String,
    pub size: u64,
    pub id: String,
}

/// Collects what `Recording`s see; writes it to a log file when it has
/// one.
#[derive(Default)]
pub struct Recorder {
    file: Option<Mutex<File>>,
    /// (game, path) -> line.
    seen: Mutex<BTreeMap<(String, String), LogLine>>,
}

impl Recorder {
    /// Recording in memory only (tests, the packer's own runs).
    pub fn in_memory() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Appending to `path` too.
    pub fn to_file(path: &Path) -> io::Result<Arc<Self>> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Arc::new(Self { file: Some(Mutex::new(file)), ..Default::default() }))
    }

    /// The process's recorder when `CHIMERA_VFS_LOG` is set (made once).
    pub fn from_env() -> Option<Arc<Self>> {
        static ENV: OnceLock<Option<Arc<Recorder>>> = OnceLock::new();
        ENV.get_or_init(|| {
            let path = std::env::var_os(LOG_ENV).filter(|p| !p.is_empty())?;
            Self::to_file(Path::new(&path))
                .inspect_err(|e| eprintln!("{LOG_ENV}: {}: {e}", Path::new(&path).display()))
                .ok()
        })
        .clone()
    }

    fn has(&self, game: &str, path: &str) -> bool {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&(game.into(), path.into()))
    }

    /// Note that `game`'s `path` was read and held `bytes`.
    pub fn record(&self, game: &str, path: &str, bytes: &[u8]) {
        if self.has(game, path) {
            return;
        }
        let line = LogLine { game: game.into(), path: path.into(), size: bytes.len() as u64, id: sha256_hex(bytes) };
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        if seen.contains_key(&(game.into(), path.into())) {
            return;
        }
        if let Some(file) = &self.file {
            let text = format!("{}\t{}\t{}\t{}\n", line.game, line.path, line.size, line.id);
            let _ = file.lock().unwrap_or_else(|e| e.into_inner()).write_all(text.as_bytes());
        }
        seen.insert((game.into(), path.into()), line);
    }

    /// Everything recorded, sorted by game and path.
    pub fn lines(&self) -> Vec<LogLine> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
    }
}

/// A mount layer whose reads are recorded.
pub struct Recording {
    inner: Box<dyn FileSource>,
    game: String,
    recorder: Arc<Recorder>,
}

impl Recording {
    pub fn new(inner: Box<dyn FileSource>, game: &str, recorder: Arc<Recorder>) -> Self {
        Self { inner, game: game.into(), recorder }
    }
}

impl FileSource for Recording {
    fn name(&self) -> String {
        self.inner.name()
    }

    fn entries(&self) -> Vec<Entry> {
        self.inner.entries()
    }

    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        let read = self.inner.read(path)?;
        if let Ok(bytes) = &read {
            self.recorder.record(&self.game, path, bytes);
        }
        Some(read)
    }

    /// A file a load only checks for is recorded too (read once here):
    /// served without it, the check would answer differently.
    fn contains(&self, path: &str) -> bool {
        let has = self.inner.contains(path);
        if has
            && !self.recorder.has(&self.game, path)
            && let Some(Ok(bytes)) = self.inner.read(path)
        {
            self.recorder.record(&self.game, path, &bytes);
        }
        has
    }

    fn fingerprint(&self, path: &str) -> Option<String> {
        self.inner.fingerprint(path)
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        self.inner.files_in(dir)
    }
}

/// The lines of a log file (`CHIMERA_VFS_LOG`'s), in file order;
/// malformed lines are errors.
pub fn read_log(path: &Path) -> Result<Vec<LogLine>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(n, l)| {
            let parts: Vec<&str> = l.split('\t').collect();
            match parts[..] {
                [game, file, size, id] => Ok(LogLine {
                    game: game.into(),
                    path: file.into(),
                    size: size.parse().map_err(|_| format!("{}:{}: bad size", path.display(), n + 1))?,
                    id: id.into(),
                }),
                _ => Err(format!("{}:{}: expected game, path, size, hash", path.display(), n + 1)),
            }
        })
        .collect()
}
