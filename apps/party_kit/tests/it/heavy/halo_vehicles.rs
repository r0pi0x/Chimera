//! Halo's Warthogs in the party (Blood Gulch from the disc image, on the
//! party's simulation, `CaSim` with `PartyPartsPlugin`): the scenario's two
//! Warthogs are there; a Combat Arms soldier (another game's movement)
//! takes the driver's seat with the use key, its own movement suspended
//! (the engine's `Carried`: jumping and walking move it nowhere but with
//! the Warthog), drives it twenty-odd metres in four seconds and gets out
//! beside it, its movement its own again; a CS:S character in the
//! passenger seat dies there and is out of it, its seat empty; a rider
//! seated at a round restart stands and the Warthog is back at its
//! placement; a rider whose character goes away (a disconnect) leaves its
//! seat empty; a map change with a rider seated leaves no seat behind.
//! Passes with a note without the disc image.

use bevy::prelude::*;
use chimera_core::core::{Carried, Damage, Dead, Health, restart_round};
use chimera_world::{
    harness::Sim,
    map::{MapDebugView, MapSimPlugin, change_map},
};
use combat_arms::harness::CaSim;
use halo_ce::game::vehicles::{HaloVehicle, Seated, VehicleDefs, level};
use party_kit::PartyPartsPlugin;

fn disc() -> bool {
    let there = halo_ce::game::INFO.mount().is_some();
    if !there {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
    }
    there
}

/// The Warthogs, west to east: (entity, its state).
fn warthogs(sim: &mut Sim) -> Vec<(Entity, HaloVehicle)> {
    let w = sim.app.world_mut();
    let mut q = w.query::<(Entity, &HaloVehicle)>();
    let mut out: Vec<(Entity, HaloVehicle)> = q.iter(w).map(|(e, v)| (e, v.clone())).collect();
    out.sort_by(|a, b| a.1.drive.body.position.x.total_cmp(&b.1.drive.body.position.x));
    out
}

/// Seat `seat`'s marker in the world (engine metres).
fn seat_at(sim: &Sim, w: Entity, seat: usize) -> Vec3 {
    let world = sim.app.world();
    let v = world.get::<HaloVehicle>(w).unwrap();
    let (d, phys) = &world.resource::<VehicleDefs>().0[v.def];
    level::to_engine(v.world(phys, v.seat_marker(d, seat)))
}

/// A character of `movement` standing on its feet at seat `seat`'s marker
/// (its centre a little over it), settled, then the use key pressed.
fn walk_up_and_use(sim: &mut Sim, w: Entity, seat: usize, movement: &'static str) -> Entity {
    let p = sim.spawn_character(seat_at(sim, w, seat) + Vec3::Y * 0.9, movement);
    sim.ticks(3);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = seat_at(sim, w, seat) + Vec3::Y * 0.9;
    press_use(sim, p);
    p
}

fn press_use(sim: &mut Sim, p: Entity) {
    sim.intent(p).use_key = true;
    sim.ticks(1);
    sim.intent(p).use_key = false;
    sim.ticks(1);
}

fn seated(sim: &Sim, p: Entity) -> Option<(Entity, usize)> {
    sim.app.world().get::<Seated>(p).map(|s| (s.vehicle, s.seat))
}

fn carried(sim: &Sim, p: Entity) -> bool {
    sim.app.world().get::<Carried>(p).is_some()
}

fn position(sim: &Sim, e: Entity) -> Vec3 {
    sim.app.world().get::<Transform>(e).unwrap().translation
}

fn party_on_blood_gulch() -> Sim {
    let map = party_kit::load_map("halo_ce:bloodgulch", 0).expect("Blood Gulch");
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    sim.seconds(3.0);
    sim
}

#[test]
fn another_games_characters_ride_blood_gulchs_warthogs() {
    if !disc() {
        return;
    }
    let mut sim = party_on_blood_gulch();
    let found = warthogs(&mut sim);
    assert_eq!(found.len(), 2, "Blood Gulch's two Warthogs in the party");
    let (w, _) = found[0];
    let placed = found[0].1.drive.body.position;

    // A Combat Arms soldier takes the driver's seat.
    let soldier = walk_up_and_use(&mut sim, w, 0, combat_arms::game::movement::ID);
    assert_eq!(seated(&sim, soldier), Some((w, 0)), "the soldier in the driver's seat");
    assert!(carried(&sim, soldier), "its movement suspended");
    assert_eq!(sim.app.world().get::<HaloVehicle>(w).unwrap().riders[0], Some(soldier));
    // Its own keys move it nowhere but with the Warthog: jumping in place.
    let over = position(&sim, soldier) - seat_at(&sim, w, 0);
    sim.intent(soldier).jump = true;
    sim.ticks(20);
    sim.intent(soldier).jump = false;
    let still = (position(&sim, soldier) - seat_at(&sim, w, 0) - over).length();
    assert!(still < 0.01, "the soldier's own jump moved it {still} m in its seat");

    // It drives: forward for four seconds, the camera the seat's.
    let start = position(&sim, w);
    let yaw = sim.intent(soldier).yaw;
    sim.intent(soldier).move_axis = Vec2::Y;
    let mut worst: f32 = 0.0;
    for _ in 0..40 {
        sim.seconds(0.1);
        let off = (position(&sim, soldier) - seat_at(&sim, w, 0) - over).length();
        worst = worst.max(off);
    }
    sim.intent(soldier).move_axis = Vec2::ZERO;
    let gone = position(&sim, w).distance(start);
    eprintln!("the soldier drove {gone} m (look yaw {yaw}), at most {worst} m off its seat");
    assert!(gone > 10.0, "drove {gone} m");
    assert!(worst < 0.01, "rode {worst} m off its seat");
    assert!(
        sim.app.world().get::<chimera_core::core::MapView>(soldier).is_some(),
        "the seat's camera"
    );
    sim.seconds(3.0);

    // A CS:S character in the passenger seat dies there.
    let css = walk_up_and_use(&mut sim, w, 1, cs_source::game::movement::ID);
    assert_eq!(
        seated(&sim, css),
        Some((w, 1)),
        "the CS:S character in the passenger seat"
    );
    let at = position(&sim, css);
    sim.app.world_mut().write_message(Damage {
        target: css,
        attacker: None,
        amount: 1000.0,
        point: at,
        dir: Vec3::NEG_Y,
        hitgroup: Default::default(),
        kind: Default::default(),
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(5);
    let world = sim.app.world();
    let dead = world.get::<Dead>(css).is_some() || world.get::<Health>(css).is_some_and(|h| h.current <= 0.0);
    assert!(dead, "the CS:S character died");
    assert_eq!(seated(&sim, css), None, "dead, out of the seat");
    assert!(!carried(&sim, css));
    assert_eq!(
        sim.app.world().get::<HaloVehicle>(w).unwrap().riders[1],
        None,
        "its seat empty"
    );

    // The soldier gets out: its movement its own again.
    press_use(&mut sim, soldier);
    assert_eq!(seated(&sim, soldier), None, "out");
    assert!(!carried(&sim, soldier));
    assert_eq!(sim.app.world().get::<HaloVehicle>(w).unwrap().riders[0], None);
    sim.seconds(1.0);
    let before = position(&sim, soldier);
    sim.intent(soldier).move_axis = Vec2::Y;
    sim.seconds(1.0);
    sim.intent(soldier).move_axis = Vec2::ZERO;
    let walked = position(&sim, soldier).distance(before);
    assert!(walked > 1.0, "walked {walked} m after getting out");

    // In again; a round restart: out, and the Warthog back at its place.
    sim.app.world_mut().get_mut::<Transform>(soldier).unwrap().translation = seat_at(&sim, w, 0) + Vec3::Y * 0.9;
    sim.ticks(2);
    sim.app.world_mut().get_mut::<Transform>(soldier).unwrap().translation = seat_at(&sim, w, 0) + Vec3::Y * 0.9;
    press_use(&mut sim, soldier);
    assert_eq!(seated(&sim, soldier), Some((w, 0)), "in again");
    restart_round(sim.app.world_mut());
    sim.ticks(2);
    assert_eq!(seated(&sim, soldier), None, "a round restart stands the rider");
    assert!(!carried(&sim, soldier));
    let v = sim.app.world().get::<HaloVehicle>(w).unwrap().clone();
    assert!(v.riders.iter().all(Option::is_none), "no rider left");
    let back = (v.drive.body.position - placed).truncate().length();
    assert!(back < 0.5, "the Warthog {back} WU from its placement");
    // It settles on its springs again.
    sim.seconds(3.0);

    // A rider whose character goes away (a disconnect): its seat empties.
    let leaver = walk_up_and_use(&mut sim, w, 0, combat_arms::game::movement::ID);
    assert_eq!(seated(&sim, leaver), Some((w, 0)));
    sim.app.world_mut().despawn(leaver);
    sim.ticks(2);
    assert_eq!(
        sim.app.world().get::<HaloVehicle>(w).unwrap().riders[0],
        None,
        "the leaver's seat empty"
    );

    // A map change with a rider seated: no seat left behind.
    let rider = walk_up_and_use(&mut sim, w, 0, combat_arms::game::movement::ID);
    assert_eq!(seated(&sim, rider), Some((w, 0)));
    let next = party_kit::load_map("halo_ce:hangemhigh", 0).expect("Hang 'Em High");
    change_map(sim.app.world_mut(), next, MapDebugView::Normal);
    sim.seconds(1.0);
    assert_eq!(seated(&sim, rider), None, "the map change stands the rider");
    assert!(!carried(&sim, rider), "its movement its own again");
    assert!(warthogs(&mut sim).is_empty(), "no Warthog on Hang 'Em High");
}
