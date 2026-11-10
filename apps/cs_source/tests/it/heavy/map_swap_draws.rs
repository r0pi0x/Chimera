//! A map swapped in (the `map` command, not `--map`) draws its world: the
//! swap's loads cap the GPU uploads (`console::LoadCaps`, `upload_pace`),
//! so a world mesh reaches the GPU frames after its entity was first
//! specialized, which Bevy then skipped for good (`upload_pace::
//! respecialize_late_meshes`). Before the fix de_dust2 entered from the
//! greybox drew nothing but a few props (dm_lockdown and Combat Arms'
//! bloodmoney likewise in Ultimate Gun Game, 2026-11-10): the screenshot's
//! mean grey was about 1 of 255, with its world about 37.
//!
//! The game binary offscreen: starts on the greybox, `map de_dust2` a
//! second in, a screenshot once it is in, then quits. Needs CS:S (an
//! install, or the asset store standing in); passes with a note without
//! one or without a GPU adapter.

use std::process::Command;

use chimera_core::mount::config::LocalConfig;

#[test]
fn a_map_swapped_in_draws_its_world() {
    let installed = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return;
    }
    let dir = std::env::temp_dir().join(format!("chimera_map_swap_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shot = dir.join("swapped.png");
    let out = Command::new(env!("CARGO_BIN_EXE_cs_source"))
        .args(["--offscreen", "320x200", "--map", "greybox", "+after", "1", "map", "de_dust2", "+after", "25"])
        .arg("screenshot")
        .arg(&shot)
        .args(["+after", "27", "quit"])
        .env("CHIMERA_CFG_DIR", &dir)
        .output()
        .expect("the game binary runs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() && (stderr.contains("adapter") || stderr.contains("Adapter")) {
        eprintln!("skipping: no GPU adapter\n{stderr}");
        return;
    }
    assert!(out.status.success(), "exit {:?}\n{stderr}", out.status);
    let log = format!("{stdout}{stderr}");
    assert!(log.contains("loading cs_source:de_dust2"), "the swap ran\n{log}");
    let image = image::open(&shot).unwrap_or_else(|e| panic!("{}: {e}\n{log}", shot.display())).to_luma8();
    let mean = image.pixels().map(|p| p.0[0] as f64).sum::<f64>() / image.pixels().len() as f64;
    assert!(mean > 15.0, "the swapped-in map's world is drawn: mean grey {mean:.1} of 255 (about 1 when it isn't)");
    let _ = std::fs::remove_dir_all(&dir);
}
