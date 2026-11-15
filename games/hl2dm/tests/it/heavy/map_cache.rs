//! The converted-map cache (chimera_world's `map::cache`) with HL2:DM's
//! key: `cache::key("hl2dm", maps::CONVERTER)` (its build.rs hashes this
//! crate's sources and data and Source's), with the map's file and the
//! load options. A map read back is the one converted (compared part by
//! part); the same file loaded as another Source game is another key.
//! Each test uses its own scratch cache folder beside the user's map
//! cache, removed after. Skips without an install.

use std::path::PathBuf;

use chimera_world::map::{
    MapData, MapLoadOptions,
    cache::{MapCache, Outcome},
};
use hl2dm::game::maps;
use sha2::{Digest, Sha256};
use source_engine::content::bsp::{self, SourceGame};

use super::map_stock::install_mount;

/// Whether the cache can write here: room for a whole entry (a gigabyte at
/// most) above the disk's free-space floor, else the cache skips its write
/// (`chimera_assets::disk`) and a miss never becomes a hit. Says why not.
fn room_for_entries(dir: &std::path::Path) -> bool {
    use chimera_core::mount::assets::disk;
    let room = disk::has_room(dir, 1_000_000_000, &disk::space);
    if !room {
        eprintln!(
            "skipping: less than a gigabyte above the disk's free-space floor at {} ({} sets the floor)",
            dir.display(),
            disk::FLOOR_ENV
        );
    }
    room
}

fn scratch(name: &str) -> PathBuf {
    let base = MapCache::default_dir()
        .and_then(|d| d.parent().map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("maps-test-hl2dm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn digest(map: &MapData) -> Vec<(&'static str, String)> {
    fn h<T: serde::Serialize>(v: &T) -> String {
        format!("{:x}", Sha256::digest(postcard::to_allocvec(v).expect("serializes")))
    }
    vec![
        ("meshes", h(&map.meshes)),
        ("textures", h(&map.textures)),
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
    let bytes = mount.read("maps/halls3.bsp").expect("halls3");
    Some(bsp::load_level_bytes_in(game, &mount, "halls3", bytes, &MapLoadOptions::lean(), cache).expect("loads"))
}

#[test]
fn a_cached_map_is_the_converted_one() {
    let Some((cold, o)) = load(&maps::HL2DM, None) else { return };
    assert_eq!(o, Outcome::Off);
    let dir = scratch("hit");
    if !room_for_entries(&dir) {
        return;
    }
    let cache = MapCache::at(&dir, u64::MAX);
    let (miss, o) = load(&maps::HL2DM, Some(&cache)).unwrap();
    assert_eq!(o, Outcome::Miss);
    let (warm, o) = load(&maps::HL2DM, Some(&cache)).unwrap();
    assert_eq!(o, Outcome::Hit);
    assert_eq!(digest(&cold), digest(&miss));
    assert_eq!(digest(&cold), digest(&warm), "read back, the same map");
    assert!(warm.light_field.is_some(), "the light field rebuilt on a hit");
    assert_eq!(warm.name, "hl2dm:halls3");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_key_is_hl2dms() {
    // HL2:DM's converter version is its own build's hash.
    assert_eq!(maps::CONVERTER.len(), maps::CONVERTER.trim().len());
    assert!(!maps::CONVERTER.is_empty());
    let dir = scratch("key");
    if !room_for_entries(&dir) {
        return;
    }
    let cache = MapCache::at(&dir, u64::MAX);
    let Some((_, o)) = load(&maps::HL2DM, Some(&cache)) else { return };
    assert_eq!(o, Outcome::Miss);
    // The same file and options as another game (another id, or the same
    // id at another converter version): other keys, converted again.
    let other_game = SourceGame { id: "other_source_game", ..maps::HL2DM };
    let other_version = SourceGame { converter: "another version", ..maps::HL2DM };
    for game in [&other_game, &other_version] {
        let (map, o) = load(game, Some(&cache)).unwrap();
        assert_eq!(o, Outcome::Miss, "{} {}", game.id, game.converter);
        assert_eq!(map.name, format!("{}:halls3", game.id));
    }
    assert_eq!(load(&maps::HL2DM, Some(&cache)).unwrap().1, Outcome::Hit);
    let _ = std::fs::remove_dir_all(&dir);
}
