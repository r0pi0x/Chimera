//! Counter-Strike 1.6's parts, one module per subsystem, and what the game tells
//! the shell about itself (`INFO`). Made by tools/new-game; the plan is
//! docs/plans/active/cs16.md, the behaviour specs/cs16/.
//!
//! - `mount`: the install as a mount (`chimera_core::mount::served`, so
//!   the asset store works too);
//! - `maps`: `MAP_LOADER` for `cs16:<map>` (none yet: the greybox plays);
//! - `movement`: `cs16:movement`, Chimera's placeholder movement with
//!   the numbers of data/movement.toml;
//! - `weapons`: the stand-in gun as data (data/weapons/);
//! - `parts`: what the game lends other games' apps (`PARTS`).
//!
//! `Cs16Plugins` adds the game's parts after `crate::SimPlugins`;
//! `BUNDLE` is the game for a party app.
//!
//! Known shortcuts:
//! - Everything is a stand-in made by tools/new-game: no maps, the
//!   placeholder movement, one gun without models. Replace each part from
//!   its spec (docs/CONTRIBUTING-GAMES.md).

pub mod maps;
pub mod mount;
pub mod movement;
pub mod parts;
pub mod weapons;

pub use maps::MAP_LOADER;

use bevy::app::{PluginGroup, PluginGroupBuilder};

/// The game's id (`cs16`): its maps' ids (`cs16:<map>`), its key in
/// chimera.local.toml (`[games.cs16] path`), `CHIMERA_CS16_ROOT`
/// and its content cache.
pub const GAME: &str = "cs16";

/// The game's folder inside its install ("" when the install is the
/// game's folder itself).
pub const CONTENT_DIR: &str = "";

/// The server tick, seconds (a stand-in: put the spec's here).
pub const TICK_INTERVAL: f64 = 1.0 / 60.0;

/// The game's world unit in metres (a stand-in: the spec's unit).
pub const METERS_PER_UNIT: f32 = 1.0;

/// What the client shell knows of the game (`game_registry`): its install
/// (`[games.cs16] path` in chimera.local.toml or `$CHIMERA_CS16_ROOT`;
/// add `steam` when it is a Steam game), folder, maps, tick and console
/// namespace (its id).
pub static INFO: crate::game_registry::GameInfo = crate::game_registry::GameInfo {
    id: GAME,
    short_name: "CS 1.6",
    name: "Counter-Strike 1.6",
    content_dir: CONTENT_DIR,
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    map_cycle: maps::map_names,
    map_names: maps::map_names,
    steam: None,
    map_file: |_| None,
    import_map: |_| Err("Counter-Strike 1.6 has no map files yet".into()),
    // No such file in an install: the built-in table is used.
    default_binds_cfg: "chimera_placeholder_binds.cfg",
    default_binds: DEFAULT_BINDS,
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: METERS_PER_UNIT,
    console_namespaces: &[],
};

/// The default binds: the shell's usual ones until a spec lists the game's.
pub const DEFAULT_BINDS: &[(&str, &str)] = &[
    ("w", "+forward"),
    ("s", "+back"),
    ("a", "+moveleft"),
    ("d", "+moveright"),
    ("space", "+jump"),
    ("ctrl", "+duck"),
    ("mouse1", "+attack"),
    ("mouse2", "+attack2"),
    ("r", "+reload"),
    ("e", "+use"),
    ("tab", "+showscores"),
];

/// The game's parts, after the engine's base (`crate::SimPlugins`): the
/// game registered (`INFO`) first, then its movement and its gun.
pub struct Cs16Plugins;

impl PluginGroup for Cs16Plugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(crate::game_registry::RegisterGame(&INFO))
            .add(movement::Cs16MovementPlugin)
            .add(weapons::Cs16GunsPlugin)
    }
}

/// The game as one bundle for an app made of several games
/// (`chimera_world::game_bundle`): its info, parts and maps, its guns'
/// names, and its parts in a composed simulation (`party_sim`).
pub static BUNDLE: chimera_world::game_bundle::GameBundle = chimera_world::game_bundle::GameBundle {
    info: &INFO,
    parts: &parts::PARTS,
    loader: &MAP_LOADER,
    guest: Some(&GUEST),
    // What of its parts stays on its own maps: its guns (their ids' prefix).
    kept: &["cs16:weapon_"],
    binds: &[],
    view_model_fov: None,
    view_model_fov_cvar: None,
    base: None,
    sim: party_sim,
    display_names: Some(weapons::display_names),
    player_options: Some(chimera_core::game_data!("data/player_options.toml")),
};

/// The game's parts on the maps of other games: nothing yet (its guns
/// have no models). Its players' bodies and guns' models go here through
/// `map::kept::dress_cached` (docs/new-game.md, step 3).
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing { game: GAME, dress: |_, _| Ok(()) };

/// What the game adds to a game of several games: the game registered,
/// its movement (not the match's) and its gun.
pub fn party_sim(app: &mut bevy::app::App) {
    use crate::game_registry::GameAppExt;
    app.register_game(&INFO);
    movement::add_movement(app);
    if !app.is_plugin_added::<weapons::Cs16GunsPlugin>() {
        app.add_plugins(weapons::Cs16GunsPlugin);
    }
}
