//! The converted-map cache (chimera_world `map::cache`) with Combat Arms'
//! WAREHOUSE from the install: the world read back from the cache is the
//! world converted (meshes, textures, lightmap, collision, entities),
//! compared by hash, the effect library added to either; a damaged entry
//! converts again. Prints cold and warm load times and the entry's size
//! (`--nocapture`). A scratch cache folder beside the user's map cache,
//! removed after. Skipped without a Combat Arms install.

use std::{
    hash::{DefaultHasher, Hasher},
    path::PathBuf,
    time::Instant,
};

use chimera_core::mount::{Mount, config::LocalConfig};
use chimera_world::map::{
    MapData, MapLoadOptions,
    cache::{MapCache, Outcome},
};
use combat_arms::game::{GAME, INFO, world};

fn mount() -> Option<Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    (INFO.open)(&install)
        .map_err(|e| eprintln!("skipping: no Combat Arms install ({e})"))
        .ok()
}

fn digest(map: &MapData) -> Vec<(&'static str, u64)> {
    fn h<T: serde::Serialize>(v: &T) -> u64 {
        let mut s = DefaultHasher::new();
        s.write(&postcard::to_allocvec(v).expect("serializes"));
        s.finish()
    }
    vec![
        ("meshes", h(&map.meshes)),
        ("textures", h(&map.textures)),
        ("lightmap", h(&map.lightmap)),
        (
            "collision",
            h(&(&map.collision_positions, &map.collision_indices, &map.collision_hulls)),
        ),
        ("brushes", h(&map.collision_brushes)),
        ("entities", h(&map.entities)),
        ("props", h(&(&map.props, &map.models))),
        ("particles", h(&map.particles)),
        ("spawns", h(&(&map.spawns, &map.spawn_yaws))),
        ("warnings", h(&map.warnings)),
    ]
}

fn load(mount: &Mount, cache: Option<&MapCache>) -> (MapData, Outcome, f32) {
    let t = Instant::now();
    let (map, o) = world::load_world_in(mount, "warehouse", &MapLoadOptions::default(), cache).expect("warehouse");
    (map, o, t.elapsed().as_secs_f32())
}

#[test]
fn a_cached_warehouse_is_the_converted_one() {
    let Some(mount) = mount() else { return };
    // Beside the user's cache, on disk (the temporary folder may be memory).
    let base = MapCache::default_dir()
        .and_then(|d| d.parent().map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("maps-test-ca-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cache = MapCache::at(&dir, u64::MAX);
    // The first load also reads the session's data once (weapons, effect
    // file): time a second cold one.
    load(&mount, None);
    let (cold, o, t_cold) = load(&mount, None);
    assert_eq!(o, Outcome::Off);
    let (miss, o, t_miss) = load(&mount, Some(&cache));
    assert_eq!(o, Outcome::Miss);
    let (warm, o, t_warm) = load(&mount, Some(&cache));
    assert_eq!(o, Outcome::Hit);
    let size: u64 = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    eprintln!(
        "warehouse world: cold {t_cold:.2} s, miss and write {t_miss:.2} s, warm {t_warm:.2} s; entry {:.1} MB",
        size as f64 / 1e6
    );
    assert_eq!(digest(&cold), digest(&miss));
    assert_eq!(digest(&cold), digest(&warm), "read back, the same world");

    // A damaged entry: converted again.
    let entry = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "map"))
        .expect("an entry");
    let mut bytes = std::fs::read(&entry).unwrap();
    bytes.truncate(bytes.len() - 1);
    std::fs::write(&entry, &bytes).unwrap();
    let (again, o, _) = load(&mount, Some(&cache));
    assert_eq!(o, Outcome::Miss);
    assert_eq!(digest(&again), digest(&cold));
    let _ = std::fs::remove_dir_all(&dir);
}
