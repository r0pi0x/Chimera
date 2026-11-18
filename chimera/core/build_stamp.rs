// The build's version stamp, for every app's build script: each app's
// build.rs does `include!("../../chimera/core/build_stamp.rs")` and calls
// `stamp()`. Plain Rust with no crate paths and no inner doc comments
// (it is included, not a module); it includes the version text's format
// (src/core/version.rs) itself.
//
// `stamp` sets `CHIMERA_GIT` (`git describe`) and `CHIMERA_VERSION` (tag
// or describe, short hash and commit date: `format_version`) for the
// app's `env!`s: `core::BuildVersion`, the `version` command, bug reports.
//
// It reruns only when this checkout's commit changes: when HEAD or the
// current branch's ref moves. Paths come from `git rev-parse --git-path`,
// so they are right in worktrees too (there `.git` is a file, and a
// missing rerun path would rerun the script, and relink every target, on
// every build). The index isn't watched (every `git add` would relink
// everything), so a `-dirty` mark is as of the last commit change.

include!("src/core/version.rs");

fn git(args: &[&str]) -> Option<String> {
    std::process::Command::new("git")
        .args(args)
        // Commit dates in UTC.
        .env("TZ", "UTC")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Stamp the build (see the top of this file).
/// In dev/test profiles, emit a stable `"dev"` stamp without watching `.git/HEAD`
/// so `git commit` or branch switches never invalidate app binaries.
fn stamp() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../chimera/core/build_stamp.rs");
    println!("cargo:rerun-if-changed=../../chimera/core/src/core/version.rs");
    println!("cargo:rerun-if-env-changed=CHIMERA_STAMP_GIT");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let force = std::env::var("CHIMERA_STAMP_GIT").is_ok_and(|v| v == "1");
    if profile != "release" && !force {
        println!("cargo:rustc-env=CHIMERA_GIT=dev");
        println!("cargo:rustc-env=CHIMERA_VERSION=dev");
        return;
    }

    let hash = git(&["describe", "--always", "--dirty"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=CHIMERA_GIT={hash}");
    let version = match (
        git(&["rev-parse", "--short", "HEAD"]),
        git(&["log", "-1", "--format=%cd", "--date=format-local:%Y-%m-%d"]),
    ) {
        (Some(short), Some(date)) => format_version(
            git(&["describe", "--tags"]).as_deref(),
            &short,
            &date,
            hash.ends_with("-dirty"),
        ),
        _ => "unknown".into(),
    };
    println!("cargo:rustc-env=CHIMERA_VERSION={version}");
    let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) else { return };
    println!("cargo:rerun-if-changed={head}");
    // The branch's own ref, or packed-refs when it is packed (a detached
    // HEAD changes HEAD itself).
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        for name in [branch.as_str(), "packed-refs"] {
            if let Some(path) = git(&["rev-parse", "--git-path", name])
                && std::path::Path::new(&path).exists()
            {
                println!("cargo:rerun-if-changed={path}");
                break;
            }
        }
    }
}
