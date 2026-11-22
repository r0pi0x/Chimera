//! Halo's vehicles in the party (Blood Gulch from the disc image, on the
//! party's simulation, `CaSim` with `PartyPartsPlugin`). By default (the
//! console variable `halo_ce:party_vehicles` 0) none is placed: no vehicle
//! entity, prop or `HaloVehicle`, the map's other props as with them. With
//! it on (`PartyVehicles` in the load's context) the scenario's two
//! Warthogs are there; a Combat Arms soldier (another game's movement)
//! takes the driver's seat with the use key, its own movement suspended
//! (the engine's `Carried`: jumping and walking move it nowhere but with
//! the Warthog), drives it twenty-odd metres in four seconds and gets out
//! beside it, its movement its own again; a CS:S character in the
//! passenger seat dies there and is out of it, its seat empty; a rider
//! seated at a round restart stands and the Warthog is back at its
//! placement; a rider whose character goes away (a disconnect) leaves its
//! seat empty; a map change with a rider seated leaves no seat behind.
//! A CS:S character drives a Ghost; a Scorpion takes a Combat Arms driver
//! and four riders of both games, carries them and lets them out.
//! Passes with a note without the disc image.

use bevy::prelude::*;
use chimera_core::core::{Carried, Damage, Dead, Health, restart_round};
use chimera_world::{
    harness::Sim,
    map::{LoadContext, MapData, MapDebugView, MapLoadOptions, MapSimPlugin, change_map},
};
use combat_arms::harness::CaSim;
use halo_ce::game::vehicles::{
    HaloVehicle, Seated, VehicleDefs, level,
    place::{PartyVehicles, Placed},
};
use party_roster::PartyPartsPlugin;

fn disc() -> bool {
    let there = halo_ce::game::INFO.mount().is_some();
    if !there {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
    }
    there
}

/// The vehicles of kind `name` (`warthog`, `ghost`, `scorpion`), west to
/// east: (entity, its state).
fn of_kind(sim: &mut Sim, name: &str) -> Vec<(Entity, HaloVehicle)> {
    let w = sim.app.world_mut();
    let defs = w.resource::<VehicleDefs>().clone();
    let mut q = w.query::<(Entity, &HaloVehicle)>();
    let mut out: Vec<(Entity, HaloVehicle)> = q
        .iter(w)
        .filter(|(_, v)| defs.0[v.def].0.name == name)
        .map(|(e, v)| (e, v.clone()))
        .collect();
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

/// Blood Gulch as the party loads it, with `halo_ce:party_vehicles` `on`.
fn blood_gulch(on: bool) -> MapData {
    let context = LoadContext::default();
    if on {
        context.insert(PartyVehicles(true));
    }
    let options = MapLoadOptions {
        context,
        ..MapLoadOptions::hdr(0)
    };
    party_roster::load_map_with("halo_ce:bloodgulch", &options).expect("Blood Gulch")
}

fn party_on_blood_gulch() -> Sim {
    let map = blood_gulch(true);
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
    let found = of_kind(&mut sim, "warthog");
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
    let next = party_roster::load_map("halo_ce:hangemhigh", 0).expect("Hang 'Em High");
    change_map(sim.app.world_mut(), next, MapDebugView::Normal);
    sim.seconds(1.0);
    assert_eq!(seated(&sim, rider), None, "the map change stands the rider");
    assert!(!carried(&sim, rider), "its movement its own again");
    assert!(of_kind(&mut sim, "warthog").is_empty(), "no Warthog on Hang 'Em High");
}

/// How far each rider is off where it rode at first, in its seat (m).
fn drift(sim: &Sim, v: Entity, riders: &[(Entity, usize, Vec3)]) -> f32 {
    riders
        .iter()
        .map(|(p, seat, over)| (position(sim, *p) - seat_at(sim, v, *seat) - *over).length())
        .fold(0.0, f32::max)
}

#[test]
fn another_games_characters_ride_a_ghost_and_a_scorpion() {
    if !disc() {
        return;
    }
    let mut sim = party_on_blood_gulch();
    let ghosts = of_kind(&mut sim, "ghost");
    let scorpions = of_kind(&mut sim, "scorpion");
    assert_eq!(
        (ghosts.len(), scorpions.len()),
        (4, 2),
        "Blood Gulch's Ghosts and Scorpions in the party"
    );

    // A CS:S character drives a Ghost.
    let g = ghosts[0].0;
    let css = walk_up_and_use(&mut sim, g, 0, cs_source::game::movement::ID);
    assert_eq!(seated(&sim, css), Some((g, 0)), "the CS:S character drives the Ghost");
    assert!(carried(&sim, css));
    let over = position(&sim, css) - seat_at(&sim, g, 0);
    let start = position(&sim, g);
    sim.intent(css).move_axis = Vec2::Y;
    sim.seconds(3.0);
    sim.intent(css).move_axis = Vec2::ZERO;
    let gone = position(&sim, g).distance(start);
    let off = drift(&sim, g, &[(css, 0, over)]);
    eprintln!("the Ghost went {gone} m, its rider {off} m off its seat");
    assert!(gone > 5.0, "the Ghost moved {gone} m");
    assert!(off < 0.01, "rode {off} m off its seat");
    sim.seconds(2.0);
    press_use(&mut sim, css);
    assert_eq!(seated(&sim, css), None, "out of the Ghost");

    // A Scorpion: a Combat Arms driver, then four riders of both games.
    let sc = scorpions[0].0;
    let seats = sim.app.world().get::<HaloVehicle>(sc).unwrap().riders.len();
    assert_eq!(seats, 5, "the Scorpion's driver and four riders");
    let mut riders = Vec::new();
    for seat in 0..seats {
        let movement = if seat % 2 == 0 {
            combat_arms::game::movement::ID
        } else {
            cs_source::game::movement::ID
        };
        let p = walk_up_and_use(&mut sim, sc, seat, movement);
        assert_eq!(
            seated(&sim, p),
            Some((sc, seat)),
            "a rider in the Scorpion's seat {seat}"
        );
        assert!(carried(&sim, p), "seat {seat}: its movement suspended");
        riders.push((p, seat, position(&sim, p) - seat_at(&sim, sc, seat)));
    }
    let driver = riders[0].0;
    let start = position(&sim, sc);
    sim.intent(driver).move_axis = Vec2::Y;
    sim.seconds(3.0);
    sim.intent(driver).move_axis = Vec2::ZERO;
    let gone = position(&sim, sc).distance(start);
    let off = drift(&sim, sc, &riders);
    eprintln!("the Scorpion went {gone} m, its riders at most {off} m off their seats");
    assert!(gone > 2.0, "the Scorpion moved {gone} m");
    assert!(off < 0.01, "riders {off} m off their seats");
    sim.seconds(2.0);
    for (p, seat, _) in riders.iter().rev() {
        press_use(&mut sim, *p);
        assert_eq!(seated(&sim, *p), None, "seat {seat}'s rider out");
        assert!(!carried(&sim, *p));
    }
    assert!(
        sim.app
            .world()
            .get::<HaloVehicle>(sc)
            .unwrap()
            .riders
            .iter()
            .all(Option::is_none)
    );
}

/// The default: no vehicle at all on Blood Gulch in a party app (no map
/// entity, no prop, no `HaloVehicle`), every other prop as with vehicles
/// (the vehicles' props are the last ones; the scenery's before them the
/// same).
#[test]
fn by_default_the_party_places_no_vehicle() {
    if !disc() {
        return;
    }
    let off = party_roster::load_map("halo_ce:bloodgulch", 0).expect("Blood Gulch");
    let on = blood_gulch(true);
    let placed = |m: &MapData| m.entities.iter().filter_map(Placed::from_entity).collect::<Vec<_>>();
    let (p_off, p_on) = (placed(&off), placed(&on));
    eprintln!(
        "off: {} props, {} vehicles; on: {} props, {} vehicles",
        off.props.len(),
        p_off.len(),
        on.props.len(),
        p_on.len()
    );
    assert!(p_off.is_empty(), "no vehicle entity by default");
    assert_eq!(p_on.len(), 8, "with the switch: 2 Warthogs, 4 Ghosts, 2 Scorpions");
    assert_eq!(
        on.props.len(),
        off.props.len() + p_on.len(),
        "one prop per vehicle, no other prop"
    );
    assert!(
        p_on.iter().all(|p| p.prop >= off.props.len()),
        "the vehicles' props come last"
    );
    for (i, (a, b)) in off.props.iter().zip(&on.props).enumerate() {
        assert!(
            a.translation == b.translation && a.rotation == b.rotation,
            "prop {i} the same both ways"
        );
    }
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(off.clone()), PartyPartsPlugin));
    sim.seconds(1.0);
    let w = sim.app.world_mut();
    let vehicles = w.query::<&HaloVehicle>().iter(w).count();
    let props: Vec<usize> = w
        .query::<&chimera_world::map::PropIndex>()
        .iter(w)
        .map(|p| p.0)
        .collect();
    assert_eq!(vehicles, 0, "no HaloVehicle");
    assert!(props.iter().all(|&i| i < off.props.len()), "no prop beyond the map's");
}
