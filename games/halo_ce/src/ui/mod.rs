//! Halo's front end on the client shell (the `ui` feature): its weapons'
//! own buttons (`binds`: `+melee`, `+zoom`) and its HUD (`hud`:
//! specs/halo_ce/hud.md, drawn from the disc's HUD tags in place of the
//! shell's fallback crosshair and status).

pub mod hud;

pub mod fx_lights;

use bevy::app::PluginGroupBuilder;
use chimera_world::game_bundle::GameClient;

/// The shell's plugins with Halo's HUD after the shell's own
/// (`hud::HaloHudPlugin`, its material's renderer).
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
        .add_after::<chimera_client::client::hud::HudPlugin>(hud::HaloHudPlugin)
        .add_after::<hud::HaloHudPlugin>(hud::material::HudRenderPlugin)
}

/// The weapons' buttons beside fire, attack2 (also the zoom) and reload:
/// the melee and the zoom as named actions (`game::weapons::systems`).
pub fn binds(app: &mut bevy::app::App) {
    use chimera_client::client::binds::BindsAppExt;
    use crate::game::weapons::systems::{MELEE_ACTION, ZOOM_ACTION};
    app.bound_action(MELEE_ACTION).bound_action(ZOOM_ACTION);
    fx_lights::plugin(app);
}

/// Halo's front end in an app made of several games (beside its
/// `game::BUNDLE`): its HUD while a Halo gun is in hand (`with_hud`) and
/// its weapons' buttons (`binds`).
pub static CLIENT: GameClient = GameClient { game: crate::game::GAME, hud: Some(with_hud), app: Some(binds) };

/// Halo's front end under the name every game's has it (`ui::party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
