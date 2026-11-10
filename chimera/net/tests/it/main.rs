//! `chimera_net`'s integration tests, as one test binary (each file a
//! module): contract tests of prediction and replication on `NetSim` with
//! the engine's base alone (`SimBasePlugins`), the greybox and the
//! placeholder movement (a minigame's callouts with the test kit's guns);
//! nothing of a game. `cargo test -p chimera_net
//! --features dev --test it`; where tests go: docs/OBSERVABILITY.md,
//! section 1.

mod console_names;
mod dedicated;
mod game_hooks;
mod minigame_callouts;
mod prediction;
mod spawn_protection;
