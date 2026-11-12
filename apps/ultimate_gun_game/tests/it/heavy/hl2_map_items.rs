//! An HL2:DM map in Ultimate Gun Game (docs/plans/active/
//! parity-gaps-ranked.md 5, 7): dm_steamlab's items (kits, batteries, ammo
//! boxes) and its two ammo crates are drawn by their own models
//! (specs/hl2dm/maps_entities.md 1, 3: the map's `model` key ignored, the
//! crate's by its `AmmoType`), the items physics objects players walk
//! through that come to rest, the crates solid; the gun game takes nothing
//! from them (pickups stay off: the user's call). Its props follow HL2:DM's
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
/// Its items and crates (maps_entities.md 0).
const ITEMS: usize = 34;
const CRATES: usize = 2;

fn is_item(class: &str) -> bool {
    class.starts_with("item_") && !class.ends_with("charger") && class != "item_ammo_crate"
}

#[test]
fn an_hl2_maps_items_and_crates_are_drawn_and_its_props_follow_hl2s_rules() {
    let Ok(data) = party_kit::load_map_with(MAP, &MapLoadOptions::lean()) else {
        eprintln!("skipping: needs the HL2:DM install");
        return;
    };
    // Each item and crate placed with its own model, meshes and all.
    let class_of = |i: usize| data.entities[i].classname().to_ascii_lowercase();
    let mut items = 0;
    let mut crates = 0;
    for p in &data.props {
        let Some(i) = p.entity else { continue };
        let class = class_of(i);
        let model = &data.models[p.model];
        if is_item(&class) {
            assert!(!model.meshes.is_empty(), "{class} has its model's meshes");
            assert!(p.physics.as_ref().is_some_and(|b| b.push == PushAway::Ignore), "{class}: walked through");
            items += 1;
        } else if class == "item_ammo_crate" {
            assert!(!model.meshes.is_empty(), "the crate has its model's meshes");
            assert!(p.physics.is_none(), "a crate never moves");
            crates += 1;
        }
    }
    assert_eq!((items, crates), (ITEMS, CRATES), "every item and crate placed");

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
    let item_nodes: Vec<_> = nodes.iter().filter(|n| is_item(&n.0)).collect();
    assert_eq!(item_nodes.len(), ITEMS, "every item drawn");
    for (c, at, rb, ..) in &item_nodes {
        assert!(rb.is_some_and(|r| r.is_dynamic()), "{c}: a physics object");
        assert!(at.is_finite() && at.y > -100.0, "{c} rests on the map: {at}");
    }
    let crate_nodes: Vec<_> = nodes.iter().filter(|n| n.0 == "item_ammo_crate").collect();
    assert_eq!(crate_nodes.len(), CRATES);
    assert!(crate_nodes.iter().all(|n| n.3), "the crates are solid");

    // HL2:DM's prop rules.
    let multiplayer: Vec<_> = nodes.iter().filter(|n| n.0 == "prop_physics_multiplayer").collect();
    eprintln!("{MAP}: {items} items, {crates} crates, {} multiplayer props", multiplayer.len());
    assert!(!multiplayer.is_empty());
    assert!(
        multiplayer.iter().all(|n| n.4.is_none_or(|p| p == PushAway::Collide)),
        "multiplayer props collide with players"
    );
}
