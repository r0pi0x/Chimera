//! Converted entries served by the asset service on localhost
//! (docs/plans/active/converted-cache.md, step 7; `map_cache::served`): a
//! server store holds a game's original files; the packer converts them
//! from the store's own records and enters the entry with its recipe; a
//! client of the service (its own store and cache, nothing local) takes
//! the entry: it downloads the recipe and the entry, never the originals,
//! converts nothing, and gets the same value. A client of another
//! converter version converts from the originals, which it then fetches.

use std::{fs, sync::Mutex};

use chimera_assets::{
    AssetSettings, CachedFiles, Manifest, RemoteConfig, Store,
    accounts::Accounts,
    map_cache::{Key, MapCache, Outcome, Served, cached_served, served},
    pack,
    server::{self, Service},
};
use chimera_vfs::{LooseDir, Mount};

use super::scratch;

/// The "conversion": both files read, joined.
fn convert(mount: &Mount, runs: &Mutex<u32>) -> Result<String, String> {
    *runs.lock().unwrap() += 1;
    let a = mount.read("models/a.mdl").map_err(|e| e.to_string())?;
    let b = mount.read("materials/b.vtf").map_err(|e| e.to_string())?;
    Ok(format!("{} bytes and {} bytes, first {}", a.len(), b.len(), a[0]))
}

fn key() -> Key {
    Key::new("guest:fixture", "v1-test").part("textures", [1u8])
}

fn served_as(converter: &str) -> Served<'static> {
    Served { name: format!("guest/fixture/{converter}"), parts: vec![("converter".into(), converter.into())], export: None }
}

#[test]
fn a_client_downloads_the_converted_entry_instead_of_the_originals() {
    let dir = scratch("converted_served");
    // The originals, packed into the server's store (no sets: each file
    // fetched alone).
    fs::create_dir_all(dir.join("install/models")).unwrap();
    fs::create_dir_all(dir.join("install/materials")).unwrap();
    fs::write(dir.join("install/models/a.mdl"), vec![7u8; 50_000]).unwrap();
    fs::write(dir.join("install/materials/b.vtf"), vec![9u8; 80_000]).unwrap();
    let mut originals = Mount::default();
    originals.push(LooseDir::new(dir.join("install")));
    let server_store = Store::new(dir.join("server"));
    let mut manifest = Manifest::new("fixture", "100", None);
    pack(&originals, &["models/a.mdl".into(), "materials/b.vtf".into()], &server_store, &mut manifest, None, 1).unwrap();
    // The packer: the store alone, as a client mounts it.
    let sink = Mutex::new(Vec::new());
    let packer = {
        let mut m = Mount::default();
        m.push(CachedFiles::new(server_store.clone(), &manifest));
        m
    };
    let runs = Mutex::new(0);
    let export = Served { export: Some(&sink), ..served_as("v1-test") };
    let packer_cache = MapCache::at(dir.join("packer-cache"), u64::MAX);
    let (made, _) = cached_served(Some(&packer_cache), &packer, key(), Some(&export), || convert(&packer, &runs)).unwrap();
    let entries = sink.into_inner().unwrap();
    for e in &entries {
        served::enter(&server_store, &mut manifest, e).unwrap();
    }
    manifest.save(&server_store.manifest_path("fixture", "100")).unwrap();
    // The service.
    let running = server::serve(Service::new(server_store.clone()).unwrap(), "127.0.0.1:0", 2).unwrap();
    let path = Accounts::path(&server_store.root);
    let mut accounts = Accounts::load(&path).unwrap();
    let (_, token) = accounts.add("tester", None, "test").unwrap();
    accounts.save(&path).unwrap();
    let client = |name: &str| {
        let store = Store::new(dir.join(name));
        let settings = AssetSettings {
            store: Some(store.clone()),
            use_install: false,
            service_first: false,
            builds: Default::default(),
            remote: Some(RemoteConfig { url: running.url(), token: token.clone(), key: None, fingerprint: None }),
            lock: None,
        };
        assert_eq!(settings.build("fixture").as_deref(), Some("100"));
        let mut mount = Mount::default();
        mount.push(CachedFiles::open(&store, "fixture", "100").unwrap().with_fetcher(settings.fetcher()));
        (store, mount, MapCache::at(dir.join(format!("{name}-cache")), u64::MAX))
    };
    let (store, mount, cache) = client("client");
    let runs = Mutex::new(0);
    let (got, how) = cached_served(Some(&cache), &mount, key(), Some(&served_as("v1-test")), || convert(&mount, &runs)).unwrap();
    assert_eq!((how, *runs.lock().unwrap()), (Outcome::Served, 0), "taken, nothing converted");
    assert_eq!(got, made, "the same value");
    for e in &entries {
        assert!(store.has(&manifest.files[&e.entry_path].id), "the entry downloaded");
        assert!(store.has(&manifest.files[&e.recipe_path].id), "its recipe downloaded");
    }
    for original in ["models/a.mdl", "materials/b.vtf"] {
        assert!(!store.has(&manifest.files[original].id), "{original}: never downloaded");
    }
    let (_, how) = cached_served(Some(&cache), &mount, key(), Some(&served_as("v1-test")), || convert(&mount, &runs)).unwrap();
    assert_eq!(how, Outcome::Hit, "kept locally after");
    // Another converter version converts, fetching the originals.
    let (store, mount, cache) = client("client-v2");
    let runs = Mutex::new(0);
    let (got, how) = cached_served(Some(&cache), &mount, key(), Some(&served_as("v2-test")), || convert(&mount, &runs)).unwrap();
    assert_eq!((how, *runs.lock().unwrap(), got), (Outcome::Miss, 1, made));
    assert!(store.has(&manifest.files["models/a.mdl"].id), "the originals fetched");
    drop(running);
    let _ = fs::remove_dir_all(&dir);
}
