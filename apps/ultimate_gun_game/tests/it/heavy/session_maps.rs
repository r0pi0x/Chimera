//! A session on real maps (backlog "frame rate decays over a session";
//! docs/performance.md, "A session's frame rate"): one process, the
//! server's map change (`party_kit::server::swap_map`), a new ladder and
//! bots walking its rungs on each map, three laps over maps of every game,
//! each map with the same ladder every lap. What a map holds must be what it
//! held on the first lap, the characters' tables must stop growing, a lap
//! after the first must add few characters' archetypes (fewer than 100; a
//! failure lists the moves that made them, `census::made_by`), and a tick
//! must cost what it did on the lap before.
//!
//! The tick is compared from the second lap on: the first makes the
//! archetypes of every game's characters on every map, which Bevy keeps, and
//! each costs every tick (about 800 instructions with no character on the
//! map: docs/performance.md, "A session's frame rate"); de_dust2, the first
//! map, pays all of them on its second visit. A tick is the mean of its
//! map's ticks' work counts (`Sim::work`, ticks making no archetype): the
//! session plays out the same on every run (`Sim::repeatable`), so the
//! same build counts the same, and a failure lists the systems that grew.
//! Prints a row per map (`--nocapture`). Passes with a note without the
//! installs.

use bevy::prelude::*;
use chimera_core::{
    metrics::census::Census,
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

/// Laps over `MAPS`: the first makes the characters' archetypes, the next
/// two are compared.
const LAPS: usize = 3;

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

/// The archetypes holding `Health` (the characters'), every one made.
fn characters(c: &Census) -> usize {
    c.made.iter().filter(|k| k.split('+').any(|n| n == "Health")).count()
}

/// What made the characters' archetypes (`chimera_census made`).
fn chimera_census_made(sim: &mut Sim) -> String {
    chimera_core::metrics::census::made_by(sim.app.world(), "Health", 20)
}

/// The mean instructions of `n` ticks' systems (`Sim::work`, the same on
/// every run), over those that made no new archetype; and the systems'
/// counts per tick.
fn tick_cost(sim: &mut Sim, n: u64) -> Option<(f64, chimera_core::metrics::work::WorkCounts)> {
    let w = sim.work(n)?;
    let steady = w.steady_ticks();
    (!steady.is_empty()).then(|| (steady.iter().sum::<u64>() as f64 / steady.len() as f64, w.systems.per(n)))
}

#[test]
fn laps_of_real_maps_hold_the_same_and_cost_the_same() {
    let first = party_kit::load_map(MAPS[0], 0);
    let Some(first) = first.ok().filter(|_| combat_arms_installed()) else {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    };
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(first), PartyPartsPlugin, UltimateGunGamePlugin::default()));
    // The same play on every run, so laps' counts compare.
    sim.repeatable();
    sim.ticks(2);
    {
        let w = sim.app.world_mut();
        let mut s = w.resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 6;
        s.quota_set = true;
    }
    let mut rows: Vec<(usize, &str, Census, Option<(f64, chimera_core::metrics::work::WorkCounts)>)> = Vec::new();
    // Characters' archetypes at the end of each lap.
    let mut laps = Vec::new();
    for lap in 0..LAPS {
        for (i, map) in MAPS.iter().enumerate() {
            if lap > 0 || i > 0 {
                let Ok(data) = party_kit::load_map(map, 0) else {
                    eprintln!("{map}: not installed");
                    continue;
                };
                swap_map(sim.app.world_mut(), map, Some(data));
            }
            // The same ladder each lap: another ladder is other guns.
            play(&mut sim, 500 + i as u64);
            let c = Census::take(sim.app.world_mut());
            let tick = tick_cost(&mut sim, 300);
            eprintln!(
                "lap {lap} {map:24} {:6} entities {:6} archetypes {:5} characters' {:5} tables {:?} instructions a tick",
                c.entities,
                c.archetypes_made,
                characters(&c),
                c.tables,
                tick.as_ref().map(|t| t.0)
            );
            rows.push((lap, map, c, tick));
        }
        laps.push(characters(&rows.last().unwrap().2));
    }
    // What a lap after the first adds must be what a lap of new deaths and
    // rungs adds, not every combination again: each archetype costs every
    // tick for good (before the characters' parts were kept on them from
    // the spawn, 1,829 after the first lap, then 526 and 277 more; with
    // them, 591, then 26 and 50; with a game switch's parts kept or moved
    // together, 410, then 19 and 43).
    let made = chimera_census_made(&mut sim);
    for lap in 1..LAPS {
        let added = laps[lap] - laps[lap - 1];
        assert!(
            added < 100,
            "lap {lap} made {added} more characters' archetypes ({} after the first lap): a part that comes and goes on \
             characters, one at a time, makes one for every combination of the others. Keep it on them from the spawn \
             (`chimera_core::character::CharacterPartsExt::on_every_character`), or put a game's parts on and off in \
             one move (docs/DESIGN.md, \"ECS style\"). {made}",
            laps[0]
        );
    }
    for (lap, map, c, tick) in rows.iter().filter(|r| r.0 > 0) {
        let Some((.., was, _)) = rows.iter().find(|r| r.0 == 0 && r.1 == *map) else { continue };
        assert!(
            (c.entities as f64) < was.entities as f64 * 1.1 + 50.0,
            "{map} held {} entities on lap 0, {} on lap {lap}: {:?}",
            was.entities,
            c.entities,
            c.diff(was).iter().filter(|l| l.starts_with("entities ")).take(12).collect::<Vec<_>>()
        );
        let before = rows.iter().find(|r| r.0 == lap - 1 && r.1 == *map).and_then(|r| r.3.as_ref());
        if let (true, Some((a, sa)), Some((b, sb))) = (*lap > 1, before, tick) {
            assert!(
                *b < a * 1.3,
                "a tick on {map} cost {a:.0} instructions on lap {}, {b:.0} on lap {lap}. Per tick:\n{}",
                lap - 1,
                sb.diff_table(sa, 20)
            );
        }
    }
    let tables = rows.last().unwrap().2.tables;
    assert!(tables < 700, "{tables} tables after three laps: a component that comes and goes on characters isn't a sparse set");
}

