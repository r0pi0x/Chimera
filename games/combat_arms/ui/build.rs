//! The version of the install's string table and art as read
//! (`install::UI_READER`: `install::strings` and `install::decode`,
//! which `install::strings_and_art` keeps in the converted cache): the
//! hash of what shapes them (`READING`: the reading, the string table's
//! parser, LithTech's formats) and of the cache's encoding's version.
//! Policy: docs/plans/active/converted-cache.md, "Converter versions".

use std::path::Path;

// Chimera's source hash (chimera_vfs), compiled in rather than a build
// dependency, so the build graph stays as it was.
#[allow(dead_code)]
#[path = "../../../chimera/vfs/src/source_hash.rs"]
mod source_hash;

/// Bump when what is kept changes through code outside `READING`.
const VERSION: u32 = 1;

/// What shapes the kept strings and art (module doc).
const READING: &[&str] = &["src/install.rs", "src/layout/text.rs", "../../../lithtech/formats/src"];

fn main() {
    let lock = Path::new("../../../Cargo.lock");
    for p in READING.iter().chain(&["../../../Cargo.lock"]) {
        println!("cargo:rerun-if-changed={p}");
    }
    let dirs: Vec<&Path> = READING.iter().map(Path::new).collect();
    println!("cargo:rustc-env=CHIMERA_CA_UI_SOURCES={}", source_hash::converter(VERSION, &dirs, lock, &["postcard"]));
}
