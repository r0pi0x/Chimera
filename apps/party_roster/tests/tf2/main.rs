//! `party_roster`'s tests of TF2 in the party, one test binary: its rungs,
//! melee and maps on the party's simulation. A change to TF2 selects this
//! binary and the cross-game one (`it`), not the other games'
//! (checks/src/affected.rs, "tests by game").
//! `cargo test -p party_roster --features dev --test tf2`.

// Shared helpers (tests/common/): each binary includes the ones it uses.
#[path = "../common/local.rs"]
mod local;
#[path = "../common/play.rs"]
mod play;

mod tf2_pool;
mod tf2_rungs;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod tf2_maps;
}
