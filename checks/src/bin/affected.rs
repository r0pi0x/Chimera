//! `affected`: which workspace packages' tests a change can affect
//! (repo_checks' `affected` module has the rules), printed as arguments
//! for `cargo nextest run` (or `cargo test`), one per line on stdout; what
//! was selected and why goes to stderr. The git hooks run it
//! (.githooks/pre-commit on the staged files, pre-push on what's pushed).
//!
//! ```text
//! cargo run -q -p repo_checks --bin affected -- [CHANGES] [OUTPUT]
//! CHANGES (any number; together their union):
//!   --staged          the index against HEAD (a commit about to be made)
//!   --range A..B      what B changes against A (`git diff A B`)
//!   --new SHA         a new branch: SHA against its merge-base with
//!                     origin/main (everything without one)
//!   --base REV        HEAD against its merge-base with REV
//!   --files F...      these paths (relative to the repository root)
//!   --trees A B       what tree (or commit) B changes against tree A,
//!                     nothing fetched (the hooks' pass stamps: what
//!                     changed since a tree passed, .githooks/stamps.sh)
//!   --remote NAME     where to fetch a commit that isn't here (default
//!                     origin; the pre-push hook passes its remote)
//! OUTPUT:
//!   --format nextest  (default) `cargo nextest run` arguments
//!   --format cargo    `cargo test` arguments (whole packages)
//!   --skip-heavy      leave out `heavy::` tests (nextest; the fast tier)
//!   --direct          only the changed packages' own tests, not their
//!                     dependents' (a change selecting everything narrows
//!                     to repo_checks): what the pre-commit hook runs
//!   --code            leave out the data files (`Change::Data`: a dev
//!                     build reads them at run time): what the pre-commit
//!                     hook type-checks
//!   --quiet           no summary on stderr
//! ```
//!
//! A commit the comparison needs that this clone doesn't have (a remote
//! ref that moved after the last fetch) is fetched from the remote first;
//! a range whose old end still can't be had compares the new end with its
//! merge-base with the remote's main instead. A changed Cargo.lock selects
//! only the members whose resolved dependencies changed between its two
//! versions (`affected::lock_affected`); everything when they can't be
//! read.
//!
//! Example: `cargo nextest run --features dev $(cargo run -q -p
//! repo_checks --bin affected -- --base origin/main)` in a shell that
//! splits on newlines only (the filter has spaces).

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use repo_checks::affected::{Selection, Workspace, is_data, lock_affected};

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Changed paths from `git diff --name-only` with `args`.
fn diff(root: &Path, args: &[&str], out: &mut Vec<String>) -> Result<(), String> {
    let mut all = vec!["diff", "--name-only", "--no-renames"];
    all.extend_from_slice(args);
    out.extend(git(root, &all)?.lines().filter(|l| !l.is_empty()).map(String::from));
    Ok(())
}

/// Whether `rev` names a commit this clone has.
fn has_commit(root: &Path, rev: &str) -> bool {
    git(root, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")]).is_ok()
}

/// Makes sure commit `rev` is here: fetched from `remote` (by itself,
/// then the whole remote) if it isn't. Says so on stderr.
fn ensure_commit(root: &Path, remote: &str, rev: &str) -> bool {
    if has_commit(root, rev) {
        return true;
    }
    eprintln!("affected: {rev} isn't in this clone: fetching it from {remote}");
    for args in [
        vec!["fetch", "--quiet", "--no-tags", remote, rev],
        vec!["fetch", "--quiet", "--no-tags", remote],
    ] {
        if let Err(e) = git(root, &args) {
            eprintln!("affected: {e}");
        }
        if has_commit(root, rev) {
            return true;
        }
    }
    false
}

/// The remote's main branch as a local ref (`<remote>/main`, else
/// `origin/main`), fetched first when it's missing.
fn remote_main(root: &Path, remote: &str) -> String {
    let main = format!("{remote}/main");
    if has_commit(root, &main) {
        return main;
    }
    if has_commit(root, "origin/main") {
        return "origin/main".into();
    }
    let _ = git(root, &["fetch", "--quiet", "--no-tags", remote, "main"]);
    if has_commit(root, &main) { main } else { "origin/main".into() }
}

/// `sha`'s merge-base with the remote's main, fetching main once if
/// there's none.
fn merge_base_main(root: &Path, remote: &str, sha: &str) -> Result<String, String> {
    let main = remote_main(root, remote);
    if let Ok(base) = git(root, &["merge-base", sha, &main]) {
        return Ok(base.trim().to_string());
    }
    let _ = git(root, &["fetch", "--quiet", "--no-tags", remote, "main"]);
    git(root, &["merge-base", sha, &main]).map(|b| b.trim().to_string())
}

/// Cargo.lock at `rev` (`""`: the index's; None: the work tree's).
fn lock_at(root: &Path, rev: Option<&str>) -> Result<String, String> {
    match rev {
        None => std::fs::read_to_string(root.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}")),
        Some(r) => git(root, &["show", &format!("{r}:Cargo.lock")]),
    }
}

fn usage() -> &'static str {
    "usage: affected [--staged] [--range A..B] [--new SHA] [--base REV] [--files F...] [--remote NAME] \
     [--trees A B] [--format nextest|cargo] [--skip-heavy] [--direct] [--code] [--quiet]"
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("affected: {e}");
            ExitCode::FAILURE
        }
    }
}

/// What to compare.
enum Source {
    /// The index against HEAD.
    Staged,
    /// `git diff A B`.
    Revs(String, String),
    /// These paths, the work tree against HEAD.
    Files(Vec<String>),
}

fn run() -> Result<(), String> {
    let root = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"])?.trim());
    let mut everything: Option<String> = None;
    let (mut format, mut skip_heavy, mut quiet, mut direct) = ("nextest".to_string(), false, false, false);
    let mut code = false;
    let mut remote = "origin".to_string();
    // The changes as given, resolved once `--remote` is known.
    let mut asked: Vec<(String, Vec<String>)> = Vec::new();
    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value\n{}", usage()));
        match arg.as_str() {
            "--staged" => asked.push((arg, Vec::new())),
            "--range" | "--new" | "--base" => {
                let v = value()?;
                asked.push((arg, vec![v]));
            }
            "--files" => {
                let mut files = Vec::new();
                while let Some(f) = args.next_if(|a| !a.starts_with("--")) {
                    files.push(f.replace('\\', "/"));
                }
                asked.push((arg, files));
            }
            "--trees" => {
                let a = value()?;
                let b = args.next().ok_or_else(|| format!("--trees needs two values\n{}", usage()))?;
                asked.push((arg, vec![a, b]));
            }
            "--remote" => remote = value()?,
            "--format" => format = value()?,
            "--skip-heavy" => skip_heavy = true,
            "--quiet" => quiet = true,
            "--direct" => direct = true,
            "--code" => code = true,
            "-h" | "--help" => {
                println!("{}", usage());
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}\n{}", usage())),
        }
    }
    if asked.is_empty() {
        return Err(format!("say what changed\n{}", usage()));
    }
    let mut sources = Vec::new();
    for (arg, mut values) in asked {
        match arg.as_str() {
            "--staged" => sources.push(Source::Staged),
            "--range" => {
                let range = values.remove(0);
                let (a, b) = range.split_once("..").ok_or("--range wants A..B")?;
                if !ensure_commit(&root, &remote, b) {
                    return Err(format!("{b} (the range's new end) isn't in this clone"));
                }
                if ensure_commit(&root, &remote, a) {
                    sources.push(Source::Revs(a.into(), b.into()));
                } else {
                    // The remote's old copy can't be had (a force-push
                    // dropped it): what the branch adds to main instead.
                    eprintln!("affected: {a} can't be had: comparing {b} with its merge-base with main instead");
                    match merge_base_main(&root, &remote, b) {
                        Ok(base) => sources.push(Source::Revs(base, b.into())),
                        Err(_) => everything = Some(format!("{a} isn't here and {b} has no merge-base with main")),
                    }
                }
            }
            "--new" => {
                let sha = values.remove(0);
                if !ensure_commit(&root, &remote, &sha) {
                    return Err(format!("{sha} isn't in this clone"));
                }
                match merge_base_main(&root, &remote, &sha) {
                    Ok(base) => sources.push(Source::Revs(base, sha)),
                    Err(_) => everything = Some(format!("{sha} has no merge-base with {remote}'s main")),
                }
            }
            "--base" => {
                let rev = values.remove(0);
                ensure_commit(&root, &remote, &rev);
                let base = git(&root, &["merge-base", &rev, "HEAD"])?;
                sources.push(Source::Revs(base.trim().into(), "HEAD".into()));
            }
            "--trees" => {
                let b = values.pop().unwrap_or_default();
                let a = values.pop().unwrap_or_default();
                sources.push(Source::Revs(a, b));
            }
            _ => sources.push(Source::Files(values)),
        }
    }
    let mut changed = Vec::new();
    // Each source's two lockfiles, where it changes Cargo.lock.
    let mut locks: Vec<Result<(String, String), String>> = Vec::new();
    for source in &sources {
        let before = changed.len();
        let (old, new) = match source {
            Source::Staged => {
                diff(&root, &["--cached"], &mut changed)?;
                (Some("HEAD"), Some(""))
            }
            Source::Revs(a, b) => {
                diff(&root, &[a, b], &mut changed)?;
                (Some(a.as_str()), Some(b.as_str()))
            }
            Source::Files(files) => {
                changed.extend(files.iter().cloned());
                (Some("HEAD"), None)
            }
        };
        if changed[before..].iter().any(|p| p == "Cargo.lock") {
            locks.push(lock_at(&root, old).and_then(|a| Ok((a, lock_at(&root, new)?))));
        }
    }
    changed.sort();
    changed.dedup();
    // The union of what each lockfile change affects.
    let lock = (!locks.is_empty()).then(|| {
        let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for pair in &locks {
            let (a, b) = pair.as_ref().map_err(|e| e.clone())?;
            for (member, crates) in lock_affected(a, b)? {
                out.entry(member).or_default().extend(crates);
            }
        }
        Ok(out)
    });
    let ws = Workspace::load(&root)?;
    if code {
        changed.retain(|p| !is_data(&ws, p));
    }
    let mut sel = Selection::with_lock(&ws, &root, &changed, lock.as_ref());
    if let Some(why) = everything {
        sel.all = Some(why);
    }
    if direct {
        sel = sel.direct();
    }
    if !quiet {
        eprint!("{}", sel.summary());
    }
    let out = match format.as_str() {
        "nextest" => sel.nextest_args(skip_heavy),
        "cargo" => sel.cargo_args(),
        other => return Err(format!("unknown format {other}\n{}", usage())),
    };
    for a in out {
        println!("{a}");
    }
    Ok(())
}
