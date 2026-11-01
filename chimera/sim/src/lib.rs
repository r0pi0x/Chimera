//! The simulation's frameworks (docs/ARCHITECTURE.md, "Crates"): the
//! weapon system (`weapon`: weapons as parts, inventory, the weapon frame,
//! economy, grenades, drops, lag compensation, sprays), the movement
//! slot's own implementations (`movement`: placeholder walking, noclip),
//! the rule-module interface (`rule_module`: a game registers its
//! modes by name), the mode runtime (`mode`: a game mode as data and
//! hooks on one match whose phases it steps, lifted from Combat Arms'),
//! minigames (`minigame`: a match described by a data file, played on the
//! mode runtime by Chimera's own flow, with the `chimera:ladder` rules) and
//! what every game's respawn does (`spawning`: the dead
//! held still, a character revived at a place) and the announcer's
//! counting of kills (`announce`). A game fills them with its data and hooks; they name
//! no game. It uses the engine's base (`chimera_core`) and the
//! simulation's map (`chimera_world`) and nothing above them, so an edit in
//! a game or the client doesn't rebuild it.
//!
//! The game crate re-exports `weapon` and `movement` at their old paths
//! (`mashup::weapon`, `mashup::movement`) and `rule_module` as
//! `mashup::rules::module`.
//!
//! `SimBasePlugins`: the engine's simulation without a game (the fixed
//! tick, `core`, the console, game events and the server log, the movement
//! slot's implementations, the weapon framework, the announcer's
//! counting): the start of a game's set (`cs_source::SimPlugins`)
//! and the base of the engine's own scenario tests (tests/it/).

pub mod announce;
pub mod identity;
pub mod minigame;
pub mod mode;
pub mod movement;
pub mod rule_module;
pub mod server_docs;
pub mod spawning;
#[cfg(feature = "testkit")]
pub mod testkit;
pub mod weapon;

// `crate::core`, `crate::map` and the rest inside, as in the game crate.
use chimera_core::{character, console, core, game_log, slots};
use chimera_world::map;

use bevy::prelude::*;

/// The engine's simulation without a game: the fixed tick at
/// `core::DEFAULT_TICK_HZ` (until a game or map sets its own), `core`,
/// the console, game events and the server log (`game_log`), the movement
/// slot's implementations, the weapon
/// framework and the announcer's counting (`announce`). A game's set starts with it (`cs_source::SimPlugins`); the
/// engine's tests run on it (`harness::Sim::new(SimBasePlugins, ..)`).
#[derive(Clone, Copy)]
pub struct SimBasePlugins;

impl Plugin for SimBasePlugins {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(core::DEFAULT_TICK_HZ))
            .add_plugins((
                core::CorePlugin,
                console::ConsolePlugin,
                chimera_core::console_scope::ConsoleScopePlugin,
                movement::MovementPlugins,
            ))
            .add_plugins(game_log::GameLogPlugin)
            .add_plugins(weapon::WeaponPlugin)
            .add_plugins(announce::AnnouncePlugin);
    }
}
