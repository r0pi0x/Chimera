//! `ultimate_gun_game`'s tests that name TF2 (the sandbox's TF2 guns,
//! platform, range and target staging): a change to TF2 selects this binary
//! and the cross-game one (`it`) (checks/src/affected.rs, "tests by game").
//! `cargo test -p ultimate_gun_game --features dev --test tf2`.

// The helpers (tests/common/): the ladder's party and what reads it.
#[path = "../common/parts.rs"]
mod parts;

mod sandbox_gun_files;
mod sandbox_platform;
mod sandbox_range;
mod target_staging;
