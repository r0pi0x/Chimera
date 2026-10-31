//! What each bot costs the party's tick on de_dust2 (backlog 2026-10-28,
//! "more bots also seemed to make it slower"; docs/performance.md, "Map
//! changes that slowed the tick"): the gun game with 2, 4, 8 and 16
//! Combat Arms bots, the tick's instructions on the thread that runs the
//! schedules over 600 ticks of play, printed as a table (`--nocapture`);
//! with `--features dev,trace` also the systems that grow most from the
//! fewest bots to the most (`metrics::span_cost`). Checks only that the
//! cost per bot stays within reach of linear: 16 bots under 12x the
//! 2-bot tick. Passes with a note without both installs (or without
//! hardware counters).

use bevy::prelude::*;
use chimera_core::{metrics::ThreadCounters, mount::config::LocalConfig};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

/// The gun game on de_dust2 with `bots` Combat Arms bots, playing; None
/// without both installs.
fn dust2_with(bots: u32) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    let map = party_kit::load_map("cs_source:de_dust2", 0).unwrap();
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = bots;
        s.quota_set = true;
    }
    // The bots in (one a tick), the navigation mesh made, a few fights.
    sim.seconds(8.0);
    Some(sim)
}

#[cfg(feature = "trace")]
thread_local!(static SPANS: std::cell::RefCell<Option<std::rc::Rc<chimera_core::metrics::span_cost::SpanCosts>>> = const { std::cell::RefCell::new(None) });

fn instructions() -> Option<u64> {
    #[cfg(feature = "trace")]
    if let Some(n) = SPANS.with(|s| s.borrow().as_ref().map(|s| s.instructions())) {
        return n;
    }
    {
        thread_local!(static C: std::cell::RefCell<ThreadCounters> = std::cell::RefCell::new(ThreadCounters::current()));
        C.with(|c| {
            let mut c = c.borrow_mut();
            c.read();
            c.read().hw.map(|h| h.instructions)
        })
    }
}

#[test]
fn each_bot_costs_about_the_same() {
    #[cfg(feature = "trace")]
    let spans = chimera_core::metrics::span_cost::SpanCosts::install().map(std::rc::Rc::new);
    #[cfg(feature = "trace")]
    SPANS.with(|s| *s.borrow_mut() = spans.clone());
    let mut rows = Vec::new();
    #[cfg(feature = "trace")]
    let mut costs = Vec::new();
    for bots in [2u32, 4, 8, 16] {
        let Some(mut sim) = dust2_with(bots) else { return };
        let ticks = 600;
        #[cfg(feature = "trace")]
        let _ = spans.as_ref().map(|s| s.take());
        let (Some(a), t0) = (instructions(), std::time::Instant::now()) else {
            eprintln!("skipping: no hardware counters ({})", chimera_core::metrics::hw_status());
            return;
        };
        sim.ticks(ticks);
        let wall = t0.elapsed().as_secs_f64() * 1e3 / ticks as f64;
        let b = instructions().unwrap_or(a);
        #[cfg(feature = "trace")]
        costs.push(spans.as_ref().map(|s| s.take()));
        let n = {
            let w = sim.app.world_mut();
            w.query_filtered::<(), With<combat_arms::game::bots::CaBot>>().iter(w).count()
        };
        rows.push((bots, n, b.saturating_sub(a) as f64 / ticks as f64, wall));
    }
    eprintln!("bots (in game) | instructions per tick | per bot over 2 bots | wall ms per tick (load-dependent)");
    let base = rows[0].2;
    for (bots, n, ins, wall) in &rows {
        let per = if *bots > 2 { (ins - base) / (*bots - 2) as f64 } else { 0.0 };
        eprintln!("{bots:4} ({n:2}) | {:8.2}M | {:6.3}M | {wall:6.2}", ins / 1e6, per / 1e6);
    }
    #[cfg(feature = "trace")]
    if let (Some(Some(first)), Some(Some(last))) = (costs.first(), costs.last()) {
        eprintln!("systems growing most from 2 to 16 bots (instructions per call, calls in 600 ticks):");
        for (label, was, now, calls) in chimera_core::metrics::span_cost::rises(first, last, 25) {
            eprintln!("{was:10.0} -> {now:10.0} x {calls:6}  {label}");
        }
    }
    let (two, sixteen) = (rows[0].2, rows[3].2);
    assert!(
        sixteen < two * 12.0,
        "16 bots cost {:.1}x the 2-bot tick: a per-bot cost grows with the bots",
        sixteen / two
    );
}
