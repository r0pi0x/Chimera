//! An HL2:DM map in Ultimate Gun Game: HL2:DM's pickups stay off there
//! (the user's call, 2026-11-12; docs/DESIGN.md "Gun game rules"), so
//! dm_steamlab's items (kits, batteries, ammo boxes) and wall chargers
//! aren't placed at all, on the server and on a client alike (both load
//! the map through the party's sources: `KitLoad::pickups`); its two ammo
//! crates stay, by their own models (specs/hl2dm/maps_entities.md 3: a
//! static solid model, the map's `model` key ignored, by its `AmmoType`),
//! solid and giving nothing. Its props follow HL2:DM's
//! rules (source_deltas.md 1.5, gravity_gun.md Corrections 1): its
//! multiplayer props collide with players (the rules' swap by the map's
//! game: hl2dm's tests/it/map_physics_rules.rs). Converted lean from the
//! install; passes with a note without it.

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use chimera_world::{
    harness::Sim,
    map::{MapEntities, MapLoadOptions, MapSimPlugin, PhysicsProp, PropEntity, PushAway},
};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::UltimateGunGamePlugin;

const MAP: &str = "hl2dm:dm_steamlab";
/// Its items and chargers in the map's entities, and its crates
/// (maps_entities.md 0).
const ITEMS: usize = 34;
const CHARGERS: usize = 2;
const CRATES: usize = 2;

fn is_item(class: &str) -> bool {
    class.starts_with("item_") && class != "item_ammo_crate"
}

#[test]
fn an_hl2_maps_pickups_are_not_placed_its_crates_are_and_its_props_follow_hl2s_rules() {
    let Ok(data) = party_kit::load_map_with(MAP, &MapLoadOptions::lean()) else {
        eprintln!("skipping: needs the HL2:DM install");
        return;
    };
    // The map has them; the party places none of its items and chargers.
    let class_of = |i: usize| data.entities[i].classname().to_ascii_lowercase();
    let in_map = |f: &dyn Fn(&str) -> bool| (0..data.entities.len()).filter(|i| f(&class_of(*i))).count();
    assert_eq!(in_map(&|c| is_item(c) && !c.ends_with("charger")), ITEMS, "the map's items");
    assert_eq!(in_map(&|c| c.ends_with("charger")), CHARGERS, "the map's chargers");
    let mut crates = 0;
    for p in &data.props {
        let Some(i) = p.entity else { continue };
        let class = class_of(i);
        let model = &data.models[p.model];
        assert!(!is_item(&class), "{class} placed: pickups are off in the party");
        if class == "item_ammo_crate" {
            assert!(!model.meshes.is_empty(), "the crate has its model's meshes");
            assert!(p.physics.is_none(), "a crate never moves");
            crates += 1;
        }
    }
    assert_eq!(crates, CRATES, "every crate placed");
    // The HL2:DM app's own load (its gameplay) places them all.
    let own = hl2dm::game::maps::load_installed_map("dm_steamlab", &MapLoadOptions::lean()).expect("the install");
    let placed = own
        .props
        .iter()
        .filter_map(|p| p.entity)
        .filter(|i| is_item(&own.entities[*i].classname().to_ascii_lowercase()))
        .count();
    assert_eq!(placed, ITEMS + CHARGERS, "the HL2:DM app's map has its items and chargers");

    let mut sim = Sim::new(
        CaSim,
        (MapSimPlugin::new(data), PartyPartsPlugin, UltimateGunGamePlugin { seed: 7, rungs: 13, ..Default::default() }),
    );
    sim.set_tick_interval(0.015);
    sim.seconds(4.0);
    let w = sim.app.world_mut();
    let entities = w.resource::<MapEntities>().entities.clone();
    let class = |i: usize| entities[i].classname().to_ascii_lowercase();
    let nodes: Vec<(String, Vec3, Option<RigidBody>, bool, Option<PushAway>)> = w
        .query::<(&PropEntity, &GlobalTransform, Option<&RigidBody>, Has<Collider>, Option<&PhysicsProp>)>()
        .iter(w)
        .map(|(p, t, rb, c, pp)| (class(p.0), t.translation(), rb.copied(), c, pp.map(|p| p.push)))
        .collect();
    assert!(!nodes.iter().any(|n| is_item(&n.0)), "no item or charger in the world");
    let crate_nodes: Vec<_> = nodes.iter().filter(|n| n.0 == "item_ammo_crate").collect();
    assert_eq!(crate_nodes.len(), CRATES);
    assert!(crate_nodes.iter().all(|n| n.3), "the crates are solid");

    // HL2:DM's prop rules.
    let multiplayer: Vec<_> = nodes.iter().filter(|n| n.0 == "prop_physics_multiplayer").collect();
    eprintln!("{MAP}: {crates} crates, {} multiplayer props", multiplayer.len());
    assert!(!multiplayer.is_empty());
    assert!(
        multiplayer.iter().all(|n| n.4.is_none_or(|p| p == PushAway::Collide)),
        "multiplayer props collide with players"
    );
}
