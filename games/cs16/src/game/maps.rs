//! Counter-Strike 1.6's maps (`MAP_LOADER`, `cs16:<map>`): GoldSrc's BSP
//! conversion (`goldsrc_engine::content::maps`) with CS 1.6's data
//! (`CS16`): its id, its converter's version (`CONVERTER`, this crate's
//! build.rs), its spawn points, its gravity and its kit (the player
//! bodies, `bodies::add_bodies`). The maps listed are the game folder's
//! (`map_names`: `maps/*.bsp` served by the layer that serves de_dust2,
//! so Half-Life's own maps in `valve/` stay out of pools and pickers; any
//! map of the chain still loads by id).
//!
//! Spawns (specs/goldsrc/bsp.md, "Entities"): `info_player_deathmatch`
//! the Terrorists' (our team 1), `info_player_start` the
//! Counter-Terrorists' (our team 2), as CS:S's sides.
//!
//! Known shortcuts:
//! - `sv_gravity` is 800 (`GRAVITY`), Half-Life's default, until
//!   specs/cs16/movement.md lists the server's.
//! - Its objectives (bomb targets, buy zones, hostages) are entities only
//!   (`goldsrc_engine::logic` keeps them as data; no rule reads them).

use goldsrc_engine::content::maps::{self as gmaps, GoldSrcGame};

use crate::{
    core::Team,
    map::{KitLoad, MapData, MapLoadOptions, MapLoader, MapWords},
    mount::Mount,
};

/// The converter's version for the converted-map cache, the kept guest
/// dressing and the served converted entries: `v<number>-<hash>`
/// (build.rs; docs/plans/active/converted-cache.md, "Converter versions").
pub const CONVERTER: &str = env!("CHIMERA_CONVERTER_SOURCES");

/// `sv_gravity`, units/s² (module doc).
pub const GRAVITY: f32 = 800.0;

/// Its spawn points: the Terrorists' (our team 1) and the
/// Counter-Terrorists' (our team 2).
pub const SPAWNS: &[(&str, Option<Team>)] = &[("info_player_deathmatch", Some(Team(1))), ("info_player_start", Some(Team(2)))];

/// CS 1.6 as a GoldSrc game for the map conversion.
pub static CS16: GoldSrcGame = GoldSrcGame {
    id: super::GAME,
    converter: CONVERTER,
    spawns: SPAWNS,
    gravity: GRAVITY,
    kit: own_kit,
};

/// The game's maps from the install.
pub static MAP_LOADER: MapLoader = MapLoader { game: super::GAME, load, words: Some(&WORDS) };

/// CS 1.6's words for what maps of different games share: the
/// Terrorists (1) the first side, the Counter-Terrorists (2) the second;
/// its maps' surfaces are already the neutral kinds
/// (`goldsrc_engine::content::brushes::surface_kind`).
pub static WORDS: MapWords = MapWords {
    teams: &[(1, crate::map::SIDE_FIRST), (2, crate::map::SIDE_SECOND)],
    surfaces: Some(chimera_core::game_data::GameData::from_text("")),
};

/// Its kit on its own maps: the player bodies.
fn own_kit(mount: &Mount, kit: KitLoad, decode: bool, data: &mut MapData) {
    if kit.bodies() {
        super::bodies::add_bodies(mount, decode, data);
    }
}

/// The game folder's maps by name, sorted (module doc).
pub fn map_names(mount: &Mount) -> Vec<String> {
    let own = mount.layer_of(&chimera_core::mount::normalize("maps/de_dust2.bsp")).map(|(i, _)| i);
    gmaps::map_names(mount)
        .into_iter()
        .filter(|n| {
            own.is_none() || mount.layer_of(&chimera_core::mount::normalize(&gmaps::map_path(n))).map(|(i, _)| i) == own
        })
        .collect()
}

/// The game's install, mounted, or why there is none.
pub fn installed_mount() -> Result<Mount, String> {
    let install = super::INFO.install().ok_or_else(|| {
        format!(
            "no `{}` install found (Steam, [games.cs16] path in chimera.local.toml, or $CHIMERA_CS16_ROOT)",
            super::GAME
        )
    })?;
    super::mount::open(&install).map_err(|e| format!("{}: {e}", install.display()))
}

/// Load map `name` (no game prefix) from the install.
fn load(name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    load_with(&installed_mount()?, name, options)
}

/// Map `name` from `mount` with `options`, through the converted-map cache.
pub fn load_with(mount: &Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    gmaps::load_level_with(&CS16, mount, name, options)
}
