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
//! Free for all (M4a, `free_for_all_doors`, the conversion's
//! `entity_rule`): a gun game spawns anyone anywhere but in the spawn
//! rooms (`SPAWN_ROOMS`, `map::SpawnRooms`: their `func_respawnroom`
//! volumes, kept by the conversion, are left out of the spawn spots), and
//! the rooms open to all. Every door a trigger opens on touch starts open (the spawn
//! rooms' and the bases' auto doors; their triggers' team filters let
//! anyone through), so the spawn spots, drawn by a walk from the spawn
//! points through the world as it stands, spread over the whole map
//! (ctf_2fort: 4054 spots, not 202 in its spawn rooms); and the rooms'
//! door brushes (`func_respawnroomvisualizer`, solid to the room's
//! enemies in TF2) are no wall (Source's conversion leaves them out, with
//! TF2's other volumes).
//!
//! Its kit (M2b): its guns' view models (the class's arms with the
//! `c_` weapon bone-merged: `view::load`), held models (the same `c_`
//! models, bone-merged by the arms' `weapon_bone`, a `barrel` apart for the
//! game to turn: `MapHeldModel::turned`) and their projectiles'
//! models (`held_kit`); (M3) the classes' bodies and the HUD's look
//! (`body_kit`: `bodies::add_bodies`, `hud::load`); no decals yet. Its own
//! particle systems the guns start (`weapons::looks`: the explosion, the
//! muzzle flashes) load with its maps (`extra_particles`). Its sounds are
//! Source's sound scripts (the map's own entries, its surfaces' and the
//! guns', `weapon_sounds`, kept in a map's table under `SOUND_PREFIX`: Source games'
//! script names collide, and a table keeps the first of a name) and the map's soundscapes. On another game's
//! map its guns come with the same (`dress_guest_with`), and with their
//! looks from TF2's own files (`add_guest_looks`: the particle systems,
//! the impact decals and effect materials), so they look the same on any
//! map; its own maps carry its impact looks too (`add_impact_looks`).
//!
//! Known shortcuts:
//! - Spawns are the round start's: a point captured later doesn't move a
//!   team's spawns forward, and multi-stage maps' per-round spawns
//!   (`round_redspawn`, `round_bluespawn`) aren't read.
//! - The `spawnflags` class mask is ignored (no classes yet).
//! - Free for all only: the touch-opened doors start open and the spawn
//!   rooms' door brushes block nobody in a team mode too; doors the round
//!   logic opens (setup gates) stay as the logic leaves them.

use source_engine::content::{
    bsp::{self, Kit, SourceGame},
    material::MaterialLoader,
    props,
    surfaceprops::SurfaceProps,
};

use crate::{
    core::Team,
    map::{MapData, MapEntity, MapLoadOptions, MapLoader, MapWords, sound::MapSounds},
};

/// The converter's version for the converted-map cache, the kept guest
/// dressing and the served converted entries: `v<number>-<hash>`, a
/// number bumped by hand and the hash of the conversion's own sources
/// and data and the parsing crates' versions (build.rs; policy:
/// docs/plans/active/converted-cache.md, "Converter versions").
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
    kit: body_kit,
    extra_particles,
    held: held_kit,
    sounds,
    fixed_models: props::no_fixed_models,
    entity_rule: free_for_all_doors,
    // Its integer HDR, auto exposure and bloom (specs/tf2/hdr_bloom.md).
    hdr_model: crate::map::HdrModel::Integer,
    // TF2's own default (`mat_hdr_level 2`), unless the player set one.
    hdr_default_level: 2,
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

/// Its team spawn rooms' volumes (install.md 2.4): a free-for-all spawns
/// anywhere on the map but in them (`map::SpawnRooms`).
pub const SPAWN_ROOMS: &[&str] = &["func_respawnroom"];

/// TF2's maps' rules for every app that plays them: its spawn rooms.
pub fn add_map_rules(app: &mut bevy::app::App) {
    use crate::map::SpawnRoomAppExt;
    app.add_spawn_rooms(super::GAME, SPAWN_ROOMS);
}

/// TF2's maps' rules as a plugin (`Tf2Plugins`).
pub struct Tf2MapsPlugin;

impl bevy::app::Plugin for Tf2MapsPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        add_map_rules(app);
    }
}

/// The trigger outputs that open a door when a player walks in.
const TOUCH_OUTPUTS: &[&str] = &["onstarttouch", "onstarttouchall", "ontrigger"];

/// TF2's maps for a free-for-all (`SourceGame::entity_rule`): every door
/// a trigger opens on touch (`OnStartTouch`, `OnStartTouchAll`,
/// `OnTrigger` with `Open`: the spawn rooms' doors, and the auto doors
/// inside the bases) starts open (`spawnpos` 1). Their triggers' filters
/// (`filter_activator_tfteam`, a class the logic doesn't know) let every
/// player through, so they open for anyone (the trigger closes them again
/// once its last toucher leaves), and the spawn spots, which a walk from
/// the spawn points through the world as it stands at the start draws
/// (`chimera_sim::minigame::spawns`), reach past them: with the doors
/// shut they stayed in the spawn rooms (ctf_2fort: 362 m2). The spawn
/// rooms' door brushes (`func_respawnroomvisualizer`, solid to the room's
/// enemies) are no wall at all (Source's conversion leaves them out).
pub fn free_for_all_doors(entities: &mut [MapEntity]) {
    let opened: Vec<String> = entities
        .iter()
        .filter(|e| e.classname().to_ascii_lowercase().starts_with("trigger_"))
        .flat_map(|e| {
            e.keyvalues.iter().filter_map(|(k, v)| {
                let key = k.split('#').next().unwrap_or("").to_ascii_lowercase();
                if !TOUCH_OUTPUTS.contains(&key.as_str()) {
                    return None;
                }
                let mut f = v.split([',', '\u{1b}']);
                let target = f.next()?.trim();
                let input = f.next()?.trim();
                (input.eq_ignore_ascii_case("open") && !target.is_empty()).then(|| target.to_string())
            })
        })
        .collect();
    for e in entities.iter_mut() {
        let class = e.classname().to_ascii_lowercase();
        if class != "func_door" && class != "func_door_rotating" {
            continue;
        }
        let Some(name) = e.get("targetname").filter(|n| !n.is_empty()).map(str::to_string) else { continue };
        if !opened.iter().any(|t| source_logic::logic::world::name_matches(t, &name)) {
            continue;
        }
        match e.keyvalues.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("spawnpos")) {
            Some((_, v)) => *v = "1".into(),
            None => e.keyvalues.push(("spawnpos".into(), "1".into())),
        }
    }
}

/// TF2's maps from the install (`map::MapLoader`): what an app that plays
/// them chooses (`map::MapSources`).
pub static MAP_LOADER: MapLoader = MapLoader {
    game: super::GAME,
    load: load_installed_map,
    words: Some(&WORDS),
};

/// TF2's words for what maps of different games share (`map::MapWords`):
/// RED (1) the first side, BLU (2) the second; its surfaces Source's
/// (`source_engine::maps::SURFACE_WORDS`).
pub static WORDS: MapWords = MapWords {
    teams: &[(1, crate::map::SIDE_FIRST), (2, crate::map::SIDE_SECOND)],
    surfaces: Some(source_engine::maps::SURFACE_WORDS),
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

/// Its players' kit: the bodies of the classes of the guns the kit wants
/// (`bodies::add_bodies`) and the HUD's look (`hud::load`).
fn body_kit(k: &mut Kit) {
    super::bodies::add_bodies(k.materials, k.surfaces, k.kit, &mut k.data.kit, &mut k.data.warnings);
    if k.kit.hud() {
        k.data.kit.hud = super::hud::load(k.materials).map(std::sync::Arc::new);
    }
    // Its guns' impacts (decals, effect materials) on its own maps too.
    add_impact_looks(k.materials, k.data);
}

/// What TF2's guns' impacts draw (Source's impact effects with TF2's
/// files: `weapons::looks::STYLE`): its impact decals and effect
/// materials and the laser dot's sprites (`weapons::dot`), each the map
/// lacks, and the map marked as carrying TF2's impacts (`map::impact`), so its guns' hits are TF2's on any map, never
/// the map's game's or the app's base game's.
pub fn add_impact_looks(materials: &mut MaterialLoader, map: &mut MapData) {
    bsp::add_guest_decals(&mut map.kit.decals, source_engine::content::decals::impact_decals(materials));
    for m in source_engine::effects::impact_effects::load_materials(materials, &[]).materials {
        if map.particles.find(&m.name).is_none() {
            map.particles.materials.push(m);
        }
    }
    // The Sniper Rifle's laser dot's sprites (`weapons::dot`).
    for name in super::weapons::dot::dot_materials() {
        if map.particles.find(&name).is_none()
            && let Some(m) = materials.particle(&name)
        {
            map.particles.materials.push(m);
        }
    }
    map.impacts.add(super::GAME);
}

/// TF2's own looks on another game's map (`dress_guest_with`): its
/// impacts (`add_impact_looks`) and the particle systems its guns start
/// (`weapons::looks`: the explosion, the muzzle flashes, the flames) from
/// TF2's own files, whatever the map's game: before, on de_dust2 its
/// guns showed no muzzle flash and no flames and its rocket blew up as
/// Source's grey puff.
pub fn add_guest_looks(materials: &mut MaterialLoader, map: &mut MapData) {
    add_impact_looks(materials, map);
    let (names, files) = (super::weapons::looks::particle_systems(), super::weapons::looks::PARTICLE_FILES);
    source_engine::content::pcf::add_guest_particle_systems(materials, map, names, files);
}

/// The guns' sound entries (weapons.md 8.3; `sets::M2_WEAPONS`) and the
/// players' voices (`voices`: pain, death and kill lines).
pub fn weapon_sounds() -> impl Iterator<Item = String> {
    weapon_sounds_on(None)
}

/// `weapon_sounds` with the entries of the unlocks on `mount`
/// (`items::on_mount`: those their `visuals` name).
pub fn weapon_sounds_on(mount: Option<&crate::mount::Mount>) -> impl Iterator<Item = String> {
    gun_sounds(mount).into_iter().chain(super::voices::Tf2Voices::built_in().entries())
}

/// The guns' sound entries alone (kept under `SOUND_PREFIX`), the
/// unlocks' on `mount` too.
fn gun_sounds(mount: Option<&crate::mount::Mount>) -> Vec<String> {
    let mut out: Vec<String> = super::sets::M2_WEAPONS.iter().flat_map(|w| w.sounds.iter().map(|s| s.to_string())).collect();
    for item in mount.map(super::items::on_mount).unwrap_or_default() {
        for s in item.looks.sounds {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// What names TF2's own sound entries in a map's table: a Source game's
/// script names collide across games (`Weapon_Pistol.Single` is HL2's too,
/// `Weapon_Shotgun.Single`, `Weapon_RPG.Single`), and a map's table keeps
/// the first entry of a name, so a gun's sound played another game's. The
/// guns' entries are kept under this prefix (`own_sound`), and their data
/// (data/weapons) names them so.
pub const SOUND_PREFIX: &str = "tf2:";

/// `script_name` (an entry of TF2's sound scripts) as TF2's table holds it.
pub fn own_sound(script_name: &str) -> String {
    format!("{SOUND_PREFIX}{script_name}")
}

/// TF2's weapon entries in `sounds` moved under `SOUND_PREFIX` (`own_sound`);
/// the bare names stay too unless `guest` (on another game's map they would
/// shadow or be shadowed by that game's).
fn own_weapon_sounds(sounds: &mut MapSounds, guest: bool, guns: &[String]) {
    for name in guns {
        let key = name.to_lowercase();
        let entry = if guest { sounds.entries.remove(&key) } else { sounds.entries.get(&key).cloned() };
        if let Some(entry) = entry {
            sounds.entries.insert(own_sound(&name).to_lowercase(), entry);
        }
    }
}

/// What players hold on TF2's maps: its guns' view and held models
/// (`add_guns`).
fn held_kit(kit: &mut Kit) {
    let mut guest = crate::map::GameKit::default();
    let wants = |g: &str| kit.kit.wants(g);
    if super::weapons::GUNS.iter().any(|g| wants(g)) {
        super::weapons::script_numbers::note_sources(kit.materials.mount());
    }
    add_guns(kit.materials, &wants, true, &mut guest, &mut kit.data.warnings);
    kit.data.kit.held.extend(guest.held);
    kit.data.kit.view_models.extend(guest.view_models);
}

/// TF2's guns that `wants`, into `into`: their view models
/// (`view::load`; with `every_class` on each class's arms that can hold
/// them and are here, else on their own class's only), their `c_` models held (bone-merged by the first bone
/// they share with their class's arms, `weapon_bone`, and their parts by
/// the other bones bodies have too: not the arms' own `vm_` bones,
/// `is_view_bone`) and their
/// projectiles' models (`weapons::PROJECTILES`: the rocket, the grenade,
/// the sticky, the needle), and the unlocks on the mount the same way
/// (`items::on_mount`: their own models, their base's projectile). What a gun's
/// `weapon/<id>` set holds is what this reads for it (`sets`).
pub fn add_guns(
    materials: &mut MaterialLoader,
    wants: &dyn Fn(&str) -> bool,
    every_class: bool,
    into: &mut crate::map::GameKit,
    warnings: &mut Vec<String>,
) {
    use super::weapons::{self as w, PROJECTILES};
    let items = super::items::on_mount(materials.mount());
    super::view::load_with(materials, wants, every_class, &items, &mut into.view_models, warnings);
    let set = w::tf2_weapons();
    // The stock guns, then the unlocks on the mount (`items::on_mount`:
    // their own models; their projectiles their base's).
    let mut guns: Vec<(String, String, Option<String>, String)> = w::arms_by_gun()
        .into_iter()
        .map(|(g, arms)| (g.to_string(), arms, set.weapons.iter().find(|d| d.id == g).and_then(|d| d.models.world.clone()), g.to_string()))
        .collect();
    for item in items {
        guns.push((item.id, item.looks.arms, item.looks.world, item.base));
    }
    let mut projectiles: Vec<&str> = Vec::new();
    for (gun, arms, path, base) in guns {
        let gun = gun.as_str();
        if !wants(gun) {
            continue;
        }
        let Some(path) = path else { continue };
        // The arms' bones as the bodies have them: the `vm_weapon_bone…`
        // bones are the arms' alone (a player model's weapon bones are
        // `weapon_bone`, `weapon_bone_1`…, `weapon_bone_L`: install.md
        // 4.5), so on a body the parts skinned to them stay with their
        // parent's (player_animation.md 7: unmatched bones keep their bind
        // pose); merged onto a bone no body has, the Shovel's blade, the
        // Shotgun's pump and the launchers' parts were never drawn.
        let skeleton = match props::load_view_model(materials, &super::sets::arms_model(&arms), gun, true) {
            Ok(v) => v.bones.into_iter().filter(|b| !is_view_bone(&b.name)).collect::<Vec<_>>(),
            Err(e) => {
                warnings.push(e);
                continue;
            }
        };
        // Its `barrel` (the Minigun's) apart, for others' copies to spin
        // (minigun.md 5: `view::spin_held_barrels`).
        match props::load_held_turning(materials, &path, gun, &skeleton, &["barrel"]) {
            Ok(mut h) => {
                // Its hitboxes, for the crit glow on others' guns
                // (effects.md 14.1).
                if let Err(e) = props::add_held_hitboxes(materials, &path, &skeleton, &mut h) {
                    warnings.push(e);
                }
                into.held.push(h)
            }
            Err(e) => warnings.push(e),
        }
        for (_, key, model) in PROJECTILES.iter().filter(|(g, ..)| *g == base) {
            // Once (an unlock and its base share it).
            if projectiles.contains(key) {
                continue;
            }
            projectiles.push(key);
            // In its bone's frame, as a held model (its nose, the model's
            // +X, wherever that frame puts it: `MapHeldModel::nose`; flyers
            // fly nose first by it).
            match props::load_held(materials, model, key, &skeleton) {
                Ok(h) => into.held.push(h),
                Err(e) => warnings.push(e),
            }
        }
    }
}

/// A bone of the class arms that no player model has (`vm_weapon_bone`,
/// `vm_weapon_bone_1`…: install.md 4.4, 4.5).
pub fn is_view_bone(name: &str) -> bool {
    name.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("vm_"))
}

/// The game's files for its guest dressing (`map::kept::Dressing::
/// mount`): the install, mounted.
pub fn guest_mount() -> Result<std::sync::Arc<crate::mount::Mount>, String> {
    let install = super::INFO
        .install()
        .ok_or_else(|| format!("no `{}` install found (Steam, [games.tf2] path, or $CHIMERA_TF2_ROOT)", super::GAME))?;
    super::mount::open(&install).map(std::sync::Arc::new).map_err(|e| e.to_string())
}

/// TF2's guns on another game's map (the party apps', where a player
/// holding a TF2 gun is TF2's): the guns `kept` names (view and held
/// models: `add_guns`) read from the TF2 install, their textures
/// appended to the map's, and their sounds where the map hasn't an entry
/// of that name. `lean` (a load without textures, the dedicated
/// server's): the textures' sizes only.
pub fn dress_guest_with(map: &mut MapData, kept: &'static [&'static str], lean: bool) -> Result<(), String> {
    dress_guest_in(&*guest_mount()?, map, kept, lean);
    Ok(())
}

/// `dress_guest_with` from `mount` (the game's files: `guest_mount`).
pub fn dress_guest_in(mount: &crate::mount::Mount, map: &mut MapData, kept: &'static [&'static str], lean: bool) {
    let (textures, cubemaps) = (map.textures.len(), map.cubemaps.len());
    let blocks_only = map.memos.get::<crate::map::BlocksOnly>().is_some();
    let mut materials = MaterialLoader::guest(mount, textures, cubemaps).lean(lean).blocks_only(blocks_only);
    let surfaces = SurfaceProps::load(&mut materials);
    let kit = crate::map::KitLoad::Players(kept);
    let wants = |g: &str| kit.wants(g);
    let mut guest = crate::map::GameKit::default();
    let mut warnings = Vec::new();
    if super::weapons::GUNS.iter().any(|g| wants(g)) {
        super::weapons::script_numbers::note_sources(mount);
    }
    add_guns(&mut materials, &wants, true, &mut guest, &mut warnings);
    super::bodies::add_bodies(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
    let guns = gun_sounds(Some(mount));
    let names = guns.clone().into_iter().chain(super::voices::Tf2Voices::built_in().entries());
    let mut sounds = source_engine::content::sound::load_with(&mut materials, "", &surfaces, &[], names, true);
    own_weapon_sounds(&mut sounds, true, &guns);
    bsp::add_guest_sounds(std::sync::Arc::make_mut(&mut map.sounds), sounds);
    let look = kit.hud().then(|| super::hud::load(&mut materials).map(std::sync::Arc::new)).flatten();
    add_guest_looks(&mut materials, map);
    materials.finish_textures();
    warnings.extend(materials.missing);
    map.textures.extend(materials.textures.split_off(textures));
    map.cubemaps.extend(materials.cubemaps.split_off(cubemaps));
    map.texture_anims.extend(materials.texture_anims);
    map.warnings.extend(warnings);
    map.kit.characters.extend(guest.characters);
    map.kit.held.extend(guest.held);
    map.kit.view_models.extend(guest.view_models);
    map.kit.add_guest_hud(look);
}

/// TF2's own effects its guns start (`weapons::looks`): the explosion and
/// the muzzle flashes.
fn extra_particles(_: &MapData) -> (&'static [&'static str], &'static [&'static str]) {
    (super::weapons::looks::particle_systems(), super::weapons::looks::PARTICLE_FILES)
}

/// TF2's sounds: Source's sound scripts (the map's own entries and its
/// surfaces'), then the map's soundscapes.
fn sounds(materials: &mut MaterialLoader, bsp: &vbsp::Bsp, name: &str, surfaces: &SurfaceProps, entities: &[MapEntity]) -> MapSounds {
    let guns = gun_sounds(Some(materials.mount()));
    let names = guns.clone().into_iter().chain(super::voices::Tf2Voices::built_in().entries());
    let mut out = source_engine::content::sound::load_with(materials, name, surfaces, entities, names, true);
    own_weapon_sounds(&mut out, false, &guns);
    source_engine::content::soundscape::load(materials, bsp, name, &mut out);
    out
}
