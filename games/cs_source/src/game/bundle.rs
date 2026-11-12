//! CS:S as one bundle for an app made of several games (lift C4,
//! `chimera_world::game_bundle`; the party apps' composer, `party_kit`,
//! loops over its bundles): its info, parts and maps, its players, guns
//! and HUD look on other games' maps (`GUEST`), its own binds (Shift
//! walks, `+speed`; Ctrl ducks), its view models' field of view, its
//! parts in the composed simulation (`party_sim`: what the party took of
//! CS:S before); its HUD and front end on the client are its `ui`'s
//! `GameClient` (`party::CLIENT`, with the `ui` feature).

use bevy::prelude::*;
use chimera_world::game_bundle::GameBundle;

use super::{GAME, INFO, MAP_LOADER};

/// CS:S's bundle.
pub static BUNDLE: GameBundle = GameBundle {
    info: &INFO,
    parts: &super::parts::PARTS,
    loader: &MAP_LOADER,
    guest: Some(&GUEST),
    kept: &["cs_source:weapon_"],
    binds: &[("shift", "+speed"), ("ctrl", "+duck")],
    view_model_fov: Some(fov),
    sim: party_sim,
    display_names: Some(super::weapons::display_names),
};


/// CS:S's players, guns and HUD look on the maps of other games.
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing {
    game: GAME,
    dress: |map, options| super::dress_guest_with(map, BUNDLE.kept, !options.textures),
};

/// CS:S's view models' field of view (its `viewmodel_fov` and
/// `default_fov`).
pub fn fov() -> crate::map::GameFov {
    let s = super::view_anim::settings();
    crate::map::GameFov {
        game: GAME,
        fov: s.fov,
        default_fov: s.default_fov,
        follows_player: false,
    }
}

/// What CS:S adds to a game of several games: the game registered,
/// Source's map logic, its guns' view models by Chimera's driver with
/// CS:S's rules and its bob and sway (while the base game's own driver
/// has let go of the local player), its guns and their impacts (sounds,
/// decals, debris, blood, tracers), props' impact and scrape sounds, the
/// hit slowdown and view flinch of a CS:S-identity player, its grenades'
/// looks, its movement for a player holding a CS:S gun and a network
/// server's per-tick animation of its bodies (their hitboxes).
pub fn party_sim(app: &mut App) {
    use crate::game_registry::GameAppExt;
    app.register_game(&INFO);
    if !app.is_plugin_added::<source_logic::logic::LogicPlugin>() {
        app.add_plugins(source_logic::logic::LogicPlugin);
    }
    super::view_anim::add_rules(app);
    super::view_anim::add_motion(
        app,
        |own: Query<(), (With<crate::core::LocalPlayer>, With<crate::weapon::view_anim::OwnViewAnim>)>| own.is_empty(),
    );
    app.add_plugins((
        super::weapons::CsGunsPlugin,
        super::impacts::BulletImpactsPlugin,
        super::impacts::ImpactSoundsPlugin,
        super::hit_reaction::HitReactionPlugin,
        super::impact_effects::ImpactEffectsPlugin,
        super::grenades::GrenadeLooksPlugin,
        super::movement::SourceMovementPlugin,
        super::player_anim::SimAnimPlugin,
    ));
}
