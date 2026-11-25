//! Converted entries the asset service serves (docs/plans/active/
//! converted-cache.md, step 7): an entry of this cache (`Header`,
//! payload), made by `chimera_pack convert` from the store's records with
//! the same converter code as the clients, stored as a record of the
//! game's manifest with its recipe beside it:
//!
//! - **Paths:** `_converted/<name>/<key>.map` (the entry) and
//!   `_converted/<name>/<key>-recipe.txt` (`Recipe`), where `<name>` says
//!   what it is and of which converter version (`guest/cs_source/<the
//!   converter's version>`: a guest dressing, `map::kept`) and `<key>`
//!   is the entry's key with `layers` = `served` (`SERVED_LAYERS`): the
//!   same parts as a local entry's (the converter's and map format's
//!   versions, the load options) but not this machine's folders. Their
//!   stems differ, so fetching the recipe never brings the entry with it
//!   (`CachedFiles`' on-demand groups); no set holds them.
//! - **The recipe:** the key's parts in words (`part converter ...`,
//!   `part format ...`) and every input of the conversion with its
//!   fingerprint (a store record's id, `record <sha-256>`), in the order
//!   the entry's header keeps them: what the entry was made from.
//! - **A client** (`take`, from `cached_served` on a local miss) whose
//!   versions match finds both paths in its mount, reads the recipe
//!   (small), checks every input against its own mount (no record
//!   fetched: a store layer's fingerprint is its manifest's record id),
//!   and only then reads the entry (fetched, its SHA-256 checked by
//!   `CachedFiles`), checks it again as a local entry (key, checksums,
//!   inputs) and keeps it in the local cache under its own key (the
//!   header's key replaced, the payload's bytes as served). Any
//!   difference (another converter or format: other paths; an install
//!   layered first, other files: other fingerprints) converts from the
//!   originals as before. `CHIMERA_CONVERTED_SERVED=0` (`SERVED_ENV`)
//!   never takes one.
//! - **The packer** (`Served::export`): each entry its conversions write
//!   (or read back) is handed over with its served key and recipe
//!   (`export`); the same files give the same bytes (the
//!   generated-assets rule), so anyone with the originals rebuilds the
//!   same records.
//!
//! - **The records a client fetched** stay in its asset store beside the
//!   local entry (a re-take after the local entry was evicted needs no
//!   download), capped like the map cache itself: after each take
//!   `evict_records` keeps the served records (entries and recipes under
//!   `SERVED_DIR`, as the mount's layers hold them on disk,
//!   `FileSource::cached_copy`) within `CHIMERA_SERVED_CACHE_GB`
//!   (`SERVED_CAP_ENV`, `DEFAULT_SERVED_CAP_GB`; free space weighs in as
//!   for the map cache, `disk::limit_cap`). Over it, the records of
//!   another converter version of the same game (older ones: the name's
//!   last part differs) go first, then the least recently taken (the
//!   record file's modified time, set by each take). Never removed: the
//!   entry just taken and its recipe, a record a fill is fetching (no
//!   `cached_copy`) and a file that won't unlink (still open on Windows).
//!   Other records of the store (the games' files) are never touched.
//!
//! Known shortcuts: a dressing that asks whether a file is here (`Access::Held`: TF2's class bodies) takes
//! the served entry only when the client holds what the packer held.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
    sync::Mutex,
};

use chimera_vfs::Mount;
use serde::de::DeserializeOwned;

use super::{Header, Input, MAGIC, MapCache, check_entry, check_inputs, disk, split_entry, unique};

/// The folder of the manifest's converted entries.
pub const SERVED_DIR: &str = "_converted";
/// The recipe's format line's version.
pub const RECIPE_FORMAT: u32 = 1;
/// `0` (`off`, `false`, `no`): never take a served entry.
pub const SERVED_ENV: &str = "CHIMERA_CONVERTED_SERVED";
/// The cap in GB (decimal) on the served records in the client's asset
/// store (module doc).
pub const SERVED_CAP_ENV: &str = "CHIMERA_SERVED_CACHE_GB";
/// The cap when `SERVED_CAP_ENV` isn't set: a quarter of the map cache's
/// (`map_cache::DEFAULT_CAP_GB`), as the local entry is the one used.
pub const DEFAULT_SERVED_CAP_GB: f64 = 10.0;
/// What a served entry's key has for the mount's layers.
pub(super) const SERVED_LAYERS: &str = "served";

/// A conversion the asset service may serve (`cached_served`).
pub struct Served<'a> {
    /// What it is and of which converter version: the folder under
    /// `SERVED_DIR` (`guest/cs_source/<converter>`).
    pub name: String,
    /// The key's parts in words, for the recipe (`converter`, `format`,
    /// `textures`, ...).
    pub parts: Vec<(String, String)>,
    /// The packer's: each entry made (or read back) is pushed here, and
    /// none is taken from the mount.
    pub export: Option<&'a Mutex<Vec<ServedEntry>>>,
}

/// A converted entry to serve, as the packer gets it (`Served::export`).
#[derive(Clone, Debug)]
pub struct ServedEntry {
    /// The entry's path in the manifest.
    pub entry_path: String,
    /// The entry's bytes (as the local cache writes them, its key the
    /// served one).
    pub entry: Vec<u8>,
    /// The recipe's path in the manifest.
    pub recipe_path: String,
    /// The recipe (`Recipe::to_text`).
    pub recipe: String,
}

/// `name`'s entry for served key `key`.
pub fn entry_path(name: &str, key: &str) -> String {
    format!("{SERVED_DIR}/{name}/{key}.map")
}

/// `name`'s recipe for served key `key`.
pub fn recipe_path(name: &str, key: &str) -> String {
    format!("{SERVED_DIR}/{name}/{key}-recipe.txt")
}

/// Whether served entries are taken (`SERVED_ENV`).
pub fn on() -> bool {
    !std::env::var(SERVED_ENV)
        .is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false" | "no"))
}

/// What a served entry was made from, as text beside it:
///
/// ```text
/// chimera-recipe 1
/// name guest/cs_source/<converter>
/// key <served key>
/// part <name> <value>
/// input <file|every|folder|held>\t<path>\t<fingerprint>
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub name: String,
    pub key: String,
    pub parts: Vec<(String, String)>,
    inputs: Vec<(Input, String)>,
}

impl Recipe {
    /// How many files (and listings) the conversion read.
    pub fn inputs(&self) -> usize {
        self.inputs.len()
    }

    /// The store records the conversion read (`record <id>` fingerprints).
    pub fn records(&self) -> Vec<&str> {
        self.inputs
            .iter()
            .filter_map(|(_, f)| f.split_once(" record ").map(|(_, id)| id))
            .collect()
    }

    pub fn to_text(&self) -> String {
        let mut out = format!("chimera-recipe {RECIPE_FORMAT}\nname {}\nkey {}\n", self.name, self.key);
        for (name, value) in &self.parts {
            out.push_str(&format!("part {name} {value}\n"));
        }
        for (input, print) in &self.inputs {
            let (kind, path) = match input {
                Input::File(p) => ("file", p),
                Input::Every(p) => ("every", p),
                Input::Folder(p) => ("folder", p),
                Input::Held(p) => ("held", p),
            };
            out.push_str(&format!("input {kind}\t{path}\t{print}\n"));
        }
        out
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some(format!("chimera-recipe {RECIPE_FORMAT}").as_str()) {
            return Err(format!("not a version {RECIPE_FORMAT} recipe"));
        }
        let mut r = Recipe { name: String::new(), key: String::new(), parts: Vec::new(), inputs: Vec::new() };
        for line in lines.filter(|l| !l.is_empty()) {
            let (what, rest) = line.split_once(' ').ok_or_else(|| format!("no value: {line}"))?;
            match what {
                "name" => r.name = rest.into(),
                "key" => r.key = rest.into(),
                "part" => {
                    let (n, v) = rest.split_once(' ').unwrap_or((rest, ""));
                    r.parts.push((n.into(), v.into()));
                }
                "input" => {
                    let mut f = rest.splitn(3, '\t');
                    let (Some(kind), Some(path), Some(print)) = (f.next(), f.next(), f.next()) else {
                        return Err(format!("an input needs a kind, a path and a fingerprint: {line}"));
                    };
                    let path = path.to_string();
                    let input = match kind {
                        "file" => Input::File(path),
                        "every" => Input::Every(path),
                        "folder" => Input::Folder(path),
                        "held" => Input::Held(path),
                        _ => return Err(format!("unknown input kind {kind}")),
                    };
                    r.inputs.push((input, print.into()));
                }
                _ => return Err(format!("unknown line {what}")),
            }
        }
        if r.name.is_empty() || r.key.is_empty() {
            return Err("a recipe without its name or key".into());
        }
        Ok(r)
    }
}

/// `entry` and its recipe stored in `store` and entered in `manifest`
/// (records already stored are kept); its stored bytes.
pub fn enter(store: &crate::Store, manifest: &mut crate::Manifest, entry: &ServedEntry) -> io::Result<u64> {
    let mut stored = 0;
    for (path, bytes) in [(&entry.entry_path, &entry.entry[..]), (&entry.recipe_path, entry.recipe.as_bytes())] {
        let put = store.put(bytes)?;
        stored += put.stored;
        manifest
            .files
            .insert(path.clone(), crate::FileRecord { id: put.id, size: bytes.len() as u64, stored: put.stored });
    }
    Ok(stored)
}

/// The manifest's served entries: each name (`guest/cs_source/<converter>`)
/// with its entries' count and plain and stored bytes (recipes too).
pub fn listed(manifest: &crate::Manifest) -> std::collections::BTreeMap<String, (usize, u64, u64)> {
    let mut out: std::collections::BTreeMap<String, (usize, u64, u64)> = Default::default();
    for (path, f) in manifest.files.range(format!("{SERVED_DIR}/")..) {
        let Some(rest) = path.strip_prefix(&format!("{SERVED_DIR}/")) else { break };
        let Some((name, file)) = rest.rsplit_once('/') else { continue };
        let e = out.entry(name.to_string()).or_default();
        e.0 += file.ends_with(".map") as usize;
        e.1 += f.size;
        e.2 += f.stored;
    }
    out
}

/// Drop the served entries whose name starts with `prefix`
/// (`guest/cs_source/<converter>`: one converter version's) from
/// `manifest` (their records stay until `chimera_assets gc`); how many
/// paths went.
pub fn drop_served(manifest: &mut crate::Manifest, prefix: &str) -> usize {
    let start = format!("{SERVED_DIR}/{prefix}");
    let before = manifest.files.len();
    manifest.files.retain(|p, _| !p.starts_with(&start));
    before - manifest.files.len()
}

/// `header` written for `payload` under `key`: the entry's bytes.
fn rekeyed(payload: &[u8], header: &Header, key: &str) -> Result<Vec<u8>, String> {
    let head = header_bytes(header, key)?;
    let mut out = Vec::with_capacity(MAGIC.len() + payload.len() + head.len() + 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(payload);
    out.extend_from_slice(&head);
    out.extend_from_slice(&(head.len() as u64).to_le_bytes());
    Ok(out)
}

fn header_bytes(header: &Header, key: &str) -> Result<Vec<u8>, String> {
    let header = Header {
        key: key.to_string(),
        inputs: header.inputs.clone(),
        payload_len: header.payload_len,
        payload_hashes: header.payload_hashes.clone(),
    };
    postcard::to_allocvec(&header).map_err(|e| e.to_string())
}

/// The packer's: the local entry at `path` as the served entry of
/// `served_key`, with its recipe.
pub(super) fn export(path: &Path, served: &Served, served_key: &str) -> Result<ServedEntry, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: the converted entry wasn't written ({e})", path.display()))?;
    let (payload, header) = split_entry(&bytes).map_err(|e| format!("{}: {e:?}", path.display()))?;
    let recipe = Recipe {
        name: served.name.clone(),
        key: served_key.to_string(),
        parts: served.parts.clone(),
        inputs: header.inputs.clone(),
    };
    Ok(ServedEntry {
        entry_path: entry_path(&served.name, served_key),
        entry: rekeyed(payload, &header, served_key)?,
        recipe_path: recipe_path(&served.name, served_key),
        recipe: recipe.to_text(),
    })
}

/// The client's: the entry `mount` serves for `served_key`, when its
/// recipe's inputs are `mount`'s files, decoded and kept at `path` under
/// `key` (module doc). None: none served, or not for these files.
pub(super) fn take<T: DeserializeOwned>(
    cache: &MapCache,
    mount: &Mount,
    served: &Served,
    served_key: &str,
    path: &Path,
    key: &str,
    read: disk::Reader,
) -> Option<T> {
    let (entry, recipe) = (entry_path(&served.name, served_key), recipe_path(&served.name, served_key));
    if !mount.contains(&entry) || !mount.contains(&recipe) {
        return None;
    }
    let start = std::time::Instant::now();
    let taken = (|| -> Result<T, String> {
        let text = mount.read(&recipe).map_err(|e| format!("its recipe: {e}"))?;
        let r = Recipe::parse(&String::from_utf8_lossy(&text)).map_err(|e| format!("its recipe: {e}"))?;
        if r.key != served_key {
            return Err("its recipe is another key's".into());
        }
        check_inputs(mount, &r.inputs).map_err(|e| format!("made from other files ({e:?})"))?;
        let bytes = mount.read(&entry).map_err(|e| e.to_string())?;
        // Fetched bytes: always checked whole.
        let value = check_entry::<T>(&bytes, served_key, mount, &entry, start, true).map_err(|e| format!("{e:?}"))?;
        let (payload, header) = split_entry(&bytes).map_err(|e| format!("{e:?}"))?;
        if let Err(e) = write_rekeyed(cache, path, payload, &header, key, read) {
            eprintln!("map cache: {}: {e}", path.display());
        }
        evict_records(mount, &served.name, &[&entry, &recipe], served_cap(), &disk::space);
        eprintln!(
            "map cache: took the served {entry} ({:.0} MB, {} inputs) in {:.2} s",
            bytes.len() as f64 / 1e6,
            r.inputs(),
            start.elapsed().as_secs_f32()
        );
        Ok(value)
    })();
    taken.map_err(|why| eprintln!("map cache: the served {entry}: {why}; converting")).ok()
}

/// The cap on served records in bytes (`SERVED_CAP_ENV`).
fn served_cap() -> u64 {
    let gb = std::env::var(SERVED_CAP_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok());
    (gb.filter(|g| g.is_finite()).unwrap_or(DEFAULT_SERVED_CAP_GB).max(0.0) * 1e9) as u64
}

/// Whether `path`'s served name is another converter version of the same
/// game as `current` (`guest/<game>/<converter>`): an older one.
fn is_older(path: &str, current: &str) -> bool {
    let name = |p: &str| p.strip_prefix(SERVED_DIR).and_then(|r| r.strip_prefix('/')).and_then(|r| r.rsplit_once('/')).map(|(n, _)| n.to_string());
    let (Some(a), Some(b)) = (name(path), Some(current)) else { return false };
    match (a.rsplit_once('/'), b.rsplit_once('/')) {
        (Some((game_a, conv_a)), Some((game_b, conv_b))) => game_a == game_b && conv_a != conv_b,
        _ => false,
    }
}

/// Remove served records from the mount's downloaded copies until the
/// rest fit `cap` (module doc): older converter versions first, then the
/// least recently taken; never `keep` (paths) nor a record in use. The
/// kept records are marked used now. Returns the bytes removed.
pub(super) fn evict_records(mount: &Mount, current: &str, keep: &[&str], cap: u64, read: disk::Reader) -> u64 {
    struct Held {
        file: std::path::PathBuf,
        size: u64,
        used: std::time::SystemTime,
        older: bool,
        keep: bool,
    }
    let prefix = format!("{SERVED_DIR}/");
    let mut held: Vec<Held> = Vec::new();
    for layer in mount.layers() {
        for e in layer.entries() {
            if !e.path.starts_with(&prefix) {
                continue;
            }
            let Some(file) = layer.cached_copy(&e.path) else { continue };
            if held.iter().any(|h| h.file == file) {
                continue;
            }
            let Ok(meta) = fs::metadata(&file) else { continue };
            held.push(Held {
                size: meta.len(),
                used: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                older: is_older(&e.path, current),
                keep: keep.contains(&e.path.as_str()),
                file,
            });
        }
    }
    for h in held.iter_mut().filter(|h| h.keep) {
        if let Ok(f) = File::options().append(true).open(&h.file) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
    }
    let mut total: u64 = held.iter().map(|h| h.size).sum();
    let Some(first) = held.first() else { return 0 };
    let cap = disk::limit_cap(cap, total, first.file.parent().unwrap_or(&first.file), read);
    let before = total;
    // Older versions first, each group least recently used first.
    held.sort_by_key(|h| (!h.older, h.used));
    for h in held.iter().filter(|h| !h.keep) {
        if total <= cap {
            break;
        }
        if fs::remove_file(&h.file).is_ok() {
            total -= h.size;
        }
    }
    before - total
}

/// A served entry kept in the local cache at `path` under `key` (a
/// temporary file renamed into place; skipped at the free-space floor).
fn write_rekeyed(
    cache: &MapCache,
    path: &Path,
    payload: &[u8],
    header: &Header,
    key: &str,
    read: disk::Reader,
) -> io::Result<()> {
    fs::create_dir_all(&cache.dir)?;
    let head = header_bytes(header, key).map_err(io::Error::other)?;
    if !disk::has_room(&cache.dir, (payload.len() + head.len()) as u64, read) {
        eprintln!("map cache: not keeping {}: less than the free-space floor would be left", path.display());
        return Ok(());
    }
    let tmp = cache.dir.join(format!("{key}.{}.{}.tmp", std::process::id(), unique()));
    let written = (|| -> io::Result<()> {
        let mut file = File::create(&tmp)?;
        file.write_all(MAGIC)?;
        file.write_all(payload)?;
        file.write_all(&head)?;
        file.write_all(&(head.len() as u64).to_le_bytes())?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    match &written {
        // The payload was checked as it was taken (`take`).
        Ok(()) => super::Stamp::leave(path),
        Err(_) => {
            let _ = fs::remove_file(&tmp);
        }
    }
    written
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chimera_vfs::LooseDir;

    use super::*;
    use crate::{CachedFiles, Manifest, Store, map_cache::{Key, Outcome, cached_served_with}};

    fn roomy(_: &Path) -> Option<disk::Space> {
        Some(disk::Space { free: 1 << 50, total: 1 << 51 })
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("chimera_served_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A "conversion": the two files it reads, joined, and a listing.
    fn convert(mount: &Mount, runs: &mut u32) -> Result<Vec<String>, String> {
        *runs += 1;
        let mut out: Vec<String> = ["a.txt", "sub/b.txt"]
            .iter()
            .map(|p| String::from_utf8(mount.read(p).unwrap()).unwrap())
            .collect();
        out.extend(mount.files_in("sub"));
        out.push(mount.contains("c.txt").to_string());
        Ok(out)
    }

    fn served<'a>(name: &str, export: Option<&'a Mutex<Vec<ServedEntry>>>) -> Served<'a> {
        Served { name: name.into(), parts: vec![("converter".into(), "v1".into()), ("variant".into(), String::new())], export }
    }

    /// The packer converts from a store's records and hands over the
    /// entry with its recipe; a client of that store (another folder for
    /// its cache) takes it without converting, keeps it, and reads it
    /// back after; another converter version, or an install's file in
    /// front of the store's, converts as before.
    #[test]
    fn a_served_entry_is_taken_when_its_versions_and_files_match() {
        let dir = temp("flow");
        let install = dir.join("install");
        fs::create_dir_all(install.join("sub")).unwrap();
        fs::write(install.join("a.txt"), b"alpha").unwrap();
        fs::write(install.join("sub/b.txt"), b"beta").unwrap();
        let mut originals = Mount::default();
        originals.push(LooseDir::new(&install));
        let store = Store::new(dir.join("store"));
        let mut manifest = Manifest::new("fixture", "1", None);
        crate::pack(&originals, &["a.txt".into(), "sub/b.txt".into()], &store, &mut manifest, None, 1).unwrap();
        manifest.save(&store.manifest_path("fixture", "1")).unwrap();
        let store_mount = |m: &Manifest| {
            let mut mount = Mount::default();
            mount.push(CachedFiles::new(store.clone(), m));
            mount
        };
        let key = || Key::new("guest:fixture", "v1").part("textures", [1u8]);
        // The packer.
        let sink = Mutex::new(Vec::new());
        let mut runs = 0;
        let packer_cache = MapCache::at(dir.join("packer-cache"), u64::MAX);
        let mount = store_mount(&manifest);
        let (made, how) = cached_served_with(Some(&packer_cache), &mount, key(), Some(&served("guest/fixture/v1", Some(&sink))), || convert(&mount, &mut runs), &roomy).unwrap();
        assert_eq!((how, runs), (Outcome::Miss, 1));
        let entries = sink.into_inner().unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert!(e.entry_path.starts_with("_converted/guest/fixture/v1/") && e.entry_path.ends_with(".map"));
        let recipe = Recipe::parse(&e.recipe).unwrap();
        assert_eq!(recipe.records().len(), 2, "both files' records: {}", e.recipe);
        assert_eq!(Recipe::parse(&recipe.to_text()).unwrap(), recipe);
        // Again: the same bytes (the generated-assets rule), read back.
        let again = Mutex::new(Vec::new());
        cached_served_with(Some(&packer_cache), &mount, key(), Some(&served("guest/fixture/v1", Some(&again))), || convert(&mount, &mut runs), &roomy).unwrap();
        assert_eq!(again.into_inner().unwrap()[0].entry, e.entry, "the same bytes again");
        for e in &entries {
            enter(&store, &mut manifest, e).unwrap();
        }
        assert_eq!(listed(&manifest).get("guest/fixture/v1").map(|l| l.0), Some(1));
        // A client of the same files.
        let client = |manifest: &Manifest, name: &str, cache: &str, install_first: bool| {
            let mut mount = Mount::default();
            if install_first {
                mount.push(LooseDir::new(&install));
            }
            mount.push(CachedFiles::new(store.clone(), manifest));
            let cache = MapCache::at(dir.join(cache), u64::MAX);
            let mut runs = 0;
            let (v, how) = cached_served_with(Some(&cache), &mount, key(), Some(&served(name, None)), || convert(&mount, &mut runs), &roomy).unwrap();
            (v, how, runs)
        };
        assert_eq!(client(&manifest, "guest/fixture/v1", "client", false), (made.clone(), Outcome::Served, 0));
        assert_eq!(client(&manifest, "guest/fixture/v1", "client", false), (made.clone(), Outcome::Hit, 0), "kept");
        assert_eq!(client(&manifest, "guest/fixture/v2", "client-v2", false).1, Outcome::Miss, "another version converts");
        assert_eq!(client(&manifest, "guest/fixture/v1", "client-install", true).1, Outcome::Miss, "other files convert");
        // A changed original (a new build's record) converts too.
        let mut changed = manifest.clone();
        fs::write(install.join("a.txt"), b"alpha 2").unwrap();
        crate::pack(&originals, &["a.txt".into()], &store, &mut changed, None, 1).unwrap();
        let (v, how, runs) = client(&changed, "guest/fixture/v1", "client-changed", false);
        assert_eq!((how, runs), (Outcome::Miss, 1));
        assert_eq!(v[0], "alpha 2");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Over the cap, the records of an older converter version go first,
    /// then the least recently taken; the entry just taken, other
    /// records of the store and records of other games' versions stay
    /// until their turn.
    #[test]
    fn served_records_over_the_cap_go_older_versions_first_then_least_recently_taken() {
        let dir = temp("cap");
        let store = Store::new(dir.join("store"));
        let mut m = Manifest::new("g", "1", None);
        let mut put = |path: String, fill: u8, age: u64| {
            let bytes: Vec<u8> = (0..4000u32).map(|i| (i as u8).wrapping_mul(fill) ^ fill).collect();
            let p = store.put(&bytes).unwrap();
            m.files.insert(path, crate::FileRecord { id: p.id.clone(), size: bytes.len() as u64, stored: p.stored });
            let f = File::options().append(true).open(store.record_path(&p.id)).unwrap();
            f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(age)).unwrap();
            p.stored
        };
        // Newest converter v3 (current): two takes; v2 (older) and another game's v1.
        let game_file = put("maps/x.bsp".into(), 11, 1000);
        let old_new = put(entry_path("guest/g/v2", "a"), 3, 50);
        let old_old = put(entry_path("guest/g/v2", "b"), 5, 500);
        let other = put(entry_path("guest/h/v1", "a"), 7, 900);
        let cur_old = put(entry_path("guest/g/v3", "a"), 13, 800);
        let cur = put(entry_path("guest/g/v3", "b"), 17, 700);
        let mut mount = Mount::default();
        mount.push(CachedFiles::new(store.clone(), &m));
        let on_disk = |m: &Manifest| m.files.keys().filter(|p| mount.layers().any(|l| l.cached_copy(p).is_some())).cloned().collect::<Vec<_>>();
        let keep = [entry_path("guest/g/v3", "b")];
        let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
        let all = old_new + old_old + other + cur_old + cur;
        // Room for everything: nothing goes (the taken one is marked used).
        assert_eq!(evict_records(&mount, "guest/g/v3", &keep, all, &roomy), 0);
        // One record too many: the oldest of the older version goes.
        let gone = evict_records(&mount, "guest/g/v3", &keep, all - 1, &roomy);
        assert_eq!(gone, old_old);
        assert!(!on_disk(&m).contains(&entry_path("guest/g/v2", "b")));
        // Down to two records' room: the other older one, then the rest by age.
        evict_records(&mount, "guest/g/v3", &keep, cur + cur_old, &roomy);
        let left = on_disk(&m);
        assert!(!left.contains(&entry_path("guest/g/v2", "a")), "{left:?}");
        assert!(!left.contains(&entry_path("guest/h/v1", "a")), "{left:?}");
        assert!(left.contains(&entry_path("guest/g/v3", "a")) && left.contains(&entry_path("guest/g/v3", "b")), "{left:?}");
        // A cap of nothing still keeps the entry just taken, and never the games' files.
        evict_records(&mount, "guest/g/v3", &keep, 0, &roomy);
        let left = on_disk(&m);
        assert!(left.contains(&entry_path("guest/g/v3", "b")) && !left.contains(&entry_path("guest/g/v3", "a")), "{left:?}");
        assert!(left.contains(&"maps/x.bsp".to_string()), "the games' files are never touched");
        assert!(game_file > 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn served_entries_are_listed_and_dropped_by_version() {
        let mut m = Manifest::new("g", "1", None);
        let rec = |n: u64| crate::FileRecord { id: format!("{:064x}", n), size: 10, stored: 4 };
        m.files.insert("maps/x.bsp".into(), rec(1));
        m.files.insert(entry_path("guest/g/v1", "k"), rec(2));
        m.files.insert(recipe_path("guest/g/v1", "k"), rec(3));
        m.files.insert(entry_path("guest/g/v2", "k"), rec(4));
        let l = listed(&m);
        assert_eq!(l.get("guest/g/v1"), Some(&(1, 20, 8)));
        assert_eq!(l.len(), 2);
        assert_eq!(drop_served(&mut m, "guest/g/v1/"), 2);
        assert_eq!(m.files.len(), 2);
    }
}
