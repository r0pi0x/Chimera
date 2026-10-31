//! The gun game's infected from the user's install (`game::rungs`,
//! docs/DESIGN.md "Gun game rules"): the Quarantine hands are marked as
//! an infected's weapons (`rungs::INFECTED`, from their `LowerAniType`)
//! and are knives, no gun rung's melee pool has them, the stock body's
//! infection override names a face the data has, and the infected
//! bodies are rung bodies. Needs the V1 key; passes with a note without.

use combat_arms::game::{data::CaData, inventory::KNIFE_HOOK, rungs};

#[test]
fn the_install_marks_its_infected_hands_and_overrides() {
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.models.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let set = ca.weapon_set();
    let infected: Vec<&str> = set.weapons.iter().filter(|w| w.has_hook(rungs::INFECTED)).map(|w| w.id.as_str()).collect();
    eprintln!("{} infected's weapons: {infected:?}", infected.len());
    for (index, want) in rungs::STOCK_HANDS.iter().zip(["combat_arms:quarantine_virus_hand", "combat_arms:quarantine_host_hand"]) {
        assert_eq!(ca.weapon_for_index(*index), Some(want), "record {index}");
        assert!(infected.contains(&want), "{want} marked infected");
    }
    assert!(set.weapons.iter().filter(|w| w.has_hook(rungs::INFECTED)).all(|w| w.has_hook(KNIFE_HOOK)), "every hand a knife");
    assert!(!set.weapons.iter().any(|w| w.id == "combat_arms:m9_knife" && w.has_hook(rungs::INFECTED)));
    let ch = ca.characters();
    let o = ch.infection_of(0).expect("Normal's override");
    assert!(ch.face.iter().any(|f| f.id == o.face), "its face {} is in the data", o.face);
    assert_eq!(ch.infected_look(0).face, o.face);
    let bodies = ch.infected_bodies();
    let pool = ch.rung_bodies();
    assert!(!bodies.is_empty() && bodies.iter().all(|b| pool.contains(b)));
    eprintln!("{} of {} rung bodies have an infection override", bodies.len(), pool.len());
}

/// Characters follow the gun on the install's data (`rungs::holders`,
/// data/rung_characters.toml): every infected's weapon is held by the
/// infected's bodies only, every other weapon by the player bodies (none
/// of them only an infected's); the coverage is printed.
#[test]
fn the_installs_weapons_map_to_their_holders() {
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.models.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let set = ca.weapon_set();
    let mut catalog = chimera_sim::weapon::data::WeaponCatalog::default();
    catalog.sets.push(set);
    let ch = ca.characters();
    let (players, zombies) = (ch.rung_bodies(), ch.infected_bodies());
    let h = rungs::holders();
    let (mut to_players, mut to_zombies, mut other) = (0, 0, 0);
    for w in &set.weapons {
        let bodies = rungs::bodies_for(&h, Some(&catalog), Some(&ca), &w.id);
        if w.has_hook(rungs::INFECTED) {
            assert_eq!(bodies, zombies, "{}: the infected's", w.id);
            to_zombies += 1;
        } else if bodies == players {
            to_players += 1;
        } else {
            other += 1;
        }
    }
    eprintln!(
        "{} weapons: {to_players} to the {} player bodies, {to_zombies} to the {} infected bodies, {other} otherwise",
        set.weapons.len(),
        players.len(),
        zombies.len()
    );
    // What a spec session should place (names only hint: by name, not
    // by data, nothing is decided from them).
    let hinted: Vec<&str> = set
        .weapons
        .iter()
        .map(|w| w.id.as_str())
        .filter(|id| ["npc", "boss", "hqraid", "powered_suit", "specialist", "assassin"].iter().any(|k| id.contains(k)))
        .collect();
    eprintln!("{} weapons whose names hint at an NPC, a boss or a specialist: {hinted:?}", hinted.len());
    assert_eq!(other, 0, "no overrides in the data yet");
}
