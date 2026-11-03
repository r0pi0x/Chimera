//! CS:S's front end in an app made of several games (its bundle's client
//! parts, `game::bundle`): its HUD on the client's shell (`with_hud`:
//! health, armour and ammo panels, death notices, damage sprites, the
//! minigame's panel and its weapon selection row, which takes the
//! shell's weapon menu's place; the radar, target ID, freeze cam and
//! voice status, each drawing only while CS:S's HUD is the gameplay's: a
//! CS:S gun in hand), the map's HUD look as CS:S's (`client_app`) and
//! what its bodies play for a player holding a CS:S gun.

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_world::game_bundle::GameClient;

/// CS:S's front end in a party app (beside its `game::BUNDLE`).
pub static CLIENT: GameClient = GameClient {
    game: crate::game::GAME,
    hud: Some(with_hud),
    app: Some(client_app),
};

/// CS:S's HUD over the shell's plugins, after the base game's HUD.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    use crate::client::{hud::HudPlugin, view::ViewPlugin, weapon_menu::WeaponMenuPlugin};
    use super::{game_hud::GameHudPlugin, hud_sprites::HudSpritesPlugin, minigame_hud::MinigameHudPlugin};
    shell
        .disable::<WeaponMenuPlugin>()
        .add_after::<HudPlugin>(GameHudPlugin)
        .add_after::<GameHudPlugin>(HudSpritesPlugin)
        .add_after::<HudSpritesPlugin>(MinigameHudPlugin)
        .add_after::<ViewPlugin>(super::weapon_select::WeaponSelectPlugin)
        .add_after::<crate::client::game_text::GameTextPlugin>(super::radar::RadarPlugin)
        .add_after::<MinigameHudPlugin>(super::target_id::TargetIdPlugin)
        .add_after::<super::target_id::TargetIdPlugin>(super::freeze_cam::FreezeCamPlugin)
        .add_after::<super::freeze_cam::FreezeCamPlugin>(super::voice_hud::VoiceHudPlugin)
}

/// The rest of CS:S's front end on the client: the map's HUD look is
/// CS:S's (`client::hud::HudLookOwner`), and what CS:S's bodies play.
pub fn client_app(app: &mut App) {
    app.insert_resource(crate::client::hud::HudLookOwner(crate::game::GAME))
        .add_plugins(crate::game::player_anim::PlayerAnimPlugin);
}
