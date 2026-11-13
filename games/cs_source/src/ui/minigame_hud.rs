//! CS:S's guns' names for the minigame's panel (the shell's overlay,
//! `client::minigame_panel`, drawn the same over every game's HUD): read
//! from CS:S's own strings (the install's `resource/cstrike_english.txt`,
//! `weapons::display_names`).

use bevy::prelude::*;
pub use chimera_client::client::minigame_panel::{MinigamePanel, PanelLine, WeaponNames, panel_lines, panel_lines_with, plain_name};

use crate::game_registry::Games;

/// CS:S's guns' names in the minigame's panel.
pub struct MinigameHudPlugin;

impl Plugin for MinigameHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponNames>().add_systems(Startup, read_names);
    }
}

fn read_names(games: Option<Res<Games>>, mut names: ResMut<WeaponNames>) {
    let Some(mount) = games.and_then(|g| g.get(crate::game::GAME)).and_then(|g| g.mount()) else { return };
    names.0.extend(crate::game::weapons::display_names(&mount));
}
