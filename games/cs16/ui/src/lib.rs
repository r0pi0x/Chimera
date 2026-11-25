//! Counter-Strike 1.6's front end on the client shell, its own crate above the
//! game's simulation (`cs16`) so that the simulation and its tests never
//! build the shell: its HUD while a CS 1.6 weapon is in hand (`hud`:
//! specs/cs16/hud.md's health, armour, ammunition, damage arcs, dynamic
//! crosshair, weapon selection and round timer from the
//! install's sprites, at the 640 or the 320 layout) and CS 1.6's kill icons in
//! every app's kill feed (`hud::load_pictures`).

pub mod hud;

use bevy::app::PluginGroupBuilder;
use chimera_world::game_bundle::GameClient;

use cs16::game::GAME;

/// The shell's plugins with CS 1.6's HUD after the shell's own.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell.add_after::<chimera_client::client::hud::HudPlugin>(hud::Cs16HudPlugin)
}

/// The game's front end in an app made of several games (beside its
/// `game::BUNDLE`): its HUD while a CS 1.6 weapon is in hand.
pub static CLIENT: GameClient = GameClient { game: GAME, ui_crate: env!("CARGO_CRATE_NAME"), hud: Some(with_hud), app: None, base: None };

/// The front end under the name every game's has it (`party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
