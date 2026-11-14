//! `chimera_net`'s integration tests, as one test binary (each file a
//! module): contract tests of prediction and replication on `NetSim` with
//! the engine's base alone (`SimBasePlugins`), the greybox and the
//! placeholder movement (a minigame's callouts with the test kit's guns);
//! nothing of a game. `cargo test -p chimera_net
//! --features dev --test it`; where tests go: docs/OBSERVABILITY.md,
//! section 1.

mod asset_offer;
mod clock_hitches;
mod assets;
mod console_names;
mod data_formats;
mod dedicated;
mod game_hooks;
mod local_load;
mod minigame_callouts;
mod prematch_lobby;
mod prediction;
mod reconnect;
mod rejoin;
mod spawn_protection;
