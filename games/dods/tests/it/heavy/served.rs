//! A DoD:S rung from the asset service alone (docs/plans/active/dods.md,
//! M3b; `chimera_pack publish dods`): what a client loads for a rung of the
//! MG42 (its view and held models, every class body with the shared
//! animation model) is recorded from the install (`mount::served::record`),
//! packed into a scratch server store as the `players` set, served on
//! 127.0.0.1, and loaded again by a client with no install and an empty
//! store that fetched the set, as a match's offer has it fetch
//! (`served::open_with`, `use_install` off): the same models, the gun's class body among them, no
//! file missing. Skipped without a DoD:S install (it packs from one). The
//! stores are scratch folders, removed after.

use chimera_core::mount::{
    Mount,
    assets::{AssetSettings, Manifest, sets, Recorder, RemoteConfig, Store, accounts::Accounts, pack, server},
    served,
};
use chimera_world::map::{GameKit, KitLoad};
use dods::game::{GAME, INFO, bodies, classes, mount, weapons};
use source_engine::content::{material::MaterialLoader, surfaceprops::SurfaceProps};

const GUN: &str = weapons::MG42;

/// What a client's kit for the rung loads from `mount`: the held and view
/// models' keys, the bodies' names (the default unnamed), and warnings.
fn rung_kit(mount: &Mount) -> (Vec<String>, Vec<String>, Vec<Option<String>>, Vec<String>) {
    let mut materials = MaterialLoader::guest(mount, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    let (mut kit, mut warnings) = (GameKit::default(), Vec::new());
    let wants = KitLoad::Players(&["dods:weapon_mg42"]);
    bodies::add_bodies(&mut materials, &surfaces, wants, &mut kit, &mut warnings);
    weapons::add_guns(&mut materials, &surfaces, &|g| wants.wants(g), &mut kit, &mut warnings);
    let held = kit.held.iter().map(|h| h.key.clone()).collect();
    let views = kit.view_models.iter().map(|v| v.key.clone()).collect();
    let bodies = kit.characters.iter().map(|c| c.name.clone()).collect();
    (held, views, bodies, warnings)
}

#[test]
fn a_dods_rung_loads_from_the_asset_service_with_no_install() {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no DoD:S install");
        return;
    };
    if !path.join("dod").is_dir() {
        eprintln!("skipping: needs the install's own folders (served from the asset store here)");
        return;
    }
    let installed = mount::open_install(&path).expect("the install mounts");
    let recorder = Recorder::in_memory();
    let recorded = served::record(installed, GAME, &recorder);
    let from_install = rung_kit(&recorded);
    assert!(from_install.0.iter().any(|k| k == GUN), "the MG42's held model: {from_install:?}");
    let paths: Vec<String> = recorder.lines().into_iter().map(|l| l.path).collect();

    let dir = std::env::temp_dir().join(format!("chimera_dods_served_rung_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server_store = Store::new(dir.join("server"));
    let mut manifest = Manifest::new(GAME, "test", Some(300));
    let report = pack(&recorded, &paths, &server_store, &mut manifest, Some("players"), 4).unwrap();
    manifest.save(&server_store.manifest_path(GAME, "test")).unwrap();
    assert!(report.missing.is_empty(), "{:?}", report.missing);
    let mut users = Accounts::default();
    let (_, token) = users.add("heavy_test", None, "").unwrap();
    users.save(&Accounts::path(&server_store.root)).unwrap();
    let running = server::serve(server::Service::new(server_store).unwrap(), "127.0.0.1:0", 4).unwrap();

    let client = Store::new(dir.join("client"));
    let settings = AssetSettings {
        store: Some(client.clone()),
        use_install: false,
        service_first: false,
        builds: Default::default(),
        remote: Some(RemoteConfig { url: running.url(), token, key: None, fingerprint: None }),
        lock: None,
    };
    // The match's offer names the game's `players` set (here everything
    // the rung read): fetched before the kit loads, as a joining client's
    // download does (`net::assets`).
    let wanted = [sets::Wanted { game: GAME.into(), sets: vec!["players".into()] }];
    sets::fetch(&settings, &wanted, "test", &std::sync::Mutex::new(None), sets::Options::default()).expect("the set arrives");
    let served_mount = served::open_with(&settings, GAME, &dir.join("no-install"), mount::open_install).unwrap();
    let fetched = rung_kit(&served_mount);
    let records = client.record_ids().len();
    drop(running);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(fetched, from_install, "the same rung from fetched files");
    assert!(records > 10, "{records} records fetched");
    // The MG42's class body (the Axis machine gunner) is among the bodies.
    let class = classes::classes().of_gun(GUN).unwrap().model();
    assert!(fetched.2.iter().any(|n| n.as_deref() == Some(class)), "{class} in {:?}", fetched.2);
}
