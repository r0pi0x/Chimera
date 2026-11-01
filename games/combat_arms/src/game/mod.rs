//! Combat Arms' parts, one module per subsystem (each its own plugin, so
//! parallel implementation work never edits the same file), and what the
//! game tells the shell about itself (`INFO`). Wave 0: placeholders on
//! Chimera's pieces, each module saying which spec will define it
//! (specs/combat_arms/<name>.md, the names docs/plans/active/combat-arms.md
//! gives them):
//!
//! - `movement`: the movement slot, `combat_arms:movement` (movement.md);
//! - `weapons`: firing (the weapon frame on placeholder data);
//! - `inventory`: classes, switching, the loadout, the knife, pickup and
//!   drop (inventory.md, implemented);
//! - `grenades`: thrown weapons: the M67, XM84, M18, M-23F and G7A, their
//!   throw, flight, fuse and effects on the server's 20 Hz frames
//!   (grenades.md, implemented);
//! - `attachments`: weapon attachments: the parts a weapon accepts, their
//!   fitting and effects (attachments.md, implemented);
//! - `rungs`: a gun game ladder's Combat Arms rungs' extras (a fitted
//!   set, a character, a melee weapon, the same for everyone on a rung);
//! - `hud`: the weapon data the HUD reads (`combat_arms:hud`: crosshair
//!   fields and the weapon panel's class; hud_crosshair.md);
//! - `data`: Combat Arms' data files (games/combat_arms/data/) and, later,
//!   the install's attribute files;
//! - `mount`: the install's `.rez` archives as a mount (`INFO.open`);
//! - `bots`: the computer soldiers of Bot Deathmatch (bots.md): roster,
//!   senses, goals, aim and fire, on navigation areas generated from the
//!   map's collision;
//! - `characters`: the player characters (characters.md): model choice,
//!   third-person animation, the posed skeleton's hit spheres and
//!   sockets, death animations and bodies;
//! - `rules`: game modes as rule modules (game_modes.md: Team Death
//!   Match and Elimination Pro, `ca_mode`);
//! - `net`: the game's part of the network (armour, the pickup prompt,
//!   the match, respawn requests;
//!   the parts above register what a client predicts themselves);
//! - `world`: the install's maps (world `.dat` files) as Chimera's
//!   `MapData` (world_dat.md);
//! - `model`: the install's models (`.ltb`) and their animations as
//!   Chimera's `MapModel` and `AnimSet` (models_ltb.md, animation.md);
//! - `props`: models the maps' objects place, as animated props;
//! - `render_style`: how a model piece's render style draws (blended,
//!   added, glowing; models_ltb.md, characters.md 2);
//! - `view_model`: the weapons' first-person models from the install
//!   with the character's hands, their animation state machine, sway,
//!   muzzle flash and shells (view_models.md);
//! - `effects`: muzzle flashes, tracers, impacts and decals by surface,
//!   blood, casings, grenade looks, and how every sound is chosen and
//!   played (effects_sounds.md);
//! - `radar`: the radar's and the full-screen map's data and rules (the
//!   spec's arithmetic, who is shown, the mission files' map images, the
//!   scanner items, what the server sends at a round start;
//!   radar_minimap.md);
//! - `volumes`: the maps' volumes (water, kill and damage volumes);
//! - `gear`: what a player's vest, helmet, goggles, backpack, gear mods and
//!   set items change, and the loadout that brings them (gear_perks.md);
//! - `perks`: the perks a loadout switches on and what each changes;
//! - `objects`: the maps' world objects: doors, moving and spinning world
//!   models, keyframers, breakables, triggers and the command strings
//!   they run (world_objects.md);
//! - `death_cam`: the death camera, the kill-cam screen's values and the
//!   respawn countdown, the client's own (death_killcam.md).
//! - `chat`: radio and chat: the radio menus and calls, chat channels,
//!   text commands, filter, muting, spam limits and voice channels
//!   (radio_chat.md).
//! - `lobby`: rooms, ready and start, the loadout's rules and the lobby
//!   on the engine (lobby.md; `ca_lobby 1`);
//! - `music`: the menu playlist, the cues of a match, map music triggers,
//!   ambient map sounds and the two volume channels (music_ambience.md).
//! - `options`: the options' settings, the player profile, key binding,
//!   the mouse, sliders, gamma and quality presets, sound levels and the
//!   Other tab's social options (options.md).
//!
//! `CombatArmsPlugins` adds them all (after `crate::SimPlugins`); the
//! HUD is the front end's (`ui::hud`).

pub mod attachments;
pub mod bots;
pub mod characters;
pub mod chat;
pub mod data;
pub mod death_cam;
pub mod effects;
pub mod gear;
pub mod grenades;
pub mod hud;
pub mod inventory;
pub mod lobby;
pub mod model;
pub mod music;
pub mod missions;
pub mod mount;
pub mod movement;
pub mod net;
pub mod parts;
pub mod perks;
pub mod objects;
pub mod options;
pub mod props;
pub mod radar;
pub mod render_style;
pub mod rules;
pub mod rungs;
pub mod special;
pub mod view_model;
pub mod volumes;
pub mod weapons;
pub mod world;

use bevy::{app::PluginGroupBuilder, prelude::*};

use crate::game_registry::{GameInfo, RegisterGame};

/// The game's id: maps are `combat_arms:<name>`, weapons
/// `combat_arms:<weapon>`, its install's key in chimera.local.toml.
pub const GAME: &str = "combat_arms";

/// Combat Arms the gameplay when the app has chosen none, and its
/// announcer pack (`parts::ANNOUNCER`)
/// (`gameplay::GameplayAppExt::default_gameplay`): its match-wide systems
/// (rules, bots, the match's state on the network) run only while it is
/// the gameplay's rules (`gameplay::gameplay_is(GAME)`).
pub struct CaGameplayPlugin;

impl Plugin for CaGameplayPlugin {
    fn build(&self, app: &mut App) {
        use crate::gameplay::{Gameplay, GameplayAppExt};
        app.default_gameplay(Gameplay::of(&INFO));
        // Its announcer (kill_streaks.md): the default pack wherever the
        // game is played (`sv_announcer default`).
        chimera_core::announcer::AnnouncerAppExt::register_announcer(app, parts::ANNOUNCER);
    }
}

/// The game's tick: the movement clock's 15 ms (specs/combat_arms/
/// movement.md, "Tick clock"; the engine's default movement rate).
pub const TICK_INTERVAL: f64 = 0.015;

/// What the shell needs of Combat Arms (`game_registry`). `open` mounts
/// the install's archives (`mount`); nothing reads from it yet (the
/// `data` module will read the install's attribute files through it),
/// there is no map cycle or map import, and the binds are the
/// table below.
pub static INFO: GameInfo = GameInfo {
    id: GAME,
    short_name: "CA",
    // PLACEHOLDER: no file of the install is read yet.
    content_dir: "",
    tick_interval: TICK_INTERVAL,
    open: mount::open,
    map_cycle: |_| Vec::new(),
    map_names: world::map_names,
    steam: None,
    // In the install's archives (`worlds*/<name>.dat`, `world::find`), not
    // a file clients download.
    map_file: |_| None,
    import_map: |_| Err("Combat Arms maps load from the install by id (combat_arms:<name>), not from a file".into()),
    // No such file in an install: the built-in table is used.
    default_binds_cfg: "chimera_placeholder_binds.cfg",
    default_binds: DEFAULT_BINDS,
    // Shift was `+speed` (walk and sprint at once) before movement.md's
    // two keys.
    old_default_binds: OLD_DEFAULT_BINDS,
    movement_cvars: &[],
    meters_per_unit: inventory::UNIT,
    console_namespaces: &["combat_arms", "ca"],
};

/// Combat Arms' default binds. Weapons, use and sprint are the default
/// profile's (specs/combat_arms/inventory.md 2: class keys 1-5, Q the
/// previous weapon, fire, zoom/knife secondary, R, E; nothing on the
/// wheel, 5; key 6, the objective class, waits for a sixth slot command in
/// the shell); sprint ("Dash", left Shift) and walk ("Speed", C) are two
/// keys (movement.md, "Input to control state"); moving, jumping and
/// crouching are PLACEHOLDER until a spec lists the profile's keys.
pub const DEFAULT_BINDS: &[(&str, &str)] = &[
    ("w", "+forward"),
    ("s", "+back"),
    ("a", "+moveleft"),
    ("d", "+moveright"),
    ("space", "+jump"),
    ("ctrl", "+duck"),
    ("shift", "+sprint"),
    ("c", "+walk"),
    ("mouse1", "+attack"),
    ("mouse2", "+attack2"),
    ("r", "+reload"),
    ("e", "+use"),
    ("q", "lastinv"),
    ("1", "slot1"),
    ("2", "slot2"),
    ("3", "slot3"),
    ("4", "slot4"),
    ("5", "slot5"),
    // The objective class (the mission bomb; `ui`'s `slot6`).
    ("6", "slot6"),
    // The scoreboard while held (ui_system.md I2; the key is ours).
    ("tab", "+showscores"),
    // Radio and chat (radio_chat.md Open question 5: from a development
    // seed profile): report, answer and request menus; all, team and clan
    // chat; Enter opens the input; push-to-talk and voice output.
    ("b", "radio_report"),
    ("v", "radio_answer"),
    ("n", "radio_request"),
    ("enter", "chat_open"),
    ("f2", "chat_all"),
    ("f3", "chat_team"),
    ("f4", "chat_clan"),
    ("t", "+voicechat"),
    ("f8", "voice_toggle"),
    // The radar's zoom (`=` expands, `-` decreases) and the full-screen
    // map while held (radar_minimap.md R3, F1; the engine's defaults).
    ("=", "+incradarscale"),
    ("-", "+decradarscale"),
    ("m", "+fulltacticalmap"),
];

/// The defaults before walk and sprint were two keys (`GameInfo::
/// old_default_binds`).
pub const OLD_DEFAULT_BINDS: &[(&str, &str)] = &[
    ("w", "+forward"),
    ("s", "+back"),
    ("a", "+moveleft"),
    ("d", "+moveright"),
    ("space", "+jump"),
    ("ctrl", "+duck"),
    ("shift", "+speed"),
    ("mouse1", "+attack"),
    ("mouse2", "+attack2"),
    ("r", "+reload"),
    ("e", "+use"),
    ("q", "lastinv"),
    ("1", "slot1"),
    ("2", "slot2"),
    ("3", "slot3"),
    ("4", "slot4"),
    ("5", "slot5"),
];

/// Combat Arms' parts, after `crate::SimPlugins`: the game registered
/// (`INFO`), then each subsystem's plugin.
pub struct CombatArmsPlugins;

impl PluginGroup for CombatArmsPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(RegisterGame(&INFO))
            .add(CaGameplayPlugin)
            .add(data::CaDataPlugin)
            .add(movement::CaMovementPlugin)
            .add(hud::CaHudDataPlugin)
            // Before `weapons`: their hooks must be there when the data
            // loads.
            .add(inventory::CaInventoryPlugin)
            .add(volumes::CaVolumesPlugin)
            .add(objects::CaObjectsPlugin)
            .add(grenades::CaGrenadesPlugin)
            .add(special::CaSpecialPlugin)
            // Its scanner hook must be there when the weapons' data loads.
            .add(radar::CaRadarPlugin)
            .add(options::CaOptionsPlugin)
            .add(attachments::CaAttachmentsPlugin)
            .add(rungs::CaRungsPlugin)
            .add(view_model::CaViewModelPlugin)
            .add(effects::CaEffectsPlugin)
            .add(weapons::CaWeaponsPlugin)
            .add(rules::CaRulesPlugin)
            .add(death_cam::CaDeathCamPlugin)
            .add(chat::CaChatPlugin)
            .add(music::CaMusicPlugin)
            .add(bots::CaBotsPlugin)
            .add(characters::CaCharactersPlugin)
            .add(gear::CaGearPlugin)
            .add(perks::CaPerksPlugin)
            .add(lobby::CaLobbyPlugin)
            .add(net::CaNetPlugin)
            .add(props::CaPropsPlugin)
    }
}

/// Combat Arms' weapons alone, after the engine's base and with no game
/// registered, rules, bots or network part: the data and every plugin
/// whose weapon hooks the data names (in `CombatArmsPlugins`' order),
/// then `weapons` (the radar's scanner hook without its radar). For an
/// app that offers its guns under another game's rules; never beside
/// `CombatArmsPlugins`, which includes all of these.
pub struct CombatArmsGunsPlugins;

impl PluginGroup for CombatArmsGunsPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(data::CaDataPlugin)
            .add(hud::CaHudDataPlugin)
            .add(inventory::CaInventoryPlugin)
            .add(grenades::CaGrenadesPlugin)
            .add(special::CaSpecialPlugin)
            .add(radar::CaScannerHook)
            .add(attachments::CaAttachmentsPlugin)
            .add(rungs::CaRungsPlugin)
            .add(view_model::CaViewModelPlugin)
            .add(effects::CaEffectsPlugin)
            .add(weapons::CaWeaponsPlugin)
    }
}
