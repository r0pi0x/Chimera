//! A game as one bundle for an app made of several games (lift C4,
//! docs/plans/active/hl2dm.md; ultimate-gun-game.md gap 2): what the
//! party apps' composer (`party_kit`) needs of each game it links, behind
//! one interface, so it loops over its bundles instead of naming each
//! game: the game's `GameInfo` (its install, maps, tick), what it lends
//! (`GameParts`), its maps (`MapLoader`) and its players, guns and HUD
//! look on other games' maps (`MapDressing`), its own binds over the
//! shared ones, its view models' field of view (and its cvar), what it
//! brings as a composition's base game (`BaseGame`), what it adds to the
//! composed simulation (`sim`), and its weapons' names from its install.
//! Its front end is apart (`GameClient`, filled by the game's `ui`, which
//! its parts never name).
//!
//! A game fills its own `GameBundle` (`cs_source::game::BUNDLE`,
//! `hl2dm::game::BUNDLE`, `combat_arms::game::BUNDLE`); the composer
//! orders them (the first is the base game, whose gameplay the others
//! join).

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_core::{game_parts::GameParts, game_registry::GameInfo};

use crate::map::{GameFov, MapDressing, MapLoader};

/// One game as a party app composes it (module doc).
pub struct GameBundle {
    pub info: &'static GameInfo,
    pub parts: &'static GameParts,
    /// Its maps.
    pub loader: &'static MapLoader,
    /// Its players, guns and HUD look on maps of other games (None: it
    /// brings none).
    pub guest: Option<&'static MapDressing>,
    /// Key prefixes of its held and view models that stay on its own maps
    /// when another game's gameplay dresses them (`MapSources::kept`).
    pub kept: &'static [&'static str],
    /// Its own binds over the shared ones (`Console::ship_layer`).
    pub binds: &'static [(&'static str, &'static str)],
    /// Its view models' field of view (None: the app's default).
    pub view_model_fov: Option<fn() -> GameFov>,
    /// Its own view-model field of view cvar (`viewmodel_fov`, saved in
    /// the config) in a composition (None: its own plugins add it, or it
    /// has none).
    pub view_model_fov_cvar: Option<fn(&mut App)>,
    /// What it brings as a composition's base game, whose gameplay, bots
    /// and HUD the others' parts play on (None: it can't be the base).
    pub base: Option<&'static BaseGame>,
    /// Its parts in the composed simulation (after the base game's), the
    /// dedicated server's too.
    pub sim: fn(&mut App),
    /// Its weapons' display names by weapon id from its install's strings
    /// (a HUD of another game names its guns so).
    pub display_names: Option<fn(&chimera_core::mount::Mount) -> Vec<(String, String)>>,
    /// Its own keys and options on a party app's Options screen (its
    /// `data/player_options.toml`: party_kit's `game_options`).
    pub player_options: Option<chimera_core::game_data::GameData>,
}

/// A game as the base of a composition (`GameBundle::base`): its
/// simulation, and its rules and bots set for the other games' parts.
pub struct BaseGame {
    /// Its simulation: the engine's base, then its parts.
    pub plugins: fn() -> PluginGroupBuilder,
    /// After the composition's minigame runtime: its own modes and bots
    /// set for a party of games (its modes off, its bots given the other
    /// games' guns).
    pub compose: fn(&mut App),
    /// At `Plugin::finish`, after the composition's (every game's guns
    /// are in the catalogue).
    pub finish: fn(&mut App),
}

/// A game's front end in an app made of several games: its own crate's
/// `ui` fills it (a game's parts never name their front end), the
/// composer's client lists them beside the bundles.
pub struct GameClient {
    pub game: &'static str,
    /// Its HUD on the client shell's plugins, after the base game's.
    pub hud: Option<fn(PluginGroupBuilder) -> PluginGroupBuilder>,
    /// The rest of its front end on the client (what its bodies play, the
    /// HUD look's owner).
    pub app: Option<fn(&mut App)>,
    /// Its front end as a composition's base game (None: it can't be the
    /// base, or its base front end is its `app`).
    pub base: Option<&'static BaseClient>,
}

/// A game's front end as a composition's base game (`GameClient::base`).
pub struct BaseClient {
    /// Its answers to the composition's main menu: its music
    /// (`MenuMusic`), its loading screen (`SetBaseLoading`).
    pub menu: fn(&mut App),
    /// The rest of its front end as the base (its other looks, the mouse
    /// look's starting values), after the guests' own.
    pub app: fn(&mut App),
    /// The map its own loading screen is up for (None: down), if it has
    /// one: shown only for its own maps (the composition's main menu
    /// raises and lowers it with `SetBaseLoading`).
    pub loading: Option<fn(&World) -> Option<String>>,
}

/// What a composition's main menu wants of the games' music, triggered
/// on every change: `app_plays`, the app's own music plays (none of a
/// game's); `menu_shown`, the main menu is up (the base game's menu
/// playlist may play). The base game's front end observes it
/// (`BaseClient::menu`).
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuMusic {
    pub app_plays: bool,
    pub menu_shown: bool,
}

/// A composition's main menu raises the base game's loading screen for a
/// map (`Some`) or lowers it (`None`); the base game's front end observes
/// it (`BaseClient::menu`).
#[derive(Event, Clone, Debug)]
pub struct SetBaseLoading(pub Option<String>);

impl std::fmt::Debug for GameBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameBundle").field("game", &self.info.id).finish_non_exhaustive()
    }
}

impl GameBundle {
    /// The game's id.
    pub fn game(&self) -> &'static str {
        self.info.id
    }
}
