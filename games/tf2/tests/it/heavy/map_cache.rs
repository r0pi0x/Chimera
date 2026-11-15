//! The converted-map cache (chimera_world's `map::cache`) with TF2's key:
//! `cache::key("tf2", maps::CONVERTER)` (its build.rs hashes this crate's
//! sources and Source's), with the map's file and the load options. A map
//! read back is the one converted (compared part by part); the same file
//! loaded as another Source game is another key. Each test uses its own
//! scratch cache folder beside the user's map cache, removed after. Skips
//! without an install.

use std::path::PathBuf;

use chimera_world::map::{
    MapData, MapLoadOptions,
    cache::{MapCache, Outcome},
};
use sha2::{Digest, Sha256};
use source_engine::content::bsp::{self, SourceGame};
use tf2::game::maps;

use super::maps::install_mount;

fn scratch(name: &str) -> PathBuf {
    let base = MapCache::default_dir()
        .and_then(|d| d.parent().map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("maps-test-tf2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn digest(map: &MapData) -> Vec<(&'static str, String)> {
    fn h<T: serde::Serialize>(v: &T) -> String {
        format!("{:x}", Sha256::digest(postcard::to_allocvec(v).expect("serializes")))
    }
    vec![
        ("meshes", h(&map.meshes)),
        ("lightmap", h(&map.lightmap)),
        ("collision", h(&(&map.collision_positions, &map.collision_indices, &map.collision_hulls))),
        ("entities", h(&map.entities)),
        ("props", h(&(&map.props, &map.models))),
        ("spawns", h(&(&map.spawns, &map.spawn_yaws))),
        ("name", map.name.clone()),
    ]
}

fn load(game: &SourceGame, cache: Option<&MapCache>) -> Option<(MapData, Outcome)> {
    let mount = install_mount()?;
    let bytes = mount.read("maps/ctf_2fort.bsp").expect("ctf_2fort");
    Some(bsp::load_level_bytes_in(game, &mount, "ctf_2fort", bytes, &MapLoadOptions::lean(), cache).expect("loads"))
}

#[test]
fn the_key_is_tf2s_and_a_hit_is_the_conversion() {
    // TF2's converter version is its own build's hash.
    assert_eq!(maps::CONVERTER.len(), maps::CONVERTER.trim().len());
    assert!(!maps::CONVERTER.is_empty());
    let Some((cold, o)) = load(&maps::TF2, None) else { return };
    assert_eq!(o, Outcome::Off);
    let dir = scratch("key");
    let cache = MapCache::at(&dir, u64::MAX);
    let (miss, o) = load(&maps::TF2, Some(&cache)).unwrap();
    assert_eq!(o, Outcome::Miss);
    let (warm, o) = load(&maps::TF2, Some(&cache)).unwrap();
    assert_eq!(o, Outcome::Hit);
    assert_eq!(digest(&cold), digest(&miss));
    assert_eq!(digest(&cold), digest(&warm), "read back, the same map");
    assert_eq!(warm.name, "tf2:ctf_2fort");
    // The same file and options as another game (another id, or the same
    // id at another converter version): other keys, converted again.
    let other_game = SourceGame { id: "other_source_game", ..maps::TF2 };
    let other_version = SourceGame { converter: "another version", ..maps::TF2 };
    for game in [&other_game, &other_version] {
        let (map, o) = load(game, Some(&cache)).unwrap();
        assert_eq!(o, Outcome::Miss, "{} {}", game.id, game.converter);
        assert_eq!(map.name, format!("{}:ctf_2fort", game.id));
    }
    assert_eq!(load(&maps::TF2, Some(&cache)).unwrap().1, Outcome::Hit);
    let _ = std::fs::remove_dir_all(&dir);
}
