//! A death costs no frame (backlog "Nothing blocks the main thread";
//! docs/performance.md, "A death's frame, a new ladder's frame"): the
//! automatic gun game on
//! de_dust2 with the drawing's assets (no window, no render world, no HUD),
//! six bots fighting, repeatable (`Sim::repeatable`), every tick counted
//! (`Sim::work`). The ticks a character died in (its ragdoll's bodies and
//! joints spawned, its drawn body handed over, the kill's rung) are held
//! against the ticks nothing died in: the costliest death tick's extra
//! instructions stay under `DEATH_TICK`. Prints each death tick and the
//! systems of the costliest (`--nocapture`). Passes with a note without the
//! installs or hardware counters.

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::{core::Died, metrics::work::WorkCounts, mount::config::LocalConfig};
use chimera_present::render::prop_material::PropMaterial;
use chimera_sim::minigame;
use chimera_world::{harness::Sim, map::Ragdoll};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use combat_arms_ui::visuals::CaVisualsPlugin;
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

const BOTS: u32 = 6;
/// Ticks counted (about 50 s of play at Combat Arms' tick).
const TICKS: usize = 3000;
/// A death tick's instructions over the median tick's, the costliest of
/// the run (measured 2026-11-14: 15.3 M, a model's first death; 18.8 M
/// when that death made its ragdoll's hulls; docs/performance.md, "A
/// death's frame, a new ladder's frame").
const DEATH_TICK: u64 = 30_000_000;

#[derive(Resource, Default)]
struct Deaths(u32);

fn count_deaths(mut died: MessageReader<Died>, mut deaths: ResMut<Deaths>) {
    deaths.0 += died.read().count() as u32;
}

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

#[test]
fn a_death_costs_no_frame() {
    let Some(map) = party_kit::load_map("cs_source:de_dust2", 0).ok().filter(|_| combat_arms_installed()) else {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    };
    let mut sim = Sim::new(
        CaSim,
        (
            |app: &mut App| {
                app.init_asset::<Image>()
                    .init_asset::<StandardMaterial>()
                    .init_asset::<PropMaterial>()
                    .init_asset::<SkinnedMeshInverseBindposes>()
                    .add_plugins(CaVisualsPlugin)
                    .init_resource::<Deaths>()
                    .add_systems(Last, count_deaths);
            },
            chimera_present::MapPlugin::new(map),
            PartyPartsPlugin,
            UltimateGunGamePlugin { seed: 36336, rungs: 13, ..Default::default() },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    sim.repeatable();
    sim.ticks(3);
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = BOTS;
        s.quota_set = true;
    }
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    combat_arms::harness::ladder_models_in(&mut sim, 30, std::time::Duration::from_secs(240));
    sim.ticks(300);
    let ragdolls = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<&Ragdoll>().iter(w).count()
    };
    let (mut quiet, mut deaths) = (Vec::new(), Vec::new());
    let mut quiet_systems = WorkCounts::default();
    let (mut seen, mut bodies) = (sim.app.world().resource::<Deaths>().0, ragdolls(&mut sim));
    let mut archetypes = sim.app.world().archetypes().len();
    for tick in 0..TICKS {
        let Some(w) = sim.work(1) else {
            eprintln!("skipping: no hardware counters");
            return;
        };
        let (now, lying) = (sim.app.world().resource::<Deaths>().0, ragdolls(&mut sim));
        let made = w.archetypes[0] - archetypes;
        archetypes = w.archetypes[0];
        if now > seen {
            deaths.push((w.ticks[0], tick, now - seen, lying as i64 - bodies as i64, made, w.systems));
        } else {
            quiet.push(w.ticks[0]);
            quiet_systems.add(&w.systems);
        }
        (seen, bodies) = (now, lying);
    }
    quiet.sort_unstable();
    let median = quiet[quiet.len() / 2];
    let m = |n: u64| n as f64 / 1e6;
    eprintln!(
        "{} deaths in {TICKS} ticks; a tick without one: median {:.2} M, p99 {:.2} M, max {:.2} M",
        deaths.len(),
        m(median),
        m(quiet[quiet.len() * 99 / 100]),
        m(*quiet.last().unwrap())
    );
    for (spent, tick, died, ragdolls, made, _) in &deaths {
        eprintln!(
            "tick {tick:4}: {:6.2} M ({died} died, {ragdolls:+} ragdolls, {made} new archetypes)",
            m(*spent)
        );
    }
    assert!(!deaths.is_empty(), "nobody died in {TICKS} ticks: the scenario measures nothing");
    // Each death's systems against a quiet tick's: the first pays its
    // model's and archetypes' one-off costs.
    let base = quiet_systems.per(quiet.len() as u64);
    for d in &deaths {
        eprintln!("tick {}'s systems over a quiet tick's:\n{}", d.1, d.5.diff_table(&base, 14));
    }
    let worst = deaths.iter().max_by_key(|d| d.0).unwrap();
    let mut sorted: Vec<u64> = deaths.iter().map(|d| d.0).collect();
    sorted.sort_unstable();
    eprintln!(
        "a death's tick: median {:.2} M, max {:.2} M; over the quiet median: {:.2} M, {:.2} M",
        m(sorted[sorted.len() / 2]),
        m(worst.0),
        m(sorted[sorted.len() / 2].saturating_sub(median)),
        m(worst.0.saturating_sub(median))
    );
    assert!(
        worst.0.saturating_sub(median) < DEATH_TICK,
        "a death's tick costs {} instructions over the median tick's {median} (limit {DEATH_TICK}); its systems:\n{}",
        worst.0 - median,
        worst.5.diff_table(&base, 20)
    );
}
