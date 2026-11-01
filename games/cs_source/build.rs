//! The map converter's version for the converted-map cache
//! (`game::bsp::CONVERTER`, chimera_world's `map::cache`): the hash of
//! the converter's sources and data, Source's format crate and the
//! lockfile (the parsing crates' versions), so any change to them makes
//! every cached CS:S map a miss.

use std::path::Path;

// Chimera's source hash (chimera_vfs), compiled in rather than a build
// dependency, so the build graph stays as it was.
#[allow(dead_code)]
#[path = "../../chimera/vfs/src/source_hash.rs"]
mod source_hash;

fn main() {
    let paths = ["src/game", "data", "../../source/formats/src", "../../Cargo.lock"];
    for p in paths {
        println!("cargo:rerun-if-changed={p}");
    }
    let dirs: Vec<&Path> = paths.iter().map(Path::new).collect();
    println!(
        "cargo:rustc-env=CHIMERA_CONVERTER_SOURCES={}",
        source_hash::hash_dirs(&dirs)
    );
}
