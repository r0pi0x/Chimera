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
//! map, pays all of them on its second visit. A tick is the median of
//! its map's quiet ticks' work counts (`Sim::work`; quiet: making no
//! archetype and with no ragdoll body awake): a death, a grenade or a
//! ragdoll makes a tick several times another, and each lap's fights
//! differ, so a mean follows the fight. A ragdoll stays awake for seconds
//! (15 bodies against the map: 2-3M a tick more), so 300 ticks are played
//! and then more until 100 were quiet (`tick_cost`): every lap's median is
//! of the same kind of tick. (Before, a lap with fewer than 100 quiet ticks
//! in its 300 took the median of them all, a ragdoll's: de_dust2's third
//! lap had 89 once the bots fired HL2:DM's SMG1 at range, and counted
//! 10.3M against the second's 7.2M.) After each map no solver body of
//! Avian's may be a despawned entity's (`chimera_world::map::solver_bodies`:
//! 1,422 were, 183k instructions a tick more every lap). The session
//! plays out the same on every run (`Sim::repeatable`), so the same build
//! counts the same, and a failure lists the systems that grew (per quiet
//! tick). Prints a row per map (`--nocapture`).
//! Passes with a note without the installs.

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

/// Avian's solver bodies whose entity is gone: the bodies of the maps
/// before, of ragdolls and gibs despawned (a leak: each costs every tick).
fn stale_solver_bodies(sim: &mut Sim) -> usize {
    let world = sim.app.world();
    let solver = world.resource::<avian3d::dynamics::solver::solver_body::SolverBodies>();
    solver.entities().iter().filter(|e| world.get_entity(**e).is_err()).count()
}

/// Ticks measured on each map at least, the quiet ones a median needs, and
/// the most ticks spent waiting for them.
const MEASURED: u64 = 300;
const QUIET: usize = 100;
const MOST: u64 = 1500;

/// The median of a map's quiet ticks' instructions (`Sim::work`, the same
/// on every run), and the systems' counts per quiet tick. A quiet tick made
/// no new archetype, nor followed one that did, and had no ragdoll body
/// awake. `MEASURED` ticks are played, then more until `QUIET` of them
/// were quiet: every lap's median is then of the same kind of tick, however
/// its fight went. A ragdoll is put to sleep 5 s after its death at the
/// latest, so `MOST` ticks with fewer than `QUIET` quiet ones are bodies
/// that never rest (or deaths without a pause), and fail.
fn tick_cost(sim: &mut Sim, map: &str) -> Option<(f64, chimera_core::metrics::work::WorkCounts)> {
    use chimera_world::map::ragdoll::RagdollBody;
    let mut systems = chimera_core::metrics::work::WorkCounts::default();
    let mut quiet = Vec::new();
    let mut archetypes = [sim.app.world().archetypes().len(); 3];
    // Held over the ticks, as one `Sim::work(n)` holds it: taking the
    // pool's threads again for every tick takes as long as the ticks.
    let _pool = chimera_core::metrics::work::hold_compute_pool();
    let mut played = 0;
    while played < MEASURED || quiet.len() < QUIET {
        assert!(
            played < MOST,
            "{map}: {} of {MOST} ticks had no ragdoll body awake and no new archetype ({QUIET} wanted): ragdolls that \
             never sleep, or an archetype made on most ticks",
            quiet.len()
        );
        played += 1;
        let w = sim.work(1)?;
        let tick = *w.ticks.first()?;
        archetypes = [archetypes[1], archetypes[2], *w.archetypes.first()?];
        if archetypes[0] != archetypes[1] || archetypes[1] != archetypes[2] {
            continue;
        }
        let world = sim.app.world_mut();
        let awake = world.query_filtered::<(), (With<RagdollBody>, Without<avian3d::prelude::Sleeping>)>().iter(world).count();
        if awake == 0 {
            quiet.push(tick);
            systems.add(&w.systems);
        }
    }
    let n = quiet.len() as u64;
    quiet.sort_unstable();
    quiet.get(quiet.len() / 2).map(|m| (*m as f64, systems.per(n)))
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
            let tick = tick_cost(&mut sim, map);
            let stale = stale_solver_bodies(&mut sim);
            assert_eq!(
                stale, 0,
                "lap {lap} {map}: {stale} of Avian's solver bodies are despawned entities' (every substep walks them: \
                 `chimera_world::map::solver_bodies`)"
            );
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

