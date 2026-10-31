//! de_dust2 from the asset store alone (docs/plans/active/asset-service.md,
//! M1): a load from the install is recorded (`mount::served::record`), the
//! files it read are packed into a scratch store (`chimera_assets::pack`),
//! and the map loads again from a mount of only that store's records: the
//! same map, no file missing. Skipped without a CS:S install. The store is
//! a scratch folder, removed after.
//!
//! M2: the same store served by the asset service on 127.0.0.1, and the
//! map loaded through the mount a game uses (`served::open_with`) with no
//! install, an empty local store and the service set: the manifest and
//! every record arrive on demand, checked, and the map is the same.

use std::{
    fmt::Write as _,
    hash::{DefaultHasher, Hasher},
    time::Instant,
};

use ::cs_source::game::{self as cs_source, bsp};
use chimera_core::mount::{
    Mount,
    assets::{AssetSettings, CachedFiles, Manifest, Recorder, RemoteConfig, Store, accounts::Accounts, pack, server},
    config::LocalConfig,
    served,
};
use chimera_world::map::{MapData, MapLoadOptions};

/// `Debug` text fed to a hasher, so a whole map compares without holding
/// its text.
struct HashWriter(DefaultHasher);

impl std::fmt::Write for HashWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// What must match between two loads: the file's hash, every mesh and
/// texture (texels included), the triangle count.
fn summary(map: &MapData) -> (Option<[u8; 32]>, usize, usize, usize, u64) {
    let mut h = HashWriter(DefaultHasher::new());
    write!(h, "{:?}{:?}", map.meshes, map.textures).unwrap();
    (map.file_hash, map.triangle_count(), map.meshes.len(), map.textures.len(), h.0.finish())
}

#[test]
fn de_dust2_loads_from_the_store_alone() {
    let Some(install) = LocalConfig::load().ok().and_then(|c| {
        chimera_core::mount::install::resolve(&c, cs_source::GAME, cs_source::INFO.steam.as_ref()).install
    }) else {
        eprintln!("skipping: no CS:S install");
        return;
    };
    let Ok(installed) = cs_source::mount::open_install(&install.path) else {
        eprintln!("skipping: no CS:S install at {}", install.path.display());
        return;
    };
    let options = MapLoadOptions::default();
    let recorder = Recorder::in_memory();
    let recorded = served::record(installed, cs_source::GAME, &recorder);
    let started = Instant::now();
    let map = bsp::load_level_with(&recorded, "de_dust2", &options).unwrap();
    let install_took = started.elapsed();
    let from_install = summary(&map);
    drop(map);
    let paths: Vec<String> = recorder.lines().into_iter().map(|l| l.path).collect();
    let read: u64 = recorder.lines().iter().map(|l| l.size).sum();

    let dir = std::env::temp_dir().join(format!("chimera_served_dust2_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(&dir);
    let mut manifest = Manifest::new(cs_source::GAME, "test", Some(240));
    let report = pack(&recorded, &paths, &store, &mut manifest, Some("de_dust2"), 4).unwrap();
    manifest.save(&store.manifest_path(cs_source::GAME, "test")).unwrap();
    assert!(report.missing.is_empty(), "{:?}", report.missing);

    let mut from_store = Mount::default();
    from_store.push(CachedFiles::open(&store, cs_source::GAME, "test").unwrap());
    let started = Instant::now();
    let map = bsp::load_level_with(&from_store, "de_dust2", &options).unwrap();
    let store_took = started.elapsed();
    let served_map = summary(&map);
    eprintln!(
        "de_dust2: {} files, {:.1} MB read, {} records, {:.1} MB stored, packed in {:.2} s; \
         load {:.2} s from the install, {:.2} s from the store",
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
fn de_dust2_loads_from_the_asset_service_with_no_install() {
    let Some(install) = LocalConfig::load().ok().and_then(|c| {
        chimera_core::mount::install::resolve(&c, cs_source::GAME, cs_source::INFO.steam.as_ref()).install
    }) else {
        eprintln!("skipping: no CS:S install");
        return;
    };
    let Ok(installed) = cs_source::mount::open_install(&install.path) else {
        eprintln!("skipping: no CS:S install at {}", install.path.display());
        return;
    };
    let options = MapLoadOptions::default();
    let recorder = Recorder::in_memory();
    let recorded = served::record(installed, cs_source::GAME, &recorder);
    let from_install = summary(&bsp::load_level_with(&recorded, "de_dust2", &options).unwrap());
    let paths: Vec<String> = recorder.lines().into_iter().map(|l| l.path).collect();

    // The server's store: what the load read, published as build "test".
    let dir = std::env::temp_dir().join(format!("chimera_served_service_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server_store = Store::new(dir.join("server"));
    let mut manifest = Manifest::new(cs_source::GAME, "test", Some(240));
    let report = pack(&recorded, &paths, &server_store, &mut manifest, Some("de_dust2"), 4).unwrap();
    manifest.save(&server_store.manifest_path(cs_source::GAME, "test")).unwrap();
    let accounts = Accounts::path(&server_store.root);
    let mut users = Accounts::default();
    let (_, token) = users.add("heavy_test", None, "").unwrap();
    users.save(&accounts).unwrap();
    let running = server::serve(server::Service::new(server_store).unwrap(), "127.0.0.1:0", 4).unwrap();

    // The client: no install, an empty store, the service.
    let client = Store::new(dir.join("client"));
    let settings = AssetSettings {
        store: Some(client.clone()),
        use_install: false,
        builds: Default::default(),
        remote: Some(RemoteConfig { url: running.url(), token, key: None }),
        lock: None,
    };
    let no_install = dir.join("no-install");
    let started = Instant::now();
    let mount = served::open_with(&settings, cs_source::GAME, &no_install, cs_source::mount::open_install).unwrap();
    let map = bsp::load_level_with(&mount, "de_dust2", &options).unwrap();
    let took = started.elapsed();
    let fetched = summary(&map);
    let records = client.record_ids().len();
    eprintln!(
        "de_dust2 from the service: {records} of {} records fetched on demand, load {:.2} s",
        report.records,
        took.as_secs_f64()
    );
    drop(running);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(fetched, from_install, "the same map from fetched files");
    assert!(records > 100 && records <= report.records, "{records} records fetched");
}
