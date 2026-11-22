//! What the scenario tests run on, at the paths the CS:S app re-exports it
//! (`cs_source_app::core`, `::map`, `::harness`, ...: where these tests
//! lived until 2026-10-15), so a test reads the same here as there:
//! Chimera's crates, Source's logic, CS:S's simulation, the harness with CS:S's base
//! (`harness::CsSim`), the greybox, and maps from the install
//! (`games::load_map`, tests/common/mod.rs). `mod stack;` in a binary's
//! main.rs; `use crate::stack::{core::Intent, harness::{CsSim, Sim}};`.
//!
//! Only re-exports: anything a test needs that isn't here belongs in its
//! crate, not in this file.

#![allow(unused_imports)]

pub use chimera_core::{character, console, game_log, game_registry, metrics, mount, slots};

/// The shared vocabulary (`chimera_core::core`).
pub mod core {
    pub use chimera_core::core::*;
}

/// Default fixed tick rate (`core::DEFAULT_TICK_HZ`).
pub use chimera_core::core::DEFAULT_TICK_HZ;

/// The engine's map with the presentation's `MapPlugin` (what draws and
/// plays it: the tests' maps are the game's, drawn-part requests and all).
pub mod map {
    pub use chimera_present::MapPlugin;
    pub use chimera_world::map::*;
}

pub use chimera_present::{audio, render};
pub use chimera_sim::{movement, weapon};
pub use chimera_world::{greybox, harness::single_threaded_schedules};
pub use cs_source::{bot, objectives, rules, swap_map};
pub use source_logic::logic;

/// Network play with CS:S's and Source's replicated types beside it.
pub mod net {
    pub use chimera_net::net::*;
    pub use cs_source::game::net::{
        ClassRequest, NetBomb, NetHostage, NetRound, ObjectiveNews, RoundOver, TeamRequest, bomb_outcome, round_phase,
    };
    pub use source_logic::logic::net::{MapHud, NetMover, mover_flags};
}

/// The headless harness: `Sim` (chimera_world), `NetSim` (chimera_net),
/// CS:S's base and test maps (`cs_source::harness`).
pub mod harness {
    pub use chimera_net::harness::*;
    pub use chimera_world::harness::*;
    pub use cs_source::harness::*;
}

/// Maps by id from the install, and CS:S's parts (`cs_source::game`)
/// (the front end's: `cs_source_ui`'s own tests).
pub mod games {
    pub use crate::common::{
        load_map, load_map_file, load_map_lean, load_map_level, load_map_level_lean, load_map_with, map_file_bytes,
    };

    pub mod cs_source {
        pub use ::cs_source::game::*;
    }
}

