//! The map pool by player count on the real server (`chimera_sim::
//! minigame::map_fit`, docs/plans/active/map-pool-fitness.md): Ultimate
//! Gun Game's dedicated server on the installs, headless, with 4, 8 and
//! 12 characters present (bots: `MaxPlayer`), each map its draw picks
//! loaded as a rotation loads it (a host's `changelevel` with no map:
//! `minigame_net::host_next_level`), checked to:
//!
//! - be in the pool at that count (`MapPool::judge`), with the pool's
//!   size at 4, 8 and 12 printed;
//! - load (the server's map is it, the match running there);
//! - have spawn spots (`spawns::candidates`) or spawn points;
//! - have its download set covered: every file of the map's game that the
//!   server's load read since the change (`CHIMERA_VFS_LOG`, a file of
//!   this test's) is in the published store's `map/<name>`, `base` or
//!   `players` set of its game, or a piece's set (`weapon/`,
//!   `character/`, `parts/`: the ladder's); another game's files in its
//!   `base`, `players` or a piece's set. A file outside them would be
//!   fetched on demand mid-match by a player without the installs
//!   (`heavy::no_fetch_in_match` plays one client through a few maps).
//!
//! Sampled: `SAMPLE` draws per count (9 loads, a few minutes), each the
//! seeded rotation's own pick, so the likelier maps come up more;
//! `CHIMERA_POOL_MAPS=<n>` draws n per count, `=all` loads every map the
//! pool holds at any of the three counts by name instead (about 120
//! maps, 5 s to load at the median: about 15-20 minutes); `CHIMERA_POOL_SEED`
//! another seed. The recorder logs a file once per process, so a file a
//! map shares with one loaded before is checked only for the first.
//! Skipped with a note without the CS:S and Combat Arms installs or a
//! published store holding their manifests (`CHIMERA_GUARD_STORE`, else
//! this machine's server store). Needs a process of its own for the
//! recorder (`CHIMERA_VFS_LOG` is read when the first mount opens): run it
//! alone, or under nextest.
//!
//! `cargo test -p ultimate_gun_game --features dev --test it heavy::pool_maps -- --nocapture`

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, Instant},
};

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    mount::{
        assets::{Manifest, Store, server},
        config::LocalConfig,
    },
};
use chimera_net::{
    dedicated::{self, GameServer, ServerHooks, ServerMinigame, StartMap},
    minigame_net::{self, MapChange},
};
use chimera_sim::{
    minigame::{
        ActiveMinigame,
        map_fit::{self, FitSettings, MapPool},
        spawns,
    },
    mode::{Match, Phase},
};
use chimera_world::map::LoadedMapName;
use party_kit::server::PARTY;

/// The counts the pool is drawn for.
const COUNTS: [usize; 3] = [4, 8, 12];
/// Draws per count by default.
const SAMPLE: usize = 3;
/// The longest a map may take to load and start its match.
const LOAD_LIMIT: Duration = Duration::from_secs(240);

static SERVER: GameServer = GameServer {
    title: "Ultimate Gun Game",
    a_title: "an Ultimate Gun Game",
    usage: "",
    hooks: ServerHooks {
        minigame: Some(ServerMinigame {
            default: ultimate_gun_game::MINIGAME,
            first_map: ultimate_gun_game::first_map,
        }),
        launch: Some(|app| {
            party_kit::server::launch(app);
            app.add_plugins(ultimate_gun_game::UltimateGunGamePlugin::launch());
        }),
        ..PARTY.hooks
    },
    ..PARTY
};

fn installed(config: &LocalConfig, game: &str, marker: &str) -> bool {
    config.game_path(game).is_some_and(|p| chimera_core::mount::served::usable(game, &p, marker))
}

/// Each game's published manifest (the newest build the store holds).
fn manifests(store: &Store) -> BTreeMap<String, Manifest> {
    let mut out = BTreeMap::new();
    for game in ["cs_source", "combat_arms", "hl2dm"] {
        let Some(build) = store.builds(game).into_iter().max_by_key(|b| b.parse::<u64>().unwrap_or(0)) else {
            continue;
        };
        if let Ok(m) = Manifest::load(&store.manifest_path(game, &build)) {
            out.insert(game.to_string(), m);
        }
    }
    out
}

/// The server's world: one frame.
fn step(app: &mut App) {
    app.update();
    std::thread::sleep(Duration::from_secs_f64(1.0 / dedicated::FPS_MAX));
}

fn until(app: &mut App, limit: Duration, mut done: impl FnMut(&mut App) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done(app) {
            return true;
        }
        step(app);
    }
    false
}

/// The recorder's lines (game, path) from byte `from` of `log` on.
fn reads_since(log: &PathBuf, from: usize) -> Vec<(String, String)> {
    let text = std::fs::read(log).unwrap_or_default();
    String::from_utf8_lossy(&text[from.min(text.len())..])
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_string(), f.next()?.to_string()))
        })
        .collect()
}

/// Whether `manifest`'s sets cover `path` of its game for map `name`'s
/// load (`own`: the map's own game).
fn covered(manifest: &Manifest, path: &str, own: Option<&str>) -> bool {
    let lower = path.to_ascii_lowercase();
    manifest.sets.iter().any(|(set, files)| {
        let wanted = set == "base"
            || set == "players"
            || own.is_some_and(|n| set.eq_ignore_ascii_case(&format!("map/{n}")))
            || ["weapon/", "character/", "parts/"].iter().any(|p| set.starts_with(p));
        wanted && (files.contains(path) || files.iter().any(|f| f.to_ascii_lowercase() == lower))
    })
}

/// What a map load left wrong (empty: none).
fn check_map(app: &mut App, map: &str, players: usize, manifests: &BTreeMap<String, Manifest>, log: &PathBuf, from: usize) -> Vec<String> {
    let mut problems = Vec::new();
    let w = app.world_mut();
    let def = w.resource::<ActiveMinigame>().def;
    let settings = FitSettings::of(def).unwrap().expect("the pool goes by the count");
    let pool = w.get_resource::<MapPool>().cloned().unwrap_or_default();
    let judged = pool.judge(map, players, &settings);
    if judged.weight() <= 0.0 {
        problems.push(format!("drawn but out at {players}: {judged:?}"));
    }
    let spots = spawns::candidates(w).len();
    let points = w.query::<&chimera_core::core::SpawnPoint>().iter(w).count();
    if spots == 0 && points == 0 {
        problems.push("no spawn spots or points".into());
    }
    let (game, name) = map.split_once(':').unwrap_or(("", map));
    let reads = reads_since(log, from);
    let mut missing: Vec<String> = Vec::new();
    for (g, path) in &reads {
        let Some(m) = manifests.get(g) else { continue };
        if !m.files.contains_key(path) {
            // Not a published file of the game (a loose or derived one).
            continue;
        }
        if !covered(m, path, (g == game).then_some(name)) {
            missing.push(format!("{g}: {path}"));
        }
    }
    if let Some(m) = manifests.get(game)
        && !m.sets.keys().any(|s| s.eq_ignore_ascii_case(&format!("map/{name}")))
    {
        problems.push(format!("no published set map/{name}"));
    }
    if !missing.is_empty() {
        problems.push(format!(
            "{} files read outside its sets (fetched mid-match by a player without the install): {}",
            missing.len(),
            missing.iter().take(12).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    eprintln!(
        "  {map} at {players}: {judged:?}; {spots} spots, {points} points; {} files read, {} outside its sets",
        reads.len(),
        missing.len()
    );
    problems
}

/// The server on `map` (loaded, its match running).
fn on(app: &mut App, map: &str) -> bool {
    until(app, LOAD_LIMIT, |app| {
        let w = app.world();
        !w.contains_resource::<MapChange>()
            && w.get_resource::<LoadedMapName>().is_some_and(|n| n.0 == map)
            && w.get_resource::<Match>().is_some_and(|m| !matches!(m.phase, Phase::MatchOver))
    })
}

/// `map` loading (asked for): its problems once it plays.
fn load(app: &mut App, map: &str, n: usize, manifests: &BTreeMap<String, Manifest>, log: &PathBuf) -> Vec<String> {
    let from = std::fs::metadata(log).map_or(0, |m| m.len() as usize);
    if !on(app, map) {
        let at = app.world().get_resource::<LoadedMapName>().map(|n| n.0.clone());
        return vec![format!("{map}: not loaded and playing within {LOAD_LIMIT:?} (the server is on {at:?})")];
    }
    check_map(app, map, n, manifests, log, from).into_iter().map(|p| format!("{map} at {n}: {p}")).collect()
}

#[test]
fn every_pool_map_loads_with_spots_and_its_set_covers_it() {
    if !cfg!(unix) {
        eprintln!("skipping: the store's paths are this machine's (Unix only)");
        return;
    }
    let Ok(config) = LocalConfig::load() else {
        eprintln!("skipping: no local config");
        return;
    };
    if !installed(&config, "cs_source", "cstrike") || !installed(&config, "combat_arms", "Game") {
        eprintln!("skipping: needs the CS:S and Combat Arms installs");
        return;
    }
    let store = std::env::var_os("CHIMERA_GUARD_STORE").map(Store::new).or_else(server::default_store);
    let manifests = store.as_ref().map(manifests).unwrap_or_default();
    if !manifests.contains_key("cs_source") || !manifests.contains_key("combat_arms") {
        eprintln!("skipping: no published asset store with CS:S's and Combat Arms' manifests (CHIMERA_GUARD_STORE)");
        return;
    }
    let all = std::env::var("CHIMERA_POOL_MAPS").is_ok_and(|v| v.trim() == "all");
    let per_count = std::env::var("CHIMERA_POOL_MAPS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(SAMPLE);
    let seed: u64 = std::env::var("CHIMERA_POOL_SEED").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(20261113);

    // The recorder, before any mount opens.
    let log = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("pool_maps_{}.log", std::process::id()));
    let _ = std::fs::remove_file(&log);
    // SAFETY: this test's process (run alone or under nextest) reads its
    // environment on this thread only until the server starts.
    unsafe { std::env::set_var("CHIMERA_VFS_LOG", &log) };

    let args: Vec<String> = [
        "-port",
        "0",
        "+maxplayers",
        "16",
        "+MaxPlayer",
        "4",
        "+map",
        "cs_source:de_dust2",
        "+mp_lobby_countdown",
        "1",
        "+mp_lobby_max_wait",
        "5",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let options = match SERVER.parse(args).unwrap() {
        dedicated::Parsed::Run(o) => o,
        dedicated::Parsed::Help => unreachable!(),
    };
    let start = StartMap::load(&SERVER, &options).expect("the first map loads");
    let mut app = dedicated::app(&SERVER, start, &options);
    app.insert_resource(chimera_sim::minigame::rotation::MapHistory::seeded(seed));
    chimera_world::harness::single_threaded_schedules(&mut app);
    app.finish();
    app.cleanup();
    step(&mut app);
    dedicated::listen(app.world_mut()).expect("the server listens");
    step(&mut app);
    assert!(
        std::fs::metadata(&log).is_ok_and(|m| m.len() > 0),
        "the recorder logged nothing: a mount opened before CHIMERA_VFS_LOG was set (run this test alone or under \
         nextest)"
    );

    // The pool at each count.
    let def = app.world().resource::<ActiveMinigame>().def;
    let settings = FitSettings::of(def).unwrap().expect("the pool goes by the count");
    let pool = app.world().get_resource::<MapPool>().cloned().unwrap_or_default();
    let maps: Vec<String> = def.maps.iter().map(|m| chimera_net::net::normalize_map(m)).collect();
    let mut union: BTreeSet<String> = BTreeSet::new();
    for n in COUNTS {
        let ins: Vec<&String> = maps.iter().filter(|m| pool.judge(m, n, &settings).weight() > 0.0).collect();
        eprintln!("the pool at {n} players: {} of {} maps", ins.len(), maps.len());
        union.extend(ins.into_iter().cloned());
    }
    let mut problems: Vec<String> = Vec::new();
    if all {
        for (i, map) in union.iter().enumerate() {
            eprintln!("[{}/{}] {map}", i + 1, union.len());
            if app.world().get_resource::<LoadedMapName>().is_some_and(|n| &n.0 == map) {
                continue;
            }
            let said = minigame_net::host_changelevel(app.world_mut(), map);
            if let Some(Err(e)) = said {
                problems.push(format!("{map}: changelevel: {e}"));
                continue;
            }
            let n = map_fit::present(app.world_mut()).max(1);
            problems.extend(load(&mut app, map, n, &manifests, &log));
        }
    } else {
        for n in COUNTS {
            app.world_mut().resource_mut::<Console>().submit(&format!("MaxPlayer {n}"));
            let ok = until(&mut app, Duration::from_secs(60), |app| map_fit::present(app.world_mut()) == n);
            assert!(ok, "{n} characters present (MaxPlayer {n}): {}", map_fit::present(app.world_mut()));
            for _ in 0..per_count {
                let said = minigame_net::host_next_level(app.world_mut());
                let Some(Ok(_)) = said else {
                    problems.push(format!("changelevel at {n}: {said:?}"));
                    continue;
                };
                let target = app
                    .world()
                    .get_resource::<MapChange>()
                    .map(|c| c.map.clone())
                    .or_else(|| app.world().get_resource::<LoadedMapName>().map(|m| m.0.clone()))
                    .unwrap_or_default();
                eprintln!("{n} players: drew {target}");
                problems.extend(load(&mut app, &target, n, &manifests, &log));
            }
        }
    }
    drop(app);
    let _ = std::fs::remove_file(&log);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
