//! The converted-map cache (docs/plans/active/asset-service.md, section
//! 10): a map's conversion (a game's BSP or world file to Chimera's map
//! data), kept on disk per user so the next load of the same map skips
//! it; chimera_world's `map::cache` is its map side (the format's version,
//! the load options in the key). The heavy tests, `mapsweep` and the apps
//! load the same maps again and again, each test in its own process; a
//! hit reads the converted map back instead.
//!
//! - **What:** whatever a loader converts (`cached`'s `T`: CS:S keeps its
//!   `MapData` with the lighting its light field closes over, Combat Arms
//!   its world before the per-session effect library, when asked to:
//!   `CHIMERA_MAP_CACHE_CA=1`, its worlds convert about as fast as they
//!   read back), serialized with postcard (big arrays as byte strings:
//!   chimera_world's `map::pod_serde`): our own format, a local cache like the asset store,
//!   never the games' files.
//! - **Where:** `<cache dir>/chimera/maps/` (`~/.cache/chimera/maps`,
//!   `%LOCALAPPDATA%\chimera\maps`), one file per key, shared by every
//!   checkout and agent slot; `CHIMERA_MAP_CACHE=<folder>` puts it
//!   elsewhere, `CHIMERA_MAP_CACHE=0` turns it off (as does `disable`, a
//!   tool's `--no-map-cache`). `CHIMERA_MAP_CACHE_GB` caps its size
//!   (`DEFAULT_CAP_GB`): after each write the least recently used entries
//!   go first (a hit marks its entry used).
//! - **Key** (the file's name, `Key`): the loader, the converter's version
//!   (the hash of its sources the game's `build.rs` takes, with
//!   chimera_world's map format: its map types' sources and a hand-bumped
//!   number),
//!   the map's name, `MapLoadOptions` and the map file's own hash. The
//!   other files the conversion reads go in the entry: the mount tracks
//!   every path asked for, found or not, and every folder listed
//!   (`chimera_vfs::Mount::track`), and the entry keeps each one's
//!   fingerprint (an archive entry's checksum and place, a store record's
//!   id, else a BLAKE3 of the bytes: `fingerprint`). A hit checks them all
//!   again; one that differs (a patched install, a new loose file in
//!   front of an archive's) converts again and replaces the entry.
//! - **Writing:** a miss takes the key's lock file (`File::lock`, freed
//!   when the process ends), so processes missing the same map at once
//!   wait for the first one's entry instead of all converting; the entry
//!   is written to a temporary file and renamed into place, so a reader
//!   never sees half of one. An entry that doesn't read (truncated, its
//!   checksum wrong, an older format) is a miss. `CHIMERA_MAP_CACHE_LOG=1`
//!   prints where a hit's time goes.
//!
//! Known shortcuts: files a converter reads without the mount are the
//! loader's to put in its key.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime},
};

use chimera_vfs::{Access, Mount};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Where the cache is, or `0` (also `off`, `false`) for none.
pub const ENV: &str = "CHIMERA_MAP_CACHE";
/// The cache's cap in GB (decimal).
pub const CAP_ENV: &str = "CHIMERA_MAP_CACHE_GB";
/// The cap when `CAP_ENV` isn't set.
pub const DEFAULT_CAP_GB: f64 = 20.0;

const MAGIC: &[u8; 8] = b"CHIMAP\0\x01";

static DISABLED: AtomicBool = AtomicBool::new(false);

/// No cache for the rest of the process (a tool's `--no-map-cache`).
pub fn disable() {
    DISABLED.store(true, Ordering::Relaxed);
}

/// A cache folder and its cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapCache {
    pub dir: PathBuf,
    /// Bytes.
    pub cap: u64,
}

/// What a `cached` load did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No cache: converted.
    Off,
    /// Read from the cache.
    Hit,
    /// Converted and written (no entry, or one whose inputs changed or
    /// that didn't read).
    Miss,
}

impl MapCache {
    pub fn at(dir: impl Into<PathBuf>, cap: u64) -> Self {
        Self { dir: dir.into(), cap }
    }

    /// The user's cache folder for converted maps.
    pub fn default_dir() -> Option<PathBuf> {
        Some(dirs::cache_dir()?.join("chimera").join("maps"))
    }

    /// The cache the environment asks for (module docs); None when it's
    /// off.
    pub fn from_env() -> Option<Self> {
        if DISABLED.load(Ordering::Relaxed) {
            return None;
        }
        let var = std::env::var(ENV).ok();
        let cap = std::env::var(CAP_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok());
        Self::from_settings(var.as_deref(), cap)
    }

    /// `from_env` with the variables' values.
    pub fn from_settings(var: Option<&str>, cap_gb: Option<f64>) -> Option<Self> {
        let cap = (cap_gb.unwrap_or(DEFAULT_CAP_GB).max(0.0) * 1e9) as u64;
        let dir = match var.map(str::trim) {
            Some("0" | "off" | "false" | "no") => return None,
            None | Some("" | "1" | "on" | "true" | "yes") => Self::default_dir()?,
            Some(path) => PathBuf::from(path),
        };
        Some(Self { dir, cap })
    }

    #[cfg(test)]
    fn entry_path(&self, key: &Key) -> PathBuf {
        self.dir.join(format!("{}.map", key.hex()))
    }

    /// Every entry with its size and when it was last used.
    fn entries(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        rd.flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "map"))
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                Some((e.path(), m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
            })
            .collect()
    }

    /// Remove the least recently used entries until the rest fit the cap
    /// (never `keep`), and temporary files left by a process that died
    /// writing.
    pub fn evict(&self, keep: Option<&Path>) {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|e| e.1).sum();
        entries.sort_by_key(|e| e.2);
        for (path, size, _) in entries {
            if total <= self.cap {
                break;
            }
            if Some(path.as_path()) == keep {
                continue;
            }
            if fs::remove_file(&path).is_ok() {
                total -= size;
            }
        }
        let Ok(rd) = fs::read_dir(&self.dir) else { return };
        let hour_ago = SystemTime::now() - Duration::from_secs(3600);
        for e in rd.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < hour_ago);
            if old && e.path().extension().is_some_and(|x| x == "tmp") {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

/// A cache key under construction: every part is hashed in, named.
#[derive(Clone)]
pub struct Key(blake3::Hasher);

impl Key {
    /// A key of `loader` (`cs_source`), whose converter's version is
    /// `converter` (its sources' hash, from its `build.rs`).
    pub fn new(loader: &str, converter: &str) -> Self {
        Self(blake3::Hasher::new())
            .part("loader", loader)
            .part("converter", converter)
    }

    /// Add `name` = `value`.
    pub fn part(mut self, name: &str, value: impl AsRef<[u8]>) -> Self {
        let value = value.as_ref();
        self.0.update(&(name.len() as u64).to_le_bytes());
        self.0.update(name.as_bytes());
        self.0.update(&(value.len() as u64).to_le_bytes());
        self.0.update(value);
        self
    }

    /// Add `name` = the hash of `bytes` (a map file's content).
    pub fn content(self, name: &str, bytes: &[u8]) -> Self {
        self.part(name, blake3::hash(bytes).as_bytes())
    }

    pub fn hex(&self) -> String {
        self.0.finalize().to_hex().to_string()
    }
}

/// A file's content as the mount serves it now: the serving layer and
/// its fingerprint (or a hash of the bytes), else `absent`.
fn file_fingerprint(mount: &Mount, path: &str) -> String {
    match mount.layer_of(path) {
        None => "absent".into(),
        Some((i, layer)) => match layer.fingerprint(path) {
            Some(f) => format!("{i} {f}"),
            None => match layer.read(path) {
                Some(Ok(bytes)) => format!("{i} blake3 {}", blake3::hash(&bytes).to_hex()),
                Some(Err(e)) => format!("{i} unreadable {:?}", e.kind()),
                None => format!("{i} vanished"),
            },
        },
    }
}

/// What `access` finds on `mount` now (module docs, "Key").
pub fn fingerprint(mount: &Mount, access: &Access) -> String {
    match access {
        Access::File(path) => file_fingerprint(mount, path),
        Access::Every(path) => {
            let mut out = String::new();
            for (i, layer) in mount.layers().enumerate().filter(|(_, l)| l.contains(path)) {
                let f = layer.fingerprint(path).unwrap_or_else(|| match layer.read(path) {
                    Some(Ok(bytes)) => format!("blake3 {}", blake3::hash(&bytes).to_hex()),
                    _ => "unreadable".into(),
                });
                out.push_str(&format!("{i} {f};"));
            }
            out
        }
        Access::Folder(dir) => {
            let mut h = blake3::Hasher::new();
            for f in mount.files_in(dir) {
                h.update(f.as_bytes());
                h.update(b"\n");
            }
            format!("list {}", h.finalize().to_hex())
        }
    }
}

/// An `Access` as an entry keeps it (chimera_vfs stays dependency-free).
#[derive(Serialize, Deserialize, Debug, Clone)]
enum Input {
    File(String),
    Every(String),
    Folder(String),
}

impl From<Access> for Input {
    fn from(a: Access) -> Self {
        match a {
            Access::File(p) => Self::File(p),
            Access::Every(p) => Self::Every(p),
            Access::Folder(p) => Self::Folder(p),
        }
    }
}

impl From<Input> for Access {
    fn from(a: Input) -> Self {
        match a {
            Input::File(p) => Self::File(p),
            Input::Every(p) => Self::Every(p),
            Input::Folder(p) => Self::Folder(p),
        }
    }
}

/// The header before an entry's payload.
#[derive(Serialize, Deserialize, Debug)]
struct Header {
    /// The key's text again (a check that the file is the one named).
    key: String,
    /// Each input (`Input`) and its fingerprint when converted.
    inputs: Vec<(Input, String)>,
    payload_len: u64,
    payload_hash: [u8; 32],
}

/// Why an entry wasn't used.
#[derive(Debug)]
enum Unusable {
    Missing,
    /// Unreadable or not ours: deleted.
    Corrupt(String),
    /// An input differs now.
    Stale(String),
}

fn read_entry<T: DeserializeOwned>(path: &Path, key: &str, mount: &Mount) -> Result<T, Unusable> {
    let start = std::time::Instant::now();
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(Unusable::Missing),
        Err(e) => return Err(Unusable::Corrupt(e.to_string())),
    };
    let corrupt = |why: &str| Unusable::Corrupt(why.to_string());
    let rest = bytes
        .strip_prefix(MAGIC.as_slice())
        .ok_or_else(|| corrupt("not a map cache entry"))?;
    let (len, rest) = rest.split_at_checked(8).ok_or_else(|| corrupt("truncated"))?;
    let len = u64::from_le_bytes(len.try_into().expect("8 bytes")) as usize;
    let (head, payload) = rest.split_at_checked(len).ok_or_else(|| corrupt("truncated header"))?;
    let header: Header = postcard::from_bytes(head).map_err(|e| corrupt(&format!("header: {e}")))?;
    if header.key != key {
        return Err(corrupt("another key's entry"));
    }
    let read = start.elapsed().as_secs_f32();
    if header.payload_len != payload.len() as u64 || *blake3::hash(payload).as_bytes() != header.payload_hash {
        return Err(corrupt("checksum"));
    }
    let checked = start.elapsed().as_secs_f32();
    let inputs = header.inputs.len();
    for (input, was) in header.inputs {
        let access = Access::from(input);
        let now = fingerprint(mount, &access);
        if now != was {
            return Err(Unusable::Stale(format!("{access:?}: was {was}, now {now}")));
        }
    }
    let compared = start.elapsed().as_secs_f32();
    let value = postcard::from_bytes(payload).map_err(|e| corrupt(&format!("payload: {e}")))?;
    if std::env::var_os("CHIMERA_MAP_CACHE_LOG").is_some() {
        eprintln!(
            "map cache: hit {} ({:.0} MB): read {read:.2} s, checksum {:.2} s, {inputs} inputs {:.2} s, decode {:.2} s",
            path.display(),
            bytes.len() as f64 / 1e6,
            checked - read,
            compared - checked,
            start.elapsed().as_secs_f32() - compared
        );
    }
    Ok(value)
}

fn write_entry<T: Serialize>(
    cache: &MapCache,
    path: &Path,
    key: &str,
    inputs: Vec<(Input, String)>,
    value: &T,
) -> io::Result<()> {
    let payload = postcard::to_allocvec(value).map_err(io::Error::other)?;
    let header = Header {
        key: key.to_string(),
        inputs,
        payload_len: payload.len() as u64,
        payload_hash: *blake3::hash(&payload).as_bytes(),
    };
    let head = postcard::to_allocvec(&header).map_err(io::Error::other)?;
    fs::create_dir_all(&cache.dir)?;
    let tmp = cache.dir.join(format!("{key}.{}.{}.tmp", std::process::id(), unique()));
    let written = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(MAGIC)?;
        f.write_all(&(head.len() as u64).to_le_bytes())?;
        f.write_all(&head)?;
        f.write_all(&payload)?;
        // No fsync: an entry cut short by a crash fails its checksum and
        // converts again.
        drop(f);
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// A number no other call in this process gets.
fn unique() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// `convert`'s result for `key`: read back from `cache` when an entry is
/// there and every file it read from `mount` is still the same, else
/// converted (with `mount` tracked) and written. Without a cache it is
/// `convert()`. A cache that can't be written to only costs the write.
pub fn cached<T: Serialize + DeserializeOwned>(
    cache: Option<&MapCache>,
    mount: &Mount,
    key: Key,
    convert: impl FnOnce() -> Result<T, String>,
) -> Result<(T, Outcome), String> {
    let Some(cache) = cache else {
        return convert().map(|v| (v, Outcome::Off));
    };
    let key = key.hex();
    let path = cache.dir.join(format!("{key}.map"));
    let try_read = || -> Option<T> {
        match read_entry::<T>(&path, &key, mount) {
            Ok(v) => {
                // Used now: the last to go (`evict`).
                if let Ok(f) = File::options().append(true).open(&path) {
                    let _ = f.set_modified(SystemTime::now());
                }
                Some(v)
            }
            Err(Unusable::Missing) => None,
            Err(Unusable::Stale(why)) => {
                eprintln!("map cache: {} changed: {why}", path.display());
                None
            }
            Err(Unusable::Corrupt(why)) => {
                eprintln!("map cache: {}: {why}; converting again", path.display());
                let _ = fs::remove_file(&path);
                None
            }
        }
    };
    if let Some(v) = try_read() {
        return Ok((v, Outcome::Hit));
    }
    let stamp = || fs::metadata(&path).and_then(|m| m.modified()).ok();
    let before = stamp();
    // Another process converting the same map: wait for its entry (read
    // again only if one was written meanwhile).
    let lock = fs::create_dir_all(&cache.dir)
        .and_then(|_| {
            File::options()
                .create(true)
                .truncate(false)
                .write(true)
                .open(cache.dir.join(format!("{key}.lock")))
        })
        .ok();
    if lock.as_ref().is_some_and(|f| f.lock().is_ok())
        && stamp().is_some_and(|t| Some(t) != before)
        && let Some(v) = try_read()
    {
        return Ok((v, Outcome::Hit));
    }
    let tracking = mount.track();
    let value = convert()?;
    let accesses = tracking.finish();
    let inputs = inputs(mount, accesses);
    if let Err(e) = write_entry(cache, &path, &key, inputs, &value) {
        eprintln!("map cache: {}: {e}", path.display());
    }
    drop(lock);
    cache.evict(Some(&path));
    Ok((value, Outcome::Miss))
}

fn inputs(mount: &Mount, accesses: BTreeSet<Access>) -> Vec<(Input, String)> {
    accesses
        .into_iter()
        .map(|a| {
            let f = fingerprint(mount, &a);
            (a.into(), f)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_vfs::LooseDir;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("chimera_map_cache_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A "conversion" that reads `a.txt` and checks for `b.txt`.
    fn convert(mount: &Mount, runs: &mut u32) -> Result<String, String> {
        *runs += 1;
        let a = String::from_utf8(mount.read("a.txt").map_err(|e| e.to_string())?).unwrap();
        Ok(format!("{a}{}", mount.contains("b.txt")))
    }

    #[test]
    fn the_settings_pick_the_folder_or_none() {
        assert_eq!(MapCache::from_settings(Some("0"), None), None);
        assert_eq!(MapCache::from_settings(Some(" off "), None), None);
        let at = MapCache::from_settings(Some("/x/maps"), Some(1.5)).unwrap();
        assert_eq!((at.dir, at.cap), (PathBuf::from("/x/maps"), 1_500_000_000));
        if let Some(d) = MapCache::default_dir() {
            assert_eq!(MapCache::from_settings(None, None).map(|c| c.dir), Some(d.clone()));
            assert_eq!(MapCache::from_settings(Some("1"), None).map(|c| c.dir), Some(d));
        }
    }

    #[test]
    fn a_key_changes_with_each_part() {
        let k = || Key::new("game", "v1").part("map", "de_x");
        assert_eq!(k().hex(), k().hex());
        assert_ne!(k().hex(), Key::new("game", "v2").part("map", "de_x").hex());
        assert_ne!(k().hex(), Key::new("other", "v1").part("map", "de_x").hex());
        assert_ne!(k().hex(), k().part("file", [1u8]).hex());
        assert_ne!(k().hex(), Key::new("game", "v1").part("map", "de_y").hex());
        // Parts are delimited: ("ab", "c") isn't ("a", "bc").
        assert_ne!(
            Key::new("g", "v").part("ab", "c").hex(),
            Key::new("g", "v").part("a", "bc").hex()
        );
    }

    #[test]
    fn a_second_load_hits_and_a_changed_input_converts_again() {
        let root = temp("inputs");
        let files = root.join("files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("a.txt"), "one").unwrap();
        let mut mount = Mount::default();
        mount.push(LooseDir::new(&files));
        let cache = MapCache::at(root.join("cache"), u64::MAX);
        let key = || Key::new("test", "v1").part("map", "m");
        let mut runs = 0;
        let (v, o) = cached(Some(&cache), &mount, key(), || convert(&mount, &mut runs)).unwrap();
        assert_eq!((v.as_str(), o, runs), ("onefalse", Outcome::Miss, 1));
        let (v, o) = cached(Some(&cache), &mount, key(), || convert(&mount, &mut runs)).unwrap();
        assert_eq!(
            (v.as_str(), o, runs),
            ("onefalse", Outcome::Hit, 1),
            "a hit doesn't convert"
        );
        // A changed file the conversion read.
        fs::write(files.join("a.txt"), "two").unwrap();
        let (v, o) = cached(Some(&cache), &mount, key(), || convert(&mount, &mut runs)).unwrap();
        assert_eq!((v.as_str(), o, runs), ("twofalse", Outcome::Miss, 2));
        // A file it looked for and didn't find appears.
        fs::write(files.join("b.txt"), "").unwrap();
        let (v, o) = cached(Some(&cache), &mount, key(), || convert(&mount, &mut runs)).unwrap();
        assert_eq!((v.as_str(), o, runs), ("twotrue", Outcome::Miss, 3));
        let (_, o) = cached(Some(&cache), &mount, key(), || convert(&mount, &mut runs)).unwrap();
        assert_eq!((o, runs), (Outcome::Hit, 3));
        // Another converter version (or format) is another key.
        let (_, o) = cached(Some(&cache), &mount, Key::new("test", "v2").part("map", "m"), || {
            convert(&mount, &mut runs)
        })
        .unwrap();
        assert_eq!((o, runs), (Outcome::Miss, 4));
        // No cache: always converts.
        let (_, o) = cached(None, &mount, key(), || convert(&mount, &mut runs)).unwrap();
        assert_eq!((o, runs), (Outcome::Off, 5));
        // A failed conversion writes nothing.
        let e = cached::<String>(Some(&cache), &mount, Key::new("test", "fails"), || Err("no".into()));
        assert_eq!(e.unwrap_err(), "no");
        assert!(!cache.entry_path(&Key::new("test", "fails")).exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_corrupt_entry_converts_again_and_is_replaced() {
        let root = temp("corrupt");
        let files = root.join("files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("a.txt"), "one").unwrap();
        let mut mount = Mount::default();
        mount.push(LooseDir::new(&files));
        let cache = MapCache::at(root.join("cache"), u64::MAX);
        let key = || Key::new("test", "v1");
        let mut runs = 0;
        cached(Some(&cache), &mount, key(), || convert(&mount, &mut runs)).unwrap();
        let path = cache.entry_path(&key());
        let good = fs::read(&path).unwrap();
        // A flipped byte in the payload, a truncated file, an empty one,
        // someone else's file.
        let mut flipped = good.clone();
        *flipped.last_mut().unwrap() ^= 0x55;
        for bad in [flipped, good[..good.len() - 3].to_vec(), Vec::new(), b"hello".to_vec()] {
            fs::write(&path, &bad).unwrap();
            let before = runs;
            let (v, o) = cached(Some(&cache), &mount, key(), || convert(&mount, &mut runs)).unwrap();
            assert_eq!((v.as_str(), o, runs), ("onefalse", Outcome::Miss, before + 1));
            assert_eq!(fs::read(&path).unwrap(), good, "rewritten whole");
        }
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_least_recently_used_entries_go_over_the_cap() {
        let root = temp("evict");
        let cache = MapCache::at(&root, 250);
        let now = SystemTime::now();
        for (i, name) in ["a", "b", "c"].iter().enumerate() {
            let p = root.join(format!("{name}.map"));
            fs::write(&p, [0u8; 100]).unwrap();
            let f = File::options().append(true).open(&p).unwrap();
            f.set_modified(now - Duration::from_secs(100 - i as u64 * 10)).unwrap();
        }
        fs::write(root.join("x.1.2.tmp"), b"").unwrap();
        cache.evict(Some(&root.join("a.map")));
        let mut left: Vec<String> = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        // a is the oldest but kept (just written); b goes, the next oldest.
        assert_eq!(left, ["a.map", "c.map", "x.1.2.tmp"]);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn processes_missing_one_key_at_once_convert_it_once() {
        let root = temp("concurrent");
        let files = root.join("files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("a.txt"), "one").unwrap();
        let mut mount = Mount::default();
        mount.push(LooseDir::new(&files));
        let cache = MapCache::at(root.join("cache"), u64::MAX);
        let runs = std::sync::atomic::AtomicU32::new(0);
        let outcomes: Vec<Outcome> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    s.spawn(|| {
                        cached(Some(&cache), &mount, Key::new("test", "v1"), || {
                            runs.fetch_add(1, Ordering::Relaxed);
                            std::thread::sleep(Duration::from_millis(300));
                            Ok(7u32)
                        })
                        .unwrap()
                        .1
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(runs.load(Ordering::Relaxed), 1, "{outcomes:?}");
        assert_eq!(outcomes.iter().filter(|o| **o == Outcome::Hit).count(), 3);
        fs::remove_dir_all(&root).ok();
    }
}
