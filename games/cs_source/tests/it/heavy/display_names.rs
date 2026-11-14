//! Every CS:S gun and grenade a drawn ladder can deal has a short name of
//! 16 characters or fewer (`chimera_sim::weapon::names::SHORT_MAX`): its
//! full name from the install's strings (`weapons::display_names`), its
//! short one from data/display_names.toml. A gun that fails needs an entry
//! there. `--nocapture` lists every gun's names. Skips without an install.

use std::collections::BTreeMap;

use ::cs_source::{
    game::{INFO, mount, parts::PARTS, weapons},
    harness::CsSim,
};
use chimera_sim::{minigame::auto::drawable_short_names, weapon::names::SHORT_MAX};
use chimera_world::harness::Sim;

#[test]
fn every_drawable_guns_short_name_fits() {
    let Some(install) = INFO.install() else {
        eprintln!("skipping: no CS:S install configured (or asset store serving it)");
        return;
    };
    let Ok(m) = mount::open(&install) else {
        eprintln!("skipping: the CS:S install doesn't mount");
        return;
    };
    let names: BTreeMap<String, String> = weapons::display_names(&m).into_iter().collect();
    assert!(!names.is_empty(), "the install's strings name CS:S's guns");
    // CS:S's guns as a party app lends them (`CsGunsPlugin`: the data and
    // its display names).
    let mut sim = Sim::with(CsSim, |app| {
        app.add_plugins(weapons::CsGunsPlugin);
    });
    // The catalogue is made as the app starts.
    sim.app.update();
    let rows = drawable_short_names(sim.app.world(), &PARTS, &names);
    assert!(rows.len() >= 20, "CS:S's guns are drawable: {}", rows.len());
    let mut long = Vec::new();
    for r in &rows {
        eprintln!("{} | {} | {} ({})", r.id, r.full, r.short, r.from.name());
        if r.short.chars().count() > SHORT_MAX {
            long.push(format!("{}: {} -> {}", r.id, r.full, r.short));
        }
    }
    assert!(
        long.is_empty(),
        "short names over {SHORT_MAX} characters: give them a `short` in games/cs_source/data/display_names.toml\n{}",
        long.join("\n")
    );
}
