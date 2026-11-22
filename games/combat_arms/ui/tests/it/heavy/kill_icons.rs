//! Every Combat Arms gun's kill icon from the user's install, the
//! install's copies too (`ironwood_m1866_lv.4` and the like: made from
//! the install's weapon list, named by their `Name`): each weapon
//! record's `Weaponkillimage` names its art (copies share their model's,
//! `kill_message_M1866.dtx`; some files are spelled otherwise or sit
//! outside `weapon/`), so the id's name alone missed most of them and the
//! shared kill feed drew their names instead. Through our readers, names
//! only; skipped without an install or the V1 key.

use std::sync::Arc;

use combat_arms::game::data::CaData;
use combat_arms_ui::{CaUi, draw::CaArt, hud::{BoundHud, icon_resolver}, install};

#[test]
fn every_gun_with_art_has_its_kill_icon() {
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: no Combat Arms install's weapons (no install, or no key)");
        return;
    }
    let Some(inst) = install::installed(&ca) else {
        eprintln!("skipping: the install's UI data is locked");
        return;
    };
    let ui = CaUi::with_install(inst.clone());
    let bound = BoundHud::of(&ui);
    assert!(!bound.kill_icons.is_empty(), "the weapons' icons reach the HUD");
    let art = CaArt(inst.art.keys().map(|k| (k.clone(), (Default::default(), inst.art[k].size))).collect());
    let resolve = icon_resolver(&art, Arc::clone(&bound.kill_icons));
    let set = ca.weapon_set();
    let (mut copies, mut with_art) = (0, 0);
    let mut missing = Vec::new();
    for w in &set.weapons {
        let copy = !ca.builtin_weapon_ids().contains(&w.id);
        copies += copy as usize;
        match resolve(&w.id) {
            Some(_) => with_art += 1,
            // A gun a ladder takes (a class) must have one.
            None if w.class.is_some() => missing.push(w.id.clone()),
            None => {}
        }
    }
    eprintln!("{} weapons ({copies} install copies): {with_art} with a kill icon", set.weapons.len());
    assert!(copies > 0, "the install's copies are in the set");
    assert!(missing.is_empty(), "guns without their icon: {missing:?}");
    // The capture's gun: the M1866's art.
    if set.get("combat_arms:ironwood_m1866_lv.4").is_some() {
        assert!(install::kill_art(&bound.kill_icons, "combat_arms:ironwood_m1866_lv.4").ends_with("/kill_message_m1866.dtx"));
        assert!(resolve("combat_arms:ironwood_m1866_lv.4").is_some());
    }
    // Ours keep theirs.
    assert!(resolve("combat_arms:m4a1").is_some());
}
