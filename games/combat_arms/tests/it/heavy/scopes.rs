//! Every install weapon whose record names a default sight
//! (`DefaultScopeAttach`, specs/combat_arms/attachments.md "Defaults in the
//! weapons file"; weapon_classes.md section 7: the snipers) gets its scope
//! in the simulation built on the install's data: the weapon document
//! merged from the record carries the sight's name, fitting resolves it in
//! the install's modification records, and the zoom has the sight's levels;
//! the special weapons too (weapon_classes.md Corrections (2026-10-30) 2:
//! they zoom through their scope part like guns).
//! Passes with a note without an install or without the V1 key.

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_sim::weapon;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        GAME, TICK_INTERVAL,
        data::CaData,
        movement,
        weapons::zoom::{CaZoom, Scope},
    },
    harness::CaSimWith,
};

/// Hooks that make a weapon a special one (special_weapons.md).
const SPECIAL_HOOKS: &[&str] = &[
    "combat_arms:projectile",
    "combat_arms:flame",
    "combat_arms:device",
    "combat_arms:bolt",
    "combat_arms:controller",
];

/// The weapons weapon_classes.md names as scoped (section 7, "Shipped
/// scopes" and the table's scope row): install `Name`, sight.
const SPEC_SCOPED: &[(&str, &str)] = &[
    ("L96A1", "Sniper Scope"),
    ("M24", "Sniper Scope"),
    ("Dragunov_SVD", "Dragunov Scope"),
    ("M107CQ", "Barrett Scope"),
    ("ASW338LM", "Barrett Scope"),
];

fn install_data() -> Option<CaData> {
    let installed = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if !installed {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    }
    let data = CaData::install();
    if data.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data can't be read (no V1 key)");
        return None;
    }
    Some(data)
}

/// The zoom's scope of each of `ids`, once each is given to a player on
/// `data` and fitted.
fn scopes_of(data: &CaData, ids: &[&str]) -> Vec<Option<Scope>> {
    let mut sim = Sim::with(CaSimWith(data.clone()), |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.seconds(0.5);
    let ws: Vec<Entity> =
        ids.iter().map(|id| weapon::give(sim.app.world_mut(), p, id).unwrap_or_else(|| panic!("no weapon {id}"))).collect();
    sim.seconds(1.0);
    ws.iter().map(|w| sim.app.world().get::<CaZoom>(*w).and_then(|z| z.scope.clone())).collect()
}

#[test]
fn every_install_weapon_with_a_default_scope_has_it() {
    let Some(data) = install_data() else { return };
    let weapons = &data.attributes().unwrap().weapons.as_ref().unwrap().weapons;
    let mods = data.attributes().unwrap().mods.as_ref().unwrap();
    let sights = data.attachments();
    let set = data.weapon_set();
    let (mut ids, mut wants, mut names) = (Vec::new(), Vec::new(), Vec::new());
    let (mut no_document, mut unknown_sight, mut not_listed, mut specials) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (index, w) in weapons.iter().enumerate().filter(|(_, w)| !w.default_scope_attach.is_empty()) {
        // A record naming a sight the modification file lacks fits nothing
        // (the lookup fails): not a scope to expect.
        let Some(record) = sights.find(&w.default_scope_attach).and_then(|i| sights.get(i)) else {
            unknown_sight.push(format!("{} ({})", w.name, w.default_scope_attach));
            continue;
        };
        let Some(id) = data.weapon_for_name(&w.name) else {
            no_document.push(w.name.clone());
            continue;
        };
        // Fitting takes only what the weapon's accepted list names
        // ("Fitting" step 3), the record linked by position (Q13): some
        // event weapons default to a sight they don't list.
        let listed = mods.weapons.get(index).is_some_and(|r| r.accepts.iter().any(|(n, _)| n.eq_ignore_ascii_case(&w.default_scope_attach)));
        if !listed {
            not_listed.push(w.name.clone());
            continue;
        }
        // The special weapons (the LGB, the napalm strike, the XM_25, the
        // Optical Buster) zoom through their scope like guns
        // (weapon_classes.md Corrections (2026-10-30) 2).
        let hooks = set.weapons.iter().find(|d| d.id == id).map(|d| &d.hooks);
        if hooks.is_some_and(|h| SPECIAL_HOOKS.iter().any(|k| h.contains_key(*k))) {
            specials.push(format!("{} ({})", w.name, w.default_scope_attach));
        }
        ids.push(id);
        wants.push(record.scope());
        names.push(format!("{} ({id}, sight \"{}\")", w.name, w.default_scope_attach));
    }
    eprintln!("not in their accepted list: {} {not_listed:?}", not_listed.len());
    eprintln!("{} special weapons with a scope: {specials:?}", specials.len());
    assert!(!specials.is_empty(), "no special weapon zooms through its scope");
    eprintln!(
        "{} install weapons with a default scope; without a document of ours: {no_document:?}; naming no sight record: {unknown_sight:?}",
        ids.len()
    );
    assert!(!ids.is_empty(), "no install weapon names a default sight");
    let gots = scopes_of(&data, &ids);
    let failures: Vec<String> = names
        .iter()
        .zip(gots.iter().zip(&wants))
        .filter(|(_, (got, want))| got.is_none() || got != want)
        .map(|(n, (got, want))| format!("{n}: got {got:?}, want {want:?}"))
        .collect();
    assert!(failures.is_empty(), "weapons whose default sight gave no scope in the sim:\n{}", failures.join("\n"));
}

#[test]
fn the_snipers_the_spec_names_are_scoped() {
    let Some(data) = install_data() else { return };
    let weapons = &data.attributes().unwrap().weapons.as_ref().unwrap().weapons;
    for (name, sight) in SPEC_SCOPED {
        let Some(record) = weapons.iter().find(|w| w.name.eq_ignore_ascii_case(name)) else {
            eprintln!("{name}: not in this install's data");
            continue;
        };
        assert!(record.default_scope_attach.eq_ignore_ascii_case(sight), "{name}: DefaultScopeAttach {:?}", record.default_scope_attach);
        let id = data.weapon_for_name(name).unwrap_or_else(|| panic!("{name}: no weapon document"));
        let scope = scopes_of(&data, &[id]).remove(0).unwrap_or_else(|| panic!("{name}: no scope"));
        assert!(scope.name.eq_ignore_ascii_case(sight) && !scope.fov.is_empty(), "{name}: {scope:?}");
    }
}
