//! The converted-map cache's map side (docs/plans/active/asset-service.md,
//! section 10; the cache itself, keys, entries and the switches, is
//! chimera_assets' `map_cache`, re-exported here): the map format's
//! version every key starts with (`key`: the hash build.rs takes of this
//! crate's `src/map`, Chimera's core types and the vfs, and
//! `FORMAT_BUMP`) and the load options a key adds (`MapKey::options`).
//! Map types serialize with serde (postcard), big arrays as byte strings
//! (`pod_serde`), textures without the texels their BC blocks decode to
//! (`texture_serde`), `MapExtras` through its list of known types; what can't
//! be serialized (`MapData::light_field`, a closure; `load_times`) is the
//! loader's to rebuild on a hit.
//!
//! Known shortcuts: code the converters call outside the folders their
//! `build.rs` hashes (or this crate's `map` module) doesn't change the
//! version: bump `FORMAT_BUMP` when such a change changes what a load
//! gives. A game name read back (`MapCharacterModel::game`) is leaked, a
//! few bytes per character model per hit.

pub use chimera_core::mount::assets::map_cache::*;

use super::MapLoadOptions;

/// Bump for a change in what a load gives that the hashed sources miss
/// (module docs, "Known shortcuts").
pub const FORMAT_BUMP: u32 = 1;

/// `f` run with this thread's instructions (and those of the threads it
/// starts) as the cache's work probe (`with_work`), when
/// `CHIMERA_MAP_CACHE_LOG` is set: a hit inside `f` says its instructions.
pub fn counted<T>(f: impl FnOnce() -> T) -> T {
    if std::env::var_os("CHIMERA_MAP_CACHE_LOG").is_none() {
        return f();
    }
    let mut counter = chimera_core::metrics::InstructionCounter::current_with_children();
    with_work(Box::new(move || counter.read()), f)
}

/// How big each part of `map` is in the cache (measuring;
/// `CHIMERA_MAP_CACHE_LOG=1` prints it after a kept dressing's read):
/// each part encoded alone, its bytes.
pub fn encoded_sizes(map: &super::MapData) -> Vec<(&'static str, usize)> {
    fn one<T: serde::Serialize>(v: &T) -> usize {
        postcard::to_allocvec(v).map_or(0, |b| b.len())
    }
    let kit = &map.kit;
    let animations: Vec<_> = kit.characters.iter().filter_map(|c| c.animations.clone()).collect();
    vec![
        ("textures", one(&map.textures)),
        ("cubemaps", one(&map.cubemaps)),
        ("bodies", one(&kit.characters)),
        ("of them, the bodies' animations", one(&animations)),
        ("held models", one(&kit.held)),
        ("view models", one(&kit.view_models)),
        ("sounds", one(&map.sounds)),
        ("particles", one(&map.particles)),
        ("the whole", one(map)),
    ]
}

/// This crate's part of every key: the map types' sources (build.rs) and
/// `FORMAT_BUMP`.
pub fn format() -> String {
    format!("{} {FORMAT_BUMP}", env!("CHIMERA_MAP_SOURCES"))
}

/// A key of `loader` (`cs_source`) whose converter's version is
/// `converter` (its sources' hash, from its build.rs), in this map format.
pub fn key(loader: &str, converter: &str) -> Key {
    Key::new(loader, converter).part("format", format())
}

/// The load options' part of a key.
pub trait MapKey {
    /// Add what of `options` changes a load: the HDR level, whether
    /// textures are decoded and how much of the kit is read.
    fn options(self, options: &MapLoadOptions) -> Self;
}

impl MapKey for Key {
    fn options(self, options: &MapLoadOptions) -> Self {
        self.part("hdr", [options.hdr_level.unwrap_or(u8::MAX)])
            .part("textures", [options.textures as u8])
            .part("kit", format!("{:?}", options.kit))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;

    /// The map format's version is the hash of `src/map` (build.rs): a
    /// serialized type of this crate outside it would change the format
    /// without changing the version.
    #[test]
    fn every_serialized_map_type_is_in_the_hashed_sources() {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { walk(&p, out) } else { out.push(p) }
            }
        }
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&src, &mut files);
        for f in files.iter().filter(|f| !f.starts_with(src.join("map"))) {
            let text = fs::read_to_string(f).unwrap();
            assert!(
                !text.contains("Serialize"),
                "{}: serialized types of the map belong in src/map (or add the file to build.rs's list)",
                f.display()
            );
        }
        assert_eq!(env!("CHIMERA_MAP_SOURCES").len(), 32);
    }

    #[test]
    fn the_options_and_the_format_are_in_the_key() {
        let k = |o: &MapLoadOptions| key("game", "v1").part("map", "de_x").options(o).hex();
        let full = k(&MapLoadOptions::default());
        assert_eq!(full, k(&MapLoadOptions::default()));
        assert_ne!(full, k(&MapLoadOptions::lean()));
        assert_ne!(full, k(&MapLoadOptions::hdr(2)));
        let mashup = MapLoadOptions {
            kit: super::super::KitLoad::Only(&[]),
            ..Default::default()
        };
        assert_ne!(full, k(&mashup));
        let unformatted = Key::new("game", "v1")
            .part("map", "de_x")
            .options(&MapLoadOptions::default())
            .hex();
        assert_ne!(full, unformatted, "the format is in every map key");
    }
}
