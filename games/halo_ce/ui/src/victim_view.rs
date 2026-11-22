//! What a hit shows its victim, drawn (`game::fx::victim::VictimView`,
//! specs/halo_ce/effects.md "Damage effect presentation"): the screen
//! flash as Chimera's full-screen light (`render::screen_post::ScreenPost`:
//! the frame times `light_scale` plus `light_add`), the impulse's and
//! shake's turn and move on the first-person camera
//! (`client::screen_post::ViewEffects`). Whatever gun is in hand: it is the
//! unit's, not the HUD's.
//!
//! The flash by type (`flash_light`), with colour c and shown intensity I:
//! lighten adds c I; darken multiplies by 1 - (1 - c) I; tint mixes,
//! frame (1 - I) + c I; invert, frame (1 - 2 c I) + c I (the frame turned
//! over at I = 1 with white); max as lighten, min as darken.
//!
//! Known shortcuts: how Halo's renderer blends each flash type is not in
//! the specs (effects.md says "by type and intensity", hud.md has no
//! rule): the mapping above is ours, max and min are approximations, and
//! the colour's alpha is not used. The flash covers the view model too
//! (the pass runs on the finished 3D frame, under the HUD).

use bevy::prelude::*;
use chimera_client::client::screen_post::ViewEffects;
use chimera_present::render::screen_post::ScreenPost;

use crate::game::fx::victim::{ShownFlash, VictimSystems, VictimView};

/// The frame's light (scale, add) for a flash (module doc).
pub fn flash_light(f: Option<&ShownFlash>) -> (Vec3, Vec3) {
    let Some(f) = f else { return (Vec3::ONE, Vec3::ZERO) };
    let c = Vec3::new(f.color[0], f.color[1], f.color[2]);
    let i = f.intensity.clamp(0.0, 1.0);
    match f.kind {
        1 | 3 => (Vec3::ONE, c * i),
        2 | 4 => (Vec3::ONE - (Vec3::ONE - c) * i, Vec3::ZERO),
        5 => (Vec3::ONE - c * (2.0 * i), c * i),
        6 => (Vec3::splat(1.0 - i), c * i),
        _ => (Vec3::ONE, Vec3::ZERO),
    }
}

fn show(view: Res<VictimView>, post: Option<ResMut<ScreenPost>>, effects: Option<ResMut<ViewEffects>>, mut written: Local<Option<(Vec3, Vec3)>>) {
    if let Some(mut post) = post {
        let (scale, add) = flash_light(view.flash.as_ref());
        // Written when ours changes (another game's light stays).
        if *written != Some((scale, add)) {
            *written = Some((scale, add));
            post.light_scale = scale;
            post.light_add = add;
        }
    }
    if let Some(mut e) = effects {
        let want = ViewEffects { rotation: view.rotation, offset: view.offset, ..*e };
        e.set_if_neq(want);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<VictimView>().add_systems(Update, show.after(VictimSystems));
}
