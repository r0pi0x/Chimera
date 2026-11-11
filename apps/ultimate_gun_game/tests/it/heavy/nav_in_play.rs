//! Bots on a map nothing loaded behind a loading screen (the command line's
//! `--map`) move as soon as the round starts (backlog "Nothing blocks the main
//! thread"; docs/performance.md): de_dust2 headless with a front end's sliced
//! floods (`nav_gen::SliceFloods`), the gun game started at once, ticks paced
//! at the game's tick in wall time; the navigation mesh made off the main
//! thread (`nav_gen::NavGenTask`) arrives and a bot walks within
//! `BOTS_WALK_WITHIN`, no tick meanwhile over `WORST_TICK` instructions
//! (work counts: `Sim::work`, the tick's systems), and the task's start
//! (`ensure_nav`: the collision's copy) under `TASK_START`. Passes with a
//! note without the installs.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use chimera_core::{core::Dead, mount::config::LocalConfig};
use chimera_sim::minigame;
use chimera_world::{
    harness::Sim,
    map::{MapSimPlugin, nav::NavMesh, nav_gen::SliceFloods},
};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        bots::{CaBot, brain::NavJob},
    },
    harness::CaSim,
};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

/// From the round's start until a bot walks (wall time at the game's tick):
/// the mesh's flood on a task, 0.4-1.3 s measured at load 15-30 (2026-11-09);
/// sliced on the main thread (80 cells a tick) it was 7.3 s here, 10-13 s in
/// the app.
const BOTS_WALK_WITHIN: Duration = Duration::from_secs(6);

/// The worst tick while the mesh is made (work counts, from the tenth
/// tick): the gun game's tick with bots standing, the task only polled;
/// 54 M measured (2026-11-11, the tick's whole work on one thread with the
/// counting's own share; 45-50 M of the main thread's when the pool shared
/// it), 55-58 M of the main thread's when the flood was sliced.
const WORST_TICK: u64 = 60_000_000;

/// What starting the task costs its tick (`ensure_nav`: the collision's
/// copy and the task's spawn): 1.3 M measured. The rest of that tick is the
/// match's start (the bots joining and their first contacts, 58-64 M in
/// all), not the task's: a flood on the main thread would cost tens of
/// millions here.
const TASK_START: u64 = 5_000_000;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

#[test]
fn bots_walk_soon_on_a_map_loaded_without_a_screen() {
    let Some(map) = party_kit::load_map("cs_source:de_dust2", 0).ok().filter(|_| combat_arms_installed()) else {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    };
    let mut sim = Sim::new(
        CaSim,
        (
            MapSimPlugin::new(map),
            PartyPartsPlugin,
            UltimateGunGamePlugin { seed: 36336, rungs: 13, ..Default::default() },
            |app: &mut App| {
                // A front end's (the client shell inserts it).
                app.init_resource::<SliceFloods>();
            },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 4;
        s.quota_set = true;
    }
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    let hw = chimera_core::metrics::work::now().is_some();
    let start = Instant::now();
    let tick = Duration::from_secs_f64(TICK_INTERVAL);
    let (mut mesh_at, mut walked_at) = (None, None);
    let mut worst = (0u64, Duration::ZERO);
    let mut waiting: Vec<u64> = Vec::new();
    let mut last: Vec<(Entity, Vec3)> = Vec::new();
    let mut ticks = 0;
    let mut started_on: Option<(u64, u64, u64, chimera_core::metrics::work::WorkCounts)> = None;
    let mut worst_systems = None;
    while start.elapsed() < Duration::from_secs(30) && walked_at.is_none() {
        let frame = Instant::now();
        let w = sim.work(1);
        ticks += 1;
        let spent = w.as_ref().map_or(0, |w| w.ticks[0]);
        if ticks >= 10 && spent > worst.0 {
            worst = (spent, start.elapsed());
            worst_systems = w.as_ref().map(|w| w.systems.clone());
        }
        if ticks >= 10 && mesh_at.is_none() {
            waiting.push(spent);
        }
        // The tick that started the flood's task (the collision copied).
        if started_on.is_none() && sim.app.world().contains_resource::<NavJob>() {
            let systems = w.map(|w| w.systems).unwrap_or_default();
            let nav = systems.0.iter().filter(|(k, _)| k.ends_with(" ensure_nav")).map(|(_, v)| v.0).sum::<u64>();
            started_on = Some((ticks, spent, nav, systems));
        }
        if mesh_at.is_none() && sim.app.world().contains_resource::<NavMesh>() {
            mesh_at = Some(start.elapsed());
        }
        let now: Vec<(Entity, Vec3)> = {
            let w = sim.app.world_mut();
            w.query_filtered::<(Entity, &Transform), (With<CaBot>, Without<Dead>)>()
                .iter(w)
                .map(|(e, t)| (e, t.translation))
                .collect()
        };
        // A bot walking: across the floor at a walk's pace (not a fall, not a
        // respawn's jump).
        let walking = now.iter().any(|(e, p)| {
            last.iter().any(|(e0, p0)| e0 == e && {
                let d = (*p - *p0).xz().length();
                d > 0.01 && d < 0.5
            })
        });
        if walking && mesh_at.is_some() {
            walked_at = Some(start.elapsed());
        }
        last = now;
        if let Some(rest) = tick.checked_sub(frame.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    // What the task's start costs the main thread: the collision's copy.
    let a = chimera_core::metrics::work::now().unwrap_or(0);
    let copy = chimera_world::map::nav_gen::CollisionSnapshot::of(sim.app.world_mut());
    let copy_cost = chimera_core::metrics::work::now().unwrap_or(0).saturating_sub(a);
    drop(copy);
    eprintln!("the collision's copy for the task: {:.1} M instructions", copy_cost as f64 / 1e6);
    let total: u64 = waiting.iter().sum();
    waiting.sort_unstable();
    eprintln!(
        "nav mesh at {mesh_at:?}, a bot walking at {walked_at:?}, the task started on tick {:?} ({:.1} M, {:.1} M of it the task's start), worst tick {:.1} M at {:?}; while waiting {} ticks, median {:.1} M, {:.0} M in all{}",
        started_on.as_ref().map(|s| s.0),
        started_on.as_ref().map_or(0, |s| s.1) as f64 / 1e6,
        started_on.as_ref().map_or(0, |s| s.2) as f64 / 1e6,
        worst.0 as f64 / 1e6,
        worst.1,
        waiting.len(),
        waiting.get(waiting.len() / 2).copied().unwrap_or(0) as f64 / 1e6,
        total as f64 / 1e6,
        if hw { "" } else { " (no hardware counters)" }
    );
    let walked = walked_at.expect("a bot walks");
    assert!(walked < BOTS_WALK_WITHIN, "a bot walked {walked:?} after the round started (limit {BOTS_WALK_WITHIN:?})");
    let top = |w: &chimera_core::metrics::work::WorkCounts| {
        w.top(15).into_iter().map(|(l, i, _)| format!("{i:>12}  {l}")).collect::<Vec<_>>().join("\n")
    };
    if let Some((_, spent, nav, systems)) = started_on.as_ref().filter(|_| hw) {
        assert!(
            *nav < TASK_START,
            "the task's start (ensure_nav) cost {nav} instructions (limit {TASK_START}) in a tick of {spent}; its \
             costliest systems:\n{}",
            top(systems)
        );
    }
    if hw {
        assert!(
            worst.0 < WORST_TICK,
            "a tick of {} instructions at {:?} (limit {WORST_TICK}); its costliest systems:\n{}",
            worst.0,
            worst.1,
            worst_systems.as_ref().map(top).unwrap_or_default()
        );
    }
}
