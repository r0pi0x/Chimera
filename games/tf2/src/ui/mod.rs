//! TF2's front end on the client shell (the `ui` feature): nothing of its
//! own yet, so a TF2 app shows the shell's fallback HUD (crosshair, health
//! and ammo). TF2's HUD is out of scope for now (docs/plans/active/tf2.md,
//! section 3, decision 7).

use bevy::app::PluginGroupBuilder;
use chimera_world::game_bundle::GameClient;

/// The shell's plugins with TF2's HUD in its place: none drawn yet, the
/// shell's own stays.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
}

/// TF2's front end in an app made of several games (beside its
/// `game::BUNDLE`).
pub static CLIENT: GameClient = GameClient { game: crate::game::GAME, hud: Some(with_hud), app: None };

/// TF2's front end under the name every game's has it (`ui::party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
