//! A party's minigame on Combat Arms' maps hides the maps' game-mode
//! objects (the user's installs): the gun game's rules aren't Combat
//! Arms', so no mode of its plays, and neither the flags' props
//! (`rules::ctf::show_flags`, whatever the gameplay) nor the modes' world
//! models (the map's start messages of a mode without objectives,
//! `rules::flow::other_rules_map_messages`: JUNK_FLEA's `QRT_Show_L`,
//! Quarantine's, which every other mode's start trigger hides) stay up.
//! The world models need the install's missions (the V1 key); without
//! them that part is noted and skipped. Passes with a note without the
//! Combat Arms install.

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_world::{
    harness::Sim,
    map::{MapBrushEntity, MapEntities, MapSimPlugin, PropEntity},
};
use combat_arms::{game::data::CaData, harness::CaSimWith};
use party_kit::{PartyPartsPlugin, server::swap_map};

/// The gun game with two players on WAREHOUSE, then on Combat Arms'
/// map `name` (with the install's data) by the server's map change, as a
/// party's rotation brings it, three seconds in; None (with a note)
/// without the install.
fn gun_game_on(name: &str) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    if config
        .game_path(combat_arms::game::GAME)
        .filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"))
        .is_none()
    {
        eprintln!("skipping: needs the Combat Arms install");
        return None;
    }
    let id = format!("combat_arms:{name}");
    let map = party_kit::load_map(&id, 0).ok()?;
    // A first map for the app to start on (the swap needs one: not the
    // greybox); its first frames run Combat Arms' rules.
    let first = party_kit::load_map("combat_arms:warehouse", 0).ok()?;
    let mut sim = Sim::new(
        CaSimWith(CaData::install()),
        (MapSimPlugin::new(first), PartyPartsPlugin),
    );
    sim.ticks(2);
    chimera_sim::minigame::start(sim.app.world_mut(), party_kit::DEFAULT_MINIGAME).expect("the gun game starts");
    for _ in 0..2 {
        sim.spawn_character(Vec3::Y, combat_arms::game::movement::ID);
    }
    sim.seconds(1.0);
    swap_map(sim.app.world_mut(), &id, Some(map));
    sim.seconds(3.0);
    eprintln!(
        "{name}: phase {:?}",
        sim.app.world().resource::<chimera_sim::mode::Match>().phase
    );
    Some(sim)
}

/// Each map entity's (classname, name) with how its prop or node is
/// shown.
fn shown(sim: &mut Sim) -> Vec<(String, String, bool)> {
    let w = sim.app.world_mut();
    let entities = w.resource::<MapEntities>().entities.clone();
    let mut out = Vec::new();
    let mut props = w.query::<(&PropEntity, &Visibility)>();
    let mut nodes = w.query::<(&MapBrushEntity, &Visibility)>();
    let rows: Vec<(usize, bool)> = props
        .iter(w)
        .map(|(e, v)| (e.0, *v != Visibility::Hidden))
        .chain(nodes.iter(w).map(|(e, v)| (e.0, *v != Visibility::Hidden)))
        .collect();
    for (i, visible) in rows {
        let Some(e) = entities.get(i) else { continue };
        let name = e.get("Name").or(e.get("targetname")).unwrap_or("").to_string();
        out.push((e.classname().to_string(), name, visible));
    }
    out
}

/// No flag of capture the flag shows on the party's Combat Arms maps
/// (those that have them).
#[test]
fn the_gun_game_hides_combat_arms_flags() {
    for name in ["warehouse", "junk_flea", "darkhills", "bloodmoney"] {
        let Some(mut sim) = gun_game_on(name) else { return };
        let rows = shown(&mut sim);
        let flags: Vec<_> = rows.iter().filter(|(c, _, _)| c == "FlagItem").collect();
        eprintln!("{name}: flags {flags:?}");
        // JUNK_FLEA has capture the flag (its flags' model is in a V2-keyed
        // archive, as every map's).
        assert!(
            name != "junk_flea" || !flags.is_empty(),
            "{name}: the map's flags are placed"
        );
        assert!(
            flags.iter().all(|(_, _, visible)| !visible),
            "{name}: a flag shows: {flags:?}"
        );
    }
}

/// JUNK_FLEA's Quarantine world model (`QRT_Show_L`) is hidden and its
/// usual one (`QRT_Hide_L`) shown, as in every mode but Quarantine.
#[test]
fn the_gun_game_sets_junk_fleas_mode_models_up() {
    let Some(mut sim) = gun_game_on("junk_flea") else {
        return;
    };
    if sim.app.world().resource::<CaData>().missions().is_none() {
        eprintln!("skipping: no install missions (the V1 key)");
        return;
    }
    let rows = shown(&mut sim);
    let model = |n: &str| rows.iter().find(|(_, name, _)| name == n).map(|r| r.2);
    eprintln!(
        "QRT_Show_L {:?}, QRT_Hide_L {:?}",
        model("QRT_Show_L"),
        model("QRT_Hide_L")
    );
    assert_eq!(model("QRT_Show_L"), Some(false), "Quarantine's model hidden");
    assert_eq!(model("QRT_Hide_L"), Some(true), "the other modes' model shown");
}
