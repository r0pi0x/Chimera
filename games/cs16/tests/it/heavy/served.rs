//! CS 1.6 from the asset service alone (`chimera_pack sets cs16`,
//! `game::sets`): what a client reads for `cs16:de_dust2` with the players'
//! kit and for the eight class bodies is recorded from the install
//! (`mount::served::record`), packed into a scratch server store as the
//! sets `players` and `map/de_dust2`, served on 127.0.0.1, and loaded
//! again by a client with no install and an empty store that fetched those
//! sets, as a match's offer has it fetch (`served::open_with`,
//! `use_install` off): the same map (meshes, textures, spawns, collision)
//! and the same bodies, nothing missing. Skipped without an install of
//! its own folders (it packs from one). The stores are scratch folders,
//! removed after.

use chimera_core::mount::{
    Mount,
    assets::{AssetSettings, Manifest, Recorder, RemoteConfig, Store, accounts::Accounts, pack, server, sets},
    served,
};
use chimera_world::map::MapData;
use cs16::game::{CONTENT_DIR, GAME, INFO, STEAM_APP, mount, sets as pieces};

/// What a client sees of the map and of the bodies, to compare.
#[derive(Debug, PartialEq)]
struct Seen {
    meshes: usize,
    textures: Vec<(String, u32, u32)>,
    spawns: usize,
    solids: usize,
    clips: usize,
    bodies: Vec<(Option<String>, usize)>,
    warnings: Vec<String>,
}

fn seen(map: &MapData, bodies: &MapData) -> Seen {
    let mut warnings: Vec<String> = map.warnings.iter().chain(&bodies.warnings).cloned().collect();
    warnings.sort();
    Seen {
        meshes: map.meshes.len(),
        textures: map.textures.iter().map(|t| (t.name.clone(), t.width, t.height)).collect(),
        spawns: map.spawns.len(),
        solids: map.collision_hulls.len(),
        clips: map.player_clips.len(),
        bodies: bodies.kit.characters.iter().map(|c| (c.name.clone(), c.model.meshes.len())).collect(),
        warnings,
    }
}

/// The class bodies on `mount`.
fn bodies(mount: &Mount) -> MapData {
    let mut data = MapData::default();
    cs16::game::bodies::add_bodies(mount, true, &mut data);
    data
}

#[test]
fn de_dust2_and_a_body_load_from_the_asset_service_with_no_install() {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no Counter-Strike 1.6 install");
        return;
    };
    if !path.join(CONTENT_DIR).is_dir() {
        eprintln!("skipping: needs the install's own folders (served from the asset store here)");
        return;
    }
    // Each set's reads, recorded from the install as `chimera_pack sets`
    // records them.
    let record = |load: &dyn Fn(&Mount)| -> Vec<String> {
        let recorder = Recorder::in_memory();
        let recorded = served::record(mount::open_install(&path).expect("the install mounts"), GAME, &recorder);
        load(&recorded);
        recorder.lines().into_iter().map(|l| l.path).collect()
    };
    let players = record(&|m| pieces::load_players(m));
    let map_files = record(&|m| {
        pieces::load_map(m, "de_dust2").expect("de_dust2 converts");
    });
    let installed = mount::open_install(&path).unwrap();
    let from_install = seen(&pieces::load_map(&installed, "de_dust2").unwrap(), &bodies(&installed));
    assert_eq!(from_install.bodies.len(), 8, "{from_install:?}");
    assert!(players.iter().any(|p| p.ends_with("gign.mdl")), "{players:?}");
    assert!(map_files.iter().any(|p| p.ends_with(".wad")), "the map's WADs: {map_files:?}");

    let dir = std::env::temp_dir().join(format!("chimera_cs16_served_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server_store = Store::new(dir.join("server"));
    let mut manifest = Manifest::new(GAME, "test", Some(STEAM_APP));
    for (set, paths) in [("players", &players), ("map/de_dust2", &map_files)] {
        let report = pack(&installed, paths, &server_store, &mut manifest, Some(set), 4).unwrap();
        assert!(report.missing.is_empty(), "{set}: {:?}", report.missing);
    }
    manifest.save(&server_store.manifest_path(GAME, "test")).unwrap();
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
    // The match's offer: the game's `players` and the map's set.
    let wanted = [sets::Wanted { game: GAME.into(), sets: vec!["players".into(), "map/de_dust2".into()] }];
    sets::fetch(&settings, &wanted, "test", &std::sync::Mutex::new(None), sets::Options::default()).expect("the sets arrive");
    let served_mount = served::open_with(&settings, GAME, &dir.join("no-install"), mount::open_install).unwrap();
    let fetched = pieces::load_map(&served_mount, "de_dust2").map(|m| seen(&m, &bodies(&served_mount)));
    let records = client.record_ids().len();
    drop(running);
    let _ = std::fs::remove_dir_all(&dir);
    let fetched = fetched.expect("de_dust2 loads from the fetched files");
    assert_eq!(fetched, from_install, "the same map and bodies from fetched files");
    assert!(fetched.meshes > 20 && fetched.spawns >= 20, "{fetched:?}");
    eprintln!("{records} records fetched; {} map files, {} body files", map_files.len(), players.len());
}
