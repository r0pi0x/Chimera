//! The damage screen effects on the screen (specs/combat_arms/
//! effects_sounds.md Follow-up (2026-11-14) 4): `game::damage_fx::
//! CaScreenFx` each frame to Chimera's full-screen passes (the tint as an
//! additive light and the darkening as the light scale, over the
//! finished 3D frame: `render::screen_post::ScreenPost`) and to the
//! first-person camera (the roll; the vertical FOV offset:
//! `client::screen_post::ViewEffects`).
//!
//! Known shortcuts: the tint and light scale apply to the finished frame
//! (the view model's too), not as the scene's global light; the FOV
//! offset is the vertical one only and doesn't accumulate frame to frame
//! (the quirk is harmless at the stock `FOVMax` 0); the HUD picture
//! (`CaScreenFx::hud`) isn't drawn (no mode we have uses a HUD with a
//! `DamageFX` control).

use bevy::prelude::*;

use crate::{
    client::screen_post::ViewEffects,
    game::damage_fx::{CaScreenFx, combine_tint},
};

pub struct CaDamageFxDrawPlugin;

impl Plugin for CaDamageFxDrawPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show);
    }
}

fn show(
    fx: Option<Res<CaScreenFx>>,
    post: Option<ResMut<chimera_present::render::screen_post::ScreenPost>>,
    view: Option<ResMut<ViewEffects>>,
) {
    let fx = fx.as_deref().cloned().unwrap_or_default();
    if let Some(mut post) = post {
        let add = combine_tint(&[fx.tint]);
        if post.light_add != add || post.light_scale != fx.light_scale {
            post.light_add = add;
            post.light_scale = fx.light_scale;
        }
    }
    if let Some(mut view) = view {
        let want = ViewEffects {
            roll: fx.roll,
            fov: fx.fov.y,
        };
        view.set_if_neq(want);
    }
}
