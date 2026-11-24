//! Counter-Strike 1.6's parts, one module per subsystem, and what the game tells
//! the shell about itself (`INFO`). Made by tools/new-game; the plan is
//! docs/plans/active/cs16.md, the behaviour specs/cs16/.
//!
//! - `mount`: the install as a mount (`chimera_core::mount::served`, so
//!   the asset store works too);
//! - `maps`: `MAP_LOADER` for `cs16:<map>`, GoldSrc's BSP conversion
//!   (`goldsrc_engine::content`) with CS 1.6's spawns and kit;
//! - `bodies`: the eight player classes' bodies and what they play;
//! - `movement`: `cs16:movement`, Chimera's placeholder movement with
//!   the numbers of data/movement.toml;
//! - `weapons`: the stand-in gun as data (data/weapons/);
//! - `parts`: what the game lends other games' apps (`PARTS`).
//!
//! `Cs16Plugins` adds the game's parts after `crate::SimPlugins`;
//! `BUNDLE` is the game for a party app.
//!
//! Known shortcuts:
//! - Movement and the gun are tools/new-game's stand-ins: the placeholder
//!   movement, one gun without models. Replace each part from its spec
//!   (docs/CONTRIBUTING-GAMES.md).

pub mod bodies;
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

/// The game's folder inside its install (the install root holds
/// `cstrike/` and Half-Life's `valve/`, the fallback;
/// specs/goldsrc/mount.md).
pub const CONTENT_DIR: &str = "cstrike";

/// The server tick, seconds: 100 Hz, GoldSrc's common case of 100
/// commands a second of 10 ms each (docs/plans/active/goldsrc.md §4.3; to
/// confirm against the server's `sys_ticrate` in specs/cs16/movement.md).
pub const TICK_INTERVAL: f64 = 0.01;

/// GoldSrc's unit, the inch, in metres (specs/goldsrc/bsp.md, "Units and
/// conventions").
pub const METERS_PER_UNIT: f32 = 0.0254;

/// Counter-Strike's Steam app (the client; the dedicated server, app 90
/// with `mod cstrike`, has the same layout).
pub const STEAM_APP: u32 = 10;

/// What the client shell knows of the game (`game_registry`): its install
/// (Steam app 10, installed as `Half-Life`, or `[games.cs16] path` in
/// chimera.local.toml, or `$CHIMERA_CS16_ROOT`: the folder holding
/// `cstrike/` and `valve/`, the client's or the dedicated server's),
/// folder, maps, tick and console namespaces (`cs16`, with the game
/// folder's name `cstrike` as an alias).
pub static INFO: crate::game_registry::GameInfo = crate::game_registry::GameInfo {
    id: GAME,
    short_name: "CS 1.6",
    name: "Counter-Strike 1.6",
    content_dir: CONTENT_DIR,
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    map_cycle: maps::map_names,
    map_names: maps::map_names,
    steam: Some(crate::mount::install::SteamGame {
        app_id: STEAM_APP,
        name: "Half-Life",
        required: &["cstrike/liblist.gam", "cstrike/maps/de_dust2.bsp", "valve/halflife.wad"],
    }),
    map_file: |name| Some(maps_path(name)),
    import_map: |_| Err("Counter-Strike 1.6 maps load from the install by id (cs16:<map>)".into()),
    // No such file in an install: the built-in table is used.
    default_binds_cfg: "chimera_placeholder_binds.cfg",
    default_binds: DEFAULT_BINDS,
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: METERS_PER_UNIT,
    console_namespaces: &["cs16", "cstrike"],
};

/// A map's file in the install (`maps/<name>.bsp`).
fn maps_path(name: &str) -> String {
    goldsrc_engine::content::maps::map_path(name)
}

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
            .add(bodies::Cs16BodiesPlugin)
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

/// The game's parts on the maps of other games: its player bodies
/// (`bodies::dress_guest_in`; its gun has no models yet), converted once
/// per app and load kind, then copied into each map
/// (`chimera_world::map::kept`).
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing {
    game: GAME,
    dress: |map, options| {
        crate::map::kept::dress_cached(
            map,
            options,
            &crate::map::kept::Dressing {
                game: GAME,
                converter: maps::CONVERTER,
                mount: &bodies::guest_mount,
                convert: &|mount, m| bodies::dress_guest_in(mount, m, !options.textures),
                variant: "",
                memos: None,
            },
        )
    },
};

/// What the game adds to a game of several games: the game registered,
/// its movement (not the match's), its gun, its bodies' animation and its
/// maps' pool rules and measurements.
pub fn party_sim(app: &mut bevy::app::App) {
    use crate::game_registry::GameAppExt;
    app.register_game(&INFO);
    movement::add_movement(app);
    if !app.is_plugin_added::<weapons::Cs16GunsPlugin>() {
        app.add_plugins((weapons::Cs16GunsPlugin, bodies::Cs16BodiesPlugin));
    }
    parts::add_map_pool(app);
}
