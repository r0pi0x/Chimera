//! The converter's version for the converted-map cache and the kept guest
//! dressing (`game::maps::CONVERTER`): a number bumped by hand (`VERSION`)
//! and the hash of what shapes the converted output (`CONVERTING`: the map
//! converter and its kit, the guest dressing with its bodies and view
//! models, Source's content converters and formats; not the gameplay) and
//! of the parsing crates' versions (`PARSERS`). Policy:
//! docs/plans/active/converted-cache.md, "Converter versions".

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
const VERSION: u32 = 4;

/// What shapes the converted output (module doc).
const CONVERTING: &[&str] = &[
    "src/game/maps.rs",
    "src/game/bodies.rs",
    "src/game/view.rs",
    "src/game/hud.rs",
    "src/game/sets.rs",
    // Source's content converters, the HUD look, the effects' looks,
    // Source's formats and Chimera's (KeyValues, WAV).
    "../../source/engine/src/content",
    "../../source/engine/src/hud/look.rs",
    "../../source/engine/src/effects",
    "../../source/formats/src",
    "../../chimera/formats/src",
];

/// The parsing crates whose versions count (`source_hash::lock_versions`).
const PARSERS: &[&str] = &[
    "vtf", "vmdl", "vbsp", "vmt-parser", "image", "half", "lzma-rs", "bzip2", "glam", "symphonia*", "postcard",
];

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
