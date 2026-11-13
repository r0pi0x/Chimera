//! What map logic shows on players' HUDs (`HudShow`, `show_on_hud`): HUD
//! messages (game_text), screen fades, hints and shakes, whose data is
//! `map::logic_view`'s (the client draws it: `client::game_text`,
//! `client::map_screen`, `client::senses`).

use bevy::prelude::*;

// What the logic shows on screen as plain data (`map::logic_view`: the
// client shell draws it without naming the logic), at its old paths.
pub use crate::map::logic_view::{
    FADE_TIME_UNIT, HUD_CHANNELS, HUD_TEXT_MAX, HudMessage, HudMessages, MapShake, ScreenFade, ScreenFades,
};

// A map's `say` line: `map::MapSay` (the network and the client read
// it without naming the logic), at its old path.
pub use crate::map::MapSay;

/// What map logic shows on one player's HUD (`to`) or everyone's (None):
/// the bridge shows it for the local player, and a network server sends
/// it to the client the player belongs to.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HudEvent {
    pub to: Option<Entity>,
    pub what: HudShow,
}

/// One thing a `HudEvent` shows.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HudShow {
    Text(HudMessage),
    Fade(ScreenFade),
    /// The key hint panel's text (env_hudhint); empty hides it.
    Hint(String),
    /// A full-screen overlay material (env_screenoverlay) for `secs`
    /// seconds (0 or less: until replaced); an empty name takes it away.
    Overlay {
        material: String,
        secs: f32,
    },
}

/// Show `what` on player `to`'s HUD (None: everyone's) from the authority
/// (map logic, a host's surprise): here at once when it is for the local
/// player or everyone (a listen server's host, an offline game), and
/// handed on as a `HudEvent`, which a network server sends to the
/// clients it is for (`net::forward_hud`: clients only, never back to
/// the host). Without the message registered it is shown here only.
pub fn send_hud(world: &mut World, to: Option<Entity>, what: HudShow) {
    let local = chimera_core::query_cache::first::<With<crate::core::LocalPlayer>>(world);
    if to.is_none() || to == local {
        let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed_secs_f64);
        show_on_hud(world, what.clone(), now);
    }
    if world.contains_resource::<bevy::ecs::message::Messages<HudEvent>>() {
        world.write_message(HudEvent { to, what });
    }
}

/// Show a `HudShow` on this client's HUD at `now` (`Time` elapsed).
pub fn show_on_hud(world: &mut World, what: HudShow, now: f64) {
    match what {
        HudShow::Text(m) => world.get_resource_or_init::<HudMessages>().show(m, now),
        HudShow::Fade(f) => world.get_resource_or_init::<ScreenFades>().show(f, now),
        HudShow::Hint(text) => world.get_resource_or_init::<ScreenFades>().hint = Some((text, now)),
        HudShow::Overlay { material, secs } => {
            world.get_resource_or_init::<ScreenFades>().overlay =
                (!material.is_empty()).then(|| (material.to_ascii_lowercase(), now, secs));
        }
    }
}
