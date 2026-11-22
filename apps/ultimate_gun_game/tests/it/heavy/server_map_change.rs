//! A dedicated server's map change costs no frame (backlog "Nothing blocks
//! the main thread"; docs/performance.md, "A dedicated server's map
//! change"): Ultimate Gun Game's server as its binary composes it
//! (`ServerPlugin` over the party's `GameServer` and its launch hook, bots by
//! `+MaxPlayer`), a new ladder on another map each time (`ugg_new <seed>
//! <rungs> <map>`: the rotation's path, `change_match`, `changelevel`), over
//! maps of every game in a row. Every frame from the command until
//! `PLAY_TICKS` of the new map's match is counted in main-thread
//! instructions (the load's task, the navigation mesh's and the freed
//! map's are other threads'); the worst must stay under `WORST_FRAME`. The
//! session plays out the same on every run (`Sim::repeatable`: a task's
//! result on the frame that first asks; `prepare::StepAFrame`: a step of
//! the spawn a frame whatever the machine's speed). Prints each map's row
//! and its frames over `SHOWN` with their costliest systems
//! (`--nocapture`); `CHIMERA_SWAP_MAPS=<id>,<id>` loads those maps instead,
//! `CHIMERA_SPAWN_AT_ONCE=1` changes map as before the change was spread
//! (the whole map in one frame, floods whole in a tick: the A/B, which
//! fails the limit). Passes with a note without the installs or counters.

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::{
    console::Console,
    core::SimClock,
    metrics::{
        ThreadCounters,
        work::{self, WorkCounts},
    },
    mount::config::LocalConfig,
};
use chimera_net::dedicated::{GameServer, Parsed, ServerHooks, ServerMinigame, ServerPlugin, StartMap};
use chimera_sim::minigame::NextMatch;
use chimera_world::{harness::Sim, map::MapFile};
use combat_arms::game::TICK_INTERVAL;
use party_roster::server::PARTY;

/// Ultimate Gun Game's server (apps/ultimate_gun_game_server), its ladder
/// seeded (`UltimateGunGamePlugin::default`) so runs compare.
static SERVER: GameServer = GameServer {
    hooks: ServerHooks {
        minigame: Some(ServerMinigame {
            default: ultimate_gun_game::MINIGAME,
            first_map: ultimate_gun_game::first_map,
        }),
        launch: Some(launch),
        ..PARTY.hooks
    },
    ..PARTY
};

fn launch(app: &mut App) {
    party_roster::server::launch(app);
    app.add_plugins(ultimate_gun_game::UltimateGunGamePlugin::default());
}

const MAPS: [&str; 4] = [
    "cs_source:cs_office",
    "hl2dm:dm_lockdown",
    "combat_arms:warehouse",
    "cs_source:de_dust2",
];

/// Ticks of the new map counted after it came in (20 s: the lobby's
/// countdown, the match's start, the bots' first walks and fights).
const PLAY_TICKS: u64 = 400;

/// Frames over this are printed (instructions).
const SHOWN: u64 = 60_000_000;

/// The worst frame from the command to `PLAY_TICKS` into the new map
/// (instructions, this test's dev build): 100-161 M measured (2026-11-14,
/// three runs: de_dust2 101-109 M, the frame of the command, its new
/// ladder 100 M of it; dm_lockdown 141-152 M and cs_office 159-161 M, the
/// first tick on the map, the logic world's spawn 66-78 M of it;
/// warehouse 133-151 M, the command's frame with the old map's unload).
/// Before the change was spread
/// (`CHIMERA_SPAWN_AT_ONCE=1`) 1.1-5.3 G: the navigation mesh flooded in
/// the map's first tick (0.9-5.2 G) and the spawn spots in its second
/// (the first join, 1.1-5.3 G), after a frame of 280-460 M with the whole
/// spawn (1.5 G on de_dust2 with the spawn's parts computed in it too).
const WORST_FRAME: u64 = 250_000_000;

fn installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

/// The server from its command line on de_dust2, with 4 bots, as the binary
/// builds it (`dedicated::app`: `ServerPlugin`, then the launch hook).
fn server() -> Option<Sim> {
    let args = ["+map", "cs_source:de_dust2", "+MaxPlayer", "4", "+ip", "127.0.0.1", "-port", "0"];
    let Ok(Parsed::Run(options)) = SERVER.parse(args.map(String::from)) else { panic!("options") };
    let start = match StartMap::load(&SERVER, &options) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    let mut sim = Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(&SERVER, start, &options));
        launch(app);
        // Each step of a spawn in a frame of its own: its cost by itself,
        // whatever the machine's speed.
        app.insert_resource(chimera_world::map::prepare::StepAFrame);
    });
    // The same session on every run: a load finishes on the frame that asks.
    sim.repeatable();
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK_INTERVAL)));
    sim.app.update();
    Some(sim)
}

/// One map's change, frame by frame.
struct Change {
    /// Each frame's instructions and the simulation's tick after it.
    frames: Vec<(u64, u64)>,
    /// The costliest systems of each frame over `SHOWN`, by frame.
    tops: Vec<(usize, String)>,
    /// The frame the map's file was first the new one's.
    arrived: usize,
    /// Frames the simulation's tick stood (the spawn's, the simulation
    /// paused).
    stood: usize,
}

fn tick(sim: &Sim) -> u64 {
    sim.app.world().resource::<SimClock>().tick
}

fn change(sim: &mut Sim, counters: &mut ThreadCounters, seed: usize, map: &str) -> Change {
    let name = map.split_once(':').map_or(map, |m| m.1);
    sim.app.world_mut().resource_mut::<Console>().submit(format!("ugg_new {seed} 13 {map}"));
    let (mut frames, mut tops) = (Vec::new(), Vec::new());
    let (mut arrived, mut stood, mut until) = (None, 0, u64::MAX);
    let _ = WorkCounts::take();
    for frame in 0.. {
        assert!(frame < 5000, "{map} never came in");
        let before = tick(sim);
        let a = counters.read().hw.map_or(0, |h| h.instructions);
        sim.app.update();
        let used = counters.read().hw.map_or(0, |h| h.instructions).saturating_sub(a);
        let systems = WorkCounts::take();
        if used > SHOWN {
            let top: Vec<String> =
                systems.top(4).iter().map(|(name, ins, _)| format!("{name} {:.1} M", *ins as f64 / 1e6)).collect();
            tops.push((frame, top.join(", ")));
        }
        // The rotation held: the next map is the test's.
        sim.app.world_mut().remove_resource::<NextMatch>();
        let now = tick(sim);
        stood += (now == before) as usize;
        frames.push((used, now));
        if arrived.is_none() && sim.app.world().get_resource::<MapFile>().is_some_and(|f| f.name.ends_with(name)) {
            arrived = Some(frame);
            until = now + PLAY_TICKS;
        }
        if now >= until {
            break;
        }
    }
    Change { frames, tops, arrived: arrived.unwrap(), stood }
}

#[test]
fn a_map_change_costs_the_server_no_frame() {
    let chosen = chimera_core::env::var("CHIMERA_SWAP_MAPS").ok();
    let maps: Vec<&str> = match &chosen {
        Some(list) => list.split(',').collect(),
        None => MAPS.to_vec(),
    };
    if !installed() || !maps.iter().all(|id| party_roster::load_map(id, 0).is_ok()) {
        eprintln!("skipping: needs the CS:S, HL2:DM and Combat Arms installs");
        return;
    }
    // The compute pool is the process's: no other test's tasks beside it.
    if !chimera_world::harness::own_process("heavy::server_map_change::a_map_change_costs_the_server_no_frame") {
        return;
    }
    let Some(mut sim) = server() else { return };
    let mut counters = ThreadCounters::current();
    let counted = counters.read().hw.is_some();
    // Into the first map's match.
    sim.ticks(200);
    // Each system's instructions from here on (`metrics::work`: the
    // schedules on this thread, the compute pool's scopes too).
    let _ = sim.work(1);
    work::set_enabled(true);
    let mut worst_all = (0u64, "");
    for (i, map) in maps.iter().enumerate() {
        let c = change(&mut sim, &mut counters, 100 + i, map);
        let worst = c.frames.iter().map(|f| f.0).max().unwrap_or(0);
        let over = c.frames.iter().filter(|f| f.0 > 100_000_000).count();
        let sum: u64 = c.frames.iter().map(|f| f.0).sum();
        eprintln!(
            "{map:24} worst frame {:7.1} M; {} frames ({} with the simulation waiting), {} over 100 M, {:.0} M in all; the map's file in on frame {}",
            worst as f64 / 1e6,
            c.frames.len(),
            c.stood,
            over,
            sum as f64 / 1e6,
            c.arrived
        );
        for (k, top) in &c.tops {
            let (used, tick) = c.frames[*k];
            eprintln!("    frame {k:4} (tick {tick}): {:7.1} M: {top}", used as f64 / 1e6);
        }
        if worst > worst_all.0 {
            worst_all = (worst, map);
        }
        // The A/B brings the map in at once.
        if chimera_core::env::var("CHIMERA_SPAWN_AT_ONCE").is_err() {
            assert!(c.stood > 2, "{map}: the simulation waited {} frames for its spawn", c.stood);
        }
    }
    if !counted {
        eprintln!("skipping the limit: no hardware counters");
        return;
    }
    assert!(
        worst_all.0 < WORST_FRAME,
        "{}: {} instructions on the server's worst frame of its map change (limit {WORST_FRAME}). Is the map spawned \
         in one frame (`dedicated::finish_level_change`), the navigation mesh flooded in a tick \
         (`nav_gen::SliceFloods`), or the spawn spots built at the first respawn?",
        worst_all.1,
        worst_all.0
    );
}
