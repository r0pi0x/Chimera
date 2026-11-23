//! Day of Defeat: Source's front end on the client shell, its own crate
//! above the game's simulation (`dods`) so that the simulation and its
//! tests never build the shell (docs/plans/active/architecture-for-iteration.md,
//! stage 1; docs/plans/active/dods.md). No HUD of its own yet (a DoD:S
//! gun in hand shows the shell's minimal HUD: `GameParts::hud` None); its
//! HUD (health, stamina, ammo, crosshair) is M3's. Its prone key, `+alt1`
//! (`party_app`), is bound to Z in the party (`game::BUNDLE`'s binds).

use bevy::app::PluginGroupBuilder;
use chimera_world::game_bundle::GameClient;

use dods::game::GAME;

/// The shell's plugins as they are: DoD:S draws no HUD of its own yet.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
}

/// DoD:S's front end in an app made of several games (beside its
/// `game::BUNDLE`): its prone key (`party_app`).
pub static CLIENT: GameClient = GameClient { game: GAME, ui_crate: env!("CARGO_CRATE_NAME"), hud: None, app: Some(party_app), base: None };

/// The party apps' front end: `+alt1` (prone, `dods::game::stance`) read
/// from the keys bound to it.
pub fn party_app(app: &mut bevy::app::App) {
    use chimera_client::client::binds::BindsAppExt;
    app.bound_action(dods::game::stance::PRONE_ACTION);
}

/// DoD:S's front end under the name every game's has it (`party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
