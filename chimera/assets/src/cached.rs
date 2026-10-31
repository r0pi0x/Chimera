//! `CachedFiles`: a manifest and its store as a read-only mount layer.
//! `entries`, `contains` and `files_in` answer from the manifest without
//! touching records; `read` decompresses the record and checks its size
//! and SHA-256 (a corrupt record is `InvalidData`, never handed to a
//! loader). A record the store lacks is asked of the fetcher, when there
//! is one (the asset service, `remote`; on demand, one request per
//! record, checked before it is kept); without one, or when it fails, the
//! read is an error that names the path (`asset miss`), so a set missing
//! a file shows at once.

use std::{
    collections::{BTreeMap, HashMap},
    io,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use chimera_vfs::{Entry, FileSource};

use crate::{Manifest, Store, manifest::FileRecord, remote::Fetcher};

/// A manifest indexed for lookups (shared between mounts of it).
#[derive(Debug)]
pub struct Index {
    pub game: String,
    pub build: String,
    /// The Steam app the build is of, when the manifest says.
    pub app: Option<u32>,
    files: HashMap<String, FileRecord>,
    /// Folder -> the files directly in it (normalized paths).
    dirs: HashMap<String, Vec<String>>,
}

impl Index {
    pub fn new(manifest: &Manifest) -> Self {
        let mut dirs: HashMap<String, Vec<String>> = HashMap::new();
        for path in manifest.files.keys() {
            let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
            dirs.entry(dir.to_string()).or_default().push(path.clone());
        }
        Self {
            game: manifest.game.clone(),
            build: manifest.build.clone(),
            app: manifest.app,
            files: manifest.files.iter().map(|(p, f)| (p.clone(), f.clone())).collect(),
            dirs,
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
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
        let got = match self.store.get(&f.id, f.size) {
            Err(e) if e.kind() == io::ErrorKind::NotFound && self.fetcher.is_some() => {
                self.fetcher.as_ref().map_or(Err(e), |fetcher| fetcher.fetch(&self.store, &f.id, f.size))
            }
            got => got,
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
