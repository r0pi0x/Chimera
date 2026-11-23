//! `ultimate_gun_game`'s tests that name DoD:S (its bodies in the
//! sandbox, its rungs): a change to DoD:S selects this binary and the cross-game one
//! (`it`) (checks/src/affected.rs, "tests by game").
//! `cargo test -p ultimate_gun_game --features dev --test dods`.

// The helpers (tests/common/): the ladder's party and what reads it.
#[path = "../common/parts.rs"]
mod parts;

mod rungs;
mod sandbox_bodies;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod sandbox_targets;
    mod party_map_kit;
}
