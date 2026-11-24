//! The first-view tests' pair of game processes (`base`'s warehouse,
//! `tf2`'s ctf_2fort; chimera_client::client::first_view, docs/performance.md,
//! "A map's first view"): a listen host and a client joining it, offscreen,
//! their counts logged (`CHIMERA_FIRST_VIEW_LOG`), and the check that every
//! frame after a cover went has nothing pending.

use std::{
    fs::File,
    net::UdpSocket,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// Frames logged after a cover goes, and checked.
pub const FRAMES: usize = 4;
/// How long the pair may take.
pub const TIMEOUT: Duration = Duration::from_secs(300);

/// A game process, killed when dropped, its output in `log`.
pub struct Game(pub Child);

impl Drop for Game {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn start(dir: &Path, name: &str, args: &[String]) -> (Game, PathBuf) {
    let log = dir.join(format!("{name}.log"));
    let cfg = dir.join(format!("cfg-{name}"));
    std::fs::create_dir_all(&cfg).unwrap();
    let out = File::create(&log).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_ultimate_gun_game"))
        .args(["--offscreen", "320x180", "+volume", "0", "+name", name])
        .args(args)
        .env("CHIMERA_CFG_DIR", &cfg)
        .env("CHIMERA_FIRST_VIEW_LOG", FRAMES.to_string())
        .env("CHIMERA_RUNG_SEED", "7")
        .stdout(Stdio::from(out.try_clone().unwrap()))
        .stderr(Stdio::from(out))
        .spawn()
        .expect("the game binary runs");
    (Game(child), log)
}

pub fn read(log: &Path) -> String {
    std::fs::read(log).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// Whether `line` is first_view's own counts line (logged by its module;
/// the log may be coloured): a frame-time hitch line (`frame_metrics`)
/// quotes the frame's log events, a first_view line among them, cut short
/// (on a loaded box the hitch came after the counts and was taken for
/// them, its `main=[]` cut off).
fn counts(line: &str) -> bool {
    line.contains("client::first_view") && !line.contains("frame_metrics") && line.contains("first_view shown+")
}

/// The last `FRAMES` lines logged after a cover went (`shown+<n>`), after
/// the last map's settle: first_view's own lines alone (`counts`).
pub fn last_shown(log: &str) -> Vec<&str> {
    let after = log.rfind("settled behind the loading screen").map_or(log, |at| &log[at..]);
    let lines: Vec<&str> = after.lines().filter(|l| counts(l)).collect();
    lines[lines.len().saturating_sub(FRAMES)..].to_vec()
}

/// The settle gave up waiting (its 20 s: a loaded box's slow shader compiles).
pub fn timed_out(log: &str) -> bool {
    log.contains("out of time; still pending")
}

pub fn check(who: &str, log: &str) {
    assert!(!log.contains("out of time; still pending"), "{who}: the settle ran out of time\n{log}");
    let shown = last_shown(log);
    assert_eq!(shown.len(), FRAMES, "{who}: no {FRAMES} frames logged after the cover went\n{log}");
    for line in shown {
        // Not `compiling=`: the HUD's own pipelines come with the cover's
        // going (1-2 for a frame), none a mesh in view waits for.
        for field in ["meshes=0 ", "materials=0 ", "pipelines=0 ", "main=[]"] {
            assert!(line.contains(field), "{who}: a first frame after the cover has {field:?}: {line}");
        }
    }
}


/// A listen host (`ugg_new 7 13 <map>` a second in) and a client joining it
/// five seconds in, run until the client's log has the frames after its
/// cover went (or the host's match is up); the two logs, host first. `None`
/// when there is no GPU adapter.
pub fn run_pair(map: &str) -> Option<(String, String)> {
    let dir = std::env::temp_dir().join(format!("chimera_ugg_first_view_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A port the system picks (never Source's 27015).
    let port = UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    let (mut host, host_log) = start(
        &dir,
        "Host",
        &args(&format!("-port {port} +maxplayers 4 +after 1 ugg_new 7 13 {map} +after 280 quit")),
    );
    std::thread::sleep(Duration::from_secs(5));
    let (mut client, client_log) = start(&dir, "Client", &args(&format!("+connect 127.0.0.1:{port} +after 280 quit")));
    let began = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let (h, c) = (read(&host_log), read(&client_log));
        let gone = [&mut host, &mut client].into_iter().any(|g| g.0.try_wait().ok().flatten().is_some());
        let in_play = |log: &str| {
            let last = format!("first_view shown+{} ", FRAMES - 1);
            let shown = log.lines().rev().find(|l| counts(l) && l.contains(&last)).map(|l| l.as_ptr() as usize - log.as_ptr() as usize);
            let settled = log.rfind("settled behind the loading screen");
            shown.zip(settled).is_some_and(|(s, l)| s > l)
        };
        let host_in = in_play(&h) && h.contains(&format!("starts on {map}"));
        if (in_play(&c) && host_in) || gone || began.elapsed() > TIMEOUT {
            break;
        }
    }
    drop((host, client));
    let (h, c) = (read(&host_log), read(&client_log));
    let _ = std::fs::remove_dir_all(&dir);
    if [&h, &c].iter().any(|l| l.contains("adapter") && l.contains("panicked")) {
        eprintln!("skipping: no GPU adapter\n{c}");
        return None;
    }
    assert!(h.contains(&format!("starts on {map}")), "the host's match didn't start on {map}\n{h}");
    Some((h, c))
}
