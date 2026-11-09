//! A session's materials don't pile up (docs/performance.md, "A session's
//! render thread"): with the drawing's assets (no window, no render world), a
//! seeded ladder played on each of several real maps in one process, the
//! server's map change between them. Every `PropMaterial` in the store is a
//! GPU buffer and bind group the render world prepares, and wgpu's per-pass
//! trackers stay sized by the most buffers ever alive; the kit's models
//! (every body, skin family, held, loose and view model of the map and the
//! ladder) wait outside it until something draws them
//! (`prop_material::DormantProps`). So the store holds about what is drawn,
//! and the same map holds the same after a lap. Prints a row per map
//! (`--nocapture`). Passes with a note without the installs.

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::mount::config::LocalConfig;
use chimera_present::render::prop_material::{DormantProps, PropMaterial};
use chimera_sim::minigame::{
    self,
    ladder::{Ladder, set_rung},
};
use chimera_world::{harness::Sim, map::LateCharacterModels};
use combat_arms::{
    game::{TICK_INTERVAL, characters::preload::LadderModels},
    harness::CaSim,
    ui::visuals::CaVisualsPlugin,
};
use party_kit::{PartyPartsPlugin, server::swap_map};
use ultimate_gun_game::{MINIGAME, RUNGS, UltimateGunGamePlugin, generate};

const MAPS: [&str; 4] = [
    "cs_source:de_dust2",
    "combat_arms:warehouse",
    "hl2dm:dm_lockdown",
    "cs_source:de_dust2",
];

/// Materials in the store that no entity draws, at most: the maps' props'
/// other skins, the view models shown once. Measured 23-33 (2026-11-09);
/// before the kit's waited outside the store, 1,600-2,000 here (10,000 in the
/// app on dm_lockdown, with a longer session's ladders).
const UNDRAWN_LIMIT: usize = 600;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

/// The ladder's models in, then a bot on every rung.
fn play(sim: &mut Sim, seed: u64) {
    let w = sim.app.world_mut();
    generate(w, seed, RUNGS).expect("a ladder");
    minigame::start(w, MINIGAME).expect("the match starts");
    let mut settled = 0;
    let started = std::time::Instant::now();
    while settled < 120 && started.elapsed().as_secs() < 240 {
        let w = sim.app.world();
        let want = w.resource::<LadderModels>().bodies.len();
        let have = w.get_resource::<LateCharacterModels>().map_or(0, |l| l.0.len());
        settled = if have >= want { settled + 1 } else { 0 };
        sim.ticks(1);
    }
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let bots: Vec<Entity> = {
        let w = sim.app.world_mut();
        w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect()
    };
    for r in 0..ladder.rungs.len() {
        for (i, b) in bots.iter().enumerate() {
            set_rung(sim.app.world_mut(), *b, (r + i * 3) % ladder.rungs.len(), &ladder);
        }
        sim.ticks(20);
    }
    sim.ticks(60);
}

/// The store's materials, those an entity draws, and the dormant ones.
fn count(w: &mut World) -> (usize, usize, usize) {
    let store = w.resource::<Assets<PropMaterial>>().len();
    let mut q = w.query::<&MeshMaterial3d<PropMaterial>>();
    let drawn: std::collections::HashSet<AssetId<PropMaterial>> = q.iter(w).map(|m| m.0.id()).collect();
    let dormant = w.get_resource::<DormantProps>().map_or(0, DormantProps::live);
    (store, drawn.len(), dormant)
}

#[test]
fn the_store_holds_what_is_drawn_map_after_map() {
    let Some(first) = party_kit::load_map(MAPS[0], 0).ok().filter(|_| combat_arms_installed()) else {
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
            chimera_present::MapPlugin::new(first),
            PartyPartsPlugin,
            UltimateGunGamePlugin::default(),
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(3);
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 4;
        s.quota_set = true;
    }
    let mut rows = Vec::new();
    for (i, map) in MAPS.iter().enumerate() {
        if i > 0 {
            let Ok(data) = party_kit::load_map(map, 0) else {
                eprintln!("{map}: not installed");
                continue;
            };
            swap_map(sim.app.world_mut(), map, Some(data));
        }
        play(&mut sim, 700 + i as u64);
        let (store, drawn, dormant) = count(sim.app.world_mut());
        eprintln!("{map:24} store {store:6} drawn {drawn:5} dormant {dormant:6}");
        rows.push((map, store, drawn));
    }
    for (map, store, drawn) in &rows {
        assert!(
            store - drawn < UNDRAWN_LIMIT,
            "{map}: {store} materials in the store (each a GPU buffer), {drawn} drawn: the kit's models' materials \
             went into the store instead of waiting outside it (`DormantProps`)"
        );
    }
    let (first, last) = (rows.first().unwrap(), rows.last().unwrap());
    if first.0 == last.0 {
        assert!(
            (last.1 as f64) < first.1 as f64 * 1.25 + 200.0,
            "{}: {} materials in the store at first, {} after the other maps",
            first.0,
            first.1,
            last.1
        );
    }
}
