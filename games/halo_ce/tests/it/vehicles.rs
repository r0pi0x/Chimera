//! Halo's vehicles in the simulation (`HaloSim` on the movement tests'
//! flat floor, `crate::movement::T`): a Warthog spawned by
//! `vehicles::spawn_vehicle` settles on its springs, a player presses the
//! use key beside the driver's seat and is seated, drives ten metres with
//! the forward key and gets out; a second player takes the passenger seat
//! and is carried; the Warthog's top speed under a 60 Hz engine tick; the
//! splatter rule's cases (vehicles.md test cases) and a moving Warthog
//! hurting a player in its way.

use bevy::prelude::*;
use chimera_core::core::Intent;
use halo_ce::game::damage::HaloHit;
use halo_ce::game::maps::space;
use halo_ce::game::vehicles::{self, HaloVehicle, Seated, seats};
use halo_ce::game::{TICK_INTERVAL, movement};

use crate::movement::{T, floor};

fn eng(p: Vec3) -> Vec3 {
    space::point(p.to_array())
}

fn game(p: Vec3) -> Vec3 {
    Vec3::from(space::to_game(p))
}

/// A player standing at `at` (WU) with a Warthog whose origin is at
/// `warthog` facing +x, at `hz`, settled for a second.
fn setup(at: Vec3, warthog: Vec3, hz: f64) -> (T, Entity) {
    let mut t = T::at_rate(vec![floor()], at, hz);
    let w = vehicles::spawn_vehicle(t.sim.app.world_mut(), "warthog", warthog, 0.0).unwrap();
    t.sim.ticks((hz as u64).max(30));
    (t, w)
}

fn vehicle(t: &T, w: Entity) -> HaloVehicle {
    t.sim.app.world().get::<HaloVehicle>(w).unwrap().clone()
}

fn press_use(t: &mut T, p: Entity) {
    t.sim.intent(p).use_key = true;
    t.sim.ticks(1);
    t.sim.intent(p).use_key = false;
    t.sim.ticks(1);
}

fn seated(t: &T, p: Entity) -> Option<Seated> {
    t.sim.app.world().get::<Seated>(p).copied()
}

fn position(t: &T, e: Entity) -> Vec3 {
    game(t.sim.app.world().get::<Transform>(e).unwrap().translation)
}

#[test]
fn a_warthog_settles_on_its_springs() {
    let (mut t, w) = setup(Vec3::new(0.0, 5.0, 0.0), Vec3::ZERO, 1.0 / TICK_INTERVAL);
    t.sim.ticks(60);
    let v = vehicle(&t, w);
    // Created 0.55 WU up, it falls onto its springs and falls asleep mid-
    // bounce, frozen 0.062 above its equilibrium (centre of mass 0.35360):
    // vehicles.md's spawn case and quirk, here on a trimesh floor through
    // the engine's point projection.
    assert!(v.drive.asleep);
    assert!(
        (v.drive.body.position.z - (0.3536 + 0.062)).abs() < 0.01,
        "{:?}",
        v.drive.body.position
    );
    assert!(v.drive.body.up.z > 0.999);
    // The prop's transform is the model origin.
    let origin = position(&t, w);
    assert!((origin.z - (0.0318 + 0.062)).abs() < 0.03, "origin {origin}");
}

#[test]
fn enter_drive_ten_metres_and_exit() {
    // The driver's seat (the fallback marker, no model) is on the left (+y).
    let (mut t, w) = setup(Vec3::new(0.0, 0.8, 0.0), Vec3::ZERO, 1.0 / TICK_INTERVAL);
    let p = t.p;
    press_use(&mut t, p);
    let s = seated(&t, p).expect("seated");
    assert_eq!((s.vehicle, s.seat), (w, 0), "the driver's seat");
    let start = vehicle(&t, w).drive.body.position;
    {
        let mut i = t.sim.intent(p);
        i.yaw = space::yaw(0.0);
        i.move_axis = Vec2::new(0.0, 1.0);
    }
    let mut ticks = 0;
    while (vehicle(&t, w).drive.body.position - start).length() * halo_ce::game::METERS_PER_UNIT < 10.0 {
        t.sim.ticks(1);
        ticks += 1;
        assert!(ticks < 300, "ten metres within 10 s");
    }
    let v = vehicle(&t, w);
    assert!((v.drive.body.position - start).dot(Vec3::X) > 0.95 * (v.drive.body.position - start).length());
    // The rider rides along at its seat.
    let rider = position(&t, p);
    assert!(
        (rider - v.drive.body.position).length() < 2.0,
        "rider {rider} vehicle {:?}",
        v.drive.body.position
    );
    // Stop and get out beside it.
    t.sim.intent(p).move_axis = Vec2::ZERO;
    t.sim.intent(p).jump = true;
    t.sim.ticks(90);
    t.sim.intent(p).jump = false;
    press_use(&mut t, p);
    assert!(seated(&t, p).is_none());
    assert!(vehicle(&t, w).riders.iter().all(Option::is_none));
    t.sim.ticks(30);
    let out = position(&t, p);
    let v = vehicle(&t, w);
    assert!(out.y > v.drive.body.position.y + 0.5, "out on the driver's side: {out}");
    assert!((out.x - v.drive.body.position.x).abs() < 1.5);
    assert!(out.z > -0.1 && out.z < 1.0, "standing on the floor: {out}");
}

#[test]
fn a_passenger_is_carried() {
    let (mut t, w) = setup(Vec3::new(0.0, 0.8, 0.0), Vec3::ZERO, 1.0 / TICK_INTERVAL);
    let driver = t.p;
    let passenger = t.sim.spawn_character(eng(Vec3::new(0.0, -0.8, 0.35)), movement::ID);
    t.sim.ticks(5);
    press_use(&mut t, driver);
    press_use(&mut t, passenger);
    assert_eq!(seated(&t, passenger).map(|s| s.seat), Some(1), "the passenger seat");
    let before = position(&t, passenger);
    {
        let mut i = t.sim.intent(driver);
        i.yaw = space::yaw(0.0);
        i.move_axis = Vec2::new(0.0, 1.0);
    }
    t.sim.ticks(90);
    let after = position(&t, passenger);
    let v = vehicle(&t, w);
    assert!(after.x - before.x > 2.0, "carried forward: {before} to {after}");
    assert!((after - v.drive.body.position).length() < 2.0);
}

#[test]
fn top_speed_under_a_60_hz_tick() {
    let (mut t, w) = setup(Vec3::new(0.0, 0.8, 0.0), Vec3::ZERO, 60.0);
    let p = t.p;
    press_use(&mut t, p);
    assert!(seated(&t, p).is_some());
    {
        let mut i = t.sim.intent(p);
        i.yaw = space::yaw(0.0);
        i.move_axis = Vec2::new(0.0, 1.0);
    }
    t.sim.ticks(600);
    let v = vehicle(&t, w);
    let speed = v.drive.body.velocity.dot(v.drive.body.forward);
    // The spec's top speed 0.23367 WU/t (7.01 WU/s) in Halo ticks.
    assert!((speed - 0.23367).abs() < 0.002, "top speed {speed}");
}

#[test]
fn the_splatter_rule() {
    // vehicles.md's cases (the pill fix succeeding).
    assert!(vehicles::splatters(Vec3::new(0.05, 0.0, 0.0), Vec3::ZERO, false));
    assert!(!vehicles::splatters(Vec3::new(0.03, 0.0, 0.0), Vec3::ZERO, false));
    assert!(!vehicles::splatters(Vec3::new(0.2, 0.0, 0.0), Vec3::ZERO, true));
    // The passenger seat's yaw arc.
    let r = [-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2];
    assert!((seats::clamp_yaw(100f32.to_radians(), r) - 90f32.to_radians()).abs() < 1e-5);
    assert!((seats::clamp_yaw(200f32.to_radians(), r) + 90f32.to_radians()).abs() < 1e-5);
    assert!((seats::clamp_yaw(45f32.to_radians(), r) - 45f32.to_radians()).abs() < 1e-5);
}

#[derive(Resource, Default)]
struct Hits(Vec<HaloHit>);

fn log(mut m: MessageReader<HaloHit>, mut hits: ResMut<Hits>) {
    hits.0.extend(m.read().cloned());
}

#[test]
fn a_moving_warthog_hurts_a_player_in_its_way() {
    let (mut t, w) = setup(Vec3::new(0.0, 0.8, 0.0), Vec3::ZERO, 1.0 / TICK_INTERVAL);
    t.sim.app.init_resource::<Hits>().add_systems(Last, log);
    let driver = t.p;
    let victim = t.sim.spawn_character(eng(Vec3::new(8.0, 0.0, 0.35)), movement::ID);
    press_use(&mut t, driver);
    {
        let mut i = t.sim.intent(driver);
        i.yaw = space::yaw(0.0);
        i.move_axis = Vec2::new(0.0, 1.0);
    }
    let _ = w;
    let mut hit = None;
    for _ in 0..240 {
        t.sim.ticks(1);
        let hits = &t.sim.app.world().resource::<Hits>().0;
        if let Some(h) = hits.iter().find(|h| h.target == victim) {
            hit = Some(h.clone());
            break;
        }
    }
    let h = hit.expect("the Warthog hits the player");
    assert_eq!(h.effect, halo_ce::game::vitality::ids::VEHICLE_COLLISION);
    assert_eq!(h.instigator, Some(driver), "owned by the driver");
    let _: &Intent = t.sim.app.world().get::<Intent>(driver).unwrap();
}
