//! Every HL2:DM gun and grenade a drawn ladder can deal has a short name
//! of 16 characters or fewer (`chimera_sim::weapon::names::SHORT_MAX`):
//! its full name from the install's strings (`weapons::display_names`),
//! its short one from data/display_names.toml. A gun that fails needs an
//! entry there. `--nocapture` lists every gun's names. Skips without an
//! install.

use std::collections::BTreeMap;

use chimera_sim::{minigame::auto::drawable_short_names, weapon::names::SHORT_MAX};
use chimera_world::harness::Sim;
use hl2dm::{
    game::{parts::PARTS, weapons},
    harness::Hl2Sim,
};

use super::map_stock::install_mount;

#[test]
fn every_drawable_guns_short_name_fits() {
    let Some(m) = install_mount() else { return };
    let names: BTreeMap<String, String> = weapons::display_names(&m).into_iter().collect();
    assert!(!names.is_empty(), "the install's strings name HL2:DM's guns");
    let mut sim = Sim::with(Hl2Sim, |_| {});
    // The catalogue is made as the app starts.
    sim.app.update();
    let rows = drawable_short_names(sim.app.world(), &PARTS, &names);
    assert!(!rows.is_empty(), "HL2:DM's guns are drawable");
    let mut long = Vec::new();
    for r in &rows {
        eprintln!("{} | {} | {} ({})", r.id, r.full, r.short, r.from.name());
        if r.short.chars().count() > SHORT_MAX {
            long.push(format!("{}: {} -> {}", r.id, r.full, r.short));
        }
    }
    assert!(
        long.is_empty(),
        "short names over {SHORT_MAX} characters: give them a `short` in games/hl2dm/data/display_names.toml\n{}",
        long.join("\n")
    );
}
