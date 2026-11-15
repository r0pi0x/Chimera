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
//! Its kit (M2b): its four guns' view models (the class's arms with the
//! `c_` weapon bone-merged: `view::load`), held models (the same `c_`
//! models, bone-merged by the arms' `weapon_bone`) and the rocket's
//! model (`held_kit`); no bodies, HUD or decals yet (M3). Its sounds are
//! Source's sound scripts (the map's own entries, its surfaces' and the
//! guns', `weapon_sounds`) and the map's soundscapes. On another game's
//! map its guns come with the same (`dress_guest_with`).
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
    held: held_kit,
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

/// No body kit of its own yet (bodies, HUD: M3).
fn no_kit(_: &mut Kit) {}

/// The guns' sound entries (weapons.md 8.3; `sets::M2_WEAPONS`).
pub fn weapon_sounds() -> impl Iterator<Item = String> {
    super::sets::M2_WEAPONS.iter().flat_map(|w| w.sounds.iter().map(|s| s.to_string()))
}

/// What players hold on TF2's maps: its guns' view and held models
/// (`add_guns`).
fn held_kit(kit: &mut Kit) {
    let mut guest = crate::map::GameKit::default();
    let wants = |g: &str| kit.kit.wants(g);
    if super::weapons::GUNS.iter().any(|g| wants(g)) {
        super::weapons::script_numbers::note_sources(kit.materials.mount());
    }
    add_guns(kit.materials, &wants, &mut guest, &mut kit.data.warnings);
    kit.data.kit.held.extend(guest.held);
    kit.data.kit.view_models.extend(guest.view_models);
}

/// TF2's guns that `wants`, into `into`: their view models
/// (`view::load`), their `c_` models held (bone-merged by the first bone
/// they share with their class's arms, `weapon_bone`) and their
/// projectiles' models (`weapons::PROJECTILES`: the rocket, the grenade,
/// the sticky, the needle). What a gun's
/// `weapon/<id>` set holds is what this reads for it (`sets`).
pub fn add_guns(materials: &mut MaterialLoader, wants: &dyn Fn(&str) -> bool, into: &mut crate::map::GameKit, warnings: &mut Vec<String>) {
    use super::weapons::{self as w, PROJECTILES};
    super::view::load(materials, wants, &mut into.view_models, warnings);
    let set = w::tf2_weapons();
    for (gun, arms) in w::arms_by_gun() {
        if !wants(gun) {
            continue;
        }
        let Some(path) = set.weapons.iter().find(|d| d.id == gun).and_then(|d| d.models.world.clone()) else { continue };
        let skeleton = match props::load_view_model(materials, &super::sets::arms_model(&arms), gun, true) {
            Ok(v) => v.bones,
            Err(e) => {
                warnings.push(e);
                continue;
            }
        };
        match props::load_held(materials, &path, gun, &skeleton) {
            Ok(h) => into.held.push(h),
            Err(e) => warnings.push(e),
        }
        for (_, key, model) in PROJECTILES.iter().filter(|(g, ..)| *g == gun) {
            // In its bone's frame, as a held model (its nose then along +X:
            // `chimera:projectile`'s `x_forward`).
            match props::load_held(materials, model, key, &skeleton) {
                Ok(h) => into.held.push(h),
                Err(e) => warnings.push(e),
            }
        }
    }
}

/// TF2's guns on another game's map (the party apps', where a player
/// holding a TF2 gun is TF2's): the guns `kept` names (view and held
/// models: `add_guns`) read from the TF2 install, their textures
/// appended to the map's, and their sounds where the map hasn't an entry
/// of that name. `lean` (a load without textures, the dedicated
/// server's): the textures' sizes only.
pub fn dress_guest_with(map: &mut MapData, kept: &'static [&'static str], lean: bool) -> Result<(), String> {
    let install = super::INFO
        .install()
        .ok_or_else(|| format!("no `{}` install found (Steam, [games.tf2] path, or $CHIMERA_TF2_ROOT)", super::GAME))?;
    let mount = super::mount::open(&install).map_err(|e| e.to_string())?;
    let (textures, cubemaps) = (map.textures.len(), map.cubemaps.len());
    let mut materials = MaterialLoader::guest(&mount, textures, cubemaps).lean(lean);
    let surfaces = SurfaceProps::load(&mut materials);
    let kit = crate::map::KitLoad::Players(kept);
    let wants = |g: &str| kit.wants(g);
    let mut guest = crate::map::GameKit::default();
    let mut warnings = Vec::new();
    if super::weapons::GUNS.iter().any(|g| wants(g)) {
        super::weapons::script_numbers::note_sources(&mount);
    }
    add_guns(&mut materials, &wants, &mut guest, &mut warnings);
    let sounds = source_engine::content::sound::load_with(&mut materials, "", &surfaces, &[], weapon_sounds(), true);
    bsp::add_guest_sounds(std::sync::Arc::make_mut(&mut map.sounds), sounds);
    materials.finish_textures();
    warnings.extend(materials.missing);
    map.textures.extend(materials.textures.split_off(textures));
    map.cubemaps.extend(materials.cubemaps.split_off(cubemaps));
    map.texture_anims.extend(materials.texture_anims);
    map.warnings.extend(warnings);
    map.kit.held.extend(guest.held);
    map.kit.view_models.extend(guest.view_models);
    Ok(())
}

fn no_extra_particles(_: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    (&[], &[])
}

/// TF2's sounds: Source's sound scripts (the map's own entries and its
/// surfaces'), then the map's soundscapes.
fn sounds(materials: &mut MaterialLoader, bsp: &vbsp::Bsp, name: &str, surfaces: &SurfaceProps, entities: &[MapEntity]) -> MapSounds {
    let mut out = source_engine::content::sound::load_with(materials, name, surfaces, entities, weapon_sounds(), true);
    source_engine::content::soundscape::load(materials, bsp, name, &mut out);
    out
}
