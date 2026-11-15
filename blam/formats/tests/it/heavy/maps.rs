//! Loading a multiplayer map of the user's Halo disc image for the heavy
//! tests of the tag readers (scenario, shaders): `load(name)` inflates
//! `maps/<name>.map` out of the image, or returns `None` with a note when
//! there is no image.

use std::sync::OnceLock;

use blam_formats::cache::MapFile;
use blam_formats::disc::DiscImage;

use super::disc_install::{SKIP, image_path};

/// The 13 multiplayer maps of the disc.
pub const MULTIPLAYER: [&str; 13] = [
    "beavercreek",
    "bloodgulch",
    "boardingaction",
    "carousel",
    "chillout",
    "damnation",
    "hangemhigh",
    "longest",
    "prisoner",
    "putput",
    "ratrace",
    "sidewinder",
    "wizard",
];

fn disc() -> Option<&'static DiscImage> {
    static DISC: OnceLock<Option<DiscImage>> = OnceLock::new();
    let d = DISC
        .get_or_init(|| image_path().map(|p| DiscImage::open(&p).unwrap()))
        .as_ref();
    if d.is_none() {
        eprintln!("{SKIP}");
    }
    d
}

/// The map `name` (`bloodgulch`), or `None` without a disc image.
pub fn load(name: &str) -> Option<MapFile> {
    let file = disc()?.read_file(&format!("maps/{name}.map")).unwrap();
    Some(MapFile::load(&file).unwrap())
}
