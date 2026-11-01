//! HL2:DM's maps: Source's BSP conversion (`source_engine::content::bsp`,
//! lifted from CS:S in M1) with HL2:DM's data (`HL2DM`): its id, its
//! converter's version (`CONVERTER`, this crate's build.rs), its spawn
//! points (`info_player_deathmatch`: anyone; specs/hl2dm/source_deltas.md,
//! "Stock maps") and its `sv_gravity` (600: physics bodies, ropes and gibs
//! fall at it; source_deltas.md 1.1). `MAP_LOADER` loads a map of the
//! install (`hl2dm:<name>`) as an app's `map::MapSources` asks.
//!
//! Known shortcuts:
//! - HL2:DM's kit is empty: no player bodies (M8, specs/hl2dm/
//!   player_models.md), HUD (M9), held or view models (M4), impact
//!   decals and looks (M10); the maps' sound entries are empty and their
//!   soundscapes unread (sound scripts are CS:S's until lift S5, M3), so
//!   the maps are silent.
//! - Team spawns (`info_player_combine`, `info_player_rebel`) and the
//!   `info_player_start` fallback aren't spawn points yet: deathmatch's
//!   spawns only (H6's spawn choice comes with the rules, M7).

use source_engine::content::{
    bsp::{self, Kit, SourceGame},
    material::MaterialLoader,
    surfaceprops::SurfaceProps,
};

use crate::map::{MapData, MapEntity, MapLoadOptions, MapLoader, sound::MapSounds};

/// The converter's version for the converted-map cache: the hash of this
/// crate's sources and data, Source's (`source_engine`'s and
/// `source_formats`') and the lockfile (build.rs).
pub const CONVERTER: &str = env!("CHIMERA_CONVERTER_SOURCES");

/// HL2:DM's `sv_gravity` (units/s², specs/hl2dm/source_deltas.md 1.1).
pub const GRAVITY: f32 = 600.0;

/// HL2:DM's spawn points: deathmatch's, for anyone.
pub const SPAWNS: &[(&str, Option<crate::core::Team>)] = &[("info_player_deathmatch", None)];

/// HL2:DM as a Source game for the map conversion.
pub static HL2DM: SourceGame = SourceGame {
    id: super::GAME,
    converter: CONVERTER,
    spawns: SPAWNS,
    gravity: GRAVITY,
    kit: no_kit,
    extra_particles: no_extra_particles,
    held: no_kit,
    sounds: no_sounds,
};

/// HL2:DM's maps from the install (`map::MapLoader`): what an app that
/// plays them chooses (`map::MapSources`).
pub static MAP_LOADER: MapLoader = MapLoader {
    game: super::GAME,
    load: load_installed_map,
};

/// Map `name` (`dm_lockdown`) from the install (`mount::install`: Steam,
/// `[games.hl2dm] path` or `$CHIMERA_HL2DM_ROOT`) with `options`.
pub fn load_installed_map(name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let install = super::INFO.install().ok_or_else(|| {
        format!(
            "no `{}` install found (Steam, [games.hl2dm] path in chimera.local.toml, or $CHIMERA_HL2DM_ROOT)",
            super::GAME
        )
    })?;
    let mount = super::mount::open(&install).map_err(|e| e.to_string())?;
    load_level_with(&mount, name, options)
}

/// Map `name` from `mount` with `options`, through the converted-map
/// cache.
pub fn load_level_with(mount: &crate::mount::Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    bsp::load_level_with(&HL2DM, mount, name, options)
}

fn no_kit(_: &mut Kit) {}

fn no_extra_particles(_: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    (&[], &[])
}

fn no_sounds(_: &mut MaterialLoader, _: &vbsp::Bsp, _: &str, _: &SurfaceProps, _: &[MapEntity]) -> MapSounds {
    MapSounds::default()
}
