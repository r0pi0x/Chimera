//! A minimal zip writer and reader for feedback bundles: deflate (flate2)
//! or stored entries, flat names, no zip64, no encryption, no extra
//! fields. Enough for Explorer and every OS's unzip to open what we write,
//! and strict on what we read: an entry larger than its caller's cap, a
//! method other than stored or deflate, an encrypted or zip64 entry, a
//! duplicate name, sizes or CRCs that disagree are errors, and inflating
//! stops at the cap (a zip bomb gets no further than that).

use std::io::{Read, Write};

use flate2::{Compression, Crc, read::DeflateDecoder, write::DeflateEncoder};

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
/// UTF-8 names (general purpose bit 11).
const UTF8: u16 = 0x0800;

/// A zip being written in memory.
pub struct ZipWriter {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
    /// DOS time and date of every entry.
    time: u16,
    date: u16,
}

/// Unix seconds as a DOS (time, date) pair, UTC.
fn dos_time(secs: u64) -> (u16, u16) {
    let (y, mo, d, h, mi, s) = super::civil(secs);
    let time = (h << 11) | (mi << 5) | (s / 2);
    let date = ((y.saturating_sub(1980)).min(127) << 9) | (mo << 5) | d;
    (time as u16, date as u16)
}

fn u16le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

impl ZipWriter {
    /// Entries dated `secs` (Unix, UTC).
    pub fn new(secs: u64) -> Self {
        let (time, date) = dos_time(secs);
        Self { out: Vec::new(), central: Vec::new(), count: 0, time, date }
    }

    /// Add `name` with `data`, deflated unless `store` (a JPEG or PNG
    /// gains nothing from it).
    pub fn add(&mut self, name: &str, data: &[u8], store: bool) {
        let mut crc = Crc::new();
        crc.update(data);
        let crc = crc.sum();
        let (method, packed) = if store {
            (0u16, data.to_vec())
        } else {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
            enc.write_all(data).expect("writing to memory");
            (8u16, enc.finish().expect("writing to memory"))
        };
        let offset = self.out.len() as u32;
        let header = |out: &mut Vec<u8>| {
            u16le(out, 20);
            u16le(out, UTF8);
            u16le(out, method);
            u16le(out, self.time);
            u16le(out, self.date);
            u32le(out, crc);
            u32le(out, packed.len() as u32);
            u32le(out, data.len() as u32);
            u16le(out, name.len() as u16);
            u16le(out, 0);
        };
        u32le(&mut self.out, LOCAL);
        header(&mut self.out);
        self.out.extend_from_slice(name.as_bytes());
        self.out.extend_from_slice(&packed);
        u32le(&mut self.central, CENTRAL);
        u16le(&mut self.central, 20); // made by: MS-DOS, 2.0
        header(&mut self.central);
        u16le(&mut self.central, 0); // comment
        u16le(&mut self.central, 0); // disk
        u16le(&mut self.central, 0); // internal attributes
        u32le(&mut self.central, 0); // external attributes
        u32le(&mut self.central, offset);
        self.central.extend_from_slice(name.as_bytes());
        self.count += 1;
    }

    /// The zip's bytes.
    pub fn finish(mut self) -> Vec<u8> {
        let at = self.out.len() as u32;
        let size = self.central.len() as u32;
        self.out.extend_from_slice(&self.central);
        u32le(&mut self.out, END);
        u16le(&mut self.out, 0);
        u16le(&mut self.out, 0);
        u16le(&mut self.out, self.count);
        u16le(&mut self.out, self.count);
        u32le(&mut self.out, size);
        u32le(&mut self.out, at);
        u16le(&mut self.out, 0);
        self.out
    }
}

/// One entry read back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

fn rd16(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "truncated zip".to_string())
}

fn rd32(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "truncated zip".to_string())
}

/// What to do with entries `cap` doesn't name: at most `max` of them, each
/// declaring at most `max_bytes`; they are skipped (never inflated or
/// stored) and their names returned in `Unpacked::dropped`.
#[derive(Clone, Copy, Debug)]
pub struct Unknown {
    pub max: usize,
    pub max_bytes: usize,
}

/// A zip read back: the allowed entries, and the names of the unknown ones
/// that were dropped.
#[derive(Debug, Default)]
pub struct Unpacked {
    pub entries: Vec<Entry>,
    pub dropped: Vec<String>,
}

/// Read every entry of `zip`, strictly: an entry `cap` doesn't name is an
/// error. `cap(name)` is the most bytes entry `name` may hold (None: the
/// name isn't allowed); `max_entries` the most entries.
pub fn read(zip: &[u8], max_entries: usize, cap: impl Fn(&str) -> Option<usize>) -> Result<Vec<Entry>, String> {
    read_lenient(zip, max_entries, cap, Unknown { max: 0, max_bytes: 0 }).map(|r| r.entries)
}

/// Like `read`, but an entry `cap` doesn't name is dropped (a newer client's
/// file) within `unknown`'s limits; over them it is an error. `max_entries`
/// counts the allowed entries only.
pub fn read_lenient(
    zip: &[u8],
    max_entries: usize,
    cap: impl Fn(&str) -> Option<usize>,
    unknown: Unknown,
) -> Result<Unpacked, String> {
    // The end record: its comment is at most 64 KiB.
    let floor = zip.len().saturating_sub(22 + 65_535);
    let end = (floor..=zip.len().saturating_sub(22))
        .rev()
        .find(|&i| rd32(zip, i).ok() == Some(END))
        .ok_or("not a zip (no end record)")?;
    let count = rd16(zip, end + 10)? as usize;
    let cd_size = rd32(zip, end + 12)? as usize;
    let cd_at = rd32(zip, end + 16)? as usize;
    if count > max_entries + unknown.max {
        return Err(format!("{count} entries (at most {})", max_entries + unknown.max));
    }
    if cd_at.checked_add(cd_size).is_none_or(|e| e > end) {
        return Err("the central directory is out of bounds".into());
    }
    let mut out: Vec<Entry> = Vec::with_capacity(count);
    let mut dropped: Vec<String> = Vec::new();
    let mut at = cd_at;
    for _ in 0..count {
        if rd32(zip, at)? != CENTRAL {
            return Err("bad central directory entry".into());
        }
        let flags = rd16(zip, at + 8)?;
        let method = rd16(zip, at + 10)?;
        let crc = rd32(zip, at + 16)?;
        let packed = rd32(zip, at + 20)?;
        let size = rd32(zip, at + 24)?;
        let name_len = rd16(zip, at + 28)? as usize;
        let extra_len = rd16(zip, at + 30)? as usize;
        let comment_len = rd16(zip, at + 32)? as usize;
        let local = rd32(zip, at + 42)? as usize;
        let name = zip.get(at + 46..at + 46 + name_len).ok_or("truncated zip")?;
        let name = std::str::from_utf8(name).map_err(|_| "an entry's name isn't UTF-8")?.to_string();
        at += 46 + name_len + extra_len + comment_len;
        if flags & 1 != 0 {
            return Err(format!("{name}: encrypted"));
        }
        if packed == u32::MAX || size == u32::MAX {
            return Err(format!("{name}: zip64 isn't accepted"));
        }
        let Some(limit) = cap(&name) else {
            if unknown.max == 0 {
                return Err(format!("{name:?} isn't an allowed entry"));
            }
            if name.is_empty() || name.contains(['/', '\\', '\0']) || name.starts_with('.') || name.contains("..") {
                return Err(format!("{name:?} isn't a safe entry name"));
            }
            if dropped.len() >= unknown.max {
                return Err(format!("more than {} unknown entries", unknown.max));
            }
            if size as usize > unknown.max_bytes {
                return Err(format!("{name}: {size} bytes (unknown entries at most {})", unknown.max_bytes));
            }
            if dropped.contains(&name) {
                return Err(format!("{name}: twice"));
            }
            dropped.push(name);
            continue;
        };
        if size as usize > limit {
            return Err(format!("{name}: {size} bytes (at most {limit})"));
        }
        if out.iter().any(|e| e.name == name) {
            return Err(format!("{name}: twice"));
        }
        if rd32(zip, local)? != LOCAL {
            return Err(format!("{name}: bad local header"));
        }
        let start = local + 30 + rd16(zip, local + 26)? as usize + rd16(zip, local + 28)? as usize;
        let raw = zip.get(start..start + packed as usize).ok_or_else(|| format!("{name}: truncated"))?;
        let data = match method {
            0 => raw.to_vec(),
            8 => {
                let mut data = Vec::new();
                DeflateDecoder::new(raw)
                    .take(limit as u64 + 1)
                    .read_to_end(&mut data)
                    .map_err(|e| format!("{name}: {e}"))?;
                data
            }
            m => return Err(format!("{name}: compression method {m} isn't accepted")),
        };
        if data.len() != size as usize {
            return Err(format!("{name}: size doesn't match"));
        }
        let mut sum = Crc::new();
        sum.update(&data);
        if sum.sum() != crc {
            return Err(format!("{name}: CRC doesn't match"));
        }
        out.push(Entry { name, data });
    }
    if out.len() > max_entries {
        return Err(format!("{} entries (at most {max_entries})", out.len()));
    }
    Ok(Unpacked { entries: out, dropped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_entries_read_back_and_caps_hold() {
        let mut w = ZipWriter::new(1_763_078_400);
        w.add("a.txt", &b"hello ".repeat(1000), false);
        w.add("b.png", b"\x89PNG not really", true);
        let zip = w.finish();
        let entries = read(&zip, 8, |_| Some(10_000)).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].data, b"hello ".repeat(1000));
        assert_eq!(entries[1].name, "b.png");
        assert!(read(&zip, 8, |n| (n == "a.txt").then_some(10_000)).unwrap_err().contains("allowed"));
        assert!(read(&zip, 8, |_| Some(100)).unwrap_err().contains("at most"));
        assert!(read(&zip, 1, |_| Some(10_000)).is_err());
        assert!(read(b"not a zip at all, no", 8, |_| Some(1)).is_err());
        let mut bad = zip.clone();
        let i = bad.windows(5).position(|w| w == b"\x89PNG ").unwrap();
        bad[i + 1] ^= 1;
        assert!(read(&bad, 8, |_| Some(10_000)).unwrap_err().contains("CRC"));
    }

    #[test]
    fn unknown_entries_are_dropped_within_their_limits() {
        let mut w = ZipWriter::new(1_763_078_400);
        w.add("a.txt", b"known", false);
        w.add("new.txt", b"from a newer client", false);
        w.add("newer.bin", &[7u8; 100], true);
        let zip = w.finish();
        let cap = |n: &str| (n == "a.txt").then_some(1000);
        let r = read_lenient(&zip, 1, cap, Unknown { max: 2, max_bytes: 200 }).unwrap();
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.dropped, ["new.txt", "newer.bin"]);
        // Strict `read` still refuses them.
        assert!(read(&zip, 8, cap).unwrap_err().contains("allowed"));
        // Too many, too large.
        assert!(read_lenient(&zip, 1, cap, Unknown { max: 1, max_bytes: 200 }).is_err());
        assert!(read_lenient(&zip, 1, cap, Unknown { max: 2, max_bytes: 50 }).unwrap_err().contains("unknown"));
        // Known caps still hold.
        assert!(read_lenient(&zip, 1, |n| (n == "a.txt").then_some(2), Unknown { max: 2, max_bytes: 200 }).is_err());
        // An unsafe unknown name is refused.
        let mut w = ZipWriter::new(1_763_078_400);
        w.add("../evil", b"x", false);
        let zip = w.finish();
        assert!(read_lenient(&zip, 1, cap, Unknown { max: 2, max_bytes: 200 }).is_err());
    }
}
