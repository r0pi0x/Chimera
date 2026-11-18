//! Generated navigation meshes kept on disk (docs/plans/active/
//! lobby-between-maps.md, "Loading behind the lobby"): a map's bots' mesh
//! (`nav_gen`, flooded from its collision on a task) is written once and
//! read back on the map's next load, so a map's second load has its mesh
//! at once instead of after seconds of flooding (a big Halo map's).
//!
//! - **The key** (`key`): the map's file (`MapFile`: its name and content
//!   hash; a map without a hash, a code-built one, is never kept), the
//!   seeds the flood starts from (the map's own: spawn points and
//!   teleporter exits, in a fixed order), the walker (`NavGenParams`, its
//!   play area and box among them), whether it was bounded by the map's
//!   visibility, the map format's version (`cache::format`: this crate's
//!   `map` sources, `nav_gen`'s among them) and the caller's own version
//!   (its code outside this crate). The same key gives the same mesh: a
//!   flood is deterministic.
//! - **Where**: beside the converted-map cache (`cache::MapCache`'s folder's
//!   parent, `nav/`), and off when that cache is off (`CHIMERA_MAP_CACHE=0`,
//!   `chimera:map_cache 0`) or with `CHIMERA_NAV_CACHE=0`;
//!   `CHIMERA_NAV_CACHE=<folder>` puts it elsewhere.
//! - **Capped** (`CAP`, 64 MB): after each write the least recently used
//!   entries (a read touches its entry) go until the rest fit.
//! - **Off the main thread**: `read` and `write` run on the caller's tasks;
//!   a file written half (a crash) or of another format reads as a miss.
//!
//! Known shortcuts: the key holds the map's seeds only, so a mesh flooded
//! with characters' feet as seeds too (a map whose spawn points don't reach
//! where they stand) is kept only when the caller leaves those out
//! (`combat_arms`' bots flood from the map's seeds when the cache is on).

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::{MapFile, cache, nav::NavMesh, nav_gen::NavGenParams};

/// The most the kept meshes take on disk (bytes).
pub const CAP: u64 = 64 << 20;

/// `0` (also `off`, `false`) turns the kept meshes off; a folder puts them there.
pub const ENV: &str = "CHIMERA_NAV_CACHE";

/// The start of every entry (a file of another format is a miss).
const MAGIC: &[u8; 8] = b"CHINAV\0\x01";

/// Where the kept meshes are, None when off (module doc).
pub fn dir() -> Option<PathBuf> {
    match std::env::var(ENV).ok().as_deref().map(str::trim) {
        Some("0" | "off" | "false") => return None,
        Some(p) if !p.is_empty() => return Some(PathBuf::from(p)),
        _ => {}
    }
    let maps = cache::MapCache::from_env()?;
    Some(maps.dir.parent().map_or_else(|| maps.dir.join("nav"), |p| p.join("nav")))
}

/// The key of a mesh flooded on `map` from `seeds` by `params`, bounded by
/// the map's visibility or not, by code of version `version` (the caller's);
/// None for a map without a content hash.
pub fn key(map: &MapFile, seeds: &[Vec3], params: &NavGenParams, bounded: bool, version: &str) -> Option<String> {
    let hash = map.hash?;
    let mut seeds: Vec<[u32; 3]> = seeds.iter().map(|s| s.to_array().map(f32::to_bits)).collect();
    seeds.sort_unstable();
    let seeds: Vec<u8> = seeds.iter().flatten().flat_map(|b| b.to_le_bytes()).collect();
    Some(
        cache::Key::new("nav", version)
            .part("format", cache::format())
            .part("map", map.name.to_ascii_lowercase())
            .part("file", hash)
            .part("seeds", seeds)
            .part("params", format!("{params:?}"))
            .part("bounded", [bounded as u8])
            .hex(),
    )
}

fn path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.nav"))
}

/// Whether `key` is kept (a quick look, for choosing a read over a flood).
pub fn has(dir: &Path, key: &str) -> bool {
    path(dir, key).is_file()
}

/// The mesh kept under `key` (None: none, or unreadable); a read touches
/// the entry (the cap evicts the least recently used).
pub fn read(dir: &Path, key: &str) -> Option<NavMesh> {
    let p = path(dir, key);
    let bytes = std::fs::read(&p).ok()?;
    let body = bytes.strip_prefix(MAGIC.as_slice())?;
    let mesh = postcard::from_bytes::<NavMesh>(body).ok()?;
    if let Ok(f) = std::fs::File::options().append(true).open(&p) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
    Some(mesh)
}

/// Keep `mesh` under `key` (written whole, then renamed into place), then
/// evict down to `cap` bytes.
pub fn write(dir: &Path, key: &str, mesh: &NavMesh, cap: u64) {
    let Ok(body) = postcard::to_allocvec(mesh) else { return };
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

/// The least recently used entries removed until the rest fit `cap`.
pub fn evict(dir: &Path, cap: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "nav"))
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
    use crate::map::nav::NavArea;

    fn mesh(n: usize) -> NavMesh {
        NavMesh {
            areas: (0..n)
                .map(|i| NavArea {
                    id: i as u32,
                    flags: 0,
                    min: Vec2::ZERO,
                    max: Vec2::ONE,
                    heights: [0.0; 4],
                    center: Vec3::ZERO,
                    links: Vec::new(),
                    place: None,
                    hiding: Vec::new(),
                    visible: Vec::new(),
                    encounters: Vec::new(),
                })
                .collect(),
            generated: true,
            ..default()
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lobby-navcache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// A mesh written reads back the same; another key, a broken file or
    /// a map without a hash is no mesh.
    #[test]
    fn a_kept_mesh_reads_back() {
        let dir = scratch("back");
        let map = MapFile { name: "test:m".into(), hash: Some([7; 32]) };
        let p = NavGenParams::default();
        let k = key(&map, &[Vec3::X, Vec3::Z], &p, true, "v1").unwrap();
        assert_eq!(key(&map, &[Vec3::Z, Vec3::X], &p, true, "v1").as_deref(), Some(k.as_str()), "seeds in any order");
        assert_ne!(key(&map, &[Vec3::X], &p, true, "v1").unwrap(), k);
        assert_ne!(key(&map, &[Vec3::X, Vec3::Z], &p, false, "v1").unwrap(), k);
        assert_ne!(key(&map, &[Vec3::X, Vec3::Z], &p, true, "v2").unwrap(), k);
        assert!(key(&MapFile { name: "test:m".into(), hash: None }, &[], &p, true, "v1").is_none());
        assert!(read(&dir, &k).is_none());
        write(&dir, &k, &mesh(5), CAP);
        assert!(has(&dir, &k));
        let back = read(&dir, &k).unwrap();
        assert_eq!(back.areas.len(), 5);
        assert!(back.generated);
        std::fs::write(path(&dir, &k), b"CHINAV\0\x01junk").unwrap();
        assert!(read(&dir, &k).is_none(), "a broken entry is a miss");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Past the cap the least recently used go.
    #[test]
    fn the_cap_keeps_the_most_recently_used() {
        let dir = scratch("cap");
        let one = postcard::to_allocvec(&mesh(200)).unwrap().len() as u64 + MAGIC.len() as u64;
        let cap = one * 2 + one / 2;
        let at = |s| std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s);
        for (i, k) in ["a", "b"].iter().enumerate() {
            write(&dir, k, &mesh(200), cap);
            std::fs::File::options().append(true).open(path(&dir, k)).unwrap().set_modified(at(1000 + i as u64)).unwrap();
        }
        // "a" read last: "b" is the least recently used.
        assert!(read(&dir, "a").is_some());
        write(&dir, "c", &mesh(200), cap);
        assert!(has(&dir, "a") && has(&dir, "c"), "the recent ones stay");
        assert!(!has(&dir, "b"), "the least recently used went");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
