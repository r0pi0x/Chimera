//! What the games that are not in the match cost a tick
//! (docs/plans/active/game-logic-distribution.md): the party's simulation
//! composes every game's parts (`party_roster::PartyPartsPlugin`), and each
//! guest game's systems run only while it is in play
//! (`chimera_core::games_in_play`). A match whose ladder draws from one
//! game only (here Combat Arms', the base game, eight of its bots),
//! counted per system by work counts (docs/performance.md, "Work counts")
//! and summed by the crate each system comes from: the games with no gun
//! in the ladder must cost under 1% of the tick, and the same match with
//! every game put in play (what every match cost before the gating) is
//! printed beside it. A game entering play mid-match (a character handed
//! its gun) runs its systems in that same tick.
//! `cargo test -p ultimate_gun_game --features dev --test it
//! heavy::idle_games -- --nocapture`.

use std::collections::BTreeMap;

use bevy::{ecs::schedule::Schedules, prelude::*};
use chimera_core::{core::SpawnPoint, games_in_play::GamesInPlay};
use chimera_sim::minigame::{self, ladder::Ladder};
use chimera_world::harness::Sim;
use ultimate_gun_game::{MINIGAME, RUNGS, generate};

use crate::parts::{rungs, ugg};

/// The games that are not in the match: every gated one (the guests;
/// Combat Arms is the base).
fn absent(world: &World) -> Vec<&'static str> {
    world.resource::<GamesInPlay>().games().map(|(g, _)| g).collect()
}

/// Registered systems per crate over every schedule.
fn systems_by_crate(world: &World) -> BTreeMap<String, usize> {
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    for (_, schedule) in world.resource::<Schedules>().iter() {
        let Ok(systems) = schedule.systems() else { continue };
        for (_, system) in systems {
            let full = system.name().as_string();
            if full.contains("ApplyDeferred") {
                continue;
            }
            *count.entry(chimera_core::games_in_play::crate_of(&full).to_string()).or_default() += 1;
        }
    }
    count
}

/// The UGG match of Combat Arms' guns only, eight Combat Arms bots on the
/// greybox, 3 s in; `all_in_play`: every game put in play first.
fn combat_arms_match(all_in_play: bool) -> Sim {
    let mut sim = ugg();
    let w = sim.app.world_mut();
    w.insert_resource(chimera_net::net::manifest::LocalMounts(
        chimera_net::net::manifest::MountReport {
            games: vec![chimera_net::net::manifest::GameMount {
                game: "combat_arms".to_string(),
                installed: true,
                ..Default::default()
            }],
        },
    ));
    sim.repeatable();
    for x in [-8.0, 8.0] {
        for z in [-25.0, -15.0, -5.0, 5.0] {
            sim.app
                .world_mut()
                .spawn((SpawnPoint::default(), Transform::from_translation(Vec3::new(x, 1.0, z))));
        }
    }
    {
        let mut s = sim
            .app
            .world_mut()
            .resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 8;
        s.quota_set = true;
    }
    let w = sim.app.world_mut();
    if all_in_play {
        for game in absent(w) {
            chimera_core::games_in_play::enter(w, game, "the test");
        }
    }
    generate(w, 7, RUNGS).expect("a ladder");
    chimera_core::gameplay::new_game(w);
    minigame::start(w, MINIGAME).expect("the match starts");
    sim.seconds(3.0);
    sim
}

fn in_play(sim: &Sim) -> Vec<(&'static str, bool)> {
    sim.app.world().resource::<GamesInPlay>().games().collect()
}

#[test]
fn absent_games_cost_under_a_percent_of_a_tick() {
    const TICKS: u64 = 300;
    let mut rows = Vec::new();
    for all in [false, true] {
        let mut sim = combat_arms_match(all);
        eprintln!("ladder: {:?}; in play: {:?}", rungs(sim.app.world()), in_play(&sim));
        if !all {
            for (game, on) in in_play(&sim) {
                let why = &sim.app.world().resource::<GamesInPlay>().why;
                assert!(!on, "{game} is in play in a match of Combat Arms' guns only: {why:?}");
            }
        }
        let Some(work) = sim.work(TICKS) else {
            eprintln!("skipping: no hardware counters");
            return;
        };
        let registered = systems_by_crate(sim.app.world());
        let tick = work.total() / TICKS;
        let systems: u64 = work.crates.values().map(|v| v.0).sum::<u64>() / TICKS;
        eprintln!(
            "{}: {TICKS} ticks, {} systems registered, {tick} instructions a tick (mean), {systems} in systems",
            if all { "every game in play (as before the gating)" } else { "gated" },
            registered.values().sum::<usize>(),
        );
        eprintln!("{:<28} {:>10} {:>12} {:>16} {:>8}", "crate", "registered", "runs/tick", "instr/tick", "share");
        let mut keys: Vec<_> = registered.keys().chain(work.crates.keys()).cloned().collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let (ins, runs) = work.crates.get(&k).copied().unwrap_or_default();
            eprintln!(
                "{:<28} {:>10} {:>12} {:>16} {:>7.2}%",
                k,
                registered.get(&k).copied().unwrap_or(0),
                runs / TICKS,
                ins / TICKS,
                100.0 * ins as f64 / work.total().max(1) as f64
            );
        }
        let absent: u64 =
            absent(sim.app.world()).iter().map(|g| work.crates.get(*g).map_or(0, |v| v.0)).sum::<u64>() / TICKS;
        eprintln!("absent games' systems: {absent} instructions a tick ({:.2}%)", 100.0 * absent as f64 / tick as f64);
        rows.push((tick, absent));
    }
    let ((gated, absent), (ungated, _)) = (rows[0], rows[1]);
    eprintln!(
        "tick: {gated} gated, {ungated} with every game in play: {:.1}% saved",
        100.0 * (ungated as f64 - gated as f64) / ungated as f64
    );
    assert!(
        absent * 100 < gated,
        "the games not in play cost {absent} instructions of a {gated}-instruction tick (bound: 1%): a guest game's \
         system runs while its game is out of play (chimera_core::games_in_play)"
    );
}

#[test]
fn a_game_entering_play_mid_match_runs_its_systems_that_tick() {
    let mut sim = combat_arms_match(false);
    let world = sim.app.world_mut();
    let e = world
        .query_filtered::<Entity, With<minigame::ladder::Rung>>()
        .iter(world)
        .next()
        .expect("a player on the ladder");
    assert!(!world.resource::<GamesInPlay>().is_in_play("hl2dm"), "{:?}", world.resource::<GamesInPlay>().why);
    // A rung of a game the match had none of: its gun handed over.
    let mut ladder = world.resource::<Ladder>().clone();
    ladder.rungs[0] = "hl2dm:weapon_smg1";
    minigame::ladder::set_rung(world, e, 0, &ladder);
    assert!(
        world.resource::<GamesInPlay>().is_in_play("hl2dm"),
        "HL2:DM is in play once a character holds its gun"
    );
    let Some(work) = sim.work(1) else {
        eprintln!("skipping the systems' runs: no hardware counters");
        return;
    };
    let runs = work.crates.get("hl2dm").map_or(0, |v| v.1);
    assert!(runs > 0, "HL2:DM's systems ran in the tick it entered play");
    assert!(
        work.crates.get("halo_ce").is_none_or(|v| v.1 == 0),
        "Halo's stay off: {:?}",
        work.crates.get("halo_ce")
    );
}
