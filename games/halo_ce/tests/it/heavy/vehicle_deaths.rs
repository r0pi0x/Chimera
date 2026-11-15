//! A Halo player killed in a vehicle's seat (`HaloSim` on a flat floor
//! with the donor map's Spartan, a Warthog made far from the floor's
//! origin): the rider is out of its seat and the seat empty, and its body
//! (`body::death::HaloCorpse`, left where the player fell) lies at the
//! Warthog, in or beside it, not at the map's origin. Passes with a note
//! without an install. And a Halo game's start (its game type set again)
//! stands every rider and puts the vehicles back at their placements.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Dead, Health, Hitgroup};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::{
    game::{
        body::{
            death::HaloCorpse,
            load::{self, DONOR_MAP},
        },
        maps::{self, space},
        movement,
        vehicles::{self, HaloVehicle, Seated, VehicleDefs, level},
    },
    harness::HaloSim,
};

use super::maps::mount;

#[test]
fn a_rider_killed_in_its_seat_leaves_its_body_at_the_vehicle() {
    let Some(mount) = mount() else { return };
    let bytes = mount.read(&maps::map_path(DONOR_MAP)).unwrap();
    let file = blam_formats::cache::MapFile::load(&bytes).unwrap();
    let body = load::spartan(&file, false).unwrap();
    let mut map = chimera_world::map::MapData {
        collision_positions: vec![
            [-100.0, 0.0, -100.0],
            [100.0, 0.0, -100.0],
            [100.0, 0.0, 100.0],
            [-100.0, 0.0, 100.0],
        ],
        collision_indices: vec![[0, 2, 1], [0, 3, 2]],
        spawns: vec![(Vec3::new(0.0, 0.1, 0.0), None)],
        ..Default::default()
    };
    load::add(&mut map, &body);
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    // 20 WU out along x (about 60 m from the origin).
    let w = vehicles::spawn_vehicle(sim.app.world_mut(), "warthog", Vec3::new(20.0, 0.0, 0.5), 0.0).unwrap();
    sim.seconds(2.0);
    let seat = {
        let world = sim.app.world();
        let v = world.get::<HaloVehicle>(w).unwrap();
        let (d, phys) = &world.resource::<VehicleDefs>().0[v.def];
        level::to_engine(v.world(phys, v.seat_marker(d, 1)))
    };
    let p = sim.spawn_character(seat + Vec3::Y * 0.9, movement::ID);
    sim.ticks(2);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = seat + Vec3::Y * 0.9;
    sim.intent(p).use_key = true;
    sim.ticks(1);
    sim.intent(p).use_key = false;
    sim.ticks(1);
    assert_eq!(
        sim.app.world().get::<Seated>(p).map(|s| (s.vehicle, s.seat)),
        Some((w, 1)),
        "seated"
    );
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 50.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    let world = sim.app.world();
    let dead = world.get::<Dead>(p).is_some() || world.get::<Health>(p).is_some_and(|h| h.current <= 0.0);
    assert!(dead, "dead");
    assert_eq!(sim.app.world().get::<Seated>(p), None, "out of its seat");
    assert_eq!(
        sim.app.world().get::<HaloVehicle>(w).unwrap().riders[1],
        None,
        "the seat empty"
    );
    let w2 = sim.app.world_mut();
    let corpses: Vec<HaloCorpse> = w2.query::<&HaloCorpse>().iter(w2).cloned().collect();
    assert_eq!(corpses.len(), 1, "one body");
    let at = corpses[0].at;
    let vehicle = sim.app.world().get::<Transform>(w).unwrap().translation;
    let from_vehicle = (at - vehicle).with_y(0.0).length();
    eprintln!(
        "body at {at} (WU {:?}), the Warthog at {vehicle}: {from_vehicle} m apart",
        space::to_game(at)
    );
    assert!(at.length() > 30.0, "not at the map's origin: {at}");
    assert!(from_vehicle < 6.0, "in or beside the Warthog: {from_vehicle} m");
}

/// A Halo game's start (`rules::start_as`, its game type set again) on
/// Blood Gulch with a rider driving a Warthog away: the rider stands and
/// the Warthog is back at its placement (`vehicles::reset_vehicles`).
#[test]
fn a_new_game_stands_the_riders_and_puts_the_vehicles_back() {
    let Some(mount) = mount() else { return };
    let map = super::maps::load(&mount, "bloodgulch", false);
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.seconds(3.0);
    let (w, placed) = {
        let world = sim.app.world_mut();
        let defs = world.resource::<VehicleDefs>().clone();
        let mut q = world.query::<(Entity, &HaloVehicle)>();
        q.iter(world)
            .filter(|(_, v)| defs.0[v.def].0.name == "warthog")
            .map(|(e, v)| (e, v.drive.body.position))
            .min_by(|a, b| a.1.x.total_cmp(&b.1.x))
            .unwrap()
    };
    let seat = {
        let world = sim.app.world();
        let v = world.get::<HaloVehicle>(w).unwrap();
        let (d, phys) = &world.resource::<VehicleDefs>().0[v.def];
        level::to_engine(v.world(phys, v.seat_marker(d, 0)))
    };
    let p = sim.spawn_character(seat + Vec3::Y * 0.9, movement::ID);
    sim.ticks(2);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = seat + Vec3::Y * 0.9;
    sim.intent(p).use_key = true;
    sim.ticks(1);
    sim.intent(p).use_key = false;
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<Seated>(p).map(|s| s.seat), Some(0), "driving");
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(3.0);
    sim.intent(p).move_axis = Vec2::ZERO;
    let away = (sim.app.world().get::<HaloVehicle>(w).unwrap().drive.body.position - placed)
        .truncate()
        .length();
    assert!(away > 5.0, "driven {away} WU away");
    halo_ce::game::rules::start_as(sim.app.world_mut(), "slayer");
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<Seated>(p), None, "the new game stands the rider");
    let v = sim.app.world().get::<HaloVehicle>(w).unwrap();
    assert!(v.riders.iter().all(Option::is_none));
    let back = (v.drive.body.position - placed).truncate().length();
    assert!(back < 0.5, "the Warthog {back} WU from its placement");
}
