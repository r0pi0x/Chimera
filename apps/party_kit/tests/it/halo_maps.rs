//! Halo in the party, without its disc image (docs/plans/active/
//! halo-ce.md, M1): the party composes it for its maps only (its game
//! registered, its surface classes, its pool rules), claims none of its
//! maps (so a machine without the disc is offered none: the pool is the
//! installed maps its rules allow), and its maps' surface names are
//! Combat Arms' through the composed table. With the disc:
//! `heavy::halo_maps`.

use chimera_sim::minigame::auto::{self, InstalledMaps};
use chimera_world::{harness::Sim, map::impact::SurfaceClasses};
use combat_arms::{game::effects::data::CaFxTables, harness::CaSim};
use party_kit::PartyPartsPlugin;

#[test]
fn halo_is_composed_for_its_maps_only() {
    let sim = Sim::new(CaSim, PartyPartsPlugin);
    let w = sim.app.world();
    assert!(w.resource::<chimera_core::game_registry::Games>().get(halo_ce::game::GAME).is_some());
    assert!(w.resource::<SurfaceClasses>().of(halo_ce::game::GAME).is_some(), "its surfaces' classes");
    let report = auto::report(w);
    let halo = report.iter().find(|c| c.game == halo_ce::game::GAME).expect("Halo reports its parts");
    assert!(halo.guns.is_empty() && halo.movement.is_none() && !halo.playable(), "{}", halo.line());
    assert!(halo.maps.is_empty(), "no map claimed without the disc: {}", halo.line());
    // The other games still lend what they did.
    for c in report.iter().filter(|c| c.game != halo_ce::game::GAME) {
        assert!(c.playable(), "{}", c.line());
    }
}

#[test]
fn the_disc_images_maps_join_by_halos_pool_rules() {
    let mut sim = Sim::new(CaSim, PartyPartsPlugin);
    let all: Vec<String> = halo_ce::game::MULTIPLAYER_MAPS.iter().map(|m| m.to_string()).collect();
    sim.app.world_mut().insert_resource(InstalledMaps(vec![(halo_ce::game::GAME.into(), all)]));
    let report = auto::report(sim.app.world());
    let maps = &report.iter().find(|c| c.game == halo_ce::game::GAME).unwrap().maps;
    for must in ["halo_ce:hangemhigh", "halo_ce:bloodgulch"] {
        assert!(maps.iter().any(|m| m == must), "{must}: {maps:?}");
    }
    assert!(maps.len() < halo_ce::game::MULTIPLAYER_MAPS.len(), "some are kept out: {maps:?}");
    // The pool a gun game is made with has them only where every player
    // has the disc (here: no reports, so every game counts).
    let pool = auto::map_pool(&report, &|g| g != halo_ce::game::GAME);
    assert!(!pool.iter().any(|m| m.starts_with("halo_ce:")), "{pool:?}");
}

/// Every surface name Halo's maps use (its material types' generic names,
/// games/halo_ce/data/surfaces.toml) is named in the composed table, as a
/// surface type Combat Arms' impact table knows.
#[test]
fn halos_surfaces_are_combat_arms_through_the_table() {
    let table = party_kit::SURFACES
        .iter()
        .find(|t| t.map == halo_ce::game::GAME && t.gameplay == combat_arms::game::GAME)
        .expect("a table for Halo's maps");
    let rows = table.rows().unwrap();
    let ca = CaFxTables::built_in();
    for row in halo_ce::game::maps::surfaces::table().values() {
        let name = row.surface.as_str();
        let to = rows.names.get(name).unwrap_or_else(|| panic!("`{name}` (Halo's {}) isn't named", row.name));
        assert!(ca.surface_id(to).is_some(), "`{name}` as `{to}`: not one of Combat Arms' surface types");
    }
}
