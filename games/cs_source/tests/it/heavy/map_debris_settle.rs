//! Ragdolls and broken props' pieces settle and sleep on real CS:S maps,
//! headless: a CS:S player killed at a map's spawns lies still and its
//! bodies go to sleep, and a broken prop's pieces come to rest apart and
//! sleep, rather than moving for good. Their hulls are cut for contacts like
//! props' (`map::collision::contact_hulls`). Skipped without an install.

use avian3d::prelude::*;
use bevy::prelude::*;
use crate::stack::{
    core::{Damage, DamageKind, Hitgroup},
    games::{
        self, cs_source,
        cs_source::{
            movement::{SourceMovementPlugin, to_engine},
            weapons::CsWeaponsPlugin,
        },
    },
    harness::{CsSim, Sim},
    map::{MapData, MapPlugin, PropEntity, Ragdoll, ragdoll::RagdollSettings},
    mount::config::LocalConfig,
    rules::Deathmatch,
};

fn load(name: &str) -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()?
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return None;
    }
    Some(games::load_map_lean(&format!("cs_source:{name}")).expect("load map"))
}

fn sim(map: MapData) -> Sim {
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    // The dead stay dead for the test.
    sim.app.world_mut().resource_mut::<Deathmatch>().respawn_delay = 1000.0;
    sim.ticks(2);
    sim
}

/// Each body's (asleep, angular rad/s); a removed one is left out.
fn motion(sim: &Sim, bodies: &[Entity]) -> Vec<(bool, f32)> {
    let w = sim.app.world();
    bodies
        .iter()
        .filter_map(|b| Some((w.get::<Sleeping>(*b).is_some(), w.get::<AngularVelocity>(*b)?.0.length())))
        .collect()
}

/// Ticks until every body of `bodies` sleeps, up to `limit` seconds: the
/// time it took, and the fastest angular speed of an awake body after
/// `quiet` seconds (jitter after the fall), for the log.
fn settle(sim: &mut Sim, bodies: &[Entity], limit: f64, quiet: f64) -> (Option<f64>, f32) {
    let ticks = (limit / cs_source::TICK_INTERVAL).round() as u64;
    let mut jitter = 0.0f32;
    for t in 1..=ticks {
        sim.ticks(1);
        let now = t as f64 * cs_source::TICK_INTERVAL;
        let m = motion(sim, bodies);
        if now > quiet {
            jitter = m.iter().filter(|b| !b.0).map(|b| b.1).fold(jitter, f32::max);
        }
        // All still there: a removed body would count as asleep.
        if m.len() == bodies.len() && m.iter().all(|b| b.0) {
            return (Some(now), jitter);
        }
    }
    (None, jitter)
}

/// The spawn points of a class (Source units).
fn spawns(map: &MapData, class: &str) -> Vec<Vec3> {
    map.entities
        .iter()
        .filter(|e| e.classname() == class)
        .map(|e| e.origin())
        .collect()
}

/// A CS:S player killed by a head shot at each of ten of a map's spawns,
/// one after another, pushed a different way each time, with the forced
/// sleep (`ragdoll_sleepaftertime`) off: its ragdoll lies down, from
/// `QUIET` s on no body turns faster than `REST` rad/s, and every body
/// sleeps within `LIMIT` s, half of them within 4 s. Run three ways in
/// fresh sims, so the bounds hold whatever the solver's order (the spawns
/// in map order, in reverse, and every other one first). Measured: up to
/// 1.49 rad/s after 4 s, asleep in 1.5-5.7 s, median 2.9 s, 57 of 60 by
/// 5 s (avian 0.8; with 0.7 up to 1.74 rad/s after 3 s, asleep in
/// 1.9-7.0 s, median 3.2 s, 51 of 60 by 5 s: 0.8 solves friction only in
/// its relax pass, so an arm lying on a de_inferno step's edge slides off
/// it at up to 5 rad/s 3.4-3.8 s after the fall). Before ragdoll joints were
/// solved in the contacts' passes (and lost avian's gyroscopic torque), a
/// limb or two kept jittering against the floor at up to 10-16 rad/s and
/// almost none slept but by the forced rule (5.25-5.7 s; docs/performance.md,
/// "Props that never sleep"). The forced rule has its own test
/// (`ragdoll::a_ragdoll_kept_moving_is_put_to_sleep_after_five_seconds`).
#[test]
fn ragdolls_settle_and_sleep() {
    const QUIET: f64 = 4.0;
    const REST: f32 = 2.5;
    const LIMIT: f64 = 8.0;
    let mut late = Vec::new();
    let mut restless = Vec::new();
    let mut times = Vec::new();
    for (name, class) in [
        ("de_dust2", "info_player_terrorist"),
        ("de_inferno", "info_player_counterterrorist"),
    ] {
        let Some(map) = load(name) else { return };
        let points: Vec<Vec3> = spawns(&map, class).into_iter().take(10).collect();
        assert!(points.len() >= 10, "{name}: only {} {class}", points.len());
        let order: Vec<usize> = (0..points.len()).collect();
        let reversed: Vec<usize> = order.iter().rev().copied().collect();
        let skipping: Vec<usize> = order.iter().step_by(2).chain(order.iter().skip(1).step_by(2)).copied().collect();
        for (run, order) in [order, reversed, skipping].into_iter().enumerate() {
            let mut sim = sim(map.clone());
            sim.app.world_mut().resource_mut::<RagdollSettings>().sleep_after = 1000.0;
            for k in order {
                let p = crate::common::spawn_still(&mut sim, to_engine(points[k]) + Vec3::Y * 0.95);
                sim.ticks(10);
                let head = sim.position(p) + Vec3::Y * 0.7;
                let dir = Quat::from_rotation_y(k as f32 * 2.1) * Vec3::NEG_Z;
                sim.app.world_mut().write_message(Damage {
                    force: Vec3::ZERO,
                    target: p,
                    attacker: None,
                    amount: 5.0,
                    point: head,
                    dir,
                    hitgroup: Hitgroup::Head,
                    kind: DamageKind::Bullet,
                    weapon: None,
                });
                sim.ticks(1);
                let ragdoll = {
                    let world = sim.app.world_mut();
                    let mut q = world.query::<&Ragdoll>();
                    q.iter(world).last().cloned().expect("a ragdoll")
                };
                let (took, jitter) = settle(&mut sim, &ragdoll.bodies, LIMIT, QUIET);
                let at = sim.app.world().get::<Transform>(ragdoll.bodies[0]).unwrap().translation;
                eprintln!(
                    "run {run}: {name} spawn {k} ragdoll at {at}: asleep after {took:?} s, \
                     up to {jitter:.3} rad/s after {QUIET} s"
                );
                times.push(took.unwrap_or(f64::INFINITY));
                if took.is_none() {
                    late.push((run, name, k));
                }
                if jitter > REST {
                    restless.push((run, name, k, jitter));
                }
            }
        }
    }
    times.sort_by(f64::total_cmp);
    let by = |t: f64| times.iter().filter(|x| **x <= t).count();
    eprintln!(
        "{} ragdolls: asleep by 3 s {}, by 4 s {}, by 5 s {}, median {:.2} s, last {:.2} s",
        times.len(),
        by(3.0),
        by(4.0),
        by(5.0),
        times[times.len() / 2],
        times[times.len() - 1]
    );
    assert!(
        restless.is_empty(),
        "(run, map, spawn, rad/s) {restless:?}: turning faster than {REST} rad/s after {QUIET} s"
    );
    assert!(late.is_empty(), "(run, map, spawn) {late:?}: still awake after {LIMIT} s");
    assert!(times[times.len() / 2] <= 4.0, "half the ragdolls still awake after 4 s");
}

/// Props that break into pieces (their models' own break pieces, else a
/// gib list's chunks): each of de_inferno's first twelve crates and pots,
/// broken by a big bullet, has pieces that all sleep within `LIMIT` s and
/// end up touching no other piece (debris doesn't collide with debris).
/// Run three ways, so the bound holds whatever order the solver meets the
/// contacts in (other entities, colliders and random draws before each
/// break): the props broken in map order, in reverse, and every other one
/// first. Pieces have their `.phy` hulls: 1.5-4.2 s measured (as boxes
/// 0.8-2.9 s). With hulls but without props' stillness sleep, 14 of the 36
/// breaks had a light piece rocking alone on the floor past 5 s (0.3-1.9
/// rad/s). Before debris stopped colliding with debris, one prop's pieces
/// piled up, one rocking on the others kept the pile awake, and how long
/// hung on the solver's order (prop 121: 1.6 s on one base, 15 s with 110
/// more world brushes).
#[test]
fn broken_prop_pieces_settle_and_sleep() {
    const LIMIT: f64 = 5.0;
    let models = ["wood_crate", "pot", "bucket", "wood_pallet"];
    let Some(map) = load("de_inferno") else { return };
    let props: Vec<usize> = (0..map.entities.len())
        .filter(|i| {
            let e = &map.entities[*i];
            e.classname().starts_with("prop_physics")
                && e.get("model").is_some_and(|m| {
                    let m = m.to_ascii_lowercase();
                    models.iter().any(|k| m.contains(k))
                })
        })
        .take(12)
        .collect();
    let mut late = Vec::new();
    let reversed: Vec<usize> = props.iter().rev().copied().collect();
    let skipping: Vec<usize> = props.iter().step_by(2).chain(props.iter().skip(1).step_by(2)).copied().collect();
    for (run, order) in [props, reversed, skipping].into_iter().enumerate() {
        let mut sim = sim(map.clone());
        sim.seconds(1.0);
        let mut broke = 0;
        for i in order {
            let node = {
                let world = sim.app.world_mut();
                world
                    .query::<(Entity, &PropEntity)>()
                    .iter(world)
                    .find(|(_, p)| p.0 == i)
                    .map(|(e, _)| e)
            };
            let Some(target) = node else { continue };
            let before = gibs(&mut sim);
            let point = sim.app.world().get::<Transform>(target).unwrap().translation;
            sim.app.world_mut().write_message(Damage {
                target,
                attacker: None,
                amount: 50.0,
                point,
                dir: Vec3::NEG_Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Bullet,
                weapon: None,
                force: Vec3::ZERO,
            });
            sim.ticks(3);
            let pieces: Vec<Entity> = gibs(&mut sim).into_iter().filter(|g| !before.contains(g)).collect();
            if pieces.is_empty() {
                continue;
            }
            broke += 1;
            let (took, jitter) = settle(&mut sim, &pieces, LIMIT, 3.0);
            let stacked = touching_pairs(&sim, &pieces);
            eprintln!(
                "run {run}: de_inferno prop {i} at {point}: {} pieces asleep after {took:?} s, \
                 jitter up to {jitter:.3} rad/s, {stacked} piece pairs touching",
                pieces.len()
            );
            assert_eq!(stacked, 0, "run {run}: prop {i}'s pieces touch each other");
            if took.is_none() {
                late.push((run, i));
            }
        }
        assert!(broke >= 10, "run {run}: only {broke} props broke into pieces");
    }
    assert!(late.is_empty(), "(run, prop) {late:?}: pieces still awake after {LIMIT} s");
}

/// Touching contact pairs between two of `bodies`.
fn touching_pairs(sim: &Sim, bodies: &[Entity]) -> usize {
    let graph = sim.app.world().resource::<ContactGraph>();
    graph
        .iter_active_touching()
        .chain(graph.iter_sleeping_touching())
        .filter(|p| {
            p.body1.is_some_and(|b| bodies.contains(&b)) && p.body2.is_some_and(|b| bodies.contains(&b))
        })
        .count()
}

/// Broken props' pieces with bodies.
fn gibs(sim: &mut Sim) -> Vec<Entity> {
    let world = sim.app.world_mut();
    world
        .query::<(Entity, &Name, &RigidBody)>()
        .iter(world)
        .filter(|(_, n, rb)| n.as_str() == "Gib" && rb.is_dynamic())
        .map(|(e, ..)| e)
        .collect()
}
