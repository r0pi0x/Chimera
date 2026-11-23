//! A sweep of several maps in one process (`map_fitness`'s loop: a map
//! loaded, a `Sim` made on it, the next map loaded) finds the install for
//! every map. `harness::Sim` turns the machine's installs off for a fast
//! test's process (`mount::offline`); on a tool's main thread it leaves
//! them on, so the second map loads as the first did. The sweep runs on a
//! thread named `main`, as a tool's does (this test's own thread is a
//! heavy one, which allows installs anyway). Under nextest (a process per
//! test) the guard starts unset, so a regression shows; in a shared
//! `cargo test` process another heavy test may have allowed installs
//! first. Skipped without a DoD:S install.

use chimera_world::{
    harness::Sim,
    map::{KitLoad, LoadContext, MapLoadOptions, MapSimPlugin},
};
use combat_arms::harness::CaSim;

const MAPS: [&str; 2] = ["dods:dod_jagd", "dods:dod_flash"];

#[test]
fn a_sweep_finds_the_install_for_each_map() {
    if dods::game::INFO.install().is_none() {
        eprintln!("skipping: no DoD:S install");
        return;
    }
    let sweep = std::thread::Builder::new()
        .name("main".into())
        .spawn(|| {
            let context = LoadContext::default();
            let options = MapLoadOptions { kit: KitLoad::Only(&[]), ..MapLoadOptions::lean().in_context(&context) };
            MAPS.iter()
                .map(|id| {
                    let data = party_roster::load_map_with(id, &options).map_err(|e| format!("{id}: {e}"))?;
                    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), party_roster::PartyPartsPlugin));
                    sim.ticks(1);
                    Ok(id.to_string())
                })
                .collect::<Vec<Result<String, String>>>()
        })
        .unwrap();
    let loaded = sweep.join().expect("the sweep runs");
    for r in &loaded {
        assert!(r.is_ok(), "every map of the sweep loads from the install: {loaded:?}");
    }
}
