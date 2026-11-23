//! DoD:S's explosives on the headless harness (`harness::DodsSim`, the
//! greybox), from specs/dods/weapons.md's and damage.md's test cases: the
//! frag's fuse from the press (thrown after 150 ticks held, it goes off
//! about 5.1 s after the press; held, in the hand), its throw (500 units/s
//! at 10° up from the eye aiming level, 23.3° down aiming 30° down), the
//! blast's 150 × (1 − r / 375) and its reach, the bazooka's shouldering
//! (no rocket lowered, the shot 0.79 s after shouldering, 50 units/s
//! shouldered), its rocket (1300 units/s, 80 units/s² of gravity, the next
//! 1.5 s on; 200 units of reach) and the rifle grenade (2000 units/s along
//! the aim, off 3.63 s after the shot).

use bevy::prelude::*;
use chimera_core::core::{Health, SimClock, Velocity};
use chimera_sim::{
    testkit,
    weapon::{
        Inventory, Weapon, WeaponEvent, WeaponEventKind, give,
        grenade::{Detonated, Projectile, spawn_projectile},
        projectile::Flyer,
    },
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use dods::{
    game::{
        TICK_INTERVAL, movement,
        weapons::{BAZOOKA, FRAG_GER, FRAG_US, RIFLEGREN_US, explosives::Shoulder},
    },
    harness::DodsSim,
};

const U: f32 = 0.0254;

fn sim() -> Sim {
    let mut sim = Sim::new(DodsSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// A player at `at` holding `id`, drawn and settled.
fn holding(sim: &mut Sim, at: Vec3, id: &str) -> (Entity, Entity) {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.0);
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(sim.app.world().get::<Weapon>(w).unwrap().id));
    (p, w)
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

/// `n` ticks, `input` before each: the ticks a blast went off on.
fn blasts(sim: &mut Sim, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Vec<u64> {
    let mut cursor = sim.app.world().resource::<Messages<Detonated>>().get_cursor_current();
    let mut out = Vec::new();
    for t in 0..n {
        input(sim, t);
        sim.ticks(1);
        let m = sim.app.world().resource::<Messages<Detonated>>();
        out.extend(cursor.read(m).map(|_| t));
    }
    out
}

fn projectile(sim: &mut Sim) -> Option<(Entity, Projectile, Vec3)> {
    let mut q = sim.app.world_mut().query::<(Entity, &Projectile, &Transform)>();
    q.iter(sim.app.world()).next().map(|(e, p, t)| (e, p.clone(), t.translation))
}

fn health_points(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current * 100.0
}

#[test]
fn the_frags_fuse_runs_from_the_press() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], FRAG_US);
    // Aimed up a little so it lands near.
    sim.intent(p).pitch = 0.3;
    let off = blasts(&mut sim, 500, |s, t| s.intent(p).fire = t < 150);
    assert_eq!(off.len(), 1, "{off:?}");
    let after = (off[0] + 1) as f64 * TICK_INTERVAL;
    assert!((5.05..5.2).contains(&after), "off {after} s after the press");
}

#[test]
fn a_frag_held_past_its_fuse_goes_off_in_the_hand() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], FRAG_GER);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 10.0;
    let before = health_points(&sim, p);
    let off = blasts(&mut sim, 400, |s, _| s.intent(p).fire = true);
    assert_eq!(off.len(), 1, "{off:?}");
    let after = (off[0] + 1) as f64 * TICK_INTERVAL;
    assert!((5.05..5.2).contains(&after), "off {after} s after the press");
    sim.ticks(2);
    let hurt = before - health_points(&sim, p);
    assert!((130.0..=150.0).contains(&hurt), "the thrower took {hurt}");
}

#[test]
fn the_frag_leaves_the_eye_at_500_and_10_degrees_up() {
    for (pitch, elevation) in [(0.0f32, 10.0f32), (-30.0, -23.3)] {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, greybox::SPAWNS[0], FRAG_US);
        sim.intent(p).pitch = pitch.to_radians();
        let eye = sim.position(p) + sim.state(p).eye_offset;
        let mut found = None;
        for t in 0..40 {
            sim.intent(p).fire = t == 0;
            sim.ticks(1);
            if let Some(g) = projectile(&mut sim) {
                found = Some(g);
                break;
            }
        }
        let (_, g, at) = found.expect("thrown");
        // As it left the hand (its flown ticks' gravity taken back).
        let v = (g.velocity + Vec3::Y * g.flight.gravity * g.ticks as f32 * TICK_INTERVAL as f32) / U;
        assert!((v.length() - 500.0).abs() < 1.0, "{pitch}: {} u/s", v.length());
        let e = (v.y / v.length()).asin().to_degrees();
        assert!((e - elevation).abs() < 0.2, "{pitch}: {e} degrees");
        assert!(at.distance(eye) / U < 12.0, "from the eye: {} units off", at.distance(eye) / U);
    }
}

/// A frag going off on the floor `units` (along -Z) from a still target:
/// the health it took.
fn frag_at(units: f32) -> f32 {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], FRAG_US);
    let t = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 10.0;
    sim.app.world_mut().get_mut::<Velocity>(t).unwrap().0 = Vec3::ZERO;
    let feet = sim.position(t) - Vec3::Y * 36.0 * U;
    let at = feet + Vec3::Z * units * U + Vec3::Y * 2.0 * U;
    let g = spawn_projectile(sim.app.world_mut(), w, at, Vec3::ZERO).unwrap();
    sim.app.world_mut().get_mut::<Projectile>(g).unwrap().fuse = 0.0;
    let _ = p;
    let before = health_points(&sim, t);
    sim.ticks(3);
    before - health_points(&sim, t)
}

#[test]
fn the_blast_falls_linearly_to_375() {
    let d = frag_at(200.0);
    // r to the body's middle, 40 over the feet: 150 × (1 − 203.6 / 375).
    assert!((d - 68.6).abs() < 2.0, "200 units: {d}");
    assert_eq!(frag_at(400.0), 0.0, "400 units");
}

#[test]
fn the_bazooka_fires_only_shouldered() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], BAZOOKA);
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut fired = Vec::new();
    for t in 0..280u64 {
        // A press every other tick (one shot a press).
        sim.intent(p).fire = t % 2 == 0 && t != 60;
        sim.intent(p).secondary = t == 60;
        sim.ticks(1);
        let m = sim.app.world().resource::<Messages<WeaponEvent>>();
        for e in cursor.read(m) {
            if e.owner == p && e.shown() && matches!(e.kind, WeaponEventKind::Fired { .. }) {
                fired.push(t);
            }
        }
        if t == 30 {
            assert!(!sim.app.world().get::<Shoulder>(w).unwrap().shouldered);
        }
    }
    assert!(sim.app.world().get::<Shoulder>(w).unwrap().shouldered, "shouldered");
    let first = *fired.first().expect("a rocket once shouldered");
    let wait = (first - 60) as f64 * TICK_INTERVAL;
    assert!((0.78..0.83).contains(&wait), "first rocket {wait} s after shouldering");
    // The next 1.5 s on (one in the launcher, then its reload).
    let next = fired.get(1).map(|n| (n - first) as f64 * TICK_INTERVAL);
    assert!(next.is_some_and(|n| (1.49..1.56).contains(&n)), "{fired:?}");
    // Shouldered, at most 50 units/s.
    sim.intent(p).fire = false;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let v = sim.velocity(p).xz().length() / U;
    assert!(v <= 50.05 && v > 45.0, "shouldered: {v}");
}

#[test]
fn the_rocket_flies_at_1300_with_a_tenth_of_gravity() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], BAZOOKA);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    sim.intent(p).fire = true;
    let mut flyer = None;
    for _ in 0..10 {
        sim.ticks(1);
        let mut q = sim.app.world_mut().query::<&Flyer>();
        if let Some(f) = q.iter(sim.app.world()).next() {
            flyer = Some(f.clone());
            break;
        }
    }
    let f = flyer.expect("a rocket");
    assert!((f.velocity.length() / U - 1300.0).abs() < 2.0, "{} u/s", f.velocity.length() / U);
    assert!((f.gravity / U - 80.0).abs() < 1e-3);
    assert!(f.blast.as_ref().is_some_and(|b| (b.radius / U - 200.0).abs() < 1e-3), "its blast reaches 200");
}

#[test]
fn the_rifle_grenade_flies_at_2000_and_goes_off_3_63_s_later() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], RIFLEGREN_US);
    sim.intent(p).pitch = 0.4;
    let fired_at = now(&sim);
    let mut seen = None;
    let off = blasts(&mut sim, 300, |s, t| {
        s.intent(p).fire = t == 0;
        if seen.is_none() {
            let mut q = s.app.world_mut().query::<&Projectile>();
            seen = q.iter(s.app.world()).next().map(|g| g.velocity.length() / U);
        }
    });
    assert!(seen.is_some_and(|v| (v - 2000.0).abs() < 1.0), "{seen:?}");
    assert_eq!(off.len(), 1, "{off:?}");
    let after = (off[0] + 1) as f64 * TICK_INTERVAL;
    let _ = fired_at;
    assert!((3.6..3.68).contains(&after), "off {after} s after the shot");
}

#[test]
fn the_explosives_are_the_ladders_grenade_and_launcher_rungs() {
    use chimera_core::game_parts::GunClass;
    let sim = sim();
    let readers = sim.app.world().resource::<chimera_sim::minigame::power::PowerReaders>();
    let set = dods::game::weapons::dods_weapons();
    for (id, class) in [
        (FRAG_US, GunClass::Grenade),
        (FRAG_GER, GunClass::Grenade),
        (RIFLEGREN_US, GunClass::Launcher),
        (dods::game::weapons::RIFLEGREN_GER, GunClass::Launcher),
        (BAZOOKA, GunClass::Launcher),
        (dods::game::weapons::PSCHRECK, GunClass::Launcher),
    ] {
        let w = set.get(id).unwrap();
        assert_eq!(w.class, Some(class), "{id}");
        assert!(!chimera_sim::weapon::harm::harmless(w, &readers.harm), "{id} hurts");
    }
}
