//! A match's game files from the host's asset service
//! (docs/plans/active/asset-service.md, "Playtest delivery"; `net::assets`):
//! on the engine's base and the greybox, a stand-in game (`fixture_game`)
//! whose files are our own fixture bytes, served by a real asset service on
//! 127.0.0.1 behind its TLS front with a self-signed certificate. The
//! server names the service (`AssetHost`); a client that reads the game from
//! the store alone fetches the offered sets (and not others) with progress,
//! then joins and spawns; one with the install fetches nothing; a revoked
//! token fails the join with the reason; a client with no files of the game
//! binds its token to the offered service and asks to start again, or, with
//! no token, is told how to enter one.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::prelude::*;
use chimera_assets::{
    AssetSettings, FileRecord, Manifest, RemoteConfig, Store,
    access::Access,
    accounts::Accounts,
    server::{self, Running, Service},
    tls::{self, Front, Identity},
};
use chimera_core::slots::Loadout;
use chimera_net::{
    harness::NetSim,
    net::{
        NetHooks,
        asset_startup::{self, AppGame, StartupFetch, Status},
        assets::{self, AssetHooks, AssetHost, AssetNames, AssetRelaunch, AssetSession, FILES, PrefetchRate},
        client::{JoinFailure, JoinProgress, JoinStage},
        manifest::{GameMount, LocalMounts, MatchManifest, MountReport},
        memory::LinkConditions,
    },
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

const GAME: &str = "fixture_game";

/// The match needs the stand-in game (as a map's game is needed).
fn needs_game(_: &mut World, m: &mut MatchManifest) {
    m.need(GAME, true);
}

fn setup(host: AssetHost) -> impl Fn(&mut App) + Send + Sync + 'static {
    move |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout {
            movement: placeholder::ID,
        });
        app.world_mut().resource_mut::<NetHooks>().manifest.push(needs_game);
        app.insert_resource(host.clone());
    }
}

/// The match's pool: a later map of the stand-in game (prefetched).
fn pool(_: &World) -> AssetNames {
    AssetNames {
        pool: vec![format!("{GAME}:later")],
        ..default()
    }
}

/// `setup` with a pool (the offer's `later`).
fn setup_pool(host: AssetHost) -> impl Fn(&mut App) + Send + Sync + 'static {
    let base = setup(host);
    move |app: &mut App| {
        base(app);
        app.world_mut().resource_mut::<AssetHooks>().0.push(pool);
    }
}

/// The asset service with the stand-in game: `base` and `players` (what a
/// match offers), `extra` (what it doesn't), `files` records in `base`.
struct Fixture {
    dir: PathBuf,
    store: Store,
    manifest: Manifest,
    _running: Running,
    front: Front,
    fingerprint: String,
}

impl Fixture {
    fn new(name: &str, files: usize, latency: Duration) -> Self {
        let dir = std::env::temp_dir().join(format!("chimera_net_assets_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(dir.join("server"));
        let mut m = Manifest::new(GAME, "7", None);
        let mut put = |path: String, bytes: Vec<u8>, set: &str| {
            let p = store.put(&bytes).unwrap();
            m.files.insert(
                path.clone(),
                FileRecord {
                    id: p.id,
                    size: bytes.len() as u64,
                    stored: p.stored,
                },
            );
            m.sets.entry(set.to_string()).or_default().insert(path);
        };
        for i in 0..files {
            // Distinct, poorly compressible bytes.
            let bytes: Vec<u8> = (0..2000u32)
                .map(|k| (k.wrapping_mul(2_654_435_761).wrapping_add(i as u32 * 7919) >> 13) as u8)
                .collect();
            put(format!("base/file{i}.dat"), bytes, "base");
        }
        put("models/player.mdl".into(), b"a player".to_vec(), "players");
        put("extra/big.dat".into(), vec![9u8; 50_000], "extra");
        // A later map (`map/later`, the pool's: prefetched): 60 kB a record.
        for i in 0..8u32 {
            // xorshift: bytes zstd can't shrink.
            let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ u64::from(i + 1);
            let bytes: Vec<u8> = (0..60_000)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    (x >> 24) as u8
                })
                .collect();
            put(format!("maps/later{i}.dat"), bytes, "map/later");
        }
        m.save(&store.manifest_path(GAME, "7")).unwrap();
        let running = server::serve(
            Service::new(store.clone()).unwrap().with_latency(latency),
            "127.0.0.1:0",
            4,
        )
        .unwrap();
        let identity = Identity::from_seed(&[3; 32], [5; 16]);
        let front = tls::front(
            "127.0.0.1:0",
            running.addr,
            &identity,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let fingerprint = identity.fingerprint();
        Self {
            dir,
            store,
            manifest: m,
            _running: running,
            front,
            fingerprint,
        }
    }

    fn user(&self, name: &str) -> String {
        let path = Accounts::path(&self.store.root);
        let mut accounts = Accounts::load(&path).unwrap();
        let (_, token) = accounts.add(name, None, "test").unwrap();
        accounts.save(&path).unwrap();
        token
    }

    fn revoke(&self, name: &str) {
        let path = Accounts::path(&self.store.root);
        let mut accounts = Accounts::load(&path).unwrap();
        accounts.revoke(name, None).unwrap();
        accounts.save(&path).unwrap();
    }

    fn host(&self) -> AssetHost {
        AssetHost::new(self.front.url(), self.fingerprint.clone())
    }

    /// A client's session: its own store, the service with `token`, the
    /// game read from the store alone (`served`) or from an install.
    fn session(&self, client: &str, token: &str, served: bool) -> AssetSession {
        AssetSession {
            settings: Some(AssetSettings {
                store: Some(Store::new(self.dir.join(client))),
                use_install: false,
                builds: Default::default(),
                remote: Some(RemoteConfig {
                    url: self.front.url(),
                    token: token.into(),
                    key: None,
                    fingerprint: Some(self.fingerprint.clone()),
                }),
                lock: None,
            }),
            served: if served {
                BTreeSet::from([GAME.to_string()])
            } else {
                BTreeSet::new()
            },
            access: Some(self.dir.join(format!("{client}-access.toml"))),
        }
    }

    /// The host publishes again at the same build: `path` (new bytes)
    /// added to `set`, the manifest's other lines unchanged. Waits until
    /// the service has read it (it looks at most once a second).
    fn republish(&mut self, set: &str, path: &str, bytes: &[u8]) -> String {
        let p = self.store.put(bytes).unwrap();
        self.manifest.files.insert(
            path.into(),
            FileRecord {
                id: p.id.clone(),
                size: bytes.len() as u64,
                stored: p.stored,
            },
        );
        self.manifest.sets.entry(set.into()).or_default().insert(path.into());
        self.manifest.save(&self.store.manifest_path(GAME, "7")).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        p.id
    }

    /// Record ids of `set`.
    fn ids(&self, set: &str) -> Vec<String> {
        self.manifest.sets[set]
            .iter()
            .map(|p| self.manifest.files[p].id.clone())
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// This machine's report: the stand-in game there (installed or served).
fn mounts(has_game: bool) -> LocalMounts {
    LocalMounts(MountReport {
        games: vec![GameMount {
            game: GAME.into(),
            installed: has_game,
            build: None,
            notes: Vec::new(),
        }],
    })
}

fn client_with(session: AssetSession, has_game: bool) -> impl FnOnce(&mut World) {
    move |w: &mut World| {
        w.insert_resource(session);
        w.insert_resource(mounts(has_game));
    }
}

fn progress(sim: &NetSim, i: usize) -> JoinProgress {
    sim.clients[i].app.world().resource::<JoinProgress>().clone()
}

/// Step (in real time too: the fetch runs on a thread) until `done`.
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

fn joined(sim: &mut NetSim, i: usize) -> bool {
    sim.clients[i]
        .app
        .world()
        .contains_resource::<chimera_net::net::client::Joined>()
        && sim.local_player(i).is_some()
}

/// Join -> the server names its service -> the client fetches the match's
/// sets of the game it reads from the store -> it joins and spawns. Only
/// the offered sets come; the stage shows the download.
#[test]
fn a_client_without_the_install_fetches_the_offered_sets_then_spawns() {
    let svc = Fixture::new("fetch", 20, Duration::ZERO);
    let token = svc.user("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    let session = svc.session("client", &token, true);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    sim.add_client(client_with(session, true));
    assert!(
        until(&mut sim, 30.0, |s| joined(s, 0)),
        "joined: {:?}",
        progress(&sim, 0)
    );
    for set in ["base", "players"] {
        assert!(svc.ids(set).iter().all(|id| store.has(id)), "{set} fetched");
    }
    assert!(
        svc.ids("extra").iter().all(|id| !store.has(id)),
        "a set the match doesn't offer isn't fetched"
    );
    let p = progress(&sim, 0);
    let stages: Vec<JoinStage> = p.log.iter().map(|l| l.0).collect();
    let at = stages
        .iter()
        .position(|s| *s == JoinStage::Downloading)
        .expect("a download stage");
    assert!(
        at < stages.iter().position(|s| *s == JoinStage::Joined).unwrap(),
        "{stages:?}"
    );
    assert!(p.failure.is_none());
}

/// A client whose install has the game (the hybrid model) fetches nothing.
#[test]
fn a_client_with_the_install_downloads_nothing() {
    let svc = Fixture::new("install", 5, Duration::ZERO);
    let token = svc.user("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    let session = svc.session("client", &token, false);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)));
    assert!(store.record_ids().is_empty(), "nothing downloaded");
    assert!(!progress(&sim, 0).log.iter().any(|l| l.0 == JoinStage::Downloading));
    let log = fs::read_to_string(svc.store.root.join("server/access.log")).unwrap_or_default();
    assert!(!log.contains("/batch"), "{log}");
}

/// A revoked token (and a mistyped one): the join fails at once with the
/// reason the loading dialog shows, not a hang.
#[test]
fn a_revoked_or_wrong_token_fails_the_join_with_the_reason() {
    let svc = Fixture::new("revoked", 5, Duration::ZERO);
    let token = svc.user("tester");
    svc.revoke("tester");
    for token in [token, "chimera1_mistyped".to_string()] {
        let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
        sim.add_client(client_with(svc.session("client", &token, true), true));
        assert!(
            until(&mut sim, 20.0, |s| progress(s, 0).failure.is_some()),
            "failed: {:?}",
            progress(&sim, 0)
        );
        let (kind, reason) = progress(&sim, 0).failure.unwrap();
        assert!(
            matches!(&kind, JoinFailure::Download { file, .. } if file == FILES),
            "{kind:?}"
        );
        assert!(
            reason.contains("refused your token") && reason.contains("chimera:asset_token"),
            "{reason}"
        );
        assert!(!joined(&mut sim, 0));
    }
}

/// The loading screen's bar: the bytes so far of the bytes in all, rising,
/// then the join.
#[test]
fn the_download_shows_its_progress() {
    // 600 records in batches of 256 on 4 connections, each answer 300 ms
    // late: the bytes arrive in steps.
    let svc = Fixture::new("progress", 600, Duration::from_millis(300));
    let token = svc.user("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(svc.session("client", &token, true), true));
    let total: u64 = ["base", "players"]
        .iter()
        .flat_map(|s| svc.manifest.sets[*s].iter())
        .map(|p| svc.manifest.files[p].stored)
        .sum();
    let mut seen = Vec::new();
    assert!(until(&mut sim, 60.0, |s| {
        if let Some(d) = progress(s, 0).download.filter(|d| d.file == FILES && d.total > 0) {
            if seen.last() != Some(&d.done) {
                seen.push(d.done);
            }
            assert_eq!(d.total, total, "the bytes in all");
        }
        joined(s, 0)
    }));
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "never back: {seen:?}");
    assert!(
        seen.iter().any(|d| *d > 0 && *d < total),
        "a step between none and all: {seen:?}"
    );
}

/// A client with no files of the game at all: with a token, the token is
/// bound to the offered service (its URL and certificate) and the game asks
/// to start again joining; without one, the join fails saying how to enter
/// it; bound to another certificate, the join fails and the token isn't
/// rebound.
#[test]
fn a_client_without_files_binds_its_token_and_starts_again() {
    let svc = Fixture::new("relaunch", 2, Duration::ZERO);
    let token = svc.user("tester");
    let no_service = |access: &Path| AssetSession {
        access: Some(access.to_path_buf()),
        ..Default::default()
    };

    // A token: bound, and the game is to start again.
    let access = svc.dir.join("a.toml");
    chimera_net::net::assets::save_token(&access, &token).unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(no_service(&access), false));
    assert!(until(&mut sim, 20.0, |s| s.clients[0]
        .app
        .world()
        .contains_resource::<AssetRelaunch>()));
    let bound = Access::load_from(&access);
    assert_eq!(bound.url.as_deref(), Some(svc.front.url().as_str()));
    assert_eq!(bound.fingerprint.as_deref(), Some(svc.fingerprint.as_str()));
    assert!(progress(&sim, 0).failure.is_none(), "{:?}", progress(&sim, 0).failure);

    // Files in the store from an earlier session, the token entered again
    // (its binding forgotten, so no service this start): bound, and the
    // game starts again rather than playing on with nothing to fetch.
    let again = svc.dir.join("again.toml");
    chimera_net::net::assets::save_token(&again, &token).unwrap();
    let mut session = svc.session("warm", &token, true);
    if let Some(s) = session.settings.as_mut() {
        s.remote = None;
    }
    session.access = Some(again.clone());
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 20.0, |s| s.clients[0]
        .app
        .world()
        .contains_resource::<AssetRelaunch>()));
    assert_eq!(Access::load_from(&again).fingerprint.as_deref(), Some(svc.fingerprint.as_str()));

    // No token: the server refuses it (it can't fetch), saying how to
    // enter one.
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(no_service(&svc.dir.join("none.toml")), false));
    assert!(until(&mut sim, 20.0, |s| progress(s, 0).failure.is_some()));
    let (kind, reason) = progress(&sim, 0).failure.unwrap();
    assert_eq!(kind, JoinFailure::Mounts);
    assert!(reason.contains("+chimera:asset_token <your token>"), "{reason}");

    // Bound to another service's certificate: refused, the binding kept.
    let other = svc.dir.join("other.toml");
    let mut a = Access::default();
    a.set_token(&token).unwrap();
    a.bind("https://elsewhere:1", &"ab".repeat(32)).unwrap();
    a.save_to(&other).unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(no_service(&other), false));
    assert!(until(&mut sim, 20.0, |s| progress(s, 0).failure.is_some()));
    let (_, reason) = progress(&sim, 0).failure.unwrap();
    assert!(reason.contains("isn't the one your token is for"), "{reason}");
    assert_eq!(Access::load_from(&other), a, "not rebound");
    assert!(!sim.clients[0].app.world().contains_resource::<AssetRelaunch>());

    // Started again with that service, which refused the token at startup
    // (revoked since): the reason, not a loop of restarts.
    svc.revoke("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    let mut session = svc.session("again", &token, true);
    session.served.clear();
    session.access = Some(access.clone());
    sim.add_client(client_with(session, false));
    assert!(until(&mut sim, 20.0, |s| progress(s, 0).failure.is_some()));
    let (_, reason) = progress(&sim, 0).failure.unwrap();
    assert!(reason.contains("refused your token"), "{reason}");
    assert!(!sim.clients[0].app.world().contains_resource::<AssetRelaunch>());
}

fn prefetch(sim: &NetSim, i: usize) -> Option<assets::PrefetchState> {
    assets::prefetch_state(sim.clients[i].app.world())
}

/// The pool's later map comes in the background at the prefetch's rate:
/// the client's, capped by the server's hint; held while the client is
/// changing level; none at all when the server says 0.
#[test]
fn the_prefetch_keeps_to_its_rate_and_holds_for_the_foreground() {
    let svc = Fixture::new("prefetch", 2, Duration::ZERO);
    let token = svc.user("tester");
    let later = svc.ids("map/later");
    let bytes: u64 = svc.manifest.sets["map/later"].iter().map(|p| svc.manifest.files[p].stored).sum();
    assert!(bytes > 300_000, "{bytes}");

    // The server caps it at 100 KB/s (the client's own is 2 MB/s).
    let mut host = svc.host();
    host.prefetch_max = 100;
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup_pool(host));
    let session = svc.session("client", &token, true);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)));
    assert!(until(&mut sim, 10.0, |s| prefetch(s, 0).is_some_and(|p| p.running)));
    let state = prefetch(&sim, 0).unwrap();
    assert_eq!(state.rate_kb, 100, "{state:?}");
    assert!(!state.paused);
    // Changing level: held.
    sim.clients[0].app.world_mut().resource_mut::<JoinProgress>().stage = JoinStage::ChangingLevel;
    sim.step();
    assert!(prefetch(&sim, 0).unwrap().paused, "held while changing level");
    sim.clients[0].app.world_mut().resource_mut::<JoinProgress>().stage = JoinStage::Joined;
    sim.step();
    assert!(!prefetch(&sim, 0).unwrap().paused);
    // At 100 KB/s the set (> 300 kB) takes more than 2.5 s: after 1 s of
    // play, never all of it (a lower bound: pacing is never faster).
    let start = std::time::Instant::now();
    until(&mut sim, 1.0, |_| false);
    let elapsed = start.elapsed().as_secs_f64();
    let have = later.iter().filter(|id| store.has(id)).count();
    assert!(have < later.len() || elapsed > 2.5, "{have} of {} after {elapsed:.1} s", later.len());
    // The client may lower it further (the cvar), and it all arrives.
    sim.clients[0].app.world_mut().insert_resource(PrefetchRate(50));
    sim.step();
    assert_eq!(prefetch(&sim, 0).unwrap().rate_kb, 50);
    sim.clients[0].app.world_mut().insert_resource(PrefetchRate(100_000));
    assert!(
        until(&mut sim, 30.0, |_| later.iter().all(|id| store.has(id))),
        "the later map prefetched"
    );

    // The server says 0: no prefetch for anyone.
    let mut host = svc.host();
    host.prefetch_max = 0;
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup_pool(host));
    let session = svc.session("client0", &token, true);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)));
    until(&mut sim, 1.0, |_| false);
    let state = prefetch(&sim, 0).unwrap();
    assert_eq!((state.rate_kb, state.running), (0, false), "{state:?}");
    assert!(later.iter().all(|id| !store.has(id)), "nothing prefetched");
}

/// A player who fetched before the host published again (same build, sets
/// grown: the 2026-11-11 republish of Combat Arms' `base`, `players` and
/// weapon sets): the next start fetches the service's newer manifest and
/// the startup sets' added files behind the window; the next join fetches
/// the offered sets' added files before it spawns; none is left to come
/// on demand, and the kept manifest is the service's.
#[test]
fn a_player_who_fetched_before_a_republish_gets_the_new_sets() {
    let mut svc = Fixture::new("republish", 5, Duration::ZERO);
    let token = svc.user("tester");
    let session = svc.session("client", &token, true);
    let settings = session.settings.clone().unwrap();
    let store = settings.store.clone().unwrap();
    let game = AppGame { game: GAME.into(), installed: false };

    // Before: the first start and join fetch the sets as they were.
    let wanted = asset_startup::plan(&settings, &[game.clone()]).expect("planned");
    assert!(matches!(run_startup(settings.clone(), wanted).0, Status::Done(_)));
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(session.clone(), true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    drop(sim);
    let kept = store.manifest_path(GAME, "7");
    let old = fs::read(&kept).unwrap();

    // The host publishes again: `base` and `players` each gain a file.
    let in_base = svc.republish("base", "base/added.dat", b"added to base");
    let in_players = svc.republish("players", "models/added.mdl", b"added to players");
    assert!(
        asset_startup::plan(&settings, &[game.clone()]).is_none(),
        "from the kept manifest alone, nothing looks missing"
    );

    // The next start: the newer manifest, then its `base` behind the window.
    assert_eq!(asset_startup::refresh(&settings, &[game.clone()], Duration::from_secs(10)), [GAME]);
    assert_ne!(fs::read(&kept).unwrap(), old, "the kept manifest replaced");
    let wanted = asset_startup::plan(&settings, &[game.clone()]).expect("the new base file is planned");
    assert!(matches!(run_startup(settings.clone(), wanted).0, Status::Done(_)));
    assert!(store.has(&in_base), "base's added file fetched at startup");
    assert!(
        asset_startup::refresh(&settings, &[game.clone()], Duration::from_secs(10)).is_empty(),
        "up to date: nothing fetched again"
    );

    // The next join: the offered `players`' added file before it spawns.
    assert!(!store.has(&in_players));
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(session.clone(), true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    assert!(store.has(&in_players), "players' added file fetched at the join");
    drop(sim);

    // A join alone (no start between) refreshes too: a third publish.
    let in_players2 = svc.republish("players", "models/added2.mdl", b"added again");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    assert!(store.has(&in_players2), "fetched at the join");
    assert_eq!(
        chimera_assets::sha256_hex(&fs::read(&kept).unwrap()),
        chimera_assets::sha256_hex(&fs::read(svc.store.manifest_path(GAME, "7")).unwrap()),
        "the kept manifest is the service's"
    );
}

/// A session's settings for the startup fetch with `token` at `url`
/// pinning `fingerprint`.
fn startup_settings(svc: &Fixture, client: &str, token: &str, url: String, fingerprint: &str) -> AssetSettings {
    let mut s = svc.session(client, token, true).settings.unwrap();
    let remote = s.remote.as_mut().unwrap();
    remote.url = url;
    remote.fingerprint = Some(fingerprint.to_string());
    s
}

fn run_startup(settings: AssetSettings, wanted: Vec<chimera_assets::sets::Wanted>) -> (Status, Vec<Status>) {
    let mut fetch = StartupFetch::start(settings, wanted);
    let start = std::time::Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < Duration::from_secs(30) {
        let s = fetch.status();
        if matches!(s, Status::Done(_) | Status::Failed(_)) {
            return (s, seen);
        }
        seen.push(s);
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the startup fetch didn't end: {seen:?}");
}

/// The startup fetch (before the window): planned only for a game that
/// isn't installed and whose startup sets the store lacks; it fetches
/// them (and nothing else) with its progress, then plans nothing; a game
/// the service doesn't list is skipped; a refused token, an unreachable
/// service and another certificate end it with the reason.
#[test]
fn the_startup_fetch_gets_what_the_games_read_first_or_says_why_not() {
    let svc = Fixture::new("startup", 300, Duration::from_millis(100));
    let token = svc.user("tester");
    let settings = startup_settings(&svc, "client", &token, svc.front.url(), &svc.fingerprint);
    let store = settings.store.clone().unwrap();
    let game = |installed| AppGame { game: GAME.into(), installed };
    let mut settings_install = settings.clone();
    settings_install.use_install = true;
    assert!(
        asset_startup::plan(&settings_install, &[game(true)]).is_none(),
        "an installed game is never fetched"
    );
    let mut no_service = settings.clone();
    no_service.remote = None;
    assert!(asset_startup::plan(&no_service, &[game(false)]).is_none(), "no service: nothing");
    let other = AppGame { game: "not_served".into(), installed: false };
    let wanted = asset_startup::plan(&settings, &[game(false), other]).expect("planned");
    assert_eq!(wanted[0].sets, ["base"]);

    let (end, seen) = run_startup(settings.clone(), wanted);
    assert!(matches!(end, Status::Done(_)), "{end:?}");
    assert!(svc.ids("base").iter().all(|id| store.has(id)), "base fetched");
    assert!(svc.ids("players").iter().all(|id| !store.has(id)), "only the startup sets");
    let total: u64 = svc.manifest.sets["base"].iter().map(|p| svc.manifest.files[p].stored).sum();
    assert!(
        seen.iter().any(|s| matches!(s, Status::Downloading { total: t, .. } if *t == total)),
        "the bytes in all shown: {seen:?}"
    );
    assert!(
        asset_startup::plan(&settings, &[game(false)]).is_none(),
        "all here: no window next time"
    );

    // Errors, each with what to do.
    let fresh = |name: &str, token: &str, url: String, fp: &str| {
        let s = startup_settings(&svc, name, token, url, fp);
        let wanted = asset_startup::plan(&s, &[game(false)]).expect("planned");
        match run_startup(s, wanted).0 {
            Status::Failed(why) => why,
            other => panic!("{other:?}"),
        }
    };
    let why = fresh("mistyped", "chimera1_mistyped", svc.front.url(), &svc.fingerprint);
    assert!(why.contains("refused your token") && why.contains("chimera:asset_token"), "{why}");
    let why = fresh("other_cert", &token, svc.front.url(), &"ab".repeat(32));
    assert!(why.contains("another certificate"), "{why}");
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let why = fresh("unreachable", &token, format!("https://{closed}"), &svc.fingerprint);
    assert!(why.contains("can't reach the asset service"), "{why}");
}
