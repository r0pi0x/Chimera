//! LithTech `.rez` resource archives as Combat Arms writes them, per
//! specs/combat_arms/rez_archive.md (section names below are the spec's):
//! a 236-byte header with an integrity-check block, file data, then a
//! plaintext directory tree at the end of the file. The header and the
//! directory are never encrypted; payloads of archives titled
//! `V1_RESOURCES_KEY` (Twofish) and `V2_RESOURCES_KEY` (AES-256) are
//! (`rez_cipher`) and read only with a key for that scheme, supplied at run
//! time; other titles (`LithTech Resource File`) are stored raw.
//!
//! `RezArchive` is a mount layer (`chimera_vfs::FileSource`, which
//! Chimera's `mount` re-exports): paths are normalized (lowercase, `/`),
//! `<dir>/<name>.<type>`. `open_dir` mounts a folder's archives in name
//! order; a game's mount (Combat Arms: its install's `Game/`) calls it
//! with the keys from its local config.
//!
//! Known shortcuts: every archive is read as a main archive (stock or
//! Combat Arms marker bytes, either version); the game's overlay rules
//! ("Reader (additional overlay archive...)": Combat Arms markers only,
//! replacing names) are not modelled, and across archives the first
//! layer with a path wins (`Mount`). The check sum S is not enforced on
//! open (`RezHeader::check_matches_fields` tells; the spec makes it
//! optional). A title other than the three observed is read raw ("Cipher
//! by title").

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
};

use chimera_vfs::{Entry, FileSource, Mount, normalize};

pub use crate::rez_cipher::{RezCipher, RezCipherKind, RezKeys};

/// "Header": the fixed header length; file data starts right after it.
pub const HEADER_LEN: usize = 0xEC;
/// Titles of archives whose payloads are encrypted; the first two
/// characters name the key scheme (`V1`, `V2`).
pub const KEYED_TITLES: [&str; 2] = ["V1_RESOURCES_KEY", "V2_RESOURCES_KEY"];
/// "Header check block": the XOR between the obfuscated and plain check
/// numbers, and between the check bytes' two copies.
const CHECK_XOR: u32 = 23_807_011;
const CHECK_BYTE_XOR: u8 = 0x11;
/// "Reader (main archive)", rule 6.
const VERSIONS: [u32; 2] = [1, 150_784];
/// "Header": marker bytes as (offset, Combat Arms value, stock value).
const MARKERS: [(usize, u8, u8); 7] = [
    (0x00, 0x26, 0x0D),
    (0x01, 0x23, 0x0A),
    (0x3E, 0x21, 0x0D),
    (0x3F, 0x22, 0x0A),
    (0x7C, 0x25, 0x0D),
    (0x7D, 0x27, 0x0A),
    (0x7E, 0x2A, 0x1A),
];
/// Directory nesting deeper than this is treated as corrupt (a block
/// pointing back at an ancestor would otherwise never end).
const MAX_DEPTH: usize = 64;

fn invalid(name: &str, msg: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{name}: {msg}"))
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// A check number: decimal text up to the first 0x00 of its 32-byte area,
/// signed 32-bit, reinterpreted as unsigned ("Edge cases": negative check
/// numbers; bytes after the 0x00 are leftovers and ignored).
fn check_number(area: &[u8]) -> Option<u32> {
    let end = area.iter().position(|&b| b == 0)?;
    std::str::from_utf8(&area[..end])
        .ok()?
        .trim()
        .parse::<i32>()
        .ok()
        .map(|v| v as u32)
}

/// "Header": the fields a reader uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RezHeader {
    /// The user title, trailing spaces stripped.
    pub title: String,
    pub version: u32,
    pub root_offset: u32,
    pub root_size: u32,
    pub root_time: u32,
    /// The end of file data at pack time, where the directory area starts.
    pub next_write: u32,
    pub archive_time: u32,
    /// File data grouped per directory.
    pub sorted: bool,
    /// The plain check number (0xA2).
    pub check: u32,
}

impl RezHeader {
    /// Parse and validate the first `HEADER_LEN` bytes of a file of
    /// `file_len` bytes by "Reader (main archive)", rules 1 to 7.
    pub fn parse(b: &[u8], file_len: u64, name: &str) -> io::Result<Self> {
        if b.len() < HEADER_LEN || file_len < HEADER_LEN as u64 {
            return Err(invalid(name, "shorter than a rez header"));
        }
        for (at, ca, stock) in MARKERS {
            if b[at] != ca && b[at] != stock {
                return Err(invalid(name, format!("marker byte at {at:#04x} is {:#04x}", b[at])));
            }
        }
        if b[0x7F] ^ CHECK_BYTE_XOR != b[0xA1] || b[0xA0] ^ CHECK_BYTE_XOR != b[0xC2] {
            return Err(invalid(name, "header check bytes don't match"));
        }
        let obfuscated =
            check_number(&b[0x80..0xA0]).ok_or_else(|| invalid(name, "unreadable check number at 0x80"))?;
        let check = check_number(&b[0xA2..0xC2]).ok_or_else(|| invalid(name, "unreadable check number at 0xA2"))?;
        if obfuscated ^ CHECK_XOR != check {
            return Err(invalid(name, "header check numbers don't match"));
        }
        let h = Self {
            title: String::from_utf8_lossy(&b[0x40..0x7C]).trim_end().to_string(),
            version: u32_at(b, 0xC3),
            root_offset: u32_at(b, 0xC7),
            root_size: u32_at(b, 0xCB),
            root_time: u32_at(b, 0xCF),
            next_write: u32_at(b, 0xD3),
            archive_time: u32_at(b, 0xD7),
            sorted: b[0xEB] != 0,
            check,
        };
        if !VERSIONS.contains(&h.version) {
            return Err(invalid(name, format!("unsupported version {}", h.version)));
        }
        if h.root_offset as u64 > file_len || h.root_size as u64 > file_len - h.root_offset as u64 {
            return Err(invalid(name, "root directory block lies outside the file"));
        }
        Ok(h)
    }

    /// Whether the plain check number is the writer's sum S of the root
    /// block's offset, size and time and the next write position.
    pub fn check_matches_fields(&self) -> bool {
        self.root_offset
            .wrapping_add(self.root_size)
            .wrapping_add(self.root_time)
            .wrapping_add(self.next_write)
            == self.check
    }

    /// The key scheme the title names (`V1`, `V2`), if its payloads are
    /// encrypted.
    pub fn key_scheme(&self) -> Option<&'static str> {
        KEYED_TITLES.iter().find(|t| self.title == **t).map(|t| &t[..2])
    }
}

/// "Type code packing": the highest non-zero byte is the extension's first
/// character, down to byte 0; 0 means no extension.
pub fn decode_type(code: u32) -> String {
    code.to_le_bytes()
        .iter()
        .rev()
        .filter(|&&b| b != 0)
        .map(|&b| b as char)
        .collect()
}

/// The inverse of `decode_type` (up to 4 characters, upper-cased), for
/// writing fixtures.
pub fn encode_type(ext: &str) -> u32 {
    let b = ext.to_ascii_uppercase().into_bytes();
    let n = b.len().min(4);
    (0..n).fold(0, |acc, i| acc | (b[i] as u32) << (8 * (n - 1 - i)))
}

/// A file's place in the archive ("File record").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RezFile {
    pub offset: u32,
    pub size: u32,
    pub time: u32,
}

/// Where an archive's bytes are: a file read on demand, or bytes in
/// memory (fixtures).
#[derive(Clone)]
enum Data {
    File(PathBuf),
    Bytes(Arc<[u8]>),
}

impl Data {
    fn len(&self) -> io::Result<u64> {
        match self {
            Data::File(p) => Ok(fs::metadata(p)?.len()),
            Data::Bytes(b) => Ok(b.len() as u64),
        }
    }
}

/// Reads ranges of an archive, keeping the file open while the directory
/// is walked.
enum Reader<'a> {
    File(File),
    Bytes(&'a [u8]),
}

impl Reader<'_> {
    /// `len` bytes at `offset`; the caller has checked the range.
    fn read_at(&mut self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        match self {
            Reader::File(f) => {
                let mut out = vec![0; len];
                f.seek(SeekFrom::Start(offset))?;
                f.read_exact(&mut out)?;
                Ok(out)
            }
            Reader::Bytes(b) => Ok(b[offset as usize..offset as usize + len].to_vec()),
        }
    }
}

/// One `.rez` archive: its header and directory, read on open; payloads
/// read on demand.
pub struct RezArchive {
    name: String,
    data: Data,
    header: RezHeader,
    files: HashMap<String, RezFile>,
    cipher: Option<RezCipher>,
    /// The archive file's size and modification time when opened
    /// (`FileSource::fingerprint`); None for bytes in memory.
    stamp: Option<String>,
}

impl RezArchive {
    /// Open the archive at `path`; `keys` unlock its payloads if its title
    /// names a scheme they have.
    pub fn open(path: impl AsRef<Path>, keys: &RezKeys) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        Self::load(path.display().to_string(), Data::File(path), keys)
    }

    /// An archive held in memory (named `name` in errors and listings).
    pub fn from_bytes(name: &str, bytes: impl Into<Arc<[u8]>>, keys: &RezKeys) -> io::Result<Self> {
        Self::load(name.to_string(), Data::Bytes(bytes.into()), keys)
    }

    fn load(name: String, data: Data, keys: &RezKeys) -> io::Result<Self> {
        let file_len = data.len()?;
        let mut reader = match &data {
            Data::File(p) => Reader::File(File::open(p)?),
            Data::Bytes(b) => Reader::Bytes(b),
        };
        if file_len < HEADER_LEN as u64 {
            return Err(invalid(&name, "shorter than a rez header"));
        }
        let head = reader.read_at(0, HEADER_LEN)?;
        let header = RezHeader::parse(&head, file_len, &name)?;
        let mut files = HashMap::new();
        // "Edge cases": a root size of 0 is an empty archive.
        if header.root_size > 0 {
            let mut walk = Walk {
                reader: &mut reader,
                name: &name,
                file_len,
                files: &mut files,
            };
            walk.block(header.root_offset, header.root_size, "", 0)?;
        }
        let cipher = header.key_scheme().and_then(|s| keys.get(s)).cloned();
        drop(reader);
        let stamp = match &data {
            Data::File(p) => std::fs::metadata(p).ok().map(|m| {
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
                format!("{} {}", m.len(), t.map_or(0, |t| t.as_nanos()))
            }),
            Data::Bytes(_) => None,
        };
        Ok(Self {
            name,
            data,
            header,
            files,
            cipher,
            stamp,
        })
    }

    pub fn header(&self) -> &RezHeader {
        &self.header
    }

    /// The archive's file, if it was opened from one.
    pub fn path(&self) -> Option<&Path> {
        match &self.data {
            Data::File(p) => Some(p),
            Data::Bytes(_) => None,
        }
    }

    /// Every file by normalized path.
    pub fn files(&self) -> impl Iterator<Item = (&String, &RezFile)> {
        self.files.iter()
    }

    /// Encrypted, and no key was given for its scheme.
    pub fn locked(&self) -> bool {
        self.header.key_scheme().is_some() && self.cipher.is_none()
    }

    /// A file's stored bytes, exactly `size` of them, not decrypted.
    pub fn read_raw(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        let f = self.files.get(path)?;
        // "Edge cases": a zero-length file's offset is not looked at.
        if f.size == 0 {
            return Some(Ok(Vec::new()));
        }
        Some(match &self.data {
            Data::File(p) => {
                File::open(p).and_then(|file| Reader::File(file).read_at(f.offset as u64, f.size as usize))
            }
            Data::Bytes(b) => Reader::Bytes(b).read_at(f.offset as u64, f.size as usize),
        })
    }
}

/// Walking the directory tree ("Directory").
struct Walk<'a, 'r> {
    reader: &'a mut Reader<'r>,
    name: &'a str,
    file_len: u64,
    files: &'a mut HashMap<String, RezFile>,
}

impl Walk<'_, '_> {
    /// One block of records under folder `dir` ("" at the root). Every
    /// range is bounds-checked ("Edge cases": bounds checks); unknown
    /// record kinds are rejected.
    fn block(&mut self, offset: u32, size: u32, dir: &str, depth: usize) -> io::Result<()> {
        let name = self.name;
        if depth > MAX_DEPTH {
            return Err(invalid(name, "directory nesting too deep"));
        }
        if offset as u64 + size as u64 > self.file_len {
            return Err(invalid(
                name,
                format!("directory block at {offset} runs past the end of the file"),
            ));
        }
        let block = self.reader.read_at(offset as u64, size as usize)?;
        let need = |pos: usize, n: usize| -> io::Result<()> {
            if pos + n > block.len() {
                Err(invalid(name, format!("truncated record in block at {offset}")))
            } else {
                Ok(())
            }
        };
        let cstr = |pos: &mut usize| -> io::Result<String> {
            let end = block[*pos..]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| invalid(name, format!("unterminated name in block at {offset}")))?;
            let s = String::from_utf8_lossy(&block[*pos..*pos + end]).into_owned();
            *pos += end + 1;
            Ok(s)
        };
        let join = |dir: &str, item: String| if dir.is_empty() { item } else { format!("{dir}/{item}") };

        let mut pos = 0;
        while pos < block.len() {
            need(pos, 16)?;
            let kind = u32_at(&block, pos);
            let (rec_offset, rec_size, time) = (
                u32_at(&block, pos + 4),
                u32_at(&block, pos + 8),
                u32_at(&block, pos + 12),
            );
            match kind {
                // "Directory record": an empty one (size 0) is not read.
                1 => {
                    pos += 16;
                    let sub = join(dir, cstr(&mut pos)?);
                    if rec_size > 0 {
                        self.block(rec_offset, rec_size, &sub, depth + 1)?;
                    }
                }
                // "File record": name without extension, description, then
                // key values, which are skipped.
                0 => {
                    need(pos, 28)?;
                    let type_code = u32_at(&block, pos + 20);
                    let key_count = u32_at(&block, pos + 24) as usize;
                    pos += 28;
                    let file = cstr(&mut pos)?;
                    let _description = cstr(&mut pos)?;
                    need(pos, key_count.saturating_mul(4))?;
                    pos += 4 * key_count;
                    if rec_size > 0 && rec_offset as u64 + rec_size as u64 > self.file_len {
                        return Err(invalid(name, format!("{file}: data runs past the end of the file")));
                    }
                    let mut full = join(dir, file);
                    let ext = decode_type(type_code);
                    if !ext.is_empty() {
                        full.push('.');
                        full.push_str(&ext);
                    }
                    // "Names": within one archive the first one read wins.
                    self.files.entry(normalize(&full)).or_insert(RezFile {
                        offset: rec_offset,
                        size: rec_size,
                        time,
                    });
                }
                k => return Err(invalid(name, format!("unknown record kind {k} in block at {offset}"))),
            }
        }
        Ok(())
    }
}

impl FileSource for RezArchive {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn entries(&self) -> Vec<Entry> {
        self.files
            .iter()
            .map(|(path, f)| Entry {
                path: path.clone(),
                size: f.size as u64,
            })
            .collect()
    }

    fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// The archive file's size and time with the entry's place, size and
    /// time, and whether it can be decrypted; None for an archive in
    /// memory (hashed instead).
    fn fingerprint(&self, path: &str) -> Option<String> {
        let f = self.files.get(path)?;
        let stamp = self.stamp.as_ref()?;
        Some(format!(
            "rez {stamp} {} {} {} {}",
            f.offset,
            f.size,
            f.time,
            self.cipher.is_some()
        ))
    }

    /// The file's bytes, decrypted when the archive is keyed; a keyed
    /// archive without its key fails with `PermissionDenied`.
    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        let raw = self.read_raw(path)?;
        Some(raw.and_then(|mut data| {
            match (self.header.key_scheme(), &self.cipher) {
                (None, _) => {}
                (Some(_), Some(cipher)) => cipher.decrypt(&mut data),
                (Some(scheme), None) => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "{}: encrypted ({scheme}); add a {scheme} key under \
                             [games.combat_arms.keys.{scheme}] in chimera.local.toml",
                            self.name
                        ),
                    ));
                }
            }
            Ok(data)
        }))
    }
}

/// Every `.rez` in `dir`, in name order, as one mount (the first archive
/// with a path wins), and the names of those locked for want of a key.
pub fn open_dir(dir: &Path, keys: &RezKeys) -> io::Result<(Mount, Vec<String>)> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", dir.display())))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("rez")))
        .collect();
    paths.sort();
    let mut mount = Mount::default();
    let mut locked = Vec::new();
    for p in paths {
        let archive = RezArchive::open(&p, keys)?;
        if archive.locked() {
            locked.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
        mount.push(archive);
    }
    Ok((mount, locked))
}
