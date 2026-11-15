//! Halo's maps: the install's `maps/<name>.map` files as Chimera's
//! `MapData` (specs/halo_ce/cache_file.md, structure_bsp.md). Not written
//! yet: `MAP_LOADER` fails with an error saying so for every map. What is
//! here is the list of maps the install holds (`map_names`).

use crate::{
    map::{MapLoadOptions, MapLoader},
    mount::Mount,
};

use super::{GAME, MULTIPLAYER_MAPS};

/// Halo's maps from the install (`map::MapLoader`), for an app's
/// `map::MapSources`. PLACEHOLDER: every load fails until the loader is
/// written.
pub static MAP_LOADER: MapLoader = MapLoader { game: GAME, load };

fn load(name: &str, _options: &MapLoadOptions) -> Result<crate::map::MapData, String> {
    Err(format!("Halo's map loading is not implemented yet ({GAME}:{name})"))
}

/// A map file's path in the install.
pub fn map_path(name: &str) -> String {
    format!("maps/{name}.map")
}

/// The multiplayer maps the mount holds (`maps/<name>.map`), in the spec's
/// order (`MULTIPLAYER_MAPS`). The campaign maps and the menu's are not
/// listed.
pub fn held(mount: &Mount) -> Vec<String> {
    MULTIPLAYER_MAPS
        .iter()
        .filter(|name| mount.contains(&map_path(name)))
        .map(|name| name.to_string())
        .collect()
}

/// The maps the mounted install holds, by name, sorted (`INFO.map_names`).
pub fn map_names(mount: &Mount) -> Vec<String> {
    let mut names = held(mount);
    names.sort();
    names
}
