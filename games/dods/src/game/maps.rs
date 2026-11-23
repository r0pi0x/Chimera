//! DoD:S's maps: Source's BSP conversion (`source_engine::content::bsp`)
//! with DoD:S's data (`DODS`): its id, its converter's version
//! (`CONVERTER`, this crate's build.rs), its spawn points and its
//! `sv_gravity` (800). `MAP_LOADER` loads a map of the install
//! (`dods:<name>`) as an app's `map::MapSources` asks. The nine stock maps
//! are BSP version 20 (specs/dods/install.md, "Maps").
//!
//! Spawns (install.md, "Maps"): `info_player_allies` (team 2, our team 1)
//! and `info_player_axis` (team 3, our team 2), 20 of each on
//! dod_avalanche and dod_flash.
//!
//! Its kit: the class bodies (`bodies::add_bodies`) and its guns' view
//! and held models (`weapons::add_guns`); on another game's map the same
//! (`dress_guest_in`), so a DoD:S player wears its body there.
//!
//! Known shortcuts (M0):
//! - No HUD look, impact decals or effects of its own; sounds are the
//!   map's and its surfaces' (Source's sound scripts), none for its guns.
//! - Its objectives (`dod_control_point`, `dod_capture_area`, bomb
//!   targets) and team-only walls (`func_teamblocker`) are not read.

use source_engine::content::{
    bsp::{self, Kit, SourceGame},
    material::MaterialLoader,
    surfaceprops::SurfaceProps,
};

use crate::{
    core::Team,
    map::{MapData, MapEntity, MapLoadOptions, MapLoader, MapWords, sound::MapSounds},
};

/// The converter's version for the converted-map cache, the kept guest
/// dressing and the served converted entries: `v<number>-<hash>`
/// (build.rs; docs/plans/active/converted-cache.md, "Converter versions").
pub const CONVERTER: &str = env!("CHIMERA_CONVERTER_SOURCES");

/// DoD:S's `sv_gravity` (units/s², specs/dods/movement.md).
pub const GRAVITY: f32 = 800.0;

/// Its spawn points: the Allies' (our team 1) and the Axis' (our team 2).
pub const SPAWNS: &[(&str, Option<Team>)] = &[("info_player_allies", Some(Team(1))), ("info_player_axis", Some(Team(2)))];

/// DoD:S as a Source game for the map conversion.
pub static DODS: SourceGame = SourceGame {
    id: super::GAME,
    converter: CONVERTER,
    spawns: SPAWNS,
    spawn_rule: bsp::no_spawn_rule,
    gravity: GRAVITY,
    kit: body_kit,
    extra_particles: no_extra_particles,
    held: held_kit,
    sounds,
    fixed_models: source_engine::content::props::no_fixed_models,
    entity_rule: bsp::no_entity_rule,
    // No HDR spec yet: the stand-in (docs/backlog.md "HDR parity").
    hdr_model: crate::map::HdrModel::StandIn,
    hdr_default_level: 0,
};

/// DoD:S's maps from the install (`map::MapLoader`).
pub static MAP_LOADER: MapLoader = MapLoader {
    game: super::GAME,
    load: load_installed_map,
    words: Some(&WORDS),
};

/// DoD:S's words for what maps of different games share (`map::MapWords`):
/// the Allies (1) the first side, the Axis (2) the second; its surfaces
/// Source's (`source_engine::maps::SURFACE_WORDS`).
pub static WORDS: MapWords = MapWords {
    teams: &[(1, crate::map::SIDE_FIRST), (2, crate::map::SIDE_SECOND)],
    surfaces: Some(source_engine::maps::SURFACE_WORDS),
};

/// The game's install, or why there is none.
fn install() -> Result<std::path::PathBuf, String> {
    super::INFO.install().ok_or_else(|| {
        format!(
            "no `{}` install found (Steam, [games.dods] path in chimera.local.toml, or $CHIMERA_DODS_ROOT)",
            super::GAME
        )
    })
}

/// Map `name` (`dod_avalanche`) from the install (`mount::install`:
/// Steam, `[games.dods] path` or `$CHIMERA_DODS_ROOT`) with `options`.
pub fn load_installed_map(name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let mount = super::mount::open(&install()?).map_err(|e| e.to_string())?;
    load_level_with(&mount, name, options)
}

/// Map `name` from `mount` with `options`, through the converted-map
/// cache.
pub fn load_level_with(mount: &crate::mount::Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    bsp::load_level_with(&DODS, mount, name, options)
}

/// Its players' kit: the class bodies.
fn body_kit(k: &mut Kit) {
    super::bodies::add_bodies(k.materials, k.surfaces, k.kit, &mut k.data.kit, &mut k.data.warnings);
}

/// What players hold on its maps: its guns' view and held models.
fn held_kit(k: &mut Kit) {
    let kit = k.kit;
    super::weapons::add_guns(k.materials, k.surfaces, &|g| kit.wants(g), &mut k.data.kit, &mut k.data.warnings);
}

fn no_extra_particles(_: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    (&[], &[])
}

/// Its sounds: Source's sound scripts (the map's own entries and its
/// surfaces'), then the map's soundscapes.
fn sounds(materials: &mut MaterialLoader, bsp: &vbsp::Bsp, name: &str, surfaces: &SurfaceProps, entities: &[MapEntity]) -> MapSounds {
    let mut out = source_engine::content::sound::load_with(materials, name, surfaces, entities, std::iter::empty(), true);
    source_engine::content::soundscape::load(materials, bsp, name, &mut out);
    out
}

/// The game's files for its guest dressing (`map::kept::Dressing::mount`):
/// the install, mounted.
pub fn guest_mount() -> Result<std::sync::Arc<crate::mount::Mount>, String> {
    super::mount::open(&install()?).map(std::sync::Arc::new).map_err(|e| e.to_string())
}

/// DoD:S's players and guns on another game's map (the party apps'):
/// the guns `kept` names (view and held models) and the class bodies,
/// read from `mount`, their textures appended to the map's. `lean`: the
/// textures' sizes only.
pub fn dress_guest_in(mount: &crate::mount::Mount, map: &mut MapData, kept: &'static [&'static str], lean: bool) {
    let (textures, cubemaps) = (map.textures.len(), map.cubemaps.len());
    let blocks_only = map.memos.get::<crate::map::BlocksOnly>().is_some();
    let mut materials = MaterialLoader::guest(mount, textures, cubemaps).lean(lean).blocks_only(blocks_only);
    let surfaces = SurfaceProps::load(&mut materials);
    let kit = crate::map::KitLoad::Players(kept);
    let mut guest = crate::map::GameKit::default();
    let mut warnings = Vec::new();
    super::bodies::add_bodies(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
    super::weapons::add_guns(&mut materials, &surfaces, &|g| kit.wants(g), &mut guest, &mut warnings);
    materials.finish_textures();
    warnings.extend(materials.missing);
    map.textures.extend(materials.textures.split_off(textures));
    map.cubemaps.extend(materials.cubemaps.split_off(cubemaps));
    map.texture_anims.extend(materials.texture_anims);
    map.warnings.extend(warnings);
    map.kit.characters.extend(guest.characters);
    map.kit.held.extend(guest.held);
    map.kit.view_models.extend(guest.view_models);
}
