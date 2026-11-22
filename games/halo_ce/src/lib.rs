//! Halo: Combat Evolved (the original Xbox's) as a game crate on Chimera
//! (docs/plans/active/halo-ce.md): the fourth game library, on the `blam`
//! loader layer (`blam_formats`). So far: what the game tells
//! the shell about itself (`game::INFO`), its install, a disc image or a
//! folder of the disc's files, as a mount (`game::mount`), and its maps
//! converted to Chimera's map data (`game::maps`). Movement, weapons and
//! the rest come as their specs (specs/halo_ce/) are implemented.
//!
//! `SimPlugins`: everything Halo's simulation needs but its parts (so far
//! the engine's base and the tick metrics), what its apps run with
//! `game::HaloPlugins` after it; `harness`: Halo on the engine's headless
//! harness (`harness::HaloSim`, its scenario tests' base). Its front end on
//! the client shell is its own crate above this one, `halo_ce_ui`
//! (games/halo_ce/ui).

pub mod game;
pub mod harness;

// `crate::game_registry`, `crate::map` and the rest inside, as in the other
// game crates (the architecture check reads `crate::<module>` against its
// layering table).
use chimera_core::{core, game_registry, gameplay, mount, slots};
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
