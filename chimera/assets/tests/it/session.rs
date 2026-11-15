//! A session's diagnostics (`feedback::session`, docs/plans/active/
//! feedback-reports.md section 13): the log's ring, file cap and rotation,
//! secrets filtered before the disk; the marker of a session that died
//! found at the next start (a simulated restart) while a live one's is
//! not; a panic in a child process leaving a held crash bundle with its
//! backtrace, log tail and health lines and no secret, which the next
//! start offers and sends into the outbox.

use std::{fs, process::Command};

use chimera_assets::feedback::{
    Kind, bundle,
    session::{self, CrashPolicy, FILE_CAP, KEEP, RING_LINES, Session, SessionOptions},
};

use super::{feedback::PLANTED, scratch};

fn options(root: &std::path::Path) -> SessionOptions {
    SessionOptions {
        root: root.to_path_buf(),
        prefix: "session".into(),
        app: "ultimate_gun_game".into(),
        build: "test-build".into(),
        net_version: "net53".into(),
    }
}

/// Lines with secrets in them, as a log would get them.
fn secret_lines() -> Vec<String> {
    vec![
        "] rcon_password PlantedRcon77".into(),
        "INFO assets: bound the token to https://198.51.100.23:27031".into(),
        "] +chimera:asset_token chimera1_5e3c0ffee5e3c0ffee5e3c0ffee5e3c0ffee".into(),
        concat!("INFO ui: opened C:\\", "Users", "\\PlantUser\\AppData\\Local\\chimera").into(),
        concat!("loading /", "home", "/plantuser/.local/share/chimera/cfg/config.cfg").into(),
        "Connecting to 203.0.113.77:27015...".into(),
    ]
}

fn assert_clean(what: &str, text: &str) {
    for p in PLANTED {
        assert!(!text.contains(p), "{what} holds {p:?}:\n{text}");
    }
}

#[test]
fn the_log_keeps_a_ring_caps_its_file_and_rotates_old_sessions() {
    let root = scratch("session_log");
    let (s, previous) = Session::start(options(&root)).unwrap();
    assert!(previous.is_empty());
    for l in secret_lines() {
        s.line(&l);
    }
    s.health("health rss=812M peak=1.1G");
    let text = fs::read_to_string(s.log_path()).unwrap();
    assert_clean("the log file", &text);
    assert!(text.contains("rcon_password (hidden)") && text.contains("(address)"), "{text}");
    assert_eq!(s.health_lines().len(), 1);
    // The ring keeps the last lines; the file starts over past its cap.
    let filler = "x".repeat(180);
    let n = (FILE_CAP as usize / 190) + 2000;
    for i in 0..n {
        s.line(&format!("line {i} {filler}"));
    }
    let tail = s.tail();
    assert_eq!(tail.len(), RING_LINES);
    assert!(tail.last().unwrap().contains(&format!("line {} ", n - 1)));
    let size = fs::metadata(s.log_path()).unwrap().len();
    assert!(size <= FILE_CAP + 64 * 1024, "{size}");
    let text = fs::read_to_string(s.log_path()).unwrap();
    assert!(text.contains("(earlier lines cut"), "the file started over");
    assert!(text.trim_end().ends_with(&format!("line {} {filler}", n - 1)));
    s.end();
    assert!(!s.marker_path().exists());
    // More sessions than kept: the oldest logs go.
    for _ in 0..KEEP + 2 {
        let (s, previous) = Session::start(options(&root)).unwrap();
        assert!(previous.is_empty(), "every earlier session ended cleanly");
        s.line("a session");
        s.end();
    }
    let logs: Vec<_> = fs::read_dir(root.join("logs"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
        .collect();
    assert_eq!(logs.len(), KEEP);
}

#[test]
fn a_session_that_died_is_found_at_the_next_start() {
    let root = scratch("session_marker");
    let (a, _) = Session::start(options(&root)).unwrap();
    a.line("the map loads");
    a.health("health rss=7.9G avail=120M/16G commit=31G/32G");
    a.set_context("map: cs_source:de_dust2\nrung: 7\n");
    // A second instance while the first runs: the first's marker is held.
    let (b, previous) = Session::start(options(&root)).unwrap();
    assert!(previous.is_empty(), "a live session isn't a dead one");
    b.end();
    // The first dies (its lock goes as a process's would).
    a.abandon();
    drop(a);
    let (c, previous) = Session::start(options(&root)).unwrap();
    assert_eq!(previous.len(), 1);
    let p = &previous[0];
    assert_eq!((p.info.app.as_str(), p.info.build.as_str()), ("ultimate_gun_game", "test-build"));
    assert!(!p.crashed());
    let report = p.report();
    assert_eq!(report.kind, Kind::Crash);
    assert!(report.text.starts_with("The session ended unexpectedly"), "{}", report.text);
    assert_eq!(report.health.len(), 1);
    assert!(report.game.contains("rung: 7"), "{}", report.game);
    let id = previous.into_iter().next().unwrap().send().unwrap();
    let zip = fs::read(root.join("feedback").join("outbox").join(format!("{id}.zip"))).unwrap();
    let opened = bundle::open(&zip).unwrap();
    assert_eq!(opened.info.kind, Kind::Crash);
    assert!(opened.entries.iter().any(|e| e.name == "health.txt"));
    c.end();
    // Settled: not found again.
    let (d, previous) = Session::start(options(&root)).unwrap();
    assert!(previous.is_empty());
    d.end();
}

/// The child of `a_panic_leaves_a_held_crash_bundle`: a session in the
/// folder it names, then a panic (nothing without the variable).
#[test]
fn panic_child() {
    let Some(root) = std::env::var_os("CHIMERA_TEST_PANIC_DIR") else { return };
    let (s, _) = Session::start(options(std::path::Path::new(&root))).unwrap();
    s.make_current();
    session::install_panic_hook();
    session::set_policy(CrashPolicy::Ask);
    for l in secret_lines() {
        s.line(&l);
    }
    s.health("health rss=812M peak=1.1G");
    s.set_context("map: cs_source:de_dust2\nrung: 3\n");
    s.set_system(concat!("os: windows\nexe: C:\\", "Users", "\\PlantUser\\ugg.exe\n"));
    panic!("a panic on purpose, token Zq9xK2mP7vL4tR8wN3bY6hJ1");
}

#[test]
fn a_panic_leaves_a_held_crash_bundle_the_next_start_sends() {
    let root = scratch("session_panic");
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["session::panic_child", "--exact", "--nocapture", "--test-threads=1"])
        .env("CHIMERA_TEST_PANIC_DIR", &root)
        .env("RUST_BACKTRACE", "0")
        .output()
        .unwrap();
    assert!(!out.status.success(), "the child panicked");
    // The restart.
    let (s, previous) = Session::start(options(&root)).unwrap();
    assert_eq!(previous.len(), 1, "{}", String::from_utf8_lossy(&out.stderr));
    let p = previous.into_iter().next().unwrap();
    assert!(p.crashed());
    let held = p.held().expect("the bundle is held (policy ask)");
    let opened = bundle::open(&fs::read(&held).unwrap()).unwrap();
    assert_eq!(opened.info.kind, Kind::Crash);
    let entry = |name: &str| {
        opened.entries.iter().find(|e| e.name == name).map(|e| String::from_utf8_lossy(&e.data).into_owned()).unwrap_or_default()
    };
    for name in ["backtrace.txt", "log.txt", "health.txt", "game.txt", "system.txt"] {
        assert!(!entry(name).is_empty(), "{name}");
    }
    assert!(entry("backtrace.txt").contains("a panic on purpose"), "{}", entry("backtrace.txt"));
    assert!(entry("backtrace.txt").contains("session.rs"), "the place");
    assert!(entry("log.txt").contains("rcon_password (hidden)"));
    assert!(entry("game.txt").contains("rung: 3"));
    assert!(opened.info.text.starts_with("panic in thread"), "{}", opened.info.text);
    for e in &opened.entries {
        assert_clean(&e.name, &String::from_utf8_lossy(&e.data));
    }
    assert_clean("report.json's text", &opened.info.text);
    // Send: the held bundle moves into the outbox, the marker goes.
    let id = p.send().unwrap();
    assert!(root.join("feedback").join("outbox").join(format!("{id}.zip")).is_file());
    assert!(!held.exists());
    s.end();
    let (s, previous) = Session::start(options(&root)).unwrap();
    assert!(previous.is_empty());
    s.end();
}

#[test]
fn a_crash_follows_the_policy() {
    let root = scratch("session_policy");
    let (s, _) = Session::start(options(&root)).unwrap();
    assert!(s.crash_with("render error: DeviceLost: gone", "", CrashPolicy::Never).is_none());
    let (s2, _) = Session::start(SessionOptions { prefix: "server".into(), ..options(&root) }).unwrap();
    let path = s2.crash_with("render error: OutOfMemory: ", "", CrashPolicy::Always).unwrap();
    assert!(path.starts_with(root.join("feedback").join("outbox")));
    // A crash keeps the marker through a clean end: offered next time.
    s2.end();
    assert!(s2.marker_path().exists());
    s.end();
    let (s3, previous) = Session::start(SessionOptions { prefix: "server".into(), ..options(&root) }).unwrap();
    assert_eq!(previous.len(), 1);
    assert!(previous[0].crashed());
    s3.end();
}
