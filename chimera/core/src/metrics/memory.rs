//! What holds a process's memory (docs/performance.md, "A session's memory"):
//! `/proc/self/smaps` by kind of mapping (Linux) and glibc's own counts, so a
//! growth can be told apart: the allocator's heap, anonymous mappings, mapped
//! files (whose pages the system can drop for free) and the GPU driver's
//! mappings. The client's `chimera_mem` adds the GPU allocator and the stores'
//! bytes. Nothing here is read outside that command: reading smaps walks
//! the page tables (milliseconds at this size).

use std::collections::BTreeMap;

/// A kind of mapping, resident and swapped bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub rss: u64,
    pub swap: u64,
}

impl Held {
    pub fn total(&self) -> u64 {
        self.rss + self.swap
    }
}

/// This process's mappings by kind. `files` names files without their folders.
#[derive(Clone, Debug, Default)]
pub struct Regions {
    /// The allocator's main arena (`[heap]`).
    pub heap: Held,
    /// Anonymous mappings: other arenas, big allocations, the driver's own.
    pub anon: Held,
    /// Device, memfd and dmabuf mappings: the GPU driver's host-visible memory.
    pub device: Held,
    /// Mapped files. Their resident pages are the system's file cache: the
    /// first thing dropped when memory runs short, at no cost but a reread.
    pub file: Held,
    pub stacks: Held,
    /// Files by name, most resident first.
    pub files: Vec<(String, u64)>,
    /// Device and other special mappings by name, most resident first.
    pub devices: Vec<(String, u64)>,
}

impl Regions {
    pub fn total(&self) -> u64 {
        self.heap.total() + self.anon.total() + self.device.total() + self.file.total() + self.stacks.total()
    }

    /// The text of a smaps file.
    pub fn parse(smaps: &str) -> Self {
        let mut r = Self::default();
        let (mut files, mut devices) = (BTreeMap::<String, u64>::new(), BTreeMap::<String, u64>::new());
        let mut name = "";
        let mut held = Held::default();
        let mut flush = |name: &str, held: &mut Held, r: &mut Regions| {
            let h = std::mem::take(held);
            let bucket = if name == "[heap]" {
                &mut r.heap
            } else if name.is_empty() {
                &mut r.anon
            } else if name.starts_with("[stack") {
                &mut r.stacks
            } else if name.starts_with("/dev/") || name.starts_with("/memfd:") || name.starts_with("anon_inode:") || name.starts_with("/dmabuf") {
                *devices.entry(short(name)).or_default() += h.rss;
                &mut r.device
            } else if name.starts_with('/') {
                *files.entry(short(name)).or_default() += h.rss;
                &mut r.file
            } else {
                // [vvar], [vdso], anon names set by a library.
                &mut r.anon
            };
            bucket.rss += h.rss;
            bucket.swap += h.swap;
        };
        for line in smaps.lines() {
            let mut words = line.split_whitespace();
            let Some(first) = words.next() else { continue };
            if first == "Rss:" {
                held.rss += kib(words.next());
            } else if first == "Swap:" {
                held.swap += kib(words.next());
            } else if first.as_bytes().first().is_some_and(u8::is_ascii_hexdigit) && first.contains('-') && !first.ends_with(':') {
                // A mapping's header: address perms offset dev inode [path].
                flush(name, &mut held, &mut r);
                name = line.split_whitespace().nth(5).unwrap_or("");
            }
        }
        flush(name, &mut held, &mut r);
        r.files = sorted(files);
        r.devices = sorted(devices);
        r
    }

    /// This process's, where the system says (Linux).
    pub fn read() -> Option<Self> {
        std::fs::read_to_string("/proc/self/smaps").ok().map(|s| Self::parse(&s))
    }
}

fn kib(word: Option<&str>) -> u64 {
    word.and_then(|w| w.parse::<u64>().ok()).unwrap_or(0) * 1024
}

/// No folders: logs and bug reports carry no machine paths.
fn short(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn sorted(map: BTreeMap<String, u64>) -> Vec<(String, u64)> {
    let mut v: Vec<_> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// glibc's allocator, all arenas.
#[derive(Clone, Copy, Debug, Default)]
pub struct Malloc {
    /// Bytes handed out and not freed (arenas and big mmap'ed blocks).
    pub in_use: u64,
    /// Free bytes the arenas keep.
    pub free: u64,
    /// Of `in_use`, blocks in mappings of their own.
    pub mmapped: u64,
}

/// `mallinfo2`, where the allocator is glibc's.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn malloc() -> Option<Malloc> {
    // SAFETY: no arguments; it locks each arena while it counts.
    let m = unsafe { libc::mallinfo2() };
    Some(Malloc {
        in_use: (m.uordblks + m.hblkhd) as u64,
        free: m.fordblks as u64,
        mmapped: m.hblkhd as u64,
    })
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub fn malloc() -> Option<Malloc> {
    None
}

/// Bytes as gigabytes or megabytes.
pub fn size(b: u64) -> String {
    if b >= 1 << 30 {
        format!("{:.2} GB", b as f64 / (1u64 << 30) as f64)
    } else {
        format!("{} MB", b >> 20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smaps_splits_by_kind() {
        let text = "\
55aa-55bb rw-p 00000000 00:00 0                          [heap]
Size:               8 kB
Rss:                4 kB
Swap:               2 kB
7f00-7f10 rw-p 00000000 00:00 0 
Rss:               16 kB
Swap:               0 kB
7f10-7f20 r--s 00000000 08:01 12 /some/folder/pak_dir.vpk
Rss:               32 kB
Swap:               0 kB
7f20-7f30 rw-s 00000000 00:05 7 /dev/nvidia0
Rss:               64 kB
Swap:               0 kB
7ffc-7ffd rw-p 00000000 00:00 0                          [stack]
Rss:                8 kB
";
        let r = Regions::parse(text);
        assert_eq!(r.heap, Held { rss: 4096, swap: 2048 });
        assert_eq!(r.anon.rss, 16 * 1024);
        assert_eq!(r.file.rss, 32 * 1024);
        assert_eq!(r.device.rss, 64 * 1024);
        assert_eq!(r.stacks.rss, 8 * 1024);
        assert_eq!(r.files, vec![("pak_dir.vpk".to_string(), 32 * 1024)]);
        assert_eq!(r.devices, vec![("nvidia0".to_string(), 64 * 1024)]);
        assert_eq!(r.total(), (4 + 2 + 16 + 32 + 64 + 8) * 1024);
    }
}
