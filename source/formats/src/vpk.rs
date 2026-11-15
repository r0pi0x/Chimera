//! Valve VPK archives, versions 1 and 2, from the public format description
//! (developer.valvesoftware.com/wiki/VPK_File_Format).
//!
//! A `<name>_dir.vpk` holds a directory tree and sometimes file data; most
//! data lives in numbered siblings `<name>_000.vpk`, `<name>_001.vpk`, ...
//!
//! A numbered archive the tree names but that isn't on disk leaves its
//! files out (`Vpk::open`), file by file: a mount then finds them in a
//! later layer or not at all, never "listed but unreadable". TF2's
//! dedicated server ships its textures' and sounds' `_dir` files without
//! their archives (specs/tf2/install.md 1.1, 1.5).

use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use chimera_vfs::{Entry, FileSource, bytes::Le, normalize};

const SIGNATURE: u32 = 0x55aa_1234;
/// Archive index meaning "data follows the tree in the _dir file".
const IN_DIR_FILE: u16 = 0x7fff;

#[derive(Clone, Debug)]
struct VpkEntry {
    crc: u32,
    preload: Vec<u8>,
    archive: u16,
    offset: u32,
    length: u32,
}

pub struct Vpk {
    dir_path: PathBuf,
    /// Path prefix of the numbered archives: `.../cstrike_pak` for
    /// `.../cstrike_pak_dir.vpk`.
    stem: PathBuf,
    /// Offset of in-dir file data: header plus tree.
    data_start: u64,
    entries: HashMap<String, VpkEntry>,
    /// Files the tree lists in numbered archives missing on disk, left out.
    skipped: usize,
}

/// The shared reader with VPK's error shape.
trait VpkRead {
    fn bad(&self, what: &str) -> io::Error;
    fn cstr_io(&mut self) -> io::Result<String>;
}

impl VpkRead for Le<'_> {
    fn bad(&self, what: &str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, format!("VPK: {what} at byte {}", self.pos))
    }

    fn cstr_io(&mut self) -> io::Result<String> {
        self.cstr().ok_or_else(|| self.bad("unterminated string"))
    }
}

impl Vpk {
    pub fn open(dir_path: impl AsRef<Path>) -> io::Result<Self> {
        let dir_path = dir_path.as_ref().to_path_buf();
        let file_name = dir_path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let stem_name = file_name.strip_suffix("_dir.vpk").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a _dir.vpk: {}", dir_path.display()),
            )
        })?;
        let stem = dir_path.with_file_name(stem_name);

        let mut file = File::open(&dir_path)?;
        let mut header = [0u8; 12];
        file.read_exact(&mut header)?;
        let mut c = Le::new(&header);
        if c.u32()? != SIGNATURE {
            return Err(c.bad("bad signature"));
        }
        let version = c.u32()?;
        let tree_size = c.u32()? as usize;
        let header_size = match version {
            1 => 12,
            2 => 28,
            v => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("VPK: unsupported version {v}"),
                ));
            }
        };
        file.seek(SeekFrom::Start(header_size))?;
        let mut tree = vec![0u8; tree_size];
        file.read_exact(&mut tree)?;

        let mut c = Le::new(&tree);
        let mut entries = HashMap::new();
        // Tree: extension { directory { file entry }* }* ; each level ends
        // with an empty string. A single space means "none".
        loop {
            let ext = c.cstr_io()?;
            if ext.is_empty() {
                break;
            }
            loop {
                let dir = c.cstr_io()?;
                if dir.is_empty() {
                    break;
                }
                loop {
                    let name = c.cstr_io()?;
                    if name.is_empty() {
                        break;
                    }
                    let crc = c.u32()?;
                    let preload_len = c.u16()? as usize;
                    let archive = c.u16()?;
                    let offset = c.u32()?;
                    let length = c.u32()?;
                    if c.u16()? != 0xffff {
                        return Err(c.bad("bad entry terminator"));
                    }
                    let preload = c.take(preload_len)?.to_vec();
                    let mut path = String::new();
                    if dir != " " {
                        path.push_str(&dir);
                        path.push('/');
                    }
                    path.push_str(&name);
                    if ext != " " {
                        path.push('.');
                        path.push_str(&ext);
                    }
                    entries.insert(
                        normalize(&path),
                        VpkEntry {
                            crc,
                            preload,
                            archive,
                            offset,
                            length,
                        },
                    );
                }
            }
        }

        // Files whose data lies in a numbered archive that isn't there are
        // left out (a file wholly in the tree's preload stays).
        let mut present: HashMap<u16, bool> = HashMap::new();
        let before = entries.len();
        entries.retain(|_, e| {
            if e.archive == IN_DIR_FILE || e.length == 0 {
                return true;
            }
            *present
                .entry(e.archive)
                .or_insert_with(|| archive_path(&stem, e.archive).is_file())
        });
        let skipped = before - entries.len();

        Ok(Self {
            dir_path,
            stem,
            data_start: header_size + tree_size as u64,
            entries,
            skipped,
        })
    }

    /// How many files the tree lists in numbered archives that are missing
    /// on disk (left out of the archive).
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// CRC32 the archive records for `path`, for integrity checks.
    pub fn crc(&self, path: &str) -> Option<u32> {
        self.entries.get(&normalize(path)).map(|e| e.crc)
    }

    fn read_entry(&self, e: &VpkEntry) -> io::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(e.preload.len() + e.length as usize);
        out.extend_from_slice(&e.preload);
        if e.length > 0 {
            let (path, offset) = if e.archive == IN_DIR_FILE {
                (self.dir_path.clone(), self.data_start + e.offset as u64)
            } else {
                (archive_path(&self.stem, e.archive), e.offset as u64)
            };
            let mut f = File::open(&path)?;
            f.seek(SeekFrom::Start(offset))?;
            let start = out.len();
            out.resize(start + e.length as usize, 0);
            f.read_exact(&mut out[start..])?;
        }
        Ok(out)
    }
}

/// Numbered archive `index` beside the directory file: `<stem>_NNN.vpk`.
fn archive_path(stem: &Path, index: u16) -> PathBuf {
    let mut p = stem.to_path_buf().into_os_string();
    p.push(format!("_{index:03}.vpk"));
    PathBuf::from(p)
}

impl FileSource for Vpk {
    fn name(&self) -> String {
        self.dir_path.display().to_string()
    }

    fn entries(&self) -> Vec<Entry> {
        self.entries
            .iter()
            .map(|(path, e)| Entry {
                path: path.clone(),
                size: e.preload.len() as u64 + e.length as u64,
            })
            .collect()
    }

    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        self.entries.get(path).map(|e| self.read_entry(e))
    }

    fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    /// The directory's CRC-32 of the entry, its sizes and where it lies.
    fn fingerprint(&self, path: &str) -> Option<String> {
        let e = self.entries.get(path)?;
        Some(format!(
            "vpk {:08x} {} {} {} {}",
            e.crc,
            e.preload.len(),
            e.length,
            e.archive,
            e.offset
        ))
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        self.entries
            .keys()
            .filter(|p| p.rsplit_once('/').is_some_and(|(d, _)| d == dir))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version 1 directory file listing `files` (path, archive, data):
    /// data of archive 0x7fff follows the tree; any other archive's is at
    /// offset 0 of `<stem>_NNN.vpk`, whose bytes the caller writes.
    fn dir_file(files: &[(&str, u16, &[u8])]) -> Vec<u8> {
        let mut tree = Vec::new();
        let mut tail: Vec<u8> = Vec::new();
        for (path, archive, data) in files {
            let (dir, file) = path.rsplit_once('/').unwrap();
            let (stem, ext) = file.rsplit_once('.').unwrap();
            for s in [ext, dir, stem] {
                tree.extend(s.as_bytes());
                tree.push(0);
            }
            tree.extend(0u32.to_le_bytes());
            tree.extend(0u16.to_le_bytes());
            tree.extend(archive.to_le_bytes());
            let offset = if *archive == IN_DIR_FILE { tail.len() } else { 0 };
            tree.extend((offset as u32).to_le_bytes());
            tree.extend((data.len() as u32).to_le_bytes());
            tree.extend(0xffffu16.to_le_bytes());
            // The end of this directory's names, of this extension's directories.
            tree.extend([0, 0]);
            if *archive == IN_DIR_FILE {
                tail.extend(*data);
            }
        }
        // The end of the extensions.
        tree.push(0);
        let mut v = Vec::new();
        v.extend(SIGNATURE.to_le_bytes());
        v.extend(1u32.to_le_bytes());
        v.extend((tree.len() as u32).to_le_bytes());
        v.extend(tree);
        v.extend(tail);
        v
    }

    #[test]
    fn files_of_a_missing_archive_are_left_out() {
        let dir = std::env::temp_dir().join(format!("vpk-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir_path = dir.join("pak_dir.vpk");
        std::fs::write(
            &dir_path,
            dir_file(&[
                ("materials/a.vmt", IN_DIR_FILE, b"in the tree"),
                ("materials/b.vtf", 0, b"archive zero"),
                ("materials/c.vtf", 1, b"archive one"),
                ("sound/d.wav", 1, b"archive one too"),
            ]),
        )
        .unwrap();
        std::fs::write(dir.join("pak_000.vpk"), b"archive zero").unwrap();
        // pak_001.vpk is missing (a dedicated server's texture archives).
        let vpk = Vpk::open(&dir_path).unwrap();
        assert_eq!(vpk.len(), 2);
        assert_eq!(vpk.skipped(), 2);
        assert!(vpk.contains("materials/a.vmt") && vpk.contains("materials/b.vtf"));
        assert!(!vpk.contains("materials/c.vtf") && vpk.read("sound/d.wav").is_none());
        assert_eq!(vpk.read("materials/a.vmt").unwrap().unwrap(), b"in the tree");
        assert_eq!(vpk.read("materials/b.vtf").unwrap().unwrap(), b"archive zero");
        // With the archive there, its files are back.
        std::fs::write(dir.join("pak_001.vpk"), b"archive one").unwrap();
        let vpk = Vpk::open(&dir_path).unwrap();
        assert_eq!((vpk.len(), vpk.skipped()), (4, 0));
        assert_eq!(vpk.read("materials/c.vtf").unwrap().unwrap(), b"archive one");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
