//! The map format's version for the converted-map cache (`map::cache`):
//! the hash of the map types' sources (this crate's `map` module and
//! Chimera's core types it uses), so changing them makes every cached
//! map a miss.

use std::path::Path;

// Chimera's source hash (chimera_vfs), compiled in rather than a build
// dependency, so the build graph stays as it was.
#[allow(dead_code)]
#[path = "../vfs/src/source_hash.rs"]
mod source_hash;

fn main() {
    let paths = ["src/map", "../core/src/core.rs", "../vfs/src/lib.rs"];
    for p in paths {
        println!("cargo:rerun-if-changed={p}");
    }
    let dirs: Vec<&Path> = paths.iter().map(Path::new).collect();
    println!("cargo:rustc-env=CHIMERA_MAP_SOURCES={}", source_hash::hash_dirs(&dirs));
}
