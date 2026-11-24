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
//! `cargo test -p ultimate_gun_game --features dev --test base heavy::first_view -- --nocapture`

use chimera_core::mount::config::LocalConfig;

#[path = "../../common/first_view_pair.rs"]
mod pair;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

#[test]
fn a_hosts_and_a_joined_clients_first_frames_have_nothing_pending() {
    if !combat_arms_installed() {
        eprintln!("skipping: needs the Combat Arms install");
        return;
    }
    let Some((h, c)) = pair::run_pair("combat_arms:warehouse") else { return };
    pair::check("the host", &h);
    assert!(
        !pair::join_timed_out(&c),
        "the client's join timed out twice (it never got the server's welcome)\n{c}\n--- the host's log ---\n{h}"
    );
    assert!(
        c.contains("the coming ladder loads") && c.contains("behind the loading screen"),
        "the client didn't load the server's ladder behind its loading screen\n{c}\n--- the host's log ---\n{h}"
    );
    pair::check("the client", &c);
}
