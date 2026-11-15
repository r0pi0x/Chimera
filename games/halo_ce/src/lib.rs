//! Halo: Combat Evolved (the original Xbox's) as a game crate on Chimera
//! (docs/plans/active/halo-ce.md): the fourth game library, on the `blam`
//! loader layer (`blam_formats`). So far a skeleton: what the game tells
//! the shell about itself (`game::INFO`), its install, a disc image or a
//! folder of the disc's files, as a mount (`game::mount`), the
//! multiplayer maps' names, and a map loader that says it is not written
//! yet (`game::maps`). Movement, weapons, maps and the rest come as their
//! specs (specs/halo_ce/) are implemented.
//!
//! `SimPlugins`: everything Halo's simulation needs but its parts (so far
//! the engine's base and the tick metrics), what its apps run with
//! `game::HaloPlugins` after it; `harness`: Halo on the engine's headless
//! harness (`harness::HaloSim`, its scenario tests' base). With the `ui`
//! feature, `ui` is its front end on the client shell.

pub mod game;
pub mod harness;
// The front end: built on the client shell (the `ui` feature).
#[cfg(feature = "ui")]
pub mod ui;

// `crate::game_registry`, `crate::map` and the rest inside, as in the other
// game crates (the architecture check reads `crate::<module>` against its
// layering table).
use chimera_core::{game_registry, gameplay, mount};
use chimera_world::map;

use bevy::prelude::*;

/// Everything Halo's simulation needs but its parts, with no window,
/// rendering or input: the engine's base (`chimera_sim::SimBasePlugins`)
/// and the tick metrics. Its parts (`game::HaloPlugins`) go after it.
pub struct SimPlugins;

impl Plugin for SimPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_sim::SimBasePlugins)
            .add_plugins(chimera_core::metrics::TickMetricsPlugin);
    }
}
