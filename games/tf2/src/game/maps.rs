//! TF2's maps: Source's BSP conversion (`source_engine::content::bsp`)
//! with TF2's data (`TF2`): its id, its converter's version (`CONVERTER`,
//! this crate's build.rs), its spawn points (`team_spawn`) and its
//! `sv_gravity` (800). `MAP_LOADER` loads a map of the install
//! (`tf2:<name>`) as an app's `map::MapSources` asks. TF2's maps store
//! most lumps, the static props' game lump (version 10) and the pak's
//! files LZMA-compressed (specs/tf2/install.md 2.1): Source's shared
//! conversion reads all three.
//!
//! Spawns (install.md 2.3): every `info_player_teamspawn` of team 2 (RED,
//! our team 1) or 3 (BLU, our team 2), as CS:S's terrorists and
//! counter-terrorists are 1 and 2; not those that start disabled
//! (`StartDisabled`), are for anyone (`TeamNum` 0: the match summary's
//! places) or only for players a trigger sends there (`SpawnMode` 1). On
//! control-point maps a spawn naming a point (`controlpoint`) is used
//! while its team owns that point: at round start, the point's
//! `point_default_owner`. So ctf_2fort has 15 and 15, cp_badlands 16 and
//! 16 (its home spawns).
//!
//! Its kit is empty for now: no bodies, held or view models, HUD or
//! decals (M2, M3); its sounds are Source's sound scripts and the map's
//! soundscapes.
//!
//! Known shortcuts:
//! - Spawns are the round start's: a point captured later doesn't move a
//!   team's spawns forward, and multi-stage maps' per-round spawns
//!   (`round_redspawn`, `round_bluespawn`) aren't read.
//! - The `spawnflags` class mask is ignored (no classes yet).

use source_engine::content::{
    bsp::{self, Kit, SourceGame},
    material::MaterialLoader,
    props,
    surfaceprops::SurfaceProps,
};

use crate::{
    core::Team,
    map::{MapData, MapEntity, MapLoadOptions, MapLoader, sound::MapSounds},
};

/// The converter's version for the converted-map cache: the hash of this
/// crate's sources, Source's (`source_engine`'s and `source_formats`')
/// and the lockfile (build.rs).
pub const CONVERTER: &str = env!("CHIMERA_CONVERTER_SOURCES");

/// TF2's `sv_gravity` (units/s², specs/tf2/movement.md).
pub const GRAVITY: f32 = 800.0;

/// TF2's team numbers in its maps (install.md, "Units and conventions").
pub const RED: i32 = 2;
pub const BLU: i32 = 3;

/// TF2 as a Source game for the map conversion.
pub static TF2: SourceGame = SourceGame {
    id: super::GAME,
    converter: CONVERTER,
    // Its spawn points are all one class with the team as a key:
    // `team_spawn`.
    spawns: &[],
    spawn_rule: team_spawn,
    gravity: GRAVITY,
    kit: no_kit,
    extra_particles: no_extra_particles,
    held: no_kit,
    sounds,
    fixed_models: props::no_fixed_models,
};

/// Our team for TF2's team number: RED (2) is 1, BLU (3) is 2.
pub fn team(team_num: i32) -> Option<Team> {
    match team_num {
        RED => Some(Team(1)),
        BLU => Some(Team(2)),
        _ => None,
    }
}

/// Whether `e` is a spawn point at round start, and for whom
/// (`SourceGame::spawn_rule`; the module's doc): an enabled
/// `info_player_teamspawn` of RED or BLU, not a triggered one, whose
/// control point (if it names one that `all` holds) its team owns at the
/// start.
pub fn team_spawn(e: &MapEntity, all: &[MapEntity]) -> Option<Option<Team>> {
    if !e.classname().eq_ignore_ascii_case("info_player_teamspawn") {
        return None;
    }
    let int = |e: &MapEntity, key: &str| e.get(key).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
    if int(e, "StartDisabled") != 0 || int(e, "SpawnMode") != 0 {
        return None;
    }
    let team_num = int(e, "TeamNum");
    let ours = team(team_num)?;
    if let Some(point) = e.get("controlpoint").map(str::trim).filter(|p| !p.is_empty()) {
        let owner = all
            .iter()
            .find(|p| {
                p.classname().eq_ignore_ascii_case("team_control_point")
                    && p.get("targetname").is_some_and(|n| n.eq_ignore_ascii_case(point))
            })
            .map(|p| int(p, "point_default_owner"));
        if owner.is_some_and(|o| o != team_num) {
            return None;
        }
    }
    Some(Some(ours))
}

/// TF2's maps from the install (`map::MapLoader`): what an app that plays
/// them chooses (`map::MapSources`).
pub static MAP_LOADER: MapLoader = MapLoader {
    game: super::GAME,
    load: load_installed_map,
};

/// Map `name` (`ctf_2fort`) from the install (`mount::install`: Steam,
/// `[games.tf2] path` or `$CHIMERA_TF2_ROOT`) with `options`.
pub fn load_installed_map(name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let install = super::INFO.install().ok_or_else(|| {
        format!(
            "no `{}` install found (Steam, [games.tf2] path in chimera.local.toml, or $CHIMERA_TF2_ROOT)",
            super::GAME
        )
    })?;
    let mount = super::mount::open(&install).map_err(|e| e.to_string())?;
    load_level_with(&mount, name, options)
}

/// Map `name` from `mount` with `options`, through the converted-map
/// cache.
pub fn load_level_with(mount: &crate::mount::Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    bsp::load_level_with(&TF2, mount, name, options)
}

/// Map `name` from its file's `bytes` (a copy downloaded from a server),
/// its content from `mount`, through the converted-map cache.
pub fn load_level_bytes_with(
    mount: &crate::mount::Mount,
    name: &str,
    bytes: Vec<u8>,
    options: &MapLoadOptions,
) -> Result<MapData, String> {
    bsp::load_level_bytes_with(&TF2, mount, name, bytes, options)
}

/// No kit of its own yet (bodies, HUD, held and view models: M2, M3).
fn no_kit(_: &mut Kit) {}

fn no_extra_particles(_: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    (&[], &[])
}

/// TF2's sounds: Source's sound scripts (the map's own entries and its
/// surfaces'), then the map's soundscapes.
fn sounds(materials: &mut MaterialLoader, bsp: &vbsp::Bsp, name: &str, surfaces: &SurfaceProps, entities: &[MapEntity]) -> MapSounds {
    let mut out = source_engine::content::sound::load_with(materials, name, surfaces, entities, std::iter::empty(), true);
    source_engine::content::soundscape::load(materials, bsp, name, &mut out);
    out
}
