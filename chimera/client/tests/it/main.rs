//! `chimera_client`'s integration tests, as one test binary (each file a
//! module): the client shell's parts on the headless harness
//! (`chimera_world::harness::Sim`) with the engine's base alone
//! (`chimera_sim::SimBasePlugins`); nothing of a game. `cargo test -p
//! chimera_client --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod match_end;
mod callouts;
mod dead_view;
mod game_app;
mod hit_marker;
mod lobby_panel;
mod logo_pass;
mod map_load_steps;
mod swap_effect;
mod text_warm;
mod metrics;
mod minigame_panel;
mod minigame_scoreboard;
mod race_chip;
mod rank_toast;
mod match_effects;
mod signal_feed;
mod respawn_countdown;
