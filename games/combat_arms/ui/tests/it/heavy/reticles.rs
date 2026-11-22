//! Every install weapon whose record names a `DotSightName` has its
//! reticle image in the install's UI art (hud_crosshair.md A8), so the
//! HUD can draw it. Through our readers, names only; skipped without an
//! install or the V1 key.

use combat_arms::game::data::CaData;
use combat_arms_ui::install;

#[test]
fn every_named_reticle_resolves() {
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: no Combat Arms install's weapons (no install, or no key)");
        return;
    }
    let Some(inst) = install::installed(&ca) else {
        eprintln!("skipping: the install's UI data is locked");
        return;
    };
    let names = install::dot_sight_names(&ca);
    eprintln!("reticles named by the install's weapons: {names:?}");
    // The weapons that name one (id, reticle), for the log.
    let file = ca.attributes().and_then(|a| a.weapons.as_ref()).unwrap();
    let named: Vec<(String, String)> = combat_arms::game::data::install_weapons_by_id(file)
        .iter()
        .filter(|(_, r)| !r.dot_sight_name.trim().is_empty())
        .map(|(id, r)| (id.to_string(), r.dot_sight_name.clone()))
        .collect();
    eprintln!("{} weapons name one", named.len());
    assert!(!named.is_empty());
    let missing: Vec<&String> = names.iter().filter(|n| !inst.art.contains_key(&install::texture_key(n))).collect();
    assert!(missing.is_empty(), "named reticles not found in the install: {missing:?}");
}
