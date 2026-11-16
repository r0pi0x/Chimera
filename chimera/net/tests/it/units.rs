//! Units of game files on demand (`net::units`, the sandbox's progressive
//! downloads): on the engine's base and the greybox, a stand-in game whose
//! files are our own synthetic bytes, served by a real asset service on
//! 127.0.0.1 (port 0). The server's own store fetches them (`ServerFiles`,
//! as a listen or dedicated server does for a sandbox's pick):
//!
//! - a gun's unit goes Absent (its size from the manifest, before
//!   fetching) -> Fetching -> Ready with the byte counts right, and its
//!   files read from a mount afterwards;
//! - what it needs comes with it (its game's shared sets, a `UnitNeeds`
//!   hook's);
//! - a unit that fails is Failed, and a retry brings it;
//! - the queue: picks first (the newest first), then pins, then the rest;
//! - the no-fetch-in-match guard refuses a unit in a normal match, allows
//!   it in the sandbox (`HoldRungs`) and a `deferred` one anywhere;
//! - the gun files seam (`gun_files::gun_ready`) waits for the unit.

use std::{collections::BTreeSet, fs, path::PathBuf, time::Duration};

use bevy::prelude::*;
use chimera_assets::{
    AssetSettings, CachedFiles, FileRecord, Manifest, RemoteConfig, Store,
    accounts::Accounts,
    server::{self, Running, Service},
};
use chimera_core::slots::Loadout;
use chimera_net::{
    harness::NetSim,
    minigame_net::MinigameNetPlugin,
    net::{
        memory::LinkConditions,
        server_assets::{ServerFiles, Via},
        units::{self, Priority, UnitBudget, UnitChanged, UnitDownloads, UnitNeeds, UnitProgress, UnitState},
    },
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{self, MinigameAppExt, MinigamePlugin, gun_files, rung_command::HoldRungs},
    movement::placeholder,
    testkit,
};
use chimera_core::mount::FileSource;
use chimera_world::greybox::GreyboxMapPlugin;

const GAME: &str = "unit_game";

const DEF: &str = r#"
id = "kit:units"
name = "Kit units"
players = { min = 1, max = 4 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "600s"
rotate = false

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
"#;

/// Bytes zstd can't shrink (xorshift from `seed`).
fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ seed;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// The service with the stand-in game: `base`, `players`, `common`, guns
/// `weapon/<a..e>`, a class body `players/heavy`, `voice/grunt`, and
/// `weapon/broken`, whose record the service lacks until `mend`.
struct Fixture {
    dir: PathBuf,
    store: Store,
    manifest: Manifest,
    broken: Vec<u8>,
    _running: Running,
    url: String,
    token: String,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("chimera_net_units_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(dir.join("server"));
        let mut m = Manifest::new(GAME, "1", None);
        let mut seed = 0;
        let mut put = |path: &str, len: usize, set: &str| {
            seed += 1;
            let bytes = noise(seed, len);
            let p = store.put(&bytes).unwrap();
            m.files.insert(path.into(), FileRecord { id: p.id, size: bytes.len() as u64, stored: p.stored });
            m.sets.entry(set.to_string()).or_default().insert(path.into());
            bytes
        };
        put("base/data.txt", 3_000, "base");
        put("models/player.mdl", 2_000, "players");
        put("sound/common.wav", 1_000, "common");
        put("models/heavy.mdl", 4_000, "players/heavy");
        put("sound/grunt.wav", 1_500, "voice/grunt");
        for g in ["a", "b", "c", "e"] {
            put(&format!("models/{g}.mdl"), 5_000, &format!("weapon/{g}"));
        }
        // The gun the tests watch fetch: 4 records of 40 kB.
        for i in 0..4 {
            put(&format!("models/gun/part{i}.mdl"), 40_000, "weapon/gun");
        }
        // A bigger one, to hold the queue while it fetches slowly.
        for i in 0..6 {
            put(&format!("models/d/part{i}.mdl"), 40_000, "weapon/d");
        }
        let broken = put("models/broken.mdl", 2_000, "weapon/broken");
        m.save(&store.manifest_path(GAME, "1")).unwrap();
        let id = m.files["models/broken.mdl"].id.clone();
        fs::remove_file(store.record_path(&id)).unwrap();
        let running = server::serve(Service::new(store.clone()).unwrap(), "127.0.0.1:0", 4).unwrap();
        let path = Accounts::path(&store.root);
        let mut accounts = Accounts::load(&path).unwrap();
        let (_, token) = accounts.add("tester", None, "test").unwrap();
        accounts.save(&path).unwrap();
        let url = format!("http://{}", running.addr);
        Self { dir, store, manifest: m, broken, _running: running, url, token }
    }

    /// The service gets the broken gun's record.
    fn mend(&self) {
        self.store.put(&self.broken).unwrap();
    }

    /// The game server's own files: its store, the service, the game
    /// read from the store alone.
    fn server_files(&self) -> ServerFiles {
        ServerFiles {
            settings: AssetSettings {
                store: Some(Store::new(self.dir.join("game_server"))),
                use_install: false,
                service_first: false,
                builds: Default::default(),
                remote: Some(RemoteConfig { url: self.url.clone(), token: self.token.clone(), key: None, fingerprint: None }),
                lock: None,
            },
            served: BTreeSet::from([GAME.to_string()]),
            via: Via::Config,
        }
    }

    /// Stored bytes of `sets`' records, each once.
    fn stored(&self, sets: &[&str]) -> u64 {
        let ids: BTreeSet<(&String, u64)> = sets
            .iter()
            .flat_map(|s| self.manifest.sets[*s].iter())
            .map(|p| (&self.manifest.files[p].id, self.manifest.files[p].stored))
            .collect();
        ids.iter().map(|(_, s)| s).sum()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// The class body a gun of the stand-in game needs (as TF2's classes do).
fn heavy(_: &World, game: &str, set: &str) -> Vec<(String, String)> {
    if game == GAME && set == "weapon/gun" { vec![(GAME.into(), "players/heavy".into())] } else { Vec::new() }
}

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin))
        .insert_resource(Loadout { movement: placeholder::ID });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_units.toml", DEF)]).expect("the definition reads");
    app.world_mut().resource_mut::<UnitNeeds>().0.push(heavy);
}

/// A server with no clients reading the stand-in game from its store;
/// `sandbox`: the minigame plays under the sandbox's rule; `kb`: the units'
/// budget, KB/s.
fn sim(svc: &Fixture, sandbox: Option<bool>, kb: i64) -> NetSim {
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 3, 0, setup);
    let w = sim.server.app.world_mut();
    w.insert_resource(svc.server_files());
    w.insert_resource(UnitBudget { kb_per_sec: kb });
    if let Some(sandbox) = sandbox {
        if sandbox {
            w.insert_resource(HoldRungs("kit:units"));
        }
        minigame::start(w, "kit:units").expect("it starts");
    }
    sim.step();
    sim
}

fn key(set: &str) -> (String, String) {
    (GAME.to_string(), set.to_string())
}

fn state(sim: &NetSim, set: &str) -> Option<UnitState> {
    sim.server.app.world().resource::<UnitProgress>().state(GAME, set).cloned()
}

/// Steps in real time (the downloads run on threads) until `done`.
fn until(sim: &mut NetSim, secs: f64, mut done: impl FnMut(&mut NetSim) -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        if done(sim) {
            return true;
        }
        sim.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    done(sim)
}

/// Every `UnitChanged` of `set` so far (read with `cursor`).
fn changes(sim: &NetSim, cursor: &mut bevy::ecs::message::MessageCursor<UnitChanged>, set: &str, into: &mut Vec<UnitState>) {
    let m = sim.server.app.world().resource::<Messages<UnitChanged>>();
    for c in cursor.read(m) {
        if c.game == GAME && c.set == set && into.last() != Some(&c.status.state) {
            into.push(c.status.state.clone());
        }
    }
}

#[test]
fn a_guns_unit_goes_absent_fetching_ready_with_its_sizes_and_reads_after() {
    let svc = Fixture::new("gun");
    // 100 KB/s: the gun's 160 kB take over a second.
    let mut sim = sim(&svc, Some(true), 100);
    let closure = ["weapon/gun", "base", "players", "common", "players/heavy"];
    let size = svc.stored(&closure);
    let gun = units::gun_unit(&format!("{GAME}:gun")).unwrap();
    assert_eq!(gun, key("weapon/gun"));
    units::describe_units(sim.server.app.world_mut(), &[gun.clone()]);
    assert!(
        until(&mut sim, 10.0, |s| state(s, "weapon/gun").is_some()),
        "described"
    );
    assert_eq!(state(&sim, "weapon/gun"), Some(UnitState::Absent { size }), "its size before fetching");
    assert!(!units::units_ready(sim.server.app.world(), &[gun.clone()]));

    let mut cursor = bevy::ecs::message::MessageCursor::<UnitChanged>::default();
    let mut seen = Vec::new();
    changes(&sim, &mut cursor, "weapon/gun", &mut seen);
    units::request_unit(sim.server.app.world_mut(), GAME, "weapon/gun").expect("the sandbox allows it");
    let mut mid = None;
    assert!(
        until(&mut sim, 30.0, |s| {
            changes(s, &mut cursor, "weapon/gun", &mut seen);
            let st = s.server.app.world().resource::<UnitProgress>().status(GAME, "weapon/gun").cloned();
            if let Some(st) = st.filter(|st| st.state == UnitState::Fetching && st.bytes_done > 0 && st.bytes_done < st.bytes_total) {
                mid.get_or_insert(st);
            }
            s.server.app.world().resource::<UnitProgress>().ready(GAME, "weapon/gun")
        }),
        "ready: {:?}",
        state(&sim, "weapon/gun")
    );
    changes(&sim, &mut cursor, "weapon/gun", &mut seen);
    let phases: Vec<&str> = seen
        .iter()
        .filter(|s| !matches!(s, UnitState::Queued { .. }))
        .map(|s| match s {
            UnitState::Absent { .. } => "absent",
            UnitState::Queued { .. } => "queued",
            UnitState::Fetching => "fetching",
            UnitState::Ready => "ready",
            UnitState::Failed { .. } => "failed",
        })
        .collect();
    assert_eq!(phases, ["absent", "fetching", "ready"], "{seen:?}");
    let mid = mid.expect("seen part way");
    assert_eq!(mid.bytes_total, size, "the total is the manifest's size");
    let done = sim.server.app.world().resource::<UnitProgress>().status(GAME, "weapon/gun").cloned().unwrap();
    assert_eq!((done.bytes_done, done.bytes_total), (size, size));
    assert_eq!((done.files_done, done.files_total), (8, 8), "4 gun parts, base, players, common, the body");
    assert!(units::units_ready(sim.server.app.world(), &[gun]));

    // Its files read from a mount of the store (no service behind it).
    let store = svc.server_files().settings.store.unwrap();
    let mount = CachedFiles::new(store, &svc.manifest);
    for path in &svc.manifest.sets["weapon/gun"] {
        let got = mount.read(path).expect("listed").expect("here");
        assert_eq!(got.len() as u64, svc.manifest.files[path].size, "{path}");
    }
}

#[test]
fn a_units_needs_come_with_it() {
    let svc = Fixture::new("needs");
    let mut sim = sim(&svc, Some(true), 0);
    let all = units::closure(sim.server.app.world(), GAME, "weapon/gun");
    assert_eq!(all, ["weapon/gun", "base", "players", "common", "players/heavy"].map(key));
    // The rung switch's seam asks for the gun and waits for it.
    let gun = format!("{GAME}:gun");
    assert!(!gun_files::gun_ready(sim.server.app.world_mut(), &gun), "not here yet: asked for");
    assert!(until(&mut sim, 30.0, |s| gun_files::gun_ready(s.server.app.world_mut(), &gun)), "{:?}", state(&sim, "weapon/gun"));
    assert!(units::units_ready(sim.server.app.world(), &all));
    for set in ["base", "players", "common", "players/heavy"] {
        assert_eq!(state(&sim, set), Some(UnitState::Ready), "{set}");
    }
    let store = svc.server_files().settings.store.unwrap();
    assert!(svc.manifest.sets["players/heavy"].iter().all(|p| store.has(&svc.manifest.files[p].id)));
    // Another gun needs no body.
    assert_eq!(units::closure(sim.server.app.world(), GAME, "weapon/a").len(), 4);
}

#[test]
fn a_unit_that_fails_is_failed_and_a_retry_brings_it() {
    let svc = Fixture::new("retry");
    let mut sim = sim(&svc, Some(true), 0);
    units::request_unit(sim.server.app.world_mut(), GAME, "weapon/broken").unwrap();
    sim.server.app.world_mut().resource_mut::<UnitDownloads>().downloads.retry_delays.clear();
    assert!(
        until(&mut sim, 30.0, |s| matches!(state(s, "weapon/broken"), Some(UnitState::Failed { .. }))),
        "failed: {:?}",
        state(&sim, "weapon/broken")
    );
    // A failed unit doesn't hold a switch (its files come on demand as the
    // gun loads), and isn't asked for again by itself.
    assert!(gun_files::gun_ready(sim.server.app.world_mut(), &format!("{GAME}:broken")));
    sim.ticks(3);
    assert!(matches!(state(&sim, "weapon/broken"), Some(UnitState::Failed { .. })));
    svc.mend();
    units::retry_unit(sim.server.app.world_mut(), GAME, "weapon/broken").unwrap();
    assert!(
        until(&mut sim, 30.0, |s| state(s, "weapon/broken") == Some(UnitState::Ready)),
        "ready after the retry: {:?}",
        state(&sim, "weapon/broken")
    );
    assert!(gun_files::gun_ready(sim.server.app.world_mut(), &format!("{GAME}:broken")));
}

#[test]
fn the_queue_goes_picks_newest_first_then_pins_then_the_rest() {
    let svc = Fixture::new("queue");
    // 50 KB/s: `weapon/d` (240 kB) holds the queue for seconds.
    let mut sim = sim(&svc, Some(true), 50);
    let w = sim.server.app.world_mut();
    units::request_unit_at(w, GAME, "weapon/d", Priority::Pick).unwrap();
    sim.step();
    assert!(until(&mut sim, 10.0, |s| state(s, "weapon/d") == Some(UnitState::Fetching)), "{:?}", state(&sim, "weapon/d"));
    let w = sim.server.app.world_mut();
    units::request_unit_at(w, GAME, "weapon/a", Priority::Later).unwrap();
    units::request_unit_at(w, GAME, "weapon/b", Priority::Pin).unwrap();
    units::request_unit_at(w, GAME, "weapon/c", Priority::Pick).unwrap();
    units::request_unit_at(w, GAME, "weapon/e", Priority::Pick).unwrap();
    sim.step();
    let queued = |s: &NetSim| s.server.app.world().resource::<UnitDownloads>().downloads.queued();
    assert_eq!(queued(&sim), ["weapon/e", "weapon/c", "weapon/b", "weapon/a"].map(key));
    for (i, set) in ["weapon/e", "weapon/c", "weapon/b", "weapon/a"].iter().enumerate() {
        assert_eq!(state(&sim, set), Some(UnitState::Queued { position: i + 1 }), "{set}");
    }
    // A pin picked moves to the front.
    units::request_unit_at(sim.server.app.world_mut(), GAME, "weapon/b", Priority::Pick).unwrap();
    sim.step();
    assert_eq!(queued(&sim), ["weapon/b", "weapon/e", "weapon/c", "weapon/a"].map(key));
    // They come in that order.
    sim.server.app.world_mut().resource_mut::<UnitBudget>().kb_per_sec = 0;
    let mut order: Vec<String> = Vec::new();
    assert!(until(&mut sim, 30.0, |s| {
        let p = s.server.app.world().resource::<UnitProgress>();
        for set in ["weapon/d", "weapon/b", "weapon/e", "weapon/c", "weapon/a"] {
            if p.ready(GAME, set) && !order.iter().any(|o| o == set) {
                order.push(set.to_string());
            }
        }
        order.len() == 5
    }));
    assert_eq!(order, ["weapon/d", "weapon/b", "weapon/e", "weapon/c", "weapon/a"]);
}

#[test]
fn in_a_normal_match_the_guard_refuses_an_on_demand_unit() {
    let svc = Fixture::new("guard");
    let mut sim = sim(&svc, Some(false), 0);
    let e = units::request_unit(sim.server.app.world_mut(), GAME, "weapon/a").expect_err("refused in a match");
    assert!(e.contains("during a match"), "{e}");
    sim.ticks(5);
    assert_eq!(state(&sim, "weapon/a"), None, "nothing asked for");
    assert!(!sim.server.app.world().contains_resource::<UnitDownloads>(), "nothing fetching");
    // A deferred unit (a voice) may come in a match.
    units::request_unit(sim.server.app.world_mut(), GAME, "voice/grunt").expect("deferred");
    assert!(until(&mut sim, 30.0, |s| state(s, "voice/grunt") == Some(UnitState::Ready)));
    // The sandbox allows it.
    let svc2 = Fixture::new("guard_sandbox");
    let mut sandbox = self::sim(&svc2, Some(true), 0);
    units::request_unit(sandbox.server.app.world_mut(), GAME, "weapon/a").expect("allowed in the sandbox");
}
