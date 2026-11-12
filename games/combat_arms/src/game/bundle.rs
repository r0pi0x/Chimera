//! Combat Arms as one bundle for an app made of several games (lift C4,
//! `chimera_world::game_bundle`): its info, parts and maps, its own binds
//! (Shift sprints, C walks: Alt is free look in Chimera), its view
//! models' field of view (its HUD on the client is its front end's
//! `GameClient`, `ui::PARTY_CLIENT`). The party apps play on it as their base game (its
//! plugins, its gameplay's dressing of every map, its bots), so its
//! `sim` adds nothing and it is no guest on other games' maps.

use chimera_world::game_bundle::GameBundle;

/// Combat Arms' bundle.
pub static BUNDLE: GameBundle = GameBundle {
    info: &super::INFO,
    parts: &super::parts::PARTS,
    loader: &super::world::MAP_LOADER,
    guest: None,
    kept: &[],
    binds: &[("shift", "+sprint"), ("c", "+walk")],
    view_model_fov: Some(fov),
    sim: |_| {},
    display_names: None,
};


/// Its view models' field of view (`ca_viewmodel_fov`'s default and
/// its default field of view).
pub fn fov() -> crate::map::GameFov {
    let s = super::view_model::settings();
    crate::map::GameFov {
        game: super::GAME,
        fov: s.fov,
        default_fov: s.default_fov,
        follows_player: false,
    }
}
