//! The converted-map cache (docs/plans/active/asset-service.md, section
//! 10): a map's conversion (a game's BSP or world file to Chimera's map
//! data), kept on disk per user so the next load of the same map skips
//! it; chimera_world's `map::cache` is its map side (the format's version,
//! the load options in the key). The heavy tests, `mapsweep` and the apps
//! load the same maps again and again, each test in its own process; a
//! hit reads the converted map back instead.
//!
//! - **What:** whatever a loader converts (`cached`'s `T`: CS:S keeps its
//!   `MapData` with the lighting its light field closes over; Combat
//!   Arms its world before the per-session effect library, only with
//!   `CHIMERA_MAP_CACHE_CA=1`: its worlds convert quickly), serialized
//!   with postcard (big arrays as byte strings, big lists element by
//!   element on threads: chimera_world's `map::pod_serde`): our own
//!   format, a local cache like the asset store, never the games' files.
//! - **Where:** `<cache dir>/chimera/maps/` (`~/.cache/chimera/maps`,
//!   `%LOCALAPPDATA%\chimera\maps`), one file per key, shared by every
//!   checkout and agent slot; `CHIMERA_MAP_CACHE=<folder>` puts it
//!   elsewhere, `CHIMERA_MAP_CACHE=0` turns it off (as does `disable`, a
//!   tool's `--no-map-cache`); the console's `chimera:map_cache 0/1`
//!   (`CVAR`, registered by chimera_sim's `SimBasePlugins`) turns it off or on
//!   over the variable from the next load, and on the command line
//!   (`+chimera:map_cache 0`) from the first, also a load made before
//!   the console runs. `CHIMERA_MAP_CACHE_GB` caps its size
//!   (`DEFAULT_CAP_GB`): after each write the least recently used entries
//!   go first (a hit marks its entry used). Free space weighs in
//!   (`disk`): the effective cap is no more than what the cache holds plus
//!   the disk's free space above the floor (`CHIMERA_DISK_FLOOR_GB`, 20 GB
//!   or 5% of the disk, whichever is smaller), and a write that would leave
//!   less than the floor free is skipped with a one-line warning (the map
//!   is converted and used all the same): a write starts only above the
//!   floor and stops at the chunk that would cross it (an entry's size
//!   isn't known before it is encoded).
//! - **Key** (the file's name, `Key`): the loader, the converter's version
//!   (the hash of its sources the game's `build.rs` takes, with
//!   chimera_world's map format: its map types' sources and a hand-bumped
//!   number), the map's name, `MapLoadOptions`, the map file's own hash
//!   and the mount's layers by name (an install, the asset store or
//!   both). The other files the conversion reads go in the entry: the
//!   mount tracks every path asked for, found or not, and every folder
//!   listed (`chimera_vfs::Mount::track`), and the entry keeps each one's
//!   fingerprint (an archive entry's checksum and place, a store record's
//!   id, else a BLAKE3 of the bytes: `fingerprint`). A hit checks them all
//!   again (on threads); one that differs (a patched install, a new loose
//!   file in front of an archive's) converts again and replaces the entry.
//! - **Writing:** a miss takes the key's lock file (`File::lock`, freed
//!   when the process ends), so processes missing the same map at once
//!   wait for the first one's entry instead of all converting; the entry
//!   is encoded straight into a temporary file (`ChunkWriter`: the
//!   payload a 16 MB chunk at a time, each hashed on a thread, then the
//!   header; a whole de_dust2's miss peaks at 1.5 GB, not 2.9 with the
//!   encoded entry held beside the map) and renamed into place, so a
//!   reader never sees half of one. Reading one maps it (processes reading the
//!   same entry share its pages), checks it (a BLAKE3 per 16 MB, on
//!   threads) and decodes it. An entry that doesn't read (truncated, its
//!   checksum wrong, an older format) is a miss. `CHIMERA_MAP_CACHE_LOG=1`
//!   prints where a hit's time goes.
//!
//! Known shortcuts: files a converter reads without the mount are the
//! loader's to put in its key. An entry truncated by hand while a process
//! reads it (mapped) crashes that process.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, SystemTime},
};

use chimera_vfs::{Access, Mount};

use crate::disk;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Where the cache is, or `0` (also `off`, `false`) for none.
pub const ENV: &str = "CHIMERA_MAP_CACHE";
/// The cache's cap in GB (decimal).
pub const CAP_ENV: &str = "CHIMERA_MAP_CACHE_GB";
/// The cap when `CAP_ENV` isn't set.
pub const DEFAULT_CAP_GB: f64 = 40.0;

/// An entry: this, the payload, the header (`Header`), the header's
/// length (u64, little-endian). The header comes last so the payload can
/// be encoded straight into the file (`ChunkWriter`).
const MAGIC: &[u8; 8] = b"CHIMAP\0\x03";

/// The payload's checksums: one BLAKE3 per this many bytes, checked on
/// threads.
const CHUNK: usize = 16 << 20;

/// The console's switch (`CVAR`): 0 not set, 1 on, 2 off.
static SWITCH: AtomicU8 = AtomicU8::new(0);

/// The console variable that turns the cache on (1) or off (0) for the
/// process, over `ENV` (chimera_sim's `SimBasePlugins` registers it). Read at
/// each map load; given on the command line (`+chimera:map_cache 0`, or
/// a name `CVAR_NAMES` lists) it holds from the first load, also one made
/// before the console runs (a dedicated server's first map).
pub const CVAR: &str = "chimera:map_cache";

/// `CVAR` as the command line may name it: in full, short, by the old
/// prefixes (the console resolves the same).
pub const CVAR_NAMES: &[&str] = &["chimera:map_cache", "map_cache", "chimera_map_cache", "mashup_map_cache"];

/// No cache for the rest of the process (a tool's `--no-map-cache`).
pub fn disable() {
    set_switch(Some(false));
}

/// Turn the cache on or off for the process over `ENV` (`CVAR`); None
/// leaves it to the environment again.
pub fn set_switch(on: Option<bool>) {
    let v = match on {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    };
    SWITCH.store(v, Ordering::Relaxed);
}

/// What `set_switch` set, else `CVAR` on the process's command line.
fn switch() -> Option<bool> {
    match SWITCH.load(Ordering::Relaxed) {
        1 => Some(true),
        2 => Some(false),
        _ => {
            static ARGS: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
            *ARGS.get_or_init(|| command_line_switch(std::env::args()))
        }
    }
}

/// `+chimera:map_cache <0|1>` (or another of `CVAR_NAMES`) in a command
/// line; the last one counts.
fn command_line_switch(args: impl Iterator<Item = String>) -> Option<bool> {
    let args: Vec<String> = args.collect();
    let at = args
        .iter()
        .rposition(|a| {
            a.strip_prefix('+')
                .is_some_and(|n| CVAR_NAMES.iter().any(|c| n.eq_ignore_ascii_case(c)))
        })?;
    parse_switch(args.get(at + 1)?)
}

/// A switch's value: 0 or 1 (also off/on, false/true, no/yes).
pub fn parse_switch(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "0" | "off" | "false" | "no" => Some(false),
        "1" | "on" | "true" | "yes" => Some(true),
        _ => None,
    }
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
        let var = std::env::var(ENV).ok();
        let cap = std::env::var(CAP_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok());
        match switch() {
            Some(false) => None,
            // On: the folder `ENV` names, else the default one.
            Some(true) => Self::from_settings(var.as_deref(), cap).or_else(|| Self::from_settings(None, cap)),
            None => Self::from_settings(var.as_deref(), cap),
        }
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
        self.evict_with(keep, &disk::space);
    }

    /// `evict` with the free-space reader (`disk::limit_cap`).
    pub fn evict_with(&self, keep: Option<&Path>, read: disk::Reader) {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|e| e.1).sum();
        let cap = disk::limit_cap(self.cap, total, &self.dir, read);
        entries.sort_by_key(|e| e.2);
        for (path, size, _) in entries {
            if total <= cap {
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
    /// A BLAKE3 of each `CHUNK` of the payload.
    payload_hashes: Vec<[u8; 32]>,
}

/// `f(i)` for each `i` in `0..n`, on up to as many threads as the machine
/// has, in order.
pub fn parallel<R: Send>(n: usize, f: impl Fn(usize) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |t| t.get())
        .min(n.max(1));
    if threads <= 1 {
        return (0..n).map(f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<(usize, R)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= n {
                            break mine;
                        }
                        mine.push((i, f(i)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, r)| r).collect()
}

fn chunk_hashes(payload: &[u8]) -> Vec<[u8; 32]> {
    let chunks: Vec<&[u8]> = payload.chunks(CHUNK).collect();
    parallel(chunks.len(), |i| *blake3::hash(chunks[i]).as_bytes())
}

/// An entry's bytes: the file mapped (its pages are the system's file
/// cache, shared by every process reading the same entry, and nothing is
/// copied), else read.
enum EntryBytes {
    Mapped(memmap2::Mmap),
    Read(Vec<u8>),
}

impl std::ops::Deref for EntryBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Mapped(m) => m,
            Self::Read(b) => b,
        }
    }
}

fn open_entry(path: &Path) -> io::Result<EntryBytes> {
    let file = File::open(path)?;
    // SAFETY: entries are never written in place (a new one is renamed
    // over the old, eviction unlinks), so the mapped bytes don't change
    // while they are read; only someone truncating the file by hand could
    // pull them away.
    match unsafe { memmap2::Mmap::map(&file) } {
        Ok(m) => Ok(EntryBytes::Mapped(m)),
        Err(_) => fs::read(path).map(EntryBytes::Read),
    }
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
    let bytes = match open_entry(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(Unusable::Missing),
        Err(e) => return Err(Unusable::Corrupt(e.to_string())),
    };
    let corrupt = |why: &str| Unusable::Corrupt(why.to_string());
    let rest = bytes
        .strip_prefix(MAGIC.as_slice())
        .ok_or_else(|| corrupt("not a map cache entry"))?;
    let (rest, len) = rest
        .split_at_checked(rest.len().wrapping_sub(8))
        .ok_or_else(|| corrupt("truncated"))?;
    let len = u64::from_le_bytes(len.try_into().expect("8 bytes")) as usize;
    let (payload, head) = rest
        .split_at_checked(rest.len().wrapping_sub(len))
        .ok_or_else(|| corrupt("truncated header"))?;
    let header: Header = postcard::from_bytes(head).map_err(|e| corrupt(&format!("header: {e}")))?;
    if header.key != key {
        return Err(corrupt("another key's entry"));
    }
    let read = start.elapsed().as_secs_f32();
    if header.payload_len != payload.len() as u64 || chunk_hashes(payload) != header.payload_hashes {
        return Err(corrupt("checksum"));
    }
    let checked = start.elapsed().as_secs_f32();
    let inputs = header.inputs.len();
    let now = parallel(inputs, |i| {
        fingerprint(mount, &Access::from(header.inputs[i].0.clone()))
    });
    for ((input, was), now) in header.inputs.into_iter().zip(now) {
        if now != was {
            return Err(Unusable::Stale(format!(
                "{:?}: was {was}, now {now}",
                Access::from(input)
            )));
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

/// Where an entry's payload is encoded to (a postcard flavor): its file,
/// a `CHUNK` at a time, each chunk's BLAKE3 taken on its own thread while
/// the next is encoded. A miss holds one chunk per hashing thread besides
/// the map, not the whole entry. Each chunk written asks the disk again
/// (`disk::has_room`): one that would leave less than the floor free
/// stops the write.
struct ChunkWriter<'a> {
    file: File,
    dir: &'a Path,
    read: disk::Reader<'a>,
    buf: Vec<u8>,
    len: u64,
    hashing: std::collections::VecDeque<std::thread::JoinHandle<[u8; 32]>>,
    hashes: Vec<[u8; 32]>,
    error: Option<WriteError>,
}

/// Why an entry wasn't written.
#[derive(Debug)]
enum WriteError {
    Io(io::Error),
    /// The disk's free space reached the floor (`disk`).
    Floor,
}

impl From<io::Error> for WriteError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl<'a> ChunkWriter<'a> {
    fn new(file: File, dir: &'a Path, read: disk::Reader<'a>) -> Self {
        Self {
            file,
            dir,
            read,
            buf: Vec::with_capacity(CHUNK),
            len: 0,
            hashing: Default::default(),
            hashes: Vec::new(),
            error: None,
        }
    }

    /// Write and hash the chunk in `buf` (the last may be short).
    fn flush_chunk(&mut self) -> Result<(), WriteError> {
        if self.buf.is_empty() {
            return Ok(());
        }
        if !disk::has_room(self.dir, self.buf.len() as u64, self.read) {
            return Err(WriteError::Floor);
        }
        self.file.write_all(&self.buf)?;
        let chunk = std::mem::replace(&mut self.buf, Vec::with_capacity(CHUNK));
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        if self.hashing.len() >= threads
            && let Some(h) = self.hashing.pop_front()
        {
            self.hashes.push(h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)));
        }
        self.hashing
            .push_back(std::thread::spawn(move || *blake3::hash(&chunk).as_bytes()));
        Ok(())
    }

    fn extend(&mut self, mut data: &[u8]) -> Result<(), WriteError> {
        self.len += data.len() as u64;
        while !data.is_empty() {
            let take = (CHUNK - self.buf.len()).min(data.len());
            self.buf.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.buf.len() == CHUNK {
                self.flush_chunk()?;
            }
        }
        Ok(())
    }

    /// The payload's length and its chunks' hashes, every byte written.
    fn finish(mut self) -> Result<(File, u64, Vec<[u8; 32]>), WriteError> {
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        self.flush_chunk()?;
        for h in self.hashing.drain(..) {
            self.hashes.push(h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)));
        }
        Ok((self.file, self.len, self.hashes))
    }
}

impl postcard::ser_flavors::Flavor for &mut ChunkWriter<'_> {
    type Output = ();

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        if self.error.is_some() {
            return Err(postcard::Error::SerializeBufferFull);
        }
        self.extend(data).map_err(|e| {
            self.error = Some(e);
            postcard::Error::SerializeBufferFull
        })
    }

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        self.try_extend(&[data])
    }

    fn finalize(self) -> postcard::Result<()> {
        Ok(())
    }
}

/// Write `value`'s entry: encoded straight into a temporary file (the
/// payload after the magic, then the header, then the header's length),
/// renamed into place when whole. False when the disk's free space is at
/// the floor (nothing written).
fn write_entry<T: Serialize>(
    cache: &MapCache,
    path: &Path,
    key: &str,
    inputs: Vec<(Input, String)>,
    value: &T,
    read: disk::Reader,
) -> io::Result<bool> {
    fs::create_dir_all(&cache.dir)?;
    let floor = || {
        disk::warn_once("map cache", || {
            format!(
                "not writing {}: less than the free-space floor would be left on its disk ({} sets it)",
                path.display(),
                disk::FLOOR_ENV
            )
        });
        Ok(false)
    };
    if !disk::has_room(&cache.dir, 0, read) {
        return floor();
    }
    let tmp = cache.dir.join(format!("{key}.{}.{}.tmp", std::process::id(), unique()));
    let written = (|| -> Result<(), WriteError> {
        let mut file = File::create(&tmp)?;
        file.write_all(MAGIC)?;
        let mut writer = ChunkWriter::new(file, &cache.dir, read);
        if let Err(e) = postcard::serialize_with_flavor(value, &mut writer) {
            // The file's error (or the floor), else the value's.
            return Err(writer.error.take().unwrap_or_else(|| WriteError::Io(io::Error::other(e))));
        }
        let (mut file, payload_len, payload_hashes) = writer.finish()?;
        let header = Header {
            key: key.to_string(),
            inputs,
            payload_len,
            payload_hashes,
        };
        let head = postcard::to_allocvec(&header).map_err(io::Error::other)?;
        file.write_all(&head)?;
        file.write_all(&(head.len() as u64).to_le_bytes())?;
        // No fsync: an entry cut short by a crash fails its checksum and
        // converts again.
        drop(file);
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    match written {
        Ok(()) => Ok(true),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            match e {
                WriteError::Floor => floor(),
                WriteError::Io(e) => Err(e),
            }
        }
    }
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
    cached_with(cache, mount, key, convert, &disk::space)
}

/// `cached` with the free-space reader.
pub fn cached_with<T: Serialize + DeserializeOwned>(
    cache: Option<&MapCache>,
    mount: &Mount,
    key: Key,
    convert: impl FnOnce() -> Result<T, String>,
    read: disk::Reader,
) -> Result<(T, Outcome), String> {
    let Some(cache) = cache else {
        return convert().map(|v| (v, Outcome::Off));
    };
    // The mount's layers by name: a load from the asset store alone and
    // one from the install are two entries, not one replacing the other
    // at each load (their inputs' fingerprints differ).
    let layers: Vec<String> = mount.layers().map(|l| l.name()).collect();
    let key = key.part("layers", layers.join("\n")).hex();
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
    if let Err(e) = write_entry(cache, &path, &key, inputs, &value, read) {
        eprintln!("map cache: {}: {e}", path.display());
    }
    drop(lock);
    cache.evict_with(Some(&path), read);
    Ok((value, Outcome::Miss))
}

fn inputs(mount: &Mount, accesses: BTreeSet<Access>) -> Vec<(Input, String)> {
    let accesses: Vec<Access> = accesses.into_iter().collect();
    let prints = parallel(accesses.len(), |i| fingerprint(mount, &accesses[i]));
    accesses.into_iter().map(Input::from).zip(prints).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_vfs::LooseDir;

    /// Plenty of room whatever the machine's disk holds.
    fn roomy(_: &Path) -> Option<disk::Space> {
        Some(disk::Space { free: 1 << 50, total: 1 << 51 })
    }

    /// `cached` on a disk with room (these tests aren't about space).
    fn cached<T: Serialize + DeserializeOwned>(
        cache: Option<&MapCache>,
        mount: &Mount,
        key: Key,
        convert: impl FnOnce() -> Result<T, String>,
    ) -> Result<(T, Outcome), String> {
        cached_with(cache, mount, key, convert, &roomy)
    }

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
    fn the_command_line_switch_is_read() {
        let args = |a: &[&str]| command_line_switch(a.iter().map(|s| s.to_string()));
        assert_eq!(args(&["app", "+map", "de_dust2"]), None);
        assert_eq!(args(&["app", "+chimera:map_cache", "0", "+map", "x"]), Some(false));
        assert_eq!(args(&["app", "+map_cache", "0"]), Some(false));
        assert_eq!(args(&["app", "+CHIMERA_MAP_CACHE", "on"]), Some(true), "the old name");
        assert_eq!(args(&["app", "+chimera_map_cache", "0", "+chimera_map_cache", "1"]), Some(true), "the last wins");
        assert_eq!(args(&["app", "+chimera_map_cache"]), None);
        assert_eq!(args(&["app", "+chimera_map_cache", "maybe"]), None);
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
        let before = cache.entries().len();
        let e = cached::<String>(Some(&cache), &mount, Key::new("test", "fails"), || Err("no".into()));
        assert_eq!(e.unwrap_err(), "no");
        assert_eq!(cache.entries().len(), before);
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
        let path = cache.entries().pop().expect("the entry").0;
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
        cache.evict_with(Some(&root.join("a.map")), &roomy);
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
    fn a_write_below_the_free_space_floor_is_skipped_and_the_map_still_loads() {
        let root = temp("floor");
        let files = root.join("files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("a.txt"), "one").unwrap();
        let mut mount = Mount::default();
        mount.push(LooseDir::new(&files));
        let cache = MapCache::at(root.join("cache"), u64::MAX);
        let key = || Key::new("test", "v1");
        let gb = 1_000_000_000u64;
        let disk_with = |free: u64| move |_: &Path| Some(disk::Space { free, total: 1000 * gb });
        let mut runs = 0;
        // 1 GB free of 1000, the floor 20 GB: converted, nothing written.
        let low = disk_with(gb);
        let (v, o) = cached_with(Some(&cache), &mount, key(), || convert(&mount, &mut runs), &low).unwrap();
        assert_eq!((v.as_str(), o, runs), ("onefalse", Outcome::Miss, 1));
        assert!(cache.entries().is_empty(), "no entry below the floor");
        // Room again: written, then a hit.
        let roomy = disk_with(500 * gb);
        cached_with(Some(&cache), &mount, key(), || convert(&mount, &mut runs), &roomy).unwrap();
        assert_eq!(cache.entries().len(), 1);
        let (_, o) = cached_with(Some(&cache), &mount, key(), || convert(&mount, &mut runs), &low).unwrap();
        assert_eq!((o, runs), (Outcome::Hit, 2), "reading needs no room");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn free_space_clamps_the_effective_cap() {
        let root = temp("clamp");
        let cache = MapCache::at(&root, u64::MAX / 4);
        let now = SystemTime::now();
        for (i, name) in ["a", "b", "c"].iter().enumerate() {
            let p = root.join(format!("{name}.map"));
            fs::write(&p, [0u8; 100]).unwrap();
            let f = File::options().append(true).open(&p).unwrap();
            f.set_modified(now - Duration::from_secs(100 - i as u64 * 10)).unwrap();
        }
        // A disk of 2000 bytes, floor 5% = 100: 150 free leaves 50 of
        // growth over the 300 held, far under the cap, so nothing goes
        // (a full disk stops growth, it doesn't empty the cache).
        let disk = |free| move |_: &Path| Some(disk::Space { free, total: 2000 });
        cache.evict_with(None, &disk(150));
        assert_eq!(cache.entries().len(), 3);
        // The same cache with a cap of 250 evicts as before.
        let capped = MapCache::at(&root, 250);
        capped.evict_with(None, &disk(1500));
        assert_eq!(capped.entries().len(), 2);
        // 120 free (20 above the floor): it may hold 220, so the 200 stay.
        capped.evict_with(None, &disk(120));
        assert_eq!(capped.entries().len(), 2);
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
