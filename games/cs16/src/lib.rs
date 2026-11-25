//! Counter-Strike 1.6 as a game crate on Chimera (docs/plans/active/cs16.md),
//! made by tools/new-game: what the game tells the shell about itself
//! (`game::INFO`), its install as a mount (`game::mount`), its maps
//! (`game::MAP_LOADER`), a stand-in movement (`game::movement`) and its
//! weapons as data with their rules as hooks (`game::weapons`). Each part
//! is replaced from its spec in specs/cs16/ (docs/CONTRIBUTING-GAMES.md).
//!
//! `SimPlugins`: everything the game's simulation needs but its parts (the
//! engine's base, the tick metrics); `harness`: the game on the engine's
//! headless harness (`harness::Cs16Sim`). Its front end is its own
//! crate above this one, `cs16_ui` (games/cs16/ui).

pub mod game;
pub mod harness;

// `crate::game_registry`, `crate::map` and the rest inside, as in the other
// game crates (the architecture check reads `crate::<module>` against its
// layering table).
#[allow(unused_imports)]
use chimera_core::{core, game_registry, gameplay, mount, slots};
#[allow(unused_imports)]
use chimera_world::map;

use bevy::prelude::*;

/// Everything the game's simulation needs but its parts, with no window,
/// rendering or input: the engine's base (`chimera_sim::SimBasePlugins`)
/// and the tick metrics. Its parts (`game::Cs16Plugins`) go after it.
pub struct SimPlugins;

impl Plugin for SimPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_sim::SimBasePlugins).add_plugins(chimera_core::metrics::TickMetricsPlugin);
    }
}
