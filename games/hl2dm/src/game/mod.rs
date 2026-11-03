//! Half-Life 2: Deathmatch's parts: what the game tells the shell about
//! itself (`INFO`: its id, folder, Steam app, tick, map names and cycle,
//! console namespaces), its install as a mount (`mount`), its maps
//! (`maps`: Source's conversion and sounds with HL2:DM's data;
//! `MAP_LOADER`), its movement (`movement`: Source's with HL2:DM's config,
//! walk, run and sprint, ladders, falls; M3), the suit (`suit`: armour,
//! the health remainder, aux power, breath and drowning, flashlight,
//! zoom; M3), the map's pickups and wall chargers (`pickups`; M3; ammo
//! boxes and respawns, M7), ammunition outside weapons (`ammo`), the ammo
//! crate (`ammo_crates`), its deathmatch and team deathmatch (`rules`,
//! M7), and
//! its bullet and melee weapons (`weapons`, M4: data and hooks; `scripts`,
//! the install's weapon scripts against the data); `Hl2dmPlugins` adds
//! them. The projectiles and the rest come with their specs
//! (specs/hl2dm/; the plan's milestones M5 to M11).
//!
//! Known shortcuts:
//! - `INFO`'s default binds are empty: the shell reads the install's
//!   `cfg/config_default.cfg`.
//! - `INFO.tick_interval` is the default tick H0 recorded (0.015 s,
//!   specs/hl2dm/source_deltas.md, "Tick and network"); the server's
//!   `-tickrate` option isn't modelled.

pub mod ammo;
pub mod ammo_crates;
pub mod bodies;
pub mod look;
pub mod maps;
pub mod mount;
pub mod movement;
pub mod parts;
pub mod pickups;
pub mod rules;
pub mod player_anim;
pub mod rungs;
pub mod scripts;
pub mod suit;
pub mod view;
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
/// map's pickups and chargers, its weapons and their view models, its
/// rules (deathmatch and team deathmatch, `rules`).
pub struct Hl2dmPlugins;

impl bevy::app::PluginGroup for Hl2dmPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        bevy::app::PluginGroupBuilder::start::<Self>()
            .add(crate::game_registry::RegisterGame(&INFO))
            .add(movement::Hl2MovementPlugin)
            .add(suit::SuitPlugin)
            .add(pickups::PickupsPlugin)
            .add(weapons::Hl2WeaponsPlugin)
            .add(view::Hl2ViewPlugin)
            .add(rules::Hl2RulesPlugin)
            .add(bodies::Hl2BodiesPlugin)
            .add(player_anim::Hl2AnimPlugin)
            .add(bodies::Hl2DeadViewPlugin)
    }
}

/// HL2:DM as one bundle for an app made of several games (lift C4,
/// `chimera_world::game_bundle`): its info, parts and maps, its guns'
/// view models on other games' maps (`GUEST`), its binds (Shift sprints,
/// `+speed`; Ctrl ducks; Alt walks: its `cfg/config_default.cfg`), its
/// view models' field of view and its parts in the composed simulation
/// (`party_sim`). No HUD yet (M9).
pub static BUNDLE: chimera_world::game_bundle::GameBundle = chimera_world::game_bundle::GameBundle {
    info: &INFO,
    parts: &parts::PARTS,
    loader: &MAP_LOADER,
    guest: Some(&GUEST),
    kept: &["hl2dm:weapon_"],
    binds: &[("shift", "+speed"), ("ctrl", "+duck"), ("alt", "+walk")],
    view_model_fov: Some(view::fov),
    sim: party_sim,
    display_names: Some(weapons::display_names),
};

/// HL2:DM's guns on the maps of other games.
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing {
    game: GAME,
    dress: |map, _| maps::dress_guest(map, BUNDLE.kept),
};

/// What HL2:DM adds to a game of several games: the game registered, map
/// logic for its maps, its movement (not the match's) with the suit, its
/// guns (data, hooks, recoil, the shove, its rung extras) and their view
/// models' rules and motion. Not its game's rules (the loadout, pickups).
pub fn party_sim(app: &mut bevy::app::App) {
    use crate::game_registry::GameAppExt;
    app.register_game(&INFO);
    if !app.is_plugin_added::<source_logic::logic::LogicPlugin>() {
        app.add_plugins(source_logic::logic::LogicPlugin);
    }
    movement::add_movement(app);
    if !app.is_plugin_added::<suit::SuitPlugin>() {
        app.add_plugins(suit::SuitPlugin);
    }
    app.add_plugins(weapons::Hl2GunsPlugin);
    view::add_rules(app);
    if !app.is_plugin_added::<bodies::Hl2BodiesPlugin>() {
        app.add_plugins((bodies::Hl2BodiesPlugin, player_anim::Hl2AnimPlugin));
    }
}
