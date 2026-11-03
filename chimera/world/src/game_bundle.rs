//! A game as one bundle for an app made of several games (lift C4,
//! docs/plans/active/hl2dm.md; ultimate-gun-game.md gap 2): what the
//! party apps' composer (`party_kit`) needs of each game it links, behind
//! one interface, so it loops over its bundles instead of naming each
//! game: the game's `GameInfo` (its install, maps, tick), what it lends
//! (`GameParts`), its maps (`MapLoader`) and its players, guns and HUD
//! look on other games' maps (`MapDressing`), its own binds over the
//! shared ones, its view models' field of view, what it adds to the
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
    /// Its parts in the composed simulation (after the base game's), the
    /// dedicated server's too.
    pub sim: fn(&mut App),
    /// Its weapons' display names by weapon id from its install's strings
    /// (a HUD of another game names its guns so).
    pub display_names: Option<fn(&chimera_core::mount::Mount) -> Vec<(String, String)>>,
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
}

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
