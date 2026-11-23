//! What the games that are not in the match cost a tick
//! (docs/plans/active/game-logic-distribution.md): the party's simulation
//! composes every game's parts (`party_roster::PartyPartsPlugin`), so each
//! game's systems are registered whether its guns are in the ladder or not.
//! A match whose ladder draws from one game only (here Combat Arms', eight
//! of its bots), counted per system by work counts (docs/performance.md,
//! "Work counts") and summed by the crate each system comes from: the rows
//! of the games with no gun in the ladder are what an absent game costs.
//! Prints only (a measurement, not a bound): `cargo test -p
//! ultimate_gun_game --features dev --test it heavy::idle_games --
//! --nocapture`.

use std::collections::BTreeMap;

use bevy::{ecs::schedule::Schedules, prelude::*};
use chimera_core::core::SpawnPoint;
use chimera_sim::minigame;
use ultimate_gun_game::{MINIGAME, RUNGS, generate};

use crate::parts::{rungs, ugg};

/// The crate a system's type path starts in (`<impl ...>` and closures
/// included), `?` when there is none.
fn crate_of(full: &str) -> String {
    let path = full.trim_start_matches(['<', '&']);
    path.split("::")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("?")
        .to_string()
}

/// (schedule label, short name) to crate, and the registered systems per
/// crate over every initialized schedule.
fn systems_by_crate(world: &World) -> (BTreeMap<(String, String), String>, BTreeMap<String, usize>) {
    let mut owner = BTreeMap::new();
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    for (label, schedule) in world.resource::<Schedules>().iter() {
        let Ok(systems) = schedule.systems() else { continue };
        let label = format!("{label:?}");
        for (_, system) in systems {
            let name = system.name();
            let full = name.as_string();
            if full.contains("ApplyDeferred") {
                continue;
            }
            let krate = crate_of(&full);
            *count.entry(krate.clone()).or_default() += 1;
            owner.insert((label.clone(), name.shortname().to_string()), krate);
        }
    }
    (owner, count)
}

#[test]
fn absent_games_cost_a_tick() {
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
    generate(w, 7, RUNGS).expect("a ladder");
    chimera_core::gameplay::new_game(w);
    minigame::start(w, MINIGAME).expect("the match starts");
    sim.seconds(3.0);
    eprintln!("ladder: {:?}", rungs(sim.app.world()));
    const TICKS: u64 = 300;
    let Some(work) = sim.work(TICKS) else {
        eprintln!("skipping: no hardware counters");
        return;
    };
    let (owner, registered) = systems_by_crate(sim.app.world());
    let mut by_crate: BTreeMap<String, (u64, u64, usize)> = BTreeMap::new();
    for (label, (ins, runs)) in &work.systems.0 {
        let (schedule, short) = label.split_once(' ').unwrap_or(("", label));
        let krate = owner
            .get(&(schedule.to_string(), short.to_string()))
            .cloned()
            .unwrap_or_else(|| "?".to_string());
        let e = by_crate.entry(krate).or_default();
        e.0 += ins;
        e.1 += runs;
        e.2 += 1;
    }
    let total: u64 = by_crate.values().map(|v| v.0).sum();
    eprintln!(
        "{TICKS} ticks, {} systems registered, {} instructions a tick (mean)",
        registered.values().sum::<usize>(),
        total / TICKS
    );
    eprintln!(
        "{:<28} {:>10} {:>12} {:>16} {:>8}",
        "crate", "registered", "systems run", "instr/tick", "share"
    );
    let mut rows: Vec<_> = registered.keys().chain(by_crate.keys()).cloned().collect();
    rows.sort();
    rows.dedup();
    for k in rows {
        let (ins, _, ran) = by_crate.get(&k).copied().unwrap_or_default();
        eprintln!(
            "{:<28} {:>10} {:>12} {:>16} {:>7.2}%",
            k,
            registered.get(&k).copied().unwrap_or(0),
            ran,
            ins / TICKS,
            100.0 * ins as f64 / total.max(1) as f64
        );
    }
    for game in ["cs_source", "hl2dm", "tf2", "halo_ce", "source_logic"] {
        let mut top: Vec<_> = work
            .systems
            .0
            .iter()
            .filter(|(l, _)| {
                let (s, sh) = l.split_once(' ').unwrap_or(("", l));
                owner.get(&(s.to_string(), sh.to_string())).is_some_and(|c| c == game)
            })
            .collect();
        top.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        eprintln!(
            "{game}'s costliest: {:?}",
            top.iter()
                .take(5)
                .map(|(l, v)| (l.as_str(), v.0 / TICKS))
                .collect::<Vec<_>>()
        );
    }
}
