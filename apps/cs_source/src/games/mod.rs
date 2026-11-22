//! The app's games: `load_map("game:name")` dispatcher. A game module
//! never uses another game's module (enforced by checks/tests/it/architecture.rs).
//!
//! `cs_source` is the `cs_source` crate's parts (games/cs_source,
//! `cs_source::game`) with the game's front end (`cs_source_ui`).

/// Counter-Strike: Source: its parts (crate `cs_source`, games/cs_source)
/// at their old paths, and its front end (`ui`, `cs_source_ui`; the
/// crate's `present` feature).
pub mod cs_source {
    pub use ::cs_source::game::*;
    // The front end: built on the client shell (the `present` feature).
    #[cfg(feature = "present")]
    pub use ::cs_source_ui as ui;
}

use crate::{
    map::{MapData, MapLoadOptions},
    mount::config::LocalConfig,
};

/// Where `game` is installed: CS:S by its Steam app too
/// (`GameInfo::install`), another by the configured or saved path.
fn install_of(game: &str) -> Option<std::path::PathBuf> {
    match game {
        cs_source::GAME => cs_source::INFO.install(),
        other => LocalConfig::load().ok()?.game_path(other),
    }
}

/// Load a map by namespaced ID, e.g. `cs_source:de_dust2`, from the
/// install (`mount::install`: chimera.local.toml, saved, or Steam).
pub fn load_map(id: &str) -> Result<MapData, String> {
    load_map_level(id, 0)
}

/// `load_map` at a Source `mat_hdr_level` (0 LDR, 1 bloom, 2 HDR); games
/// without HDR data ignore it.
pub fn load_map_level(id: &str, hdr_level: u8) -> Result<MapData, String> {
    load_map_with(id, &MapLoadOptions::hdr(hdr_level))
}

/// `load_map_level` with `options`: the dedicated server's are
/// `MapLoadOptions::lean` (no textures).
pub fn load_map_with(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let (game, name) = id
        .split_once(':')
        .ok_or_else(|| format!("map ID `{id}` should look like game:name"))?;
    match game {
        // CS:S's loader (`cs_source::MAP_LOADER`), as a mashup app uses it.
        cs_source::GAME => (cs_source::MAP_LOADER.load)(name, options),
        other => Err(format!("no map loader for `{other}` yet")),
    }
}

/// `load_map_level` from a file of the map (a downloaded copy), its
/// content from the game's install.
pub fn load_map_file(id: &str, file: &std::path::Path, hdr_level: u8) -> Result<MapData, String> {
    load_map_file_with(id, file, &MapLoadOptions::hdr(hdr_level))
}

/// `load_map_file` with `options`.
pub fn load_map_file_with(id: &str, file: &std::path::Path, options: &MapLoadOptions) -> Result<MapData, String> {
    let (game, name) = id
        .split_once(':')
        .ok_or_else(|| format!("map ID `{id}` should look like game:name"))?;
    let install = install_of(game)
        .ok_or_else(|| format!("no `{game}` install found (chimera_install <folder>, or chimera.local.toml)"))?;
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    match game {
        cs_source::GAME => {
            let mount = cs_source::mount::open(&install).map_err(|e| e.to_string())?;
            cs_source::bsp::load_level_bytes_with(&mount, name, bytes, options)
        }
        other => Err(format!("no map loader for `{other}` yet")),
    }
}

/// The bytes of map `id`'s file as `load_map` reads it (the install, its
/// downloads, Chimera's content cache), for the network's map offer and
/// hash checks (`net::maps::MapFiles`). None if there's none.
pub fn map_file_bytes(id: &str) -> Option<Vec<u8>> {
    let (game, name) = id.split_once(':')?;
    let install = install_of(game)?;
    match game {
        cs_source::GAME => cs_source::mount::open(&install)
            .ok()?
            .read(&format!("maps/{name}.bsp"))
            .ok(),
        _ => None,
    }
}
