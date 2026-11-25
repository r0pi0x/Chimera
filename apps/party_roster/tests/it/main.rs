//! `party_roster`'s cross-game tests, one test binary: every game's HUD,
//! kill feed, ladders and bodies together, the party's start-up, the
//! converted prints. It names every game the party composes (below), so a
//! change to any of them selects it (checks/src/affected.rs, "tests by
//! game"). Tests of one game go in that game's binary (`tests/<game>/`),
//! of the party as such in `tests/party/`.
//! `cargo test -p party_roster --features dev --test it`.

// Every game the party composes: a change to any selects this binary.
use {combat_arms as _, cs_source as _, halo_ce as _, hl2dm as _, originals as _, tf2 as _};

// Shared helpers (tests/common/): each binary includes the ones it uses.
#[path = "../common/halo_party.rs"]
mod halo_party;
#[path = "../common/huds.rs"]
mod huds;
#[path = "../common/mixed.rs"]
mod mixed;
#[path = "../common/play.rs"]
mod play;

mod arena_wall;
mod character_fit;
mod halo_maps;
mod kill_feed;
mod ladders_across_games;
mod overlay_guard;
mod props_across_games;
mod push_across_games;
mod startup;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod converted;
    mod converted_prints;
    mod halo_party_hud;
    mod report_spots;
}
