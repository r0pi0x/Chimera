//! Halo's front end on the client shell (the `ui` feature): its weapons'
//! own buttons (`binds`: `+melee`, `+zoom`) and no HUD of its own yet, so
//! a Halo app shows the shell's fallback HUD (crosshair, health and ammo).
//! Its HUD comes with specs/halo_ce/hud.md.

use bevy::app::PluginGroupBuilder;
use chimera_world::game_bundle::GameClient;

/// The shell's plugins with Halo's HUD in its place: none drawn yet, the
/// shell's own stays.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
}

/// The weapons' buttons beside fire, attack2 (also the zoom) and reload:
/// the melee and the zoom as named actions (`game::weapons::systems`).
pub fn binds(app: &mut bevy::app::App) {
    use chimera_client::client::binds::BindsAppExt;
    use crate::game::weapons::systems::{MELEE_ACTION, ZOOM_ACTION};
    app.bound_action(MELEE_ACTION).bound_action(ZOOM_ACTION);
}

/// Halo's front end in an app made of several games (beside its
/// `game::BUNDLE`): its weapons' buttons (`binds`); no HUD there
/// (`hud: None`) until it draws one of its own: a Halo rung shows the
/// fallback HUD.
pub static CLIENT: GameClient = GameClient { game: crate::game::GAME, hud: None, app: Some(binds) };

/// Halo's front end under the name every game's has it (`ui::party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
