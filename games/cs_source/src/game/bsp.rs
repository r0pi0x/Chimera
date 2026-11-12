//! CS:S's maps: Source's BSP conversion (`source_engine::content::bsp`,
//! lift S2 of docs/plans/active/hl2dm.md, re-exported here with its
//! neighbours at their old paths) with CS:S's data and hooks
//! (`CS_SOURCE`): its converter's version (`CONVERTER`, this crate's
//! build.rs), its spawn classes and teams (`SPAWNS`), its gravity
//! (`GRAVITY`, 800), the kit CS:S's maps carry (`kit`: the players' bodies by
//! class (`CLASS_MODELS`), the hostages' on maps with hostages, impact
//! decals and spray logos, the HUD look and the map's overview, round
//! sounds, the radio, bots' names, CS:S's impact looks; `held`: the guns'
//! held and view models, the muzzle flash and the shells), its sound
//! scripts (`sound::load`) and slime splashes; and `dress_guest`, CS:S's
//! players and guns on another game's map. The `load*` functions are
//! Source's with `CS_SOURCE`.

pub use source_engine::content::bsp::*;

use super::{material::MaterialLoader, surfaceprops::SurfaceProps};
use crate::{
    map::{MapData, MapLoadOptions},
    mount::Mount,
};

/// The player models by class (`core::PlayerClass` 1-8, CS:S's
/// `joinclass` numbers): Phoenix Connexion, Elite Crew, Arctic Avengers,
/// Guerilla Warfare; SEAL Team 6, GSG-9, SAS, GIGN.
pub const CLASS_MODELS: [&str; 8] = [
    "models/player/t_phoenix.mdl",
    "models/player/t_leet.mdl",
    "models/player/t_arctic.mdl",
    "models/player/t_guerilla.mdl",
    "models/player/ct_urban.mdl",
    "models/player/ct_gsg9.mdl",
    "models/player/ct_sas.mdl",
    "models/player/ct_gign.mdl",
];

/// The converter's version for the converted-map cache: the hash of this
/// crate's converter sources and data, Source's (`source_engine`'s and
/// `source_formats`') and the lockfile (build.rs), so any change to them
/// is a new version.
pub const CONVERTER: &str = env!("CHIMERA_CONVERTER_SOURCES");

/// CS:S's spawn points: terrorists' (our team 1, CS:S's 2) and
/// counter-terrorists' (our 2, CS:S's 3).
pub const SPAWNS: &[(&str, Option<crate::core::Team>)] = &[
    ("info_player_terrorist", Some(crate::core::Team(1))),
    ("info_player_counterterrorist", Some(crate::core::Team(2))),
];

/// CS:S's `sv_gravity` (units/s²): what its maps' physics bodies, ropes
/// and gibs fall at.
pub const GRAVITY: f32 = 800.0;

/// CS:S as a Source game for the map conversion: its id, converter
/// version and hooks.
pub static CS_SOURCE: SourceGame = SourceGame {
    id: super::GAME,
    converter: CONVERTER,
    spawns: SPAWNS,
    gravity: GRAVITY,
    kit,
    extra_particles,
    held,
    sounds,
    fixed_models: source_engine::content::props::no_fixed_models,
};

pub fn load(mount: &Mount, name: &str) -> Result<MapData, String> {
    source_engine::content::bsp::load(&CS_SOURCE, mount, name)
}

/// Load at a Source `mat_hdr_level` (`source_engine::content::bsp::
/// load_level`).
pub fn load_level(mount: &Mount, name: &str, hdr_level: u8) -> Result<MapData, String> {
    source_engine::content::bsp::load_level(&CS_SOURCE, mount, name, hdr_level)
}

/// `load_level` with `options` (`MapLoadOptions::lean`: no textures
/// decoded, for a dedicated server or a test that never draws).
pub fn load_level_with(mount: &Mount, name: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    source_engine::content::bsp::load_level_with(&CS_SOURCE, mount, name, options)
}

/// `load_level` from the map file's bytes (a copy downloaded from a
/// server, in the content cache), its content from `mount`.
pub fn load_level_bytes(mount: &Mount, name: &str, bytes: Vec<u8>, hdr_level: u8) -> Result<MapData, String> {
    source_engine::content::bsp::load_level_bytes(&CS_SOURCE, mount, name, bytes, hdr_level)
}

/// `load_level_bytes` with `options`, through the converted-map cache.
pub fn load_level_bytes_with(
    mount: &Mount,
    name: &str,
    bytes: Vec<u8>,
    options: &MapLoadOptions,
) -> Result<MapData, String> {
    source_engine::content::bsp::load_level_bytes_with(&CS_SOURCE, mount, name, bytes, options)
}

/// `load_level_bytes_with` through `cache` (None: converted), and whether
/// it was read from there (tests, measurements).
pub fn load_level_bytes_in(
    mount: &Mount,
    name: &str,
    bytes: Vec<u8>,
    options: &MapLoadOptions,
    cache: Option<&crate::map::cache::MapCache>,
) -> Result<(MapData, crate::map::cache::Outcome), String> {
    source_engine::content::bsp::load_level_bytes_in(&CS_SOURCE, mount, name, bytes, options, cache)
}

/// CS:S's kit before the map's effects: character bodies (a terrorist and
/// a counter-terrorist model, CS:S teams 2 and 3, ours 1 and 2, the CT one
/// for anyone else, then the other classes', `core::PlayerClass`, the
/// class menu), and with a whole kit the hostages' bodies, decals, HUD,
/// round sounds, radio and bots' names; the map's overview, CS:S's impact
/// effects' materials and its impact looks.
fn kit(k: &mut Kit) {
    add_bodies(k.materials, k.surfaces, k.kit, &mut k.data.kit, &mut k.data.warnings);
    if k.kit.parts() {
        // Hostages' bodies, on maps with hostages (picked by name).
        if k.data.entities.iter().any(|e| e.classname().eq_ignore_ascii_case("hostage_entity")) {
            for path in super::objectives::HOSTAGE_MODELS {
                match super::props::load_character(super::GAME, k.materials, k.surfaces, path, None) {
                    Ok(mut c) => {
                        c.name = Some(path.to_string());
                        k.data.kit.characters.push(c);
                    }
                    Err(e) => k.data.warnings.push(e),
                }
            }
        }
        k.data.kit.decals = super::decals::impact_decals(k.materials);
        super::decals::spray_logos(k.materials, &mut k.data.kit.decals);
        k.data.kit.hud = super::hud::load(k.materials, k.name).map(std::sync::Arc::new);
        k.data.kit.round_sounds = super::sound::round_sounds();
        k.data.kit.radio = super::radio::load(k.materials);
        // Bots' names (the install's botprofile.db).
        k.data.kit.bot_profiles = k.materials
            .read("botprofile.db")
            .map(|b| crate::map::bot_profiles::BotProfiles::parse(&String::from_utf8_lossy(&b)))
            .filter(|p| !p.0.is_empty());
    } else {
        // A mashup's map: its impacts' decals still (they go with the
        // impact looks below, which every load claims, and `GameKit::stay`
        // keeps them); without them a CS:S gun drew no bullet holes on
        // CS:S's own maps in the party apps.
        k.data.kit.decals = super::decals::impact_decals(k.materials);
    }
    // A mashup's map for players of CS:S (`KitLoad::Players`): its HUD
    // look too (a whole kit read it above, in its usual order).
    if k.kit.hud() && !k.kit.parts() {
        k.data.kit.hud = super::hud::load(k.materials, k.name).map(std::sync::Arc::new);
    }
    k.data.overview = super::hud::overview(k.materials, k.name);
    k.data.particles = super::impact_effects::load_materials(k.materials);
    // Its impacts' looks (effect materials; the impact decals, with any
    // kit): CS:S's impacts can be drawn on it.
    k.data.impacts.add(super::GAME);
}

/// CS:S's sounds: its sound scripts and the map's sounds (`sound::load`),
/// then the map's soundscapes (`soundscape::load`).
fn sounds(
    materials: &mut MaterialLoader,
    bsp: &vbsp::Bsp,
    name: &str,
    surfaces: &SurfaceProps,
    entities: &[crate::map::MapEntity],
) -> crate::map::sound::MapSounds {
    let mut sounds = super::sound::load(materials, name, surfaces, entities);
    super::soundscape::load(materials, bsp, name, &mut sounds);
    sounds
}

/// CS:S's slime splashes (`.pcf` systems) on maps with slime.
fn extra_particles(data: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    if data.water.iter().any(|w| w.slime) {
        (
            &super::impact_effects::SLIME_SPLASHES,
            &[super::impact_effects::SLIME_SPLASH_FILE],
        )
    } else {
        (&[], &[])
    }
}

/// What characters hold: the weapons' world models (held on the player
/// skeleton; a mashup's kept ones take the body's bones without keeping
/// the body), and with a whole kit the muzzle flash and the shells.
fn held(k: &mut Kit) {
    add_guns(k.materials, k.surfaces, k.kit, &mut k.data.kit, &mut k.data.warnings);
    if k.kit.parts() {
        k.data.kit.muzzle_flash = Some(super::view_anim::muzzle_flash(k.materials));
        for (key, path, bounce) in super::view_anim::SHELLS {
            match super::props::load_shell(k.materials, path) {
                Ok(model) => k.data.kit.shells.push(crate::map::shells::MapShell {
                    key: key.to_string(),
                    model,
                    bounce: Some(bounce.to_string()),
                }),
                Err(e) => k.data.warnings.push(e),
            }
        }
        k.data.kit.shell_physics = Some(super::view_anim::shell_physics());
    }
}

/// The players' character bodies `kit` asks for into `into`: a T and a CT
/// model (CS:S teams 2 and 3; ours are 1 and 2), the CT one for anyone
/// else; then the other classes' (`core::PlayerClass`, the class menu).
fn add_bodies(
    materials: &mut MaterialLoader,
    surfaces: &super::surfaceprops::SurfaceProps,
    kit: crate::map::KitLoad,
    into: &mut crate::map::GameKit,
    warnings: &mut Vec<String>,
) {
    if kit.bodies() {
        for (path, team) in [
            (CLASS_MODELS[0], Some(crate::core::Team(1))),
            (CLASS_MODELS[4], None),
        ] {
            match super::props::load_character(super::GAME, materials, surfaces, path, team) {
                Ok(c) => into.characters.push(c),
                Err(e) => warnings.push(e),
            }
        }
        for (i, path) in CLASS_MODELS.iter().enumerate().filter(|(i, _)| i % 4 != 0) {
            let team = crate::core::Team(1 + i as u8 / 4);
            match super::props::load_character(super::GAME, materials, surfaces, path, Some(team)) {
                Ok(mut c) => {
                    c.class = Some(i as u8 + 1);
                    into.characters.push(c);
                }
                Err(e) => warnings.push(e),
            }
        }
    }
}

/// The weapons' world models (held on the player skeleton: a mashup's
/// kept ones take the body's bones without keeping the body) and view
/// models `kit` wants into `into`.
fn add_guns(
    materials: &mut MaterialLoader,
    surfaces: &super::surfaceprops::SurfaceProps,
    kit: crate::map::KitLoad,
    into: &mut crate::map::GameKit,
    warnings: &mut Vec<String>,
) {
    let skeleton = match into.characters.first() {
        Some(c) => Some(c.bones.clone()),
        None if !kit.parts() && kit.any_models() => {
            super::props::load_character(super::GAME, materials, surfaces, CLASS_MODELS[0], None)
                .ok()
                .map(|c| c.bones)
        }
        None => None,
    };
    if let Some(skeleton) = skeleton {
        for (weapon, path) in super::weapons::world_models()
            .iter()
            .chain(super::objectives::WORLD_MODELS)
            .filter(|(weapon, _)| kit.wants(weapon))
        {
            match super::props::load_held(materials, path, weapon, &skeleton) {
                Ok(h) => into.held.push(h),
                Err(e) => warnings.push(e),
            }
        }
        for (weapon, path) in super::weapons::silenced_world_models() {
            let key = super::weapons::silenced_key(weapon);
            if !kit.wants(&key) {
                continue;
            }
            match super::props::load_held(materials, path, &key, &skeleton) {
                Ok(h) => into.held.push(h),
                Err(e) => warnings.push(e),
            }
        }
    }
    // What the local player sees of them: the view models.
    for (weapon, path, right_handed) in super::weapons::view_models() {
        if !kit.wants(weapon) {
            continue;
        }
        match super::props::load_view_model(materials, path, weapon, right_handed) {
            Ok(v) => into.view_models.push(v),
            Err(e) => warnings.push(e),
        }
    }
}

/// CS:S's players' bodies, the guns `kept` names (held and view models)
/// and, on a map without one, its HUD look, read from `mount` into a map
/// of another game (the party's Combat Arms maps, where a player holding
/// a CS:S gun is CS:S's, `slots::GameIdentity`). The kit's textures are
/// appended to the map's (`MaterialLoader::guest`: its indices begin past
/// them), so the bodies and models are the same as CS:S's own maps give.
/// The bodies go after the map's own. `lean`: the textures' sizes only
/// (`MaterialLoader::lean`: a load without textures, the dedicated server's).
pub fn dress_guest(mount: &Mount, map: &mut MapData, kept: &'static [&'static str], lean: bool) {
    let (textures, cubemaps) = (map.textures.len(), map.cubemaps.len());
    let mut materials = MaterialLoader::guest(mount, textures, cubemaps).lean(lean);
    let surfaces = super::surfaceprops::SurfaceProps::load(&mut materials);
    let kit = crate::map::KitLoad::Players(kept);
    let mut guest = crate::map::GameKit::default();
    let mut warnings = Vec::new();
    add_bodies(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
    add_guns(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
    guest.hud = super::hud::load(&mut materials, "").map(std::sync::Arc::new);
    // The guns' sounds (and the rest CS:S's maps always carry: steps,
    // pain, impacts) where the map hasn't an entry of that name: a CS:S
    // gun fired on another game's map plays its own sound.
    let sounds = super::sound::load(&mut materials, "", &surfaces, &[]);
    add_guest_sounds(std::sync::Arc::make_mut(&mut map.sounds), sounds);
    // CS:S's impacts (a CS:S gun's bullet holes, debris, sparks and
    // impact sounds, `map::impact`): its impact decals and effect
    // materials beside the map's own.
    add_guest_decals(&mut map.kit.decals, super::decals::impact_decals(&mut materials));
    for m in super::impact_effects::load_materials(&mut materials).materials {
        if map.particles.find(&m.name).is_none() {
            map.particles.materials.push(m);
        }
    }
    map.impacts.add(super::GAME);
    materials.finish_textures();
    warnings.extend(materials.missing);
    map.textures.extend(materials.textures.split_off(textures));
    map.cubemaps.extend(materials.cubemaps.split_off(cubemaps));
    map.texture_anims.extend(materials.texture_anims);
    map.warnings.extend(warnings);
    map.kit.characters.extend(guest.characters);
    map.kit.held.extend(guest.held);
    map.kit.view_models.extend(guest.view_models);
    map.kit.add_guest_hud(guest.hud);
}

