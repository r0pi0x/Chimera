//! Free disk space, and the floor under which the caches stop writing
//! (docs/backlog.md, "Disk guards": on 2026-10-31 the dev box's disk
//! filled to 100%). `space` reads the free and total bytes of the disk a
//! path is on (`statvfs` on Unix, `GetDiskFreeSpaceExW` on Windows); the
//! map cache (`map_cache`) and the store's on-demand writes
//! (`Store::put_stored`) ask `has_room` before writing and skip the write
//! below the floor with a one-line warning (the data is still loaded).
//!
//! - **Floor:** `CHIMERA_DISK_FLOOR_GB` (decimal GB, default 20), but
//!   never more than 5% of the disk, so a small disk still caches; `0`
//!   turns the guard off. `tools/doctor` and `tools/post-push-tests.sh`
//!   (`tools/disk-free.sh`) use the same rule.
//! - **Injected reader:** the functions take the reader (`Reader`), so
//!   tests give them any free space they like; `space` is the real one.

use std::{
    collections::HashSet,
    path::Path,
    sync::{Mutex, OnceLock},
};

/// The floor's size in GB (decimal).
pub const FLOOR_ENV: &str = "CHIMERA_DISK_FLOOR_GB";
/// The floor when `FLOOR_ENV` isn't set.
pub const DEFAULT_FLOOR_GB: f64 = 20.0;
/// The floor never exceeds this share of the disk.
pub const FLOOR_SHARE: f64 = 0.05;

/// A disk's free (to an ordinary user) and total bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub free: u64,
    pub total: u64,
}

/// What reads the free space of the disk a path is on.
pub type Reader<'a> = &'a dyn Fn(&Path) -> Option<Space>;

/// The free space of the disk `path` is on (or its nearest existing
/// parent); None when it can't be read.
pub fn space(path: &Path) -> Option<Space> {
    let mut at = Some(path);
    while let Some(p) = at {
        if p.exists() {
            return space_of(p);
        }
        at = p.parent();
    }
    space_of(Path::new("."))
}

#[cfg(unix)]
fn space_of(path: &Path) -> Option<Space> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: `c` is a valid NUL-terminated path and `st` is only read
    // after statvfs reports success (0), which fills it.
    let st = unsafe {
        let mut st = std::mem::zeroed::<libc::statvfs>();
        if libc::statvfs(c.as_ptr(), &mut st) != 0 {
            return None;
        }
        st
    };
    let unit = st.f_frsize as u64;
    Some(Space { free: st.f_bavail as u64 * unit, total: st.f_blocks as u64 * unit })
}

#[cfg(windows)]
fn space_of(path: &Path) -> Option<Space> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is NUL-terminated; the three out-pointers are valid
    // u64s, only read after the call reports success (non-zero).
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut total_free)
    };
    (ok != 0).then_some(Space { free, total })
}

#[cfg(not(any(unix, windows)))]
fn space_of(_path: &Path) -> Option<Space> {
    None
}

/// The floor in bytes for a disk of `total` bytes, given the setting in GB
/// (None: the default): the smaller of it and `FLOOR_SHARE` of the disk.
pub fn floor_for(total: u64, floor_gb: Option<f64>) -> u64 {
    let gb = floor_gb.filter(|g| g.is_finite()).unwrap_or(DEFAULT_FLOOR_GB).max(0.0);
    if gb == 0.0 {
        return 0;
    }
    ((gb * 1e9) as u64).min((total as f64 * FLOOR_SHARE) as u64)
}

/// `floor_for` with the environment's setting.
pub fn floor(total: u64) -> u64 {
    let gb = std::env::var(FLOOR_ENV).ok().and_then(|v| v.trim().parse::<f64>().ok());
    floor_for(total, gb)
}

/// Whether a write of `need` bytes into `dir` leaves the disk's free space
/// at or above the floor. Unreadable free space counts as room.
pub fn has_room(dir: &Path, need: u64, read: Reader) -> bool {
    let Some(s) = read(dir) else { return true };
    s.free.saturating_sub(need) >= floor(s.total)
}

/// The cache cap once free space is weighed in: `cap`, but no more than
/// the `current` bytes it holds plus what is free above the floor. A full
/// disk stops the cache growing; it doesn't empty it.
pub fn limit_cap(cap: u64, current: u64, dir: &Path, read: Reader) -> u64 {
    let Some(s) = read(dir) else { return cap };
    cap.min(current.saturating_add(s.free.saturating_sub(floor(s.total))))
}

/// Print `msg` to stderr, once per process for each `what`.
pub fn warn_once(what: &'static str, msg: impl FnOnce() -> String) {
    static SEEN: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if seen.lock().unwrap_or_else(|e| e.into_inner()).insert(what) {
        eprintln!("{what}: {}", msg());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;

    #[test]
    fn the_floor_is_the_smaller_of_the_setting_and_a_share_of_the_disk() {
        assert_eq!(floor_for(1000 * GB, None), 20 * GB);
        assert_eq!(floor_for(100 * GB, None), 5 * GB, "5% of a small disk");
        assert_eq!(floor_for(1000 * GB, Some(50.0)), 50 * GB);
        assert_eq!(floor_for(1000 * GB, Some(0.0)), 0, "0 turns it off");
        assert_eq!(floor_for(1000 * GB, Some(f64::NAN)), 20 * GB);
    }

    #[test]
    fn writes_skip_below_the_floor() {
        let at = |free| move |_: &Path| Some(Space { free, total: 1000 * GB });
        let dir = Path::new("x");
        assert!(has_room(dir, 0, &at(21 * GB)));
        assert!(!has_room(dir, 0, &at(19 * GB)));
        assert!(has_room(dir, GB, &at(21 * GB + GB)));
        assert!(!has_room(dir, 2 * GB, &at(21 * GB)), "the write itself counts");
        assert!(has_room(dir, 5 * GB, &|_: &Path| None), "unreadable: write");
    }

    #[test]
    fn the_cap_is_clamped_by_free_space() {
        let at = |free| move |_: &Path| Some(Space { free, total: 1000 * GB });
        let dir = Path::new("x");
        // Plenty free: the cap stands.
        assert_eq!(limit_cap(40 * GB, 10 * GB, dir, &at(500 * GB)), 40 * GB);
        // 25 GB free, floor 20: the cache may grow 5 GB past what it holds.
        assert_eq!(limit_cap(40 * GB, 10 * GB, dir, &at(25 * GB)), 15 * GB);
        // Below the floor: no growth, no shrinking.
        assert_eq!(limit_cap(40 * GB, 10 * GB, dir, &at(GB)), 10 * GB);
        assert_eq!(limit_cap(40 * GB, 10 * GB, dir, &|_: &Path| None), 40 * GB);
    }

    #[test]
    fn the_real_reader_reads_this_disk() {
        let s = space(&std::env::temp_dir()).expect("free space of the temp folder");
        assert!(s.total > 0 && s.free <= s.total);
        // A folder that doesn't exist yet: its parent's disk.
        assert!(space(&std::env::temp_dir().join("chimera_no_such").join("deeper")).is_some());
    }
}
