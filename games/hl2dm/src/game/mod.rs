//! Half-Life 2: Deathmatch's parts. M0: what the game tells the shell
//! about itself (`INFO`: its id, folder, Steam app, tick, map names and
//! cycle, console namespaces) and its install as a mount (`mount`).
//! Movement, weapons, rules and the rest come with their specs
//! (specs/hl2dm/; the plan's milestones M1 to M11).
//!
//! Known shortcuts:
//! - `INFO`'s default binds and movement cvars are empty: the shell reads
//!   the install's `cfg/config_default.cfg`, and HL2:DM's movement comes
//!   with M3 (specs/hl2dm/movement.md).
//! - `INFO.tick_interval` is the default tick H0 recorded (0.015 s,
//!   specs/hl2dm/source_deltas.md, "Tick and network"); the server's
//!   `-tickrate` option isn't modelled.

pub mod mount;

/// The game's id (`hl2dm`): its maps' ids (`hl2dm:dm_lockdown`), its key
/// in chimera.local.toml (`[games.hl2dm] path`) and its content cache.
pub const GAME: &str = "hl2dm";

/// The game's folder inside its install, holding `gameinfo.txt`.
pub const CONTENT_DIR: &str = "hl2mp";

/// HL2:DM's server tick: 0.015 s (66.67 Hz), the game's default
/// (specs/hl2dm/source_deltas.md, "Tick and network").
pub const TICK_INTERVAL: f64 = 0.015;

/// What the client shell knows of HL2:DM (`game_registry`): its install
/// (Steam app 320, or `[games.hl2dm] path`, or `$CHIMERA_HL2DM_ROOT`),
/// folder, map names and cycle, tick and console namespaces (`hl2dm`,
/// with the game folder's name `hl2mp` as an alias).
pub static INFO: crate::game_registry::GameInfo = crate::game_registry::GameInfo {
    id: GAME,
    short_name: "HL2:DM",
    content_dir: CONTENT_DIR,
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    map_cycle: source_engine::maps::map_cycle,
    map_names: source_engine::maps::map_names,
    steam: Some(crate::mount::install::SteamGame {
        app_id: 320,
        name: "Half-Life 2: Deathmatch",
        required: &["hl2mp/hl2mp_pak_dir.vpk", "hl2/hl2_misc_dir.vpk"],
    }),
    map_file: |name| Some(format!("maps/{name}.bsp")),
    import_map: mount::import_map,
    default_binds_cfg: "cfg/config_default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: 0.0254,
    console_namespaces: &["hl2dm", "hl2mp"],
};

/// The stock maps (specs/hl2dm/source_deltas.md, "Stock maps"): the
/// seven of the map cycle and halls3, which isn't in it.
pub const STOCK_MAPS: &[&str] = &[
    "dm_lockdown",
    "dm_overwatch",
    "dm_powerhouse",
    "dm_resistance",
    "dm_runoff",
    "dm_steamlab",
    "dm_underpass",
    "halls3",
];
