//! A session on real maps (backlog "frame rate decays over a session";
//! docs/performance.md, "A session's frame rate"): one process, the
//! server's map change (`party_kit::server::swap_map`), a new ladder and
//! bots walking its rungs on each map, two laps over maps of every game.
//! What a map holds must be what it held on the first lap, the characters'
//! tables must stop growing, and a tick must cost what it did. Prints a row
//! per map (`--nocapture`). Passes with a note without the installs.

use bevy::prelude::*;
use chimera_core::{
    metrics::{ThreadCounters, census::Census},
    mount::config::LocalConfig,
};
use chimera_sim::minigame::{
    self,
    ladder::{Ladder, set_rung},
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_kit::{PartyPartsPlugin, server::swap_map};
use ultimate_gun_game::{MINIGAME, RUNGS, UltimateGunGamePlugin, generate};

const MAPS: [&str; 5] = [
    "cs_source:de_dust2",
    "cs_source:cs_office",
    "hl2dm:dm_lockdown",
    "combat_arms:warehouse",
    "cs_source:de_inferno",
];

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

fn play(sim: &mut Sim, seed: u64) {
    let w = sim.app.world_mut();
    generate(w, seed, RUNGS).expect("a ladder");
    minigame::start(w, MINIGAME).expect("the match starts");
    sim.seconds(4.0);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let bots: Vec<Entity> = {
        let w = sim.app.world_mut();
        w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect()
    };
    for r in 0..ladder.rungs.len() {
        for (i, b) in bots.iter().enumerate() {
            set_rung(sim.app.world_mut(), *b, (r + i * 3) % ladder.rungs.len(), &ladder);
        }
        sim.seconds(0.5);
    }
    sim.seconds(2.0);
}

/// Instructions per tick over `n` ticks without a new archetype.
fn tick_cost(sim: &mut Sim, n: usize) -> Option<f64> {
    let mut c = ThreadCounters::current();
    let (mut total, mut steady) = (0u64, 0u64);
    let mut last = sim.app.world().archetypes().len();
    for _ in 0..n {
        let a = c.read().hw?.instructions;
        sim.ticks(1);
        let used = c.read().hw?.instructions.saturating_sub(a);
        let now = sim.app.world().archetypes().len();
        if now == last {
            total += used;
            steady += 1;
        }
        last = now;
    }
    (steady > 0).then(|| total as f64 / steady as f64)
}

#[test]
fn two_laps_of_real_maps_hold_the_same_and_cost_the_same() {
    let first = party_kit::load_map(MAPS[0], 0);
    let Some(first) = first.ok().filter(|_| combat_arms_installed()) else {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    };
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(first), PartyPartsPlugin, UltimateGunGamePlugin::default()));
    sim.ticks(2);
    {
        let w = sim.app.world_mut();
        let mut s = w.resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 6;
        s.quota_set = true;
    }
    let mut rows: Vec<(usize, &str, Census, Option<f64>)> = Vec::new();
    for lap in 0..2 {
        for (i, map) in MAPS.iter().enumerate() {
            if lap > 0 || i > 0 {
                let Ok(data) = party_kit::load_map(map, 0) else {
                    eprintln!("{map}: not installed");
                    continue;
                };
                swap_map(sim.app.world_mut(), map, Some(data));
            }
            play(&mut sim, 500 + (lap * MAPS.len() + i) as u64);
            let c = Census::take(sim.app.world_mut());
            let tick = tick_cost(&mut sim, 200);
            eprintln!(
                "lap {lap} {map:24} {:6} entities {:6} archetypes {:5} tables {:?} instructions a tick",
                c.entities, c.archetypes_made, c.tables, tick
            );
            rows.push((lap, map, c, tick));
        }
    }
    for (lap, map, c, tick) in rows.iter().filter(|r| r.0 == 1) {
        let Some((.., was, was_tick)) = rows.iter().find(|r| r.0 == 0 && r.1 == *map) else { continue };
        assert!(
            (c.entities as f64) < was.entities as f64 * 1.1 + 50.0,
            "{map} held {} entities on lap 0, {} on lap {lap}: {:?}",
            was.entities,
            c.entities,
            c.diff(was).iter().filter(|l| l.starts_with("entities ")).take(12).collect::<Vec<_>>()
        );
        if let (Some(a), Some(b)) = (was_tick, tick) {
            assert!(*b < a * 1.3, "a tick on {map} cost {a:.0} instructions on lap 0, {b:.0} on lap {lap}");
        }
    }
    let tables = rows.last().unwrap().2.tables;
    assert!(tables < 700, "{tables} tables after two laps: a component that comes and goes on characters isn't a sparse set");
}
