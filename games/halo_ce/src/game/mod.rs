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
//! - `maps::materials`: the shader classes as looks (reflections,
//!   self-illumination, masks, scrolls);
//! - `maps::teleporters`: the scenario's teleporter pairs as the engine's
//!   teleporters; `maps::collision` also publishes climbable sheets as
//!   ladders;
//! - `movement`: `halo_ce:movement`, Halo's player movement
//!   (specs/halo_ce/movement.md; its numbers in data/movement.toml);
//! - `vitality`: a Halo player's body and shield, recharge, overshield,
//!   camouflage and stun (health_shields.md; its numbers in
//!   data/vitality.toml and data/damage.toml), and `damage`: the seam every
//!   Halo damage source delivers through (`damage::HaloHit`);
//! - `weapons`: the nine held weapons and the two grenades (weapons.md,
//!   projectiles.md, multiplayer_rules.md 5): data/weapons/*.toml and
//!   data/grenades.toml, per-tick weapon rules with charging, projectiles
//!   with material responses, explosions and pickups, hits through
//!   `damage::HaloHit`, lent as rungs in `parts`;
//! - `weapons::aim`: the aim assist's parameters, levels and the bent
//!   shot (weapons.md section 15), on by default;
//! - `body`: the players' Spartan body (model, animations from the graph's
//!   unit tables, hit boxes from the collision model, armour colours), on
//!   Halo's maps and, from the donor map, on any other;
//! - `body::death`: a dead player's body, its death animation, sound
//!   and removal;
//! - `view`: the guns in hand: each gun's first-person view model with the
//!   arms, played from the weapons' events one frame a tick, hidden while
//!   zoomed, and its held model and stance; on Halo's maps and, from the
//!   donor map, on any other;
//! - `fx`: the guns' sounds and effects (effects.md, sounds.md): the library
//!   from the donor map, effects played as the engine's sounds, particles,
//!   decals and lights, shots shown on every machine; `fx::objects`: thrown
//!   grenades, explosions, plasma countdowns, items on the ground and their
//!   pickup sounds, on every machine;
//! - `vehicles`: the scenario's vehicles (the Warthog, the Ghost and the
//!   Scorpion; vehicles.md): placement as moving props, the mass-point
//!   physics on the engine's collision, driving, seats, the seat camera
//!   (`camera`) and the model's overlays (`look`), splatter and riders'
//!   damage;
//! - `rules`: Halo's game types (multiplayer_rules.md): the variant
//!   chosen by `halo_ce:gametype`, slayer, capture the flag, king of the
//!   hill, oddball, spawns, equipment and scoring, the match's view;
//! - `respawn`: Halo's players' loadout and respawn by
//!   multiplayer_rules.md 4 (`HaloRules`, Halo's own app only);
//! - `weapons::aim`'s `Bend` is off by default (`halo_autoaim`); `fx`
//!   has the dev command `halo_ce_blast` (a detonation at a point);
//! - `vehicles` also seats any game's character (the engine's
//!   `Carried`) and replicates who sits where (`NetSeat`, net part
//!   `halo_ce_vehicles`); in the party apps vehicles are placed only with
//!   `halo_ce:party_vehicles` on (off by default);
//! - `cross_game`: Halo's mechanisms under the party's cross-game table
//!   (a body holds the table's incoming, the armour brought back, the
//!   power reader's hand);
//! - `weapons::systems::BatteryRoundsPlugin`: what Halo's weapons state
//!   to the engine beyond their own weapon code: the battery weapons'
//!   rounds as a `Magazine`, the triggers that fire when released, each
//!   shot as the engine's shot fact;
//! - the front end's `ui::party_view` (the party zoom on the player's
//!   field of view) and `ui::kill_icons` (the kill-feed icons from the
//!   disc);
//! - `weapons::rung`: the party ladder's grenade rung; `movement` also
//!   fits the pill to the map's envelope (`body`, `envelope`);
//! - `view::displays`: the counter, compass and gauges on the guns
//!   (weapons.md 18);
//! - `eased`: the 30 Hz poses and objects drawn between their ticks at
//!   the frame rate (display only);
//! - `ticks`: how many 30 Hz Halo ticks an engine tick advances, exactly,
//!   at any tick rate (`halo_ticks`);
//! - `audio`: looping sounds, the level's background sound, environment
//!   and sound scenery, projectile loops and footsteps (`maps::audio`
//!   hands the level's sound data over);
//! - `sets`: Halo's asset sets for `chimera_pack sets` (`base` and `players`
//!   the donor map, `map/<name>` per multiplayer map); `mount::build` reads
//!   the install's build from a map header;
//! - the HUD is the front end's (`ui::hud`): `parts` names it (`hud:
//!   Some(GAME)`), so a Halo gun in a party app shows it;
//! - `parts`: what the game lends other games' apps (`PARTS`): its maps,
//!   by its pool rules and measurements (data/map_pool.toml,
//!   data/map_fitness.toml), played with other games' players and guns.
//!
//! `HaloPlugins` adds the game's parts after `crate::SimPlugins`.

pub mod audio;
pub mod body;
pub mod campaign;
pub mod cross_game;
pub mod damage;
pub mod eased;
pub mod fx;
pub mod maps;
pub mod mount;
pub mod movement;
pub mod parts;
pub mod respawn;
pub mod rules;
pub mod sets;
pub mod ticks;
pub mod vehicles;
pub mod view;
pub mod vitality;
pub mod weapons;

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
    ("f", "+melee"),
    ("g", "+grenade"),
    ("q", "+grenade_switch"),
    ("e", "+use"),
    ("tab", "+showscores"),
];

/// Halo's own keys over an app's shared binds, for a player whose identity
/// is Halo's (a Halo rung's in a party app): its melee and grenade buttons
/// (`weapons::systems::MELEE_ACTION`, `weapons::grenades::GRENADE_ACTION`),
/// as in `DEFAULT_BINDS`. No party game has a throw button of its own (the
/// other games' grenades are weapons in a slot), so Halo's is its own.
pub const BUNDLE_BINDS: &[(&str, &str)] = &[("f", "+melee"), ("g", "+grenade"), ("q", "+grenade_switch")];

/// Halo's parts, after `crate::SimPlugins`: the game registered (`INFO`),
/// its movement (`movement`), its players' vitality (`vitality`), its
/// damage seam (`damage`) and its players' bodies (`body`).
pub struct HaloPlugins;

impl PluginGroup for HaloPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(RegisterGame(&INFO))
            .add(movement::HaloMovementPlugin)
            .add(vitality::VitalityPlugin)
            .add(damage::DamagePlugin)
            .add(weapons::HaloWeaponsPlugin)
            .add(weapons::systems::BatteryRoundsPlugin)
            .add(fx::HaloFxPlugin)
            .add(audio::HaloAudioPlugin)
            .add(body::HaloBodyPlugin)
            .add(view::HaloViewPlugin)
            .add(respawn::HaloRespawnPlugin)
            .add(vehicles::HaloVehiclesPlugin)
            .add(rules::HaloRulesPlugin)
            .add(maps::teleporters::TeleportersPlugin)
    }
}

/// Halo as one bundle for an app made of several games
/// (`chimera_world::game_bundle`): its info, parts and maps; it brings its
/// players' body to other games' maps (`guest`: the Spartan, from the
/// donor map, `body::load::GUEST`) and adds its registration, its bodies'
/// animation and colours (`body::HaloBodyPlugin`), its surface names' classes (`maps::surfaces::
/// SURFACES`) and its map pool's rules and measurements
/// (`parts::add_map_pool`) to a composed simulation: its maps are played
/// there with the other games' movement and guns.
pub static BUNDLE: chimera_world::game_bundle::GameBundle = chimera_world::game_bundle::GameBundle {
    info: &INFO,
    parts: &parts::PARTS,
    loader: &MAP_LOADER,
    guest: Some(&audio::GUEST),
    // Its guns' view and held models stay on its maps in an app whose
    // gameplay is another game's (a party's Halo rungs on Hang 'Em High).
    kept: &["halo_ce:"],
    binds: BUNDLE_BINDS,
    view_model_fov: Some(view::fov),
    sim: |app| {
        use crate::game_registry::GameAppExt;
        app.register_game(&INFO);
        // Its maps' surface names as classes, for the guns of the games
        // played on them.
        crate::map::impact::SurfaceClassesAppExt::register_surface_classes(app, maps::surfaces::SURFACES);
        parts::add_map_pool(app);
        // Its guns as rungs: its movement (not the match's), its vitality
        // and damage seam, its weapons (`parts::add_guns`).
        parts::add_guns(app);
        weapons::grenades::party(app);
        // Its players' body, worn by its guns' holders (`body`).
        app.add_plugins(body::HaloBodyPlugin);
        app.add_plugins(campaign::CampaignPlugin);
        app.add_plugins(view::HaloViewPlugin);
        app.add_plugins(vehicles::HaloVehiclesPlugin);
        vehicles::place::party_vehicles(app);
        if !app.is_plugin_added::<audio::HaloAudioPlugin>() {
            app.add_plugins(audio::HaloAudioPlugin);
        }
        if !app.is_plugin_added::<maps::teleporters::TeleportersPlugin>() {
            app.add_plugins(maps::teleporters::TeleportersPlugin);
        }
    },
    display_names: None,
};
