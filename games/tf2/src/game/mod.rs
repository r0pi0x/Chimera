//! Team Fortress 2's parts, one module per subsystem, and what the game
//! tells the shell about itself (`INFO`: its id, folder, Steam app, tick,
//! map names and cycle, console namespaces). M1 of
//! docs/plans/active/tf2.md:
//!
//! - `mount`: the install's search path from its own `tf/gameinfo.txt`
//!   (`source_engine::search_path`), the asset store under or instead of
//!   it (`mount::served`); an archive the `_dir` files list but that is
//!   missing (the dedicated server's textures and sounds) is skipped file
//!   by file (specs/tf2/install.md 1.5);
//! - `maps`: Source's BSP conversion with TF2's data (`TF2`: spawns from
//!   `info_player_teamspawn`, gravity 800; `MAP_LOADER` for `tf2:<map>`);
//! - `bodies` (M3): the classes' player models as bodies (the gun's
//!   class's, the team's skin), `player_anim` their animation on Source's
//!   shared half (activities by state and the gun's kind, the move grid,
//!   aim, gestures);
//! - `hud` (M3): TF2's HUD look from the install (the scheme's colours
//!   and fonts, the layout, the kill icons, the health crosses), in the
//!   map's kit;
//! - `classes` (M3): the nine classes as data (data/classes.toml: health,
//!   speed, eyes, air jumps, models) and which class each gun makes its
//!   holder;
//! - `movement` (M3): `tf2:movement`, Source's movement with TF2's config
//!   and the class of the gun in hand (speed, eyes, the Scout's air jump,
//!   max health);
//! - `parts`: what the game lends other games' apps (`PARTS`): its twelve
//!   guns, its movement;
//! - `ice`: the ICE cipher of the encrypted scripts, ported from the
//!   designer's public-domain reference (THIRD_PARTY.md; M2a);
//! - `scripts`: the weapon and class scripts (`.ctx`) through the mount,
//!   plain from the asset store or decrypted with the user's key, and
//!   which of the two served;
//! - `sets`: what `chimera_pack tf2`'s `weapon/<id>` and `players` sets
//!   hold;
//! - `weapons` (M2b): the Scattergun, Rocket Launcher, Shotgun and
//!   Minigun as data (data/weapons) with TF2's gun rules as hooks (the
//!   distance modifier, the first pellet, the rocket's blast, the
//!   Minigun's spin), where their numbers came from (`script_numbers`);
//!   (tf2-guns2) the Pistol, SMG, Sniper Rifle, Revolver, Grenade and
//!   Stickybomb Launchers, Flame Thrower and Syringe Gun with theirs (the
//!   scoped charge, the grenade, the sticky, the flame, the needle), and
//!   their looks (muzzle flashes, explosions: `weapons::looks`);
//! - `view`: the guns' view models (the class's arms with the `c_` weapon
//!   bone-merged) and their rules on Chimera's driver.
//!
//! `Tf2Plugins` adds the game's parts after `crate::SimPlugins`; `BUNDLE`
//! is the game for a party app (its maps, its guns: `GUEST` on other
//! games' maps).
//!
//! Known shortcuts:
//! - `INFO`'s default binds are empty: the shell reads the install's
//!   `cfg/config_default.cfg`.
//! - The tick is the measured default (0.015 s, specs/tf2/movement.md,
//!   "Units and conventions"); no `-tickrate`.

pub mod bodies;
pub mod classes;
pub mod hud;
pub mod ice;
pub mod maps;
pub mod mount;
pub mod movement;
pub mod parts;
pub mod player_anim;
pub mod scripts;
pub mod sets;
pub mod view;
pub mod weapons;

pub use maps::MAP_LOADER;

use bevy::app::{PluginGroup, PluginGroupBuilder};

/// The game's id (`tf2`): its maps' ids (`tf2:ctf_2fort`), its key in
/// chimera.local.toml (`[games.tf2] path`), `CHIMERA_TF2_ROOT` and its
/// content cache.
pub const GAME: &str = "tf2";

/// The game's folder inside its install, holding `gameinfo.txt`.
pub const CONTENT_DIR: &str = "tf";

/// TF2's server tick: 0.015 s (66.67 Hz; specs/tf2/movement.md, measured
/// on the dedicated server).
pub const TICK_INTERVAL: f64 = 0.015;

/// Source's unit, the inch, in metres.
pub const METERS_PER_UNIT: f32 = 0.0254;

/// The maps M1 plays and tests (specs/tf2/install.md 2.2).
pub const M1_MAPS: &[&str] = &["ctf_2fort", "cp_badlands"];

/// What the client shell knows of TF2 (`game_registry`): its install
/// (Steam app 440, the retail client, or `[games.tf2] path`, or
/// `$CHIMERA_TF2_ROOT`; the folder holding `tf/`, `hl2/` and
/// `platform/`), folder, map names and cycle, tick and console
/// namespaces (`tf2`, with the game folder's name `tf` as an alias). The
/// required files are in both the client's and the dedicated server's
/// layouts (install.md 1.3).
pub static INFO: crate::game_registry::GameInfo = crate::game_registry::GameInfo {
    id: GAME,
    short_name: "TF2",
    content_dir: CONTENT_DIR,
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    map_cycle: source_engine::maps::map_cycle,
    map_names: source_engine::maps::map_names,
    steam: Some(crate::mount::install::SteamGame {
        app_id: 440,
        name: "Team Fortress 2",
        required: &["tf/tf2_misc_dir.vpk", "hl2/hl2_misc_dir.vpk"],
    }),
    map_file: |name| Some(format!("maps/{name}.bsp")),
    import_map: mount::import_map,
    default_binds_cfg: "cfg/config_default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: source_engine::source_movement::CVARS,
    meters_per_unit: METERS_PER_UNIT,
    console_namespaces: &["tf2", "tf"],
};

/// TF2's parts, after the engine's base (`crate::SimPlugins`): the game
/// registered (`INFO`), its movement (`movement`), its guns
/// (`weapons`) and their view models' rules (`view`).
pub struct Tf2Plugins;

impl PluginGroup for Tf2Plugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(crate::game_registry::RegisterGame(&INFO))
            .add(movement::Tf2MovementPlugin)
            .add(weapons::Tf2GunsPlugin)
            .add(Tf2ViewPlugin)
            .add(bodies::Tf2BodiesPlugin)
            .add(player_anim::Tf2AnimPlugin)
    }
}

/// TF2's view models in its own simulation: the rules and settings.
pub struct Tf2ViewPlugin;

impl bevy::app::Plugin for Tf2ViewPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        view::add_rules(app);
        app.insert_resource(crate::map::ViewModelSettings {
            fov: view::VIEWMODEL_FOV,
            default_fov: view::DEFAULT_FOV,
            right_hand: 1,
            draw: 1,
        });
    }
}

/// TF2 as one bundle for an app made of several games
/// (`chimera_world::game_bundle`): its info, parts and maps, its guns'
/// view and held models and its classes' bodies on other games' maps
/// (`GUEST`), its view models' field of view, its guns' names, and its parts
/// in a composed simulation (`party_sim`). No binds of its own (TF2's
/// are the shared ones).
pub static BUNDLE: chimera_world::game_bundle::GameBundle = chimera_world::game_bundle::GameBundle {
    info: &INFO,
    parts: &parts::PARTS,
    loader: &MAP_LOADER,
    guest: Some(&GUEST),
    kept: &["tf2:tf_weapon_"],
    binds: &[],
    view_model_fov: Some(view::fov),
    sim: party_sim,
    display_names: Some(weapons::display_names),
};

/// TF2's guns on the maps of other games.
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing {
    game: GAME,
    dress: |map, options| maps::dress_guest_with(map, BUNDLE.kept, !options.textures),
};

/// What TF2 adds to a game of several games: the game registered, map
/// logic for its maps, its movement (not the match's), its guns
/// and their view models' rules.
pub fn party_sim(app: &mut bevy::app::App) {
    use crate::game_registry::GameAppExt;
    app.register_game(&INFO);
    if !app.is_plugin_added::<source_logic::logic::LogicPlugin>() {
        app.add_plugins(source_logic::logic::LogicPlugin);
    }
    movement::add_movement(app);
    app.add_plugins(weapons::Tf2GunsPlugin);
    view::add_rules(app);
    if !app.is_plugin_added::<bodies::Tf2BodiesPlugin>() {
        app.add_plugins((bodies::Tf2BodiesPlugin, player_anim::Tf2AnimPlugin));
    }
}
