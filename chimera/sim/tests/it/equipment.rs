//! What the weapon layer gives and takes on its own (`chimera_sim::weapon`,
//! plan ecs-cleanup step 5): a new character's starting weapons (or its
//! own kit) in the first tick after it appears, drawn that same tick; and
//! placed map weapons the logic removed (`map::entities::RemovedWeapons`)
//! taken out of the world in the tick the list changed, loose or carried,
//! and only those newly removed.

use bevy::prelude::*;
use chimera_sim::{
    movement::placeholder,
    testkit::{self, PISTOL, RIFLE, SHOTGUN, kit_sim},
    weapon::{Inventory, OwnKit, StartingWeapons, Weapon, drop::Loose, equip::MapWeapon, give, spawn_weapon},
};
use chimera_world::{greybox, harness::Sim, map::entities::RemovedWeapons};

fn ids(sim: &Sim, p: Entity) -> Vec<&'static str> {
    let world = sim.app.world();
    world
        .get::<Inventory>(p)
        .map(|i| i.weapons.iter().map(|w| world.get::<Weapon>(*w).unwrap().id).collect())
        .unwrap_or_default()
}

/// The tick timing is the old poller's (`give_starting_weapons` was an
/// exclusive system before selection): nothing at the spawn, everything
/// after the first tick, the last given already in hand, so selection ran
/// after the giving in that same tick.
#[test]
fn starting_weapons_come_in_the_first_tick_after_the_spawn_and_are_drawn_in_it() {
    let mut sim = kit_sim();
    sim.app.insert_resource(StartingWeapons {
        team: Vec::new(),
        all: vec![PISTOL, RIFLE],
    });
    let p = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    let own = sim.spawn_character(greybox::SPAWNS[1], placeholder::ID);
    sim.app.world_mut().entity_mut(own).insert(OwnKit(vec![SHOTGUN]));
    assert!(sim.app.world().get::<Inventory>(p).is_none(), "nothing at the spawn itself");
    sim.ticks(1);
    assert_eq!(ids(&sim, p), [PISTOL, RIFLE]);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(RIFLE), "the last given, drawn in the same tick");
    assert_eq!(ids(&sim, own), [SHOTGUN], "a character's own kit in place of the starting weapons");
    assert_eq!(testkit::active_id(sim.app.world(), own), Some(SHOTGUN));
    // Given once.
    sim.ticks(3);
    assert_eq!(ids(&sim, p), [PISTOL, RIFLE]);
}

/// A loose placed weapon (map entity `index`) at `at`.
fn lay(sim: &mut Sim, id: &str, index: usize, at: Vec3) -> (Entity, Entity) {
    let world = sim.app.world_mut();
    let holder = world.spawn_empty().id();
    let weapon = spawn_weapon(world, id, holder).unwrap();
    world.get_mut::<Weapon>(weapon).unwrap().owner = None;
    world.entity_mut(weapon).insert(MapWeapon(index));
    world.despawn(holder);
    let item = world
        .spawn((
            Loose {
                weapon,
                dropper: None,
                since: 0.0,
            },
            MapWeapon(index),
            Transform::from_translation(at),
        ))
        .id();
    (item, weapon)
}

#[test]
fn weapons_the_logic_removed_leave_in_that_tick_loose_or_carried_and_only_once() {
    let mut sim = kit_sim();
    let p = testkit::with_player(&mut sim, None, PISTOL);
    let carried = give(sim.app.world_mut(), p, RIFLE).unwrap();
    sim.app.world_mut().entity_mut(carried).insert(MapWeapon(1));
    sim.ticks(1);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(RIFLE));
    let far = greybox::SPAWNS[0] + Vec3::new(8.0, 0.0, 8.0);
    let (item0, weapon0) = lay(&mut sim, SHOTGUN, 0, far);
    let (item2, weapon2) = lay(&mut sim, SHOTGUN, 2, far + Vec3::X);
    sim.ticks(1);

    sim.app.insert_resource(RemovedWeapons(vec![0, 1]));
    sim.ticks(1);
    let world = sim.app.world();
    assert!(world.get_entity(item0).is_err() && world.get_entity(weapon0).is_err(), "the loose one gone");
    assert!(world.get_entity(carried).is_err(), "the carried one gone");
    let inv = world.get::<Inventory>(p).unwrap();
    assert!(!inv.weapons.contains(&carried) && inv.active != Some(carried) && inv.wanted != Some(carried));
    assert!(world.get_entity(item2).is_ok() && world.get_entity(weapon2).is_ok(), "the third untouched");
    sim.ticks(1);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(PISTOL), "the next weapon drawn");

    // A round restart puts placed weapon 0 back while the list still
    // names it: it stays.
    let (again, _) = lay(&mut sim, SHOTGUN, 0, far);
    sim.ticks(2);
    assert!(sim.app.world().get_entity(again).is_ok(), "a re-placed weapon isn't removed again");
    // The logic removes it again (a new list): it goes.
    sim.app.insert_resource(RemovedWeapons(vec![1]));
    sim.ticks(1);
    sim.app.insert_resource(RemovedWeapons(vec![0, 1]));
    sim.ticks(1);
    assert!(sim.app.world().get_entity(again).is_err());
}
