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
/// data/rung_characters.toml): an infected's weapon the data names no
/// holders for is held by the infected's bodies only, any other such
/// weapon by the player bodies; a weapon the data names (the NPC and boss
/// guns, the suit weapons, the NPC infected's hands: characters.md 12)
/// by bodies that exist, an infected's hand by infected bodies (the
/// player bodies' infected, an infected mercenary or an NPC); the
/// coverage is printed.
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
    let (mut to_players, mut to_zombies, mut named) = (0, 0, 0);
    for w in &set.weapons {
        let bodies = rungs::bodies_for(&h, Some(&catalog), Some(&ca), &w.id);
        let infected = w.has_hook(rungs::INFECTED);
        if h.override_for(&w.id).is_some() {
            assert!(!bodies.is_empty(), "{}: its holders exist", w.id);
            if infected {
                for b in &bodies {
                    let r = ch.model(*b).unwrap();
                    let fam = ch.family(r);
                    assert!(zombies.contains(b) || r.npc.is_some() || fam.class == Some(39), "{}: {} isn't an infected body", w.id, r.name);
                }
            }
            named += 1;
        } else if infected {
            assert_eq!(bodies, zombies, "{}: the infected's", w.id);
            to_zombies += 1;
        } else {
            assert_eq!(bodies, players, "{}: the player bodies", w.id);
            to_players += 1;
        }
    }
    eprintln!(
        "{} weapons: {to_players} to the {} player bodies, {to_zombies} to the {} infected bodies, {named} to the bodies the data names",
        set.weapons.len(),
        players.len(),
        zombies.len()
    );
    assert!(named >= 60, "{named}");
}
