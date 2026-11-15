//! The record store on disk: `records/<ab>/<id>.zst` (a file's plain bytes,
//! zstd level 19, `LEVEL`; records packed before 2026-11-15 are level 3
//! until `repack`ed, and read the same; `id` = lowercase hex SHA-256 of the plain bytes) and
//! `manifests/<game>/<build>.txt`. Writes are atomic (a temporary file,
//! then a rename), so packers running at once never leave half a record;
//! a record already there is never written again (dedup by id). A fetched
//! record (`put_stored`) is not stored when the disk is below the
//! free-space floor (`disk`).

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::disk;

/// zstd level for records: 19 (docs/performance.md, "First join
/// download, 2026-11-14": 11% smaller than level 3 for the same decode
/// cost, about 2 ms a plain MB; packing costs about 0.4 s a plain MB on
/// one thread, once per publish). A record's id is its plain bytes'
/// hash, so a record of any level reads the same (`unpack`).
pub const LEVEL: i32 = 19;

/// The lowercase hex SHA-256 of `bytes`: a record's id.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Whether `id` looks like a record id (64 lowercase hex characters).
pub fn is_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A store folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    pub root: PathBuf,
}

/// What `put` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Put {
    pub id: String,
    /// The record's size on disk (compressed).
    pub stored: u64,
    /// False: the record was there already (dedup).
    pub new: bool,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The record file of `id`.
    pub fn record_path(&self, id: &str) -> PathBuf {
        self.root.join("records").join(&id[..2.min(id.len())]).join(format!("{id}.zst"))
    }

    /// The folder of `game`'s manifests.
    pub fn manifest_dir(&self, game: &str) -> PathBuf {
        self.root.join("manifests").join(game)
    }

    /// The manifest file of `game` at `build`.
    pub fn manifest_path(&self, game: &str, build: &str) -> PathBuf {
        self.manifest_dir(game).join(format!("{build}.txt"))
    }

    pub fn has(&self, id: &str) -> bool {
        self.record_path(id).is_file()
    }

    /// Store `bytes` as a record (unless one with its id is there).
    pub fn put(&self, bytes: &[u8]) -> io::Result<Put> {
        let id = sha256_hex(bytes);
        let path = self.record_path(&id);
        if let Ok(meta) = fs::metadata(&path) {
            return Ok(Put { id, stored: meta.len(), new: false });
        }
        let packed = zstd::bulk::compress(bytes, LEVEL)?;
        write_atomic(&path, &packed)?;
        Ok(Put { id, stored: packed.len() as u64, new: true })
    }

    /// Store a record as it came from a server (`packed`: its zstd bytes),
    /// after checking it is record `id` of `size` plain bytes; returns the
    /// plain bytes. A record that isn't what it claims (tampered with,
    /// wrong, cut short) is `InvalidData` and never stored.
    pub fn put_stored(&self, id: &str, size: u64, packed: &[u8]) -> io::Result<Vec<u8>> {
        self.put_stored_with(id, size, packed, &disk::space)
    }

    /// `put_stored` with the free-space reader: below the floor
    /// (`disk::has_room`) the record is checked and returned but not
    /// stored, with a one-line warning.
    pub fn put_stored_with(&self, id: &str, size: u64, packed: &[u8], read: disk::Reader) -> io::Result<Vec<u8>> {
        if !is_id(id) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{id:?} is not a record id")));
        }
        let plain = unpack(id, size, packed)?;
        let path = self.record_path(id);
        if !path.is_file() {
            if disk::has_room(&self.root, packed.len() as u64, read) {
                write_atomic(&path, packed)?;
            } else {
                disk::warn_once("asset store", || {
                    format!(
                        "not storing fetched records in {}: less than the free-space floor would be left on its disk ({} sets it)",
                        self.root.display(),
                        disk::FLOOR_ENV
                    )
                });
            }
        }
        Ok(plain)
    }

    /// Record `id`'s plain bytes, checked: exactly `size` bytes whose
    /// SHA-256 is `id`, else `InvalidData` (a corrupt or wrong record);
    /// `NotFound` when the store lacks it.
    pub fn get(&self, id: &str, size: u64) -> io::Result<Vec<u8>> {
        let path = self.record_path(id);
        let packed = fs::read(&path).map_err(|e| io::Error::new(e.kind(), format!("record {id}: {e}")))?;
        unpack(id, size, &packed)
    }

    /// Record `id` packed again at `LEVEL` (checked as `size` plain bytes)
    /// when that is smaller, written beside it and renamed in (a hard
    /// link to another store's record is replaced, never written
    /// through). Its stored size before and after.
    pub fn repack(&self, id: &str, size: u64) -> io::Result<(u64, u64)> {
        let path = self.record_path(id);
        let packed = fs::read(&path).map_err(|e| io::Error::new(e.kind(), format!("record {id}: {e}")))?;
        let plain = unpack(id, size, &packed)?;
        let again = zstd::bulk::compress(&plain, LEVEL)?;
        if again.len() >= packed.len() {
            return Ok((packed.len() as u64, packed.len() as u64));
        }
        write_atomic(&path, &again)?;
        Ok((packed.len() as u64, again.len() as u64))
    }

    /// `manifest`'s stored sizes set to its records' sizes in this store
    /// (after a `repack`); how many changed. Records the store lacks keep
    /// theirs.
    pub fn restat(&self, manifest: &mut crate::Manifest) -> usize {
        let mut changed = 0;
        for f in manifest.files.values_mut() {
            if let Ok(meta) = fs::metadata(self.record_path(&f.id))
                && meta.len() != f.stored
            {
                f.stored = meta.len();
                changed += 1;
            }
        }
        changed
    }

    /// Every record id in the store, sorted.
    pub fn record_ids(&self) -> Vec<String> {
        let mut out = Vec::new();
        let Ok(dirs) = fs::read_dir(self.root.join("records")) else {
            return out;
        };
        for dir in dirs.flatten() {
            let Ok(files) = fs::read_dir(dir.path()) else { continue };
            for f in files.flatten() {
                let name = f.file_name().to_string_lossy().into_owned();
                if let Some(id) = name.strip_suffix(".zst").filter(|id| is_id(id)) {
                    out.push(id.to_string());
                }
            }
        }
        out.sort();
        out
    }

    /// The games with manifests, sorted.
    pub fn games(&self) -> Vec<String> {
        let mut out: Vec<String> = fs::read_dir(self.root.join("manifests"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        out.sort();
        out
    }

    /// `game`'s builds with a manifest, oldest first (numeric builds by
    /// number, then the rest by name).
    pub fn builds(&self, game: &str) -> Vec<String> {
        let mut out: Vec<String> = fs::read_dir(self.manifest_dir(game))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".txt").map(String::from))
            .collect();
        sort_builds(&mut out);
        out
    }

    /// `game`'s newest build (`builds`' last), if it has a manifest.
    pub fn latest_build(&self, game: &str) -> Option<String> {
        self.builds(game).pop()
    }
}

/// Record `id`'s plain bytes from its stored (zstd) bytes, checked: exactly
/// `size` bytes whose SHA-256 is `id`, else `InvalidData`.
pub fn unpack(id: &str, size: u64, packed: &[u8]) -> io::Result<Vec<u8>> {
    let bad = |what: String| io::Error::new(io::ErrorKind::InvalidData, format!("record {id}: {what}"));
    let size = usize::try_from(size).map_err(|_| bad("too large".into()))?;
    // One byte more than wanted, so a longer record shows as one.
    let plain = zstd::bulk::decompress(packed, size + 1).map_err(|e| bad(format!("not zstd ({e})")))?;
    if plain.len() != size {
        return Err(bad(format!("{} bytes, the manifest says {size}", plain.len())));
    }
    if sha256_hex(&plain) != id {
        return Err(bad("hash mismatch (corrupt or tampered record)".into()));
    }
    Ok(plain)
}

/// Builds oldest first: numeric builds by number, then the rest by name.
pub fn sort_builds(builds: &mut [String]) {
    builds.sort_by_key(|b| (b.parse::<u64>().map_or(1, |_| 0), b.parse::<u64>().unwrap_or(0), b.clone()));
}

/// Whether `name` is safe as a game or build name in a path or URL
/// (letters, digits, `_`, `-`, `.`; not empty, not `.` or `..`).
pub fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// Lowercase hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `hex`'s bytes back, if `text` is hex of exactly `N` bytes.
pub fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Write `bytes` to `path` through a temporary file beside it.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.{}.{:?}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        std::thread::current().id()
    ));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fetched_record_is_not_stored_below_the_free_space_floor() {
        let root = std::env::temp_dir().join(format!("chimera_store_floor_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = Store::new(&root);
        let plain = b"some record".to_vec();
        let id = sha256_hex(&plain);
        let packed = zstd::bulk::compress(&plain, LEVEL).unwrap();
        let gb = 1_000_000_000u64;
        let disk_with = |free: u64| move |_: &Path| Some(disk::Space { free, total: 1000 * gb });
        let got = store.put_stored_with(&id, plain.len() as u64, &packed, &disk_with(gb)).unwrap();
        assert_eq!(got, plain, "still loads");
        assert!(!store.has(&id), "not stored below the floor");
        store.put_stored_with(&id, plain.len() as u64, &packed, &disk_with(500 * gb)).unwrap();
        assert!(store.has(&id));
        // A record that isn't what it claims is refused whatever the space.
        assert!(store.put_stored_with(&id, 3, &packed, &disk_with(500 * gb)).is_err());
        fs::remove_dir_all(&root).ok();
    }

    /// A record packed at level 3 (before `LEVEL` was 19) and repacked
    /// reads back the same bytes; the repack is smaller, replaces a hard
    /// link without touching the other store's file, and the manifest's
    /// stored size follows it (`restat`).
    #[test]
    fn a_level_3_record_repacked_at_level_19_reads_the_same() {
        assert_eq!(LEVEL, 19);
        let root = std::env::temp_dir().join(format!("chimera_store_repack_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (old, new) = (Store::new(root.join("old")), Store::new(root.join("new")));
        // Text-like bytes zstd shrinks more at 19 than at 3.
        let plain: Vec<u8> = (0..200_000u32)
            .flat_map(|i| format!("entry {} value {}\n", i % 977, (i * 7919) % 104_729).into_bytes())
            .collect();
        let id = sha256_hex(&plain);
        let level3 = zstd::bulk::compress(&plain, 3).unwrap();
        write_atomic(&old.record_path(&id), &level3).unwrap();
        fs::create_dir_all(new.record_path(&id).parent().unwrap()).unwrap();
        fs::hard_link(old.record_path(&id), new.record_path(&id)).unwrap();
        let mut m = crate::Manifest::new("g", "1", None);
        m.files.insert(
            "data/a.txt".into(),
            crate::FileRecord { id: id.clone(), size: plain.len() as u64, stored: level3.len() as u64 },
        );
        let (before, after) = new.repack(&id, plain.len() as u64).unwrap();
        assert_eq!(before, level3.len() as u64);
        assert!(after < before, "{after} < {before}");
        assert_eq!(new.get(&id, plain.len() as u64).unwrap(), plain, "the same bytes back");
        assert_eq!(fs::read(old.record_path(&id)).unwrap(), level3, "the linked store's record untouched");
        assert_eq!(new.restat(&mut m), 1);
        assert_eq!(m.files["data/a.txt"].stored, after);
        // Packed again: no smaller, left as it is.
        assert_eq!(new.repack(&id, plain.len() as u64).unwrap(), (after, after));
        // A record put now is level 19 already.
        let put = Store::new(root.join("put")).put(&plain).unwrap();
        assert_eq!(put.stored, after);
        fs::remove_dir_all(&root).ok();
    }
}
