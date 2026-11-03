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
//!   decals and looks (M10). Its sounds (Source's sound scripts and
//!   soundscapes, lift S5) are the entries it plays so far (`SOUNDS`:
//!   movement, the suit, pickups and chargers); its weapons' come with
//!   them (M4).
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
    sounds,
};

/// The sound entries HL2:DM plays whatever the map (beyond the
/// surfaces' and the map's own): the model footsteps, the ladder, the
/// suit's (sprint, drowning, flashlight), pickups and chargers
/// (specs/hl2dm/movement.md, suit.md).
pub const SOUNDS: &[&str] = &[
    "NPC_Citizen.RunFootstepLeft",
    "NPC_Citizen.RunFootstepRight",
    "NPC_CombineS.RunFootstepLeft",
    "NPC_CombineS.RunFootstepRight",
    "NPC_MetroPolice.RunFootstepLeft",
    "NPC_MetroPolice.RunFootstepRight",
    "Ladder.StepRight",
    "HL2Player.SprintStart",
    "HL2Player.SprintNoPower",
    "HL2Player.FlashLightOn",
    "HL2Player.FlashLightOff",
    "HL2Player.Use",
    "HL2Player.UseDeny",
    "Player.FallDamage",
    "Player.DrownStart",
    "Player.DrownContinue",
    "HealthKit.Touch",
    "HealthVial.Touch",
    "ItemBattery.Touch",
    "AlyxEMP.Charge",
    "SuitRecharge.Start",
    "SuitRecharge.ChargingLoop",
    "SuitRecharge.Deny",
    "WallHealth.Start",
    "WallHealth.LoopingContinueCharge",
    "WallHealth.Deny",
    "WallHealth.Recharge",
];

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

/// Map `name` from its file's `bytes` (a copy downloaded from a server),
/// its content from `mount`, through the converted-map cache.
pub fn load_level_bytes_with(
    mount: &crate::mount::Mount,
    name: &str,
    bytes: Vec<u8>,
    options: &MapLoadOptions,
) -> Result<MapData, String> {
    bsp::load_level_bytes_with(&HL2DM, mount, name, bytes, options)
}

fn no_kit(_: &mut Kit) {}

fn no_extra_particles(_: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    (&[], &[])
}

/// HL2:DM's sounds: Source's sound scripts with the entries HL2:DM plays
/// (`SOUNDS`), then the map's soundscapes.
fn sounds(materials: &mut MaterialLoader, bsp: &vbsp::Bsp, name: &str, surfaces: &SurfaceProps, entities: &[MapEntity]) -> MapSounds {
    let mut out =
        source_engine::content::sound::load(materials, name, surfaces, entities, SOUNDS.iter().map(|s| s.to_string()));
    source_engine::content::soundscape::load(materials, bsp, name, &mut out);
    out
}
