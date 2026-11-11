//! What each bot costs the party's tick on de_dust2 (backlog 2026-10-28,
//! "more bots also seemed to make it slower"; docs/performance.md, "Map
//! changes that slowed the tick"): the gun game with 2, 4, 8 and 16
//! Combat Arms bots, the tick's instructions on the thread that runs the
//! schedules over 600 ticks of play (the mean and the median tick: the
//! fights differ from run to run, and the mean moves with a few big
//! ticks), printed as a table (`--nocapture`) with the poses and hitbox sets the
//! ticks made (Combat Arms' `CaPose`, `core::LazyHitboxes`);
//! with `--features dev,trace` also the systems that grow most from the
//! fewest bots to the most (`metrics::span_cost`). Checks only that the
//! cost per bot stays within reach of linear: 16 bots under 12x the
//! 2-bot tick. `CHIMERA_BOT_COST_GUNS=cs_source` puts every bot on the
//! ladder's first rung with a gun of that game (Chimera's hitscan on
//! Combat Arms' bodies: docs/performance.md, "A broad phase for the
//! hitscan"). Passes with a note without both installs (or without
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
    if let Some(game) = std::env::var("CHIMERA_BOT_COST_GUNS").ok().filter(|g| !g.is_empty()) {
        // Every bot on the ladder's first rung with a gun of that game
        // (`cs_source`: Chimera's hitscan on Combat Arms' bodies).
        let w = sim.app.world_mut();
        let ladder = w.resource::<chimera_sim::minigame::ladder::Ladder>().clone();
        let prefix = format!("{game}:");
        let rung = (0..ladder.rungs.len())
            .find(|&r| ladder.character(r).is_none() && ladder.rungs[r].starts_with(&prefix) && !ladder.rungs[r].contains("knife"))
            .unwrap_or_else(|| panic!("no rung with a {game} gun: {:?}", ladder.rungs));
        let bots: Vec<Entity> = w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect();
        for b in bots {
            chimera_sim::minigame::ladder::set_rung(w, b, rung, &ladder);
        }
        eprintln!("every bot on rung {rung}: {}", ladder.rungs[rung]);
        sim.seconds(1.0);
    }
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

/// The characters' poses (`CaPose`) and hitboxes made on demand
/// (`core::LazyHitboxes`) as a tick left them. Both are made when first
/// read, and the next tick's shots read them before Combat Arms' `drive`
/// replaces them: kept here, they tell afterwards how many were made.
#[derive(Default)]
struct Kept(Vec<combat_arms::game::characters::CaPose>, Vec<chimera_core::core::LazyHitboxes>);

impl Kept {
    /// How many of the kept ones were made (poses, hitbox sets), and keep
    /// this tick's instead.
    fn made(&mut self, sim: &mut Sim) -> (usize, usize) {
        let made = (
            self.0.iter().filter(|p| p.is_posed_any()).count(),
            self.1.iter().filter(|h| h.0.is_made()).count(),
        );
        let w = sim.app.world_mut();
        self.0 = w.query::<&combat_arms::game::characters::CaPose>().iter(w).cloned().collect();
        self.1 = w.query::<&chimera_core::core::LazyHitboxes>().iter(w).cloned().collect();
        made
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
        // Each tick's own count too: the median isn't moved by the odd
        // tick that does a large one-off job (a navigation flood, a bot's
        // first route), which the mean is.
        let mut each = Vec::with_capacity(ticks);
        let mut last = a;
        let mut extra = 0u64;
        let (mut posed, mut boxed) = (0usize, 0usize);
        let mut kept = Kept::default();
        kept.made(&mut sim);
        for _ in 0..ticks {
            sim.ticks(1);
            let now = instructions().unwrap_or(last);
            each.push(now.saturating_sub(last));
            // What the tick made of the characters' poses and hitboxes
            // (counted outside the tick's instructions).
            let (p, b) = kept.made(&mut sim);
            posed += p;
            boxed += b;
            last = instructions().unwrap_or(now);
            extra += last.saturating_sub(now);
        }
        each.sort_unstable();
        let median = each[ticks / 2] as f64;
        let wall = t0.elapsed().as_secs_f64() * 1e3 / ticks as f64;
        let b = instructions().unwrap_or(a).saturating_sub(extra);
        #[cfg(feature = "trace")]
        costs.push(spans.as_ref().map(|s| s.take()));
        let n = {
            let w = sim.app.world_mut();
            w.query_filtered::<(), With<combat_arms::game::bots::CaBot>>().iter(w).count()
        };
        rows.push((bots, n, b.saturating_sub(a) as f64 / ticks as f64, median, wall, posed as f64 / ticks as f64, boxed as f64 / ticks as f64));
    }
    eprintln!("bots (in game) | instructions per tick | the median tick | per bot over 2 bots | wall ms per tick (load-dependent) | poses made a tick | hitbox sets made a tick");
    let base = rows[0].2;
    for (bots, n, ins, median, wall, posed, boxed) in &rows {
        let per = if *bots > 2 { (ins - base) / (*bots - 2) as f64 } else { 0.0 };
        eprintln!(
            "{bots:4} ({n:2}) | {:8.2}M | {:8.2}M | {:6.3}M | {wall:6.2} | {posed:5.2} | {boxed:5.2}",
            ins / 1e6,
            median / 1e6,
            per / 1e6
        );
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
