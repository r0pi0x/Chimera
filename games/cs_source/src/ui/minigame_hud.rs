//! The minigame's panel on CS:S's HUD (the shell's,
//! `client::minigame_panel`), its guns named in CS:S's own strings (the
//! install's `resource/cstrike_english.txt`, `weapons::display_names`).

use bevy::prelude::*;
pub use chimera_client::client::minigame_panel::{MinigamePanel, PanelLine, WeaponNames, panel_lines, panel_lines_with, plain_name};

use crate::game_registry::Games;

/// The minigame's panel on CS:S's HUD.
pub struct MinigameHudPlugin;

impl Plugin for MinigameHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_client::client::minigame_panel::MinigamePanelPlugin(crate::game::GAME))
            .add_systems(Startup, read_names);
    }
}

fn read_names(games: Option<Res<Games>>, mut names: ResMut<WeaponNames>) {
    let Some(mount) = games.and_then(|g| g.get(crate::game::GAME)).and_then(|g| g.mount()) else { return };
    names.0.extend(crate::game::weapons::display_names(&mount));
}
