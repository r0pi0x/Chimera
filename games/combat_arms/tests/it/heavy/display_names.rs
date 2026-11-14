//! Every Combat Arms gun and grenade a drawn ladder can deal has a short
//! name of 16 characters or fewer (`chimera_sim::weapon::names::SHORT_MAX`):
//! its full name from the install's string table (`ui::install::
//! weapon_names`, else its record's name as the id), its short one from
//! data/display_names.toml (an override, else the automatic split). A gun
//! that fails needs an entry there: a new install's guns show up here.
//! `--nocapture` lists every gun's names and where the short one came from.
//! Needs the V1 key and the `ui` feature (the string table); skips with a
//! note without.

use std::collections::BTreeMap;

use chimera_core::mount::config::LocalConfig;
use chimera_sim::{
    minigame::auto::drawable_short_names,
    weapon::names::{NameSource, SHORT_MAX},
};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{GAME, INFO, data::CaData, parts::PARTS},
    harness::CaSimWith,
};

/// The install's weapon names by id (the `ui` feature's string table).
fn install_names() -> Option<BTreeMap<String, String>> {
    let install = LocalConfig::load().ok().and_then(|c| c.game_path(GAME))?;
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return None;
    }
    let mount = (INFO.open)(&install).ok()?;
    #[cfg(feature = "ui")]
    {
        use combat_arms::ui::install;
        let strings = install::strings(&mount, &install::language())?;
        return Some(install::weapon_names(&mount, &strings).into_iter().collect());
    }
    #[allow(unreachable_code)]
    {
        let _ = mount;
        None
    }
}

#[test]
fn every_drawable_guns_short_name_fits() {
    let Some(names) = install_names() else {
        eprintln!("skipping: no Combat Arms install, or no string table (the `ui` feature)");
        return;
    };
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let sim = Sim::with(CaSimWith(ca), |_| {});
    let rows = drawable_short_names(sim.app.world(), &PARTS, &names);
    assert!(rows.len() > 1000, "the install's guns are drawable: {}", rows.len());
    let mut long = Vec::new();
    let mut by = [0usize; 3];
    for r in &rows {
        eprintln!("{} | {} | {} ({})", r.id, r.full, r.short, r.from.name());
        by[r.from as usize] += 1;
        if r.short.chars().count() > SHORT_MAX {
            long.push(format!("{}: {} -> {}", r.id, r.full, r.short));
        }
    }
    eprintln!("{} guns: {} overridden, {} split, {} as they are", rows.len(), by[0], by[1], by[2]);
    // The split's samples (docs/plans/active/gun-display-names.md).
    let short = |id: &str| rows.iter().find(|r| r.id == id).map(|r| (r.short.as_str(), r.from));
    assert_eq!(short("combat_arms:double_barreled_ar-15_godra_mythic"), Some(("Ra AR-15 DB", NameSource::Override)));
    assert!(
        long.is_empty(),
        "short names over {SHORT_MAX} characters: give them a `short` in games/combat_arms/data/display_names.toml\n{}",
        long.join("\n")
    );
}
