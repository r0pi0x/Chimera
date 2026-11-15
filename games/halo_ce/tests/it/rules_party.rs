//! Halo's game types stay out of a party app (`game::rules`): Halo as one
//! bundle of a composed simulation (`game::BUNDLE.sim`, what Ultimate Gun
//! Game and Lucker Party add for Halo's maps and guns) registers no Halo
//! mode and no match, so the party's own rules play on Halo's maps.

use bevy::prelude::*;
use chimera_sim::mode::Modes;
use halo_ce::game::{BUNDLE, rules::HaloMatch};

#[test]
fn a_composed_simulation_has_no_halo_rules() {
    let mut app = App::new();
    app.add_plugins(halo_ce::SimPlugins);
    (BUNDLE.sim)(&mut app);
    let halo_modes = app.world().get_resource::<Modes>().map_or(0, |m| m.all().filter(|(g, _)| *g == halo_ce::game::GAME).count());
    assert_eq!(halo_modes, 0);
    assert!(app.world().get_resource::<HaloMatch>().is_none());
}
