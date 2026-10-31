//! The asset service on localhost (docs/plans/active/asset-service.md, M2):
//! a server store packed from our own fixture bytes, served on
//! 127.0.0.1 (a free port), asked by the client side (`Remote`,
//! `AssetSettings`, `CachedFiles` with a fetcher). Tokens: a valid one gets
//! manifests and records, none or a revoked one 401, another game's files
//! 403; the client refuses a tampered record, a manifest signed by
//! another key or not the locked one, and plain HTTP off this machine.

use std::{fs, io::ErrorKind, path::Path, time::Duration};

use chimera_assets::{
    AssetSettings, AssetsLock, CachedFiles, Manifest, Remote, RemoteConfig, Store,
    accounts::Accounts,
    lock::Pin,
    pack,
    server::{self, Running, Service},
    sign,
    store::sha256_hex,
};
use chimera_vfs::{FileSource, LooseDir, Mount};

use super::{all_paths, scratch, zstd_of};

/// Files of game `fixture` (build 100) and game `other` (build 1); one
/// file is the same bytes in both (one record).
fn write_games(root: &Path) -> (Mount, Mount) {
    let write = |rel: &str, bytes: &[u8]| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    };
    write("fixture/maps/test.bsp", &[3u8; 70_000]);
    write("fixture/sound/a.wav", b"fixture sound");
    write("fixture/shared.txt", b"bytes both games have");
    write("other/models/b.mdl", b"only the other game has this");
    write("other/shared.txt", b"bytes both games have");
    let mount = |dir: &str| {
        let mut m = Mount::default();
        m.push(LooseDir::new(root.join(dir)));
        m
    };
    (mount("fixture"), mount("other"))
}

fn publish(store: &Store, game: &str, build: &str, mount: &Mount) -> Manifest {
    let mut m = Manifest::new(game, build, None);
    pack(mount, &all_paths(mount), store, &mut m, Some("all"), 2).unwrap();
    m.save(&store.manifest_path(game, build)).unwrap();
    m
}

struct Fixture {
    dir: std::path::PathBuf,
    server_store: Store,
    running: Running,
    fixture: Manifest,
    other: Manifest,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = scratch(name);
        let (a, b) = write_games(&dir.join("install"));
        let server_store = Store::new(dir.join("server"));
        let fixture = publish(&server_store, "fixture", "100", &a);
        let other = publish(&server_store, "other", "1", &b);
        let service = Service::new(server_store.clone()).unwrap();
        let running = server::serve(service, "127.0.0.1:0", 2).unwrap();
        Self { dir, server_store, running, fixture, other }
    }

    /// A new token for `name` (`games`: None = all).
    fn user(&self, name: &str, games: Option<&[&str]>) -> String {
        let path = Accounts::path(&self.server_store.root);
        let mut accounts = Accounts::load(&path).unwrap();
        let (_, token) = accounts.add(name, games.map(|g| g.iter().map(|s| s.to_string()).collect()), "test").unwrap();
        accounts.save(&path).unwrap();
        token
    }

    fn remote(&self, token: &str) -> Remote {
        Remote::new(self.config(token)).unwrap()
    }

    fn config(&self, token: &str) -> RemoteConfig {
        RemoteConfig { url: self.running.url(), token: token.into(), key: None }
    }

    fn client_store(&self, name: &str) -> Store {
        Store::new(self.dir.join(name))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_valid_token_gets_signed_manifests_and_records() {
    let f = Fixture::new("svc_valid");
    let token = f.user("tester", None);
    let remote = f.remote(&token);
    assert!(remote.whoami().unwrap().starts_with("internal tester 1\ngames *"));
    let games = remote.games().unwrap();
    assert_eq!(
        games.iter().map(|g| (g.0.as_str(), g.1.as_str())).collect::<Vec<_>>(),
        [("fixture", "100"), ("other", "1")]
    );
    // The manifest: signed with the server's key, saved with its signature.
    let client = f.client_store("client");
    let manifest = remote.fetch_manifest(&client, "fixture", "100", None).unwrap();
    assert_eq!(manifest, f.fixture);
    let saved = client.manifest_path("fixture", "100");
    let key = remote.server_key().unwrap();
    assert_eq!(key, sign::public_hex(&sign::load_or_create_key(&server::key_path(&f.server_store)).unwrap()));
    sign::verify(&key, &fs::read(&saved).unwrap(), &fs::read_to_string(sign::sig_path(&saved)).unwrap()).unwrap();
    // One record, as stored.
    let id = &f.fixture.files["maps/test.bsp"].id;
    assert_eq!(remote.record(id).unwrap(), fs::read(f.server_store.record_path(id)).unwrap());
    // A batch fills the client's store.
    let wanted: Vec<(String, u64)> = f.fixture.files.values().map(|r| (r.id.clone(), r.size)).collect();
    let (n, bytes, failed) = remote.fill(&client, &wanted);
    assert!(failed.is_empty(), "{failed:?}");
    assert_eq!(n, 3);
    assert!(bytes > 0);
    assert_eq!(remote.fill(&client, &wanted).0, 0, "nothing again");
    // The access log names the account and what it got.
    let log = fs::read_to_string(f.server_store.root.join("server/access.log")).unwrap();
    assert!(log.contains(&format!("internal:tester#1 GET /r/{id} 200 records=1 bytes=")), "{log}");
    assert!(log.contains("internal:tester#1 POST /batch 200 records=3"), "{log}");
}

#[test]
fn files_arrive_on_demand_through_the_settings_a_game_uses() {
    let f = Fixture::new("svc_on_demand");
    let token = f.user("tester", None);
    let client = f.client_store("client");
    let settings = AssetSettings {
        store: Some(client.clone()),
        use_install: false,
        builds: Default::default(),
        remote: Some(f.config(&token)),
        lock: None,
    };
    // No manifest in the store: the service's newest build is fetched.
    assert_eq!(settings.build("fixture").as_deref(), Some("100"));
    assert!(client.manifest_path("fixture", "100").is_file());
    let cached = CachedFiles::open(&client, "fixture", "100").unwrap().with_fetcher(settings.fetcher());
    assert_eq!(cached.read("maps/test.bsp").unwrap().unwrap(), vec![3u8; 70_000]);
    assert_eq!(cached.read("shared.txt").unwrap().unwrap(), b"bytes both games have");
    assert!(client.has(&f.fixture.files["maps/test.bsp"].id), "kept in the store");
    // Without the service the same mount misses.
    let offline = CachedFiles::open(&client, "fixture", "100").unwrap();
    assert_eq!(offline.read("sound/a.wav").unwrap().unwrap_err().kind(), ErrorKind::NotFound);
    // A locked build is fetched only with the locked manifest.
    let mut lock = AssetsLock::default();
    lock.games.insert("other".into(), Pin { build: "1".into(), manifest: sha256_hex(b"another manifest") });
    let locked = AssetSettings { lock: Some(lock), ..settings.clone() };
    assert_eq!(locked.build("other"), None, "not the manifest assets.lock pins");
    assert!(!client.manifest_path("other", "1").is_file());
    let id = sha256_hex(f.other.to_text().as_bytes());
    let mut lock = AssetsLock::default();
    lock.games.insert("other".into(), Pin { build: "1".into(), manifest: id });
    let other_client = f.client_store("client2");
    let locked = AssetSettings { lock: Some(lock), store: Some(other_client), ..settings };
    assert_eq!(locked.build("other").as_deref(), Some("1"));
}

#[test]
fn a_missing_unknown_or_revoked_token_is_401() {
    let f = Fixture::new("svc_401");
    let token = f.user("tester", None);
    let id = f.fixture.files["sound/a.wav"].id.clone();
    let status = |token: &str| {
        let e = f.remote(token).record(&id).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::PermissionDenied, "{e}");
        e.to_string()
    };
    assert!(status("").contains("HTTP 401"));
    assert!(status("chimera1_0000").contains("HTTP 401"));
    assert!(status("someone else's kind of token").contains("HTTP 401"));
    // The public key needs no token.
    assert!(f.remote("").server_key().is_ok());
    // Revoked while the server runs: refused at the next request.
    let remote = f.remote(&token);
    remote.record(&id).unwrap();
    let path = Accounts::path(&f.server_store.root);
    let mut accounts = Accounts::load(&path).unwrap();
    assert_eq!(accounts.revoke("tester", None).unwrap(), 1);
    accounts.save(&path).unwrap();
    let e = remote.record(&id).unwrap_err();
    assert!(e.to_string().contains("HTTP 401") && e.to_string().contains("revoked"), "{e}");
    assert!(remote.whoami().is_err());
    // Another device's token of the same account works until revoked.
    let second = f.user("tester", None);
    f.remote(&second).record(&id).unwrap();
}

#[test]
fn another_games_files_are_403() {
    let f = Fixture::new("svc_403");
    let token = f.user("fixture_only", Some(&["fixture"]));
    let remote = f.remote(&token);
    let client = f.client_store("client");
    assert_eq!(remote.games().unwrap().iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["fixture"]);
    let e = remote.fetch_manifest(&client, "other", "1", None).unwrap_err();
    assert!(e.contains("HTTP 403"), "{e}");
    let only_other = &f.other.files["models/b.mdl"].id;
    let e = remote.record(only_other).unwrap_err();
    assert!(e.kind() == ErrorKind::PermissionDenied && e.to_string().contains("HTTP 403"), "{e}");
    // A record both games refer to is the account's game's too.
    remote.record(&f.other.files["shared.txt"].id).unwrap();
    let batch = remote
        .batch(&[only_other.clone(), f.fixture.files["sound/a.wav"].id.clone(), sha256_hex(b"nobody's")])
        .unwrap();
    assert_eq!(batch.iter().map(|b| b.1).collect::<Vec<_>>(), [403, 200, 404]);
    assert!(batch[0].2.is_empty());
}

#[test]
fn the_client_refuses_a_tampered_record_or_manifest() {
    let f = Fixture::new("svc_tamper");
    let token = f.user("tester", None);
    let remote = f.remote(&token);
    let client = f.client_store("client");
    remote.fetch_manifest(&client, "fixture", "100", None).unwrap();
    // A record changed on the server (or on the way): refused, not kept.
    let id = f.fixture.files["sound/a.wav"].id.clone();
    fs::write(f.server_store.record_path(&id), zstd_of(b"fixture sounD")).unwrap();
    let settings = AssetSettings {
        store: Some(client.clone()),
        use_install: false,
        builds: Default::default(),
        remote: Some(f.config(&token)),
        lock: None,
    };
    let cached = CachedFiles::open(&client, "fixture", "100").unwrap().with_fetcher(settings.fetcher());
    let e = cached.read("sound/a.wav").unwrap().unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidData, "{e}");
    assert!(!client.has(&id), "a tampered record is never stored");
    let (_, _, failed) = remote.fill(&client, &[(id.clone(), 13)]);
    assert_eq!(failed.len(), 1, "{failed:?}");
    // A manifest signed by another key than the pinned one.
    let other_key = sign::public_hex(&sign::load_or_create_key(&f.dir.join("other.key")).unwrap());
    let pinned = Remote::new(RemoteConfig { key: Some(other_key), ..f.config(&token) }).unwrap();
    let e = pinned.fetch_manifest(&f.client_store("client2"), "fixture", "100", None).unwrap_err();
    assert!(e.contains("bad signature"), "{e}");
    // Not the manifest assets.lock pins.
    let e = remote.fetch_manifest(&f.client_store("client3"), "fixture", "100", Some(&sha256_hex(b"x"))).unwrap_err();
    assert!(e.contains("assets.lock pins"), "{e}");
}

#[test]
fn tokens_go_over_https_only_off_this_machine() {
    let config = |url: &str| RemoteConfig { url: url.into(), token: "t".into(), key: None };
    for url in ["https://assets.example.ts.net", "http://127.0.0.1:8731", "http://localhost:1/", "http://[::1]:5"] {
        assert!(config(url).base().is_ok(), "{url}");
    }
    for url in ["http://assets.example.ts.net", "http://127.0.0.1.example.com", "http://user@127.0.0.1.x", "ftp://x"] {
        assert!(config(url).base().is_err(), "{url}");
    }
    assert!(!format!("{:?}", config("https://x")).contains("\"t\""), "Debug never shows the token");
}

#[test]
fn a_publish_is_served_without_a_restart() {
    let f = Fixture::new("svc_publish");
    let token = f.user("tester", None);
    let remote = f.remote(&token);
    assert_eq!(remote.latest_build("fixture").unwrap().as_deref(), Some("100"));
    let mut m = f.fixture.clone();
    m.build = "101".into();
    m.save(&f.server_store.manifest_path("fixture", "101")).unwrap();
    // The server checks its manifests at most once a second.
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(remote.latest_build("fixture").unwrap().as_deref(), Some("101"));
    // Signed when the server read it (no `.sig` was written with it).
    assert!(sign::sig_path(&f.server_store.manifest_path("fixture", "101")).is_file());
    remote.fetch_manifest(&f.client_store("client"), "fixture", "101", None).unwrap();
}

#[test]
fn accounts_and_the_lock_round_trip() {
    let dir = scratch("svc_files");
    let path = dir.join("accounts.toml");
    let mut accounts = Accounts::default();
    let (one, token) = accounts.add("a.b-c_d", Some(vec!["cs_source".into()]), "laptop").unwrap();
    let (two, _) = accounts.add("a.b-c_d", None, "desktop").unwrap();
    assert_eq!((one, two), (1, 2));
    assert!(accounts.add("not ok", None, "").is_err());
    accounts.save(&path).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains(&token), "only the token's hash is kept");
    assert!(text.contains(&sha256_hex(token.as_bytes())));
    assert_eq!(Accounts::load(&path).unwrap(), accounts);
    assert_eq!(accounts.revoke("a.b-c_d", Some(2)).unwrap(), 1);
    assert!(accounts.revoke("a.b-c_d", Some(9)).is_err());
    assert!(accounts.revoke("nobody", None).is_err());

    let mut lock = AssetsLock::default();
    lock.games.insert("cs_source".into(), Pin { build: "25338318".into(), manifest: sha256_hex(b"m") });
    let lock_path = dir.join("assets.lock");
    lock.save(&lock_path).unwrap();
    assert_eq!(AssetsLock::load(&lock_path).unwrap(), lock);
    assert!(fs::read_to_string(&lock_path).unwrap().contains("[cs_source]\nbuild = \"25338318\""));
    assert!(AssetsLock::parse("[cs_source]\nbuild = \"../x\"\nmanifest = \"00\"\n").is_err());
    let _ = fs::remove_dir_all(&dir);
}
