//! Counter-Strike 1.6's maps (`MAP_LOADER`, `cs16:<map>`). None yet: the greybox
//! and other games' maps play. A loader returns `MapData` in metres from
//! the mount (docs/new-game.md, step 3; `chimera_world::map`).
//!
//! Known shortcuts:
//! - No map loads (tools/new-game's stand-in).

use crate::{
    map::{MapData, MapLoadOptions, MapLoader},
    mount::Mount,
};

/// The game's maps by name, as the mount has them: none yet.
pub fn map_names(_: &Mount) -> Vec<String> {
    Vec::new()
}

/// Load map `name` (no game prefix).
fn load(name: &str, _: &MapLoadOptions) -> Result<MapData, String> {
    Err(format!("{}:{name}: Counter-Strike 1.6 has no map loader yet (docs/plans/active/cs16.md)", super::GAME))
}

/// The game's map loader.
pub static MAP_LOADER: MapLoader = MapLoader { game: super::GAME, load, words: None };
