//! The converter's version for the converted-map cache, the kept guest
//! dressing and the merged weapon documents (`game::world::CONVERTER`): a
//! number bumped by hand (`VERSION`) and the hash of what shapes the
//! converted output (`CONVERTING`: the world converter, the models,
//! characters, effects and view models it converts, the install data's
//! reading, the data they read, LithTech's formats; not the gameplay) and
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
const VERSION: u32 = 1;

/// What shapes the converted output (module doc).
const CONVERTING: &[&str] = &[
    "src/game/world.rs",
    "src/game/model.rs",
    "src/game/props.rs",
    "src/game/render_style.rs",
    "src/game/volumes.rs",
    "src/game/characters.rs",
    "src/game/characters/kept.rs",
    "src/game/characters/models.rs",
    "src/game/characters/records.rs",
    "src/game/characters/skeleton.rs",
    "src/game/characters/families.rs",
    "src/game/effects",
    "src/game/objects/mod.rs",
    "src/game/objects/geometry.rs",
    "src/game/objects/lights.rs",
    "src/game/objects/markers.rs",
    "src/game/objects/sound.rs",
    "src/game/view_model.rs",
    "src/game/view_model/hands.rs",
    "src/game/view_model/parts.rs",
    "src/game/view_model/attached.rs",
    "src/game/data.rs",
    "src/game/data/attributes.rs",
    "src/game/data/store.rs",
    "data/effects.toml",
    "data/render_styles.toml",
    "data/hands.toml",
    "data/surfaces.toml",
    "data/debris.toml",
    "data/npc_bodies.toml",
    "../../lithtech/formats/src",
    "../../chimera/formats/src",
];

/// The parsing crates whose versions count (`source_hash::lock_versions`).
const PARSERS: &[&str] = &["aes", "glam", "symphonia*", "postcard"];

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
