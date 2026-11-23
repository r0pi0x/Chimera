//! `ultimate_gun_game`'s tests that name Halo: CE (the sandbox's Halo guns):
//! a change to Halo selects this binary and the cross-game one (`it`)
//! (checks/src/affected.rs, "tests by game").
//! `cargo test -p ultimate_gun_game --features dev --test halo_ce`.

// The helpers (tests/common/): the ladder's party and what reads it.
#[path = "../common/parts.rs"]
mod parts;

mod melee_bash;
mod sandbox;
