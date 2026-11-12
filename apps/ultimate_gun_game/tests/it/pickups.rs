//! No gun pickups in Ultimate Gun Game (the user's decision; the mode's
//! `[pickups] weapons = false`, `weapon::drop::NoWeaponPickups`): a map's
//! guns aren't placed, walking over any game's gun takes nothing, the drop
//! key is off, and other pickups (HL2:DM's health kit, here with its game's
//! pickups added by hand: the party never composes them, docs/DESIGN.md
//! "Gun game rules") go on working.

use bevy::prelude::*;
use chimera_core::{console::Console, core::Health};
use chimera_sim::{
    minigame::{self, ladder::Ladder},
    weapon::{Armor, Inventory, Weapon, drop::{self, Loose, NoWeaponPickups}, equip::MapWeapon, respawn},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::{MapEntities, MapEntity}};
use combat_arms::{game::movement, harness::CaSim};
use hl2dm::game::{pickups::{Pickup, PickupsPlugin}, weapons as hl2};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

/// Where the guns and the kit lie, and the player stands, on the greybox.
const SPOT: Vec3 = Vec3::new(0.0, 1.0, 0.0);

fn map_entity(class: &str) -> MapEntity {
    MapEntity {
        keyvalues: vec![("classname".to_string(), class.to_string()), ("origin".to_string(), "0 0 8".to_string())],
        ..Default::default()
    }
}

/// The automatic gun game, a player in it, nothing yet on the ground.
fn playing() -> (Sim, Entity) {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin, PickupsPlugin, UltimateGunGamePlugin { seed: 3, rungs: 64, ..Default::default() }));
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    let me = sim.spawn_character(SPOT, movement::ID);
    sim.seconds(1.5);
    (sim, me)
}

fn carries(sim: &Sim, me: Entity, id: &str) -> bool {
    let w = sim.app.world();
    w.get::<Inventory>(me).unwrap().weapons.iter().any(|g| w.get::<Weapon>(*g).is_some_and(|x| x.id == id))
}

fn at_feet(sim: &Sim, me: Entity) -> Transform {
    Transform::from_translation(sim.app.world().get::<Transform>(me).unwrap().translation)
}

/// An HL2:DM gun lying under the player, as a map would have placed it.
fn hl2_gun_underfoot(sim: &mut Sim, me: Entity) {
    let at = at_feet(sim, me);
    let w = sim.app.world_mut();
    respawn::place_one(w, hl2::SMG1, 0, at, 0.0).expect("placed");
}

/// A CS:S gun the player dropped.
fn cs_gun_dropped(sim: &mut Sim, me: Entity) {
    let w = sim.app.world_mut();
    chimera_sim::weapon::give(w, me, "cs_source:weapon_ak47");
    let gun = w.get::<Inventory>(me).unwrap().weapons.iter().copied().find(|g| w.get::<Weapon>(*g).unwrap().id == "cs_source:weapon_ak47");
    drop::drop_this(w, me, gun.expect("given"), false);
    sim.ticks(1);
}

#[test]
fn guns_on_the_ground_are_not_taken_and_a_health_kit_is() {
    let (mut sim, me) = playing();
    assert!(sim.app.world().contains_resource::<NoWeaponPickups>(), "the mode's rule");

    hl2_gun_underfoot(&mut sim, me);
    cs_gun_dropped(&mut sim, me);
    let lying = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query_filtered::<(), With<Loose>>().iter(w).count()
    };
    assert_eq!(lying(&mut sim), 2);
    sim.seconds(2.0);
    assert!(!carries(&sim, me, hl2::SMG1), "an HL2:DM gun isn't taken");
    assert!(!carries(&sim, me, "cs_source:weapon_ak47"), "a dropped CS:S gun isn't taken");
    assert_eq!(lying(&mut sim), 2, "both still lie there");

    // The same guns are taken without the rule: the test sees the pickup.
    sim.app.world_mut().remove_resource::<NoWeaponPickups>();
    sim.seconds(2.0);
    assert!(carries(&sim, me, hl2::SMG1) || carries(&sim, me, "cs_source:weapon_ak47"), "without the rule one is taken");
    sim.app.world_mut().insert_resource(NoWeaponPickups);

    // A health kit under the player heals it, the rule or not.
    sim.app.world_mut().get_mut::<Health>(me).unwrap().current = 0.5;
    // On an HL2:DM rung: its identity wears the suit the kit's touch asks of
    // a character.
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let rung = ladder.rungs.iter().position(|id| id.starts_with("hl2dm:")).expect("an HL2:DM rung");
    minigame::ladder::set_rung(sim.app.world_mut(), me, rung, &ladder);
    sim.seconds(0.5);
    sim.app.world_mut().get_mut::<Health>(me).unwrap().current = 0.5;
    sim.app.world_mut().entity_mut(me).insert_if_new(Armor::default());
    let at = at_feet(&sim, me).translation / 0.0254;
    let kit = MapEntity {
        keyvalues: vec![
            ("classname".to_string(), "item_healthkit".to_string()),
            ("origin".to_string(), format!("{} {} {}", at.x, -at.z, at.y)),
        ],
        ..Default::default()
    };
    sim.app.world_mut().insert_resource(MapEntities { entities: vec![kit].into(), scale: 0.0254 });
    sim.seconds(0.5);
    let w = sim.app.world_mut();
    assert_eq!(w.query_filtered::<(), With<Pickup>>().iter(w).count(), 1, "the kit is out");
    let health = sim.app.world().get::<Health>(me).unwrap().current;
    assert!((health - 0.75).abs() < 1e-3, "+25 points: {health}");
}

#[test]
fn a_maps_guns_are_not_placed() {
    let (mut sim, _me) = playing();
    let map = MapEntities { entities: vec![map_entity("weapon_smg1"), map_entity("weapon_ak47")].into(), scale: 0.0254 };
    sim.app.world_mut().insert_resource(map);
    sim.seconds(0.5);
    let placed = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query_filtered::<(), With<MapWeapon>>().iter(w).count()
    };
    assert_eq!(placed(&mut sim), 0, "no gun of the map's lies there");
    // A mode with pickups on the same map has them.
    sim.app.world_mut().remove_resource::<NoWeaponPickups>();
    sim.seconds(0.5);
    assert!(placed(&mut sim) > 0, "without the rule the map's guns lie there");
}

#[test]
fn the_drop_key_is_off() {
    let (mut sim, me) = playing();
    let before = sim.app.world().get::<Inventory>(me).unwrap().weapons.len();
    sim.app.world_mut().entity_mut(me).insert(chimera_core::core::LocalPlayer);
    sim.app.world_mut().resource_mut::<Console>().submit("drop");
    sim.seconds(0.5);
    assert_eq!(sim.app.world().get::<Inventory>(me).unwrap().weapons.len(), before, "the gun stays in hand");
}
