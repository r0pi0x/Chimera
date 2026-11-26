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
        app.init_resource::<WeaponNames>().add_systems(Update, read_names);
    }
}

/// The names from the install's strings, read behind the frames
/// (`tasks::Behind`: the mount is opened for them, up to 2.7 G
/// instructions of the first frame), once.
fn read_names(
    games: Option<Res<Games>>,
    mut names: ResMut<WeaponNames>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
    mut reading: Local<Option<chimera_core::tasks::Behind<Vec<(String, String)>>>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let reading = reading.get_or_insert_with(|| {
        let game = games.and_then(|g| g.get(crate::game::GAME));
        chimera_core::tasks::Behind::start(move || {
            game.and_then(|g| g.mount()).map(|mount| crate::game::weapons::display_names(&mount)).unwrap_or_default()
        })
    });
    if let Some(read) = reading.take(wait.is_some()) {
        *done = true;
        if !read.is_empty() {
            names.0.extend(read);
        }
    }
}
