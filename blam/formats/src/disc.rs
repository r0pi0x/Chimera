//! The Xbox disc image's file system (specs/halo_ce/disc_image.md): a
//! volume descriptor, directory tables linked as binary trees, files as
//! runs of 2048-byte sectors, read from an image file (never loaded whole)
//! or from bytes, and offered as a `chimera_vfs::FileSource`.
//!
//! The whole directory tree is read once at open into a table of
//! normalized paths (lower case, `/`) with each file's place; a read then
//! seeks to the file and reads exactly its bytes. Every refusal the spec
//! lists is an `io::Error` (`InvalidData` for a corrupt image,
//! `InvalidInput` for a bad path, `NotFound` for a missing one); nothing
//! panics or loops on corrupt input.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
};

use chimera_vfs::{Entry, FileSource, LooseDir, Mount, bytes::Le, normalize};

/// The only sector size.
pub const SECTOR_SIZE: u64 = 2048;
/// The sector of the volume descriptor, counted from the partition base.
pub const VOLUME_SECTOR: u64 = 32;
/// The text at both ends of the volume descriptor.
pub const VOLUME_MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";
/// The fixed part of a directory entry.
pub const ENTRY_HEADER: usize = 14;
/// The attribute bit of a directory (the only bit a reader looks at).
pub const ATTR_DIRECTORY: u8 = 0x10;
/// The deepest directory nesting a reader accepts.
pub const MAX_DEPTH: usize = 64;
/// Where the partition may start in the image file, in the order probed
/// (the spec's "Where the partition starts"; only the first is verified).
pub const PARTITION_BASES: [u64; 5] = [0x0000_0000, 0x1830_0000, 0x0FD9_0000, 0x0209_0000, 0x89D8_0000];

/// Where the volume descriptor is, from the partition base.
const VOLUME_OFFSET: u64 = VOLUME_SECTOR * SECTOR_SIZE;
/// Where the closing magic is in the descriptor.
const CLOSING_MAGIC: u64 = 0x7EC;
/// A link is a u16 in units of 4 bytes, so an entry starts within this
/// many bytes of its table's start; a table is never read past it.
const MAX_ENTRY_START: u64 = 0xFFFF * 4;
/// What of a table can hold an entry: the farthest start plus the longest
/// entry.
const TABLE_WINDOW: u64 = MAX_ENTRY_START + ENTRY_HEADER as u64 + 255;

/// One entry of a directory, as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    /// The name with the disc's own casing (ASCII, or Latin-1 when a byte
    /// is above 0x7F).
    pub name: String,
    pub is_dir: bool,
    /// Bytes of the file, or of the subdirectory's table.
    pub size: u64,
}

/// Where a file's bytes are, from the start of the image file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Extent {
    offset: u64,
    size: u64,
}

/// What the image's bytes come from.
enum Backing {
    File { path: PathBuf, stamp: String },
    Bytes(Arc<[u8]>),
}

/// A disc image, parsed: the file table of the whole tree.
pub struct DiscImage {
    name: String,
    backing: Backing,
    len: u64,
    base: u64,
    volume_time: u64,
    /// Normalized path to place, for every file.
    files: BTreeMap<String, Extent>,
    /// Normalized directory path ("" for the root) to its entries, in
    /// folded-name order (a left-to-right walk of the tree).
    dirs: BTreeMap<String, Vec<DirEntry>>,
}

/// What `DiscImage::resolve` found.
enum Node<'a> {
    Dir(&'a [DirEntry]),
    File(Extent),
}

impl DiscImage {
    /// Open an image file: probe the partition base, read the volume
    /// descriptor and every directory table. File data is not read.
    pub fn open(path: &Path) -> io::Result<Self> {
        let name = path.display().to_string();
        let ctx = |e: io::Error| io::Error::new(e.kind(), format!("{name}: {e}"));
        let meta = std::fs::metadata(path).map_err(ctx)?;
        if !meta.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name}: not a file"),
            ));
        }
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
        let stamp = format!("{} {}", meta.len(), mtime.map_or(0, |t| t.as_nanos()));
        let backing = Backing::File {
            path: path.to_path_buf(),
            stamp,
        };
        Self::parse(name, backing, meta.len())
    }

    /// Open an image held in memory (tests).
    pub fn from_bytes(bytes: Arc<[u8]>) -> io::Result<Self> {
        let len = bytes.len() as u64;
        Self::parse("disc image (memory)".into(), Backing::Bytes(bytes), len)
    }

    /// The byte offset in the image file where the game partition starts.
    pub fn base(&self) -> u64 {
        self.base
    }

    /// The volume's creation time, 100 ns ticks since 1601-01-01 UTC.
    pub fn volume_time(&self) -> u64 {
        self.volume_time
    }

    /// The image's length in bytes.
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Directories, not counting the root.
    pub fn dir_count(&self) -> usize {
        self.dirs.len() - 1
    }

    /// The entries of a directory in folded-name order (a walk of the
    /// tree), names as stored. `""` is the root. Path rules as in the
    /// spec: either separator, case-insensitive, `..` refused; a file is
    /// an error, a missing directory `NotFound`.
    pub fn list(&self, dir: &str) -> io::Result<&[DirEntry]> {
        match self.resolve(dir)? {
            Node::Dir(entries) => Ok(entries),
            Node::File(_) => Err(self.err(io::ErrorKind::InvalidInput, format!("{dir}: a file, not a directory"))),
        }
    }

    /// The bytes of a file (path rules as `list`); a directory is an error.
    pub fn read_file(&self, path: &str) -> io::Result<Vec<u8>> {
        match self.resolve(path)? {
            Node::File(extent) => self.read_extent(extent),
            Node::Dir(_) => Err(self.err(io::ErrorKind::InvalidInput, format!("{path}: a directory, not a file"))),
        }
    }

    /// Where a file's bytes are in the image file: offset and size. `None`
    /// for anything that is not a file there (or a refused path).
    pub fn extent(&self, path: &str) -> Option<(u64, u64)> {
        match self.resolve(path) {
            Ok(Node::File(e)) => Some((e.offset, e.size)),
            _ => None,
        }
    }

    fn err(&self, kind: io::ErrorKind, msg: String) -> io::Error {
        io::Error::new(kind, format!("{}: {msg}", self.name))
    }

    /// Follow `path` component by component under the spec's path rules.
    fn resolve(&self, path: &str) -> io::Result<Node<'_>> {
        let mut at = String::new();
        let mut node = Node::Dir(&self.dirs[""]);
        for part in path.replace('\\', "/").split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                return Err(self.err(io::ErrorKind::InvalidInput, format!("{path}: \"..\" is refused")));
            }
            if matches!(node, Node::File(_)) {
                return Err(self.err(
                    io::ErrorKind::InvalidInput,
                    format!("{path}: a file used as a directory"),
                ));
            }
            if !at.is_empty() {
                at.push('/');
            }
            at.push_str(&normalize(part));
            node = if let Some(entries) = self.dirs.get(&at) {
                Node::Dir(entries)
            } else if let Some(extent) = self.files.get(&at) {
                Node::File(*extent)
            } else {
                return Err(self.err(io::ErrorKind::NotFound, format!("{path}: not found")));
            };
        }
        Ok(node)
    }

    fn read_extent(&self, e: Extent) -> io::Result<Vec<u8>> {
        if e.size == 0 {
            return Ok(Vec::new());
        }
        self.read_at(e.offset, e.size)
    }

    /// Exactly `len` bytes at `offset`; a short read is an error.
    fn read_at(&self, offset: u64, len: u64) -> io::Result<Vec<u8>> {
        read_at(&self.backing, self.len, &self.name, offset, len)
    }

    fn parse(name: String, backing: Backing, len: u64) -> io::Result<Self> {
        let bad = |msg: String| io::Error::new(io::ErrorKind::InvalidData, format!("{name}: {msg}"));
        let rd = |offset: u64, n: u64| read_at(&backing, len, &name, offset, n);

        // The partition base: the first candidate with the opening magic,
        // which must then have the closing magic too.
        let mut base = None;
        for &candidate in &PARTITION_BASES {
            let at = candidate + VOLUME_OFFSET;
            if at.checked_add(SECTOR_SIZE).is_none_or(|end| end > len) {
                continue;
            }
            if rd(at, 20)? == VOLUME_MAGIC {
                base = Some(candidate);
                break;
            }
        }
        let Some(base) = base else {
            return Err(bad(
                "not an Xbox disc image (no volume descriptor at any known partition base)".into(),
            ));
        };
        let descriptor = rd(base + VOLUME_OFFSET, SECTOR_SIZE)?;
        if &descriptor[CLOSING_MAGIC as usize..CLOSING_MAGIC as usize + 20] != VOLUME_MAGIC {
            return Err(bad(format!(
                "volume descriptor at partition base {base:#x} has no closing magic"
            )));
        }
        let mut le = Le::new(&descriptor);
        le.take(20)?;
        let root_sector = le.u32()?;
        let root_size = le.u32()?;
        let volume_time = u64::from_le_bytes(le.array()?);

        let room = len - base;
        let mut files = BTreeMap::new();
        let mut dirs: BTreeMap<String, Vec<DirEntry>> = BTreeMap::new();
        let mut taken: BTreeSet<String> = BTreeSet::new();
        let mut tables: BTreeSet<u32> = BTreeSet::new();
        // Directories still to read: normalized path, table sector and
        // size, depth (the root is 0).
        let mut todo: Vec<(String, u32, u32, usize)> = vec![(String::new(), root_sector, root_size, 0)];
        while let Some((dir, sector, size, depth)) = todo.pop() {
            let what = if dir.is_empty() {
                "the root directory".to_string()
            } else {
                format!("directory {dir}")
            };
            if depth > MAX_DEPTH {
                return Err(bad(format!("{what}: nested more than {MAX_DEPTH} levels deep")));
            }
            let mut listing = Vec::new();
            if size != 0 {
                let start = u64::from(sector) * SECTOR_SIZE;
                if start.checked_add(u64::from(size)).is_none_or(|end| end > room) {
                    return Err(bad(format!(
                        "{what}: table at sector {sector}, {size} bytes, passes the image's end"
                    )));
                }
                if !tables.insert(sector) {
                    return Err(bad(format!(
                        "{what}: table at sector {sector} is used by two directories"
                    )));
                }
                let window = u64::from(size).min(TABLE_WINDOW);
                let table = rd(base + start, window)?;
                for raw in parse_table(&table, u64::from(size)).map_err(|m| bad(format!("{what}: {m}")))? {
                    let key = if dir.is_empty() {
                        normalize(&raw.name)
                    } else {
                        format!("{dir}/{}", normalize(&raw.name))
                    };
                    if !taken.insert(key.clone()) {
                        return Err(bad(format!("{what}: two names fold to {key}")));
                    }
                    let is_dir = raw.attr & ATTR_DIRECTORY != 0;
                    if is_dir {
                        todo.push((key, raw.sector, raw.size, depth + 1));
                    } else if raw.size != 0 {
                        let offset = u64::from(raw.sector) * SECTOR_SIZE;
                        if offset.checked_add(u64::from(raw.size)).is_none_or(|end| end > room) {
                            return Err(bad(format!(
                                "{what}: file {}: sector {}, {} bytes, passes the image's end",
                                raw.name, raw.sector, raw.size
                            )));
                        }
                        files.insert(
                            key,
                            Extent {
                                offset: base + offset,
                                size: u64::from(raw.size),
                            },
                        );
                    } else {
                        files.insert(key, Extent { offset: 0, size: 0 });
                    }
                    listing.push(DirEntry {
                        name: raw.display,
                        is_dir,
                        size: u64::from(raw.size),
                    });
                }
            }
            dirs.insert(dir, listing);
        }
        Ok(Self {
            name,
            backing,
            len,
            base,
            volume_time,
            files,
            dirs,
        })
    }
}

/// Exactly `n` bytes at `offset` of the image.
fn read_at(backing: &Backing, len: u64, name: &str, offset: u64, n: u64) -> io::Result<Vec<u8>> {
    let short = || {
        io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("{name}: the image ends before byte {offset} + {n}"),
        )
    };
    if offset.checked_add(n).is_none_or(|end| end > len) {
        return Err(short());
    }
    let n = usize::try_from(n).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name}: {n} bytes do not fit in memory"),
        )
    })?;
    match backing {
        Backing::Bytes(bytes) => {
            let start = offset as usize;
            bytes.get(start..start + n).map(<[u8]>::to_vec).ok_or_else(short)
        }
        Backing::File { path, .. } => {
            let ctx = |e: io::Error| io::Error::new(e.kind(), format!("{name}: {e}"));
            let mut f = File::open(path).map_err(ctx)?;
            f.seek(SeekFrom::Start(offset)).map_err(ctx)?;
            let mut out = vec![0; n];
            f.read_exact(&mut out).map_err(|e| {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    short()
                } else {
                    ctx(e)
                }
            })?;
            Ok(out)
        }
    }
}

/// One table entry, as stored.
struct Raw {
    left: u16,
    right: u16,
    sector: u32,
    size: u32,
    attr: u8,
    /// The name decoded for display and lookup.
    name: String,
    display: String,
}

/// The entries of one directory table in tree order (left subtree, node,
/// right subtree), checking everything the spec's safety list asks. `data`
/// is the table's start (at least what its links can reach), `size` the
/// table's size in bytes.
fn parse_table(data: &[u8], size: u64) -> Result<Vec<Raw>, String> {
    let max_nodes = size / 4;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut stack: Vec<Raw> = Vec::new();
    let mut next = Some(0u64);
    loop {
        while let Some(at) = next {
            if !seen.insert(at) {
                return Err(format!("the tree loops back to byte {at}"));
            }
            if seen.len() as u64 > max_nodes {
                return Err(format!("more entries than {max_nodes} (the table's size / 4)"));
            }
            let entry = entry_at(data, size, at)?;
            next = link(entry.left);
            stack.push(entry);
        }
        let Some(entry) = stack.pop() else { break };
        next = link(entry.right);
        out.push(entry);
    }
    Ok(out)
}

/// A link in bytes from the table start; 0 means none.
fn link(units: u16) -> Option<u64> {
    (units != 0).then(|| u64::from(units) * 4)
}

fn entry_at(data: &[u8], size: u64, at: u64) -> Result<Raw, String> {
    if at + ENTRY_HEADER as u64 > size {
        return Err(format!("an entry at byte {at} passes the table's end ({size} bytes)"));
    }
    let rest = usize::try_from(at)
        .ok()
        .and_then(|a| data.get(a..))
        .ok_or("an entry outside the data read")?;
    let mut le = Le::new(rest);
    let left = le.u16().map_err(String::from)?;
    let right = le.u16().map_err(String::from)?;
    let sector = le.u32().map_err(String::from)?;
    let file_size = le.u32().map_err(String::from)?;
    let attr = le.u8().map_err(String::from)?;
    let n = le.u8().map_err(String::from)?;
    if left == 0xFFFF && right == 0xFFFF && sector == 0xFFFF_FFFF {
        return Err(format!("byte {at} is fill, not an entry"));
    }
    if at + ENTRY_HEADER as u64 + u64::from(n) > size {
        return Err(format!("the entry at byte {at} passes the table's end ({size} bytes)"));
    }
    let bytes = le.take(usize::from(n)).map_err(String::from)?;
    if bytes.is_empty() {
        return Err(format!("the entry at byte {at} has an empty name"));
    }
    if bytes.iter().any(|&b| b == 0 || b == b'/' || b == b'\\') {
        return Err(format!("the entry at byte {at} has a NUL or a separator in its name"));
    }
    // ASCII, or Latin-1 when any byte is above 0x7F.
    let display: String = bytes.iter().map(|&b| char::from(b)).collect();
    Ok(Raw {
        left,
        right,
        sector,
        size: file_size,
        attr,
        name: display.clone(),
        display,
    })
}

/// The tree's order: bytes compared after folding a-z to A-Z, a prefix
/// smaller than what it begins. (Upper case, not lower: `_` and the other
/// characters between `Z` and `a` sort after the letters.)
pub fn fold_cmp(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let fold = |s: &[u8]| s.iter().map(|c| c.to_ascii_uppercase()).collect::<Vec<_>>();
    fold(a).cmp(&fold(b))
}

impl FileSource for DiscImage {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn entries(&self) -> Vec<Entry> {
        self.files
            .iter()
            .map(|(path, e)| Entry {
                path: path.clone(),
                size: e.size,
            })
            .collect()
    }

    fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        let Some(entries) = self.dirs.get(dir) else {
            return Vec::new();
        };
        entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| {
                if dir.is_empty() {
                    normalize(&e.name)
                } else {
                    format!("{dir}/{}", normalize(&e.name))
                }
            })
            .collect()
    }

    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        let extent = *self.files.get(path)?;
        Some(self.read_extent(extent))
    }

    /// The image file's size and time with the file's place and size;
    /// `None` for an image in memory (hashed instead).
    fn fingerprint(&self, path: &str) -> Option<String> {
        let e = self.files.get(path)?;
        let Backing::File { stamp, .. } = &self.backing else {
            return None;
        };
        Some(format!("xiso {stamp} {} {}", e.offset, e.size))
    }
}

/// Mount a Halo install: `path` is a disc image file (one layer, the
/// image) or a folder holding the disc's files (a `LooseDir`).
pub fn open_install(path: &Path) -> io::Result<Mount> {
    let mut mount = Mount::default();
    if path.is_dir() {
        mount.push(LooseDir::new(path));
    } else if path.is_file() {
        mount.push(DiscImage::open(path)?);
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: neither a disc image file nor a folder", path.display()),
        ));
    }
    Ok(mount)
}
