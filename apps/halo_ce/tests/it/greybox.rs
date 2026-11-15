//! The app's simulation on the greybox (no install): a player on Halo's
//! movement stands and walks; the app's static is well formed; a
//! named map fails with the loader's message instead of crashing.

use bevy::prelude::*;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::MapLoadOptions,
};
use halo_ce::game::movement;

#[test]
fn a_player_walks_the_greybox() {
    let mut sim = Sim::new(halo_ce_app::SimPlugin, GreyboxMapPlugin);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    assert!(sim.state(p).on_ground, "stands on the greybox");
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    assert!(sim.position(p).distance(start) > 1.0, "walks");
}

#[test]
fn the_static_is_well_formed() {
    let app = &halo_ce_app::APP;
    assert_eq!(app.bin, "halo_ce");
    assert!(std::ptr::eq(app.info, &halo_ce::game::INFO));
    assert!((app.about)().contains("halo_ce:"));
    assert_eq!(app.hooks.server, Some("halo_ce_server"));
}

#[test]
fn a_named_map_fails_cleanly_when_it_cannot_load() {
    // Another game's map is refused with a message; Halo's own may load once
    // its loader and a disc exist, but never panics.
    let _ = halo_ce_app::load_map_with("halo_ce:hangemhigh", &MapLoadOptions::default());
    assert!(halo_ce_app::load_map_with("cs_source:de_dust2", &MapLoadOptions::default()).is_err());
}
