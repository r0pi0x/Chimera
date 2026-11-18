//! The map converter's version for the converted-map cache
//! (`game::maps::CONVERTER`): a number bumped by hand (`VERSION`) and the
//! hash of what shapes the converted output (`CONVERTING`: the map
//! converter, the donors' bodies, views, effects and sounds, Blam's
//! formats; not the gameplay) and of the parsing crates' versions
//! (`PARSERS`). Policy: docs/plans/active/converted-cache.md, "Converter
//! versions".

use std::path::Path;

// Chimera's source hash (chimera_vfs), compiled in rather than a build
// dependency, so the build graph stays as it was.
#[allow(dead_code)]
#[path = "../../chimera/vfs/src/source_hash.rs"]
mod source_hash;

/// The converter's number (docs/plans/active/converted-cache.md,
/// "Converter versions"): bump it when what the conversion gives changes
/// through code or data outside `CONVERTING` (a gun's model path in its
/// data, a gameplay module a dressing calls); the converted-output test
/// (party_kit `heavy::converted::converted_output_matches_its_version`)
/// fails until it is.
const VERSION: u32 = 1;

/// What shapes the converted output (module doc).
const CONVERTING: &[&str] = &[
    "src/game/maps.rs",
    "src/game/maps",
    "src/game/body",
    "src/game/view",
    "src/game/fx",
    "src/game/audio",
    "../../blam/formats/src",
];

/// The parsing crates whose versions count (`source_hash::lock_versions`).
const PARSERS: &[&str] = &["flate2", "glam", "postcard"];

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
