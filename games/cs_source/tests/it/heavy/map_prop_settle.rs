//! Knocked physics props settle and sleep on a real CS:S map, headless:
//! de_dust2's props, each given a bullet-like push, come to rest and go to
//! sleep within a few seconds (as in Source), rather than rocking on the
//! ground every tick; busy stock maps have no body awake 10 s after loading
//! (props on many-cornered floors, props placed wedged), and such a prop
//! still wakes and moves when shot. Skipped without an install.

use avian3d::prelude::*;
use bevy::prelude::*;
use crate::stack::{
    games::{
        self, cs_source,
        cs_source::movement::{SourceMovementPlugin, to_engine},
    },
    harness::{CsSim, Sim},
    map::{MapData, MapPlugin, PhysicsProp, PropIndex},
    mount::config::LocalConfig,
};

fn dust2() -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()?
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return None;
    }
    Some(games::load_map_lean("cs_source:de_dust2").expect("load de_dust2"))
}

/// The dynamic physics props: (prop index, entity).
fn bodies(sim: &mut Sim) -> Vec<(usize, Entity)> {
    let world = sim.app.world_mut();
    let mut v: Vec<_> = world
        .query_filtered::<(Entity, &PropIndex, &RigidBody), With<PhysicsProp>>()
        .iter(world)
        .filter(|(_, _, rb)| rb.is_dynamic())
        .map(|(e, i, _)| (i.0, e))
        .collect();
    v.sort();
    v
}

/// A shot's push (`weapon::deliver`): an AK-47-sized bullet impulse
/// (damage 36 * 75 force scale, Source units) a little above the body's
/// centre, sideways and a bit down.
fn knock(sim: &mut Sim, body: Entity, dir: Vec3) {
    let world = sim.app.world_mut();
    let centre = world.get::<ComputedCenterOfMass>(body).map_or(Vec3::ZERO, |c| c.0);
    let at = world.get::<Transform>(body).unwrap().transform_point(centre) + Vec3::Y * 0.05;
    let impulse = to_engine(dir.normalize() * 36.0 * 75.0);
    let mut q = world.query::<Forces>();
    let mut forces = q.get_mut(world, body).unwrap();
    forces.apply_linear_impulse_at_point(impulse, at);
}

/// (linear m/s, angular rad/s, asleep)
fn motion(sim: &mut Sim, body: Entity) -> (f32, f32, bool) {
    let w = sim.app.world();
    (
        w.get::<LinearVelocity>(body).unwrap().0.length(),
        w.get::<AngularVelocity>(body).unwrap().0.length(),
        w.get::<Sleeping>(body).is_some(),
    )
}

/// Ticks until every one of `knocked` sleeps, up to `limit` seconds:
/// how long the last took (None if some are still awake) and those still
/// awake with their speeds (index, m/s, rad/s).
fn settle(sim: &mut Sim, knocked: &[(usize, Entity)], limit: f64) -> (Option<f64>, Vec<(usize, f32, f32)>) {
    let ticks = (limit / cs_source::TICK_INTERVAL).round() as u64;
    for t in 1..=ticks {
        sim.ticks(1);
        if knocked.iter().all(|(_, e)| motion(sim, *e).2) {
            return (Some(t as f64 * cs_source::TICK_INTERVAL), Vec::new());
        }
    }
    let awake = knocked
        .iter()
        .filter_map(|(i, e)| {
            let (l, a, asleep) = motion(sim, *e);
            (!asleep).then_some((*i, l, a))
        })
        .collect();
    (None, awake)
}

fn sim(map: MapData) -> Sim {
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim
}

/// de_dust2's prop 390, an oil drum standing on the crease between two
/// displacement triangles near (21, 0, -20) (engine units), which the
/// capacity test's bots knocked into rocking for good: pushed like a
/// bullet hit, from each side in turn, it sleeps within 4 s each time.
#[test]
fn a_knocked_oil_drum_settles_and_sleeps() {
    let Some(map) = dust2() else { return };
    let mut sim = sim(map);
    sim.seconds(4.0);
    let all = bodies(&mut sim);
    let &(_, body) = all.iter().find(|(i, _)| *i == 390).expect("prop 390 is a physics prop");
    for dir in [Vec3::X, Vec3::NEG_Y, Vec3::NEG_X, Vec3::Y] {
        knock(&mut sim, body, dir - Vec3::Z * 0.3);
        sim.ticks(1);
        assert!(!motion(&mut sim, body).2, "the push wakes it");
        let (took, awake) = settle(&mut sim, &[(390, body)], 4.0);
        let at = sim.app.world().get::<Transform>(body).unwrap().translation;
        eprintln!("pushed {dir}: asleep after {took:?} s at {at}");
        assert!(
            awake.is_empty(),
            "pushed {dir}, still awake 4 s later at {at}: {awake:?}"
        );
    }
}

/// Every de_dust2 physics prop, knocked like a bullet hit (all at once,
/// each a different way), sleeps within 24 s: oil drum 375, knocked over
/// on the ramp near (34, 3, -70), rolls about 19 m down it and the gully
/// below and sleeps after about 18 s; every other prop within a few.
#[test]
fn every_knocked_dust2_prop_settles() {
    let Some(map) = dust2() else { return };
    let mut sim = sim(map);
    sim.seconds(4.0);
    let all = bodies(&mut sim);
    assert!(all.len() >= 60, "{} dynamic props", all.len());
    for (n, (_, e)) in all.iter().enumerate() {
        let a = n as f32 * 2.4;
        knock(&mut sim, *e, Vec3::new(a.cos(), a.sin(), -0.3));
    }
    sim.ticks(1);
    let (took, awake) = settle(&mut sim, &all, 24.0);
    eprintln!("{} props asleep after {took:?} s", all.len());
    assert!(
        awake.is_empty(),
        "still awake 24 s after the push (index, m/s, rad/s): {awake:?}"
    );
}


/// Most dynamic bodies a busy stock map may have awake 10 s after it
/// loads (`map_settles_with_no_body_awake`). Each awake one costs the
/// solver, transform propagation and its shadow's checks every tick.
const AWAKE_BOUND: usize = 0;

/// The dynamic bodies awake, as (prop index, m/s, rad/s).
fn awake_bodies(sim: &mut Sim) -> Vec<(Option<usize>, f32, f32)> {
    let world = sim.app.world_mut();
    world
        .query_filtered::<(&RigidBody, Option<&PropIndex>, &LinearVelocity, &AngularVelocity), (
            Without<Sleeping>,
            Without<RigidBodyDisabled>,
        )>()
        .iter(world)
        .filter(|(rb, ..)| rb.is_dynamic())
        .map(|(_, i, v, w)| (i.map(|i| i.0), v.0.length(), w.0.length()))
        .collect()
}

/// Busy stock maps settle: 10 s after loading, no physics prop is awake.
/// Before (docs/performance.md, "Props that never sleep"), de_inferno
/// kept two awake for good (potted plant 666 wobbling on a six-cornered
/// floor face, milk carton 761 wedged in a junk pile) and cs_italy two
/// (glass jugs 511 and 512 placed inside a cabinet's base). Then the plant
/// and the wedged jug, knocked like a bullet hit, still wake and move, and
/// sleep again.
#[test]
fn map_settles_with_no_body_awake() {
    for (name, knocked) in [
        ("de_inferno", Some(666)),
        ("cs_italy", Some(511)),
        ("cs_office", None),
        ("de_nuke", None),
    ] {
        let installed = LocalConfig::load()
            .ok()
            .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
            .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
        if !installed {
            eprintln!("skipping: no CS:S install configured");
            return;
        }
        let map = games::load_map_lean(&format!("cs_source:{name}")).expect("load map");
        let mut sim = sim(map);
        sim.seconds(10.0);
        let awake = awake_bodies(&mut sim);
        eprintln!("{name}: {} awake 10 s in: {awake:?}", awake.len());
        assert!(
            awake.len() <= AWAKE_BOUND,
            "{name}: {} bodies awake 10 s in (prop, m/s, rad/s): {awake:?}",
            awake.len()
        );
        let Some(index) = knocked else { continue };
        let &(_, body) = bodies(&mut sim).iter().find(|(i, _)| *i == index).expect("the prop");
        let before = sim.app.world().get::<Position>(body).unwrap().0;
        knock(&mut sim, body, Vec3::X - Vec3::Z * 0.3);
        sim.ticks(1);
        assert!(!motion(&mut sim, body).2, "{name}: the push wakes prop {index}");
        sim.seconds(0.3);
        let moved = sim.app.world().get::<Position>(body).unwrap().0.distance(before);
        assert!(moved > 0.01, "{name}: prop {index} moved {moved} m after the push");
        let (took, still) = settle(&mut sim, &[(index, body)], 6.0);
        eprintln!("{name}: prop {index} moved {moved:.3} m, asleep again after {took:?} s");
        assert!(still.is_empty(), "{name}: prop {index} still awake 6 s after the push: {still:?}");
    }
}


