//! Launchers through the real fire path (special_weapons.md 1.1, 2, 3, 4;
//! weapon_firing.md Recoil): the 2026-10-31 sighting of a level-aimed LAW
//! rocket that climbed steeply and appeared about 75 frames after the
//! press. Neither is the game's: a level shot leaves level (the cone's
//! thousandths only) and a rocket flies straight (`Gravity` 0), the view
//! kick of 5.5 degrees comes after the shot and never bends it; the round
//! leaves on the fire key at 0 ms of the fire animation, so a press on a
//! drawn launcher launches within three updates and a press during the
//! draw launches when the draw (the LAW's 1333 ms, about 80 frames at 60
//! Hz) ends: that wait was the screenshot run pressing a launcher it had
//! just been given. Shells fall under the object gravity (the M32 family),
//! flames fly straight at 500 u/s.

use bevy::prelude::*;
use chimera_core::core::SimClock;
use chimera_sim::weapon;
use chimera_world::harness::Sim;
use combat_arms::game::special::CaObject;

use super::special_weapons::{FLAME, LAW, M32, RPG, armed, player, sim};

/// The first projectile of `id` after the press: seconds from the press
/// and its velocity (u/s).
fn first_projectile(sim: &mut Sim, id: &str, limit: f64) -> Option<(f64, Vec3)> {
    let t0 = sim.app.world().resource::<SimClock>().now;
    while sim.app.world().resource::<SimClock>().now - t0 < limit {
        sim.ticks(1);
        let now = sim.app.world().resource::<SimClock>().now;
        let w = sim.app.world_mut();
        if let Some(o) = w.query::<&CaObject>().iter(w).find(|o| o.weapon == id) {
            return Some((now - t0, o.velocity));
        }
    }
    None
}

/// Cone of the rocket ammo: the meters stay at 0, so a rocket leaves within
/// 0.001 of the aim on each axis (special_weapons.md 2, Spread).
const CONE: f32 = 0.0015;

/// A level LAW or RPG-7 shot leaves level at 1300 u/s (no gravity, the
/// shooter's speed not added), the view kick not in it, on the first
/// update after the press.
#[test]
fn level_rocket_leaves_level() {
    for id in [LAW, RPG] {
        let mut sim = sim();
        let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
        armed(&mut sim, p, id);
        assert_eq!(sim.intent(p).pitch, 0.0, "level aim");
        sim.intent(p).fire = true;
        let (t, v) = first_projectile(&mut sim, id, 1.0).unwrap_or_else(|| panic!("{id}: no rocket"));
        sim.intent(p).fire = false;
        assert!(t <= 0.05, "{id}: leaves within three updates of the press: {t}");
        assert!((v.length() - 1300.0).abs() < 1.0, "{id}: 1300 u/s: {v:?}");
        assert!(v.y.abs() / 1300.0 < CONE, "{id}: level, not climbing: {v:?}");
        assert!(v.x.abs() / 1300.0 < CONE && v.z > 1299.0, "{id}: straight ahead (yaw pi looks down +Z): {v:?}");
        // Straight flight: height unchanged after ten steps.
        let before = sim.app.world_mut().query::<&CaObject>().iter(sim.app.world()).next().map(|o| o.pos);
        sim.seconds(0.5);
        let w = sim.app.world_mut();
        if let (Some(b), Some(o)) = (before, w.query::<&CaObject>().iter(w).find(|o| o.weapon == id)) {
            let climbed = (o.pos.y - b.y) / (o.pos - b).length().max(1.0);
            assert!(climbed.abs() < CONE, "{id}: flight stays level: {climbed}");
        }
    }
}

/// The view kick (5.5 degrees up, special_weapons.md 2) follows the shot:
/// the angles the shot left along are the ones aimed, and the rocket's
/// direction is not changed by the kick that comes after.
#[test]
fn rocket_ignores_its_own_kick() {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    armed(&mut sim, p, LAW);
    sim.intent(p).fire = true;
    let (_, v) = first_projectile(&mut sim, LAW, 1.0).expect("a rocket");
    sim.intent(p).fire = false;
    sim.ticks(10);
    let w = sim.app.world_mut();
    let o = w.query::<&CaObject>().iter(w).find(|o| o.weapon == LAW).expect("still flying");
    let now_dir = o.velocity.normalize();
    assert!((now_dir - v.normalize()).length() < 1e-4, "the kick does not bend the flight: {v:?} then {:?}", o.velocity);
}

/// A press during the draw launches when the draw ends: the LAW's 1333 ms,
/// then the fire key at 0 ms (the sighting's wait of about 75 frames).
#[test]
fn press_during_draw_waits_for_the_draw() {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    let w = weapon::give(sim.app.world_mut(), p, LAW).expect("law");
    let draw = sim.app.world().get::<weapon::Weapon>(w).expect("weapon").draw_time as f64;
    assert!((draw - 1.333).abs() < 0.01, "the LAW draws in 1333 ms: {draw}");
    sim.intent(p).fire = true;
    let (t, _) = first_projectile(&mut sim, LAW, 3.0).expect("a rocket");
    sim.intent(p).fire = false;
    assert!(t >= draw - 0.05, "not before the draw ends: {t}");
    assert!(t <= draw + 0.1, "within a few updates of the draw's end: {t} (draw {draw})");
}

/// Shells drop under the object gravity, flames and rockets do not: a
/// level M32 shell leaves at 1550 u/s with no vertical speed, then falls;
/// a level flame puff flies at 500 u/s and stays level.
#[test]
fn shells_fall_flames_and_rockets_do_not() {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    armed(&mut sim, p, M32);
    sim.intent(p).fire = true;
    let (t, v) = first_projectile(&mut sim, M32, 1.0).expect("a shell");
    sim.intent(p).fire = false;
    assert!(t <= 0.05, "{t}");
    assert!((v.length() - 1550.0).abs() < 2.0, "1550 u/s: {v:?}");
    assert!(v.y.abs() / 1550.0 < 0.01, "level at the muzzle: {v:?}");
    let w = sim.app.world_mut();
    let y0 = w.query::<&CaObject>().iter(w).find(|o| o.weapon == M32).unwrap().velocity.y;
    sim.ticks(10);
    let w = sim.app.world_mut();
    if let Some(o) = w.query::<&CaObject>().iter(w).find(|o| o.weapon == M32) {
        assert!(o.velocity.y < y0 - 1.0, "the shell falls: {y0} then {}", o.velocity.y);
    }

    let mut sim = self::sim();
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    armed(&mut sim, p, FLAME);
    sim.intent(p).fire = true;
    let (t, v) = first_projectile(&mut sim, FLAME, 2.0).expect("a flame puff");
    sim.intent(p).fire = false;
    assert!(t <= 0.5, "{t}");
    assert!((v.length() - 500.0).abs() < 1.0, "500 u/s: {v:?}");
    assert!(v.y.abs() / 500.0 < 0.01, "level: {v:?}");
}
