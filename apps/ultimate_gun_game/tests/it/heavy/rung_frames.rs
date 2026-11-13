//! A real ladder's rungs cost no frame (backlog "Nothing blocks the main
//! thread"; docs/performance.md): on de_dust2 with the drawing's assets (no
//! window, no render world, no HUD or UI), a seeded ladder of Combat Arms and
//! CS:S guns, a bot walks every rung once what the ladder loads ahead has come
//! in, each rung's worst frame in main-thread instructions (its ticks
//! included: the simulation, the bodies and held models drawn, the loads'
//! arrival) under a limit. A HUD's first frame and the render thread are not
//! in it (the app's walk: docs/performance.md). Instructions because
//! the box is loaded; a read of an install file on the frame of a rung (30 ms)
//! shows at once. Passes with a note without the installs or counters.

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::{metrics::ThreadCounters, mount::config::LocalConfig};
use chimera_present::render::{bodies::PlayModels, prop_material::PropMaterial};
use chimera_sim::minigame::{self, ladder::{Ladder, set_rung}};
use chimera_world::harness::Sim;
use combat_arms::{
    game::TICK_INTERVAL,
    harness::CaSim,
    ui::visuals::CaVisualsPlugin,
};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

/// The worst frame of a rung change, ticks and all: 12.5-27.7 M measured
/// (2026-11-09, seed 36336's ladder here: CS:S, HL2:DM and Combat Arms' own
/// guns, not install ones, so no install body or LTB timing read); a rung that
/// read an LTB on the frame was 60-100 M more.
const WORST_RUNG_FRAME: u64 = 45_000_000;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

#[test]
fn a_ladders_rungs_stay_under_the_limit() {
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
                    .add_plugins(CaVisualsPlugin);
            },
            chimera_present::MapPlugin::new(map),
            PartyPartsPlugin,
            UltimateGunGamePlugin { seed: 36336, rungs: 13, ..Default::default() },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(3);
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 4;
        s.quota_set = true;
    }
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    // What the ladder loads ahead (a body's 0.5-1 s on a thread each): every
    // body that can arrive, then bodies drawn for 300 ticks.
    combat_arms::harness::ladder_bodies_in(&mut sim, 30, std::time::Duration::from_secs(240));
    let started = std::time::Instant::now();
    let mut settled = 0;
    while settled < 300 {
        let drawn = sim.app.world().get_resource::<PlayModels>().map_or(0, |p| p.bodies.len());
        settled = if drawn > 0 { settled + 1 } else { 0 };
        assert!(started.elapsed().as_secs() < 240, "no body drawn");
        sim.ticks(1);
    }
    // A bot walks the ladder (what a rung costs is the same for any player;
    // a local player's own view model and HUD are not in this headless world).
    let players: Vec<Entity> = {
        let w = sim.app.world_mut();
        w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect()
    };
    let Some(&player) = players.first() else {
        panic!("no bot joined");
    };
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let mut counters = ThreadCounters::current();
    if counters.read().hw.is_none() {
        eprintln!("skipping the limit: no hardware counters");
        return;
    }
    let mut worst_all = (0u64, 0usize);
    for rung in 0..ladder.rungs.len() {
        let a = counters.read().hw.unwrap().instructions;
        set_rung(sim.app.world_mut(), player, rung, &ladder);
        let mut worst = counters.read().hw.unwrap().instructions.saturating_sub(a);
        for _ in 0..30 {
            let a = counters.read().hw.unwrap().instructions;
            sim.app.update();
            worst = worst.max(counters.read().hw.unwrap().instructions.saturating_sub(a));
        }
        eprintln!("rung {:2} {:44} worst frame {:7.1} M", rung + 1, ladder.rungs[rung], worst as f64 / 1e6);
        if worst > worst_all.0 {
            worst_all = (worst, rung);
        }
        // Settle between rungs.
        sim.ticks(20);
    }
    assert!(
        worst_all.0 < WORST_RUNG_FRAME,
        "rung {} ({}): {} instructions on its worst frame (limit {WORST_RUNG_FRAME})",
        worst_all.1 + 1,
        ladder.rungs[worst_all.1],
        worst_all.0
    );
}
