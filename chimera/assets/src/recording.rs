//! What a load reads, recorded: `Recording` wraps a mount layer and tells a
//! `Recorder` each file read from it, once per game and path per process,
//! with its size and SHA-256, and apart from those each file a load only
//! checks for (`contains`, found), which it never reads: on a served
//! mount a read would download the file. With `CHIMERA_VFS_LOG=<file>`
//! every game mount does this (Chimera's `mount::served`) and the
//! recorder appends one line per file to that file: a read is
//! `<game>\t<path>\t<size>\t<sha256>`, a check `<game>\t<path>\tcheck`.
//! Several processes may append to one log (a test run's binaries): each
//! line is one write. A set for `chimera_pack` is the read lines of one
//! game (`read_log`); checks never put a file in a set (a served mount
//! answers them from its manifest), and `read_log_with_checks` gives them
//! to the tools that sort files by use (`chimera_pack lean`).

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

/// What a check line has in place of the size and hash.
pub const CHECK: &str = "check";

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
    /// (game, path) of the files only checked for (found), not read.
    checked: Mutex<std::collections::BTreeSet<(String, String)>>,
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

    /// Note that a load checked for `game`'s `path` and found it, without
    /// reading it (once per game and path; a file also read has its read
    /// line besides).
    pub fn check(&self, game: &str, path: &str) {
        let mut checked = self.checked.lock().unwrap_or_else(|e| e.into_inner());
        if !checked.insert((game.into(), path.into())) {
            return;
        }
        if let Some(file) = &self.file {
            let text = format!("{game}\t{path}\t{CHECK}\n");
            let _ = file.lock().unwrap_or_else(|e| e.into_inner()).write_all(text.as_bytes());
        }
    }

    /// Everything read, sorted by game and path.
    pub fn lines(&self) -> Vec<LogLine> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
    }

    /// The (game, path) of every file only checked for, sorted.
    pub fn checks(&self) -> Vec<(String, String)> {
        self.checked.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
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

    /// A file a load only checks for is recorded as a check, never read:
    /// a served mount answers the check from its manifest (a read would
    /// download the file), and a check alone never puts a file in a set.
    fn contains(&self, path: &str) -> bool {
        let has = self.inner.contains(path);
        if has {
            self.recorder.check(&self.game, path);
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

/// The read lines of a log file (`CHIMERA_VFS_LOG`'s), in file order
/// (check lines left out); malformed lines are errors.
pub fn read_log(path: &Path) -> Result<Vec<LogLine>, String> {
    read_log_with_checks(path).map(|(reads, _)| reads)
}

/// A log file's read lines and, apart, the (game, path) of its check
/// lines, each in file order; malformed lines are errors.
pub fn read_log_with_checks(path: &Path) -> Result<(Vec<LogLine>, Vec<(String, String)>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_log(&text).map_err(|(n, e)| format!("{}:{n}: {e}", path.display()))
}

/// `read_log_with_checks` of a log's text; Err: (line number, what).
pub fn parse_log(text: &str) -> Result<(Vec<LogLine>, Vec<(String, String)>), (usize, String)> {
    let (mut reads, mut checks) = (Vec::new(), Vec::new());
    for (n, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let parts: Vec<&str> = l.split('\t').collect();
        match parts[..] {
            [game, file, CHECK] => checks.push((game.to_string(), file.to_string())),
            [game, file, size, id] => reads.push(LogLine {
                game: game.into(),
                path: file.into(),
                size: size.parse().map_err(|_| (n + 1, "bad size".to_string()))?,
                id: id.into(),
            }),
            _ => return Err((n + 1, "expected game, path, size, hash (or game, path, check)".into())),
        }
    }
    Ok((reads, checks))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layer that counts its reads.
    struct Counting(Arc<std::sync::atomic::AtomicUsize>);

    impl FileSource for Counting {
        fn name(&self) -> String {
            "counting".into()
        }

        fn entries(&self) -> Vec<Entry> {
            Vec::new()
        }

        fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            (path != "absent.txt").then(|| Ok(path.as_bytes().to_vec()))
        }

        fn contains(&self, path: &str) -> bool {
            path != "absent.txt"
        }
    }

    /// A check is logged apart from reads and reads nothing (on a served
    /// mount a read downloads); the log's reader keeps checks out of the
    /// reads a set is made from.
    #[test]
    fn a_check_is_logged_apart_and_never_reads_the_file() {
        let dir = std::env::temp_dir().join(format!("chimera_recording_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let log = dir.join("vfs.log");
        let recorder = Recorder::to_file(&log).unwrap();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let layer = Recording::new(Box::new(Counting(count.clone())), "g", recorder.clone());
        let reads = |_: &Recording| count.load(std::sync::atomic::Ordering::Relaxed);
        assert!(layer.contains("models/body.ltb"));
        assert!(!layer.contains("absent.txt"));
        assert_eq!(reads(&layer), 0, "a check reads nothing");
        assert!(layer.read("ui/font.ttf").unwrap().is_ok());
        assert!(layer.contains("ui/font.ttf"));
        assert_eq!(reads(&layer), 1);
        let (lines, checks) = read_log_with_checks(&log).unwrap();
        assert_eq!(lines.iter().map(|l| l.path.as_str()).collect::<Vec<_>>(), ["ui/font.ttf"]);
        assert_eq!(checks, [("g".to_string(), "models/body.ltb".to_string()), ("g".into(), "ui/font.ttf".into())]);
        assert_eq!(read_log(&log).unwrap().len(), 1, "checks never reach a set");
        assert_eq!(recorder.checks().len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
