//! Tests of the shell tools that guard the disk (docs/backlog.md, "Disk
//! guards"): `agent-slot.sh trim-incremental` (oldest first, past a limit,
//! never under a running cargo) and `disk-free.sh` (the floor). Unix only
//! (the tools are bash); they run on temporary folders, never the repo's.

#![cfg(unix)]

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("repo root").to_path_buf()
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("chimera_script_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A session folder `incremental/<crate>/<name>` of `kb` KB, last used
/// `age` seconds ago.
fn session(target: &Path, krate: &str, name: &str, kb: usize, age: u64) -> PathBuf {
    let dir = target.join("debug/incremental").join(krate).join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("data.bin"), vec![1u8; kb * 1024]).unwrap();
    let when = SystemTime::now() - Duration::from_secs(age);
    File::open(&dir).unwrap().set_modified(when).unwrap();
    dir
}

fn trim(limit_gb: &str, targets: &[&Path]) -> String {
    let out = Command::new("bash")
        .arg(repo().join("tools/agent-slot.sh"))
        .arg("trim-incremental")
        .args(targets)
        .env("CHIMERA_INCREMENTAL_MAX_GB", limit_gb)
        .env("AGENT_SLOTS_DIR", std::env::temp_dir().join("chimera_script_slots_unused"))
        .output()
        .expect("run agent-slot.sh");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn incremental_is_trimmed_oldest_first_past_the_limit() {
    let root = temp("trim");
    let target = root.join("target");
    // 4 sessions of 300 KB = 1.2 MB; the limit 0.001 GB = ~1 MB, the goal
    // 3/4 of it (~768 KB): the two oldest go.
    let old = session(&target, "a-1", "s-old", 300, 400);
    let mid = session(&target, "b-2", "s-mid", 300, 300);
    let new = session(&target, "a-1", "s-new", 300, 200);
    let newest = session(&target, "b-2", "s-newest", 300, 100);
    let said = trim("0.001", &[&target]);
    assert!(!old.exists() && !mid.exists(), "oldest go: {said}");
    assert!(new.exists() && newest.exists(), "newest stay: {said}");
    // Under the limit: untouched.
    trim("0.001", &[&target]);
    assert!(new.exists() && newest.exists());
    // Other target folders (a triple's profile) are found too.
    let triple = root.join("t2");
    let deep = triple.join("x86_64-pc-windows-gnu/debug/incremental/c-3/s-1");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("data.bin"), vec![1u8; 2 << 20]).unwrap();
    trim("0.001", &[&triple]);
    assert!(!deep.exists(), "found under a target triple");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_target_a_cargo_holds_is_left_alone() {
    let root = temp("locked");
    let target = root.join("target");
    let a = session(&target, "a-1", "s-a", 1500, 300);
    let lock = File::create(target.join("debug/.cargo-lock")).unwrap();
    lock.lock().unwrap(); // what a running cargo holds
    let said = trim("0.001", &[&target]);
    assert!(a.exists(), "not touched while locked");
    assert!(said.contains("left alone"), "{said}");
    drop(lock);
    trim("0.001", &[&target]);
    assert!(!a.exists(), "trimmed once cargo is gone");
    fs::remove_dir_all(&root).ok();
}

/// `disk_check` with a `df` that reports `free_kb` of `total_kb`.
fn disk_check(free_kb: u64, total_kb: u64, floor_env: Option<&str>) -> (bool, String) {
    let root = temp("df");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let df = bin.join("df");
    fs::write(
        &df,
        format!("#!/bin/sh\necho Filesystem 1024-blocks Used Available Capacity Mounted\necho x {total_kb} 0 {free_kb} 0% /\n"),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&df, fs::Permissions::from_mode(0o755)).unwrap();
    let script = format!(". {}/tools/disk-free.sh; disk_check /; echo \"$?:$DISK_FLOOR_GB\"", repo().display());
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(script).env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()));
    cmd.env_remove("CHIMERA_DISK_FLOOR_GB");
    if let Some(f) = floor_env {
        cmd.env("CHIMERA_DISK_FLOOR_GB", f);
    }
    let out = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
    fs::remove_dir_all(&root).ok();
    let (code, floor) = out.trim().split_once(':').unwrap();
    (code == "0", floor.to_string())
}

#[test]
fn the_free_space_floor_is_the_smaller_of_the_setting_and_5_percent() {
    let gb = 1_000_000_000u64 / 1024; // KB per decimal GB
    // 1000 GB disk: floor 20 GB.
    assert_eq!(disk_check(25 * gb, 1000 * gb, None), (true, "20".into()));
    assert_eq!(disk_check(15 * gb, 1000 * gb, None), (false, "20".into()));
    // 100 GB disk: 5% = 5 GB.
    assert_eq!(disk_check(6 * gb, 100 * gb, None), (true, "5".into()));
    assert_eq!(disk_check(4 * gb, 100 * gb, None), (false, "5".into()));
    // The setting moves it; 0 turns it off.
    assert_eq!(disk_check(40 * gb, 1000 * gb, Some("50")), (false, "50".into()));
    assert!(disk_check(gb, 1000 * gb, Some("0")).0);
}
