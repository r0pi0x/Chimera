//! A match's first frames have nothing they show still coming in, on the
//! host and on a client that joined it (`chimera_client::client::
//! first_view`, docs/performance.md, "A map's first view"): every mesh the
//! cameras see on the GPU, its material prepared, its pipeline compiled, no
//! model still loading, from the frame the lobby's cover goes. The client loads the server's ladder's Combat Arms models
//! behind its loading screen, as the host does: the server's manifest
//! carries the ladder (`MinigameOffer::ladder`). Before: a joined client's
//! ladder models loaded in play, and the map's meshes no camera saw while
//! it settled (warehouse's decals, glass and light rays) and the first
//! rung's view model compiled their pipelines as the cover went (7-9
//! meshes for 15 frames; a pistol for 28 on the client).
//!
//! Two of this crate's binaries offscreen: a listen host (`ugg_new 7 13
//! combat_arms:warehouse` a second in) and a client joining it five seconds
//! in (it comes in during the level change), their counts logged
//! (`CHIMERA_FIRST_VIEW_LOG`); each run ends once the client's log has the
//! frames after its cover went. Needs Combat Arms (an install); passes with
//! a note without one or without a GPU adapter. About a minute.
//!
//! `cargo test -p ultimate_gun_game --features dev --test it heavy::first_view -- --nocapture`

use std::{
    fs::File,
    net::UdpSocket,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use chimera_core::mount::config::LocalConfig;

/// Frames logged after a cover goes, and checked.
const FRAMES: usize = 4;
/// How long the pair may take.
const TIMEOUT: Duration = Duration::from_secs(300);

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

/// A game process, killed when dropped, its output in `log`.
struct Game(Child);

impl Drop for Game {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(dir: &Path, name: &str, args: &[String]) -> (Game, PathBuf) {
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

fn read(log: &Path) -> String {
    std::fs::read(log).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// The last `FRAMES` lines logged after a cover went (`shown+<n>`), after
/// the last map's settle.
fn last_shown(log: &str) -> Vec<&str> {
    let after = log.rfind("settled behind the loading screen").map_or(log, |at| &log[at..]);
    let lines: Vec<&str> = after.lines().filter(|l| l.contains("first_view") && l.contains(" shown+")).collect();
    lines[lines.len().saturating_sub(FRAMES)..].to_vec()
}

fn check(who: &str, log: &str) {
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

#[test]
fn a_hosts_and_a_joined_clients_first_frames_have_nothing_pending() {
    if !combat_arms_installed() {
        eprintln!("skipping: needs the Combat Arms install");
        return;
    }
    let dir = std::env::temp_dir().join(format!("chimera_ugg_first_view_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A port the system picks (never Source's 27015).
    let port = UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    let (mut host, host_log) = start(
        &dir,
        "Host",
        &args(&format!("-port {port} +maxplayers 4 +after 1 ugg_new 7 13 combat_arms:warehouse +after 280 quit")),
    );
    std::thread::sleep(Duration::from_secs(5));
    let (mut client, client_log) = start(&dir, "Client", &args(&format!("+connect 127.0.0.1:{port} +after 280 quit")));
    let began = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let (h, c) = (read(&host_log), read(&client_log));
        let gone = [&mut host, &mut client].into_iter().any(|g| g.0.try_wait().ok().flatten().is_some());
        let in_play = |log: &str| {
            let shown = log.rfind(&format!("first_view shown+{} ", FRAMES - 1));
            let settled = log.rfind("settled behind the loading screen");
            shown.zip(settled).is_some_and(|(s, l)| s > l)
        };
        let host_in = in_play(&h) && h.contains("starts on combat_arms:warehouse");
        if (in_play(&c) && host_in) || gone || began.elapsed() > TIMEOUT {
            break;
        }
    }
    drop((host, client));
    let (h, c) = (read(&host_log), read(&client_log));
    let _ = std::fs::remove_dir_all(&dir);
    if [&h, &c].iter().any(|l| l.contains("adapter") && l.contains("panicked")) {
        eprintln!("skipping: no GPU adapter\n{c}");
        return;
    }
    assert!(h.contains("starts on combat_arms:warehouse"), "the host's match didn't start on warehouse\n{h}");
    check("the host", &h);
    assert!(
        c.contains("the coming ladder loads") && c.contains("behind the loading screen"),
        "the client didn't load the server's ladder behind its loading screen\n{c}"
    );
    check("the client", &c);
}
