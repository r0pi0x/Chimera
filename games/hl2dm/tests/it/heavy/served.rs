//! dm_lockdown from the asset store alone (docs/plans/active/
//! asset-service.md; `chimera_pack publish hl2dm`): a conversion from the
//! install is recorded (`mount::served::record`), the files it read are
//! packed into a scratch store, and the map converts again from a mount
//! of only that store's records (`the_store_alone`), and from the asset
//! service on 127.0.0.1 with no install, an empty local store and the
//! service set (`the_service_with_no_install`): the same map, no file
//! missing. Skipped without an HL2:DM install (the checks pack from it).
//! The stores are scratch folders, removed after.

use std::{
    fmt::Write as _,
    hash::{DefaultHasher, Hasher},
};

use chimera_core::mount::{
    Mount,
    assets::{AssetSettings, CachedFiles, Manifest, Recorder, RemoteConfig, Store, accounts::Accounts, pack, server},
    served,
};
use chimera_world::map::{MapData, MapLoadOptions};
use hl2dm::game::{GAME, INFO, mount};

use super::map_stock::convert;

const MAP: &str = "dm_lockdown";

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

/// The install's own mount (not the store's), with what a conversion of
/// `MAP` read from it; the summary of that conversion.
fn recorded() -> Option<(Mount, Vec<String>, (Option<[u8; 32]>, usize, usize, usize, u64))> {
    let path = INFO.install()?;
    if !path.join("hl2mp").is_dir() {
        eprintln!("skipping: needs the install's own folders (served from the asset store here)");
        return None;
    }
    let installed = mount::open_install(&path).ok()?;
    let recorder = Recorder::in_memory();
    let recorded = served::record(installed, GAME, &recorder);
    let from_install = summary(&convert(&recorded, MAP, &MapLoadOptions::default()));
    let paths = recorder.lines().into_iter().map(|l| l.path).collect();
    Some((recorded, paths, from_install))
}

#[test]
fn dm_lockdown_converts_from_the_store_alone() {
    let Some((recorded, paths, from_install)) = recorded() else { return };
    let dir = std::env::temp_dir().join(format!("chimera_hl2dm_served_map_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(&dir);
    let mut manifest = Manifest::new(GAME, "test", Some(320));
    let report = pack(&recorded, &paths, &store, &mut manifest, Some(MAP), 4).unwrap();
    manifest.save(&store.manifest_path(GAME, "test")).unwrap();
    assert!(report.missing.is_empty(), "{:?}", report.missing);
    let mut from_store = Mount::default();
    from_store.push(CachedFiles::open(&store, GAME, "test").unwrap());
    let from_the_store = summary(&convert(&from_store, MAP, &MapLoadOptions::default()));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(from_the_store, from_install, "the same map from the store");
    assert!(report.files > 100, "{} files", report.files);
}

#[test]
fn dm_lockdown_converts_from_the_asset_service_with_no_install() {
    let Some((recorded, paths, from_install)) = recorded() else { return };
    let dir = std::env::temp_dir().join(format!("chimera_hl2dm_served_service_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server_store = Store::new(dir.join("server"));
    let mut manifest = Manifest::new(GAME, "test", Some(320));
    let report = pack(&recorded, &paths, &server_store, &mut manifest, Some(MAP), 4).unwrap();
    manifest.save(&server_store.manifest_path(GAME, "test")).unwrap();
    let mut users = Accounts::default();
    let (_, token) = users.add("heavy_test", None, "").unwrap();
    users.save(&Accounts::path(&server_store.root)).unwrap();
    let running = server::serve(server::Service::new(server_store).unwrap(), "127.0.0.1:0", 4).unwrap();

    let client = Store::new(dir.join("client"));
    let settings = AssetSettings {
        store: Some(client.clone()),
        use_install: false,
        builds: Default::default(),
        remote: Some(RemoteConfig { url: running.url(), token, key: None }),
        lock: None,
    };
    let served_mount = served::open_with(&settings, GAME, &dir.join("no-install"), mount::open_install).unwrap();
    let fetched = summary(&convert(&served_mount, MAP, &MapLoadOptions::default()));
    let records = client.record_ids().len();
    drop(running);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(fetched, from_install, "the same map from fetched files");
    assert!(records > 100 && records <= report.records, "{records} records fetched");
}
