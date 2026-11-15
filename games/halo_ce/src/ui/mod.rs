//! Halo's front end on the client shell (the `ui` feature): nothing of its
//! own yet, so a Halo app shows the shell's fallback HUD (crosshair,
//! health and ammo). Its HUD comes with specs/halo_ce/hud.md.

use bevy::app::PluginGroupBuilder;
use chimera_world::game_bundle::GameClient;

/// The shell's plugins with Halo's HUD in its place: none drawn yet, the
/// shell's own stays.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
}

/// Halo's front end in an app made of several games (beside its
/// `game::BUNDLE`): no HUD there (`hud: None`) until it draws one of its
/// own; no player holds a Halo gun yet, so the party's HUD checks
/// (its kill feed, the overlay guard) don't ask for one.
pub static CLIENT: GameClient = GameClient { game: crate::game::GAME, hud: None, app: None };

/// Halo's front end under the name every game's has it (`ui::party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
