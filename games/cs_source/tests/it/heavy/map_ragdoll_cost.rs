//! What a ragdoll costs a tick on a real map (docs/performance.md, "A
//! ragdoll's cost"): CS:S players killed by a head shot at de_dust2's
//! terrorist spawns, one after another, in a repeatable sim, counted by
//! `Sim::work` (instructions per system, the same on every run). Each
//! death's spawn tick and its awake ticks (until every body sleeps) are
//! measured against ticks with no ragdoll awake, then the ten lying asleep
//! against the world without them; a ragdoll's extra instructions per
//! awake tick must stay under `AWAKE_BUDGET` (2.03M before 2026-11-12,
//! 1.00M after, run alone), its spawn under `SPAWN_BUDGET`, a sleeping one
//! under `ASLEEP_BUDGET`. Prints the systems that grew (`--nocapture`).
//! Skipped without an install or hardware counters.

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
    map::{MapData, MapPlugin, Ragdoll, ragdoll::RagdollBody},
    mount::config::LocalConfig,
    rules::Deathmatch,
};
use chimera_core::metrics::work::WorkCounts;

/// A ragdoll's extra instructions per tick while any of its bodies is
/// awake (test build; measured: see docs/performance.md).
const AWAKE_BUDGET: f64 = 1_350_000.0;
/// Its spawn tick's extra instructions (the first death pays its model's
/// hulls; the budget is for the later ones).
const SPAWN_BUDGET: f64 = 5_000_000.0;
/// A sleeping ragdoll's (a gun game keeps one per dead player).
const ASLEEP_BUDGET: f64 = 45_000.0;
/// The deaths measured.
const DEATHS: usize = 10;

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

/// Touching contact pairs with a ragdoll body in them, and their manifolds.
fn ragdoll_contacts(sim: &mut Sim) -> (usize, usize) {
    let world = sim.app.world_mut();
    let bodies: std::collections::HashSet<Entity> = world
        .query_filtered::<Entity, With<RagdollBody>>()
        .iter(world)
        .collect();
    let graph = world.resource::<ContactGraph>();
    graph
        .iter_active_touching()
        .filter(|p| p.body1.is_some_and(|b| bodies.contains(&b)) || p.body2.is_some_and(|b| bodies.contains(&b)))
        .fold((0, 0), |(n, m), p| (n + 1, m + p.manifolds.len()))
}

/// The fastest awake ragdoll body's linear (m/s) and angular (rad/s) speed.
fn fastest(sim: &mut Sim) -> (f32, f32) {
    let world = sim.app.world_mut();
    world
        .query_filtered::<(&LinearVelocity, &AngularVelocity), (With<RagdollBody>, Without<Sleeping>)>()
        .iter(world)
        .fold((0.0f32, 0.0f32), |(v, w), (l, a)| {
            (v.max(l.0.length()), w.max(a.0.length()))
        })
}

fn awake(sim: &mut Sim) -> usize {
    let world = sim.app.world_mut();
    world
        .query_filtered::<(), (With<RagdollBody>, Without<Sleeping>)>()
        .iter(world)
        .count()
}

#[test]
fn a_ragdoll_costs_little_a_tick_on_de_dust2() {
    let Some(map) = load("de_dust2") else { return };
    let points: Vec<Vec3> = map
        .entities
        .iter()
        .filter(|e| e.classname() == "info_player_terrorist")
        .map(|e| e.origin())
        .take(DEATHS)
        .collect();
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim.repeatable();
    sim.app.world_mut().resource_mut::<Deathmatch>().respawn_delay = 1000.0;
    sim.ticks(2);
    // Every victim first, so their spawns' archetypes are made before
    // counting.
    let victims: Vec<Entity> = points
        .iter()
        .map(|p| crate::common::spawn_still(&mut sim, to_engine(*p) + Vec3::Y * 0.95))
        .collect();
    sim.seconds(1.0);
    let Some(quiet) = sim.work(120) else {
        eprintln!("skipping: no hardware counters");
        return;
    };
    let base = quiet.median() as f64;
    let base_systems = quiet.systems.per(120);
    let (mut spawns, mut per_tick, mut lives) = (Vec::new(), Vec::new(), Vec::new());
    let mut awake_systems = WorkCounts::default();
    let mut awake_ticks = 0u64;
    for (k, p) in victims.iter().enumerate() {
        let head = sim.position(*p) + Vec3::Y * 0.7;
        let dir = Quat::from_rotation_y(k as f32 * 2.1) * Vec3::NEG_Z;
        sim.app.world_mut().write_message(Damage {
            force: Vec3::ZERO,
            target: *p,
            attacker: None,
            amount: 500.0,
            point: head,
            dir,
            hitgroup: Hitgroup::Head,
            kind: DamageKind::Bullet,
            weapon: None,
        });
        let spawn = sim.work(1).expect("counters");
        spawns.push(spawn.total() as f64 - base);
        let ragdolls = {
            let world = sim.app.world_mut();
            world.query::<&Ragdoll>().iter(world).count()
        };
        assert_eq!(ragdolls, k + 1, "death {k} made no ragdoll");
        // Until every body sleeps (the forced rule puts them to sleep by 5 s).
        let (mut total, mut ticks, mut pairs, mut manifolds, mut still) = (0u64, 0u64, 0usize, 0usize, 0u64);
        while awake(&mut sim) > 0 && ticks < 600 {
            let w = sim.work(1).expect("counters");
            total += w.total();
            awake_systems.add(&w.systems);
            ticks += 1;
            let (n, m) = ragdoll_contacts(&mut sim);
            pairs += n;
            manifolds += m;
            let (v, w) = fastest(&mut sim);
            if v < 0.05 && w < 0.5 {
                still += 1;
            }
        }
        assert!(ticks < 600, "death {k}'s ragdoll still awake after 9 s");
        awake_ticks += ticks;
        let extra = total as f64 / ticks.max(1) as f64 - base;
        eprintln!(
            "death {k}: spawn tick +{:.2}M, awake {ticks} ticks ({:.2} s; {still} of them all under 5 cm/s and 0.5 \
             rad/s), +{:.3}M a tick, {:.1} touching pairs ({:.1} manifolds) a tick",
            spawns[k] / 1e6,
            ticks as f64 * cs_source::TICK_INTERVAL,
            extra / 1e6,
            pairs as f64 / ticks.max(1) as f64,
            manifolds as f64 / ticks.max(1) as f64
        );
        per_tick.push(extra);
        lives.push(extra * ticks as f64);
    }
    // Every ragdoll asleep: what they cost lying there.
    let lying = sim.work(120).expect("counters");
    // Against the same world without them.
    {
        let world = sim.app.world_mut();
        let all: Vec<Ragdoll> = world.query::<&Ragdoll>().iter(world).cloned().collect();
        let roots: Vec<Entity> = world.query_filtered::<Entity, With<Ragdoll>>().iter(world).collect();
        for e in all.iter().flat_map(|r| r.bodies.iter().chain(&r.joints)).chain(&roots) {
            world.despawn(*e);
        }
    }
    sim.ticks(10);
    let gone = sim.work(120).expect("counters");
    let asleep_each = (lying.median() as f64 - gone.median() as f64) / DEATHS as f64;
    eprintln!(
        "{DEATHS} ragdolls asleep: +{:.0} instructions a tick each; the systems that grew:\n{}",
        asleep_each,
        lying.systems.per(120).diff_table(&gone.systems.per(120), 8)
    );
    let awake_per = awake_systems.per(awake_ticks);
    eprintln!(
        "quiet tick {:.2}M; per awake tick, the systems that grew:\n{}",
        base / 1e6,
        awake_per.diff_table(&base_systems, 25)
    );
    let mean = per_tick.iter().sum::<f64>() / per_tick.len() as f64;
    let later_spawns = spawns[1..].iter().copied().fold(0.0, f64::max);
    eprintln!(
        "a ragdoll: +{:.3}M a tick awake (mean of {DEATHS}), {:.0}M over its waking life, spawn {:.2}M first, \
         {:.2}M at most after",
        mean / 1e6,
        lives.iter().sum::<f64>() / lives.len() as f64 / 1e6,
        spawns[0] / 1e6,
        later_spawns / 1e6
    );
    assert!(
        mean < AWAKE_BUDGET,
        "a ragdoll costs {mean:.0} instructions a tick awake (budget {AWAKE_BUDGET:.0}): see the table above"
    );
    assert!(
        asleep_each < ASLEEP_BUDGET,
        "a sleeping ragdoll costs {asleep_each:.0} instructions a tick (budget {ASLEEP_BUDGET:.0})"
    );
    assert!(
        later_spawns < SPAWN_BUDGET,
        "a ragdoll's spawn tick costs {later_spawns:.0} instructions (budget {SPAWN_BUDGET:.0})"
    );
}
