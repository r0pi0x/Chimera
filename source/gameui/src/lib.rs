//! Source's GameUI on Chimera's client shell (lift S7 of
//! docs/plans/active/hl2dm.md): the one menu every Source game shows,
//! drawn from the install's own files (`source_engine::hud::gameui`). The
//! main menu and the Esc menu with their dialogs (`game_menu`), the
//! options' settings (`options`), the server browser (`server_browser`),
//! the dialog that finds the install (`first_run`).
//!
//! What differs per game is data: `MenuGame`, given to `GameMenuPlugin` and
//! `FirstRunPlugin`. CS:S's: `cs_source::ui::MENU_GAME`, with its bots, teams and
//! rounds; HL2:DM's: `hl2dm::ui::MENU_GAME`, with none of those. A setting
//! a game lacks shows greyed, since its cvar isn't registered.
//!
//! Layering: above the client shell and `source_engine`, below the games
//! (a game's `ui` feature turns it on; a dedicated server never builds it).

use bevy::prelude::*;
use chimera_client::client;
use chimera_core::{console, core, game_registry, game_registry::GameInfo, mount};
use chimera_net::net;
use chimera_present::{assets, audio};
use chimera_world::map;
#[cfg(test)]
use chimera_sim::weapon;

pub mod first_run;
pub mod game_menu;
pub mod options;
pub mod server_browser;

/// A game's bots in the menus (Create Server's bot rows and Bot page, the
/// `menu bots` dialog). A game without them (`MenuGame::bots` None) shows
/// none of these and its new games run no bot lines.
#[derive(Debug, PartialEq)]
pub struct BotMenu {
    /// Create Server's difficulty buttons: fallback word, then the values
    /// of `bot_reaction`, `bot_aim_error`, `bot_turn_rate`.
    pub difficulties: &'static [(&'static str, f32, f32, f32)],
    /// Which team bots join (the Bot page): token and our word.
    pub teams: [(&'static str, &'static str); 3],
    /// The Bot page's cvars (fieldName, cvar, token).
    pub cvars: [(&'static str, &'static str, &'static str); 2],
    /// "Include bots" and "how many": tokens.
    pub include_token: &'static str,
    pub count_token: &'static str,
    /// The difficulty buttons' tokens, this plus the index.
    pub difficulty_token: &'static str,
    pub join_team_token: &'static str,
    /// The `menu bots` dialog's words for each team's bot.
    pub team_words: [&'static str; 2],
}

/// A game's quick start (`chimera_quickstart`): a map, how many bots.
#[derive(Debug, PartialEq)]
pub struct QuickStart {
    /// Played if the install has it, else the first map.
    pub map: &'static str,
    pub bots: u8,
}

/// What's a game's own in the shared menus.
pub struct MenuGame {
    /// Its id, install and search path.
    pub info: &'static GameInfo,
    /// Its full name in the first-run dialog.
    pub name: &'static str,
    /// Its folder in a Steam library (the first-run dialog's example, and
    /// where a pasted library folder leads).
    pub steam_folder: &'static str,
    /// Its strings file over GameUI's and Valve's.
    pub strings: &'static str,
    pub bots: Option<BotMenu>,
    /// Ours on Create Server's Game page: rounds or deathmatch.
    pub rounds_cvar: Option<&'static str>,
    /// `menu team`: the two teams' names, for a game whose players pick
    /// from a dialog.
    pub teams: Option<[&'static str; 2]>,
    pub quick_start: Option<QuickStart>,
    /// A setting's control the install's layout marks hidden stays hidden.
    /// False: it shows (CS:S's code shows the controls its layouts hide;
    /// HL2:DM's layout hides its crosshair page for good).
    pub layout_hides: bool,
}

/// The menu's game (none in a model under test, or before the plugin set
/// it); compared by identity.
#[derive(Clone, Copy, Default)]
pub struct GameRef(pub Option<&'static MenuGame>);

impl PartialEq for GameRef {
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (Some(a), Some(b)) => std::ptr::eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl std::fmt::Debug for GameRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.map_or("no game", |g| g.info.id))
    }
}

impl GameRef {
    pub fn bots(self) -> Option<&'static BotMenu> {
        self.0.and_then(|g| g.bots.as_ref())
    }

    pub fn rounds_cvar(self) -> Option<&'static str> {
        self.0.and_then(|g| g.rounds_cvar)
    }

    pub fn teams(self) -> Option<[&'static str; 2]> {
        self.0.and_then(|g| g.teams)
    }

    pub fn quick_start(self) -> Option<&'static QuickStart> {
        self.0.and_then(|g| g.quick_start.as_ref())
    }

    pub fn layout_hides(self) -> bool {
        self.0.is_some_and(|g| g.layout_hides)
    }

    /// Its full name.
    pub fn name(self) -> &'static str {
        self.0.map_or("The game", |g| g.name)
    }
}

/// The game menu opened (`game_menu::open_menu`): a game's panels with
/// popups of their own close them.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameMenuOpened;

/// Everything of GameUI for a windowed game: the menu, the server browser
/// and the first-run dialog.
pub struct GameUiPlugin(pub &'static MenuGame);

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            game_menu::GameMenuPlugin(self.0),
            server_browser::ServerBrowserPlugin,
            first_run::FirstRunPlugin(self.0),
        ));
    }
}
