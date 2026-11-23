//! Tests of the shell tools that guard the disk (docs/backlog.md, "Disk
//! guards"): `agent-slot.sh trim-incremental` (oldest first, past a limit,
//! never under a running cargo; a slot's trim takes them before built
//! crates), `jobserver-heal.sh` (lost tokens put back) and `disk-free.sh`
//! (the floor), `wait-for` (named waits with a timeout, logged) and the
//! hooks' pass stamps (`.githooks/stamps.sh`: the key, merged passes,
//! per-package reuse after docs edits and rebases), and
//! `post-push-tests.sh`'s queue (a verdict under frequent pushes, a stale
//! lock taken over, starvation reported). Unix only
//! (the tools are bash); they run on temporary folders, never the repo's.

#![cfg(unix)]

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime},
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

/// A slot's trim (what `acquire` runs on the slot it takes) drops the old
/// incremental sessions before anything else and keeps what was built:
/// sweeping `target/` by size alone removed the built crates and left the
/// caches (docs/performance.md, "Iteration speed").
#[test]
fn a_slots_trim_takes_old_incremental_sessions_and_keeps_the_built_crates() {
    let slots = temp("slot_trim");
    let target = slots.join("slot1/target");
    let old = session(&target, "a-1", "s-old", 900, 400);
    let new = session(&target, "a-1", "s-new", 300, 100);
    let built = target.join("debug/deps/libbuilt.rlib");
    fs::create_dir_all(built.parent().unwrap()).unwrap();
    fs::write(&built, vec![1u8; 400 * 1024]).unwrap();
    let out = Command::new("bash")
        .arg(repo().join("tools/agent-slot.sh"))
        .args(["trim", "slot1"])
        .env("CHIMERA_INCREMENTAL_MAX_GB", "0.001")
        .env("AGENT_SLOT_MAX_GB", "60")
        .env("AGENT_SLOTS_DIR", &slots)
        .output()
        .expect("run agent-slot.sh");
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{said}");
    assert!(!old.exists(), "the old session goes: {said}");
    assert!(new.exists() && built.exists(), "the new session and the built crate stay: {said}");
    fs::remove_dir_all(&slots).ok();
}

/// The tokens in `fifo` (taken out and put back; an `x` written after them
/// marks the end, so the read never blocks).
fn tokens(fifo: &mut File) -> usize {
    use std::io::{Read, Write};
    fifo.write_all(b"x").unwrap();
    let mut n = 0;
    let mut b = [0u8; 1];
    loop {
        fifo.read_exact(&mut b).unwrap();
        if b[0] == b'x' {
            break;
        }
        n += 1;
    }
    fifo.write_all(&vec![b'+'; n]).unwrap();
    n
}

/// The agents' jobserver gets back the tokens killed builds took with
/// them, and loses extra ones, when no build is attached to it
/// (tools/jobserver-heal.sh; docs/performance.md, "Iteration speed").
#[test]
fn a_drained_jobserver_is_refilled() {
    use std::io::Write;
    if Command::new("python3").arg("--version").output().is_err() {
        return;
    }
    let root = temp("jobserver");
    let fifo = root.join("jobserver.fifo");
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    // This process is the holder (as agent-env.sh's `sleep infinity`).
    let mut held = fs::OpenOptions::new().read(true).write(true).open(&fifo).unwrap();
    let heal = |every: &str| {
        let out = Command::new("bash")
            .arg(repo().join("tools/jobserver-heal.sh"))
            .arg(&fifo)
            .arg("12")
            .arg(std::process::id().to_string())
            .env("AGENT_JS_HEAL_EVERY", every)
            .output()
            .expect("run jobserver-heal.sh");
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    // 3 of 12 left, nothing building: 9 come back.
    held.write_all(b"+++").unwrap();
    let said = heal("0");
    assert_eq!(tokens(&mut held), 12, "{said}");
    assert!(said.contains("9 were lost"), "{said}");
    // Full: nothing to do, nothing said.
    assert_eq!(heal("0"), "");
    // Too many (an estimate was short once): back to the full set.
    held.write_all(b"++++").unwrap();
    heal("0");
    assert_eq!(tokens(&mut held), 12);
    // Checked a moment ago: not looked at again within the interval.
    held.write_all(b"+").unwrap();
    heal("3600");
    assert_eq!(tokens(&mut held), 13);
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

/// `tools/wait-for <label> <args...>` with its log in `log`: (exit code, log).
fn wait_for(label: &str, args: &[&str], log: &Path) -> (i32, String) {
    let out = Command::new("bash")
        .arg(repo().join("tools/wait-for"))
        .arg(label)
        .args(args)
        .env("CHIMERA_WAIT_LOG", log)
        .output()
        .expect("run wait-for");
    (out.status.code().unwrap_or(-1), fs::read_to_string(log).unwrap_or_default())
}

#[test]
fn wait_for_ends_when_the_thing_happens_or_at_its_timeout_and_logs_both() {
    let root = temp("wait_for");
    let log = root.join("waits.log");
    let file = root.join("ready.txt");
    fs::write(&file, "starting\nready on port 1234\n").unwrap();
    let (code, text) = wait_for("server up", &["--every", "1", "--timeout", "10s", "--file", file.to_str().unwrap(), "--match", "ready on port"], &log);
    assert_eq!(code, 0);
    assert!(text.contains("\tdone\tserver up\t"), "{text}");
    let (code, text) = wait_for("never", &["--every", "1", "--timeout", "1s", "--", "false"], &log);
    assert_eq!(code, 124);
    assert!(text.contains("\ttimeout\tnever\t"), "{text}");
    // Nothing to wait for is refused, not an endless wait.
    assert_eq!(wait_for("nothing", &[], &log).0, 2);
    fs::remove_dir_all(&root).ok();
}

/// Runs `script` in bash in the git repository `dir` with the hooks'
/// `stamps.sh` sourced; its stdout.
fn with_stamps(dir: &Path, script: &str) -> String {
    let out = Command::new("bash")
        .arg("-c")
        .arg(format!(". '{}'\n{script}", repo().join(".githooks/stamps.sh").display()))
        .current_dir(dir)
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("run bash");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    if !err.is_empty() {
        eprintln!("{err}");
    }
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn stamps_key_the_working_tree_and_merge_the_packages_that_passed() {
    let root = temp("stamps");
    let git = |args: &[&str]| {
        let ok = Command::new("git").args(args).current_dir(&root).env_remove("GIT_DIR").env_remove("GIT_INDEX_FILE").output().unwrap();
        assert!(ok.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&ok.stderr));
    };
    git(&["init", "-q"]);
    fs::write(root.join("a.rs"), "one").unwrap();
    git(&["add", "a.rs"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "a"]);
    let key = with_stamps(&root, "tree_key").trim().to_string();
    assert_eq!(key.len(), 40, "{key}");
    // The same tree, the same key; an unstaged edit (the same size, in the
    // same second as the commit: git's "racily clean" case) or an untracked
    // file changes it.
    assert_eq!(with_stamps(&root, "tree_key").trim(), key);
    fs::write(root.join("a.rs"), "two").unwrap();
    let edited = with_stamps(&root, "tree_key").trim().to_string();
    assert_ne!(edited, key);
    fs::write(root.join("new.rs"), "x").unwrap();
    assert_ne!(with_stamps(&root, "tree_key").trim(), edited);
    // A commit hook's pass, then the push hook's: the packages merge, and
    // the commit hook's selection is still covered.
    let out = with_stamps(
        &root,
        r#"k=$(tree_key)
        stamp_record "$k" pre-commit "$(printf 'cs_source\nrepo_checks\n')" "chimera_core"
        stamp_record "$k" pre-push repo_checks ""
        covered "$(printf 'cs_source\nrepo_checks\n')" "$(stamp_tested "$k")" && echo tested-covered
        covered chimera_core "$(stamp_checked "$k")" && echo checked-covered
        covered hl2dm "$(stamp_tested "$k")" || echo hl2dm-not
        covered hl2dm '*' && echo star-covers
        pkgs "$(printf -- '-p\na\n-p\nb\n')" | tr '\n' ,"#,
    );
    assert_eq!(out, "tested-covered\nchecked-covered\nhl2dm-not\nstar-covers\na,b,", "{out}");
    // A first pass with nothing type-checked beyond what it tested (the
    // push hook's alone) is recorded too, and leaves no scratch file.
    fs::write(root.join("other.rs"), "y").unwrap();
    let out = with_stamps(
        &root,
        r#"k=$(tree_key)
        stamp_record "$k" pre-push repo_checks ""
        covered repo_checks "$(stamp_tested "$k")" && echo pushed-covered
        ls "$stamps" | grep -c tmp || true"#,
    );
    assert_eq!(out, "pushed-covered\n0\n", "{out}");
    fs::remove_dir_all(&root).ok();
}

/// Per-package reuse (`stamp_reuse`): after a pass on one tree, a later
/// tree keeps each package whose inputs (its files, its dependencies',
/// as `affected` reads the crate graph) are unchanged. The sequence an
/// agent makes: a pass with crate `b` changed, a docs edit, a rebase
/// bringing an unrelated crate's change, then a change to `b`'s
/// dependency `a`, which reruns `b`.
#[test]
fn stamps_reuse_each_package_whose_inputs_did_not_change() {
    let root = temp("stamp_reuse");
    let git = |args: &[&str]| {
        let ok = Command::new("git").args(args).current_dir(&root).env_remove("GIT_DIR").env_remove("GIT_INDEX_FILE").output().unwrap();
        assert!(ok.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&ok.stderr));
    };
    // `checks` is repo_checks, which any change selects (the docs rules).
    let members = "[workspace]\nmembers = [\"a\", \"b\", \"c\", \"checks\"]\nresolver = \"2\"\n";
    fs::write(root.join("Cargo.toml"), members).unwrap();
    for (dir, name, deps) in [("a", "a", ""), ("b", "b", "a = { path = \"../a\" }\n"), ("c", "c", ""), ("checks", "repo_checks", "")] {
        fs::create_dir_all(root.join(dir).join("src")).unwrap();
        fs::write(
            root.join(dir).join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{deps}"),
        )
        .unwrap();
        fs::write(root.join(dir).join("src/lib.rs"), "pub fn f() {}\n").unwrap();
    }
    fs::write(root.join("README.md"), "# t\n").unwrap();
    let lock = Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
        .args(["generate-lockfile", "--offline"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(lock.status.success(), "{}", String::from_utf8_lossy(&lock.stderr));
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base"]);
    let reuse = |label: &str| {
        let out = with_stamps(
            &root,
            &format!(
                "affected_cmd() {{ '{}' \"$@\"; }}\ntree_state\nstamp_reuse\necho \"tested $(echo $reuse_tested)\"\necho \"checked $(echo $reuse_checked)\"",
                env!("CARGO_BIN_EXE_affected")
            ),
        );
        eprintln!("{label}: {out}");
        out
    };
    // The agent's change to b, and its `--worktree` pass: b's fast tier
    // and repo_checks', b's tests type-checked.
    fs::write(root.join("b/src/lib.rs"), "pub fn f() { a::f() }\n").unwrap();
    with_stamps(&root, "k=$(tree_key); stamp_record \"$k\" 'pre-commit --worktree' \"$(printf 'b\\nrepo_checks\\n')\" b");
    // A docs edit: b still passes; repo_checks (the docs rules) reruns.
    fs::write(root.join("README.md"), "# t\nmore\n").unwrap();
    assert_eq!(reuse("docs edit"), "tested b\nchecked b\n");
    // A new untracked doc too, then a rebase that brings c's change: still b.
    fs::write(root.join("notes.md"), "n\n").unwrap();
    git(&["add", "b", "README.md"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "b"]);
    fs::write(root.join("c/src/lib.rs"), "pub fn g() {}\n").unwrap();
    git(&["add", "c"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "main's c"]);
    assert_eq!(reuse("rebase over c"), "tested b\nchecked b\n");
    // A dependency's source change: b's inputs changed, nothing reused.
    fs::write(root.join("a/src/lib.rs"), "pub fn f() { let _ = 1; }\n").unwrap();
    assert_eq!(reuse("a changed"), "tested \nchecked \n");
    // The workspace manifest (or the hooks) invalidates every package.
    fs::write(root.join("a/src/lib.rs"), "pub fn f() {}\n").unwrap();
    assert_eq!(reuse("a restored"), "tested b\nchecked b\n");
    fs::write(root.join("Cargo.toml"), format!("{members}# x\n")).unwrap();
    assert_eq!(reuse("workspace manifest"), "tested \nchecked \n");
    // keep_pkgs narrows a selection to the packages left to run.
    let out = with_stamps(&root, "keep_pkgs \"$(printf -- '-p\\na\\n-p\\nb\\n--features\\na/x,b/y\\n')\" b | tr '\\n' ' '");
    assert_eq!(out, "-p b --features b/y ");
    fs::remove_dir_all(&root).ok();
}

/// `agent-slot.sh acquire` takes the free slot whose build is warmest for
/// the branch's base (a built target/ over an empty one, then the fewest
/// differing files) and says how warm it is.
#[test]
fn acquire_prefers_the_warmest_slot_and_says_so() {
    let root = temp("slot_warmth");
    let repo_dir = root.join("repo");
    fs::create_dir_all(repo_dir.join("tools")).unwrap();
    fs::copy(repo().join("tools/agent-slot.sh"), repo_dir.join("tools/agent-slot.sh")).unwrap();
    let git = |args: &[&str]| {
        let ok = Command::new("git").args(args).current_dir(&repo_dir).env_remove("GIT_DIR").env_remove("GIT_INDEX_FILE").output().unwrap();
        assert!(ok.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&ok.stderr));
    };
    fs::write(repo_dir.join("lib.rs"), "a").unwrap();
    git(&["init", "-q", "-b", "main"]);
    git(&["add", "."]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "a"]);
    let slot = |args: &[&str]| {
        let out = Command::new("bash")
            .arg(repo_dir.join("tools/agent-slot.sh"))
            .args(args)
            .current_dir(&repo_dir)
            .env("AGENT_SLOTS", "2")
            .env("AGENT_SLOTS_DIR", root.join("slots"))
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        (String::from_utf8_lossy(&out.stdout).trim().to_string(), String::from_utf8_lossy(&out.stderr).into_owned())
    };
    let (one, err) = slot(&["acquire", "x"]);
    assert!(err.contains("cold"), "{err}");
    let (two, _) = slot(&["acquire", "y"]);
    slot(&["release", &one]);
    slot(&["release", &two]);
    // Only slot2 has a build: a new branch gets it, warm.
    fs::create_dir_all(Path::new(&two).join("target/debug/deps")).unwrap();
    let (got, err) = slot(&["acquire", "z"]);
    assert_eq!(got, two, "{err}");
    assert!(err.contains("warm, built at"), "{err}");
    fs::remove_dir_all(&root).ok();
}

/// Sourcing `tools/agent-env.sh` (twice at once) never stops or restarts a
/// running sccache server, not even one that looks hung (no jobserver
/// tokens, no children), and starts none while one listens: a restart
/// killed other agents' builds (docs/OBSERVABILITY.md, "Shared compile
/// cache").
#[test]
fn sourcing_agent_env_leaves_a_running_sccache_server_alone() {
    use std::process::Stdio;
    let ok = |c: &str| Command::new(c).arg("--version").output().is_ok();
    if !ok("python3") || !ok("flock") || Command::new("ss").arg("-V").output().is_err() {
        return;
    }
    let root = temp("sccache_env");
    let log = root.join("calls.log");
    let fake = root.join("sccache");
    fs::write(&fake, format!("#!/bin/sh\necho \"$@\" >> {}\n", log.display())).unwrap();
    Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
    // A listener with an empty jobserver pipe: what a hung server looks like.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut server = Command::new("python3")
        .arg("-c")
        .arg(
            "import os,socket,sys,time\nr,w=os.pipe()\ns=socket.socket()\n\
             s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)\n\
             s.bind(('127.0.0.1',int(sys.argv[1])))\ns.listen()\ntime.sleep(120)",
        )
        .arg(port.to_string())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let up = Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(up.elapsed() < Duration::from_secs(10), "the fake server never listened");
        std::thread::yield_now();
    }
    let source = || {
        Command::new("bash")
            .arg("-c")
            .arg(format!("source {}/tools/agent-env.sh", repo().display()))
            .env("SCCACHE_BIN", &fake)
            .env("SCCACHE_SERVER_PORT", port.to_string())
            .env("AGENT_JS_DIR", root.join("js"))
            .env("XDG_RUNTIME_DIR", &root)
            .env("AGENT_JOBS", "2")
            .env("AGENT_NO_SLICE", "1")
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let (a, b) = (source(), source());
    let said: String = [a, b]
        .into_iter()
        .map(|c| String::from_utf8_lossy(&c.wait_with_output().unwrap().stderr).into_owned())
        .collect();
    let alive = server.try_wait().unwrap().is_none();
    let calls = fs::read_to_string(&log).unwrap_or_default();
    server.kill().ok();
    server.wait().ok();
    // The jobserver holder agent-env.sh started for this test's FIFO: left
    // running, it held whatever it inherited (it once held post-push's lock
    // for six hours).
    if let Ok(pid) = fs::read_to_string(root.join("js/holder.pid")) {
        Command::new("kill").arg(pid.trim()).status().ok();
    }
    fs::remove_dir_all(&root).ok();
    assert!(alive, "the running server was stopped: {said}");
    assert_eq!(calls, "", "sccache was called (stop or start) while a server listened: {said}");
}

/// A wall-clock pause while real processes (the post-push runs) work.
fn pause(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// A post-push setup on temporary folders: a bare origin, its clone (the
/// main checkout, holding `tools/post-push-tests.sh`, the real
/// `disk-free.sh` and stubs of the rest) and a fake cargo whose `test` takes
/// `FAKE_SLOW` seconds and leaves a detached `sleep` behind, as a jobserver
/// holder does.
struct PostPush {
    root: PathBuf,
    main: PathBuf,
    out: PathBuf,
    leaks: PathBuf,
    n: usize,
}

impl PostPush {
    fn new(name: &str, slow: &str) -> PostPush {
        let root = temp(name);
        let (main, out, leaks) = (root.join("main"), root.join("out"), root.join("leaks"));
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&out).unwrap();
        fs::write(
            bin.join("cargo"),
            format!(
                "#!/bin/sh\ncase \"$*\" in\n\
                 \"hakari --version\"|\"nextest --version\") exit 1 ;;\n\
                 *\"--bin affected\"*) echo --workspace ;;\n\
                 test*) setsid sleep 120 </dev/null >/dev/null 2>&1 & echo $! >>{leaks}; sleep {slow} ;;\n\
                 esac\nexit 0\n",
                leaks = leaks.display()
            ),
        )
        .unwrap();
        let pp = PostPush { root, main, out, leaks, n: 0 };
        pp.sh(&pp.root, "git init -q --bare -b main origin.git && git clone -q origin.git main 2>/dev/null");
        let tools = pp.main.join("tools");
        fs::create_dir_all(&tools).unwrap();
        for f in ["post-push-tests.sh", "disk-free.sh"] {
            fs::copy(repo().join("tools").join(f), tools.join(f)).unwrap();
        }
        fs::write(tools.join("agent-slot.sh"), "#!/bin/sh\nexit 0\n").unwrap();
        fs::write(tools.join("agent-env.sh"), "agent_run() { \"$@\"; }\n").unwrap();
        pp.sh(&pp.root, &format!("chmod +x {0}/cargo {1}/*.sh", bin.display(), tools.display()));
        pp.sh(&pp.main, "git add -A && git commit -qm init && git push -q origin main");
        pp
    }

    fn sh(&self, dir: &Path, script: &str) -> String {
        let o = self.cmd("bash").arg("-c").arg(script).current_dir(dir).output().unwrap();
        assert!(o.status.success(), "{script}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }

    fn cmd(&self, program: &str) -> Command {
        let mut c = Command::new(program);
        c.env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", self.root.join("bin").display()))
            .env("HOME", &self.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .env("CHIMERA_POST_PUSH_DIR", self.root.join("pp"))
            .env("CHIMERA_POST_PUSH_OUT", &self.out)
            .env("CHIMERA_POST_PUSH_POLL", "1")
            .env("CHIMERA_PUSH_TESTS", "affected")
            .env("CHIMERA_DISK_FLOOR_GB", "0");
        c
    }

    /// Commits and pushes one change to main; its short hash.
    fn push(&mut self) -> String {
        self.n += 1;
        self.sh(
            &self.main,
            &format!("echo {0} >f && git add f && git commit -qm c{0} && git push -q origin main", self.n),
        );
        self.sh(&self.main, "git rev-parse --short HEAD")
    }

    /// Starts `tools/post-push-tests.sh` from the main checkout, as a push does.
    fn run(&self, i: usize) -> std::process::Child {
        let err = File::create(self.root.join(format!("run{i}.err"))).unwrap();
        self.cmd("bash")
            .arg(self.main.join("tools/post-push-tests.sh"))
            .current_dir(&self.main)
            .stdout(std::process::Stdio::null())
            .stderr(err)
            .spawn()
            .unwrap()
    }

    fn file(&self, name: &str) -> String {
        fs::read_to_string(self.out.join(name)).unwrap_or_default()
    }

    /// Waits for every run to end (a minute at most), then checks no
    /// leftover process holds the lock and kills them.
    fn finish(&self, runs: Vec<std::process::Child>) -> String {
        let up = Instant::now();
        for mut r in runs {
            while r.try_wait().unwrap().is_none() {
                if up.elapsed() > Duration::from_secs(90) {
                    r.kill().ok();
                }
                pause(100);
            }
        }
        let lock = self.out.join(".lock");
        let mut holders = Vec::new();
        for pid in fs::read_to_string(&self.leaks).unwrap_or_default().split_whitespace() {
            for fd in fs::read_dir(format!("/proc/{pid}/fd")).into_iter().flatten().flatten() {
                if fs::read_link(fd.path()).is_ok_and(|p| p == lock) {
                    holders.push(pid.to_string());
                }
            }
            Command::new("kill").arg(pid).status().ok();
        }
        let errs: String = (0..8)
            .map(|i| fs::read_to_string(self.root.join(format!("run{i}.err"))).unwrap_or_default())
            .collect();
        assert!(up.elapsed() < Duration::from_secs(90), "the runs never ended: {errs}");
        assert!(holders.is_empty(), "processes a run left behind hold its lock: {holders:?}");
        errs
    }
}

/// Pushes arriving while a slow run tests: the run finishes and records its
/// verdict, the waiters replace each other, and the last one tests the
/// latest main over the whole untested range. A run started from a copy of
/// the script that differs from the pushed one re-executes the pushed one,
/// and nothing a run leaves behind keeps the lock (a leaked jobserver
/// holder had starved every run for six hours).
#[test]
fn post_push_under_frequent_pushes_records_a_verdict_for_the_latest_main() {
    let ok = |c: &str| Command::new(c).arg("--version").output().is_ok();
    if !ok("flock") || !ok("git") || !ok("setsid") {
        return;
    }
    let mut pp = PostPush::new("post_push_queue", "3");
    let first = pp.push();
    // The main checkout's copy is older than the pushed one.
    let script = pp.main.join("tools/post-push-tests.sh");
    let mut s = fs::read_to_string(&script).unwrap();
    s.push_str("# an older copy\n");
    fs::write(&script, s).unwrap();
    let mut runs = vec![pp.run(0)];
    let up = Instant::now();
    while !pp.file("last.log").contains("=== the tests") {
        assert!(up.elapsed() < Duration::from_secs(30), "the first run never reached its tests");
        pause(50);
    }
    let mut last = String::new();
    for i in 1..4 {
        last = pp.push();
        runs.push(pp.run(i));
        pause(400);
    }
    let errs = pp.finish(runs);
    let history = pp.file("history.txt");
    let verdicts: Vec<String> =
        history.lines().map(|l| l.split(' ').take(2).collect::<Vec<_>>().join(" ")).collect();
    assert_eq!(verdicts, [format!("PASS {first}"), format!("PASS {last}")], "{history}\n{errs}");
    assert!(errs.contains("running the pushed copy"), "{errs}");
    let log = fs::read_dir(&pp.out)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            let n = p.file_name().unwrap().to_string_lossy().into_owned();
            n.ends_with(".log") && n != "last.log" && fs::read_to_string(p).unwrap().contains(&format!("..{last})"))
        })
        .expect("the last run's log");
    let log = fs::read_to_string(log).unwrap();
    assert!(log.contains(&format!("({first}..{last})")), "{log}");
    fs::remove_dir_all(&pp.root).ok();
}

/// A lock no live run holds (here an outside `flock`, as a process that
/// inherited it): the waiting run moves it aside after two polls and
/// tests, and the inbox gets STALE lines, for the lock and for the hours
/// without a verdict.
#[test]
fn post_push_takes_a_stale_lock_over_and_reports_starvation() {
    let ok = |c: &str| Command::new(c).arg("--version").output().is_ok();
    if !ok("flock") || !ok("git") || !ok("setsid") {
        return;
    }
    let mut pp = PostPush::new("post_push_stale", "0");
    fs::write(pp.out.join("history.txt"), "PASS 0000000 2020-01-01T00:00Z 1m tests: x\n").unwrap();
    fs::write(pp.out.join("pushes.txt"), "1111111 2020-01-02T00:00Z main x -\n").unwrap();
    let mut holder = Command::new("flock").arg(pp.out.join(".lock")).args(["sleep", "120"]).spawn().unwrap();
    let up = Instant::now();
    while Command::new("flock").arg("-n").arg(pp.out.join(".lock")).arg("true").status().unwrap().success() {
        assert!(up.elapsed() < Duration::from_secs(10), "the outside lock was never taken");
        pause(20);
    }
    let sha = pp.push();
    let errs = pp.finish(vec![pp.run(0)]);
    let holder_pid = holder.id();
    holder.kill().ok();
    holder.wait().ok();
    let (history, inbox) = (pp.file("history.txt"), pp.file("inbox"));
    assert!(history.contains(&format!("PASS {sha}")), "{history}\n{errs}");
    assert!(inbox.contains("no post-push verdict for over"), "{inbox}");
    assert!(inbox.contains(&format!("held by no live run but by: {holder_pid} (flock")), "{inbox}");
    fs::remove_dir_all(&pp.root).ok();
}

/// An idle period (the last verdict long ago) followed by one push is no
/// starvation: no STALE line.
#[test]
fn post_push_idle_period_then_one_push_is_not_stale() {
    let ok = |c: &str| Command::new(c).arg("--version").output().is_ok();
    if !ok("flock") || !ok("git") || !ok("setsid") {
        return;
    }
    let mut pp = PostPush::new("post_push_idle", "0");
    fs::write(pp.out.join("history.txt"), "PASS 0000000 2020-01-01T00:00Z 1m tests: x\n").unwrap();
    let sha = pp.push();
    let errs = pp.finish(vec![pp.run(0)]);
    let (history, inbox) = (pp.file("history.txt"), pp.file("inbox"));
    assert!(history.contains(&format!("PASS {sha}")), "{history}\n{errs}");
    assert!(!inbox.contains("STALE"), "{inbox}");
    fs::remove_dir_all(&pp.root).ok();
}
