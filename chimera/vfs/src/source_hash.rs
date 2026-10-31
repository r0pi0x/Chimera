//! A content hash of source folders, for build scripts: the converted-map
//! cache's converter version (chimera_world's `map::cache`) is the hash
//! of the converters' own sources, so any change to them is a new
//! version without anyone bumping a number. Dependency-free (a 128-bit
//! FNV-1a over each file's path, relative to its folder, and bytes, in
//! sorted order), so a game's `build.rs` can use it as a build
//! dependency without building anything else.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
const PRIME: u128 = 0x0000000001000000000000000000013b;

/// A running 128-bit FNV-1a.
#[derive(Clone, Copy, Debug)]
pub struct Fnv(u128);

impl Default for Fnv {
    fn default() -> Self {
        Self(OFFSET)
    }
}

impl Fnv {
    pub fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u128;
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    /// 32 hex digits.
    pub fn hex(&self) -> String {
        format!("{:032x}", self.0)
    }
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// The hash of every file under each of `dirs` (or the file itself, for
/// a path that is one; a missing one counts, by position): names relative to their folder with `/`, each
/// folder labelled by its position, so moving a file or renaming it
/// changes the hash, and where the folders are on disk doesn't.
pub fn hash_dirs(dirs: &[&Path]) -> String {
    let mut h = Fnv::default();
    for (i, dir) in dirs.iter().enumerate() {
        h.write(format!("dir {i}\n").as_bytes());
        let mut list = Vec::new();
        if files(dir, &mut list).is_err() {
            h.write(b"missing\n");
            continue;
        }
        let mut named: Vec<(String, PathBuf)> = list
            .into_iter()
            .filter_map(|p| {
                let rel = match p.strip_prefix(dir).ok()?.to_string_lossy().replace('\\', "/") {
                    r if r.is_empty() => p.file_name()?.to_string_lossy().into_owned(),
                    r => r,
                };
                Some((rel, p))
            })
            .collect();
        named.sort();
        for (rel, path) in named {
            let bytes = fs::read(&path).unwrap_or_default();
            // Text the same whatever the checkout's line endings.
            let bytes: Vec<u8> = bytes.into_iter().filter(|b| *b != b'\r').collect();
            h.write(format!("file {rel} {}\n", bytes.len()).as_bytes());
            h.write(&bytes);
        }
    }
    h.hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_follows_content_names_and_files_only() {
        let root = std::env::temp_dir().join(format!("chimera_source_hash_{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        fs::create_dir_all(a.join("sub")).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("x.rs"), b"fn x() {}\n").unwrap();
        fs::write(a.join("sub").join("y.rs"), b"fn y() {}\n").unwrap();
        let first = hash_dirs(&[&a, &b]);
        assert_eq!(first, hash_dirs(&[&a, &b]), "stable");
        fs::write(a.join("x.rs"), b"fn x() {}\r\n").unwrap();
        assert_eq!(first, hash_dirs(&[&a, &b]), "line endings don't count");
        fs::write(a.join("x.rs"), b"fn x() { }\n").unwrap();
        let changed = hash_dirs(&[&a, &b]);
        assert_ne!(first, changed, "an edit is a new hash");
        fs::write(b.join("z.rs"), b"").unwrap();
        let added = hash_dirs(&[&a, &b]);
        assert_ne!(changed, added, "a new file, even empty, is a new hash");
        fs::rename(b.join("z.rs"), b.join("w.rs")).unwrap();
        assert_ne!(added, hash_dirs(&[&a, &b]), "a rename is a new hash");
        assert_ne!(
            hash_dirs(&[&a]),
            hash_dirs(&[&a, &root.join("none")]),
            "a missing folder counts"
        );
        fs::remove_dir_all(&root).ok();
    }
}
