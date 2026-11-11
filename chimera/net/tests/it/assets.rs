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
        assets::{AssetHost, AssetRelaunch, AssetSession, FILES},
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
}
