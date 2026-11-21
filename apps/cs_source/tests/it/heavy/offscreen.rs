//! The game binary with `--offscreen` (`chimera_client::client::offscreen`):
//! no window system, the whole client (renderer, pipelined rendering, HUD,
//! menus) drawing into an image. It starts on the greybox (no install
//! needed), runs its frames, saves a screenshot and frame dumps of the
//! image's size that aren't blank, writes the frame log, prints its frame
//! times and exits by itself. Passes with a note where wgpu finds no
//! adapter. Also: runs that end a few frames in, while pipelines still
//! compile on the async compute threads, exit cleanly
//! (`chimera_client::client::exit_drain`). And a post pass whose settings
//! component leaves the camera stops (`mat_monitorgamma` back at its
//! default must not leave the frame white).

use std::process::Command;

#[test]
fn the_game_runs_offscreen_and_saves_its_frames() {
    let dir = std::env::temp_dir().join(format!("chimera_offscreen_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("shot.png");
    let dumps = dir.join("dumps");
    let log = dir.join("frames.csv");
    let out = Command::new(env!("CARGO_BIN_EXE_cs_source"))
        .args(["--offscreen", "320x200", "--map", "greybox", "--frames", "40", "--dump-every", "10"])
        .arg("--screenshot")
        .arg(&shot)
        .arg("--dump")
        .arg(&dumps)
        .arg("--frame-log")
        .arg(&log)
        // Not the user's config folder or audio device.
        .env("CHIMERA_CFG_DIR", &dir)
        .output()
        .expect("the game binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() && (stderr.contains("adapter") || stderr.contains("Adapter")) {
        eprintln!("skipping: no GPU adapter\n{stderr}");
        return;
    }
    assert!(out.status.success(), "exit {:?}\n{stderr}", out.status);
    assert!(stderr.contains("offscreen 320x200:"), "the frame times are printed\n{stderr}");

    let image = image::open(&shot).unwrap_or_else(|e| panic!("{}: {e}\n{stderr}", shot.display())).to_rgb8();
    assert_eq!(image.dimensions(), (320, 200), "the screenshot is the offscreen size");
    let first = image.get_pixel(0, 0);
    assert!(image.pixels().any(|p| p != first), "the screenshot shows something");

    // Frames 10, 20, 30 and 40 asked for; the last may not arrive before
    // the run ends.
    let frames: Vec<_> = std::fs::read_dir(&dumps).map(|d| d.flatten().collect()).unwrap_or_default();
    assert!(frames.len() >= 3, "frame dumps: {frames:?}\n{stderr}");
    assert!(dumps.join("frame_000000.png").exists());

    let lines = std::fs::read_to_string(&log).unwrap_or_default();
    let count = |world: &str| lines.lines().filter(|l| l.starts_with(world)).count();
    assert!(count("main,") >= 30 && count("render,") >= 30, "the frame log has every frame\n{lines}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A run ended while its pipelines still compile: before
/// `exit_drain`, the device could be destroyed on a compile thread while
/// the main thread ran the exit handlers, and about one exit in five
/// crashed in the GPU driver (SIGSEGV, or `free(): invalid size`). Several
/// short runs, each must end with success and no signal, and the exit
/// must have waited for compiling pipelines in at least one (three frames
/// in, dozens still compile: the wait is what is tested, since the crash
/// itself is a race that a few runs rarely hit).
#[test]
fn runs_ended_while_pipelines_compile_exit_cleanly() {
    let dir = std::env::temp_dir().join(format!("chimera_offscreen_exit_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut waited = 0;
    for run in 0..6 {
        let out = Command::new(env!("CARGO_BIN_EXE_cs_source"))
            .args(["--offscreen", "320x200", "--map", "greybox", "--frames", "3"])
            .env("CHIMERA_CFG_DIR", &dir)
            .output()
            .expect("the game binary runs");
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !out.status.success() && (stderr.contains("adapter") || stderr.contains("Adapter")) {
            eprintln!("skipping: no GPU adapter\n{stderr}");
            return;
        }
        let tail: String = stderr.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        assert!(out.status.success(), "run {run}: exit {:?}\n{tail}", out.status);
        if let Some(line) = stderr.lines().find(|l| l.contains("exit: waited")) {
            eprintln!("run {run}: {line}");
            waited += 1;
        }
    }
    assert!(waited > 0, "no run's exit waited for compiling pipelines (`exit: waited` in the log)");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `mat_monitorgamma` off its default and back: the UI camera's
/// `MonitorGamma` is removed, and its uniform index stayed in the render
/// world, so the gamma pass ran on with zeroed settings and turned every
/// frame white (seen 2026-11-21 in runs with `+wait`, where a config's
/// repaired brightness came back to 2.2 a few dozen frames in;
/// `chimera_client::client::gamma::gamma_pass`).
#[test]
fn brightness_back_at_its_default_leaves_the_frame_as_it_was() {
    let dir = std::env::temp_dir().join(format!("chimera_offscreen_gamma_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("shot.png");
    let out = Command::new(env!("CARGO_BIN_EXE_cs_source"))
        .args(["--offscreen", "320x200", "--map", "greybox", "--frames", "150"])
        .arg("--screenshot")
        .arg(&shot)
        .args(["+volume", "0", "+mat_monitorgamma", "1.6", "+wait", "40", "+mat_monitorgamma", "2.2"])
        .env("CHIMERA_CFG_DIR", &dir)
        .output()
        .expect("the game binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() && (stderr.contains("adapter") || stderr.contains("Adapter")) {
        eprintln!("skipping: no GPU adapter\n{stderr}");
        return;
    }
    assert!(out.status.success(), "exit {:?}\n{stderr}", out.status);
    let image = image::open(&shot).unwrap_or_else(|e| panic!("{}: {e}\n{stderr}", shot.display())).to_rgb8();
    let white = image.pixels().filter(|p| p.0.iter().all(|&c| c >= 250)).count();
    let all = image.pixels().count();
    assert!(white * 2 < all, "{white} of {all} pixels white: the gamma pass ran on after its removal");
    let _ = std::fs::remove_dir_all(&dir);
}
