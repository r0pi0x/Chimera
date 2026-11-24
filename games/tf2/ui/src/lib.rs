//! TF2's front end on the client shell, its own crate above the game's
//! simulation (`tf2`) so that the simulation and its tests never build the
//! shell (docs/plans/active/architecture-for-iteration.md, stage 1; M3 of
//! docs/plans/active/tf2.md; specs/tf2/hud.md): its HUD while a TF2 gun is
//! in hand, drawn under its own root (`HudRoot(GAME)`) by its own unit
//! (H / 480): the health cross with its number, the overheal and low-health
//! pulses, the ammo panel in the team's colour (clip and reserve, or one
//! number), the crosshair, an item's meter (a watch's cloak), the player
//! under the crosshair (a disguised Spy as his disguise); in TF2's fonts and colours from the install's
//! scheme (its look comes with the map: `tf2::game::hud`). Its guns' kill icons
//! join every app's kill feed (`systems::register_kill_icons`); the feed's
//! rows are the app's.
//!
//! - `hud`: the frame of draws from the local player's state (tests read
//!   it without a window);
//! - `systems`: the Bevy side (inputs, nodes, kill icons).

pub mod hud;
pub mod systems;

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_world::game_bundle::GameClient;

use tf2::game::GAME;

/// TF2's HUD: its state, root and nodes, drawing while its HUD is the
/// gameplay's.
pub struct Tf2HudPlugin;

impl Plugin for Tf2HudPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .get_resource_or_init::<chimera_client::client::hud::HudDraws>()
            .0
            .push((GAME, systems::DRAWS));
        // Its kill feed is the app's: it shows none of its own.
        chimera_client::client::kill_feed::GameKillFeedAppExt::register_game_kill_feed(app, GAME, |_| 0);
        app.init_resource::<systems::Tf2HudState>()
            .init_resource::<systems::Tf2Target>()
            .init_resource::<chimera_client::client::weapon_icons::WeaponIcons>()
            .add_systems(Startup, systems::spawn_root)
            .add_systems(
                Update,
                (
                    (systems::target, systems::collect, systems::sync).chain().run_if(chimera_core::gameplay::hud_is(GAME)),
                    systems::register_kill_icons,
                ),
            );
    }
}

/// The shell's plugins with TF2's HUD after the shell's own.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell.add_after::<chimera_client::client::hud::HudPlugin>(Tf2HudPlugin)
}

/// TF2's front end in an app made of several games (beside its
/// `game::BUNDLE`): its HUD while a TF2 gun is in hand.
pub static CLIENT: GameClient = GameClient { game: GAME, ui_crate: env!("CARGO_CRATE_NAME"), hud: Some(with_hud), app: None, base: None };

/// TF2's front end under the name every game's has it (`party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
