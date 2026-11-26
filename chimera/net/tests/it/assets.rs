//! A match's game files from the host's asset service
//! (docs/plans/active/asset-service.md, "Playtest delivery"; `net::assets`):
//! on the engine's base and the greybox, a stand-in game (`fixture_game`)
//! whose files are our own fixture bytes, served by a real asset service on
//! 127.0.0.1 behind its TLS front with a self-signed certificate. The
//! server names the service (`AssetHost`); a client that reads the game from
//! the store alone fetches the offered sets (and not others) with progress,
//! then joins and spawns; one with the install fetches nothing; a revoked
//! token fails the join with the reason; a client with no files of the game
//! binds its token to the offered service and fetches them in the same game
//! (no restart), or, with no token, is told how to enter one; a game's
//! weapons built from its files are built again before the match is checked.

use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
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
        assets::{self, AssetHooks, AssetHost, AssetNames, AssetSession, AssetsArrived, FILES, JoinSets, MapAhead, StreamLink, Streams},
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

/// The match hands out the stand-in game's gun (`weapon/gun`).
fn gun(_: &World) -> AssetNames {
    AssetNames {
        weapons: vec![format!("{GAME}:weapon_gun")],
        ..default()
    }
}

/// `setup` with the gun in the offer.
fn setup_gun(host: AssetHost) -> impl Fn(&mut App) + Send + Sync + 'static {
    let base = setup(host);
    move |app: &mut App| {
        base(app);
        app.world_mut().resource_mut::<AssetHooks>().0.push(gun);
    }
}

/// A client part's own sets for the match (music's first track).
fn first_track(_: &World) -> Vec<(String, String)> {
    vec![(GAME.into(), "music/bgm/first.mp3".into())]
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
        Self::make(name, files, latency, false)
    }

    /// A hub's: account tickets only (`TicketAuth`, its keys in the
    /// store's `server/accounts/ticket.pub`; no revocation feed).
    fn with_tickets(name: &str, files: usize) -> Self {
        Self::make(name, files, Duration::ZERO, true)
    }

    fn make(name: &str, files: usize, latency: Duration, tickets: bool) -> Self {
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
        // Each match's other sets: `common`, a gun with its sound, the
        // music's first track.
        put("sound/explode.wav".into(), b"boom".to_vec(), "common");
        put("models/gun.mdl".into(), b"a gun".to_vec(), "weapon/gun");
        put("sound/gun/fire.wav".into(), b"bang".to_vec(), "weapon/gun");
        put("loose/bgm/first.mp3".into(), b"a first track".to_vec(), "music/bgm/first.mp3");
        // The next map (`map/later`, the vote's winner) and the next music
        // track (`music/bgm/next.mp3`): 60 kB a record, 8 and 6 of them.
        for i in 0..14u32 {
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
            if i < 8 {
                put(format!("maps/later{i}.dat"), bytes, "map/later");
            } else {
                put(format!("loose/bgm/next{i}.mp3"), bytes, "music/bgm/next.mp3");
            }
        }
        m.save(&store.manifest_path(GAME, "7")).unwrap();
        let service = if tickets {
            let auth: Vec<Box<dyn chimera_assets::accounts::Authenticator>> = vec![
                Box::new(chimera_assets::accounts::TicketAuth::new(
                    chimera_assets::accounts::ticket_keys_path(&store.root),
                )),
            ];
            Service::with_authenticators(store.clone(), auth).unwrap()
        } else {
            Service::new(store.clone()).unwrap()
        };
        let running = server::serve(service.with_latency(latency), "127.0.0.1:0", 4).unwrap();
        // A certificate of its own per fixture: the process's live session
        // (`account::set_live`, the last account a test opened) is sent to any
        // service whose certificate it names (`AccountFile::serves`), so
        // fixtures sharing one identity sent each other's tickets (refused:
        // another service's key) when `cargo test`'s threads overlapped.
        static NEXT_IDENTITY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);
        let mut seed = [3; 32];
        seed[31] = NEXT_IDENTITY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let identity = Identity::from_seed(&seed, [5; 16]);
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
                service_first: false,
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
            account: Some(self.dir.join(format!("{client}-account.toml"))),
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

/// Step (the fetch runs on a thread: `let_io_run` between frames) until
/// `done`, at most `secs` x 1000 rounds: a count, not a clock, so a
/// loaded machine takes longer but doesn't flake it.
fn until(sim: &mut NetSim, secs: f64, mut done: impl FnMut(&mut NetSim) -> bool) -> bool {
    for _ in 0..(secs * 1000.0) as u64 {
        if done(sim) {
            return true;
        }
        sim.step();
        chimera_world::harness::let_io_run();
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

/// A file the match reads that its sets didn't bring is fetched on demand:
/// the client counts it against the match on screen (`net::on_demand`)
/// and the service marks the request ` on-demand` in its access log; the
/// match's end ends its count.
#[test]
fn an_on_demand_fetch_during_the_match_is_counted_and_marked() {
    use chimera_assets::{CachedFiles, Remote};
    use chimera_net::net::on_demand::OnDemandCount;
    use chimera_core::mount::FileSource;

    let svc = Fixture::new("on_demand", 5, Duration::ZERO);
    let token = svc.user("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    let session = svc.session("client", &token, true);
    let settings = session.settings.clone().unwrap();
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    sim.step();
    // A loader reads a file of a set the match didn't offer.
    let store = settings.store.clone().unwrap();
    let remote = Remote::new(settings.remote.clone().unwrap()).unwrap();
    let files = CachedFiles::open(&store, GAME, "7").unwrap().with_fetcher(Some(std::sync::Arc::new(remote)));
    assert!(files.read("extra/big.dat").unwrap().is_ok());
    sim.step();
    let count = sim.clients[0].app.world().resource::<OnDemandCount>().clone();
    assert!(count.map.is_some(), "in the match: {count:?}");
    assert!(count.during.iter().any(|f| f.path == "extra/big.dat" && f.game == GAME), "{count:?}");
    // The service writes its line once the answer is sent.
    let marked = |log: &str| log.lines().any(|l| l.contains("GET /r/") && l.ends_with(" on-demand"));
    let mut log = String::new();
    for _ in 0..50 {
        log = fs::read_to_string(svc.store.root.join("server/access.log")).unwrap_or_default();
        if marked(&log) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(marked(&log), "{log}");
    assert!(
        !log.lines().any(|l| l.contains("POST /batch") && l.ends_with(" on-demand")),
        "the join's download isn't marked: {log}"
    );
    // The match ends (here: the next match's lobby shows): its count
    // starts again.
    sim.clients[0].app.world_mut().resource_mut::<chimera_net::net::on_demand::LobbyShowing>().0 = true;
    sim.step();
    let count = sim.clients[0].app.world().resource::<OnDemandCount>().clone();
    assert!(count.map.is_none() && count.during.is_empty(), "{count:?}");
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
            reason.contains("refused you") && reason.contains("chimera:asset_token"),
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
    let total: u64 = ["base", "players", "common"]
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

/// A client with no files of the game and a token bound to no service
/// (a tester's first join), its own empty store.
fn fresh_client(svc: &Fixture, client: &str, token: &str) -> (AssetSession, PathBuf) {
    let access = svc.dir.join(format!("{client}-access.toml"));
    chimera_net::net::assets::save_token(&access, token).unwrap();
    let mut session = svc.session(client, token, false);
    if let Some(s) = session.settings.as_mut() {
        s.remote = None;
    }
    session.access = Some(access.clone());
    (session, access)
}

/// Whether client `i` reads the game from the store now, with the service.
fn reads_from(sim: &NetSim, i: usize, url: &str) -> bool {
    let world = sim.clients[i].app.world();
    let session = world.resource::<AssetSession>();
    session.served.contains(GAME)
        && session.settings.as_ref().and_then(|s| s.remote.as_ref()).is_some_and(|r| r.url == url)
        && world.resource::<LocalMounts>().0.has(GAME)
}

/// The server's view of its one player's mount report (its userinfo's).
fn server_report(sim: &mut NetSim) -> Option<MountReport> {
    let world = sim.server.app.world_mut();
    let mut q = world.query::<(&chimera_net::net::NetCharacter, &chimera_core::core::UserInfo)>();
    let info = q.iter(world).find(|(c, _)| c.owner.is_some())?.1.0.clone();
    chimera_net::net::manifest::from_userinfo(info.iter().map(|(k, v)| (k.as_str(), v.as_str())))
}

/// Counts the remounts (`AssetsArrived`).
static REMOUNTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn count_remount(_: On<AssetsArrived>) {
    REMOUNTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// A tester's first join: a token bound to no service, no files of the
/// game. The token is bound to the offered service and the game's startup
/// and offered sets come in this same app, with the join's download
/// stage; the parts read again (`AssetsArrived`), and the client joins and
/// spawns without leaving the server (no game started again). Joining
/// again (the server restarted) reuses the binding: nothing bound anew,
/// nothing read again.
#[test]
fn a_first_join_binds_the_token_and_fetches_in_the_same_game() {
    let svc = Fixture::new("bind_here", 4, Duration::ZERO);
    let token = svc.user("tester");
    let (session, access) = fresh_client(&svc, "fresh", &token);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(move |w: &mut World| {
        client_with(session, false)(w);
        w.add_observer(count_remount);
    });
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    let bound = Access::load_from(&access);
    assert_eq!(bound.url.as_deref(), Some(svc.front.url().as_str()));
    assert_eq!(bound.fingerprint.as_deref(), Some(svc.fingerprint.as_str()));
    for set in ["base", "players"] {
        assert!(svc.ids(set).iter().all(|id| store.has(id)), "{set} fetched");
    }
    assert!(svc.ids("extra").iter().all(|id| !store.has(id)), "only what the match offers");
    assert!(reads_from(&sim, 0, &svc.front.url()), "the game is read from the store with the service now");
    assert_eq!(REMOUNTS.load(std::sync::atomic::Ordering::SeqCst), 1, "read again once");
    // The server hears that the client has the game now (its mount report
    // sent again: the next match's checks and ladders count it).
    assert!(
        until(&mut sim, 10.0, |s| server_report(s).is_some_and(|r| r.has(GAME))),
        "the server's view of the client: {:?}",
        server_report(&mut sim)
    );
    let p = progress(&sim, 0);
    assert!(p.failure.is_none(), "{:?}", p.failure);
    let stages: Vec<JoinStage> = p.log.iter().map(|l| l.0).collect();
    let at = stages.iter().position(|s| *s == JoinStage::Downloading).expect("a download stage");
    assert!(at < stages.iter().position(|s| *s == JoinStage::Joined).unwrap(), "{stages:?}");
    // The server saw one player, who never left.
    assert_eq!(sim.clients.len(), 1);

    // The server restarts; the client joins it again on its own, with the
    // binding it has.
    let before = fs::read(&access).unwrap();
    sim.stop_server();
    assert!(until(&mut sim, 10.0, |s| !joined(s, 0)), "the client heard the server go");
    sim.start_server();
    assert!(until(&mut sim, 60.0, |s| joined(s, 0)), "joined again: {:?}", progress(&sim, 0));
    assert_eq!(fs::read(&access).unwrap(), before, "the binding as it was");
    assert_eq!(REMOUNTS.load(std::sync::atomic::Ordering::SeqCst), 1, "nothing read again");
    assert!(reads_from(&sim, 0, &svc.front.url()));
    assert!(progress(&sim, 0).failure.is_none());
}

/// A game the match only wants (another game's guns in a ladder, here
/// `unpublished_game`), which the service doesn't serve.
const UNPUBLISHED: &str = "unpublished_game";

fn wants_unpublished(_: &mut World, m: &mut MatchManifest) {
    m.need(UNPUBLISHED, false);
}

/// A first join to a match that also wants a game the service doesn't
/// publish (2026-11-15: Halo's guns joined the party's ladders before its
/// sets were published, and every first join failed with "the asset
/// service doesn't serve Halo"): the required game's files come, the
/// wanted one is left out with a note, and the client joins.
#[test]
fn a_first_join_goes_on_without_a_wanted_game_the_service_lacks() {
    let svc = Fixture::new("bind_wanted", 4, Duration::ZERO);
    let token = svc.user("tester");
    let (session, _access) = fresh_client(&svc, "fresh_wanted", &token);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    let base = setup(svc.host());
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, move |app: &mut App| {
        base(app);
        app.world_mut().resource_mut::<NetHooks>().manifest.push(wants_unpublished);
    });
    sim.add_client(client_with(session, false));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    assert!(svc.ids("base").iter().all(|id| store.has(id)), "the required game's base fetched");
    assert!(reads_from(&sim, 0, &svc.front.url()));
    assert!(progress(&sim, 0).failure.is_none(), "{:?}", progress(&sim, 0).failure);
}

/// The same with the token bound already (any start after the first: this
/// process reads from the offered service from its start): the wanted
/// game the service doesn't serve is left out and the client joins
/// (2026-11-15: such a start left every UGG match with "the asset service
/// doesn't serve tf2").
#[test]
fn a_bound_client_goes_on_without_a_wanted_game_the_service_lacks() {
    let svc = Fixture::new("bound_wanted", 4, Duration::ZERO);
    let token = svc.user("tester");
    let session = svc.session("bound_wanted", &token, true);
    let access = session.access.clone().unwrap();
    chimera_net::net::assets::save_token(&access, &token).unwrap();
    let mut bound = Access::load_from(&access);
    bound.bind(&svc.front.url(), &svc.fingerprint).unwrap();
    bound.save_to(&access).unwrap();
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    let base = setup(svc.host());
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, move |app: &mut App| {
        base(app);
        app.world_mut().resource_mut::<NetHooks>().manifest.push(wants_unpublished);
    });
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    assert!(svc.ids("base").iter().all(|id| store.has(id)), "the served game's base fetched");
    assert!(progress(&sim, 0).failure.is_none(), "{:?}", progress(&sim, 0).failure);
}

/// The stand-in game's weapon documents: what a machine has without its
/// files (`first`), and with them (`first` and `arrived`: a game whose
/// weapon set is built from its install, Combat Arms').
const WEAPONS_SET: &str = "fixture_game:weapons";

fn weapon_docs(with_files: bool) -> Vec<(String, String)> {
    let doc = |id: &str| (format!("{id}.toml"), format!("id = \"{GAME}:{id}\"\nslot = 1\ndraw = 1.0\n"));
    let mut docs = vec![doc("first")];
    if with_files {
        docs.push(doc("arrived"));
    }
    docs
}

/// The stand-in game's weapon set made from `weapon_docs`, put in place
/// of this world's (`server_docs::renew_own`): what a game's
/// `AssetsArrived` observer does once its files are here.
fn renew_weapons(world: &mut World, with_files: bool) {
    use chimera_sim::{server_docs::{self, DocApply}, weapon::data::WeaponSet};
    let docs = weapon_docs(with_files);
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set: &'static WeaponSet = Box::leak(Box::new(WeaponSet::parse(&refs, &[]).unwrap()));
    server_docs::renew_own(world, WEAPONS_SET, docs, DocApply::Weapons(set)).unwrap();
}

/// A client's manifest hook (as the minigame's, `minigame_net`): the
/// weapon the match hands out must be in the catalogue.
fn needs_arrived_weapon(world: &mut World, _: &MatchManifest) -> Result<(), String> {
    let id = format!("{GAME}:arrived");
    world
        .get_resource::<chimera_sim::weapon::data::WeaponCatalog>()
        .and_then(|c| c.get(&id))
        .map(|_| ())
        .ok_or_else(|| format!("weapon {id}: not in the weapon catalogue"))
}

/// A tester's first join with a game whose weapons are built from its
/// files (Combat Arms' install weapons): the client starts with the
/// built-in set only, the server has the full one. Once the files arrive,
/// the game's `AssetsArrived` observer queues its set made again; that is
/// in place before the join checks the match (its minigame's weapons in
/// the catalogue), at the server's registry indices, and as this
/// machine's own documents: none are taken from the server.
#[test]
fn a_first_join_rebuilds_a_games_weapons_before_the_match_is_checked() {
    use chimera_sim::{server_docs::{DocApply, DocsAppExt, ServerDocs}, weapon::{WeaponRegistry, data::WeaponDataAppExt}};
    let svc = Fixture::new("bind_weapons", 2, Duration::ZERO);
    let token = svc.user("tester");
    let (session, _) = fresh_client(&svc, "weapons", &token);
    let base = setup(svc.host());
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, move |app: &mut App| {
        base(app);
        // Every app starts with the built-in set, and a weapon of another
        // game registered after it (its index moves with the set's).
        let docs = weapon_docs(false);
        let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let set = app.add_weapon_data(&refs).unwrap();
        app.server_weapon_docs(WEAPONS_SET, set, docs.clone());
        app.add_weapon_data(&[("later.toml", "id = \"other:later\"\nslot = 1\ndraw = 1.0\n")]).unwrap();
    });
    // The server reads the game's files.
    renew_weapons(sim.server.app.world_mut(), true);
    sim.add_client(move |w: &mut World| {
        client_with(session, false)(w);
        w.resource_mut::<NetHooks>().accept.push(needs_arrived_weapon);
        w.add_observer(|a: On<AssetsArrived>, mut commands: Commands| {
            if a.games.iter().any(|g| g == GAME) {
                commands.queue(|w: &mut World| renew_weapons(w, true));
            }
        });
    });
    let ids = |w: &World| w.resource::<WeaponRegistry>().0.iter().map(|d| d.id).collect::<Vec<_>>();
    assert_eq!(ids(sim.clients[0].app.world()), [format!("{GAME}:first").as_str(), "other:later"], "built in only");
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    let client = sim.clients[0].app.world();
    assert_eq!(ids(client), ids(sim.server.app.world()), "the server's indices");
    assert!(ids(client).contains(&format!("{GAME}:arrived").as_str()));
    let own = client.resource::<ServerDocs>().get(WEAPONS_SET).unwrap();
    assert!(!own.replaced(), "this machine's own documents, none taken from the server");
    assert_eq!(*own.docs, weapon_docs(true));
    assert!(matches!(own.apply, DocApply::Weapons(s) if s.weapons.len() == 2));
    assert!(progress(&sim, 0).failure.is_none());
}

/// A player signed in to the hub (an account file, no asset token at all)
/// with no files of the game: the offered service has the hub's
/// certificate, so the join uses the account's ticket (nothing bound in
/// the access file), fetches the offered sets and spawns; the service
/// logs the account. The ticket here is made with a test key the service
/// trusts (`ticket.pub`), as the accounts service would make it.
#[test]
fn a_hub_account_joins_with_its_ticket_and_no_token() {
    use chimera_assets::{account::AccountFile, ticket};
    let svc = Fixture::with_tickets("account_join", 3);
    let key = chimera_assets::sign::load_or_create_key(&svc.dir.join("ticket.key")).unwrap();
    let keys_path = chimera_assets::accounts::ticket_keys_path(&svc.store.root);
    fs::create_dir_all(keys_path.parent().unwrap()).unwrap();
    fs::write(&keys_path, ticket::TicketKeys::single(key.verifying_key()).to_text()).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let (account, device) = account_ids();
    let claims = ticket::Claims {
        version: ticket::VERSION,
        key_id: String::new(),
        account,
        device,
        username: "ann".into(),
        role: "tester".into(),
        content: vec!["*".into()],
        games: vec!["*".into()],
        list: 1,
        report: false,
        muted: false,
        audience: ticket::SERVICES.into(),
        issued: now,
        expires: now + ticket::LIFETIME_SECS,
        display_name: "ann".into(),
    };
    let file = AccountFile {
        url: svc.front.url(),
        fingerprint: Some(svc.fingerprint.clone()),
        assets_url: None,
        account: ticket::id_hex(&account),
        device: ticket::id_hex(&device),
        username: "ann".into(),
        display_name: "ann".into(),
        device_key: "03".repeat(32),
        ticket: Some(ticket::issue(&key, &claims)),
    };
    let account_path = svc.dir.join("ann-account.toml");
    file.save_to(&account_path).unwrap();
    let access = svc.dir.join("ann-access.toml");
    let mut session = svc.session("ann", "", false);
    if let Some(s) = session.settings.as_mut() {
        s.remote = None;
    }
    session.access = Some(access.clone());
    session.account = Some(account_path);
    assert!(session.can_fetch(), "an account can fetch");
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(session, false));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    for set in ["base", "players"] {
        assert!(svc.ids(set).iter().all(|id| store.has(id)), "{set} fetched");
    }
    assert!(reads_from(&sim, 0, &svc.front.url()));
    let token = sim.clients[0].app.world().resource::<AssetSession>().settings.as_ref().unwrap().remote.as_ref().unwrap().token.clone();
    assert!(token.starts_with(ticket::PREFIX), "the account's ticket, not a token");
    assert!(Access::load_from(&access).token.is_none(), "nothing bound in the access file");
    let log = fs::read_to_string(svc.store.root.join("server").join("access.log")).unwrap();
    assert!(log.lines().any(|l| l.contains("account:ann") && l.contains(" 200 ")), "{log}");
}

/// Files in the store from an earlier session and the token entered again
/// (its binding forgotten, so no service this start): bound at the join
/// and filled in the same game, rather than playing on with nothing to
/// fetch.
#[test]
fn a_token_entered_again_is_bound_at_the_join_and_the_store_filled() {
    let svc = Fixture::new("rebind", 3, Duration::ZERO);
    let token = svc.user("tester");
    let again = svc.dir.join("again.toml");
    chimera_net::net::assets::save_token(&again, &token).unwrap();
    let mut session = svc.session("warm", &token, true);
    if let Some(s) = session.settings.as_mut() {
        s.remote = None;
    }
    session.access = Some(again.clone());
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    assert_eq!(Access::load_from(&again).fingerprint.as_deref(), Some(svc.fingerprint.as_str()));
    assert!(svc.ids("players").iter().all(|id| store.has(id)), "the offered sets fetched");
    assert!(reads_from(&sim, 0, &svc.front.url()));
}

/// The join's failures, in the game's loading dialog: a token the
/// service refuses (revoked or mistyped) at the first join; no token
/// (how to enter one); a token bound to another service's certificate
/// (not rebound); a service this game already reads from that refuses
/// the token now.
#[test]
fn a_join_that_cant_bind_says_why_in_the_game() {
    let svc = Fixture::new("bind_fail", 2, Duration::ZERO);
    let token = svc.user("tester");
    let failure = |sim: &mut NetSim| {
        assert!(until(sim, 20.0, |s| progress(s, 0).failure.is_some()), "{:?}", progress(sim, 0));
        assert!(!joined(sim, 0));
        progress(sim, 0).failure.unwrap()
    };

    // A mistyped token at the first join: bound, then refused.
    let (session, _) = fresh_client(&svc, "mistyped", "chimera1_mistyped");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(session, false));
    let (kind, reason) = failure(&mut sim);
    assert!(matches!(&kind, JoinFailure::Download { file, .. } if file == FILES), "{kind:?}");
    assert!(reason.contains("refused you") && reason.contains("chimera:asset_token"), "{reason}");

    // No token: the server refuses it (it can't fetch), saying how to
    // enter one.
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(
        AssetSession { access: Some(svc.dir.join("none.toml")), ..Default::default() },
        false,
    ));
    let (kind, reason) = failure(&mut sim);
    assert_eq!(kind, JoinFailure::Mounts);
    assert!(reason.contains("+chimera:asset_token <your token>"), "{reason}");

    // Bound to another service's certificate: refused, the binding kept.
    let other = svc.dir.join("other.toml");
    let mut a = Access::default();
    a.set_token(&token).unwrap();
    a.bind("https://elsewhere:1", &"ab".repeat(32)).unwrap();
    a.save_to(&other).unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    sim.add_client(client_with(AssetSession { access: Some(other.clone()), ..Default::default() }, false));
    let (_, reason) = failure(&mut sim);
    assert!(reason.contains("isn't the one your token is for"), "{reason}");
    assert_eq!(Access::load_from(&other), a, "not rebound");

    // Already reading from that service, which refuses the token now
    // (revoked since): the reason.
    let bound = svc.dir.join("bound.toml");
    let mut a = Access::default();
    a.set_token(&token).unwrap();
    a.bind(&svc.front.url(), &svc.fingerprint).unwrap();
    a.save_to(&bound).unwrap();
    svc.revoke("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    let mut session = svc.session("again", &token, true);
    session.served.clear();
    session.access = Some(bound);
    sim.add_client(client_with(session, false));
    let (_, reason) = failure(&mut sim);
    assert!(reason.contains("refused you"), "{reason}");
}

/// The stand-in game as a registered game whose maps have files
/// (`maps/<name>.bsp`), so a match on its map offers the map's set.
static FIXTURE_GAME: chimera_core::game_registry::GameInfo = chimera_core::game_registry::GameInfo {
    id: GAME,
    short_name: "FG",
    name: "FG",
    content_dir: "fg",
    tick_interval: 1.0 / 64.0,
    open: |_| Err(std::io::ErrorKind::NotFound.into()),
    map_cycle: |_| Vec::new(),
    map_names: |_| Vec::new(),
    unopened: None,
    map_file: |n| Some(format!("maps/{n}.bsp")),
    steam: None,
    import_map: |_| Err("no".into()),
    default_binds_cfg: "cfg/default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: 0.0254,
    console_namespaces: &[],
};

/// The community map the server plays, which the client's install lacks.
const COMMUNITY_MAP: &str = "fixture_game:gg_community";

/// `setup` with the stand-in game registered (server and clients).
fn setup_registered(host: AssetHost) -> impl Fn(&mut App) + Send + Sync + 'static {
    let base = setup(host);
    move |app: &mut App| {
        base(app);
        app.world_mut()
            .get_resource_or_init::<chimera_core::game_registry::Games>()
            .register(&FIXTURE_GAME);
    }
}

/// A server in the middle of a match on `COMMUNITY_MAP` (its file
/// `bytes`), and a client whose install of the game lacks that map (a
/// stock install, a community map; its token bound to the service, or
/// bound to none: `bound`): the client gets the map's set from the asset
/// service, keeps the file where the map's verify step finds it, and never
/// asks the game server for it.
fn community_map_join(svc: &Fixture, token: &str, client: &str, bound: bool, bytes: &[u8]) {
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup_registered(svc.host()));
    let server_bytes = bytes.to_vec();
    {
        let w = sim.server.app.world_mut();
        w.insert_resource(chimera_world::map::LoadedMapName(COMMUNITY_MAP.into()));
        w.insert_resource(chimera_world::map::MapFile {
            name: COMMUNITY_MAP.into(),
            hash: Some(chimera_net::net::maps::sha256(bytes)),
        });
        w.insert_resource(chimera_net::net::maps::MapFiles {
            read: std::sync::Arc::new(move |id: &str| (id == COMMUNITY_MAP).then(|| server_bytes.clone())),
            cache: None,
        });
    }
    sim.step();
    let cache = svc.dir.join(format!("{client}-content"));
    let mut session = svc.session(client, token, false);
    if !bound {
        let access = svc.dir.join(format!("{client}-access.toml"));
        chimera_net::net::assets::save_token(&access, token).unwrap();
        session.settings.as_mut().unwrap().remote = None;
        session.access = Some(access);
    }
    let files = chimera_net::net::maps::MapFiles {
        // The install: the stock maps only.
        read: std::sync::Arc::new(|_: &str| None),
        cache: Some(cache.clone()),
    };
    sim.add_client(move |w: &mut World| {
        client_with(session, true)(w);
        w.insert_resource(files);
    });
    let mut from_server = false;
    let kept = cache.join("maps/gg_community.bsp");
    let done = until(&mut sim, 30.0, |s| {
        let p = progress(s, 0);
        from_server |= p.download.as_ref().is_some_and(|d| d.from == "server");
        p.failure.is_some() || matches!(p.stage, JoinStage::LoadingMap | JoinStage::Joined)
    });
    let p = progress(&sim, 0);
    assert!(done && p.failure.is_none(), "{:?}", p);
    assert!(!from_server, "the map never asked of the game server");
    assert_eq!(fs::read(&kept).ok().as_deref(), Some(bytes), "the map's file kept in the content cache");
    if !bound {
        let access = svc.dir.join(format!("{client}-access.toml"));
        assert_eq!(Access::load_from(&access).fingerprint.as_deref(), Some(svc.fingerprint.as_str()), "bound");
    }
}

/// A community map the client's install lacks (2026-11-14: "Could not
/// download maps/gg_mario_vs_wario.bsp: The server has no file to send"):
/// its set comes from the asset service the offer names, whether the
/// client's token was bound already or is bound at this join.
#[test]
fn a_community_map_the_install_lacks_comes_from_the_asset_service() {
    let mut svc = Fixture::new("community", 2, Duration::ZERO);
    let token = svc.user("tester");
    let bytes: Vec<u8> = (0..40_000u32).map(|k| (k.wrapping_mul(2_654_435_761) >> 11) as u8).collect();
    svc.republish("map/gg_community", "maps/gg_community.bsp", &bytes);
    community_map_join(&svc, &token, "bound_client", true, &bytes);
    community_map_join(&svc, &token, "fresh_client", false, &bytes);
}

/// The match's sets, all of them, are in the store when the player
/// spawns: the offer's (`base`, `players`, `common`, the ladder's gun
/// with its sound) and the client's own (`JoinSets`: music's first
/// track); a set nobody names isn't fetched.
#[test]
fn a_client_entering_a_match_has_every_needed_set_before_it_spawns() {
    let svc = Fixture::new("needed", 5, Duration::from_millis(20));
    let token = svc.user("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup_gun(svc.host()));
    let session = svc.session("client", &token, true);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    sim.add_client(move |w: &mut World| {
        client_with(session, true)(w);
        w.get_resource_or_init::<JoinSets>().0.push(first_track);
    });
    // Stepped until the player is in play (joined, its character here),
    // and no further.
    let mut rounds = 0u64;
    while !joined(&mut sim, 0) {
        rounds += 1;
        assert!(rounds < 30_000, "spawned: {:?}", progress(&sim, 0));
        sim.step();
        chimera_world::harness::let_io_run();
    }
    for set in ["base", "players", "common", "weapon/gun", "music/bgm/first.mp3"] {
        assert!(svc.ids(set).iter().all(|id| store.has(id)), "{set} here before the player spawned");
    }
    for set in ["extra", "map/later", "music/bgm/next.mp3"] {
        assert!(svc.ids(set).iter().all(|id| !store.has(id)), "{set}: nobody needs it now");
    }
}

/// The vote's winner (`minigame::NextMatch`) is fetched ahead at once, at
/// full speed, holding a stream meanwhile; a stream due in a few seconds
/// over a slow link (200 KB/s) is paced to its time and ready before it
/// is due.
#[test]
fn the_voted_map_comes_ahead_and_a_stream_is_ready_before_it_is_due() {
    let svc = Fixture::new("ahead", 2, Duration::ZERO);
    let token = svc.user("tester");
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup(svc.host()));
    let session = svc.session("client", &token, true);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    sim.add_client(client_with(session, true));
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)));
    let next = svc.ids("music/bgm/next.mp3");
    let bytes: u64 = svc.manifest.sets["music/bgm/next.mp3"].iter().map(|p| svc.manifest.files[p].stored).sum();
    assert!(bytes > 300_000, "{bytes}");
    assert!(next.iter().all(|id| !store.has(id)), "not with the join");

    // The stream: due in 8 s on a slow link (50 KB/s: the set takes
    // seconds of wall time, which its thread paces by the clock, so a
    // loaded test thread still looks while it runs).
    const LINK: u64 = 50_000;
    let world = sim.clients[0].app.world_mut();
    world.insert_resource(StreamLink(LINK));
    world.resource_mut::<Streams>().request(GAME, "music/bgm/next.mp3", 8.0);
    let asked = std::time::Instant::now();
    let mut rate = 0;
    assert!(until(&mut sim, 5.0, |s| {
        let state = assets::stream_state(s.clients[0].app.world()).unwrap();
        rate = state.rate;
        state.running.is_some()
    }));
    assert!((1..=LINK).contains(&rate), "paced to its time, at most the link: {rate}");

    // The vote locks in the next map: its fetch begins within two
    // frames, the stream held while it is under way (`run_streams` holds
    // it in the frame the fetch starts, whatever the thread's speed).
    // Looked at after every frame from the first: the local fetch may end
    // within a few milliseconds, so a look that starts later can miss the
    // hold (it failed about 1 run in 3 that way). Nothing else holds the
    // stream here, so a pause seen once the fetch began is its hold.
    assert!(!assets::stream_state(sim.clients[0].app.world()).unwrap().paused, "not held before");
    sim.clients[0].app.world_mut().insert_resource(chimera_sim::minigame::NextMatch {
        map: Some(format!("{GAME}:later")),
        at: 0.0,
    });
    let mut held = false;
    let mut look = |s: &NetSim| {
        let w = s.clients[0].app.world();
        if w.contains_resource::<MapAhead>() {
            held |= assets::stream_state(w).unwrap().paused;
        }
    };
    sim.step();
    look(&sim);
    sim.step();
    look(&sim);
    let ahead = sim.clients[0].app.world().get_resource::<MapAhead>().map(|a| a.map.clone());
    assert_eq!(ahead, Some(format!("{GAME}:later")), "the next map's fetch began");
    let later = svc.ids("map/later");
    assert!(
        until(&mut sim, 10.0, |s| {
            look(s);
            let w = s.clients[0].app.world();
            later.iter().all(|id| store.has(id)) && w.get_resource::<MapAhead>().is_some_and(|a| a.result.is_some())
        }),
        "the next map arrived ahead"
    );
    let ahead = sim.clients[0].app.world().resource::<MapAhead>();
    assert!(matches!(ahead.result, Some(Ok(_))), "{:?}", ahead.result);
    assert!(held, "the stream was held while the next map came");

    // The stream goes on and is ready before it is due.
    assert!(
        until(&mut sim, 40.0, |s| s.clients[0].app.world().resource::<Streams>().ready(GAME, "music/bgm/next.mp3")),
        "ready"
    );
    let took = asked.elapsed().as_secs_f64();
    // (Ready within the rounds above, the stream's due time in game time:
    // no wall-clock upper bound, a loaded machine would flake it.)
    // Never faster than the link: the set at 200 KB/s at best.
    assert!(took >= bytes as f64 / LINK as f64 * 0.9, "{took:.2} s for {bytes} bytes");
    assert!(next.iter().all(|id| store.has(id)));
}

/// A stream's rate: its bytes within half its time left, at least the
/// floor, at most the link.
#[test]
fn a_streams_rate_follows_its_time_left() {
    use assets::{STREAM_MIN_RATE, stream_rate};
    assert_eq!(stream_rate(1_000_000, 20.0, 4_000_000), 100_000);
    assert_eq!(stream_rate(1_000, 60.0, 4_000_000), STREAM_MIN_RATE);
    assert_eq!(stream_rate(10_000_000, 1.0, 2_000_000), 2_000_000, "due sooner than the link allows");
    assert_eq!(stream_rate(1_000_000, -5.0, 4_000_000), 2_000_000, "overdue: as fast as it may");
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

    // The next start: the newer manifest, then its `base` in the background.
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

/// A game whose `base` comes with a match (`sets::startup_sets`: Halo's)
/// is not in the startup plan, whether or not the store has it; the
/// other games' plan is what it was.
#[test]
fn a_game_whose_base_comes_with_a_match_is_not_a_startup_download() {
    let svc = Fixture::new("startup_halo", 300, Duration::from_millis(100));
    let token = svc.user("tester");
    let settings = startup_settings(&svc, "client", &token, svc.front.url(), &svc.fingerprint);
    let halo = AppGame { game: "halo_ce".into(), installed: false };
    assert!(asset_startup::plan(&settings, &[halo.clone()]).is_none(), "nothing to fetch for Halo alone");
    let ours = AppGame { game: GAME.into(), installed: false };
    let wanted = asset_startup::plan(&settings, &[halo, ours]).expect("the other game is planned");
    assert_eq!(wanted.len(), 1, "{wanted:?}");
    assert_eq!((wanted[0].game.as_str(), wanted[0].sets.as_slice()), (GAME, ["base".to_string()].as_slice()));
}

/// The startup fetch: planned only for a game that
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
        "all here: nothing next time"
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
    assert!(why.contains("refused you") && why.contains("chimera:asset_token"), "{why}");
    let why = fresh("other_cert", &token, svc.front.url(), &"ab".repeat(32));
    assert!(why.contains("another certificate"), "{why}");
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let why = fresh("unreachable", &token, format!("https://{closed}"), &svc.fingerprint);
    assert!(why.contains("can't reach the asset service"), "{why}");
}

// --- The server's own game files (`net::server_assets`).

/// An account file signed in to `svc`'s hub as `name`, its ticket made
/// with a test key the service trusts (`ticket.pub`), as the accounts
/// service would make it.
/// An account and a device no other test's account has: the process's
/// signed-in session (`account::set_live`, made by the last account file
/// a test opened) stands in its newer ticket for any ticket of its device
/// (`account::fresh_ticket`), so two tests with the same device in one
/// process (`cargo test`'s threads) sent each other's tickets, signed
/// with the other service's key, and were refused when they overlapped
/// (the more often, the more loaded the machine).
fn account_ids() -> ([u8; 16], [u8; 16]) {
    static NEXT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (mut account, mut device) = ([0xa0; 16], [0xd0; 16]);
    account[15] = n;
    device[15] = n;
    (account, device)
}

fn hub_account(svc: &Fixture, name: &str) -> PathBuf {
    use chimera_assets::{account::AccountFile, ticket};
    let key = chimera_assets::sign::load_or_create_key(&svc.dir.join("ticket.key")).unwrap();
    let keys_path = chimera_assets::accounts::ticket_keys_path(&svc.store.root);
    fs::create_dir_all(keys_path.parent().unwrap()).unwrap();
    fs::write(&keys_path, ticket::TicketKeys::single(key.verifying_key()).to_text()).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let (account, device) = account_ids();
    let claims = ticket::Claims {
        version: ticket::VERSION,
        key_id: String::new(),
        account,
        device,
        username: name.into(),
        role: "host".into(),
        content: vec!["*".into()],
        games: vec!["*".into()],
        list: 4,
        report: true,
        muted: false,
        audience: ticket::SERVICES.into(),
        issued: now,
        expires: now + ticket::LIFETIME_SECS,
        display_name: name.into(),
    };
    let file = AccountFile {
        url: svc.front.url(),
        fingerprint: Some(svc.fingerprint.clone()),
        assets_url: None,
        account: ticket::id_hex(&account),
        device: ticket::id_hex(&device),
        username: name.into(),
        display_name: name.into(),
        device_key: "05".repeat(32),
        ticket: Some(ticket::issue(&key, &claims)),
    };
    let path = svc.dir.join(format!("{name}-account.toml"));
    file.save_to(&path).unwrap();
    path
}

/// The stand-in game's gun as a document whose numbers are `text`'s.
fn docs_from(text: &str) -> Vec<(String, String)> {
    vec![("first.toml".to_string(), text.to_string())]
}

/// A dedicated server with no install of the game, an empty store and
/// only a token of its own (no service named: it goes to the service the
/// server offers its players, pinned by its certificate) fetches, before
/// it starts, the game's `base` and the start map's sets, then builds its
/// server documents from the fetched files (read with nothing to fetch
/// from: they are all here). A client whose own numbers differ (a
/// machine without the files, the specs' fallback) plays with the
/// server's. Before the match, the server also fetches the sets the
/// match hands out (the gun's `weapon/gun`), never the others.
#[test]
fn a_dedicated_server_without_installs_fetches_its_files_with_its_credential_and_builds_its_documents_from_them() {
    use chimera_assets::CachedFiles;
    use chimera_core::mount::FileSource;
    use chimera_net::net::server_assets::{self, Via};
    use chimera_sim::{server_docs::{DocsAppExt, ServerDocs}, weapon::data::{WeaponCatalog, WeaponDataAppExt}};
    let mut svc = Fixture::new("server_files", 3, Duration::ZERO);
    // The game's data the server builds its documents from (invented
    // numbers), in its `base`, as TF2's scripts are.
    let real = format!("id = \"{GAME}:first\"\nslot = 1\ndraw = 0.25\n");
    svc.republish("base", "scripts/first.toml", real.as_bytes());
    let token = svc.user("game-server");
    let mut session = svc.session("dedicated", "", true);
    session.settings.as_mut().unwrap().remote = None;
    session.access = Some(svc.dir.join("dedicated-none.toml"));
    session.account = Some(svc.dir.join("dedicated-no-account.toml"));
    let files = server_assets::from_session(&session, &svc.host(), Some(&token)).expect("a credential");
    assert_eq!(files.via, Via::Offered);
    let remote = files.settings.remote.clone().unwrap();
    assert_eq!((remote.url.as_str(), remote.fingerprint.as_deref()), (svc.front.url().as_str(), Some(svc.fingerprint.as_str())));
    let files = server_assets::fetch_at_start(files, &format!("{GAME}:later"));
    let store = files.settings.store.clone().unwrap();
    for set in ["base", "players", "common", "map/later"] {
        assert!(svc.ids(set).iter().all(|id| store.has(id)), "{set} fetched before the start");
    }
    assert!(svc.ids("weapon/gun").iter().all(|id| !store.has(id)), "no gun yet");
    assert!(svc.ids("extra").iter().all(|id| !store.has(id)), "nothing the server doesn't need");
    // The server's documents from the store alone (no fetcher: no
    // download while it plays).
    let cached = CachedFiles::open(&store, GAME, "7").unwrap();
    let text = String::from_utf8(cached.read("scripts/first.toml").unwrap().unwrap()).unwrap();
    assert_eq!(text, real);
    let client_docs = docs_from(&format!("id = \"{GAME}:first\"\nslot = 1\ndraw = 1.0\n"));
    let base = setup_gun(svc.host());
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, move |app: &mut App| {
        base(app);
        let docs = &client_docs;
        let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let set = app.add_weapon_data(&refs).unwrap();
        app.server_weapon_docs(WEAPONS_SET, set, docs.clone());
    });
    // Every app starts with the numbers it has without the files; the
    // server's parts build its documents from the fetched file (as TF2's
    // guns do at the server's build, after the start's download).
    {
        use chimera_sim::{server_docs::{self, DocApply}, weapon::data::WeaponSet};
        let docs = docs_from(&text);
        let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let set: &'static WeaponSet = Box::leak(Box::new(WeaponSet::parse(&refs, &[]).unwrap()));
        server_docs::renew_own(sim.server.app.world_mut(), WEAPONS_SET, docs, DocApply::Weapons(set)).unwrap();
    }
    sim.server.app.insert_resource(files);
    let draw = |w: &World| w.resource::<WeaponCatalog>().get(&format!("{GAME}:first")).unwrap().draw;
    assert_eq!(draw(sim.server.app.world()), 0.25, "the server's numbers are the fetched file's");
    // A client with the install (nothing to fetch) and other numbers.
    sim.add_client(client_with(svc.session("player", &token, false), true));
    assert_eq!(draw(sim.clients[0].app.world()), 1.0);
    assert!(until(&mut sim, 30.0, |s| joined(s, 0)), "joined: {:?}", progress(&sim, 0));
    assert!(sim.clients[0].app.world().resource::<ServerDocs>().get(WEAPONS_SET).unwrap().replaced(), "the server's documents taken");
    assert_eq!(draw(sim.clients[0].app.world()), 0.25, "the client plays with the server's numbers");
    // The match hands out the gun: the server fetched its set too.
    assert!(
        until(&mut sim, 30.0, |s| !server_assets::fetching(s.server.app.world()) && svc.ids("weapon/gun").iter().all(|id| store.has(id))),
        "the match's gun fetched by the server"
    );
    assert!(svc.ids("extra").iter().all(|id| !store.has(id)));
    let log = fs::read_to_string(svc.store.root.join("server").join("access.log")).unwrap();
    assert!(log.lines().any(|l| l.contains("game-server") && l.contains(" 200 ")), "{log}");
}

/// A listen server is the host's process: with no service configured and
/// no token, it fetches the match's sets with the host's own credential
/// (here its hub account's ticket, as its client joins with it), the
/// service logging the account; a change of level fetches the coming
/// map's set before the map loads.
#[test]
fn a_listen_server_fetches_the_match_files_with_the_hosts_credential() {
    use chimera_net::net::server_assets::{self, ServerFiles, Via};
    let svc = Fixture::with_tickets("listen_files", 3);
    let account = hub_account(&svc, "hosty");
    // The host's session as its client made it: its store, no service
    // named, its account.
    let mut session = svc.session("host", "", true);
    session.settings.as_mut().unwrap().remote = None;
    session.access = Some(svc.dir.join("host-access.toml"));
    session.account = Some(account);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup_gun(svc.host()));
    sim.server.app.insert_resource(session);
    server_assets::for_host(sim.server.app.world_mut());
    let files = sim.server.app.world().resource::<ServerFiles>().clone();
    assert_eq!(files.via, Via::Account);
    let token = &files.settings.remote.as_ref().unwrap().token;
    assert!(token.starts_with(chimera_assets::ticket::PREFIX), "the host's ticket, not a token");
    assert!(
        until(&mut sim, 30.0, |s| {
            !server_assets::fetching(s.server.app.world())
                && ["base", "players", "common", "weapon/gun"].iter().all(|set| svc.ids(set).iter().all(|id| store.has(id)))
        }),
        "the match's sets fetched by the host"
    );
    assert!(svc.ids("map/later").iter().all(|id| !store.has(id)), "not the next map yet");
    // A change of level names the coming map before it loads (the served
    // one is still the old): its set comes with the change's fetch.
    sim.server.app.insert_resource(chimera_world::map::LoadedMapName(format!("{GAME}:later")));
    let job = server_assets::fetch_now(sim.server.app.world_mut()).expect("the next map's fetch");
    job.wait().unwrap();
    assert!(svc.ids("map/later").iter().all(|id| store.has(id)), "the coming map's set");
    assert!(svc.ids("extra").iter().all(|id| !store.has(id)));
    let log = fs::read_to_string(svc.store.root.join("server").join("access.log")).unwrap();
    assert!(log.lines().any(|l| l.contains("account:hosty") && l.contains(" 200 ")), "{log}");
}

/// A listen host whose match's sets are all in its store looks at the
/// store once for them, not every second: a look reads every served
/// game's manifest and each record of the sets (up to a second with five
/// games' manifests), and done on the main thread every second it
/// froze the host's frames about once a second (docs/performance.md,
/// "Stutter on Beaver Creek"). The look runs on the job's thread; the
/// same sets aren't looked at again until they change, nor are the
/// match's sets named again while nothing they are made of changes
/// (`server_assets::MatchInputs`; "The sandbox's jitter").
#[test]
fn a_server_looks_at_its_store_once_per_change_of_the_matchs_sets() {
    use chimera_net::net::server_assets::{self, ServerFiles};
    let svc = Fixture::with_tickets("listen_looks", 3);
    let account = hub_account(&svc, "looker");
    let mut session = svc.session("host", "", true);
    session.settings.as_mut().unwrap().remote = None;
    session.access = Some(svc.dir.join("host-access.toml"));
    session.account = Some(account);
    let store = session.settings.as_ref().unwrap().store.clone().unwrap();
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 7, 0, setup_gun(svc.host()));
    sim.server.app.insert_resource(session);
    server_assets::for_host(sim.server.app.world_mut());
    assert!(sim.server.app.world().contains_resource::<ServerFiles>());
    // The first frame looks at the match's sets and starts the job; wait
    // on the job itself (its thread can lag any count of frames under
    // load), then a frame takes its end.
    sim.step();
    let job = sim.server.app.world().get_resource::<server_assets::ServerFetching>().expect("the first frame's look").0.clone();
    job.wait().unwrap();
    sim.step();
    assert!(!server_assets::fetching(sim.server.app.world()), "the job's end taken");
    assert!(
        ["base", "players", "common", "weapon/gun"].iter().all(|set| svc.ids(set).iter().all(|id| store.has(id))),
        "the match's sets fetched by the host"
    );
    let looks = server_assets::store_looks(sim.server.app.world());
    assert_eq!(looks, 1, "one look fetched them");
    // Every look from now on names the same sets: none looks at the store
    // or starts a job.
    for _ in 0..5 {
        assert!(server_assets::fetch_now(sim.server.app.world_mut()).is_none(), "no job for the same sets");
    }
    let named = server_assets::match_looks(sim.server.app.world());
    for _ in 0..200 {
        sim.step();
    }
    assert_eq!(server_assets::store_looks(sim.server.app.world()), looks, "the same sets looked at again");
    assert_eq!(server_assets::match_looks(sim.server.app.world()), named, "the match's sets named again with nothing changed");
    assert!(!server_assets::fetching(sim.server.app.world()));
    // A change of level names other sets: one look, on the job's thread,
    // which fetches the coming map's.
    sim.server.app.insert_resource(chimera_world::map::LoadedMapName(format!("{GAME}:later")));
    let job = server_assets::fetch_now(sim.server.app.world_mut()).expect("a look for the new sets");
    assert_eq!(server_assets::store_looks(sim.server.app.world()), looks + 1);
    job.wait().unwrap();
    assert!(svc.ids("map/later").iter().all(|id| store.has(id)), "the coming map's set");
    // Looked at with everything there: done, nothing fetched.
    sim.server.app.insert_resource(chimera_world::map::LoadedMapName(chimera_net::net::GREYBOX.to_string()));
    let job = server_assets::fetch_now(sim.server.app.world_mut()).expect("a look for the first map's sets again");
    assert_eq!(job.wait(), Ok(String::new()), "everything was in the store");
}
