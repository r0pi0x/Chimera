//! The engine's stable base (docs/ARCHITECTURE.md, "Crates"): the shared
//! vocabulary (`core`), the console and its scope (`console_scope`: the
//! game played, the games' namespaces), the slot registries (`slots`), the
//! character's components (`character`), the performance metrics, the
//! mounts (`mount`: the VFS over a game's files, local config, finding
//! installs), the environment variables and per-user folders under
//! their new and old names (`env`), game events and the server log (`game_log`), the games an app plays
//! (`game_registry`: what a game tells the shell about itself) and the
//! gameplay (`gameplay`: which game fills each match-wide slot), the parts
//! each game lends a game made of several (`game_parts`), announcers'
//! packs and kill facts (`announcer`), minigame cue packs (`cues`), queries
//! kept between calls of exclusive code (`query_cache`), values worked out
//! once and kept by their owner (`memo`), values made on first use
//! (`later`). Code
//! everything else uses and that changes rarely, in its own crate so an
//! edit higher up doesn't recompile it.
//!
//! The game crate re-exports each module at its old path (`mashup::core`,
//! `mashup::console`, ...); `mashup::core` adds the build's `VERSION`.

pub mod announcer;
pub mod character;
pub mod console;
pub mod console_names;
pub mod console_scope;
pub mod core;
pub mod cues;
pub mod env;
pub mod game_log;
pub mod game_parts;
pub mod gameplay;
pub mod game_registry;
pub mod later;
pub mod memo;
pub mod metrics;
pub mod mount;
pub mod profile;
pub mod query_cache;
pub mod slots;
