//! Bodies on the code-built maps (the sandbox's range, the greybox): their
//! kit is a map of game `chimera` (`map::sources::CODE_MAP_KIT`) that the
//! app's `MapSources` dresses like any other game's map, spawned beside
//! the code-built geometry. On the range with a kit dressed with Combat
//! Arms' and CS:S's bodies (a stand-in for the guests' dressing, which reads the
//! installs: `heavy::range_kit`), a player has one drawn body with meshes,
//! the range's own parts stay (its layout, one sun: the kit brings none)
//! and the party's own `MAP_SOURCES` has the kit's loader.

use bevy::prelude::*;
use chimera_present::render::bodies::CharacterBody;
use chimera_sim::minigame;
use chimera_world::{
    harness::Sim,
    map::{
        MapData, MapDebugView, MapLoadOptions, MapPart, change_map,
        sources::{CODE_MAP_KIT, MapDressing, MapSources},
    },
    range::{RangeLayout, RangeMapPlugin, RangePart},
};
use combat_arms::harness::CaSim;
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

use crate::{
    bodies::{character, render_assets},
    play::{place, soldier},
};

fn two_bodies(map: &mut MapData, _: &MapLoadOptions) -> Result<(), String> {
    map.kit.characters.extend([character("combat_arms"), character("cs_source")]);
    Ok(())
}

static BODIES: MapDressing = MapDressing { game: "combat_arms", dress: two_bodies };
static SOURCES: MapSources = MapSources {
    loaders: &[&CODE_MAP_KIT],
    gameplay: Some(&BODIES),
    teams: &[],
    surfaces: &[],
    kept: &[],
    players: true,
    guests: &[],
};

#[test]
fn a_player_on_the_range_has_a_drawn_body() {
    let mut sim = Sim::new(CaSim, (render_assets, chimera_present::MapPlugin::empty(), PartyPartsPlugin, RangeMapPlugin));
    sim.ticks(1);
    let kit = SOURCES.load_with("chimera:range", &MapLoadOptions::default()).expect("the range's kit");
    assert_eq!(kit.kit.characters.len(), 2, "dressed with the bodies");
    assert!(kit.entity_scale > 0.0, "the kit's entity space is real (metres)");
    change_map(sim.app.world_mut(), kit, MapDebugView::Normal);
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let p = soldier(&mut sim);
    let layout = sim.app.world().resource::<RangeLayout>().clone();
    place(&mut sim, p, layout.at_distance(10.0) + Vec3::Y);
    sim.seconds(1.0);
    let w = sim.app.world_mut();
    assert!(w.contains_resource::<RangeLayout>(), "the range stays beside its kit");
    let suns = w.query_filtered::<(), (With<DirectionalLight>, With<MapPart>)>().iter(w).count();
    assert_eq!(suns, 0, "the kit brings no sun of its own");
    assert!(w.query_filtered::<(), (With<DirectionalLight>, With<RangePart>)>().iter(w).count() == 1);
    let kids: Vec<Entity> = w.get::<Children>(p).map(|c| c.to_vec()).unwrap_or_default();
    let bodies: Vec<Entity> = kids.into_iter().filter(|c| w.get::<CharacterBody>(*c).is_some()).collect();
    assert_eq!(bodies.len(), 1, "one body");
    let meshes = w
        .get::<Children>(bodies[0])
        .map(|c| c.iter().filter(|m| w.get::<Mesh3d>(*m).is_some()).count())
        .unwrap_or(0);
    assert!(meshes > 0, "the body has drawn meshes");
}

#[test]
fn the_partys_maps_dress_the_code_built_maps() {
    // Their kits loaded from the installs: `heavy::range_kit`.
    assert!(party_roster::MAP_SOURCES.loader("chimera").is_some());
    assert!(party_roster::load_map_with("chimera:nowhere", &MapLoadOptions::lean()).is_err());
}
