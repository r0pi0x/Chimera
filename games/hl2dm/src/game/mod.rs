//! Half-Life 2: Deathmatch's parts: what the game tells the shell about
//! itself (`INFO`: its id, folder, Steam app, tick, map names and cycle,
//! console namespaces), its install as a mount (`mount`), its maps
//! (`maps`: Source's conversion and sounds with HL2:DM's data;
//! `MAP_LOADER`), its movement (`movement`: Source's with HL2:DM's config,
//! walk, run and sprint, ladders, falls; M3), the suit (`suit`: armour,
//! the health remainder, aux power, breath and drowning, flashlight,
//! zoom; M3), the map's pickups and wall chargers (`pickups`; M3), and
//! M1's test weapon (`weapons`); `Hl2dmPlugins` adds them. Weapons,
//! rules and the rest come with their specs (specs/hl2dm/; the plan's
//! milestones M4 to M11).
//!
//! Known shortcuts:
//! - `INFO`'s default binds are empty: the shell reads the install's
//!   `cfg/config_default.cfg`.
//! - `INFO.tick_interval` is the default tick H0 recorded (0.015 s,
//!   specs/hl2dm/source_deltas.md, "Tick and network"); the server's
//!   `-tickrate` option isn't modelled.

pub mod maps;
pub mod mount;
pub mod movement;
pub mod pickups;
pub mod suit;
pub mod weapons;

pub use maps::MAP_LOADER;

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
    movement_cvars: source_engine::source_movement::CVARS,
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

/// HL2:DM's parts, after the engine's base (`crate::SimPlugins`): the
/// game registered (`INFO`), its movement (the match's), the suit, the
/// map's pickups and chargers, and its weapons.
pub struct Hl2dmPlugins;

impl bevy::app::PluginGroup for Hl2dmPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        bevy::app::PluginGroupBuilder::start::<Self>()
            .add(crate::game_registry::RegisterGame(&INFO))
            .add(movement::Hl2MovementPlugin)
            .add(suit::SuitPlugin)
            .add(pickups::PickupsPlugin)
            .add(weapons::Hl2WeaponsPlugin)
    }
}
