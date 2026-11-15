//! Halo's parts, one module per subsystem, and what the game tells the
//! shell about itself (`INFO`). M1 of docs/plans/active/halo-ce.md: the
//! game registers, its install mounts and its maps load; M2 begins with
//! its movement; weapons and the rest come with their specs
//! (specs/halo_ce/<name>.md):
//!
//! - `mount`: the install, a disc image or a folder of the disc's files, as
//!   a mount (`INFO.open`; specs/halo_ce/disc_image.md);
//! - `maps`: the install's map cache files as Chimera's `MapData`
//!   (`MAP_LOADER`; cache_file.md, structure_bsp.md, collision.md,
//!   shaders.md, scenario.md);
//! - `movement`: `halo_ce:movement`, Halo's player movement
//!   (specs/halo_ce/movement.md; its numbers in data/movement.toml);
//! - `parts`: what the game lends other games' apps (`PARTS`): its maps,
//!   by its pool rules and measurements (data/map_pool.toml,
//!   data/map_fitness.toml), played with other games' players and guns.
//!
//! `HaloPlugins` adds the game's parts after `crate::SimPlugins`; none yet
//! but the game's registration.

pub mod maps;
pub mod mount;
pub mod movement;
pub mod parts;

pub use maps::MAP_LOADER;

use bevy::{app::PluginGroupBuilder, prelude::*};

use crate::game_registry::{GameInfo, RegisterGame};

/// The game's id: maps are `halo_ce:<name>`, its install's key in
/// chimera.local.toml (`[games.halo_ce] path`), `CHIMERA_HALO_CE_ROOT`.
pub const GAME: &str = "halo_ce";

/// The game's tick: 30 per second (specs/halo_ce/cache_file.md, "The
/// world").
pub const TICK_INTERVAL: f64 = 1.0 / 30.0;

/// The world unit in metres: 10 feet (cache_file.md, "The world").
pub const METERS_PER_UNIT: f32 = 3.048;

/// The disc's 13 multiplayer maps, by file name (`maps/<name>.map`;
/// cache_file.md, "What is on the disc": type 1), in the spec's order. The
/// ten campaign maps (`a10` ... `d40`) and the menu's (`ui`) are not
/// listed: they are not played in a match.
pub const MULTIPLAYER_MAPS: &[&str] = &[
    "beavercreek",
    "bloodgulch",
    "boardingaction",
    "carousel",
    "chillout",
    "damnation",
    "hangemhigh",
    "longest",
    "prisoner",
    "putput",
    "ratrace",
    "sidewinder",
    "wizard",
];

/// What the shell needs of Halo (`game_registry`). `open` mounts the
/// install (`mount`: a disc image file or a folder; no Steam app, so it is
/// found by `$CHIMERA_HALO_CE_ROOT`, `[games.halo_ce] path` or the saved
/// path only). No map import (maps load from the install by id), the
/// binds are the built-in table below.
pub static INFO: GameInfo = GameInfo {
    id: GAME,
    short_name: "Halo",
    // The install is a disc image or the disc's files: no game folder
    // inside it (so the shell's `download/` and `cfg/` lookups find
    // nothing and the built-in binds are used).
    content_dir: "",
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    // PLACEHOLDER: the multiplayer maps in the spec's order, those the
    // mount holds; the real rotation is the game type rules' (a later spec).
    map_cycle: maps::held,
    map_names: maps::map_names,
    steam: None,
    // A map is a file of the install's disc, not one clients download.
    map_file: |_| None,
    import_map: |_| Err("Halo maps load from the install by id (halo_ce:<name>), not from a file".into()),
    // No such file in an install: the built-in table is used.
    default_binds_cfg: "chimera_placeholder_binds.cfg",
    default_binds: DEFAULT_BINDS,
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: METERS_PER_UNIT,
    // `halo` as the short alias; other games have two namespaces too.
    console_namespaces: &["halo_ce", "halo"],
};

/// Halo's default binds. PLACEHOLDER: the Xbox game has a controller's
/// layout and no spec lists keyboard keys yet, so these are the shell's
/// usual ones on the commands the shell has.
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

/// Halo's parts, after `crate::SimPlugins`: the game registered (`INFO`) and
/// its movement (`movement`).
pub struct HaloPlugins;

impl PluginGroup for HaloPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(RegisterGame(&INFO)).add(movement::HaloMovementPlugin)
    }
}

/// Halo as one bundle for an app made of several games
/// (`chimera_world::game_bundle`): its info, parts and maps; it brings no
/// players or guns to other games' maps yet (`guest: None`) and adds
/// its registration, its surface names' classes (`maps::surfaces::
/// SURFACES`) and its map pool's rules and measurements
/// (`parts::add_map_pool`) to a composed simulation: its maps are played
/// there with the other games' movement and guns.
pub static BUNDLE: chimera_world::game_bundle::GameBundle = chimera_world::game_bundle::GameBundle {
    info: &INFO,
    parts: &parts::PARTS,
    loader: &MAP_LOADER,
    guest: None,
    kept: &[],
    binds: &[],
    view_model_fov: None,
    sim: |app| {
        use crate::game_registry::GameAppExt;
        app.register_game(&INFO);
        // Its maps' surface names as classes, for the guns of the games
        // played on them.
        crate::map::impact::SurfaceClassesAppExt::register_surface_classes(app, maps::surfaces::SURFACES);
        parts::add_map_pool(app);
    },
    display_names: None,
};
