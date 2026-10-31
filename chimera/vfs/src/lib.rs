//! A read-only virtual filesystem over a game's archives and loose files:
//! `FileSource` (an archive such as `source_formats`' `vpk::Vpk` or
//! `lithtech_formats`' `.rez`, or a `LooseDir`), `Mount` (layers searched in
//! order, like a game's search path) and path normalization. Neutral and
//! dependency-free: it sits under both format crates and Chimera's `mount`.
//!
//! What a load asks of a mount can be tracked (`Mount::track`: every path
//! read or checked, every folder listed, found or not), and a layer can
//! name a file's content cheaply (`FileSource::fingerprint`): together the
//! converted-map cache's inputs (chimera_world's `map::cache`), checked
//! on a hit without reading the files again.

pub mod bytes;
pub mod source_hash;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// One file visible through a mount.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Normalized path: lowercase, `/` separators, no leading slash.
    pub path: String,
    pub size: u64,
}

/// A source of files: an archive or a loose directory.
pub trait FileSource: Send + Sync {
    /// Human-readable name for listings and errors.
    fn name(&self) -> String;
    fn entries(&self) -> Vec<Entry>;
    /// `None` if this source doesn't have `path` (already normalized).
    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>>;
    /// Whether this source has `path` (already normalized): `read` would
    /// give `Some`. Sources override it to answer without reading.
    fn contains(&self, path: &str) -> bool {
        self.read(path).is_some()
    }
    /// The files directly in folder `dir` (normalized, no trailing slash),
    /// as normalized paths. Sources override it to avoid listing
    /// everything.
    fn files_in(&self, dir: &str) -> Vec<String> {
        self.entries()
            .into_iter()
            .filter(|e| e.path.rsplit_once('/').is_some_and(|(d, _)| d == dir))
            .map(|e| e.path)
            .collect()
    }
    /// A cheap name for the content of `path` (already normalized) that
    /// changes when the content does, without reading it: an archive
    /// entry's checksum and size, a store record's id. `None` (the
    /// default): the reader hashes the bytes instead. Only compared with
    /// the same layer's own fingerprints.
    fn fingerprint(&self, _path: &str) -> Option<String> {
        None
    }
}

/// What a load asked of a mount (`Mount::track`), by normalized path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Access {
    /// A file read or checked for (`read`, `contains`): found or not.
    File(String),
    /// A file read from every layer that has it (`read_every`).
    Every(String),
    /// A folder listed (`files_in`).
    Folder(String),
}

type Log = Arc<Mutex<BTreeSet<Access>>>;

/// A mount's accesses being collected (`Mount::track`); `finish` returns
/// them. Dropping it stops the collection.
pub struct Tracking<'a> {
    mount: &'a Mount,
    log: Log,
}

impl Tracking<'_> {
    /// Everything asked since `track`, sorted.
    pub fn finish(self) -> BTreeSet<Access> {
        std::mem::take(&mut *self.log.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl Drop for Tracking<'_> {
    fn drop(&mut self) {
        let mut logs = self.mount.logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.retain(|l| !Arc::ptr_eq(l, &self.log));
        self.mount.tracking.store(logs.len(), Ordering::Release);
    }
}

/// Normalize a path the way Source and LithTech resolve them:
/// case-insensitive, either slash.
pub fn normalize(path: &str) -> String {
    // Doubled separators occur in game data (e.g. a model's texture
    // directory `models\props_junk\\`); file systems ignore them, so do we.
    // So are `.` components (static prop names like `./models/...` on
    // de_nuke and de_train).
    let path = path.replace('\\', "/").to_lowercase();
    path.split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Layers searched in order; the first layer that has a file wins, like a
/// game's search path.
#[derive(Default)]
pub struct Mount {
    layers: Vec<Box<dyn FileSource>>,
    /// Open `Tracking`s (several loads may track one mount at once; each
    /// sees the others' accesses too, which only makes it stricter).
    logs: Mutex<Vec<Log>>,
    tracking: AtomicUsize,
}

impl Mount {
    pub fn push(&mut self, layer: impl FileSource + 'static) {
        self.layers.push(Box::new(layer));
    }

    /// Add a layer already boxed (one taken out by `into_layers`).
    pub fn push_boxed(&mut self, layer: Box<dyn FileSource>) {
        self.layers.push(layer);
    }

    pub fn layers(&self) -> impl Iterator<Item = &dyn FileSource> {
        self.layers.iter().map(|l| l.as_ref())
    }

    /// The layers, in search order, to wrap or rearrange.
    pub fn into_layers(self) -> Vec<Box<dyn FileSource>> {
        self.layers
    }

    /// Each layer replaced by `f(layer)`, in the same order (e.g. wrapped
    /// in a recorder of what is read).
    pub fn map_layers(self, f: impl FnMut(Box<dyn FileSource>) -> Box<dyn FileSource>) -> Mount {
        Mount { layers: self.layers.into_iter().map(f).collect(), ..Default::default() }
    }

    /// Start collecting what is asked of this mount (`Tracking::finish`).
    pub fn track(&self) -> Tracking<'_> {
        let log = Log::default();
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.push(log.clone());
        self.tracking.store(logs.len(), Ordering::Release);
        Tracking { mount: self, log }
    }

    fn note(&self, access: impl FnOnce() -> Access) {
        if self.tracking.load(Ordering::Acquire) == 0 {
            return;
        }
        let access = access();
        for log in self.logs.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            log.lock().unwrap_or_else(|e| e.into_inner()).insert(access.clone());
        }
    }

    /// The layer that serves `path` (normalized): its index and itself.
    pub fn layer_of(&self, path: &str) -> Option<(usize, &dyn FileSource)> {
        self.layers
            .iter()
            .enumerate()
            .find(|(_, l)| l.contains(path))
            .map(|(i, l)| (i, l.as_ref()))
    }

    /// `path` from every layer that has it, in search order (files a game
    /// merges along its search path, such as localization).
    pub fn read_every(&self, path: &str) -> Vec<Vec<u8>> {
        let path = normalize(path);
        self.note(|| Access::Every(path.clone()));
        self.layers.iter().filter_map(|l| l.read(&path)?.ok()).collect()
    }

    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let path = normalize(path);
        self.note(|| Access::File(path.clone()));
        self.layers
            .iter()
            .find_map(|l| l.read(&path))
            .unwrap_or_else(|| Err(io::Error::new(io::ErrorKind::NotFound, format!("not in mount: {path}"))))
    }

    /// Whether a layer has `path` (`read` finds it, though reading may
    /// still fail), without reading it.
    pub fn contains(&self, path: &str) -> bool {
        let path = normalize(path);
        self.note(|| Access::File(path.clone()));
        self.layers.iter().any(|l| l.contains(&path))
    }

    /// The files directly in folder `dir` in any layer, sorted by path
    /// (one listing of a folder, e.g. CS:S's spray logos, without listing
    /// the whole install).
    pub fn files_in(&self, dir: &str) -> Vec<String> {
        let dir = normalize(dir);
        self.note(|| Access::Folder(dir.clone()));
        let mut seen = std::collections::BTreeSet::new();
        for layer in &self.layers {
            seen.extend(layer.files_in(&dir));
        }
        seen.into_iter().collect()
    }

    /// Every visible file, sorted by path, with the layer it comes from.
    /// Files shadowed by an earlier layer are omitted.
    pub fn entries(&self) -> Vec<(Entry, usize)> {
        let mut seen = BTreeMap::new();
        for (i, layer) in self.layers.iter().enumerate() {
            for e in layer.entries() {
                seen.entry(e.path.clone()).or_insert((e, i));
            }
        }
        seen.into_values().collect()
    }
}

/// Loose files under a directory.
pub struct LooseDir {
    root: PathBuf,
    /// Lowercase extensions to hide, e.g. archives that are mounted as their
    /// own layers.
    hidden_exts: Vec<&'static str>,
}

impl LooseDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            hidden_exts: Vec::new(),
        }
    }

    pub fn hiding(mut self, exts: &[&'static str]) -> Self {
        self.hidden_exts.extend_from_slice(exts);
        self
    }

    fn hidden(&self, path: &str) -> bool {
        path.rsplit_once('.')
            .is_some_and(|(_, ext)| self.hidden_exts.contains(&ext))
    }

    fn walk(dir: &Path, root: &Path, out: &mut Vec<Entry>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for entry in rd.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                Self::walk(&path, root, out);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(Entry {
                    path: normalize(&rel.to_string_lossy()),
                    size: meta.len(),
                });
            }
        }
    }
}

impl FileSource for LooseDir {
    fn name(&self) -> String {
        self.root.display().to_string()
    }

    fn entries(&self) -> Vec<Entry> {
        let mut out = Vec::new();
        Self::walk(&self.root, &self.root, &mut out);
        out.retain(|e| !self.hidden(&e.path));
        out
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        // The folder, matched case-insensitively as `read` does.
        let mut at = self.root.clone();
        for part in dir.split('/').filter(|p| !p.is_empty()) {
            let direct = at.join(part);
            if direct.is_dir() {
                at = direct;
                continue;
            }
            let Some(found) = fs::read_dir(&at).ok().and_then(|rd| {
                rd.flatten()
                    .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))
            }) else {
                return Vec::new();
            };
            at = found.path();
        }
        let Ok(rd) = fs::read_dir(&at) else { return Vec::new() };
        rd.flatten()
            .filter(|e| e.metadata().is_ok_and(|m| m.is_file()))
            .map(|e| normalize(&format!("{dir}/{}", e.file_name().to_string_lossy())))
            .filter(|p| !self.hidden(p))
            .collect()
    }

    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        self.locate(path).map(fs::read)
    }

    fn contains(&self, path: &str) -> bool {
        self.locate(path).is_some()
    }
}

impl LooseDir {
    /// The file at `path` (normalized) on disk, if it is there and not
    /// hidden.
    fn locate(&self, path: &str) -> Option<PathBuf> {
        if self.hidden(path) {
            return None;
        }
        // Fast path: exact case. Otherwise match case-insensitively.
        let direct = self.root.join(path);
        if direct.is_file() {
            return Some(direct);
        }
        let mut dir = self.root.clone();
        for part in path.split('/') {
            let found = fs::read_dir(&dir)
                .ok()?
                .flatten()
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
            dir = found.path();
        }
        dir.is_file().then_some(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_in_lists_one_folder_of_every_layer() {
        let root = std::env::temp_dir().join(format!("chimera_files_in_{}", std::process::id()));
        let logos = root.join("Materials").join("VGUI").join("logos");
        fs::create_dir_all(logos.join("ui")).unwrap();
        fs::write(logos.join("Spray_A.vtf"), b"a").unwrap();
        fs::write(logos.join("ui").join("spray_a.vmt"), b"b").unwrap();
        let mut mount = Mount::default();
        mount.push(LooseDir::new(&root));
        assert_eq!(
            mount.files_in("materials/vgui/logos"),
            vec!["materials/vgui/logos/spray_a.vtf"]
        );
        assert!(mount.files_in("materials/none").is_empty());
        assert!(mount.contains("MATERIALS/vgui/logos/spray_a.vtf"));
        assert!(!mount.contains("materials/vgui/logos/spray_b.vtf"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn tracking_collects_what_is_asked_found_or_not() {
        let root = std::env::temp_dir().join(format!("chimera_track_{}", std::process::id()));
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a").join("x.txt"), b"x").unwrap();
        let mut mount = Mount::default();
        mount.push(LooseDir::new(&root));
        mount.read("before").ok();
        let t = mount.track();
        mount.read("A/x.txt").unwrap();
        assert!(!mount.contains("a/missing"));
        mount.files_in("a");
        assert_eq!(mount.read_every("a/x.txt"), vec![b"x".to_vec()]);
        let got: Vec<Access> = t.finish().into_iter().collect();
        assert_eq!(
            got,
            vec![
                Access::File("a/missing".into()),
                Access::File("a/x.txt".into()),
                Access::Every("a/x.txt".into()),
                Access::Folder("a".into()),
            ]
        );
        assert_eq!(mount.tracking.load(Ordering::Acquire), 0, "dropped with finish");
        assert_eq!(mount.layer_of("a/x.txt").map(|(i, _)| i), Some(0));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn normalize_drops_empty_and_dot_components() {
        assert_eq!(super::normalize("./Models\\props//a.MDL"), "models/props/a.mdl");
    }
}
