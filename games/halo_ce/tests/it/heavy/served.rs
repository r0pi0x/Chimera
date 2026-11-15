//! Halo from the asset store alone (`chimera_pack pack halo_ce`,
//! docs/assets-for-developers.md): the disc's build as its maps name it
//! (`mount::build`), Hang 'Em High and the donor map packed from the disc
//! image into a scratch store, then Hang 'Em High converted from a mount
//! of only that store (as `mount::served` gives a client with no disc):
//! the same map as from the disc, the map list from the store's files, a
//! second load from the converted-map cache. And the sets
//! (`game::sets`): each map's load reads its own file alone, `base` the
//! donor map. Skipped without the disc image (the checks pack from it).
//! The stores are scratch folders, removed after.

use std::{
    collections::BTreeSet,
    fmt::Write as _,
    hash::{DefaultHasher, Hasher},
    path::PathBuf,
};

use chimera_core::mount::{
    Mount,
    assets::{AssetSettings, Manifest, Recorder, Store, pack},
    served,
};
use chimera_world::map::{
    MapData, MapLoadOptions,
    cache::{MapCache, Outcome},
};
use halo_ce::game::{GAME, INFO, maps, mount, sets};

const MAP: &str = "hangemhigh";

/// The disc image or folder itself (not the store's stand-in).
fn install() -> Option<PathBuf> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return None;
    };
    if served::alone(GAME, &path) || !(path.is_file() || path.join("maps").is_dir()) {
        eprintln!("skipping: packs from the disc image itself (served from the asset store here)");
        return None;
    }
    Some(path)
}

/// `Debug` text fed to a hasher, so a whole map compares without holding
/// its text.
struct HashWriter(DefaultHasher);

impl std::fmt::Write for HashWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

/// What must match between two loads: the file's hash, the counts, every
/// mesh, texture and collision triangle.
fn summary(map: &MapData) -> (Option<[u8; 32]>, usize, usize, usize, u64) {
    let mut h = HashWriter(DefaultHasher::new());
    write!(h, "{:?}{:?}{:?}{:?}", map.meshes, map.textures, map.collision_indices, map.spawns).unwrap();
    (map.file_hash, map.triangle_count(), map.meshes.len(), map.textures.len(), h.0.finish())
}

fn load(mount: &Mount, cache: Option<&MapCache>) -> (MapData, Outcome) {
    maps::load_in(mount, MAP, &MapLoadOptions::default(), cache).unwrap()
}

/// The files `f` reads from the disc at `path` (a recorded mount, as
/// `chimera_pack sets` taps it).
fn reads(path: &std::path::Path, f: impl FnOnce(&Mount) -> Result<(), String>) -> BTreeSet<String> {
    let recorder = Recorder::in_memory();
    let recorded = served::record(mount::open_install(path).unwrap(), GAME, &recorder);
    f(&recorded).unwrap();
    recorder.lines().into_iter().map(|l| l.path).collect()
}

#[test]
fn the_disc_names_its_build() {
    let Some(path) = install() else { return };
    // specs/halo_ce/cache_file.md: every disc map's header says it.
    assert_eq!(mount::build(&path).unwrap(), "01.10.12.2276");
}

#[test]
fn hangemhigh_converts_from_the_store_alone() {
    let Some(path) = install() else { return };
    let disc = mount::open_install(&path).unwrap();
    let (from_disc, _) = load(&disc, None);
    let build = mount::build(&path).unwrap();

    let dir = std::env::temp_dir().join(format!("chimera_halo_served_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(dir.join("store"));
    let files = [maps::map_path(MAP), maps::map_path(sets::DONOR_MAP)];
    let mut manifest = Manifest::new(GAME, &build, None);
    let report = pack(&disc, &files, &store, &mut manifest, None, 2).unwrap();
    assert!(report.missing.is_empty(), "{:?}", report.missing);
    assert_eq!(report.records, 2);
    manifest.save(&store.manifest_path(GAME, &build)).unwrap();

    // A client with no disc: the install off, its path anything.
    let settings = AssetSettings {
        store: Some(store.clone()),
        use_install: false,
        builds: Default::default(),
        remote: None,
        lock: None,
    };
    let served_mount = served::open_with(&settings, GAME, &dir.join("no-disc"), mount::open_install).unwrap();
    assert_eq!((INFO.map_names)(&served_mount), ["bloodgulch", "hangemhigh"]);
    // The store's stand-in for a missing install counts as one.
    let stand_in = settings.stand_in_install(GAME).unwrap();
    assert!(stand_in.is_dir(), "the manifest folder");

    let cache = MapCache::at(dir.join("map-cache"), 1 << 30);
    let (first, outcome) = load(&served_mount, Some(&cache));
    assert_ne!(outcome, Outcome::Hit);
    let (again, outcome) = load(&served_mount, Some(&cache));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(summary(&first), summary(&from_disc), "the same map from the store");
    assert_eq!(outcome, Outcome::Hit, "the second load from the converted-map cache");
    assert_eq!(summary(&again), summary(&from_disc));
}

#[test]
fn each_set_reads_what_it_names() {
    let Some(path) = install() else { return };
    for name in [MAP, "wizard"] {
        assert_eq!(reads(&path, |m| sets::load_map(m, name)), BTreeSet::from([maps::map_path(name)]), "map/{name}");
    }
    let donor = BTreeSet::from([maps::map_path(sets::DONOR_MAP)]);
    assert_eq!(reads(&path, sets::load_base), donor, "base");
    assert_eq!(reads(&path, sets::load_players), donor, "players");
    assert_eq!(reads(&path, |m| sets::load_weapon(m, "assault_rifle")), donor, "weapon/<id>");
}
