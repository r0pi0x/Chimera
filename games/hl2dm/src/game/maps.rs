//! HL2:DM's maps: Source's BSP conversion (`source_engine::content::bsp`,
//! lifted from CS:S in M1) with HL2:DM's data (`HL2DM`): its id, its
//! converter's version (`CONVERTER`, this crate's build.rs), its spawn
//! points (`info_player_deathmatch`: anyone; specs/hl2dm/source_deltas.md,
//! "Stock maps") and its `sv_gravity` (600: physics bodies, ropes and gibs
//! fall at it; source_deltas.md 1.1). `MAP_LOADER` loads a map of the
//! install (`hl2dm:<name>`) as an app's `map::MapSources` asks.
//!
//! Its kit: its 19 player bodies (the `kit` hook, `bodies::add_bodies`),
//! its weapons' world models held on them (`bodies::add_held`) and view
//! models (the `held` hook, `add_view_models`), right-handed and never
//! mirrored (specs/hl2dm/player_models.md 12.1). `dress_guest` brings them
//! onto another game's map (the party's), read from the HL2:DM install.
//! Also its chat formats (`chat`: no radio, the dead read by the living).
//!
//! Known shortcuts:
//! - HL2:DM's kit has no HUD (M9), muzzle flash, impact decals and looks
//!   (M10). Its
//!   sounds (Source's sound scripts and soundscapes, lift S5) are the
//!   entries it plays so far (`SOUNDS`: movement, the suit, pickups and
//!   chargers); its weapons' entries aren't among them yet.
//! - The map's `SpawnPoint`s are deathmatch's only; HL2:DM's rules
//!   choose among the map's spot entities themselves (`rules::spawns`:
//!   the team classes, `info_player_start`).

use source_engine::content::{
    bsp::{self, Kit, SourceGame},
    material::MaterialLoader,
    props,
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
    kit: body_kit,
    extra_particles: no_extra_particles,
    held: held_kit,
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
    "BaseCombatCharacter.AmmoPickup",
    "AmmoCrate.Open",
    "AmmoCrate.Close",
    "HudChat.Message",
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

/// HL2:DM's kit before the map's effects: its 19 player bodies
/// (`bodies::add_bodies`).
fn body_kit(k: &mut Kit) {
    super::bodies::add_bodies(k.materials, k.surfaces, k.kit, &mut k.data.kit, &mut k.data.warnings);
    k.data.kit.radio = Some(chat());
}

/// HL2:DM's chat (specs/hl2dm/rules.md "Chat"): no radio; its line
/// formats (the localization's `HL2MP_Chat_*`, the name in its team's
/// colour), no "*DEAD*" mark and the dead read by the living,
/// `HudChat.Message` on each line.
pub fn chat() -> crate::map::radio::RadioCommands {
    use crate::map::radio::{RadioCommands, SayFormats, TeamSay};
    let all = "\u{3}%s1\u{1} :  %s2".to_string();
    let team = TeamSay {
        alive: "\u{1}(TEAM) \u{3}%s1\u{1} :  %s2".into(),
        alive_place: None,
        dead: "\u{1}(TEAM) \u{3}%s1\u{1} :  %s2".into(),
    };
    RadioCommands {
        commands: Vec::new(),
        menus: Vec::new(),
        format: String::new(),
        format_location: None,
        say: SayFormats {
            all: all.clone(),
            all_dead: all,
            all_spectator: "\u{1}*SPEC* \u{3}%s1\u{1} :  %s2".into(),
            team: vec![(1, team.clone()), (2, team)],
            team_spectator: "\u{1}(Spectator) \u{3}%s1\u{1} :  %s2".into(),
            sound: Some("HudChat.Message".into()),
            dead_heard: true,
            ..SayFormats::default()
        },
    }
}

/// What players hold on HL2:DM's maps: its weapons' world models on the
/// bodies and their view models.
fn held_kit(kit: &mut Kit) {
    super::bodies::add_held(kit.materials, kit.surfaces, kit.kit, &mut kit.data.kit, &mut kit.data.warnings);
    let mut models = Vec::new();
    add_view_models(kit.materials, kit.kit, &mut models, &mut kit.data.warnings);
    kit.data.kit.view_models.extend(models);
}

/// HL2:DM's weapons' view models that `kit` wants, read through
/// `materials` into `into`: right-handed and never mirrored.
pub fn add_view_models(
    materials: &mut MaterialLoader,
    kit: crate::map::KitLoad,
    into: &mut Vec<crate::map::MapViewModel>,
    warnings: &mut Vec<String>,
) {
    for (weapon, path) in super::weapons::view_models() {
        if !kit.wants(&weapon) {
            continue;
        }
        match props::load_view_model(materials, &path, &weapon, true) {
            Ok(mut v) => {
                v.allow_flipping = false;
                into.push(v);
            }
            Err(e) => warnings.push(e),
        }
    }
}

/// HL2:DM's players and guns on another game's map (the party apps',
/// where a player holding an HL2:DM gun is HL2:DM's): its 19 bodies, the
/// guns `kept` names (held and view models), read from the HL2:DM
/// install, their textures appended to the map's; the bodies go after the
/// map's own.
pub fn dress_guest(map: &mut MapData, kept: &'static [&'static str]) -> Result<(), String> {
    let install = super::INFO
        .install()
        .ok_or_else(|| format!("no `{}` install found (Steam, [games.hl2dm] path, or $CHIMERA_HL2DM_ROOT)", super::GAME))?;
    let mount = super::mount::open(&install).map_err(|e| e.to_string())?;
    let (textures, cubemaps) = (map.textures.len(), map.cubemaps.len());
    let mut materials = MaterialLoader::guest(&mount, textures, cubemaps);
    let surfaces = SurfaceProps::load(&mut materials);
    let kit = crate::map::KitLoad::Players(kept);
    let mut guest = crate::map::GameKit::default();
    let mut warnings = Vec::new();
    super::bodies::add_bodies(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
    super::bodies::add_held(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
    add_view_models(&mut materials, kit, &mut guest.view_models, &mut warnings);
    materials.finish_textures();
    warnings.extend(materials.missing);
    map.textures.extend(materials.textures.split_off(textures));
    map.cubemaps.extend(materials.cubemaps.split_off(cubemaps));
    map.texture_anims.extend(materials.texture_anims);
    map.warnings.extend(warnings);
    map.kit.characters.extend(guest.characters);
    map.kit.held.extend(guest.held);
    map.kit.view_models.extend(guest.view_models);
    Ok(())
}

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
