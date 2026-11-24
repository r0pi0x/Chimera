//! The report's screenshot is the frame the player saw when they asked
//! for it (`chimera_client::client::feedback`): the game binary offscreen
//! (`chimera_client::client::offscreen`, no window system), a report
//! asked for at the main menu, in the pre-match lobby (`ui_scene lobby
//! between`) and in a match on the greybox, each frame kept by
//! `feedback_shot` (the JPEG the report sends) and compared with a
//! screenshot of the window taken at the key: the UI is in it (the
//! lobby's frame was black while the lobby hid itself for the report's
//! overlay from the key press, report 3025fbb1), at the window's size,
//! and the report dialog, drawn after the frame was taken, is not.
//! Each run is a scene script (`chimera_client::client::scene`): the key
//! is pressed once nothing loads (`wait_for idle`) and no logo pass
//! between screens is under way (`wait_for still`), not a count of
//! frames after the screen change: on a loaded box 60 frames could end
//! inside the lobby's 1.2 s pass, its bands moving between the key's
//! frame and the report's (3 frames later in a menu), and a quarter of
//! the UI "lost". The pictures of a failing run are kept in the target's
//! tmp folder (`report_frame-<name>-*`). Passes with a note where wgpu
//! finds no adapter.

use std::{path::Path, process::Command};

const SIZE: (u32, u32) = (640, 360);

/// The game on `script` (a scene's lines) with `opts`, in `dir`.
fn run(dir: &Path, name: &str, opts: &[&str], script: &[String]) -> String {
    let scene = dir.join(format!("{name}.scene"));
    std::fs::write(&scene, script.join("\n")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ultimate_gun_game"))
        .args(["--offscreen", &format!("{}x{}", SIZE.0, SIZE.1)])
        .args(opts)
        .arg("--scene")
        .arg(&scene)
        .arg("--out")
        .arg(dir.join(format!("{name}-scene")))
        // Not the user's config folder.
        .env("CHIMERA_CFG_DIR", dir)
        .output()
        .expect("the game binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success() || no_adapter(&stderr), "exit {:?}\n{stderr}", out.status);
    stderr
}

/// The scene's lines until the screen is settled: nothing loading, no
/// logo pass under way (a few frames after the line that changed it).
fn settled() -> Vec<String> {
    lines("wait 2\nwait_for idle 3600\nwait_for still 3600\nwait 5")
}

/// The scene's lines for one report: the window's screenshot at the key
/// (`<name>-seen.png`), the report, its frame (`<name>-report.jpg`) and
/// the window with the dialog open (`<name>-dialog.png`), then cancelled.
/// The key and the seen screenshot are one frame's lines.
fn report(dir: &Path, name: &str) -> Vec<String> {
    let p = |f: &str| dir.join(format!("{name}-{f}")).display().to_string();
    [
        settled(),
        vec![format!("screenshot {}", p("seen.png")), "feedback".into(), "wait 60".into()],
        vec![format!("feedback_shot {}", p("report.jpg")), format!("screenshot {}", p("dialog.png"))],
        lines("wait 10\nfeedback_press cancel\nwait 10"),
    ]
    .concat()
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(String::from).collect()
}

/// wgpu found no GPU (or software) adapter: nothing to draw with.
fn no_adapter(stderr: &str) -> bool {
    stderr.contains("Unable to find a GPU") || stderr.contains("no adapter") || stderr.contains("NoAdapter")
}

fn luma(path: &Path) -> Option<image::GrayImage> {
    image::open(path).ok().map(|i| i.to_luma8())
}

/// Of the pixels bright in `seen` (UI: text, panels' edges), the share
/// that differ much in `other`.
fn lost(seen: &image::GrayImage, other: &image::GrayImage) -> f32 {
    let (mut bright, mut differ) = (0u32, 0u32);
    for (a, b) in seen.pixels().zip(other.pixels()) {
        if a.0[0] > 110 {
            bright += 1;
            if (a.0[0] as i32 - b.0[0] as i32).abs() > 60 {
                differ += 1;
            }
        }
    }
    assert!(bright > 200, "the screen shows some UI ({bright} bright pixels)");
    differ as f32 / bright as f32
}

/// The share of pixels in the window's middle that differ much.
fn middle_change(a: &image::GrayImage, b: &image::GrayImage) -> f32 {
    let (w, h) = a.dimensions();
    let (mut n, mut differ) = (0u32, 0u32);
    for y in h / 4..h * 3 / 4 {
        for x in w / 4..w * 3 / 4 {
            n += 1;
            if (a.get_pixel(x, y).0[0] as i32 - b.get_pixel(x, y).0[0] as i32).abs() > 30 {
                differ += 1;
            }
        }
    }
    differ as f32 / n as f32
}

fn check(dir: &Path, name: &str, stderr: &str) {
    let file = |f: &str| dir.join(format!("{name}-{f}"));
    let (Some(seen), Some(shot), Some(dialog)) = (luma(&file("seen.png")), luma(&file("report.jpg")), luma(&file("dialog.png"))) else {
        panic!("{name}: the pictures were saved\n{stderr}");
    };
    // A failing run's pictures, kept (the scratch folder goes).
    let keep = || {
        for f in ["seen.png", "report.jpg", "dialog.png"] {
            let _ = std::fs::copy(file(f), Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("report_frame-{name}-{f}")));
        }
    };
    assert_eq!(shot.dimensions(), SIZE, "{name}: the report's frame is the window's size");
    let lost = lost(&seen, &shot);
    if lost >= 0.2 {
        keep();
    }
    assert!(lost < 0.2, "{name}: the report's frame shows the UI seen at the key ({:.0}% of it lost)", lost * 100.0);
    let dialog_in = middle_change(&dialog, &shot);
    if dialog_in <= 0.05 {
        keep();
    }
    assert!(dialog_in > 0.05, "{name}: the report's frame is not the one with the dialog ({:.1}% differ)", dialog_in * 100.0);
}

#[test]
fn a_reports_frame_shows_the_ui_seen_at_the_key_without_the_dialog() {
    let dir = std::env::temp_dir().join(format!("chimera_report_frame_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // The main menu, then the lobby over it.
    let mut script = lines("volume 0\nmainmenu");
    script.extend(report(&dir, "menu"));
    script.extend(lines("ui_scene lobby between"));
    script.extend(report(&dir, "lobby"));
    let stderr = run(&dir, "menus", &[], &script);
    if !dir.join("menu-seen.png").exists() && no_adapter(&stderr) {
        eprintln!("skipping: no GPU adapter\n{stderr}");
        return;
    }
    check(&dir, "menu", &stderr);
    check(&dir, "lobby", &stderr);
    // A match on the greybox: the HUD.
    let mut script = lines("volume 0\nwait_for spawned 3600");
    script.extend(report(&dir, "match"));
    let stderr = run(&dir, "match", &["--map", "greybox"], &script);
    check(&dir, "match", &stderr);
    let _ = std::fs::remove_dir_all(&dir);
}
