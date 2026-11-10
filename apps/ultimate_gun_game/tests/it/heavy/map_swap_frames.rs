//! A map change behind the loading screen costs no frame (backlog "Nothing
//! blocks the main thread"; docs/performance.md, "A map's swap"): the
//! shell's `map` flow (`console::start_map_load`, then `finish_map_load`
//! each frame: the load on a task, the spawn a step a frame, the settling) on the gun
//! game's simulation with the drawing's asset stores (no window, no render
//! world), over maps of every game in a row. Every frame from the command
//! to entering the game is counted in main-thread instructions; the worst
//! must stay under a limit. Prints each map's row (`--nocapture`):
//! `CHIMERA_SPAWN_AT_ONCE=1` spawns each map in one frame (the A/B),
//! `CHIMERA_SWAP_MAPS=<id>,<id>` loads those maps instead and
//! `CHIMERA_SWAP_FRAMES=1` prints each frame from the load's end with its
//! `CLOCK_MONOTONIC` span (to cut it out of a `perf record -k
//! CLOCK_MONOTONIC` profile). Passes with a note without the installs or
//! counters.

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_client::client::console::{self, MapLoadStage};
use chimera_core::{metrics::ThreadCounters, mount::config::LocalConfig};
use chimera_present::render::{
    beams::BeamMaterial, detail::DetailMaterial, prop_material::PropMaterial, rope_material::RopeMaterial,
    shadows::ShadowMaterial, sprite_material::SpriteMaterial, water::WaterMaterial, world_material::WorldMaterial,
};
use chimera_world::harness::Sim;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim, ui::visuals::CaVisualsPlugin};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::UltimateGunGamePlugin;

const MAPS: [&str; 4] = [
    "cs_source:de_dust2",
    "cs_source:cs_office",
    "hl2dm:dm_lockdown",
    "combat_arms:warehouse",
];

/// The worst frame from a `map` command to entering the game, its tick
/// included (instructions, this test's dev build): 150-340 M measured
/// (2026-11-09: the frame the whole map goes into the world, and the first
/// tick on it); the map spawned in one frame was 0.7-1.3 G with its parts
/// prepared ahead, 1.0-2.9 G before.
const WORST_LOAD_FRAME: u64 = 500_000_000;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

fn sim() -> Sim {
    let mut sim = Sim::new(
        CaSim,
        (
            |app: &mut App| {
                app.init_asset::<Image>()
                    .init_asset::<StandardMaterial>()
                    .init_asset::<WorldMaterial>()
                    .init_asset::<PropMaterial>()
                    .init_asset::<SpriteMaterial>()
                    .init_asset::<RopeMaterial>()
                    .init_asset::<ShadowMaterial>()
                    .init_asset::<WaterMaterial>()
                    .init_asset::<BeamMaterial>()
                    .init_asset::<DetailMaterial>()
                    .init_asset::<SkinnedMeshInverseBindposes>()
                    .add_plugins(CaVisualsPlugin)
                    .insert_resource(party_kit::client_hooks())
                    .add_systems(Update, console::finish_map_load);
            },
            chimera_present::MapPlugin::empty(),
            PartyPartsPlugin,
            UltimateGunGamePlugin { seed: 36336, rungs: 13, ..Default::default() },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(3);
    sim
}

/// One map's load, frame by frame: (instructions, phase, wall ms of the
/// update) with phase 0 loading on the task, 1 spawning (from the frame the
/// load is done), 2 settling.
struct Load {
    frames: Vec<(u64, u8, f64)>,
    secs: f64,
}

impl Load {
    fn worst(&self, phase: u8) -> f64 {
        self.frames.iter().filter(|f| f.1 == phase).map(|f| f.0).max().unwrap_or(0) as f64 / 1e6
    }

    fn count(&self, phase: u8) -> usize {
        self.frames.iter().filter(|f| f.1 == phase).count()
    }

    fn ms(&self, phase: u8) -> f64 {
        self.frames.iter().filter(|f| f.1 == phase).map(|f| f.2).sum()
    }

    fn total(&self, phase: u8) -> f64 {
        self.frames.iter().filter(|f| f.1 == phase).map(|f| f.0).sum::<u64>() as f64 / 1e6
    }
}

fn load(sim: &mut Sim, counters: &mut ThreadCounters, id: &str) -> Load {
    let started = std::time::Instant::now();
    console::start_map_load(sim.app.world_mut(), id, None);
    let mut frames = Vec::new();
    while console::map_loading(sim.app.world()) {
        assert!(started.elapsed().as_secs() < 120, "{id}: not in after 120 s");
        let before = console::map_load_stage(sim.app.world());
        let a = counters.read().hw.map_or(0, |h| h.instructions);
        let t0 = chimera_core::metrics::counters::monotonic_ns();
        let wall = std::time::Instant::now();
        sim.app.update();
        let ms = wall.elapsed().as_secs_f64() * 1e3;
        let used = counters.read().hw.map_or(0, |h| h.instructions).saturating_sub(a);
        let after = console::map_load_stage(sim.app.world());
        // `CHIMERA_SWAP_FRAMES=1`: each frame from the load's end, to cut out
        // of a profile (`perf record -k CLOCK_MONOTONIC`).
        if (before, after) != (MapLoadStage::Loading, MapLoadStage::Loading)
            && chimera_core::env::var("CHIMERA_SWAP_FRAMES").is_ok()
        {
            eprintln!(
                "{id}: frame {} {before:?} -> {after:?} {:.1} M at monotonic {:?}..{:?} ns",
                frames.len(),
                used as f64 / 1e6,
                t0,
                chimera_core::metrics::counters::monotonic_ns()
            );
        }
        let phase = match (before, after) {
            (MapLoadStage::Loading, MapLoadStage::Loading) => 0,
            (MapLoadStage::Loading | MapLoadStage::Spawning, _) => 1,
            _ => 2,
        };
        frames.push((used, phase, ms));
        // A frame's pace, so the load's threads run beside it.
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    Load { frames, secs: started.elapsed().as_secs_f64() }
}

#[test]
fn a_map_change_costs_no_frame() {
    let ready = MAPS.iter().all(|id| party_kit::load_map(id, 0).is_ok()) && combat_arms_installed();
    if !ready {
        eprintln!("skipping: needs the CS:S, HL2:DM and Combat Arms installs");
        return;
    }
    let mut sim = sim();
    let mut counters = ThreadCounters::current();
    let counted = counters.read().hw.is_some();
    let mut worst_all = (0u64, "");
    // `CHIMERA_SWAP_MAPS=<id>,<id>`: these maps instead (for a profile).
    let chosen = chimera_core::env::var("CHIMERA_SWAP_MAPS").ok();
    let maps: Vec<&str> = match &chosen {
        Some(list) => list.split(',').collect(),
        None => MAPS.to_vec(),
    };
    for id in maps {
        let l = load(&mut sim, &mut counters, id);
        let worst = l.frames.iter().map(|f| f.0).max().unwrap_or(0);
        eprintln!(
            "{id:22} {:5.2} s: loading {:4} frames (worst {:5.1} M), spawning {:2} frames in {:5.1} ms, {:6.1} M (worst {:5.1} M), settling {:2} frames in {:5.1} ms (worst {:5.1} M)",
            l.secs,
            l.count(0),
            l.worst(0),
            l.count(1),
            l.ms(1),
            l.total(1),
            l.worst(1),
            l.count(2),
            l.ms(2),
            l.worst(2),
        );
        if worst > worst_all.0 {
            worst_all = (worst, id);
        }
        // Into the game a little, as a player would be.
        sim.ticks(30);
    }
    if !counted {
        eprintln!("skipping the limit: no hardware counters");
        return;
    }
    assert!(
        worst_all.0 < WORST_LOAD_FRAME,
        "{}: {} instructions on its worst frame between the map command and the game (limit {WORST_LOAD_FRAME})",
        worst_all.1,
        worst_all.0
    );
}
