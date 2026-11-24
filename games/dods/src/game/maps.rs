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
//! (`dress_guest_in`), so a DoD:S player wears its body there; and the
//! particle materials of Source's explosion and impact effects (its
//! explosives' look), on another game's map those it lacks.
//!
//! Its sounds: Source's sound scripts (the map's own entries and its
//! surfaces') and its guns' (`weapon_sounds`: every entry of the game's
//! weapon sound script, `scripts/game_sounds_weapons.txt`, so the scripts'
//! shots, reloads and swings and the view models' own sound events, the
//! draw, clip and bolt, are all there), the guns' kept in a map's table
//! under `SOUND_PREFIX` (Source games' script names collide, and a table
//! keeps the first of a name; on another game's map only under it); then
//! the map's soundscapes.
//!
//! Known shortcuts (M0):
//! - No impact decals of its own; its effects are Source's shared
//!   explosion only (the HUD is `dods_ui::hud`'s, its pictures `hud`'s).
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

/// Its players' kit: the class bodies, and the effect materials its
/// explosives' look draws with (`weapons::explosives::looks`).
fn body_kit(k: &mut Kit) {
    super::bodies::add_bodies(k.materials, k.surfaces, k.kit, &mut k.data.kit, &mut k.data.warnings);
    k.data.particles = effect_materials(k.materials);
}

/// The particle materials of Source's explosion (`source_engine::effects::
/// explosions`) and the impact effects, from the install.
fn effect_materials(materials: &mut MaterialLoader) -> crate::map::particles::MapParticles {
    source_engine::effects::impact_effects::load_materials(materials, &[source_engine::effects::explosions::MATERIALS])
}

/// What DoD:S's guns draw on another game's map (`dress_guest_in`): its
/// explosions' and impacts' particle materials (`effect_materials`), each
/// the map lacks by name. `chimera_pack sets dods --only base` records
/// what this reads into DoD:S's `base` set, so a player without the
/// install has them before a match (they were in its maps' sets alone:
/// `particle_smokegrenade.vtf` was fetched on demand mid-match).
pub fn add_guest_looks(materials: &mut MaterialLoader, map: &mut MapData) {
    for m in effect_materials(materials).materials {
        if map.particles.find(&m.name).is_none() {
            map.particles.materials.push(m);
        }
    }
}

/// What players hold on its maps: its guns' view and held models.
fn held_kit(k: &mut Kit) {
    let kit = k.kit;
    super::weapons::add_guns(k.materials, k.surfaces, &|g| kit.wants(g), &mut k.data.kit, &mut k.data.warnings);
}

fn no_extra_particles(_: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    (&[], &[])
}

/// Its sounds: Source's sound scripts (the map's own entries, its
/// surfaces' and its guns', `weapon_sounds`), then the map's soundscapes.
fn sounds(materials: &mut MaterialLoader, bsp: &vbsp::Bsp, name: &str, surfaces: &SurfaceProps, entities: &[MapEntity]) -> MapSounds {
    let guns = weapon_sounds(materials.mount());
    let mut out = source_engine::content::sound::load_with(materials, name, surfaces, entities, guns.clone(), true);
    own_weapon_sounds(&mut out, false, &guns);
    source_engine::content::soundscape::load(materials, bsp, name, &mut out);
    out
}

/// The game's weapon sound script (its guns' shots, reloads, swings and
/// the view models' own events: `Weapon_Thompson.Draw`).
pub const WEAPON_SOUNDS: &str = "scripts/game_sounds_weapons.txt";

/// What names DoD:S's own weapon entries in a map's table (TF2's rule:
/// `tf2::game::maps::SOUND_PREFIX`): a Source game's script names collide
/// across games (`Weapon_Knife.Swing`, `Bullets.DefaultNearmiss`), and a
/// map's table keeps the first entry of a name. The guns' data
/// (`weapons::script_numbers`) and the view models' sound events
/// (`view::rules`) name them so.
pub const SOUND_PREFIX: &str = "dods:";

/// `script_name` (an entry of DoD:S's sound scripts) as DoD:S's table
/// holds it.
pub fn own_sound(script_name: &str) -> String {
    format!("{SOUND_PREFIX}{script_name}")
}

/// Every entry of the weapon sound script on `mount` (`WEAPON_SOUNDS`), in
/// the file's order; none without it.
pub fn weapon_sounds(mount: &crate::mount::Mount) -> Vec<String> {
    use chimera_core::mount::keyvalues::{self, Escapes};
    let Ok(bytes) = mount.read(WEAPON_SOUNDS) else { return Vec::new() };
    keyvalues::parse(&String::from_utf8_lossy(&bytes), Escapes::Off)
        .items()
        .iter()
        .map(|(name, _)| name.clone())
        .collect()
}

/// DoD:S's weapon entries in `sounds` copied under `SOUND_PREFIX`
/// (`own_sound`); the bare names stay too unless `guest` (on another
/// game's map they would shadow or be shadowed by that game's).
fn own_weapon_sounds(sounds: &mut MapSounds, guest: bool, guns: &[String]) {
    for name in guns {
        let key = name.to_lowercase();
        let entry = if guest { sounds.entries.remove(&key) } else { sounds.entries.get(&key).cloned() };
        if let Some(entry) = entry {
            sounds.entries.insert(own_sound(name).to_lowercase(), entry);
        }
    }
}

/// The game's files for its guest dressing (`map::kept::Dressing::mount`):
/// the install, mounted.
pub fn guest_mount() -> Result<std::sync::Arc<crate::mount::Mount>, String> {
    super::mount::open(&install()?).map(std::sync::Arc::new).map_err(|e| e.to_string())
}

/// DoD:S's players and guns on another game's map (the party apps'):
/// the guns `kept` names (view and held models) and the class bodies,
/// read from `mount`, their textures appended to the map's, the
/// explosions' particle materials the map lacks, and their
/// guns' sounds (`weapon_sounds`, under `SOUND_PREFIX` only). `lean`: the
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
    add_guest_looks(&mut materials, map);
    let guns = weapon_sounds(mount);
    let mut sounds = source_engine::content::sound::load_with(&mut materials, "", &surfaces, &[], guns.clone(), true);
    own_weapon_sounds(&mut sounds, true, &guns);
    bsp::add_guest_sounds(std::sync::Arc::make_mut(&mut map.sounds), sounds);
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
