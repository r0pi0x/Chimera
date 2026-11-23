//! The Pyro's airblast (specs/tf2/weapons_more.md 7.4; `weapons::airblast`)
//! on the greybox (`harness::Tf2Sim`): its ammo and refire (20, 0.75 s,
//! the next flame 1.0 s), a standing Soldier at level aim pushed 591.6
//! along and 100 up (the measured push), a reflected rocket turned the
//! Pyro's and mini-critting its own Soldier, a stuck sticky knocked loose
//! (2000 from the Pyro, its owner kept, no re-stick for 1 s), a burning
//! teammate put out and not pushed, and its look (`pyro_blast` once per
//! blast, the stream stopped).

use bevy::prelude::*;
use chimera_core::core::{Health, Team};
use chimera_sim::{
    testkit,
    weapon::{Magazine, WeaponEventKind, projectile::Flyer},
};
use chimera_world::{greybox, map::psys::{StartParticleSystem, StopParticleSystems}};
use tf2::game::weapons::{
    FLAMETHROWER, ROCKET_LAUNCHER, STICKY_LAUNCHER,
    airblast::{AIRBLAST, Deflected},
    flame::{self, Burning},
    sticky::Unstuck,
};

use super::weapons::{health_points, holding, rockets, run, sim, target};

const U: f32 = 0.0254;

fn blasts(events: &[(u64, WeaponEventKind)]) -> Vec<u64> {
    events.iter().filter(|(_, k)| matches!(k, WeaponEventKind::Action(AIRBLAST))).map(|(t, _)| *t).collect()
}

/// 20 ammo a blast, the next 0.75 s (50 ticks) later on a held attack2,
/// the flame not before 1.0 s; refused below 20.
#[test]
fn an_airblast_costs_20_and_comes_every_0_75_s() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], FLAMETHROWER);
    let clip = sim.app.world().get::<Magazine>(w).unwrap().clip;
    let ev = run(&mut sim, p, 60, |s, _| {
        s.intent(p).secondary = true;
        s.intent(p).fire = true;
    });
    assert_eq!(blasts(&ev), [0, 50], "{ev:?}");
    let first_flame = super::weapons::fired(&ev).first().copied();
    assert!(first_flame.is_none_or(|t| t >= 66), "no flame for 1.0 s: {first_flame:?}");
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, clip - 40);
    // Below 20: nothing.
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 19;
    let ev = run(&mut sim, p, 60, |s, _| s.intent(p).secondary = true);
    assert!(blasts(&ev).is_empty(), "{ev:?}");
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 19);
}

/// 7.4's measured push: a standing Soldier 130 units ahead at level aim
/// takes 591.6 along the push and 100 up, then rises under gravity alone.
#[test]
fn a_standing_soldier_at_level_aim_is_pushed_591_6_and_100_up() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let t = target(&mut sim, from, 130.0);
    assert!(sim.state(t).on_ground);
    {
        let mut i = sim.intent(p);
        i.yaw = 0.0;
        i.pitch = 0.0;
    }
    let mut seen = Vec::new();
    run(&mut sim, p, 4, |s, k| {
        s.intent(p).secondary = k == 0;
        seen.push(s.velocity(t) / U);
    });
    seen.push(sim.velocity(t) / U);
    // The tick after the blast's: the push as added.
    let v = seen[1];
    let flat = Vec2::new(v.x, v.z).length();
    assert!((flat - 591.6).abs() < 0.5, "along: {flat} ({v})");
    assert!((v.y - 100.0).abs() < 0.5, "up: {} ({v})", v.y);
    assert!(v.z < 0.0, "away from the Pyro: {v}");
    // Then gravity: 800 units/s² (half a tick's worth at each end).
    let dy = seen[2].y - seen[1].y;
    assert!((dy + 800.0 * 0.015).abs() < 1.0, "fell {dy} in a tick");
    assert!(!sim.state(t).on_ground);
    let push = sim.app.world().get::<chimera_core::core::ExternalPush>(t).copied().unwrap();
    assert!(push.no_footing > 0.0 && push.air_control > 0.0, "{push:?}");
}

/// 7.4: an enemy rocket in reach turns toward the Pyro's aim point at its
/// speed, becomes his (owner) and mini-crits its own Soldier: 90 x 1.35.
#[test]
fn a_reflected_rocket_is_the_pyros_and_mini_crits() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let (s, _) = holding(&mut sim, from - Vec3::Z * 512.0 * U, ROCKET_LAUNCHER);
    sim.app.world_mut().entity_mut(p).insert(Team(2));
    sim.app.world_mut().entity_mut(s).insert(Team(3));
    sim.app.world_mut().get_mut::<Health>(s).unwrap().current = 10.0;
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 10.0;
    let (pc, sc) = (sim.position(p), sim.position(s));
    testkit::aim_at(&mut sim, s, pc);
    testkit::aim_at(&mut sim, p, sc);
    let eye = pc + sim.state(p).eye_offset;
    let mut blasted = false;
    for k in 0..60 {
        sim.intent(s).fire = k == 0;
        blasted = rockets(&mut sim, s).iter().any(|(_, at, _)| at.distance(eye) < 180.0 * U);
        sim.intent(p).secondary = blasted;
        sim.ticks(1);
        if blasted {
            break;
        }
    }
    assert!(blasted, "the rocket came near");
    let mine = rockets(&mut sim, p);
    assert_eq!(mine.len(), 1, "the rocket is the Pyro's now");
    let (r, _, v) = mine[0];
    assert!((v.length() / U - 1100.0).abs() < 1.0, "its speed kept: {}", v.length() / U);
    assert!(v.normalize().dot((sc - pc).normalize()) > 0.95, "back toward the Soldier: {v}");
    assert_eq!(sim.app.world().get::<Deflected>(r), Some(&Deflected(1)));
    let before = health_points(&sim, s);
    sim.seconds(1.0);
    let took = before - health_points(&sim, s);
    assert!((took - 122.0).abs() < 1.5, "a mini-crit 90 x 1.35: took {took}");
}

/// 7.4: a stuck enemy sticky is knocked loose at 2000 out from the Pyro,
/// stays its owner's and doesn't stick again for 1 s.
#[test]
fn a_stuck_sticky_is_knocked_loose_and_keeps_its_owner() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let (d, _) = holding(&mut sim, from - Vec3::Z * 300.0 * U, STICKY_LAUNCHER);
    let floor = sim.position(p).y + sim.state(p).hull_min.y;
    testkit::aim_at(&mut sim, d, from - Vec3::Z * 150.0 * U + Vec3::Y * (floor - from.y));
    run(&mut sim, d, 120, |s, k| s.intent(d).fire = k < 3);
    let sticky = {
        let mut q = sim.app.world_mut().query::<(Entity, &Flyer)>();
        q.iter(sim.app.world()).find(|(_, f)| f.owner == Some(d)).map(|(e, f)| (e, f.stuck_until.is_some()))
    };
    let (e, stuck) = sticky.expect("a sticky");
    assert!(stuck, "stuck on the floor");
    // Put it 70 units in front of the Pyro's centre (as if on a wall),
    // and aim at it.
    let spot = sim.position(p) - Vec3::Z * 70.0 * U;
    sim.app.world_mut().get_mut::<Transform>(e).unwrap().translation = spot;
    testkit::aim_at(&mut sim, p, spot);
    run(&mut sim, p, 1, |s, _| s.intent(p).secondary = true);
    let f = sim.app.world().get::<Flyer>(e).expect("still there").clone();
    assert_eq!(f.owner, Some(d), "still the Demoman's");
    assert!(f.stuck_until.is_none(), "loose");
    assert!(sim.app.world().get::<Unstuck>(e).is_some());
    assert!(sim.app.world().get::<Deflected>(e).is_none(), "stickies don't mini-crit");
    // About 2000 units/s (a tick's gravity and drag on it), straight out
    // from him.
    let speed = f.velocity.length() / U;
    assert!((1970.0..=2010.0).contains(&speed), "{speed}");
    assert!(f.velocity.normalize().dot(Vec3::NEG_Z) > 0.99, "out from the Pyro: {}", f.velocity);
    for _ in 0..60 {
        sim.ticks(1);
        let Some(f) = sim.app.world().get::<Flyer>(e) else { break };
        assert!(f.stuck_until.is_none(), "no re-stick within 1 s");
    }
}

/// 7.4: a burning teammate in reach is put out and not pushed.
#[test]
fn a_burning_teammate_is_put_out_and_not_pushed() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let t = target(&mut sim, from, 100.0);
    sim.app.world_mut().entity_mut(p).insert(Team(2));
    sim.app.world_mut().entity_mut(t).insert((Team(2), flame::ignited(0.0, p, FLAMETHROWER, &flame::stock_params())));
    {
        let mut i = sim.intent(p);
        i.yaw = 0.0;
        i.pitch = 0.0;
    }
    run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
    assert!(sim.app.world().get::<Burning>(t).is_none(), "put out");
    assert!(sim.velocity(t).length() < 0.01, "not pushed: {}", sim.velocity(t));
}

/// flames.md 3.1.1: one `pyro_blast` per blast at the muzzle, the stream
/// stopped (tagged with the gun).
#[test]
fn each_airblast_shows_pyro_blast_once() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], FLAMETHROWER);
    let mut starts = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut stops = sim.app.world().resource::<Messages<StopParticleSystems>>().get_cursor_current();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for k in 0..60 {
        sim.intent(p).secondary = true;
        sim.ticks(1);
        let world = sim.app.world();
        a.extend(starts.read(world.resource::<Messages<StartParticleSystem>>()).filter(|s| s.name == "pyro_blast").map(|_| k));
        b.extend(stops.read(world.resource::<Messages<StopParticleSystems>>()).filter(|s| s.tag == w).map(|_| k));
    }
    assert_eq!(a, [0, 50]);
    assert_eq!(b, [0, 50], "the stream stopped with each");
}
