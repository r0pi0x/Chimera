//! The dedicated server's memory over a session of map changes (backlog,
//! "UGG playtest rehearsal"; docs/performance.md, "The dedicated server's
//! memory"): Ultimate Gun Game's server as its binary composes it
//! (`ServerPlugin` over the party's `GameServer` and its launch hook, bots by
//! `+MaxPlayer`), a new ladder on another map each time (`ugg_new <seed>
//! <rungs> <map>`: the rotation's path, `change_match`, `changelevel`,
//! `swap_map`), three laps over maps of CS:S and Combat Arms, the rotation
//! held so the maps are the test's. Each map plays until the server has
//! given its free pages back (`metrics::memory::release_after_change`, 20 s
//! of the server's time after the map came in), then counts: the
//! allocator's bytes in use (`metrics::memory::malloc`), what the process
//! holds of its heap and anonymous mappings, resident or swapped
//! (`Regions`), and the world's entities. The session plays out the same on
//! every run (`Sim::repeatable`), so the bytes are the same within a few MB;
//! it has its process to itself (`harness::own_process`); about 90 s.
//!
//! What it holds: a map has in use on its third visit what it had on its
//! second (`LAP_GROWTH`) and about the same entities; the server has less
//! than `IN_USE_LIMIT` in use (Combat Arms' models' textures were decoded on
//! the server, which draws nothing: 3.3 GB in use), and holds little more
//! than that (`HELD_BEYOND_IN_USE`: glibc kept 1.3-1.8 GB free in its
//! arenas, more each map, before the server gave them back). Prints a row per map (`--nocapture`). Passes
//! with a note without the installs, or off Linux with glibc (no counts).

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::{console::Console, metrics::memory, mount::config::LocalConfig};
use chimera_net::dedicated::{GameServer, Parsed, ServerHooks, ServerMinigame, ServerPlugin, StartMap};
use chimera_sim::minigame::NextMatch;
use chimera_world::{harness::Sim, map::MapFile};
use combat_arms::game::TICK_INTERVAL;
use party_kit::server::PARTY;

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
    party_kit::server::launch(app);
    app.add_plugins(ultimate_gun_game::UltimateGunGamePlugin::default());
}

const MAPS: [&str; 4] = [
    "cs_source:cs_office",
    "combat_arms:warehouse",
    "cs_source:de_inferno",
    "cs_source:de_dust2",
];

const LAPS: usize = 3;

/// The server's bytes in use at most, on any map. Measured 2026-11-11: 3.20-3.40
/// GB before the server kept Combat Arms' textures' sizes only, 2.02-2.24 GB
/// after.
const IN_USE_LIMIT: u64 = 2560 << 20;

/// What a map may have in use more on its third visit than on its second:
/// measured 3-11 MB (the archetypes' tables, which Bevy keeps); over seven
/// laps under 1 MB a map from the third on.
const LAP_GROWTH: u64 = 32 << 20;

/// What the process may hold beyond its bytes in use once the server gave its
/// free pages back (glibc's fragments, the allocator's own): measured
/// 1.3-1.8 GB before the server gave them back, 2-160 MB after (it moves
/// with the box's load: whichever thread's arena a block came from).
const HELD_BEYOND_IN_USE: u64 = 500 << 20;

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
    });
    // The same session on every run: a load finishes on the frame that asks.
    sim.repeatable();
    sim.app.init_resource::<memory::Releases>();
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK_INTERVAL)));
    sim.app.update();
    Some(sim)
}

/// Frames until map `map` plays (with `repeatable`, on the frame
/// `changelevel` ran).
fn until_swapped(sim: &mut Sim, map: &str) {
    let name = map.split_once(':').map_or(map, |m| m.1);
    for _ in 0..100 {
        if sim.app.world().get_resource::<MapFile>().is_some_and(|f| f.name.ends_with(name)) {
            return;
        }
        sim.app.update();
    }
    let out = &sim.app.world().resource::<Console>().output;
    let last: Vec<&str> = out.iter().rev().take(20).map(|l| l.text.as_str()).collect();
    panic!("{map} never came in; the console's last lines (newest first): {last:#?}");
}

/// Play until the server has given its free pages back after this map came
/// in (`release_after_change`: 20 s of its time, then a thread), the
/// rotation held so the next map is the test's.
fn play_until_released(sim: &mut Sim) {
    let releases = sim.app.world().resource::<memory::Releases>().clone();
    let before = releases.count();
    let ticks = ((memory::RELEASE_AFTER_SECONDS as f64 + 2.0) * sim.tick_hz()).round() as u64;
    for _ in 0..ticks {
        sim.ticks(1);
        sim.app.world_mut().remove_resource::<NextMatch>();
    }
    // The release's thread: waited for, not timed.
    let started = std::time::Instant::now();
    while releases.count() == before {
        assert!(started.elapsed().as_secs() < 120, "the server never gave its free pages back after a map change");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[derive(Clone, Copy, Debug)]
struct Row {
    lap: usize,
    map: &'static str,
    in_use: u64,
    held: u64,
    entities: usize,
}

#[test]
fn map_changes_keep_no_memory() {
    if memory::malloc().is_none() || memory::Regions::read().is_none() {
        eprintln!("skipping: counts the allocator's bytes on Linux with glibc only");
        return;
    }
    if !installed() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    // The allocator's counts are the process's: no other test beside it.
    if !chimera_world::harness::own_process("heavy::server_map_memory::map_changes_keep_no_memory") {
        return;
    }
    let Some(mut sim) = server() else { return };
    let mut rows: Vec<Row> = Vec::new();
    for lap in 0..LAPS {
        for (i, map) in MAPS.iter().enumerate() {
            sim.app.world_mut().resource_mut::<Console>().submit(format!("ugg_new {} 13 {map}", 100 + i));
            sim.app.update();
            until_swapped(&mut sim, map);
            play_until_released(&mut sim);
            let in_use = memory::malloc().unwrap().in_use;
            let regions = memory::Regions::read().unwrap();
            let held = regions.heap.total() + regions.anon.total();
            let entities = sim.app.world().entities().count_spawned() as usize;
            eprintln!(
                "lap {lap} {map:24} in use {:>5} MB, heap and anonymous {:>5} MB (resident or swapped), {entities:5} entities",
                in_use >> 20,
                held >> 20
            );
            rows.push(Row { lap, map, in_use, held, entities });
        }
    }
    for r in &rows {
        assert!(
            r.in_use < IN_USE_LIMIT,
            "{} MB in use on {} (lap {}): more than {} MB. Does the server keep textures' texels (`world::SizesOnly`)?",
            r.in_use >> 20,
            r.map,
            r.lap,
            IN_USE_LIMIT >> 20
        );
        assert!(
            r.held < r.in_use + HELD_BEYOND_IN_USE,
            "{} MB held, {} MB in use on {} (lap {}): the free pages weren't given back (`release_after_change`)",
            r.held >> 20,
            r.in_use >> 20,
            r.map,
            r.lap
        );
    }
    for r in rows.iter().filter(|r| r.lap == LAPS - 1) {
        let was = rows.iter().find(|w| w.lap == r.lap - 1 && w.map == r.map).unwrap();
        assert!(
            r.in_use < was.in_use + LAP_GROWTH,
            "{}: {} MB in use on lap {}, {} MB on lap {}: a map change keeps something",
            r.map,
            was.in_use >> 20,
            was.lap,
            r.in_use >> 20,
            r.lap
        );
        // The bots' fights differ a little lap to lap (a ragdoll, a grenade).
        assert!(
            (r.entities as f64) < was.entities as f64 * 1.1 + 50.0,
            "{}: {} entities on lap {}, {} on lap {}",
            r.map,
            was.entities,
            was.lap,
            r.entities,
            r.lap
        );
    }
}
