//! TF2's maps in an automatic pool (docs/plans/active/tf2.md, M4a): TF2
//! claims no map, so a party where TF2 isn't installed (nor in the store)
//! is offered none; where it is, the installed maps its data/
//! map_pool.toml allows join (the 14 chosen for a free-for-all), every
//! other one (payload, arena, holiday twins) stays out, and each allowed
//! one has measurements (data/map_fitness.toml). With the install:
//! `heavy::tf2_maps`.

use chimera_sim::minigame::{
    auto::{self, InstalledMaps},
    map_fit::MapPool,
};
use chimera_world::harness::Sim;
use combat_arms::harness::CaSim;
use party_roster::PartyPartsPlugin;

/// The maps the pool allows (data/map_pool.toml's allow lists).
const ALLOWED: &[&str] = &[
    "ctf_2fort",
    "cp_badlands",
    "koth_viaduct",
    "koth_harvest_final",
    "koth_nucleus",
    "cp_granary",
    "ctf_turbine",
    "ctf_doublecross",
    "koth_lakeside_final",
    "cp_process_final",
    "koth_king",
    "koth_highpass",
    "koth_sharkbay",
    "koth_suijin",
];

#[test]
fn tf2_claims_no_map_and_its_rules_pick_the_installed_ones() {
    let mut sim = Sim::new(CaSim, PartyPartsPlugin);
    let report = auto::report(sim.app.world());
    let tf2 = report.iter().find(|c| c.game == tf2::game::GAME).expect("TF2 reports its parts");
    assert!(tf2.maps.is_empty(), "no map without the install: {}", tf2.line());

    let mut installed: Vec<String> = ALLOWED.iter().map(|m| m.to_string()).collect();
    installed.extend(["pl_badwater", "arena_well", "koth_viaduct_event", "cp_well", "mvm_decoy", "ctf_2fort_invasion"].map(String::from));
    sim.app.world_mut().insert_resource(InstalledMaps(vec![(tf2::game::GAME.into(), installed)]));
    let report = auto::report(sim.app.world());
    let mut maps = report.iter().find(|c| c.game == tf2::game::GAME).unwrap().maps.clone();
    maps.sort();
    let mut want: Vec<String> = ALLOWED.iter().map(|m| format!("tf2:{m}")).collect();
    want.sort();
    assert_eq!(maps, want);

    let pool = sim.app.world().resource::<MapPool>();
    for m in ALLOWED {
        assert!(pool.fitness(&format!("tf2:{m}")).is_some(), "{m}: measured");
    }
}
