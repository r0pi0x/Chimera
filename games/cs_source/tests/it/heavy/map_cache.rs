//! The converted-map cache (chimera_world `map::cache`) with de_dust2 from
//! the install: a map read back from the cache is the map converted (its
//! meshes, textures, lightmap, collision, brushes, entities, visibility,
//! navigation, the light field), compared by hash; a file the conversion
//! read that changes, or a new one in front of it, converts again; a
//! damaged entry converts again. Prints cold and warm load times and the
//! entry's size (`--nocapture`). Each test uses its own scratch cache
//! folder beside the user's map cache, removed after. Skipped without a
//! CS:S install.

use std::{path::PathBuf, time::Instant};

use ::cs_source::game::{bsp, mount};
use chimera_core::mount::{LooseDir, Mount};
use chimera_world::map::{
    MapData, MapLoadOptions,
    cache::{MapCache, Outcome},
};
use sha2::{Digest, Sha256};

fn install_mount() -> Option<Mount> {
    let Some(install) = ::cs_source::game::INFO.install() else {
        eprintln!("skipping: no CS:S install configured (or asset store serving it)");
        return None;
    };
    mount::open(&install).ok()
}

fn scratch(name: &str) -> PathBuf {
    // Beside the user's cache, on disk: a whole entry is a gigabyte, and
    // the temporary folder may be memory (tmpfs).
    let base = MapCache::default_dir()
        .and_then(|d| d.parent().map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("maps-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// The hash of each part that must match between a converted map and one
/// read back.
fn digest(map: &MapData) -> Vec<(&'static str, String)> {
    fn h<T: serde::Serialize>(v: &T) -> String {
        let bytes = postcard::to_allocvec(v).expect("serializes");
        format!("{:x}", Sha256::digest(&bytes))
    }
    vec![
        ("meshes", h(&map.meshes)),
        ("textures", h(&map.textures)),
        ("lightmap", h(&map.lightmap)),
        (
            "collision",
            h(&(&map.collision_positions, &map.collision_indices, &map.collision_hulls)),
        ),
        (
            "brushes",
            h(&(&map.collision_brushes, &map.brush_tree, &map.trace_skip)),
        ),
        ("entities", h(&map.entities)),
        ("props", h(&(&map.props, &map.models))),
        ("visibility", h(&(&map.visibility, &map.sky_vis))),
        ("nav", h(&map.kit.nav)),
        ("characters", h(&(&map.kit.characters, &map.kit.held))),
        ("water", h(&(&map.water, &map.water_materials))),
        ("spawns", h(&(&map.spawns, &map.spawn_yaws))),
        ("file", h(&map.file_hash)),
        ("warnings", h(&map.warnings)),
    ]
}

/// The light field at the spawns (a closure, rebuilt on a hit).
fn probes(map: &MapData) -> Vec<String> {
    let field = map.light_field.as_ref().expect("a light field");
    map.spawns
        .iter()
        .take(8)
        .map(|(p, _)| format!("{:?}", (field.0)(*p + bevy::math::Vec3::Y)))
        .collect()
}

fn load(mount: &Mount, options: &MapLoadOptions, cache: Option<&MapCache>) -> (MapData, Outcome, f32) {
    let bytes = mount.read("maps/de_dust2.bsp").expect("de_dust2");
    let t = Instant::now();
    let (map, outcome) = bsp::load_level_bytes_in(mount, "de_dust2", bytes, options, cache).expect("loads");
    (map, outcome, t.elapsed().as_secs_f32())
}

/// A part's serialized size, whatever its type.
mod erased {
    pub trait Ser {
        fn size(&self) -> usize;
    }
    impl<T: serde::Serialize> Ser for T {
        fn size(&self) -> usize {
            postcard::to_allocvec(self).map_or(0, |b| b.len())
        }
    }
}

fn entry_size(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "map"))
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Cold (no cache), the miss that writes, the hit; each the same map.
fn cold_and_warm(options: &MapLoadOptions, what: &str) {
    let Some(mount) = install_mount() else { return };
    let dir = scratch(what);
    let cache = MapCache::at(&dir, u64::MAX);
    let (cold, o, t_cold) = load(&mount, options, None);
    assert_eq!(o, Outcome::Off);
    let (miss, o, t_miss) = load(&mount, options, Some(&cache));
    assert_eq!(o, Outcome::Miss);
    let size = entry_size(&dir);
    // What the entry is made of (MB), largest first.
    let mb = |v: &dyn erased::Ser| v.size() as f64 / 1e6;
    let mut parts: Vec<(&str, f64)> = vec![
        ("textures", mb(&cold.textures)),
        ("cubemaps", mb(&cold.cubemaps)),
        ("meshes", mb(&cold.meshes)),
        ("lightmap", mb(&cold.lightmap)),
        ("models", mb(&cold.models)),
        ("props", mb(&cold.props)),
        ("kit", mb(&cold.kit)),
        ("sounds", mb(&cold.sounds)),
        ("collision", mb(&(&cold.collision_positions, &cold.collision_indices, &cold.collision_hulls, &cold.collision_brushes))),
        ("entities", mb(&cold.entities)),
        ("visibility", mb(&cold.visibility)),
    ];
    parts.sort_by(|a, b| b.1.total_cmp(&a.1));
    eprintln!("de_dust2 {what} entry parts (MB): {parts:.1?}");
    let (warm, o, t_warm) = load(&mount, options, Some(&cache));
    assert_eq!(o, Outcome::Hit);
    let (_, _, t_warm2) = load(&mount, options, Some(&cache));
    eprintln!(
        "de_dust2 {what}: cold {t_cold:.2} s, miss and write {t_miss:.2} s, warm {t_warm:.2} s and {t_warm2:.2} s; \
         entry {:.1} MB",
        size as f64 / 1e6
    );
    assert_eq!(digest(&cold), digest(&miss));
    assert_eq!(digest(&cold), digest(&warm), "read back, the same map");
    assert_eq!(probes(&cold), probes(&warm), "the same light field");
    assert_eq!(warm.load_times.first().map(|t| t.0), Some("map cache"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_cached_dust2_is_the_converted_one() {
    cold_and_warm(&MapLoadOptions::default(), "full");
}

#[test]
fn a_cached_lean_dust2_is_the_converted_one() {
    cold_and_warm(&MapLoadOptions::lean(), "lean");
}

#[test]
fn a_changed_input_or_a_damaged_entry_converts_again() {
    let Some(install) = install_mount() else { return };
    // A loose folder in front of the install's layers: what a mod or a
    // patched file would be.
    let dir = scratch("inputs");
    let front = dir.join("front");
    std::fs::create_dir_all(front.join("maps")).unwrap();
    let mut mount = Mount::default();
    mount.push(LooseDir::new(&front));
    for layer in install.into_layers() {
        mount.push_boxed(layer);
    }
    let cache = MapCache::at(dir.join("cache"), u64::MAX);
    let options = MapLoadOptions::lean();
    let (first, o, _) = load(&mount, &options, Some(&cache));
    assert_eq!(o, Outcome::Miss);
    assert_eq!(load(&mount, &options, Some(&cache)).1, Outcome::Hit);

    // The navigation mesh, read by the conversion: a copy in front of the
    // install's is another input (same bytes, another layer).
    let nav = mount.read("maps/de_dust2.nav").expect("dust2's nav mesh");
    std::fs::write(front.join("maps").join("de_dust2.nav"), &nav).unwrap();
    let (_, o, _) = load(&mount, &options, Some(&cache));
    assert_eq!(o, Outcome::Miss, "a new file in front of one read");
    assert_eq!(load(&mount, &options, Some(&cache)).1, Outcome::Hit);
    // Its content changed: no navigation mesh in it.
    std::fs::write(front.join("maps").join("de_dust2.nav"), b"not a nav mesh").unwrap();
    let (changed, o, _) = load(&mount, &options, Some(&cache));
    assert_eq!(o, Outcome::Miss, "a changed file");
    assert!(
        first.kit.nav.is_some() && changed.kit.nav.is_none(),
        "converted from the new file"
    );
    std::fs::remove_file(front.join("maps").join("de_dust2.nav")).unwrap();
    let (back, o, _) = load(&mount, &options, Some(&cache));
    assert_eq!(o, Outcome::Miss, "the file gone again");
    assert_eq!(digest(&back), digest(&first));

    // A damaged entry (one byte of the payload flipped): converted again
    // and rewritten.
    let entry = std::fs::read_dir(dir.join("cache"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "map"))
        .expect("an entry");
    let mut bytes = std::fs::read(&entry).unwrap();
    let at = bytes.len() / 2;
    bytes[at] ^= 0x40;
    std::fs::write(&entry, &bytes).unwrap();
    let (again, o, _) = load(&mount, &options, Some(&cache));
    assert_eq!(o, Outcome::Miss, "a damaged entry is a miss");
    assert_eq!(digest(&again), digest(&first));
    assert_eq!(
        load(&mount, &options, Some(&cache)).1,
        Outcome::Hit,
        "and is written again"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
