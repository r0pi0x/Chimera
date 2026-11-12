//! Each stock map's pickups as HL2:DM's game makes them (specs/hl2dm/
//! maps_entities.md 0-4, the census): its items (kits, vials, batteries,
//! ammo boxes: 329 in all), chargers and ammo crates, each item come to
//! rest at a spot and listed for relocation; on dm_lockdown a kit taken
//! comes back 2000 ticks later at its spot and a placed weapon's copy
//! 1333 ticks after it is taken (Chimera's `weapon::respawn`); each
//! item's model follows its body and each crate's model is its collision
//! (`pickups::ItemModel`, `ammo_crates::CrateModel`). Converted
//! lean from the install; skips without one.

use bevy::prelude::*;
use chimera_core::core::SimClock;
use chimera_sim::weapon::{
    drop::Loose,
    equip::MapWeapon,
    respawn::{self, Copied, MapPickup, Respawning},
};
use chimera_world::{
    harness::Sim,
    map::{MapLoadOptions, MapSimPlugin},
};
use hl2dm::{
    game::{
        STOCK_MAPS,
        ammo_crates::{AmmoCrate, CrateModel},
        pickups::{Charger, Item, ItemModel, Pickup},
    },
    harness::Hl2Sim,
};

use super::map_stock::{convert, install_mount};

/// Items, chargers and crates per map (maps_entities.md 0).
const COUNTS: [(&str, usize, usize, usize); 8] = [
    ("dm_lockdown", 56, 3, 0),
    ("dm_overwatch", 13, 0, 0),
    ("dm_powerhouse", 86, 6, 0),
    ("dm_resistance", 9, 2, 0),
    ("dm_runoff", 41, 0, 0),
    ("dm_steamlab", 34, 2, 2),
    ("dm_underpass", 75, 5, 0),
    ("halls3", 15, 2, 0),
];

#[test]
fn each_stock_maps_pickups() {
    let Some(mount) = install_mount() else { return };
    let mut total = 0;
    for name in STOCK_MAPS {
        let data = convert(&mount, name, &MapLoadOptions::lean());
        let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(data));
        sim.set_tick_interval(0.015);
        sim.seconds(5.0);
        let w = sim.app.world_mut();
        let items: Vec<(Pickup, MapPickup, Transform)> =
            w.query::<(&Pickup, &MapPickup, &Transform)>().iter(w).map(|(p, m, t)| (p.clone(), m.clone(), *t)).collect();
        let chargers = w.query::<&Charger>().iter(w).count();
        let crates = w.query::<&AmmoCrate>().iter(w).count();
        let weapons = w.query_filtered::<&Loose, With<MapWeapon>>().iter(w).count();
        let rested = items.iter().filter(|(_, m, _)| m.spot.is_some() && m.next_check.is_some()).count();
        let ammo = items.iter().filter(|(p, ..)| matches!(p.item, Item::Ammo { .. })).count();
        eprintln!(
            "{name}: {} items ({ammo} ammo boxes, {rested} at rest), {chargers} chargers, {crates} crates, {weapons} placed weapons",
            items.len()
        );
        let want = COUNTS.iter().find(|c| c.0 == *name).unwrap();
        assert_eq!((items.len(), chargers, crates), (want.1, want.2, want.3), "{name}");
        // Each item drawn by its model, on its body's bottom; each crate's
        // model its collision.
        let models: Vec<(usize, Transform)> = w
            .query_filtered::<(&chimera_world::map::PropEntity, &Transform), With<ItemModel>>()
            .iter(w)
            .map(|(p, t)| (p.0, *t))
            .collect();
        assert_eq!(models.len(), items.len(), "{name}: every item's model follows it");
        for (index, t) in &models {
            let (_, _, body) = items.iter().find(|(p, ..)| p.map_index == *index).expect("its item");
            let bottom = body.transform_point(Vec3::new(0.0, -8.0 * 0.0254, 0.0));
            assert!(t.translation.distance(bottom) < 1e-3, "{name}: item {index}'s model at its body");
        }
        let linked = w.query_filtered::<(), With<CrateModel>>().iter(w).count();
        assert_eq!(linked, crates, "{name}: every crate's model");
        // Nearly all settle within 5 s (a few may still roll).
        assert!(rested * 10 >= items.len() * 9, "{name}: {rested} of {} at rest", items.len());
        total += items.len();

        if *name != "dm_lockdown" {
            continue;
        }
        // A kit taken comes back 2000 ticks later, at its spot.
        let kit = {
            let w = sim.app.world_mut();
            w.query::<(Entity, &Pickup, &MapPickup)>()
                .iter(w)
                .find(|(_, p, m)| p.item == Item::HealthKit && m.spot.is_some())
                .map(|(e, ..)| e)
                .expect("a kit at rest")
        };
        let taken = sim.app.world().resource::<SimClock>().tick;
        respawn::taken(sim.app.world_mut(), kit);
        sim.ticks(1);
        {
            let index = sim.app.world().get::<Pickup>(kit).unwrap().map_index;
            let w = sim.app.world_mut();
            let hidden = w
                .query_filtered::<(&chimera_world::map::PropEntity, &Visibility), With<ItemModel>>()
                .iter(w)
                .any(|(p, v)| p.0 == index && *v == Visibility::Hidden);
            assert!(hidden, "a taken kit's model is hidden");
        }
        let mut back = None;
        for _ in 0..2099 {
            sim.ticks(1);
            if back.is_none() && sim.app.world().get::<Respawning>(kit).is_none() {
                back = Some(sim.app.world().resource::<SimClock>().tick - taken);
            }
        }
        assert_eq!(back, Some(2000), "the kit's return");
        // A placed weapon's copy 1333 ticks after it is taken.
        let loose = {
            let w = sim.app.world_mut();
            w.query_filtered::<Entity, (With<Loose>, With<MapWeapon>, With<MapPickup>)>().iter(w).next()
        };
        if let Some(loose) = loose {
            let taken = sim.app.world().resource::<SimClock>().tick;
            respawn::weapon_taken(sim.app.world_mut(), loose);
            sim.app.world_mut().despawn(loose);
            let mut appeared = None;
            for _ in 0..1400 {
                sim.ticks(1);
                let w = sim.app.world_mut();
                if appeared.is_none() && w.query_filtered::<(), (With<Loose>, With<Copied>)>().iter(w).next().is_some() {
                    appeared = Some(w.resource::<SimClock>().tick - taken);
                }
            }
            assert_eq!(appeared, Some(1333), "the weapon's copy");
        }
    }
    assert_eq!(total, 329);
}
