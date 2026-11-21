//! Team Fortress 2 (Steam app 440) as a game crate on Chimera
//! (docs/plans/active/tf2.md): the fifth game library, on Source's loader
//! layer beside CS:S and HL2:DM. So far (M1): what the game tells the
//! shell about itself (`game::INFO`), its install as a mount
//! (`game::mount`, from its `tf/gameinfo.txt`; the dedicated server's
//! layout, whose texture and sound archives are missing, mounts too), its
//! maps (`game::maps`, `game::MAP_LOADER`: Source's conversion with TF2's
//! data, spawns from `info_player_teamspawn`); (M2a) its encrypted
//! scripts (`game::ice`, `game::scripts`) and asset sets (`game::sets`);
//! (M2b) its guns (`game::weapons`, `game::view`); and (M3) its classes
//! (`game::classes`) and movement (`game::movement`: Source's with TF2's
//! config, the class of the gun in hand). Source behaviour it shares with the other
//! Source games comes from `source_engine`, never from another game.
//!
//! `SimPlugins`: everything TF2's simulation needs but its parts (the
//! engine's base, Source's map logic, the tick metrics), what its apps run
//! with `game::Tf2Plugins` after it; `harness`: TF2 on the engine's
//! headless harness (`harness::Tf2Sim`, its scenario tests' base). Its
//! front end on the client shell (its HUD, its guns' kill icons) is its own
//! crate above this one, `tf2_ui` (games/tf2/ui).

pub mod game;
pub mod harness;

// `crate::game_registry`, `crate::map` and the rest inside, as in the other
// game crates (the architecture check reads `crate::<module>` against its
// layering table).
use chimera_core::{console, core, game_registry, gameplay, mount, slots};
use chimera_sim::weapon;
use chimera_world::map;

use bevy::prelude::*;

/// Everything TF2's simulation needs but its parts, with no window,
/// rendering or input: the engine's base (`chimera_sim::SimBasePlugins`),
/// Source's map logic (`source_logic`: doors, triggers on its maps) and
/// the tick metrics. Its parts (`game::Tf2Plugins`) go after it.
pub struct SimPlugins;

impl Plugin for SimPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_sim::SimBasePlugins)
            .add_plugins(source_logic::logic::LogicPlugin)
            .add_plugins(chimera_core::metrics::TickMetricsPlugin);
    }
}
