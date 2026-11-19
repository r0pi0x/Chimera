//! A map's spawn spots kept on disk (docs/plans/active/lobby-between-maps.md,
//! "Loading behind the lobby"): `minigame::spawns` makes them from the
//! map's collision (a flood, then a ray and a box per point: seconds of
//! slices on a big map, a quarter of a swap's first-tick cost), writes
//! them once, and reads them back on the map's next load, on a task, with
//! no flood and no tests. The spots are the same ones: they are kept as
//! the exact floats, in the order they are tried.
//!
//! - **The key** (`key`): the map's file (`MapFile`: its name and content
//!   hash; a map without a hash is never kept), the caller's own parts
//!   (the seed, the spawn points, the walker, the play area, the hazards,
//!   the meshes the spots stand on: everything that decides the spots) and
//!   the caller's version, plus the map format's version (`cache::format`).
//!   The same key gives the same spots: the walk is deterministic.
//! - **Where**: beside the kept navigation meshes (`nav_cache`, `spots/`
//!   next to `nav/`); off when the converted-map cache is off or with
//!   `CHIMERA_SPOT_CACHE=0`; `CHIMERA_SPOT_CACHE=<folder>` puts it
//!   elsewhere.
//! - **Capped** (`CAP`, 4 MB, a few hundred maps: an entry is about 18 KB
//!   at most): after each write the least recently used entries (a read
//!   touches its entry) go until the rest fit.
//! - **Off the main thread**: `read` and `write` are for the caller's
//!   tasks; a file written half or of another format reads as a miss.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::{MapFile, cache};

/// The most the kept spots take on disk (bytes).
pub const CAP: u64 = 4 << 20;

/// `0` (also `off`, `false`) turns the kept spots off; a folder puts them there.
pub const ENV: &str = "CHIMERA_SPOT_CACHE";

/// The start of every entry (a file of another format is a miss).
const MAGIC: &[u8; 8] = b"CHISPT\0\x01";

/// Where the kept spots are, in place of the environment's choice (a test's
/// own folder; `None` inside: off).
#[derive(Resource, Clone, Debug)]
pub struct SpotCacheDir(pub Option<PathBuf>);

/// Where the kept spots are, None when off (module doc).
pub fn dir() -> Option<PathBuf> {
    match std::env::var(ENV).ok().as_deref().map(str::trim) {
        Some("0" | "off" | "false") => return None,
        Some(p) if !p.is_empty() => return Some(PathBuf::from(p)),
        _ => {}
    }
    let maps = cache::MapCache::from_env()?;
    Some(maps.dir.parent().map_or_else(|| maps.dir.join("spots"), |p| p.join("spots")))
}

/// The key of spots made on `map` from `parts` (name, bytes) by code of
/// version `version` (the caller's); None for a map without a content hash.
pub fn key(map: &MapFile, parts: &[(&str, Vec<u8>)], version: &str) -> Option<String> {
    let hash = map.hash?;
    let mut k = cache::Key::new("spots", version)
        .part("format", cache::format())
        .part("map", map.name.to_ascii_lowercase())
        .part("file", hash);
    for (name, bytes) in parts {
        k = k.part(name, bytes);
    }
    Some(k.hex())
}

fn path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.spots"))
}

/// Whether `key` is kept.
pub fn has(dir: &Path, key: &str) -> bool {
    path(dir, key).is_file()
}

/// The spots kept under `key` (None: none, or unreadable); a read touches
/// the entry (the cap evicts the least recently used).
pub fn read(dir: &Path, key: &str) -> Option<Vec<Vec3>> {
    let p = path(dir, key);
    let bytes = std::fs::read(&p).ok()?;
    let body = bytes.strip_prefix(MAGIC.as_slice())?;
    let spots = postcard::from_bytes::<Vec<[f32; 3]>>(body).ok()?;
    if let Ok(f) = std::fs::File::options().append(true).open(&p) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
    Some(spots.into_iter().map(Vec3::from).collect())
}

/// Keep `spots` under `key` (written whole, then renamed into place), then
/// evict down to `cap` bytes.
pub fn write(dir: &Path, key: &str, spots: &[Vec3], cap: u64) {
    let raw: Vec<[f32; 3]> = spots.iter().map(|s| s.to_array()).collect();
    let Ok(body) = postcard::to_allocvec(&raw) else { return };
    if body.len() as u64 + MAGIC.len() as u64 > cap || std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let p = path(dir, key);
    let tmp = dir.join(format!("{key}.{}.tmp", std::process::id()));
    let mut bytes = Vec::with_capacity(MAGIC.len() + body.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&body);
    if std::fs::write(&tmp, &bytes).is_err() || std::fs::rename(&tmp, &p).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    evict(dir, cap);
}

/// Drop `key`'s entry (it did not read).
pub fn remove(dir: &Path, key: &str) {
    let _ = std::fs::remove_file(path(dir, key));
}

/// The least recently used entries removed until the rest fit `cap`.
pub fn evict(dir: &Path, cap: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "spots"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    for (_, len, p) in files {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("chimera-spotcache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Spots written read back bit for bit; another key, a broken file or a
    /// map without a hash is no entry.
    #[test]
    fn kept_spots_read_back_the_same() {
        let dir = scratch("back");
        let map = MapFile { name: "test:m".into(), hash: Some([7; 32]) };
        let parts = [("seed", vec![1u8, 2])];
        let k = key(&map, &parts, "v1").unwrap();
        assert_ne!(key(&map, &[("seed", vec![1u8, 3])], "v1").unwrap(), k);
        assert_ne!(key(&map, &parts, "v2").unwrap(), k);
        assert!(key(&MapFile { name: "test:m".into(), hash: None }, &parts, "v1").is_none());
        assert!(read(&dir, &k).is_none());
        let spots = vec![Vec3::new(0.1, -2.5, 3.0), Vec3::new(f32::MIN_POSITIVE, 1e7, -0.0)];
        write(&dir, &k, &spots, CAP);
        assert!(has(&dir, &k));
        let back = read(&dir, &k).unwrap();
        assert!(spots.iter().zip(&back).all(|(a, b)| a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)));
        std::fs::write(path(&dir, &k), b"CHISPT\0\x01junk").unwrap();
        assert!(read(&dir, &k).is_none(), "a broken entry is a miss");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Past the cap the least recently used go.
    #[test]
    fn the_cap_keeps_the_most_recently_used() {
        let dir = scratch("cap");
        let spots = vec![Vec3::ONE; 500];
        let one = postcard::to_allocvec(&vec![[1f32; 3]; 500]).unwrap().len() as u64 + MAGIC.len() as u64;
        let cap = one * 2 + one / 2;
        let at = |s| std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s);
        for (i, k) in ["a", "b"].iter().enumerate() {
            write(&dir, k, &spots, cap);
            std::fs::File::options().append(true).open(path(&dir, k)).unwrap().set_modified(at(1000 + i as u64)).unwrap();
        }
        assert!(read(&dir, "a").is_some());
        write(&dir, "c", &spots, cap);
        assert!(has(&dir, "a") && has(&dir, "c"), "the recent ones stay");
        assert!(!has(&dir, "b"), "the least recently used went");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
