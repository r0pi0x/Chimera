//! A map's first gameplay frame has nothing it shows still coming in
//! (`chimera_client::client::first_view`): every mesh the camera sees on
//! the GPU, its material prepared, its pipeline compiled, no pipeline
//! still compiling, no late-built model pending. Before the fix the game
//! was entered with the map's pipelines still compiling, its models
//! popping in in front of the player.
//!
//! The game binary offscreen: starts on the greybox, `map de_dust2` a
//! second in, its counts logged every frame (`CHIMERA_FIRST_VIEW_LOG`),
//! then quits. Needs CS:S (an install, or the asset store standing in);
//! passes with a note without one or without a GPU adapter.

use std::process::Command;

use chimera_core::mount::config::LocalConfig;

#[test]
fn the_first_gameplay_frame_has_nothing_pending() {
    let installed = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return;
    }
    let dir = std::env::temp_dir().join(format!("chimera_first_view_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cs_source"))
        .args(["--offscreen", "320x200", "--map", "greybox", "+volume", "0", "+after", "1", "map", "de_dust2"])
        .args(["+after", "40", "quit"])
        .env("CHIMERA_CFG_DIR", &dir)
        .env("CHIMERA_FIRST_VIEW_LOG", "3")
        .output()
        .expect("the game binary runs");
    let _ = std::fs::remove_dir_all(&dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() && (stderr.contains("adapter") || stderr.contains("Adapter")) {
        eprintln!("skipping: no GPU adapter\n{stderr}");
        return;
    }
    assert!(out.status.success(), "exit {:?}\n{stderr}", out.status);
    let log = format!("{stdout}{stderr}");
    assert!(!log.contains("out of time; still pending"), "the settle ran out of time\n{log}");
    let first = log
        .lines()
        .find(|l| l.contains("first_view in+0 "))
        .unwrap_or_else(|| panic!("no first gameplay frame logged\n{log}"));
    for field in ["meshes=0 ", "materials=0 ", "pipelines=0 ", "compiling=0 ", "main=[]"] {
        assert!(first.contains(field), "the first gameplay frame has {field:?}: {first}");
    }
    assert!(!first.contains("visible=0 "), "the camera saw the map: {first}");
}
