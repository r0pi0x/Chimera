//! HL2:DM's front end (the `ui` feature, M9 of docs/plans/active/hl2dm.md;
//! specs/hl2dm/hud.md, H8): its HUD on the client shell. The panels are
//! Half-Life 2's: translucent rounded boxes in the bottom corners with a
//! dim-yellow label and large digits that glow and flash when they change,
//! driven by the named events of the install's `scripts/hudanimations.txt`
//! (`source_engine::hud::anim`); an aux power bar; the quick info
//! brackets and the crosshair; HL2's horizontal weapon selection; red
//! damage flashes; the pickup history; death notices; target ID; the
//! scoreboard and the intermission.
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
//! - The damage flashes' trapezoids are straight bands and draw alpha
//!   blended, not added; the corner staircase and border of the
//!   scoreboard are drawn from boxes, its avatars and section colours'
//!   background are left out, and its rows start 26 units down (the file
//!   gives the list the whole panel).
//! - The console selection styles (`hud_fastswitch` 2 and 3), the MOTD
//!   window, the hints and the spectator GUI are not built.
//! - An ammo crate's refused opening sends no "FULL" row (the spec names
//!   only pickups; `game::hud_notes` has boxes and weapons).
//! - On another game's map (the party apps) the map's own HUD look is that
//!   game's, so this HUD draws only when its look is the map's.

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

/// HL2:DM's HUD panels (the look, the state machine, the nodes), without
/// the weapon selection's commands: what a party app adds (`party`).
pub struct Hl2dmHudPlugin;

impl Plugin for Hl2dmHudPlugin {
    fn build(&self, app: &mut App) {
        systems::register_cvars(app);
        app.init_resource::<systems::Hl2dmHud>()
            .init_resource::<chimera_client::client::weapon_icons::WeaponIcons>()
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
            .add_systems(Update, systems::teardown.run_if(resource_removed::<ActiveHud>));
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
        app.add_plugins(Hl2dmHudPlugin)
            // The selection closes at 1.25 s (a quarter into its fade).
            .add_systems(Update, (lights::flash, lights::fade).chain())
            .insert_resource(WeaponMenuTimes { menu: 1.25, show: 1.25, fade: 0.25, select_hold: 0.15 })
            .add_systems(Update, systems::selection_commands.before(menu_input).run_if(chimera_core::gameplay::hud_is(GAME)));
    }
}

/// HL2:DM's front end in an app made of several games (its bundle's
/// client parts, `game::BUNDLE`): the HUD panels on the client's shell,
/// drawing while HL2:DM's HUD is the gameplay's (an HL2:DM gun in hand)
/// and the map's look is HL2:DM's.
pub mod party {
    use bevy::app::PluginGroupBuilder;
    use chimera_world::game_bundle::GameClient;

    pub static CLIENT: GameClient = GameClient { game: crate::game::GAME, hud: Some(with_hud), app: None };

    /// The HUD over the shell's plugins, after the base game's.
    pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
        shell.add_after::<chimera_client::client::hud::HudPlugin>(super::Hl2dmHudPlugin)
    }
}
