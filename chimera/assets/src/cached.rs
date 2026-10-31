//! `CachedFiles`: a manifest and its store as a read-only mount layer.
//! `entries`, `contains` and `files_in` answer from the manifest without
//! touching records; `read` decompresses the record and checks its size
//! and SHA-256 (a corrupt record is `InvalidData`, never handed to a
//! loader). A record the store lacks is asked of the fetcher, when there
//! is one (the asset service, `remote`), with its group in one batch: the
//! records the store lacks of the smallest set holding the file (the
//! files after it in the set first, up to `GROUP_BYTES` and `GROUP_FILES`
//! a batch), else of the files beside it sharing its name before the
//! first dot (a model's `.mdl`, `.vvd`, `.vtx`, `.phy`); each is checked
//! before it is kept. One group is fetched at a time per manifest, so
//! loads that miss files of the same group at once wait for one batch.
//! `CHIMERA_ASSET_BATCH=0` fetches only the file asked for (measuring,
//! debugging). Without a fetcher, or when it fails, the read is an error
//! that names the path (`asset miss`), so a set missing a file shows at
//! once.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use chimera_vfs::{Entry, FileSource};

use crate::{Manifest, Store, manifest::FileRecord, remote::Fetcher};

/// Most stored bytes one on-demand batch asks for (the miss that caused it
/// waits for the whole batch).
pub const GROUP_BYTES: u64 = 32_000_000;
/// Most records one on-demand batch asks for.
pub const GROUP_FILES: usize = 1024;
/// `0` turns batching off: an on-demand fetch is the one record.
pub const BATCH_ENV: &str = "CHIMERA_ASSET_BATCH";

/// A manifest indexed for lookups (shared between mounts of it).
#[derive(Debug)]
pub struct Index {
    pub game: String,
    pub build: String,
    /// The Steam app the build is of, when the manifest says.
    pub app: Option<u32>,
    /// The manifest lists every file the game mounts (a whole publish),
    /// not only some sets.
    pub complete: bool,
    files: HashMap<String, FileRecord>,
    /// Folder -> the files directly in it (normalized paths).
    dirs: HashMap<String, Vec<String>>,
    /// Each set's paths (sorted) and stored bytes.
    sets: Vec<(String, Vec<String>, u64)>,
    /// Path -> the sets holding it (indexes into `sets`).
    in_sets: HashMap<String, Vec<u32>>,
    /// Records seen in the store (not looked for again).
    present: Mutex<HashSet<String>>,
    /// Held while a group is fetched.
    fetching: Mutex<()>,
}

impl Index {
    pub fn new(manifest: &Manifest) -> Self {
        let mut dirs: HashMap<String, Vec<String>> = HashMap::new();
        for path in manifest.files.keys() {
            let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
            dirs.entry(dir.to_string()).or_default().push(path.clone());
        }
        let mut in_sets: HashMap<String, Vec<u32>> = HashMap::new();
        let sets = manifest
            .sets
            .iter()
            .enumerate()
            .map(|(i, (name, paths))| {
                for p in paths {
                    in_sets.entry(p.clone()).or_default().push(i as u32);
                }
                (name.clone(), paths.iter().cloned().collect(), manifest.set_size(name).2)
            })
            .collect();
        Self {
            game: manifest.game.clone(),
            build: manifest.build.clone(),
            app: manifest.app,
            complete: manifest.complete,
            files: manifest.files.iter().map(|(p, f)| (p.clone(), f.clone())).collect(),
            dirs,
            sets,
            in_sets,
            present: Mutex::new(HashSet::new()),
            fetching: Mutex::new(()),
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Whether `store` has record `id` (remembered once seen).
    fn has(&self, store: &Store, id: &str) -> bool {
        let mut present = self.present.lock().unwrap_or_else(|e| e.into_inner());
        if present.contains(id) {
            return true;
        }
        let has = store.has(id);
        if has {
            present.insert(id.to_string());
        }
        has
    }

    /// The records to fetch with `path`'s (which `store` lacks): `path`
    /// first, then the missing records of the smallest set holding it
    /// (from `path` on, wrapping), else of its companions (same folder,
    /// same name before the first dot); at most `GROUP_FILES` records and
    /// about `GROUP_BYTES` stored.
    pub fn group(&self, store: &Store, path: &str) -> Vec<(String, u64)> {
        let Some(first) = self.files.get(path) else {
            return Vec::new();
        };
        let mut out = vec![(first.id.clone(), first.size)];
        let mut seen: HashSet<&str> = HashSet::from([first.id.as_str()]);
        let mut bytes = first.stored;
        let set = self
            .in_sets
            .get(path)
            .and_then(|sets| sets.iter().map(|i| &self.sets[*i as usize]).min_by_key(|s| (s.2, s.1.len())));
        let others: Vec<&String> = match set {
            Some((_, paths, _)) => {
                let at = paths.binary_search_by(|p| p.as_str().cmp(path)).unwrap_or_else(|i| i);
                paths[at..].iter().chain(&paths[..at]).collect()
            }
            None => {
                let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
                let stem = name.split('.').next().unwrap_or(name);
                let mut companions: Vec<&String> = self
                    .dirs
                    .get(dir)
                    .into_iter()
                    .flatten()
                    .filter(|p| {
                        let n = p.rsplit_once('/').map_or(p.as_str(), |(_, n)| n);
                        n.split('.').next() == Some(stem)
                    })
                    .collect();
                companions.sort();
                companions
            }
        };
        for p in others {
            if out.len() >= GROUP_FILES || bytes >= GROUP_BYTES {
                break;
            }
            let Some(f) = self.files.get(p.as_str()) else { continue };
            if !seen.insert(f.id.as_str()) || self.has(store, &f.id) {
                continue;
            }
            out.push((f.id.clone(), f.size));
            bytes += f.stored;
        }
        out
    }
}

/// Indexes already built, by manifest file and its modification time
/// (each mount of a game would otherwise parse its manifest again).
static INDEXES: Mutex<BTreeMap<std::path::PathBuf, (Option<SystemTime>, Arc<Index>)>> = Mutex::new(BTreeMap::new());

pub struct CachedFiles {
    store: Store,
    index: Arc<Index>,
    fetcher: Option<Arc<dyn Fetcher>>,
}

/// Whether on-demand fetches take the missing file's group (`BATCH_ENV`).
fn batching() -> bool {
    std::env::var(BATCH_ENV).map_or(true, |v| !matches!(v.trim(), "0" | "false" | "off" | "no"))
}

impl CachedFiles {
    pub fn new(store: Store, manifest: &Manifest) -> Self {
        Self { store, index: Arc::new(Index::new(manifest)), fetcher: None }
    }

    /// Records the store lacks are fetched with `fetcher` (when Some).
    pub fn with_fetcher(mut self, fetcher: Option<Arc<dyn Fetcher>>) -> Self {
        self.fetcher = fetcher;
        self
    }

    /// `game`'s manifest at `build` in `store`, indexed once per process
    /// (again when the file changes).
    pub fn open(store: &Store, game: &str, build: &str) -> Result<Self, String> {
        let path = store.manifest_path(game, build);
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let mut cache = INDEXES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((when, index)) = cache.get(&path)
            && *when == modified
            && modified.is_some()
        {
            return Ok(Self { store: store.clone(), index: index.clone(), fetcher: None });
        }
        let index = Arc::new(Index::new(&Manifest::load(&path)?));
        cache.insert(path, (modified, index.clone()));
        Ok(Self { store: store.clone(), index, fetcher: None })
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    /// `path`'s record `f` from the fetcher: its group in one batch (one
    /// group at a time), then the record alone if the batch didn't bring
    /// it.
    fn fetch(&self, fetcher: &dyn Fetcher, path: &str, f: &FileRecord) -> io::Result<Vec<u8>> {
        let _one_at_a_time = self.index.fetching.lock().unwrap_or_else(|e| e.into_inner());
        // Another load may have fetched it while this one waited.
        match self.store.get(&f.id, f.size) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            got => return got,
        }
        if batching() {
            let group = self.index.group(&self.store, path);
            if group.len() > 1 {
                let _ = fetcher.fetch_many(&self.store, &group);
                match self.store.get(&f.id, f.size) {
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    got => return got,
                }
            }
        }
        fetcher.fetch(&self.store, &f.id, f.size)
    }
}

impl FileSource for CachedFiles {
    fn name(&self) -> String {
        format!("{} (assets {} build {})", self.store.root.display(), self.index.game, self.index.build)
    }

    fn entries(&self) -> Vec<Entry> {
        self.index.files.iter().map(|(p, f)| Entry { path: p.clone(), size: f.size }).collect()
    }

    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        let f = self.index.files.get(path)?;
        let got = match (self.store.get(&f.id, f.size), &self.fetcher) {
            (Err(e), Some(fetcher)) if e.kind() == io::ErrorKind::NotFound => self.fetch(fetcher.as_ref(), path, f),
            (got, _) => got,
        };
        Some(got.map_err(|e| {
            let e = io::Error::new(e.kind(), format!("{}: {path}: {e}", self.index.game));
            if e.kind() == io::ErrorKind::InvalidData {
                eprintln!("asset refused {e}");
            } else {
                eprintln!("asset miss {e}");
            }
            e
        }))
    }

    fn contains(&self, path: &str) -> bool {
        self.index.files.contains_key(path)
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        self.index.dirs.get(dir).cloned().unwrap_or_default()
    }
}
