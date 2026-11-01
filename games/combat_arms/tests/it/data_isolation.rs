//! Two apps in one process keep their own data (architecture review E2,
//! X3): the weapon sets, and everything else `CaData` holds, are resources
//! of the app, not process-wide caches, so a test (or a server hosting two
//! matches) that loads other weapon data doesn't change what any other app
//! plays.

use bevy::prelude::*;
use chimera_sim::weapon::{self, data::WeaponCatalog};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        data::{CaData, WEAPON_DATA},
        movement,
        weapons::{CaDamage, DAMAGE, DamageParams},
    },
    harness::{CaSim, CaSimWith},
};

const M4A1: &str = "combat_arms:m4a1";

/// The built-in documents with the M4A1's damage per ray doubled (35 to
/// 70): data an install-merged or minigame-tuned set would differ by.
fn tuned() -> CaData {
    let docs = WEAPON_DATA
        .iter()
        .map(|(name, text)| {
            let text = if *name == "m4a1.toml" {
                assert!(text.contains("damage = 35.0"));
                text.replacen("damage = 35.0", "damage = 70.0", 1)
            } else {
                text.to_string()
            };
            (name.to_string(), text)
        })
        .collect();
    CaData::builtin_with_weapons(docs)
}

/// The M4A1's damage per ray in the data `sim` registered.
fn catalog_damage(sim: &Sim) -> f32 {
    let d = sim.app.world().resource::<WeaponCatalog>().get(M4A1).unwrap();
    d.hook::<DamageParams>(DAMAGE).unwrap().damage
}

/// The M4A1 a soldier of `sim` holds: its damage per ray, in health.
fn held_damage(sim: &mut Sim) -> f32 {
    let p = sim.spawn_character(Vec3::new(0.0, 2.0, 0.0), movement::ID);
    let gun = weapon::give(sim.app.world_mut(), p, M4A1).expect("the M4A1 is registered");
    sim.app.world().get::<CaDamage>(gun).unwrap().damage
}

#[test]
fn two_sims_with_different_weapon_data_dont_see_each_others() {
    let mut plain = Sim::new(CaSim, GreyboxMapPlugin);
    // Built after the first: nothing it loads reaches the one before.
    let mut other = Sim::new(CaSimWith(tuned()), GreyboxMapPlugin);

    assert_eq!(catalog_damage(&plain), 35.0, "built-in data");
    assert_eq!(catalog_damage(&other), 70.0, "tuned data");
    assert!((held_damage(&mut plain) - 0.35).abs() < 1e-6);
    assert!((held_damage(&mut other) - 0.70).abs() < 1e-6);

    // Running either doesn't change the other, nor the shared built-in data.
    plain.ticks(5);
    other.ticks(5);
    assert_eq!(catalog_damage(&plain), 35.0);
    assert_eq!(catalog_damage(&other), 70.0);
    let builtin = CaData::builtin().weapon_set().get(M4A1).unwrap().hook::<DamageParams>(DAMAGE).unwrap().damage;
    assert_eq!(builtin, 35.0);

    // Each app's data is its own resource, with its own set.
    let (a, b) = (plain.app.world().resource::<CaData>().weapon_set(), other.app.world().resource::<CaData>().weapon_set());
    assert!(!std::ptr::eq(a, b));
    assert!(std::ptr::eq(a, CaData::builtin().weapon_set()), "plain Sims share the immutable built-in set");
}

/// What the old switch did: a Sim on the built-in data and another on the
/// install's (or on any other `CaData`) in one process.
#[test]
fn a_builtin_app_and_an_install_app_coexist() {
    let builtin = Sim::new(CaSim, GreyboxMapPlugin);
    let install = Sim::new(CaSimWith(CaData::install()), GreyboxMapPlugin);
    assert!(builtin.app.world().resource::<CaData>().builtin_only());
    assert!(!install.app.world().resource::<CaData>().builtin_only());
    // The install's attribute data (when this machine has one) never
    // reaches the built-in app.
    assert!(builtin.app.world().resource::<CaData>().attributes().is_none());
}

/// What map loads work out (the models' rigs, groups and held sockets, the
/// effect library) and what an app loads after (its late models) are the
/// app's, not the process's (once statics: docs/plans/active/
/// ecs-cleanup.md, item 13): two apps in one process each see their own,
/// a map's go with it when it is unloaded, and each app's map loads get
/// its own context, which holds the one install data both share.
#[test]
fn two_apps_keep_their_own_loaded_models_and_effects() {
    use std::sync::Arc;

    use chimera_world::map::{self, LoadContext, MapData, MapSimPlugin};
    use combat_arms::game::{
        characters::models::{self, CaModelIndex},
        effects::library::{self, FxLibrary},
    };

    // A map whose load registered a held model's socket and an effect
    // library, as Combat Arms' loader does (`MapData::memos`).
    let mut loaded = MapData::default();
    loaded.memos.entry::<CaModelIndex>().register_held_socket("combat_arms:law", "Flash", "Right_hand", Vec3::X);
    library::set_library(&mut loaded.memos, Some(Arc::new(FxLibrary::default())));
    let mut a = Sim::with(CaSim, move |app| {
        app.add_plugins(MapSimPlugin::new(loaded));
    });
    let mut b = Sim::with(CaSim, |app| {
        app.add_plugins(MapSimPlugin::new(MapData::default()));
    });
    a.ticks(1);
    b.ticks(1);
    let socket = |sim: &Sim| models::models_in(sim.app.world()).held_socket("combat_arms:law", "flash");
    assert!(socket(&a).is_some(), "a's map registered it");
    assert!(socket(&b).is_none(), "b's map didn't");
    assert!(library::library_in(a.app.world()).is_some() && library::library_in(b.app.world()).is_none());

    // What an app loads later (on demand) is its own too, and stays across
    // map changes.
    models::late_in(b.app.world_mut()).0.register_held_socket("combat_arms:m4a1", "Flash", "Right_hand", Vec3::Y);
    let late = |sim: &Sim| models::models_in(sim.app.world()).held_socket("combat_arms:m4a1", "flash");
    assert!(late(&b).is_some() && late(&a).is_none());
    map::unload_map(a.app.world_mut());
    map::unload_map(b.app.world_mut());
    assert!(socket(&a).is_none() && library::library_in(a.app.world()).is_none(), "gone with a's map");
    assert!(late(&b).is_some(), "b's late model stays");

    // Each app's loads get its own context; the install data in them is
    // the one both share while they hold it.
    let (ca, cb) = (LoadContext::of(a.app.world_mut()), LoadContext::of(b.app.world_mut()));
    assert_ne!(ca, cb, "one context per app");
    assert!(CaData::install_for(&ca).same(&CaData::install_for(&cb)), "one install data");
    assert!(CaData::install_for(&ca).same(&CaData::install()), "the process's while an app holds it");
}
