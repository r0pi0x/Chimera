//! Day of Defeat: Source's parts, one module per subsystem, and what the
//! game tells the shell about itself (`INFO`: its id, folder, Steam app,
//! tick, map names and cycle, console namespaces). M0 of
//! docs/plans/active/dods.md:
//!
//! - `mount`: the install's search path from its own `dod/gameinfo.txt`
//!   (`source_engine::search_path`), the asset store under or instead of
//!   it (`mount::served`); the dedicated server's layout mounts as it is
//!   (specs/dods/install.md);
//! - `maps`: Source's BSP conversion with DoD:S's data (`DODS`: spawns
//!   from `info_player_allies` and `info_player_axis`, gravity 800;
//!   `MAP_LOADER` for `dods:<map>`);
//! - `bodies`: the twelve classes' player models as bodies (MDL 44, their
//!   animations from the shared MDL 48), worn by name;
//! - `movement`: `dods:movement`, Source's movement with DoD:S's run
//!   speed, friction and stances (stand 72 / eye 58, duck 45 / eye 34),
//!   with `stance` (prone, deployed machine guns) and `stamina`;
//! - `weapons`: M0's stand-in gun, the Colt, as data;
//! - `player_anim`: the bodies' animation from the shared model's
//!   activities (stance, movement, the gun's set, sights, deploy, fire and
//!   reload gestures);
//! - `parts`: what the game lends other games' apps (`PARTS`).
//!
//! `DodsPlugins` adds the game's parts after `crate::SimPlugins`; `BUNDLE`
//! is the game for a party app (its maps, its gun, its bodies on other
//! games' maps: `GUEST`).
//!
//! Known shortcuts:
//! - `INFO`'s default binds are empty: the shell reads the install's
//!   `cfg/config_default.cfg`.
//! - The tick is the measured default (0.015 s, install.md, "Server
//!   facts"); no `-tickrate`.

pub mod bodies;
pub mod maps;
pub mod mount;
pub mod movement;
pub mod parts;
pub mod player_anim;
pub mod stamina;
pub mod stance;
pub mod view;
pub mod weapons;

pub use maps::MAP_LOADER;

use bevy::app::{PluginGroup, PluginGroupBuilder};

/// The game's id (`dods`): its maps' ids (`dods:dod_avalanche`), its key
/// in chimera.local.toml (`[games.dods] path`), `CHIMERA_DODS_ROOT` and
/// its content cache.
pub const GAME: &str = "dods";

/// The game's folder inside its install, holding `gameinfo.txt`.
pub const CONTENT_DIR: &str = "dod";

/// DoD:S's server tick: 0.015 s (66.7 Hz; specs/dods/install.md,
/// "Server facts").
pub const TICK_INTERVAL: f64 = 0.015;

/// Source's unit, the inch, in metres.
pub const METERS_PER_UNIT: f32 = 0.0254;

/// The nine stock maps (specs/dods/install.md, "Maps").
pub const STOCK_MAPS: &[&str] = &[
    "dod_anzio",
    "dod_argentan",
    "dod_avalanche",
    "dod_colmar",
    "dod_donner",
    "dod_flash",
    "dod_jagd",
    "dod_kalt",
    "dod_palermo",
];

/// What the client shell knows of DoD:S (`game_registry`): its install
/// (Steam app 300, the retail client, or `[games.dods] path`, or
/// `$CHIMERA_DODS_ROOT`: the dedicated server's layout from SteamCMD needs
/// one of the two; the folder holding `dod/`, `hl2/` and `platform/`),
/// folder, map names and cycle, tick and console namespaces (`dods`, with
/// the game folder's name `dod` as an alias). The required files are in
/// both layouts (install.md, "Layouts").
pub static INFO: crate::game_registry::GameInfo = crate::game_registry::GameInfo {
    id: GAME,
    short_name: "DoD:S",
    name: "Day of Defeat: Source",
    content_dir: CONTENT_DIR,
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    map_cycle: source_engine::maps::map_cycle,
    map_names: source_engine::maps::map_names,
    steam: Some(crate::mount::install::SteamGame {
        app_id: 300,
        name: "Day of Defeat Source",
        required: &["dod/gameinfo.txt", "dod/dod_pak_dir.vpk", "hl2/hl2_misc_dir.vpk"],
    }),
    map_file: |name| Some(format!("maps/{name}.bsp")),
    import_map: mount::import_map,
    default_binds_cfg: "cfg/config_default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: source_engine::source_movement::CVARS,
    meters_per_unit: METERS_PER_UNIT,
    console_namespaces: &["dods", "dod"],
};

/// DoD:S's parts, after the engine's base (`crate::SimPlugins`): the game
/// registered (`INFO`), its movement, its gun and its bodies.
pub struct DodsPlugins;

impl PluginGroup for DodsPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(crate::game_registry::RegisterGame(&INFO))
            .add(movement::DodsMovementPlugin)
            .add(DodsViewPlugin)
            .add(weapons::DodsGunsPlugin)
            .add(bodies::DodsBodiesPlugin)
            .add(player_anim::DodsAnimPlugin)
    }
}

/// DoD:S's view models in its own simulation: the rules and settings.
pub struct DodsViewPlugin;

impl bevy::app::Plugin for DodsViewPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        view::add_rules(app);
        app.insert_resource(crate::map::ViewModelSettings { fov: view::VIEWMODEL_FOV, default_fov: 90.0, right_hand: 1, draw: 1 });
    }
}

/// DoD:S as one bundle for an app made of several games
/// (`chimera_world::game_bundle`): its info, parts and maps, its guns'
/// models and its class bodies on other games' maps (`GUEST`), its view models' field of view, its guns'
/// names, and its parts in a composed simulation (`party_sim`).
pub static BUNDLE: chimera_world::game_bundle::GameBundle = chimera_world::game_bundle::GameBundle {
    info: &INFO,
    parts: &parts::PARTS,
    loader: &MAP_LOADER,
    guest: Some(&GUEST),
    kept: &["dods:weapon_"],
    // DoD:S's prone key (its `+alt1`; `stance`).
    binds: &[("z", "+alt1")],
    view_model_fov: Some(view::fov),
    view_model_fov_cvar: None,
    base: None,
    sim: party_sim,
    display_names: Some(weapons::display_names),
    player_options: Some(chimera_core::game_data!("data/player_options.toml")),
};

/// DoD:S's gun and bodies on the maps of other games.
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing {
    game: GAME,
    // Converted once per app and load kind, then copied into each map
    // (`chimera_world::map::kept`).
    dress: |map, options| {
        crate::map::kept::dress_cached(
            map,
            options,
            &crate::map::kept::Dressing {
                game: GAME,
                converter: maps::CONVERTER,
                mount: &maps::guest_mount,
                convert: &|mount, m| maps::dress_guest_in(mount, m, BUNDLE.kept, !options.textures),
                variant: "",
                memos: None,
            },
        )
    },
};

/// What DoD:S adds to a game of several games: the game registered, map
/// logic for its maps, its movement (not the match's), its view-model
/// rules, its guns, its
/// bodies and their animation, and its class bodies as practice targets (`targets`).
pub fn party_sim(app: &mut bevy::app::App) {
    use crate::game_registry::GameAppExt;
    app.register_game(&INFO);
    if !app.is_plugin_added::<source_logic::logic::LogicPlugin>() {
        app.add_plugins(source_logic::logic::LogicPlugin);
    }
    movement::add_movement(app);
    view::add_rules(app);
    if !app.is_plugin_added::<weapons::DodsGunsPlugin>() {
        app.add_plugins((weapons::DodsGunsPlugin, bodies::DodsBodiesPlugin, player_anim::DodsAnimPlugin));
        bodies::add_targets(app);
    }
}
