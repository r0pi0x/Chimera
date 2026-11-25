//! The map converter's version for the converted-map cache, the kept
//! guest dressing and the served entries (`game::maps::CONVERTER`): a
//! number bumped by hand (`VERSION`) and the hash of what shapes the
//! converted output (`CONVERTING`: the game's maps, bodies and weapons'
//! models and sounds (their data names them), GoldSrc's
//! conversion and formats; not the gameplay) and of the parsing crates'
//! versions (`PARSERS`). Policy: docs/plans/active/converted-cache.md,
//! "Converter versions".

use std::path::Path;

// Chimera's source hash (chimera_vfs), compiled in rather than a build
// dependency, so the build graph stays as it was.
#[allow(dead_code)]
#[path = "../../chimera/vfs/src/source_hash.rs"]
mod source_hash;

/// The converter's number (docs/plans/active/converted-cache.md,
/// "Converter versions"): bump it when what the conversion gives changes
/// through code or data outside `CONVERTING`.
const VERSION: u32 = 3;

/// What shapes the converted output (module doc).
const CONVERTING: &[&str] = &[
    "src/game/maps.rs",
    "src/game/bodies.rs",
    "src/game/weapons/mod.rs",
    "src/game/weapons/looks.rs",
    "data/weapons",
    "../../goldsrc/engine/src",
    "../../goldsrc/formats/src",
];

/// The parsing crates whose versions count (`source_hash::lock_versions`).
const PARSERS: &[&str] = &["image", "glam", "postcard"];

fn main() {
    let lock = Path::new("../../Cargo.lock");
    for p in CONVERTING.iter().chain(&["../../Cargo.lock"]) {
        println!("cargo:rerun-if-changed={p}");
    }
    let dirs: Vec<&Path> = CONVERTING.iter().map(Path::new).collect();
    println!(
        "cargo:rustc-env=CHIMERA_CONVERTER_SOURCES={}",
        source_hash::converter(VERSION, &dirs, lock, PARSERS)
    );
}
