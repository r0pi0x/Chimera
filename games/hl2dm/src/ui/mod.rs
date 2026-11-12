//! HL2:DM's front end (the `ui` feature, M9 of docs/plans/active/hl2dm.md;
//! specs/hl2dm/hud.md, H8): its menus (GameUI's, `source_gameui`, with
//! `MENU_GAME`) and its HUD on the client shell. The panels are
//! Half-Life 2's: translucent rounded boxes in the bottom corners with a
//! dim-yellow label and large digits that glow and flash when they change,
//! driven by the named events of the install's `scripts/hudanimations.txt`
//! (`source_engine::hud::anim`); an aux power bar; the quick info
//! brackets and the crosshair; HL2's horizontal weapon selection; red
//! damage flashes; the pickup history; death notices; target ID; the
//! scoreboard and the intermission; the message of the day at join
//! (GameUI's text window, `source_gameui::motd`, in the install's
//! `resource/ui/textwindow.res`).
//!
//! - `hud`: the state machine (`Hl2Hud`): what each element remembers, the
//!   events it starts, the `Frame` it draws.
//! - `numeric`, `aux`, `quick`, `damage`, `selection`, `history`,
//!   `notices`, `scoreboard`: the elements; `look`: the install's layout,
//!   scheme and fonts at a window size; `frame`: the draw list.
//! - `lights`: the respawn light.
//! - `kill_icons`: its guns' icons for any app's kill feed.
//! - `systems`: the Bevy side (inputs from the simulation, the messages,
//!   the selection's commands, the nodes).
//!
//! The chat is the shell's (`client::chat`, in the `HudChat` box the look
//! sets from `resource/ui/basechat.res`). The simulation side of
//! `hl2dm` never uses this module.
//!
//! Known shortcuts:
//! - The menus: the Internet tab is greyed (no master server), and the
//!   layout's look is unmeasured against the real game (no refcmp of
//!   GameUI). The Multiplayer tab's player model and spray pickers work
//!   (`source_gameui::game_menu::pickers`); Import Spray is an in-game
//!   dialog taking a typed file path (no native file dialog dependency);
//!   an imported spray draws in the world like an install's (its picture,
//!   reduced to 64 x 64, travels with the spray); Create Server's Server Type
//!   sets `sv_lan` and the other cvars without any server code reading
//!   them yet; `TextEntry1` is drawn with the scheme's text-field look,
//!   unchecked against a capture.
//! - The damage flashes' trapezoids are straight bands and draw alpha
//!   blended, not added; the corner staircase and border of the
//!   scoreboard are drawn from boxes (its avatar cells empty, as without
//!   Steam), its section colours' background is left out, and its rows
//!   start 26 units down (the file gives the list the whole panel).
//! - The console selection styles (`hud_fastswitch` 2 and 3), the hints
//!   and the spectator GUI are not built. The message of the day
//!   (`source_gameui::motd`) draws HL2:DM's frame as boxes, without its
//!   logo.
//! - An ammo crate's refused opening sends no "FULL" row (the spec names
//!   only pickups; `game::hud_notes` has boxes and weapons).

pub mod aux_power;
pub mod damage;
pub mod frame;
pub mod history;
pub mod hud;
pub mod kill_icons;
pub mod lights;
pub mod look;
pub mod notices;
pub mod numeric;
pub mod quick;
pub mod scoreboard;
pub mod selection;
pub mod systems;

use bevy::prelude::*;
use chimera_client::client::weapon_menu::{WeaponMenuTimes, menu_input};
use chimera_world::map::hud::ActiveHud;

use crate::game::GAME;

/// What's HL2:DM's own in GameUI's menus (`source_gameui`): its names. Its
/// real menus have no bots (`hl2mp_bot_*` isn't built), no team dialog (specs/
/// hl2dm/hud.md 14: the client creates none) and no rounds.
pub static MENU_GAME: source_gameui::MenuGame = source_gameui::MenuGame {
    info: &crate::game::INFO,
    name: "Half-Life 2: Deathmatch",
    steam_folder: "Half-Life 2 Deathmatch",
    strings: "resource/hl2mp_english.txt",
    bots: None,
    rounds_cvar: None,
    teams: None,
    quick_start: None,
    // `nocrosshair` in its gameinfo.txt: its layout hides the crosshair's
    // controls and the Multiplayer tab's Advanced button.
    layout_hides: true,
    // Its Multiplayer page's player model and spray pickers (specs/hl2dm/
    // game_ui.md 2.1-2.3) and Create Server's Server Type (4).
    player_models: true,
    sprays: true,
    server_types: true,
};

/// HL2:DM's HUD panels (the look, the state machine, the nodes), without
/// the weapon selection's commands: what a party app adds (`party`).
pub struct Hl2dmHudPlugin;

impl Plugin for Hl2dmHudPlugin {
    fn build(&self, app: &mut App) {
        systems::register_cvars(app);
        // What it draws instead of the shell's, in an app that switches HUDs
        // too (the shell's `HudDraws`).
        app.world_mut().get_resource_or_init::<chimera_client::client::hud::HudDraws>().0.push((GAME, systems::DRAWS));
        // Its death notices, counted for the one-feed check (none while
        // the app draws the shared feed: `kill_feed::OwnKillFeed`).
        chimera_client::client::kill_feed::GameKillFeedAppExt::register_game_kill_feed(app, GAME, systems::feed_lines);
        app.init_resource::<systems::Hl2dmHud>()
            .init_resource::<chimera_client::client::weapon_icons::WeaponIcons>()
            // The game's pickups say them (not composed in the party apps).
            .add_message::<crate::game::hud_notes::HudPickup>()
            .add_systems(Startup, (systems::spawn_root, kill_icons::register))
            .add_systems(
                Update,
                (
                    systems::build.run_if(resource_exists_and_changed::<ActiveHud>),
                    (systems::target_id, systems::collect, systems::collect_board, systems::sync).chain().run_if(resource_exists::<ActiveHud>),
                )
                    .chain()
                    .distributive_run_if(chimera_core::gameplay::hud_is(GAME)),
            )
            .add_systems(Update, systems::teardown.run_if(resource_removed::<ActiveHud>))
            .add_systems(
                Update,
                systems::hide_at_main_menu.after(systems::sync).run_if(resource_exists::<chimera_client::client::front_end::MenuStatus>),
            );
    }
}

/// HL2:DM's HUD for a windowed game (after `client::ClientPlugins`): the
/// panels and the weapon selection's commands (`slotN`, `invnext`,
/// `invprev`, attack, `cancelselect`), which take the shell's weapon
/// menu's place.
pub struct Hl2dmUiPlugin;

impl Plugin for Hl2dmUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<chimera_client::client::weapon_menu::WeaponMenuPlugin>() {
            app.add_plugins(chimera_client::client::weapon_menu::WeaponMenuPlugin);
        }
        app.add_plugins((Hl2dmHudPlugin, source_gameui::GameUiPlugin(&MENU_GAME), source_gameui::motd::MotdWindowPlugin))
            // The selection closes at 1.25 s (a quarter into its fade).
            .add_systems(Update, (lights::flash, lights::fade).chain())
            .insert_resource(WeaponMenuTimes { menu: 1.25, show: 1.25, fade: 0.25, select_hold: 0.15 })
            .add_systems(Update, systems::selection_commands.before(menu_input).run_if(chimera_core::gameplay::hud_is(GAME)));
    }
}

/// HL2:DM's front end in an app made of several games (its bundle's
/// client parts, `game::BUNDLE`): the HUD panels on the client's shell,
/// drawing while HL2:DM's HUD is the gameplay's (an HL2:DM gun in hand), on
/// any game's map (its look comes with the map: `game::maps::dress_guest`),
/// with the minigame's panel and its weapon selection and lights while it
/// is up.
pub mod party {
    use bevy::{app::PluginGroupBuilder, prelude::*};
    use chimera_core::gameplay::hud_is;
    use chimera_world::game_bundle::GameClient;

    use super::{GAME, lights, systems};

    pub static CLIENT: GameClient = GameClient { game: GAME, hud: Some(with_hud), app: Some(client_app) };

    /// The HUD over the shell's plugins, after the base game's.
    pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
        shell
            .add_after::<chimera_client::client::hud::HudPlugin>(super::Hl2dmHudPlugin)
            .add_after::<super::Hl2dmHudPlugin>(chimera_client::client::minigame_panel::MinigamePanelPlugin(GAME))
    }

    /// The selection's commands (the weapon menu's data is the shell's, the
    /// row CS:S's or this HUD's) and the suit's lights while its HUD is up.
    pub fn client_app(app: &mut App) {
        app.add_systems(
            Update,
            (
                (lights::flash, lights::fade).chain().run_if(hud_is(GAME)),
                systems::selection_commands.before(chimera_client::client::weapon_menu::menu_input).run_if(hud_is(GAME)),
            ),
        );
    }
}
