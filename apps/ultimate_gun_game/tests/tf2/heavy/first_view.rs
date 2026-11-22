//! TF2's ctf_2fort's first frames have nothing they show still coming in,
//! on a listen host and on a client that joined it (the same check as
//! `base`'s warehouse one, tests/common/first_view_pair.rs). ctf_2fort has
//! a 3D skybox: its props are drawn by the sky camera, which draws the
//! whole sky from above while the map settles behind the cover
//! (`chimera_present::render::sky`, `first_view_warm`); before, a client
//! had 16 of them compiling for about 8 frames after its cover went.
//!
//! Needs the TF2 install (`CHIMERA_TF2_ROOT`); passes with a note without
//! one or without a GPU adapter. A couple of minutes.
//!
//! `cargo test -p ultimate_gun_game --features dev --test tf2 heavy::first_view -- --nocapture`

use chimera_core::mount::config::LocalConfig;

#[path = "../../common/first_view_pair.rs"]
mod pair;

fn tf2_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config.game_path("tf2").is_some_and(|p| chimera_core::mount::served::usable("tf2", &p, "tf"))
}

#[test]
fn ctf_2forts_first_frames_have_nothing_pending_on_a_host_and_a_client() {
    if !tf2_installed() {
        eprintln!("skipping: needs the TF2 install");
        return;
    }
    let Some((h, c)) = pair::run_pair("tf2:ctf_2fort") else { return };
    // ctf_2fort's settle takes 9-17 s on a loaded box and can run out of its
    // 20 s there (docs/performance.md): that says nothing of the sky.
    if pair::timed_out(&h) || pair::timed_out(&c) {
        eprintln!("skipping: a settle ran out of time (a loaded box)");
        return;
    }
    pair::check("the host", &h);
    pair::check("the client", &c);
}
