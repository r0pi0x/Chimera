//! WAREHOUSE from the asset store alone (docs/plans/active/asset-service.md,
//! M1): a load from the install is recorded (`mount::served::record`), the
//! files it read are packed into a scratch store, decrypted
//! (`chimera_assets::pack`; the keys stay in this process), and the map
//! loads again from a mount of only that store's records: the same map, no
//! file missing, no key needed. Skipped without a Combat Arms install and
//! its keys. The store is a scratch folder, removed after.
//!
//! M2: the same store served by the asset service on 127.0.0.1, and the
//! map loaded through the mount a game uses (`served::open_with`) with no
//! install, no keys, an empty local store and the service set: the
//! manifest and every record arrive on demand, checked, and the map is
//! the same.

use std::{
    fmt::Write as _,
    hash::{DefaultHasher, Hasher},
    time::Instant,
};

use chimera_core::mount::{
    Mount,
    assets::{AssetSettings, CachedFiles, Manifest, Recorder, RemoteConfig, Store, accounts::Accounts, pack, server},
    config::LocalConfig,
    install, served,
};
use chimera_world::map::MapData;
use combat_arms::game::{GAME, mount, world};

/// `Debug` text fed to a hasher, so a whole map compares without holding
/// its text.
struct HashWriter(DefaultHasher);

impl std::fmt::Write for HashWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// What must match between two loads: every mesh and texture (texels
/// included), the triangle count, the kit's characters.
fn summary(map: &MapData) -> (usize, usize, usize, usize, u64) {
    let mut h = HashWriter(DefaultHasher::new());
    write!(h, "{:?}{:?}", map.meshes, map.textures).unwrap();
    (map.triangle_count(), map.meshes.len(), map.textures.len(), map.kit.characters.len(), h.0.finish())
}

#[test]
fn warehouse_loads_from_the_store_alone() {
    let Some(found) = LocalConfig::load().ok().and_then(|c| install::resolve(&c, GAME, None).install) else {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    };
    let Ok(installed) = mount::open_install(&found.path) else {
        eprintln!("skipping: no Combat Arms install at {}", found.path.display());
        return;
    };
    let recorder = Recorder::in_memory();
    let recorded = served::record(installed, GAME, &recorder);
    let started = Instant::now();
    let map = world::load_from(&recorded, "warehouse").unwrap();
    let install_took = started.elapsed();
    let from_install = summary(&map);
    drop(map);
    let paths: Vec<String> = recorder.lines().into_iter().map(|l| l.path).collect();
    let read: u64 = recorder.lines().iter().map(|l| l.size).sum();

    let dir = std::env::temp_dir().join(format!("chimera_served_warehouse_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(&dir);
    let mut manifest = Manifest::new(GAME, "test", None);
    let report = pack(&recorded, &paths, &store, &mut manifest, Some("warehouse"), 4).unwrap();
    manifest.save(&store.manifest_path(GAME, "test")).unwrap();
    assert!(report.missing.is_empty(), "{:?}", report.missing);

    let mut from_store = Mount::default();
    from_store.push(CachedFiles::open(&store, GAME, "test").unwrap());
    let started = Instant::now();
    let map = world::load_from(&from_store, "warehouse").unwrap();
    let store_took = started.elapsed();
    let served_map = summary(&map);
    eprintln!(
        "warehouse: {} files, {:.1} MB read, {} records, {:.1} MB stored, packed in {:.2} s; \
         load {:.2} s from the install (first in the process), {:.2} s from the store",
        report.files,
        read as f64 / 1e6,
        report.records,
        report.stored as f64 / 1e6,
        report.took.as_secs_f64(),
        install_took.as_secs_f64(),
        store_took.as_secs_f64()
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(served_map, from_install, "the same map from the store");
    assert!(report.files > 100, "{} files", report.files);
}

#[test]
fn warehouse_loads_from_the_asset_service_with_no_install() {
    let Some(found) = LocalConfig::load().ok().and_then(|c| install::resolve(&c, GAME, None).install) else {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    };
    let Ok(installed) = mount::open_install(&found.path) else {
        eprintln!("skipping: no Combat Arms install at {}", found.path.display());
        return;
    };
    let recorder = Recorder::in_memory();
    let recorded = served::record(installed, GAME, &recorder);
    let from_install = summary(&world::load_from(&recorded, "warehouse").unwrap());
    let paths: Vec<String> = recorder.lines().into_iter().map(|l| l.path).collect();

    // The server's store: what the load read, decrypted, published as
    // build "test".
    let dir = std::env::temp_dir().join(format!("chimera_served_ca_service_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server_store = Store::new(dir.join("server"));
    let mut manifest = Manifest::new(GAME, "test", None);
    let report = pack(&recorded, &paths, &server_store, &mut manifest, Some("warehouse"), 4).unwrap();
    manifest.save(&server_store.manifest_path(GAME, "test")).unwrap();
    let mut users = Accounts::default();
    let (_, token) = users.add("heavy_test", None, "").unwrap();
    users.save(&Accounts::path(&server_store.root)).unwrap();
    let running = server::serve(server::Service::new(server_store).unwrap().with_latency(latency()), "127.0.0.1:0", 4).unwrap();

    // The client: no install, no keys, an empty store, the service.
    let client = Store::new(dir.join("client"));
    let settings = AssetSettings {
        store: Some(client.clone()),
        use_install: false,
        builds: Default::default(),
        remote: Some(RemoteConfig { url: running.url(), token, key: None }),
        lock: None,
    };
    let started = Instant::now();
    let mount = served::open_with(&settings, GAME, &dir.join("no-install"), mount::open_install).unwrap();
    let fetched = summary(&world::load_from(&mount, "warehouse").unwrap());
    let took = started.elapsed();
    let records = client.record_ids().len();
    eprintln!(
        "warehouse from the service: {records} of {} records fetched on demand, load {:.2} s",
        report.records,
        took.as_secs_f64()
    );
    drop(running);
    let log = std::fs::read_to_string(dir.join("server/server/access.log")).unwrap_or_default();
    let (singles, batches) = (log.matches(" /r/").count(), log.matches(" /batch ").count());
    eprintln!("  requests: {singles} single records, {batches} batches; latency {:?}", latency());
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(fetched, from_install, "the same map from fetched files");
    assert!(records > 100 && records <= report.records, "{records} records fetched");
}

/// `CHIMERA_ASSET_LATENCY_MS`: the test service delays each answer this
/// long, to measure on-demand fetching as over a slower link (Tailscale, a
/// VPS); with `CHIMERA_ASSET_BATCH=0` too, one request per record.
fn latency() -> std::time::Duration {
    let ms = std::env::var("CHIMERA_ASSET_LATENCY_MS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    std::time::Duration::from_millis(ms)
}
