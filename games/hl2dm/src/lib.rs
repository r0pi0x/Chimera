//! Half-Life 2: Deathmatch (Steam app 320) as a game crate on Chimera
//! (docs/plans/active/hl2dm.md): a third game library beside CS:S and
//! Combat Arms. So far: what the game tells the shell about itself
//! (`game::INFO`), its install as a mount (`game::mount`, from its
//! `gameinfo.txt`), its maps (`game::maps`, `game::MAP_LOADER`), its
//! movement and suit (M3) and its bullet and melee weapons
//! (`game::weapons`, M4; `game::Hl2dmPlugins`). Source
//! behaviour it shares with CS:S comes from `source_engine`, never from
//! `cs_source` (a lift, not a copy).
//!
//! `SimPlugins`: everything HL2:DM's simulation needs but its parts (the
//! engine's base, Source's map logic, the tick metrics), what its apps
//! run with `game::Hl2dmPlugins` after it; `harness`: HL2:DM on the
//! engine's headless harness (`harness::Hl2Sim`, its scenario tests'
//! base).

pub mod game;
pub mod harness;
pub mod rules;

// `crate::game_registry`, `crate::map` and the rest inside, as in the
// other game crates (the architecture check reads `crate::<module>`
// against its layering table).
use chimera_core::{console, core, game_log, game_registry, mount, slots};
use chimera_sim::weapon;
use chimera_world::map;

use bevy::prelude::*;

/// Everything HL2:DM's simulation needs but its parts, with no window,
/// rendering or input: the engine's base (`chimera_sim::SimBasePlugins`),
/// Source's map logic (`source_logic`: doors, buttons, triggers on its
/// maps) and the tick metrics. Its parts (`game::Hl2dmPlugins`) go after
/// it.
pub struct SimPlugins;

impl Plugin for SimPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_sim::SimBasePlugins)
            .add_plugins(source_logic::logic::LogicPlugin)
            .add_plugins(chimera_core::metrics::TickMetricsPlugin);
    }
}
