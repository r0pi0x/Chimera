//! A tick of the gun game on a map with a large generated navigation mesh
//! stays under a budget of instructions (docs/performance.md, "cs_militia's
//! spikes"): cs_militia's mesh had ten times de_dust2's areas (11,510
//! against 1,123), and the bots' questions of it (`NavMesh::area_at`, a
//! search of every area for each point of a straight-walk test) made
//! single ticks three to four times the median. Its player clips now keep
//! the mesh off the hills (1,604 areas), so the large mesh guarded is
//! Combat Arms' deserthole (7,850 areas, no clips; with `area_at`
//! searching every area its bots' thinking is 10.6 M a tick and its
//! costliest steady ticks 109 M, both over the limits); cs_militia keeps
//! the same budget. Headless, the automatic gun game with six bots, the
//! floods at once (no `SliceFloods`), repeatable (`Sim::repeatable`), work
//! counts (`Sim::work`): the same on every run of the same build, whatever
//! the machine's load.
//!
//! `CHIMERA_MAP_TICKS=<game:map>` counts another map instead and prints
//! it, checking nothing (the references: `cs_source:de_dust2`,
//! `cs_source:cs_office`). Passes with a note without the installs or
//! hardware counters.

use bevy::prelude::*;
use chimera_core::{metrics::work::WorkCounts, mount::config::LocalConfig};
use chimera_sim::minigame;
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

/// The large generated mesh guarded.
const LARGE: &str = "combat_arms:deserthole";
/// Fewest areas `LARGE`'s mesh must keep to be the large mesh guarded.
const LARGE_AREAS: usize = 5000;
const MILITIA: &str = "cs_source:cs_militia";
const BOTS: u32 = 6;
/// Ticks before the count: the floods, the first spawns, the props at rest.
const WARM_UP: u64 = 600;
/// Ticks counted (about 50 s of play at Combat Arms' tick).
const TICKS: usize = 3000;

/// The bots' thinking a tick, the mean over the counted ticks
/// (`bots::brain::think`), measured 2026-11-13: deserthole 0.29 M (10.6 M
/// with `area_at` searching every area), cs_militia 0.39 M (5.47 M with
/// its 11,510 areas searched; 2.75 M with bots spawned on floors apart
/// from the rest of their mesh, chasing each other without a path),
/// de_dust2 0.39 M.
const BOTS_PER_TICK: u64 = 1_500_000;

/// The costliest tenth of a percent of the ticks that made no archetype
/// (a death's or a ragdoll's new tables cost Bevy's one-off matching,
/// which is another matter), measured: deserthole 11.9 M (108.5 M with
/// `area_at` searching every area), cs_militia 17.1 M (30.4 M with its
/// 11,510 areas, a path search over the whole mesh; 97.2 M searching
/// every area).
const WORST_TICKS: u64 = 45_000_000;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

fn quantile(sorted: &[u64], q: f64) -> u64 {
    sorted
        .get(((sorted.len() as f64 * q) as usize).min(sorted.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0)
}

#[test]
fn a_tick_on_a_large_mesh_stays_in_its_budget() {
    let other = std::env::var("CHIMERA_MAP_TICKS").ok();
    let name = other.as_deref().unwrap_or(LARGE);
    let check = other.is_none();
    let Some(areas) = within_budget(name, check) else { return };
    if check {
        assert!(
            areas > LARGE_AREAS,
            "{name}'s generated mesh has {areas} areas: no longer the large mesh this guards"
        );
    }
}

#[test]
fn a_tick_on_cs_militia_stays_in_its_budget() {
    if std::env::var("CHIMERA_MAP_TICKS").is_ok() {
        return;
    }
    within_budget(MILITIA, true);
}

/// Counts `name`'s ticks and prints them; with `check`, fails a tick over
/// the budget. The number of areas of its navigation mesh; None when
/// skipped.
fn within_budget(name: &str, check: bool) -> Option<usize> {
    let Some(map) = party_kit::load_map(name, 0).ok().filter(|_| combat_arms_installed()) else {
        eprintln!("skipping {name}: needs its game's and the Combat Arms installs");
        return None;
    };
    let mut sim = Sim::new(
        CaSim,
        (
            MapSimPlugin::new(map),
            PartyPartsPlugin,
            UltimateGunGamePlugin {
                seed: 36336,
                rungs: 13,
                ..Default::default()
            },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    sim.repeatable();
    {
        let mut s = sim
            .app
            .world_mut()
            .resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = BOTS;
        s.quota_set = true;
    }
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    sim.ticks(WARM_UP);
    let areas = sim
        .app
        .world()
        .get_resource::<chimera_world::map::nav::NavMesh>()
        .map_or(0, |n| n.areas.len());
    let (mut ticks, mut steady, mut systems) = (Vec::new(), Vec::new(), WorkCounts::default());
    let mut worst: (u64, WorkCounts) = (0, WorkCounts::default());
    let mut archetypes = [0usize; 2];
    for _ in 0..TICKS {
        let Some(w) = sim.work(1) else {
            eprintln!("skipping: no hardware counters");
            return None;
        };
        let (spent, now) = (w.ticks[0], w.archetypes[0]);
        ticks.push(spent);
        systems.add(&w.systems);
        if archetypes == [now; 2] {
            steady.push(spent);
            if spent > worst.0 {
                worst = (spent, w.systems);
            }
        }
        archetypes = [archetypes[1], now];
    }
    let of_bots = |w: &WorkCounts| {
        w.0.iter()
            .filter(|(k, _)| k.ends_with(" think"))
            .map(|(_, v)| v.0)
            .sum::<u64>()
    };
    let bots = of_bots(&systems) / TICKS as u64;
    ticks.sort_unstable();
    steady.sort_unstable();
    let m = |n: u64| n as f64 / 1e6;
    let top = |w: &WorkCounts, per: u64| {
        w.top(25)
            .into_iter()
            .map(|(l, i, _)| format!("{:>10.3} M  {l}", m(i / per)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let worst_ticks = quantile(&steady, 0.999);
    eprintln!(
        "{name}: {areas} navigation areas, {BOTS} bots, {TICKS} ticks: median {:.2} M, p99 {:.2} M, max {:.2} M; the {} that made \
         no archetype: median {:.2} M, p99 {:.2} M, p99.9 {:.2} M, max {:.2} M; the bots' thinking {:.3} M a tick ({:.2} M in the \
         costliest steady tick)\nsystems, a tick:\n{}\nthe costliest steady tick's:\n{}",
        m(quantile(&ticks, 0.5)),
        m(quantile(&ticks, 0.99)),
        m(*ticks.last().unwrap()),
        steady.len(),
        m(quantile(&steady, 0.5)),
        m(quantile(&steady, 0.99)),
        m(worst_ticks),
        m(worst.0),
        m(bots),
        m(of_bots(&worst.1)),
        top(&systems, TICKS as u64),
        top(&worst.1, 1),
    );
    if !check {
        return Some(areas);
    }
    assert!(
        bots < BOTS_PER_TICK,
        "the bots' thinking costs {bots} instructions a tick (limit {BOTS_PER_TICK}); the systems, a tick:\n{}",
        top(&systems, TICKS as u64)
    );
    assert!(
        worst_ticks < WORST_TICKS,
        "the costliest steady ticks reach {worst_ticks} instructions (limit {WORST_TICKS}); the costliest's systems:\n{}",
        top(&worst.1, 1)
    );
    Some(areas)
}
