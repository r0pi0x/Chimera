//! The games an app plays, as data the game fills in (`GameInfo`), so the
//! client shell names no game: where its install is and how to mount it,
//! its folder there, its map cycle, its server tick, its default binds
//! and the movement cvars the debug UI lists.
//!
//! A game registers its `GameInfo` (a `static`) with
//! `app.register_game(&INFO)` (or the `RegisterGame` plugin, which a game's
//! plugin group adds); `Games` keeps them in that order and the first is
//! the default game: the one a bare map name (`de_dust2`) belongs to, whose
//! install's maps `map` lists. An app without a game registered has no
//! default game: maps then need their game's id (`cs_source:de_dust2`) and
//! the shell's install lookups find nothing.
//!
//! What a game can't say as data is a plain `fn` (opening its install,
//! reading its map cycle, importing a map file): no trait objects, so a
//! `GameInfo` is a `static` and `Games` is cheap to clone.

use std::{
    io,
    path::{Path, PathBuf},
};

use bevy::prelude::*;

use crate::mount::{Mount, config, install::SteamGame};

/// One game, as the shell needs to know it.
#[derive(Debug)]
pub struct GameInfo {
    /// The id maps are named under (`cs_source` in `cs_source:de_dust2`);
    /// also the install's key in chimera.local.toml and the content cache's
    /// folder (`mount::config::content_dir`).
    pub id: &'static str,
    /// A short name for menus and headings (`CS:S`).
    pub short_name: &'static str,
    /// The game's own folder inside its install (`cstrike`): its
    /// `download/` folder, its `cfg/`.
    pub content_dir: &'static str,
    /// The server tick its maps run at, in seconds.
    pub tick_interval: f64,
    /// Mount the install at a path (the game's search path).
    pub open: fn(&Path) -> io::Result<Mount>,
    /// The map cycle a server plays in turn, from the mounted install.
    pub map_cycle: fn(&Mount) -> Vec<String>,
    /// The maps the mounted install holds, by name (sorted): each game's
    /// own layout (Source's `maps/<name>.bsp`, Combat Arms'
    /// `worlds*/<name>.dat`).
    pub map_names: fn(&Mount) -> Vec<String>,
    /// The Steam app the install is found by (`mount::install`): None for
    /// a game found by its configured path only.
    pub steam: Option<SteamGame>,
    /// A map's downloadable file, relative to a content folder (the
    /// install's download folder, Chimera's cache): `maps/<name>.bsp`.
    /// The network's map offer (`net::maps`) names, caches and fetches by
    /// it. None: the map has no such file (Combat Arms' lives in the
    /// install's archives), so it is offered by id and hash only.
    pub map_file: fn(&str) -> Option<String>,
    /// Copy a map file into Chimera's content cache; returns its name.
    pub import_map: fn(&Path) -> Result<String, String>,
    /// The default binds' config file, relative to `content_dir`
    /// (`cfg/config_default.cfg`); its `bind` lines are the defaults.
    pub default_binds_cfg: &'static str,
    /// The default binds when that file can't be read: (key, command), in
    /// the file's order.
    pub default_binds: &'static [(&'static str, &'static str)],
    /// The default binds a saved config written before the game's current
    /// ones had (one without the shell's `binds::DEFAULTS_MARK`): the
    /// defaults not among these are added to it once, on free keys. Empty
    /// for a game whose defaults never changed.
    pub old_default_binds: &'static [(&'static str, &'static str)],
    /// The movement cvars the debug UI's movement tab lists: (name, what
    /// it sets).
    pub movement_cvars: &'static [(&'static str, &'static str)],
    /// The game's world unit in metres (Source's inch, 0.0254; Combat
    /// Arms' 0.018): what its console variables' distances are in when
    /// it is the gameplay (`cam_idealdist`).
    pub meters_per_unit: f32,
}

impl GameInfo {
    /// Where the game is installed (chimera.local.toml, saved, or found:
    /// `mount::install`).
    pub fn install(&self) -> Option<PathBuf> {
        config::LocalConfig::load().ok()?.game_path_in(self.id, self.steam.as_ref())
    }

    /// The install mounted (`open`), if there is one and it opens.
    pub fn mount(&self) -> Option<Mount> {
        (self.open)(&self.install()?).ok()
    }

    /// Whether the mount is the asset store alone (`mount::served::alone`:
    /// the install off or missing): only the sets packed so far, so a
    /// check of the whole install has nothing to check.
    pub fn served_alone(&self) -> bool {
        self.install().is_some_and(|i| crate::mount::served::alone(self.id, &i))
    }

    /// Whether the mount is the asset store alone with only some sets
    /// (`mount::served::partial`): a check of the whole install skips. A
    /// whole publish isn't partial: files arrive on demand.
    pub fn served_partial(&self) -> bool {
        self.install().is_some_and(|i| crate::mount::served::partial(self.id, &i))
    }

    /// Chimera's content cache for the game (imported maps).
    pub fn content_cache(&self) -> Option<PathBuf> {
        config::content_dir(self.id)
    }

    /// The game's download folder in its install (maps fetched from
    /// servers).
    pub fn download_dir(&self) -> Option<PathBuf> {
        Some(self.install()?.join(self.content_dir).join("download"))
    }

    /// The text of the install's default binds file, if it reads.
    pub fn default_binds_text(&self) -> Option<String> {
        let path = self.install()?.join(self.content_dir).join(self.default_binds_cfg);
        std::fs::read_to_string(path).ok()
    }

    /// Map `name`'s id in this game (`cs_source:de_dust2`).
    pub fn map_id(&self, name: &str) -> String {
        format!("{}:{name}", self.id)
    }

    /// The tick as a duration.
    pub fn tick(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(self.tick_interval)
    }
}

/// The games registered, in order; the first is the default game.
#[derive(Resource, Clone, Default, Debug)]
pub struct Games(Vec<&'static GameInfo>);

impl Games {
    /// The default game (the first registered), if any.
    pub fn default_game(&self) -> Option<&'static GameInfo> {
        self.0.first().copied()
    }

    /// The game with id `id` (case-insensitive), if registered.
    pub fn get(&self, id: &str) -> Option<&'static GameInfo> {
        self.0.iter().copied().find(|g| g.id.eq_ignore_ascii_case(id))
    }

    /// Every game registered, the default first.
    pub fn all(&self) -> impl Iterator<Item = &'static GameInfo> + '_ {
        self.0.iter().copied()
    }

    /// Add `game` (once; a second registration of the same id is ignored).
    pub fn register(&mut self, game: &'static GameInfo) {
        if self.get(game.id).is_none() {
            self.0.push(game);
        }
    }
}

/// The default game of a world, if one is registered.
pub fn default_game(world: &World) -> Option<&'static GameInfo> {
    world.get_resource::<Games>()?.default_game()
}

/// Registering games on an app.
pub trait GameAppExt {
    /// Register `game` (`Games`); the first registered is the default.
    fn register_game(&mut self, game: &'static GameInfo) -> &mut Self;
}

impl GameAppExt for App {
    fn register_game(&mut self, game: &'static GameInfo) -> &mut Self {
        self.world_mut().get_resource_or_init::<Games>().register(game);
        self
    }
}

/// A plugin that registers one game (for a game's plugin group). Not
/// unique: an app may add one per game.
pub struct RegisterGame(pub &'static GameInfo);

impl Plugin for RegisterGame {
    fn build(&self, app: &mut App) {
        app.register_game(self.0);
    }

    fn is_unique(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_mount(_: &Path) -> io::Result<Mount> {
        Err(io::ErrorKind::NotFound.into())
    }

    const A_INFO: GameInfo = GameInfo {
        id: "a_game",
        short_name: "A",
        content_dir: "a",
        tick_interval: 0.015,
        open: no_mount,
        map_cycle: |_| Vec::new(),
        map_names: |_| Vec::new(),
        map_file: |n| Some(format!("maps/{n}.bsp")),
        steam: None,
        import_map: |_| Err("no".into()),
        default_binds_cfg: "cfg/default.cfg",
        default_binds: &[("w", "+forward")],
        old_default_binds: &[],
        movement_cvars: &[],
        meters_per_unit: 0.0254,
    };
    static A: GameInfo = A_INFO;
    static B: GameInfo = GameInfo { id: "b_game", ..A_INFO };

    #[test]
    fn the_first_game_registered_is_the_default() {
        let mut app = App::new();
        assert!(default_game(app.world()).is_none());
        app.add_plugins((RegisterGame(&B), RegisterGame(&A))).register_game(&B);
        let games = app.world().resource::<Games>();
        assert_eq!(games.default_game().map(|g| g.id), Some("b_game"));
        assert_eq!(games.get("A_GAME").map(|g| g.id), Some("a_game"));
        assert_eq!(games.all().count(), 2);
        assert_eq!(A.map_id("x"), "a_game:x");
    }
}
