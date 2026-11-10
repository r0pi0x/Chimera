//! Bots on a map nothing loaded behind a loading screen (the command line's
//! `--map`) move as soon as the round starts (backlog "Nothing blocks the main
//! thread"; docs/performance.md): de_dust2 headless with a front end's sliced
//! floods (`nav_gen::SliceFloods`), the gun game started at once, ticks paced
//! at the game's tick in wall time; the navigation mesh made off the main
//! thread (`nav_gen::NavGenTask`) arrives and a bot walks within
//! `BOTS_WALK_WITHIN`, no tick meanwhile over `WORST_TICK` main-thread
//! instructions. Passes with a note without the installs.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use chimera_core::{core::Dead, metrics::ThreadCounters, mount::config::LocalConfig};
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

/// The worst tick while the mesh is made (main-thread instructions; from the
/// tenth tick, and the task's first: the match's start, 38-53 M with the
/// collision's copy's 0.2-0.5 M): the gun game's tick with bots standing, the
/// task only polled; 45-50 M measured, 55-58 M when the flood was sliced.
const WORST_TICK: u64 = 60_000_000;

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
    let mut counters = ThreadCounters::current();
    let hw = counters.read().hw.is_some();
    let start = Instant::now();
    let tick = Duration::from_secs_f64(TICK_INTERVAL);
    let (mut mesh_at, mut walked_at) = (None, None);
    let mut worst = (0u64, Duration::ZERO);
    let mut waiting: Vec<u64> = Vec::new();
    let mut last: Vec<(Entity, Vec3)> = Vec::new();
    let mut ticks = 0;
    let mut started_on = None;
    while start.elapsed() < Duration::from_secs(30) && walked_at.is_none() {
        let frame = Instant::now();
        let a = counters.read().hw.map_or(0, |h| h.instructions);
        sim.ticks(1);
        ticks += 1;
        let spent = counters.read().hw.map_or(0, |h| h.instructions).saturating_sub(a);
        if ticks >= 10 && spent > worst.0 {
            worst = (spent, start.elapsed());
        }
        if ticks >= 10 && mesh_at.is_none() {
            waiting.push(spent);
        }
        // The tick that started the flood's task (the collision copied).
        if started_on.is_none() && sim.app.world().contains_resource::<NavJob>() {
            started_on = Some((ticks, spent));
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
    let a = counters.read().hw.map_or(0, |h| h.instructions);
    let copy = chimera_world::map::nav_gen::CollisionSnapshot::of(sim.app.world_mut());
    let copy_cost = counters.read().hw.map_or(0, |h| h.instructions).saturating_sub(a);
    drop(copy);
    eprintln!("the collision's copy for the task: {:.1} M instructions", copy_cost as f64 / 1e6);
    let total: u64 = waiting.iter().sum();
    waiting.sort_unstable();
    eprintln!(
        "nav mesh at {mesh_at:?}, a bot walking at {walked_at:?}, the task started on tick {:?} ({:.1} M), worst tick {:.1} M at {:?}; while waiting {} ticks, median {:.1} M, {:.0} M in all{}",
        started_on.map(|s| s.0),
        started_on.map_or(0, |s| s.1) as f64 / 1e6,
        worst.0 as f64 / 1e6,
        worst.1,
        waiting.len(),
        waiting.get(waiting.len() / 2).copied().unwrap_or(0) as f64 / 1e6,
        total as f64 / 1e6,
        if hw { "" } else { " (no hardware counters)" }
    );
    let walked = walked_at.expect("a bot walks");
    assert!(walked < BOTS_WALK_WITHIN, "a bot walked {walked:?} after the round started (limit {BOTS_WALK_WITHIN:?})");
    if let Some((_, spent)) = started_on.filter(|_| hw) {
        assert!(spent < WORST_TICK, "the task's first tick: {spent} instructions (limit {WORST_TICK})");
    }
    if hw {
        assert!(worst.0 < WORST_TICK, "a tick of {} instructions at {:?} (limit {WORST_TICK})", worst.0, worst.1);
    }
}
