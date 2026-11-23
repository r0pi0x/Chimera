//! Halo's front end on the client shell, its own crate above the game's
//! simulation (`halo_ce`), so that the simulation and its tests never build
//! the shell (docs/plans/active/architecture-for-iteration.md, stage 1): its weapons'
//! own buttons (`binds`: `+melee`, `+zoom`, `+grenade`, `+grenade_switch`) and its HUD (`hud`:
//! specs/halo_ce/hud.md, drawn from the disc's HUD tags in place of the
//! shell's fallback crosshair and status), its weapons' kill-feed icons
//! from the disc (`kill_icons`) and, in the party apps, its zoom on the
//! player's own field of view (`party_view`); a hit's screen flash and
//! camera impulse and shake on its victim (`victim_view`).

pub mod hud;

pub mod fx_lights;

pub mod party_view;

pub mod kill_icons;

pub mod victim_view;

// `crate::game`, `crate::map` and the rest, as inside the game's crate,
// where this code began.
use chimera_core::{core, gameplay};
use chimera_world::map;
use halo_ce::game;

use bevy::app::PluginGroupBuilder;
use chimera_world::game_bundle::GameClient;

/// The shell's plugins with Halo's HUD after the shell's own
/// (`hud::HaloHudPlugin`, its material's renderer).
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
        .add_after::<chimera_client::client::hud::HudPlugin>(hud::HaloHudPlugin)
        .add_after::<hud::HaloHudPlugin>(hud::material::HudRenderPlugin)
        .add_after::<hud::HaloHudPlugin>(hud::score::HaloScorePlugin)
}

/// The weapons' buttons beside fire, attack2 (also the zoom) and reload:
/// the melee, the zoom and the grenade throw as named actions
/// (`game::weapons::systems`, `game::weapons::grenades`).
pub fn binds(app: &mut bevy::app::App) {
    use chimera_client::client::binds::BindsAppExt;
    use crate::game::weapons::{
        grenades::{GRENADE_ACTION, SWITCH_ACTION},
        systems::{MELEE_ACTION, ZOOM_ACTION},
    };
    app.bound_action(MELEE_ACTION).bound_action(ZOOM_ACTION).bound_action(GRENADE_ACTION).bound_action(SWITCH_ACTION);
    fx_lights::plugin(app);
    victim_view::plugin(app);
}

/// Halo's front end in an app made of several games (beside its
/// `game::BUNDLE`): its HUD while a Halo gun is in hand (`with_hud`), its
/// weapons' buttons (`binds`) and the HUD's party layout (`party_app`).
pub static CLIENT: GameClient = GameClient { game: crate::game::GAME, ui_crate: env!("CARGO_CRATE_NAME"), hud: Some(with_hud), app: Some(party_app), base: None };

/// The party apps' front end: the buttons, and the HUD's shield and
/// health block moved to the bottom left in the motion tracker's place
/// (the shared kill feed is at the top right), no tracker
/// (`hud::layout::HudLayout::Party`), and the zoom narrowing the player's
/// own field of view (`party_view`). Halo's own app keeps the disc's
/// layout and Halo's angles.
pub fn party_app(app: &mut bevy::app::App) {
    binds(app);
    app.insert_resource(hud::HaloHudLayout(hud::layout::HudLayout::Party));
    party_view::plugin(app);
    // The HUD's data read at once: the kill feed's icons (`kill_icons`).
    app.add_systems(bevy::app::Update, hud::nodes::load);
}

/// Halo's front end under the name every game's has it (`party::CLIENT`;
/// docs/new-game.md).
pub mod party {
    pub use super::{CLIENT, with_hud};
}
