//! The install's gun families (`game::data::families`; docs/plans/active/
//! guns-picker-v2.md, step 1): its skins, editions and levels are far fewer
//! guns, grouped by their first-person model, never across classes. Needs
//! the V1 key; skips with a note without. `--nocapture` prints the counts.

use chimera_core::mount::config::LocalConfig;
use chimera_sim::{minigame::auto, weapon::data::WeaponCatalog};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{GAME, data::CaData},
    harness::CaSimWith,
};

#[test]
fn the_installs_skins_fold_into_families_by_model() {
    if LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_none() {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    }
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let sim = Sim::with(CaSimWith(ca), |_| {});
    let catalog = sim.app.world().resource::<WeaponCatalog>();
    let weapons = catalog.weapons();
    let classed: Vec<_> = weapons.iter().filter(|d| d.id.starts_with("combat_arms:") && d.class.is_some()).collect();
    let families: std::collections::BTreeSet<String> = classed.iter().map(|d| auto::family_of(catalog, &d.id)).collect();
    eprintln!("{} classed weapons in {} families", classed.len(), families.len());
    assert!(classed.len() > 1000, "the install's weapons: {}", classed.len());
    assert!(families.len() * 2 < classed.len(), "skins fold: {} families", families.len());
    // Each family is of one class.
    let mut class_of = std::collections::BTreeMap::new();
    for d in &classed {
        let f = auto::family_of(catalog, &d.id);
        let c = d.class.unwrap();
        assert_eq!(*class_of.entry(f.clone()).or_insert(c), c, "{f} spans classes");
    }
    // The AK-47's skins are one family.
    let ak = auto::family_of(catalog, "combat_arms:ak-47");
    let skins = classed.iter().filter(|d| auto::family_of(catalog, &d.id) == ak).count();
    eprintln!("{ak}: {skins} weapons");
    assert!(skins > 5, "{ak}: {skins}");
}
