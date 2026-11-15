//! Halo's zoom on the party apps' world camera (`game::view::party_zoom_fov`,
//! the view's module doc "Zoom in the party apps"): while the local player
//! looks through a Halo gun's zoom (its `weapon::Zoomed`, whose field of
//! view is Halo's absolute angle), the first-person camera's field of view
//! is the player's own (`video::PlayerFov`) narrowed by Halo's
//! magnification, set after the shell's `zoom_camera` (which put Halo's
//! absolute angle there) with the same screen effects added. Unzoomed, or
//! with another game's gun, the shell's camera is untouched. Run only by
//! the party's front end (`ui::party_app`): Halo's own app keeps Halo's
//! angles.
//!
//! The mouse while zoomed is the engine's rule for a game's own zoom
//! scale: Halo's `Zoomed::sensitivity` (one over the zoom's nominal
//! magnification), as Combat Arms' scopes give theirs, not the FOV rule
//! (`input::MouseSettings::zoomed_scale`); it does not depend on the
//! player's field of view.
//!
//! Known shortcuts: spectating a Halo player in first person shows the
//! target's zoom at Halo's absolute angle (the shell's rule for every
//! game's scope).

use bevy::prelude::*;
use chimera_client::client::{FirstPersonCamera, screen_post::ViewEffects, spectate::SpecView, video::PlayerFov};
use chimera_core::core::LocalPlayer;
use chimera_sim::weapon::{Inventory, Zoomed};

use crate::game::{view::party_zoom_fov, weapons::HaloGun};

/// The world camera's vertical field of view (radians) for the player's
/// `player` and Halo's zoomed `zoomed` (degrees horizontal at 4:3), plus
/// the screen effects' `extra` degrees.
pub fn camera_fov(player: f32, zoomed: f32, extra: f32) -> f32 {
    (chimera_world::map::view_model::vertical_fov(party_zoom_fov(player, zoomed)) + extra).to_radians()
}

/// The local player's Halo zoom on the first-person camera (module doc).
#[allow(clippy::type_complexity)]
pub fn zoom(
    player: Option<Single<(&Inventory, Option<&Zoomed>), With<LocalPlayer>>>,
    guns: Query<(), With<HaloGun>>,
    spectating: Option<Res<SpecView>>,
    player_fov: Option<Res<PlayerFov>>,
    effects: Option<Res<ViewEffects>>,
    mut cameras: Query<&mut Projection, With<FirstPersonCamera>>,
) {
    if spectating.is_some_and(|s| s.pose.is_some()) {
        return;
    }
    let Some((inv, Some(z))) = player.map(|p| p.into_inner()) else { return };
    if !inv.active.is_some_and(|a| guns.contains(a)) {
        return;
    }
    let fov = camera_fov(player_fov.map_or(90.0, |f| f.0), z.fov, effects.map_or(0.0, |e| e.fov));
    for mut projection in &mut cameras {
        if let Projection::Perspective(p) = projection.as_ref()
            && (p.fov - fov).abs() > 1e-6
            && let Projection::Perspective(p) = projection.as_mut()
        {
            p.fov = fov;
        }
    }
}

/// Adds `zoom` after the shell's camera zoom.
pub fn plugin(app: &mut App) {
    app.add_systems(Update, zoom.after(chimera_client::client::zoom_camera));
}
