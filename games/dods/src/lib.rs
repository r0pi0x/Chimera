//! Day of Defeat: Source (Steam app 300; the dedicated server, app
//! 232290) as a game crate on Chimera (docs/plans/active/dods.md): the
//! sixth game library, on Source's loader layer beside CS:S, HL2:DM and
//! TF2. So far (M0): what the game tells the shell about itself
//! (`game::INFO`), its install as a mount (`game::mount`, from its
//! `dod/gameinfo.txt`), its maps (`game::maps`, `game::MAP_LOADER`:
//! Source's conversion, spawns from `info_player_allies` and
//! `info_player_axis`), its twelve class bodies (`game::bodies`), a
//! stand-in movement with its stances (`game::movement`) and one stand-in
//! gun (`game::weapons`) so its bodies can be worn in the party apps.
//! Source behaviour it shares with the other Source games comes from
//! `source_engine`, never from another game.
//!
//! `SimPlugins`: everything DoD:S's simulation needs but its parts (the
//! engine's base, Source's map logic, the tick metrics); `harness`:
//! DoD:S on the engine's headless harness (`harness::DodsSim`). Its front
//! end is its own crate above this one, `dods_ui` (games/dods/ui).

pub mod game;
pub mod harness;

// `crate::game_registry`, `crate::map` and the rest inside, as in the other
// game crates (the architecture check reads `crate::<module>` against its
// layering table).
use chimera_core::{core, game_registry, gameplay, mount, slots};
use chimera_world::map;

use bevy::prelude::*;

/// Everything DoD:S's simulation needs but its parts, with no window,
/// rendering or input: the engine's base (`chimera_sim::SimBasePlugins`),
/// Source's map logic (`source_logic`) and the tick metrics. Its parts
/// (`game::DodsPlugins`) go after it.
pub struct SimPlugins;

impl Plugin for SimPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_sim::SimBasePlugins)
            .add_plugins(source_logic::logic::LogicPlugin)
            .add_plugins(chimera_core::metrics::TickMetricsPlugin);
    }
}
