//! Combat Arms' data: the documents built into the game
//! (games/combat_arms/data/, parsed at run time by Chimera's
//! `weapon::data`, docs/weapon-data.md) and, when the install can be read,
//! its attribute files over them (specs/combat_arms/attribute_data.md,
//! "Defaults and overrides"; the records: `attributes`).
//!
//! Built in: the M4A1 (data/weapons/m4a1.toml, the numbers of
//! weapon_firing.md's worked example), the M9 knife (m9.toml,
//! inventory.md's numbers), the grenades (m67.toml, xm84.toml, m18.toml,
//! m23f.toml, g7a.toml: grenades.md's), one weapon per class from
//! weapon_classes.md's "Typical weapons" (mp5a4, m60, beretta_m92fs,
//! remington_870, l96a1, dragunov_svd: an install weapon of a class we
//! have no document for is skipped, one of a class we have copies it) and
//! the movement numbers
//! (data/movement.toml, `movement::MovementData`).
//!
//! Built in too: the attachment records (data/attachments/mods.toml,
//! attachments.md's tables; `attachments::CaModTable`), each weapon's
//! accepted list and defaults in its `combat_arms:attachments` table.
//!
//! The install: with `[games.combat_arms] path` and a `V1` key in
//! chimera.local.toml, `install_attributes` opens the install's
//! `ATTRIBUTES.rez` and reads the weapons, server, models, marks,
//! character attachments and modification files (`attributes`; the
//! marks, goggles and backpacks go to `characters::records`). Then:
//! - weapons (`merge_weapon_docs`): each of our documents whose ID (after
//!   `combat_arms:`) matches an install weapon's `Name` (case-insensitive,
//!   spaces as `_`: `slug`; `INSTALL_NAMES` for the two of ours named
//!   otherwise, the knife `M9` and the `M-23F`) takes the install's
//!   numbers for every field it maps (`weapon_values`; the models,
//!   `model_values`: `PVModelNormal`, `PVSkin#` and `Pos` as `[models]
//!   view`, `view_skins`, `view_offset`, `HHModel` as `world`,
//!   `MuzzlePos` and `BreachOffset` in `combat_arms:view_model`; a
//!   grenade's projectile record, `grenade_values`: the effect file's
//!   `ProjectileFX#` its ammo's `ProjectileFXName` names, its `Velocity`,
//!   `Lifetime`, `DropVelocity`, `FireOffset`, `CanHitSameKind` and
//!   thrown `Model`, `ModelScale`, `Skin#` in `combat_arms:grenade`;
//!   `effects_patch`: the weapon's own sounds and effects, never its
//!   template's, in `combat_arms:effects` and `combat_arms:view_model`
//!   (both made for a document without one: the crossbow, the grenades,
//!   the MP5A4): `FireSnd`, `SilencedFireSnd1..10`, `DryFireSnd`,
//!   `ReloadSnd1..3`, `SelectSnd`, `DeselectSnd`, `MiscSound1..5` with
//!   `FireSndRadius`, `ReloadSndRadius` and `SelectSndRadius`, the muzzle
//!   groups `PVMuzzleFXName`, `HHMuzzleFXName` and the flash-hider groups,
//!   the view model's attached groups `PVAttachFXName#` with
//!   `PVAttachFXAlwaysShow`,
//!   and the default ammo's `ImpactFXName`, `FireFXName` and
//!   `TracerFXName` (a missing field: the spec's default, no sound or
//!   effect; an empty impact or tracer name: `BulletFmj`, `Standard`;
//!   a thrown weapon's ammo effects are the grenade hook's), and
//!   `sound_values`: the grenade hook's `call_out` (`PlayAutoThrowSound` and `Guntype` 7); the
//!   third-person `AniType`, `LowerAniType` and `HHSkin#` go to
//!   `characters::records`): the record's value, or the spec's
//!   built-in default when the install lacks it, never ours, and the
//!   inventory class from `[WeaponOrder]` as its `slot` (the class - 1;
//!   in no list: ours); an impact projectile weapon's (`projectile_patch`,
//!   special_weapons.md 1): its projectile record's `Velocity`,
//!   `Lifetime`, `Gravity`, `CanHitSameKind` and world model in
//!   `combat_arms:projectile` (drawn in flight), the ammo's lasting
//!   sphere (made when the ammo has one), the `Guntype` 9 shooter
//!   exemption, and in `combat_arms:effects` (made) its `FireSnd`, the
//!   ammo's `ImpactFXName` with the effect file's `ImpactFX#` record
//!   (`impact_record`: the explosion's sound and group), launched ammo;
//!   its frame trace keeps our document's range. Install
//!   weapons that match none of ours are new documents, copied from our
//!   matched document of the same `Guntype` whose install weapon's ammo
//!   fires the same projectile record (`template_for`: a flame launcher
//!   copies the flamethrower, not the first rocket launcher), else the
//!   first of that `Guntype` (its parts and hooks) with
//!   their own numbers, after ours in install order (ours keep their
//!   registry indices); a copy keeps its template's `class` (an
//!   automatic gun game's) but a grenade's (a smoke copied from a frag
//!   doesn't kill); a copied gun or knife (a trigger or a knife table
//!   and a view model, not a grenade) gets a `combat_arms:timing` table
//!   (`timing_table`): its fire sequence, reload, draw and swings are its
//!   own view model's animations, read when it is first built
//!   (`weapons::timing`), not the template's; a copy with `AccumPreFire`
//!   gets a `combat_arms:spin` table (its spin's lengths from the model);
//!   a copied knife whose record plays the infected leg set
//!   (`LowerAniType` 1 or 2: the Quarantine hands) gets a
//!   `combat_arms:infected` table (`infected_table`: a gun game's infected
//!   rung, `rungs`);
//!   every weapon's `FireAniRate` goes in `combat_arms:inventory` (made
//!   when the template has none, the record's other item fields with it);
//! - movement (`merge_movement`): the body type `FigureType0` replaces
//!   `character.figure` (`StiffDuration` with it), and every `FigureType#`
//!   is `MovementData::figures` (each vest record's body type, `gear`);
//! - attachments (`attachment_table`): the modification file's `Mod#`
//!   records over ours by name (ours not in it stay); a weapon document
//!   with a `combat_arms:attachments` table takes the install weapon's
//!   accepted list from the modification file's `Weapon#` record **of the
//!   same index** (attachments.md Q13: by position; a differing `Name` is
//!   logged; no record: an empty list, logged, Q2), its `Default*Attach`
//!   parts, `DefaultScopeVisible`, `FireSndRadius`,
//!   `SilencedFireRadius1..10`, the record's slot options
//!   (`MuzzleOPtion`..`MechSightOption`) and the part offsets
//!   (`MuzzlePVPosOff`/`RotOff` and the scope's and third person's:
//!   `part_offsets`) (`merge_weapon_docs_with_mods`). The audit
//!   (`heavy::rung_attachments`): of the install's 1942 pool guns, 1835
//!   records carry the weapon's own name, the other 107 a spelling of it
//!   or a shared list, two swapped (the `loaded` R870 and AT-22), which
//!   the game links by position too.
//! Without an install or key, or when a document fails to parse (logged
//! with its name and line), the built-in data is used unchanged.
//! `weapon_set` and `movement::movement_data` are the merged data;
//! `use_builtin_data_only` (the test harness, `CaSim`) turns the install
//! off for the process. A network server offers the merged weapon
//! documents, attachment records and movement numbers to clients whose
//! own differ (`served`).
//!
//! The other modules read data only through this one: `weapons` registers
//! the set, `inventory` names what a player starts with.
//!
//! Known shortcuts:
//! - A mapped value is written only where our document already has the
//!   table it goes in (the weapon's parts and hooks are ours; the install
//!   changes their numbers), and `trigger.per_press` only where the key
//!   is already there. The zoom fields (`ZoomTime`, `CameraSway*`,
//!   `ReloadAfterFire`, `ZoomedMovePenalty`: weapon_classes.md) go in the
//!   `combat_arms:zoom` table (made empty first in a document that fits
//!   parts, `combat_arms:attachments`, so the machine guns and shotguns
//!   the install gives a sight zoom; the special weapons, with no
//!   attachments table, get none: their own scope designators, `LGB_scope`
//!   and the like, aren't specified), `AccumPreFire` in `combat_arms:spin`'s;
//!   `ReloadPerBullet` 0 removes our `magazine.shells` (1 keeps it: ours
//!   are the spec's timings, a copy's its own view model's, `timing`).
//!   Not mapped (no field
//!   in our data yet): a projectile's box (its model's first animation
//!   box x `ModelScale`: the template's stays), `AltVelocity`,
//!   the projectile record's `ActivateSound`, `FinishSound` and
//!   `CreateSound` (special_weapons.md Follow-up 2: not read yet; its
//!   `FXName`, `FlySound`, flare and smoke trail are, `look_table`),
//!   `TripleShot*` timing (the trigger's `latency`, a
//!   flame copy's fire keys and a knife copy's swings come from the
//!   copy's view model: `timing`),

//!   `ContinuousFire`, render styles,
//!   armour, `[Perturb]`, `[Player]` (the client's
//!   speeds are movement.md's: the spec's open question 2), `Model#`/`Hair#`
//!   and worn gear, the modification file's gear lists and per-slot
//!   option fields.
//! - An ammo's `FireFXName`, `TracerFXName` and `ImpactFXName` are names
//!   only: the records come from our built-in tables (data/effects.toml,
//!   the stock ones), and a name they lack (a water gun's, `Standard`) keeps
//!   the template's value; an impact projectile weapon's impact record is
//!   read from the install (`impact_record`).
//! - Special functions in the modification file are read by name
//!   (`InstDamage`, `Stun`, ...; targets `All`, `Player`, ...); a number
//!   there is skipped (the spec doesn't say how numbers map).
//! - Ammo is patched through the weapon that uses it as its default
//!   (`ammo.<id>.max` = `MaxAmount` - `ShotsPerClip`, the clip being part
//!   of the total: weapon_firing.md, "Ammo"); our ammo IDs aren't matched
//!   to ammo records on their own.
//! - Lengths are converted with movement's `meters_per_unit` and the
//!   document's own `units.length` (not its base's).
//! - Without the install our movement data has one figure (the
//!   reference body's) for every body type.
//! - Test case 29's install document has no `Ammo0`, which case 26 says
//!   fails to load; its test adds one.
//! - The install adds some 2,000 weapons; a map loads the held
//!   (third-person) models of ours only (`builtin_weapon_ids`: what
//!   players are given); an install weapon given by other means loads
//!   its held model when first held (`characters::held_on_demand`) and
//!   its view model when first drawn (`view_model::on_demand`).
//!   The skins' `Hands` placeholder and `PV_L_ARM` / `PV_R_ARM` tags (no
//!   `.dtx` or `.spr`) are empty slots (`skin_paths`); a sprite skin
//!   (`.spr`, the animated ones: rainbow, lava-crystal, light-up guns)
//!   draws its first frame only (the sprite's timing isn't in the specs).

pub mod attributes;
pub mod served;
mod store;

pub use store::{CaData, CaDataInner};

use std::io;

use bevy::prelude::*;
use lithtech_formats::attributes::Document;
use toml::{Table, Value};

use self::attributes::{
    ATTACHMENTS_FILE, AmmoRecord, PERKS_FILE, SETS_FILE, AttachmentsFile, MARKS_FILE, MOD_FILE, MODELS_FILE, MarksFile, ModFile, ModelsFile, SERVER_FILE, ServerPlayer, ServerTimeWeight, WEAPONS_FILE,
 WeaponRecord,
    WeaponsFile, FX_FILE, FigureTypeRecord, FireRecord, ImpactRecord, ProjectileRecord,
};
use super::{
    attachments::CaModTable,
    movement::{Figure, MovementData},
};

macro_rules! data_file {
    ($name:literal) => {
        ($name, include_str!(concat!("../../data/weapons/", $name)))
    };
}

/// Combat Arms' weapon documents (games/combat_arms/data/weapons), built
/// in, in registration order (the network names weapons by registry
/// index).
pub const WEAPON_DATA: &[(&str, &str)] = &[
    data_file!("m9.toml"),
    data_file!("m4a1.toml"),
    data_file!("m67.toml"),
    data_file!("xm84.toml"),
    data_file!("m18.toml"),
    data_file!("m23f.toml"),
    data_file!("g7a.toml"),
    // One weapon per class (weapon_classes.md, "Typical weapons").
    data_file!("mp5a4.toml"),
    data_file!("m60.toml"),
    data_file!("beretta_m92fs.toml"),
    data_file!("remington_870.toml"),
    data_file!("l96a1.toml"),
    data_file!("dragunov_svd.toml"),
    // The mission bomb (game_modes.md M3).
    data_file!("xk2.toml"),
    // Special weapons (special_weapons.md): rockets and launchers (2, 3), the
    // flamethrower (4), devices (6), crossbows (7) and the melee weapons beyond
    // the M9 (5).
    data_file!("law.toml"),
    data_file!("rpg-7.toml"),
    data_file!("m136_at4.toml"),
    data_file!("m32.toml"),
    data_file!("m32_incendiarybomb.toml"),
    data_file!("m32_stun.toml"),
    data_file!("m79.toml"),
    data_file!("mzp_1.toml"),
    data_file!("xm_25.toml"),
    data_file!("flamethrower.toml"),
    data_file!("m16a1_mine.toml"),
    data_file!("hornet.toml"),
    data_file!("claymore.toml"),
    data_file!("claymore_controller.toml"),
    data_file!("mobile_grenade.toml"),
    data_file!("mobile_grenade_controller.toml"),
    data_file!("caltrops.toml"),
    data_file!("nerve_confuse_mine.toml"),
    data_file!("auto-turret.toml"),
    data_file!("firstaidkit.toml"),
    data_file!("medkit.toml"),
    data_file!("lgb.toml"),
    data_file!("airstrike_napalm.toml"),
    data_file!("assassin_bowgun_nor.toml"),
    data_file!("assassin_bowgun_expl.toml"),
    data_file!("assassin_bowgun_expl_controller.toml"),
    data_file!("balisong_knife.toml"),
    data_file!("bowie_knife.toml"),
    data_file!("chainsaw.toml"),
    data_file!("dompedrosword.toml"),
    data_file!("gi_shovel.toml"),
    data_file!("guillotine.toml"),
    data_file!("knuckle.toml"),
    data_file!("kukuri.toml"),
    data_file!("m11-tactical_knife.toml"),
    data_file!("machete.toml"),
    data_file!("ninjato.toml"),
    data_file!("sickle.toml"),
    data_file!("sledgehammer.toml"),
    data_file!("strider_axe.toml"),
    data_file!("survival_machete.toml"),
    data_file!("tiger_strider.toml"),
    data_file!("tracker_knife.toml"),
    data_file!("uamc_ka_bar.toml"),
    // The scanner items (radar_minimap.md S1, S2).
    data_file!("heartbeat_sensor.toml"),
    data_file!("heartbeat_detector.toml"),
    data_file!("satellite_scanner.toml"),
];

/// The namespace of our weapon and ammo IDs.
const NS: &str = "combat_arms:";

/// The modification file's records over `table` by name (ours not in it
/// stay).
pub fn merge_mods(table: &mut CaModTable, mods: &ModFile) {
    for r in &mods.mods {
        if !r.name.is_empty() {
            table.insert(r.clone());
        }
    }
}

/// The movement data with the install's body type over it (called by
/// `CaData::movement`).
pub fn with_install_movement(mut data: MovementData, attributes: Option<&InstallAttributes>) -> MovementData {
    if let Some(models) = attributes.and_then(|a| a.models.as_ref()) {
        merge_movement(&mut data, models);
    }
    data
}

/// What was read from the install's attribute files: each `None` when its
/// file is missing, locked or fails to parse or load.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstallAttributes {
    pub weapons: Option<WeaponsFile>,
    pub server: Option<ServerPlayer>,
    /// The server file's `[TimeWeight]` (`ReloadCheckTime`).
    pub time_weight: Option<ServerTimeWeight>,
    pub models: Option<ModelsFile>,
    /// The modification file (attachments).
    pub mods: Option<ModFile>,
    /// The team marks (`[TeamMark]`: `TeamName#`, `RenderStyle#`).
    pub marks: Option<MarksFile>,
    /// The character attachments (`Goggle#`, `Backpack#`).
    pub attachments: Option<AttachmentsFile>,
    /// The gear and perk records (gear_perks.md), read from the weapons,
    /// models, attachments, modification, perk and set files.
    pub gear: Option<super::gear::records::GearInstall>,
}

/// Parse one attribute document, errors naming it and the line.
pub fn parse_document(name: &str, bytes: &[u8]) -> Result<Document, String> {
    Document::parse(bytes).map_err(|e| format!("{name}: {e}"))
}

/// Read the attribute files through `read` (an in-archive path to its
/// bytes). Returns the records and, for each file that couldn't be used,
/// why (the caller logs them).
pub fn read_attributes(read: impl Fn(&str) -> io::Result<Vec<u8>>) -> (InstallAttributes, Vec<String>) {
    let mut problems = Vec::new();
    let mut doc = |name: &str| match read(name).map_err(|e| format!("{name}: {e}")).and_then(|b| parse_document(name, &b)) {
        Ok(d) => Some(d),
        Err(e) => {
            problems.push(e);
            None
        }
    };
    let weapons = doc(WEAPONS_FILE);
    let server = doc(SERVER_FILE);
    let models = doc(MODELS_FILE);
    let mods = doc(MOD_FILE);
    let marks = doc(MARKS_FILE);
    let attachments = doc(ATTACHMENTS_FILE);
    let perks = doc(PERKS_FILE);
    let sets = doc(SETS_FILE);
    let fx = doc(FX_FILE);
    let any_gear_file = [&weapons, &models, &attachments, &mods, &perks, &sets].iter().any(|d| d.is_some());
    let gear = any_gear_file.then(|| {
        super::gear::records::GearInstall::read(
            weapons.as_ref(),
            models.as_ref(),
            attachments.as_ref(),
            mods.as_ref(),
            perks.as_ref(),
            sets.as_ref(),
        )
    });
    let weapons = weapons.and_then(|d| {
        WeaponsFile::read(&d)
            .map_err(|e| problems.push(format!("{WEAPONS_FILE}: {e}")))
            .ok()
    });
    // The projectile records the weapons' ammo names (grenades.md, Data).
    let weapons = weapons.map(|mut w| {
        if let Some(fx) = &fx {
            w.projectiles = ProjectileRecord::read_all(fx);
            w.impacts = ImpactRecord::read_all(fx);
            w.fires = FireRecord::read_all(fx);
        }
        w
    });
    let attrs = InstallAttributes {
        weapons,
        time_weight: server.as_ref().map(ServerTimeWeight::read),
        server: server.and_then(|d| ServerPlayer::read(&d)),
        models: models.map(|d| ModelsFile::read(&d)),
        mods: mods.map(|d| ModFile::read(&d)),
        marks: marks.and_then(|d| MarksFile::read(&d)),
        attachments: attachments.map(|d| AttachmentsFile::read(&d)),
        gear,
    };
    (attrs, problems)
}

/// Read an install's `ATTRIBUTES.rez` with the configured keys. `Err`
/// when it can't be opened or is locked for want of its key.
pub fn open_attributes(path: &std::path::Path) -> Result<InstallAttributes, String> {
    let keys = super::mount::keys()?;
    let archive = lithtech_formats::rez::RezArchive::open(path, &keys).map_err(|e| e.to_string())?;
    if archive.locked() {
        return Err(format!(
            "{}: encrypted, and no key for it in chimera.local.toml",
            path.display()
        ));
    }
    let mut mount = crate::mount::Mount::default();
    mount.push(archive);
    // Its reads are a load's too (`CHIMERA_VFS_LOG`): the asset store
    // serves them at the same paths (`attributes_from_mount`).
    if let Some(recorder) = crate::mount::assets::Recorder::from_env() {
        mount = crate::mount::served::record(mount, super::GAME, &recorder);
    }
    attributes_from_mount(&mount)
}

/// The attribute files read from `mount` (`ATTRIBUTES.rez` alone, or a
/// game mount served from the asset store, which has its files at the
/// same paths). `Err` when the weapons file isn't there.
pub fn attributes_from_mount(mount: &crate::mount::Mount) -> Result<InstallAttributes, String> {
    if !mount.contains(WEAPONS_FILE) {
        return Err(format!("no {WEAPONS_FILE} in the mount"));
    }
    let (attrs, problems) = read_attributes(|name| mount.read(name));
    for p in problems {
        warn!("Combat Arms: the install's {p}; using the built-in data for it");
    }
    Ok(attrs)
}

/// Our ID for an install name: lower case, anything but letters, digits,
/// `.`, `-` and `_` as `_` (`5.56mm M4A1` is `5.56mm_m4a1`).
pub fn slug(name: &str) -> String {
    name.chars()
        .map(|c| match c.to_ascii_lowercase() {
            c @ ('a'..='z' | '0'..='9' | '.' | '-' | '_') => c,
            _ => '_',
        })
        .collect()
}

/// When a mapped value is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum When {
    /// The table it goes in exists.
    Table,
    /// The key itself is already there.
    Key,
    /// The table is made when missing (`hooks` itself must exist).
    Create,
}

/// One mapped value: where it goes in a weapon document.
struct Set {
    path: Vec<String>,
    value: Value,
    when: When,
}

fn set(path: &[&str], value: impl Into<Value>) -> Set {
    Set {
        path: path.iter().map(|s| s.to_string()).collect(),
        value: value.into(),
        when: When::Table,
    }
}

fn floats(v: impl IntoIterator<Item = f64>) -> Value {
    Value::Array(v.into_iter().map(Value::Float).collect())
}

fn cone(c: &attributes::Cone) -> Value {
    let mut t = Table::new();
    t.insert("min".into(), Value::Integer(c.min.into()));
    t.insert("max".into(), Value::Integer(c.max.into()));
    t.insert("inc".into(), Value::Float(c.inc));
    t.insert("dec".into(), Value::Float(c.dec));
    Value::Table(t)
}

/// `CameraSway*` as the zoom hook's `sway`.
fn sway(s: &[f64; 4]) -> Value {
    let mut t = Table::new();
    for (k, v) in ["x_speed", "y_speed", "x_freq", "y_freq"].iter().zip(s) {
        t.insert((*k).into(), Value::Float(*v));
    }
    Value::Table(t)
}

fn state(p: &attributes::AmmoPerturb) -> Value {
    let mut t = Table::new();
    for (k, v) in [
        ("fire", p.fire),
        ("fire_move", p.fire_move),
        ("fire_duck", p.fire_duck),
        ("fire_move_duck", p.fire_move_duck),
        ("walk", p.walk),
        ("run", p.run),
        ("jump", p.jump),
    ] {
        t.insert(k.into(), Value::Float(v));
    }
    Value::Table(t)
}

/// What an install weapon (and its default ammo, `ammo_id` in our
/// document) sets in a weapon document: our parts (`trigger`, `magazine`,
/// `hitscan`, `ammo`) in the document's units (`length_scale`: metres per
/// world unit over the document's metres per length unit), and the
/// parameters of Combat Arms' hooks (`combat_arms:damage`, `:spread`,
/// `:recoil`, `:handling` in game/weapons.rs; `:inventory` in
/// game/inventory.rs; `:hud` in game/hud.rs) in the game's own
/// units (world units, ms, degrees, hit points).
fn weapon_values(w: &WeaponRecord, ammo: Option<(&str, &AmmoRecord)>, length_scale: f64) -> Vec<Set> {
    const DAMAGE: &str = "combat_arms:damage";
    const SPREAD: &str = "combat_arms:spread";
    const RECOIL: &str = "combat_arms:recoil";
    const HANDLING: &str = "combat_arms:handling";
    const INVENTORY: &str = "combat_arms:inventory";
    const HUD: &str = "combat_arms:hud";
    const ZOOM: &str = "combat_arms:zoom";
    const SPIN: &str = "combat_arms:spin";
    let mut out = vec![
        // `FireAniRate` on every weapon (the inventory table made where
        // the template has none: its other fields then land too).
        Set {
            when: When::Create,
            ..set(&["hooks", INVENTORY, "fire_rate"], w.fire_ani_rate)
        },
        set(&["trigger", "automatic"], w.rapid_fire_count != 1),
        Set {
            when: When::Key,
            ..set(&["trigger", "per_press"], i64::from(w.rapid_fire_count.max(0)))
        },
        set(&["magazine", "clip"], i64::from(w.shots_per_clip)),
        set(&["hitscan", "range"], w.range as f64 * length_scale),
        set(&["hitscan", "pellets"], i64::from(w.vectors_per_round)),
        set(&["hooks", DAMAGE, "roll"], w.damage_roll()),
        set(&["hooks", DAMAGE, "effect_range"], floats(w.effect_range.iter().map(|&e| e as f64))),
        set(&["hooks", DAMAGE, "fire_delay"], w.fire_delay as f64),
        set(&["hooks", SPREAD, "cone"], cone(&w.perturb)),
        set(&["hooks", SPREAD, "zoomed_cone"], cone(&w.zoomed)),
        set(&["hooks", SPREAD, "dynamic_factor"], w.dynamic_perturb_factor),
        set(&["hooks", RECOIL, "pitch"], w.base_cam_recoil_pitch),
        set(&["hooks", RECOIL, "accel_pitch"], w.base_cam_recoil_accel_pitch),
        set(&["hooks", RECOIL, "max_pitch"], w.max_cam_recoil_pitch),
        set(&["hooks", RECOIL, "random_pitch"], w.max_random_pitch),
        set(&["hooks", RECOIL, "wag_r"], w.base_cam_recoil_wag_r),
        set(&["hooks", RECOIL, "wag_l"], w.base_cam_recoil_wag_l),
        set(&["hooks", RECOIL, "max_wag"], w.max_cam_recoil_wag),
        set(&["hooks", RECOIL, "random_wag"], w.max_random_wag),
        set(&["hooks", RECOIL, "back_jitter"], w.base_cam_back_jitter),
        set(&["hooks", RECOIL, "back_jitter_recover"], w.back_jitter_recover),
        set(&["hooks", HANDLING, "move_penalty"], w.move_penalty),
        set(&["hooks", INVENTORY, "infinite"], w.infinite_ammo != 0),
        set(&["hooks", INVENTORY, "drop_on_death"], w.drop_by_player_dead),
        set(&["hooks", HUD, "crosshair"], i64::from(w.cross_hair_type.clamp(0, 255))),
        set(&["hooks", HUD, "disable_crosshair"], w.disable_crosshair != 0),
        set(&["hooks", HUD, "duck_perturb"], i64::from(w.duck_perturb)),
        set(&["hooks", HUD, "move_duck_perturb"], i64::from(w.move_duck_perturb)),
        set(&["hooks", HUD, "wide_count"], i64::from(w.wide_fire_max_num)),
        set(&["hooks", HUD, "bomb"], w.gun_type == 8),
        // weapon_classes.md: the zoom fields, the bolt's zoom-out, the
        // minigun's reversible spin.
        set(&["hooks", ZOOM, "zoom_time"], w.zoom_time),
        set(&["hooks", ZOOM, "sway"], sway(&w.camera_sway)),
        set(&["hooks", ZOOM, "reload_after_fire"], w.reload_after_fire != 0),
        set(&["hooks", ZOOM, "zoomed_move_penalty"], w.zoomed_move_penalty),
        // weapon_classes.md 3.3: `AccumPreFire` marks the spinning guns;
        // the spin's lengths are the copy's view model's (`timing`).
        Set {
            when: if w.accum_pre_fire != 0 { When::Create } else { When::Table },
            ..set(&["hooks", SPIN, "reversible"], w.accum_pre_fire != 0)
        },
    ];
    if w.triple_shot_mode != 0 {
        out.extend([
            set(&["hooks", SPREAD, "burst_cone"], cone(&w.triple_shot)),
            set(&["hooks", RECOIL, "burst_pitch"], w.triple_shot_base_cam_recoil_pitch),
            set(&["hooks", RECOIL, "burst_max_pitch"], w.triple_shot_max_cam_recoil_pitch),
        ]);
    }
    // A reticle (hud_crosshair.md A8) makes the HUD table if the document
    // has none: the knives' documents don't carry one.
    if !w.dot_sight_name.trim().is_empty() {
        out.push(Set {
            when: When::Create,
            ..set(&["hooks", HUD, "dot_sight"], w.dot_sight_name.trim())
        });
    }
    if let Some((id, a)) = ammo {
        out.extend([
            set(&["ammo", id, "max"], i64::from((a.max_amount - w.shots_per_clip).max(0))),
            set(&["hooks", DAMAGE, "damage"], a.inst_damage as f64),
            set(&["hooks", DAMAGE, "damage_type"], i64::from(a.inst_damage_type)),
            set(&["hooks", DAMAGE, "range_damage"], floats(a.range_damage.iter().copied())),
            set(&["hooks", DAMAGE, "move_penalty"], a.damage_move_penalty),
            set(&["hooks", DAMAGE, "move_penalty_duration"], a.damage_move_penalty_duration),
            set(&["hooks", SPREAD, "state"], state(&a.perturb)),
            set(&["hooks", SPREAD, "zoomed_state"], state(&a.zoomed_perturb)),
            set(&["hooks", SPREAD, "dec_delay"], a.dec_delay as f64),
            set(&["hooks", RECOIL, "jitter_dec_delay"], a.jitter_dec_delay as f64),
            set(&["hooks", RECOIL, "jitter_wait_recover"], a.jitter_wait_recover),
            set(&["hooks", RECOIL, "jitter_recover"], a.jitter_recover),
        ]);
    }
    out
}

/// What an install weapon's default ammo sets in the special weapons'
/// hooks (special_weapons.md, "Where the numbers live"): the blast, the
/// over-time sphere, the hit points, the counts and the knife's reach and
/// damages. Their projectile records (`FX.TXT`) aren't used: speeds,
/// lifetimes and boxes are ours (the grenades' are: `grenade_values`).
fn special_values(w: &WeaponRecord, a: &AmmoRecord, length_scale: f64) -> Vec<Set> {
    use super::special::{BOLT, DEVICE, PROJECTILE};
    let mut out = Vec::new();
    let area = |hook: &str, out: &mut Vec<Set>| {
        out.extend([
            set(&["hooks", hook, "area_damage"], a.area_damage as f64),
            set(&["hooks", hook, "area_radius"], a.area_damage_radius as f64),
        ]);
        if a.area_damage_type >= 0 {
            out.push(set(&["hooks", hook, "area_type"], i64::from(a.area_damage_type)));
        }
    };
    area(PROJECTILE, &mut out);
    area(DEVICE, &mut out);
    out.push(set(&["hooks", PROJECTILE, "health"], a.health));
    out.push(set(&["hooks", DEVICE, "health"], a.health));
    out.push(set(&["hooks", BOLT, "health"], a.health));
    for hook in [PROJECTILE] {
        out.extend([
            set(&["hooks", hook, "over_time", "rate"], a.prog_damage),
            set(&["hooks", hook, "over_time", "radius"], a.prog_damage_radius),
            set(&["hooks", hook, "over_time", "lifetime"], a.prog_damage_lifetime),
            set(&["hooks", hook, "over_time", "duration"], a.prog_damage_duration),
        ]);
    }
    out.extend([
        set(&["hooks", DEVICE, "spawned"], i64::from(a.spawned_amount.max(0))),
        set(&["hooks", DEVICE, "max"], i64::from(a.max_amount.max(0))),
        set(&["hooks", DEVICE, "caltrops", "rate"], a.prog_damage),
        set(&["hooks", DEVICE, "caltrops", "duration"], a.prog_damage_duration),
        set(&["hooks", DEVICE, "caltrops", "radius"], a.prog_damage_radius),
        set(&["hooks", DEVICE, "caltrops", "stun_penalty"], a.stunned_move_penalty),
        set(&["hooks", DEVICE, "caltrops", "stun_time"], a.stunned_time),
        set(&["hooks", DEVICE, "nerve", "amount"], a.area_damage as f64),
        set(&["hooks", DEVICE, "nerve", "duration"], a.area_damage as f64),
        set(&["hooks", DEVICE, "nerve", "radius"], a.area_damage_radius as f64),
        set(&["hooks", BOLT, "damage", "damage"], a.inst_damage as f64),
        set(&["hooks", BOLT, "damage", "damage_type"], i64::from(a.inst_damage_type)),
    ]);
    // The melee weapons: `Range` and the two buttons' damages.
    const KNIFE: &str = super::inventory::KNIFE_HOOK;
    out.extend([
        set(&["hooks", KNIFE, "range"], w.range as f64 * length_scale),
        set(&["hooks", KNIFE, "primary", "damage"], a.inst_damage as f64),
        set(&["hooks", KNIFE, "secondary", "damage"], a.alt_inst_damage as f64),
    ]);
    out
}

/// What a grenade's projectile record (`ProjectileFX#`, `FX.TXT`;
/// grenades.md "Data") sets in the `combat_arms:grenade` hook: the speed,
/// fuse, drop speed, `FireOffset` and `CanHitSameKind` where the record
/// has them, and the thrown model (`Model`, `ModelScale`, `Skin#`: drawn
/// in flight, `characters::models`).
fn grenade_values(p: &ProjectileRecord, length_scale: f64) -> Vec<Set> {
    const GRENADE: &str = super::grenades::HOOK;
    let mut out = Vec::new();
    let mut opt = |key: &str, v: Option<f64>, scale: f64| {
        if let Some(v) = v {
            out.push(set(&["hooks", GRENADE, key], v * scale));
        }
    };
    opt("speed", p.velocity, length_scale);
    opt("fuse", p.lifetime, 1.0);
    opt("drop_speed", p.drop_velocity, length_scale);
    opt("fire_offset", p.fire_offset, length_scale);
    if let Some(h) = p.can_hit_same_kind {
        out.push(set(&["hooks", GRENADE, "hits_same_kind"], h));
    }
    if !p.model.is_empty() {
        let skins = skin_paths(&p.skins).into_iter().map(Value::String).collect::<Vec<_>>();
        out.extend([
            set(&["hooks", GRENADE, "thrown_model"], super::model::normalize(&p.model)),
            set(&["hooks", GRENADE, "thrown_scale"], floats(p.model_scale)),
            set(&["hooks", GRENADE, "thrown_skins"], Value::Array(skins)),
        ]);
    }
    out
}

/// An attribute file's skin list as archive paths: entries that aren't
/// texture files (the stock data's `Hands` placeholder and the
/// `PV_L_ARM` / `PV_R_ARM` tags, view_models.md 2) are empty. A sprite
/// (`.spr`, an animated texture sequence: world_dat.md) is a texture
/// file: the model draws its first frame (`world::textures`).
pub fn skin_paths(skins: &[String]) -> Vec<String> {
    skins
        .iter()
        .map(|s| {
            let p = super::model::normalize(s);
            if is_skin_file(&p) { p } else { String::new() }
        })
        .collect()
}

/// Whether a skin entry names a texture file: a `.dtx` or a sprite
/// (`.spr`), any case.
pub fn is_skin_file(path: &str) -> bool {
    let p = path.trim().to_ascii_lowercase();
    p.ends_with(".dtx") || p.ends_with(".spr")
}

/// The install weapon's grenade hook value: whether a release makes the
/// team call-out (`PlayAutoThrowSound` on and `Guntype` 7).
fn sound_values(w: &WeaponRecord) -> Vec<Set> {
    vec![set(
        &["hooks", super::grenades::HOOK, "call_out"],
        w.play_auto_throw_sound && w.gun_type == THROWN_GUNTYPE,
    )]
}

/// A list of strings as normalized archive paths (or names), the trailing
/// empty entries cut; `lead` empty entries first (the PV flash-hider
/// groups are indexed by level, from 1).
fn string_array(items: &[String], lead: usize, normalize: bool) -> Value {
    let mut v: Vec<&String> = items.iter().collect();
    while v.last().is_some_and(|s| s.is_empty()) {
        v.pop();
    }
    let lead = if v.is_empty() { 0 } else { lead };
    Value::Array(
        std::iter::repeat_n(String::new(), lead)
            .chain(v.into_iter().map(|s| if normalize { super::model::normalize(s) } else { s.clone() }))
            .map(Value::String)
            .collect(),
    )
}

/// An effect name from an ammo record, as written into
/// `combat_arms:effects`: `None` keeps what the document has. A missing
/// key is no effect (""); a present but empty one is `empty_means`
/// (effects_sounds.md, "Effect tables"); a name our built-in tables
/// don't have keeps the template's (their records aren't read yet).
fn effect_name(name: &Option<String>, empty_means: &str, known: impl Fn(&str) -> bool) -> Option<String> {
    match name.as_deref() {
        None => Some(String::new()),
        Some(n) => {
            let n = if n.is_empty() { empty_means } else { n };
            if n.is_empty() || known(n) { Some(n.to_string()) } else { None }
        }
    }
}

/// The weapon's own sounds and effects (effects_sounds.md, "How effects
/// are described as data" and 11), into `combat_arms:effects` and
/// `combat_arms:view_model`, both made where the document has none (a
/// copy would otherwise play its template's): its `FireSnd`,
/// `SilencedFireSnd1..10`, `DryFireSnd`, `ReloadSnd1..3`, `SelectSnd`,
/// `DeselectSnd`, `MiscSound1..5` with their radii, the muzzle groups
/// (`PVMuzzleFXName`, `HHMuzzleFXName`, the flash-hider groups) and the
/// default ammo's `ImpactFXName`, `FireFXName` and `TracerFXName`. A
/// missing field is the spec's default: no sound, no group, no effect.
/// A thrown weapon's ammo effects are the grenade hook's.
fn effects_patch(doc: &mut Table, w: &WeaponRecord, ammo: Option<&AmmoRecord>) {
    const EFFECTS: &str = super::effects::data::EFFECTS;
    const VIEW: &str = "combat_arms:view_model";
    let Some(Value::Table(hooks)) = doc.get_mut("hooks") else { return };
    let tables = super::effects::data::CaFxTables::built_in();
    let text = |s: &str| Value::String(super::model::normalize(s));
    let made = !hooks.contains_key(EFFECTS);
    let Value::Table(fx) = hooks.entry(EFFECTS).or_insert_with(|| Value::Table(Table::new())) else { return };
    if made && w.gun_type == THROWN_GUNTYPE {
        fx.insert("ammo_kind".into(), Value::String("thrown".into()));
    }
    fx.insert("fire_sound".into(), text(&w.fire_snd));
    fx.insert("fire_radius".into(), Value::Float(f64::from(w.fire_snd_radius)));
    fx.insert("alt_fire_sound".into(), text(&w.alt_fire_snd));
    fx.insert("silenced_sounds".into(), string_array(&w.silenced_fire_snd, 0, true));
    fx.insert("dry_fire_sound".into(), text(&w.dry_fire_snd));
    fx.insert("reload_sounds".into(), string_array(&w.reload_snd, 0, true));
    fx.insert("select_sound".into(), text(&w.select_snd));
    fx.insert("deselect_sound".into(), text(&w.deselect_snd));
    fx.insert("misc_sounds".into(), string_array(&w.misc_sounds, 0, true));
    fx.insert("reload_radius".into(), Value::Float(f64::from(w.reload_snd_radius)));
    fx.insert("select_radius".into(), Value::Float(f64::from(w.select_snd_radius)));
    fx.insert("hh_muzzle".into(), Value::String(w.hh_muzzle_fx.clone()));
    fx.insert("hh_flash_hider".into(), string_array(&w.hh_flash_hider_fx, 0, false));
    if w.gun_type != THROWN_GUNTYPE
        && let Some(a) = ammo
    {
        let names = [
            ("impact", effect_name(&a.impact_fx, "BulletFmj", |n| tables.impact(n).is_some())),
            ("fire", effect_name(&a.fire_fx, "", |n| tables.fire(n).is_some())),
            ("tracer", effect_name(&a.tracer_fx, "Standard", |n| tables.tracer(n).is_some())),
        ];
        for (key, name) in names {
            if let Some(n) = name {
                fx.insert(key.into(), Value::String(n));
            }
        }
    }
    let Value::Table(view) = hooks.entry(VIEW).or_insert_with(|| Value::Table(Table::new())) else { return };
    view.insert("muzzle_fx".into(), Value::String(w.pv_muzzle_fx.clone()));
    view.insert("flash_hider_fx".into(), string_array(&w.pv_flash_hider_fx, 1, false));
    view.insert(
        "attach_fx".into(),
        Value::Array(w.pv_attach_fx.iter().map(|g| Value::String(g.clone())).collect()),
    );
    view.insert("attach_fx_always".into(), Value::Boolean(w.pv_attach_fx_always));
}

/// `Guntype` of the thrown weapons.
const THROWN_GUNTYPE: i32 = 7;

/// The install weapon's models and their placement: `PVModelNormal`
/// with `PVSkin#` and `Pos` as the view model, `HHModel` as the held one,
/// `MuzzlePos` and `BreachOffset` in our `combat_arms:view_model` table
/// (when the document has one).
fn model_values(w: &WeaponRecord) -> Vec<Set> {
    let vec3 = |v: [f64; 3]| floats(v);
    let mut out = vec![
        set(&["hooks", super::view_model::VIEW, "muzzle_pos"], vec3(w.muzzle_pos)),
        set(&["hooks", super::view_model::VIEW, "breach_offset"], vec3(w.breach_offset)),
    ];
    if !w.pv_model.is_empty() {
        let skins = skin_paths(&w.pv_skins).into_iter().map(Value::String).collect::<Vec<_>>();
        out.extend([
            set(&["models", "view"], super::model::normalize(&w.pv_model)),
            set(&["models", "view_skins"], Value::Array(skins)),
            set(&["models", "view_offset"], vec3(w.pos)),
        ]);
    }
    if !w.hh_model.is_empty() {
        out.push(set(&["models", "world"], super::model::normalize(&w.hh_model)));
    }
    out
}

/// Our IDs (after `combat_arms:`) whose install `Name` is not their
/// slug: the knife is the install's `M9`, the incendiary grenade its
/// `M-23F`.
const INSTALL_NAMES: &[(&str, &str)] = &[("m9_knife", "M9"), ("m23f", "M-23F")];

/// Whether install weapon `name` is our document `id` (without
/// `combat_arms:`): case-insensitively by name ("Our loader"), through
/// its slug or `INSTALL_NAMES`.
fn is_install_name(name: &str, id: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    match INSTALL_NAMES.iter().find(|(ours, _)| *ours == id) {
        Some((_, install)) => name.eq_ignore_ascii_case(install),
        None => slug(name) == id,
    }
}

/// Weapon `id`'s record in `file` (the merge's match: `combat_arms:` and
/// the slug of its `Name`).
pub fn install_weapon_in<'a>(file: &'a WeaponsFile, id: &str) -> Option<&'a WeaponRecord> {
    let id = id.strip_prefix(NS)?;
    file.weapons.iter().find(|w| is_install_name(&w.name, id))
}

/// Every install weapon record by the id `install_weapon_in` finds it
/// under (`combat_arms:` and the slug of its `Name`, or our id for an
/// `INSTALL_NAMES` name; the first record of a name, as there), in one
/// pass.
pub fn install_weapons_by_id(file: &WeaponsFile) -> std::collections::BTreeMap<String, &WeaponRecord> {
    let mut out = std::collections::BTreeMap::new();
    for w in file.weapons.iter().filter(|w| !w.name.is_empty()) {
        let slugged = slug(&w.name);
        // A slug that is one of `INSTALL_NAMES`' ids matches only its
        // install name.
        if !INSTALL_NAMES.iter().any(|(ours, _)| *ours == slugged) {
            out.entry(format!("{NS}{slugged}")).or_insert(w);
        }
        for (ours, _) in INSTALL_NAMES.iter().filter(|(_, install)| w.name.eq_ignore_ascii_case(install)) {
            out.entry(format!("{NS}{ours}")).or_insert(w);
        }
    }
    out
}

/// Write `s` into `doc` when its rule allows; whether it was written.
fn apply(doc: &mut Table, s: &Set) -> bool {
    let Some((last, parents)) = s.path.split_last() else {
        return false;
    };
    let mut t = doc;
    for p in parents {
        if s.when == When::Create && !t.contains_key(p) {
            t.insert(p.clone(), Value::Table(Table::new()));
        }
        t = match t.get_mut(p) {
            Some(Value::Table(x)) => x,
            _ => return false,
        };
    }
    if s.when == When::Key && !t.contains_key(last) {
        return false;
    }
    t.insert(last.clone(), s.value.clone());
    true
}

fn doc_id(doc: &Table) -> Option<&str> {
    doc.get("id")?.as_str()
}

/// The weapon's offsets for its parts' models
/// (`attachments::PartOffsets`).
fn part_offsets(w: &WeaponRecord) -> Value {
    let one = |[pos, rot]: [[f64; 3]; 2]| {
        let mut t = Table::new();
        t.insert("pos".into(), floats(pos));
        t.insert("rot".into(), floats(rot));
        Value::Table(t)
    };
    let mut t = Table::new();
    t.insert("muzzle_pv".into(), one(w.muzzle_pv_off));
    t.insert("scope_pv".into(), one(w.scope_pv_off));
    t.insert("muzzle_hh".into(), one(w.muzzle_hh_off));
    t.insert("scope_hh".into(), one(w.scope_hh_off));
    Value::Table(t)
}

/// What an install weapon (the weapons file's record `w`, index `index`)
/// sets in a document's `combat_arms:attachments` table: its defaults,
/// sound radii and, with the modification file, the accepted list of its
/// record of the same index (attachments.md Q13; none: empty, Q2).
fn attachment_values(w: &WeaponRecord, index: usize, mods: Option<&ModFile>) -> Vec<Set> {
    const ATTACH: &str = super::attachments::ATTACHMENTS;
    let mut defaults = Table::new();
    for (k, v) in [
        ("muzzle", &w.default_muzzle_attach),
        ("sight", &w.default_scope_attach),
        ("magazine", &w.default_magazine_attach),
        ("bullet", &w.default_bullet_attach),
        ("mech_sight", &w.default_mech_sight_attach),
    ] {
        if !v.is_empty() {
            defaults.insert(k.into(), Value::String(v.clone()));
        }
    }
    let mut out = vec![
        set(&["hooks", ATTACH, "defaults"], Value::Table(defaults)),
        set(&["hooks", ATTACH, "default_sight_visible"], w.default_scope_visible),
        set(&["hooks", ATTACH, "fire_sound_radius"], i64::from(w.fire_snd_radius)),
        set(&["hooks", ATTACH, "part_offsets"], part_offsets(w)),
        set(
            &["hooks", ATTACH, "silenced_radius"],
            Value::Array(w.silenced_fire_radius.iter().map(|r| Value::Integer((*r).into())).collect()),
        ),
    ];
    if let Some(mods) = mods {
        let accepts = match mods.weapons.get(index) {
            Some(r) => {
                if !r.name.eq_ignore_ascii_case(&w.name) {
                    info!(
                        "Combat Arms: modification record Weapon{index} is named \"{}\", weapon {index} \"{}\" (linked by position)",
                        r.name, w.name
                    );
                }
                out.push(set(
                    &["hooks", ATTACH, "slot_options"],
                    Value::Array(r.options.iter().map(|o| Value::Integer((*o).into())).collect()),
                ));
                r.accepts
                    .iter()
                    .map(|(name, variant)| {
                        let mut t = Table::new();
                        t.insert("name".into(), Value::String(name.clone()));
                        t.insert("variant".into(), Value::Integer((*variant).into()));
                        Value::Table(t)
                    })
                    .collect()
            }
            None => {
                warn!("Combat Arms: no modification record for weapon {index} \"{}\": it accepts no attachments", w.name);
                Vec::new()
            }
        };
        out.push(set(&["hooks", ATTACH, "accepts"], Value::Array(accepts)));
    }
    out
}

/// Patch one document with an install weapon (and its default ammo when
/// the document's magazine names an ammo).
fn patch(doc: &mut Table, w: &WeaponRecord, file: &WeaponsFile, mods: Option<&ModFile>, meters_per_unit: f64) {
    let doc_length = doc
        .get("units")
        .and_then(|u| u.get("length"))
        .and_then(|l| l.as_float().or_else(|| l.as_integer().map(|i| i as f64)))
        .unwrap_or(1.0);
    let ammo_id = doc.get("magazine").and_then(|m| m.get("ammo")).and_then(|a| a.as_str()).map(String::from);
    let ammo = match (&ammo_id, file.default_ammo(w)) {
        (Some(id), Some(a)) => Some((id.as_str(), a)),
        _ => None,
    };
    // In f32, as both scales are read: equal scales give exactly 1.
    let length_scale = (meters_per_unit as f32 / doc_length as f32) as f64;
    // A weapon that fits parts can fit a sight, and every sight zooms
    // (attachments.md "Sights"), whatever its class: a document of ours
    // without the zoom table (the M60, the Remington 870, the rockets)
    // gets an empty one for the install's values (`apply` writes only into
    // tables there), or the machine guns and shotguns the install gives a
    // sight could never zoom.
    if let Some(Value::Table(hooks)) = doc.get_mut("hooks")
        && hooks.contains_key(super::attachments::ATTACHMENTS)
    {
        hooks.entry("combat_arms:zoom").or_insert_with(|| Value::Table(Table::new()));
    }
    // An impact projectile weapon's round is its projectile: the frame's
    // trace stays the document's (a unit's: it traces nothing), not the
    // record's `Range`.
    let projectile_range = hook_table(doc, super::special::PROJECTILE)
        .and(doc.get("hitscan"))
        .and_then(|h| h.get("range"))
        .cloned();
    for s in weapon_values(w, ammo, length_scale) {
        apply(doc, &s);
    }
    if let Some(r) = projectile_range {
        apply(doc, &set(&["hitscan", "range"], r));
    }
    // Its own sounds and effects, not its template's.
    effects_patch(doc, w, file.default_ammo(w));
    for s in sound_values(w) {
        apply(doc, &s);
    }
    if let Some(a) = file.default_ammo(w) {
        for s in special_values(w, a, length_scale) {
            apply(doc, &s);
        }
        // A grenade's flight and look: its projectile record.
        if let Some(p) = file.projectile(a) {
            for s in grenade_values(p, length_scale) {
                apply(doc, &s);
            }
        }
        // An impact projectile's flight, look, lasting sphere and sounds
        // (special_weapons.md 1, "Where the numbers live").
        if hook_table(doc, super::special::PROJECTILE).is_some() {
            projectile_patch(doc, w, a, file, length_scale);
        }
    }
    // The models (view_models.md 1, 2, 4; characters.md 2): the install's
    // files replace ours, its placement with them.
    if !(w.pv_model.is_empty() && w.hh_model.is_empty()) && !doc.contains_key("models") {
        doc.insert("models".into(), Value::Table(Table::new()));
    }
    for s in model_values(w) {
        apply(doc, &s);
    }
    // `ReloadPerBullet` 0: a magazine reload (the shell part stays when it
    // is 1: a copy's timings are then its own model's, `timing_table`).
    if w.reload_per_bullet == 0
        && let Some(Value::Table(m)) = doc.get_mut("magazine")
    {
        m.remove("shells");
    }
    // The inventory class: the order list it is in (our `slot` is the
    // class - 1; in none, ours stays).
    let id = file.weapons.iter().position(|x| std::ptr::eq(x, w));
    if let Some(i) = id {
        for s in attachment_values(w, i, mods) {
            apply(doc, &s);
        }
    }
    if let Some(class) = id.map(|i| file.inventory_class(i)).filter(|c| *c > 0) {
        apply(doc, &set(&["slot"], i64::from(class - 1)));
    }
}

/// Our weapon documents with the install's weapons merged in ("Defaults
/// and overrides", "Our loader"): `(name, TOML text)` in registration
/// order, ours first. `meters_per_unit`: metres per world unit.
pub fn merge_weapon_docs(
    ours: &[(&str, &str)],
    install: &WeaponsFile,
    meters_per_unit: f64,
) -> Result<Vec<(String, String)>, String> {
    merge_weapon_docs_with_mods(ours, install, None, meters_per_unit)
}

/// `merge_weapon_docs` with the install's modification file: each
/// document's accepted list from the record of its install weapon's index.
pub fn merge_weapon_docs_with_mods(
    ours: &[(&str, &str)],
    install: &WeaponsFile,
    mods: Option<&ModFile>,
    meters_per_unit: f64,
) -> Result<Vec<(String, String)>, String> {
    let mut docs = Vec::new();
    for (name, text) in ours {
        let t: Table = text.parse().map_err(|e| format!("{name}: {e}"))?;
        docs.push((name.to_string(), t));
    }
    // Which install weapon each of ours is (`None`: no match).
    let matched: Vec<Option<usize>> = docs
        .iter()
        .map(|(_, t)| {
            let id = doc_id(t)?.strip_prefix(NS)?;
            install.weapons.iter().position(|w| is_install_name(&w.name, id))
        })
        .collect();
    let mut ids: std::collections::HashSet<String> = docs.iter().filter_map(|(_, t)| doc_id(t).map(String::from)).collect();
    let mut ammo_ids: std::collections::HashSet<String> = docs
        .iter()
        .filter_map(|(_, t)| t.get("ammo")?.as_table())
        .flat_map(|a| a.keys().cloned())
        .collect();
    for ((_, t), m) in docs.iter_mut().zip(&matched) {
        if let Some(i) = m {
            patch(t, &install.weapons[*i], install, mods, meters_per_unit);
        }
    }
    // New records: copies of our matched document of the same class.
    let mut new = Vec::new();
    let mut skipped = 0usize;
    for (i, w) in install.weapons.iter().enumerate() {
        if w.name.is_empty() || matched.contains(&Some(i)) {
            continue;
        }
        let id = format!("{NS}{}", slug(&w.name));
        let template = template_for(install, w, &matched, &docs);
        let (Some(template), Some(ammo)) = (template, file_default_ammo(install, w)) else {
            skipped += 1;
            continue;
        };
        if !ids.insert(id.clone()) {
            continue;
        }
        let mut t = template.clone();
        t.insert("id".into(), Value::String(id.clone()));
        t.remove("ammo");
        // A gun keeps its template's class (an automatic gun game's); a
        // thrown one doesn't: a smoke or flash copied from a frag isn't
        // a grenade that kills.
        if t.get("class").and_then(Value::as_str) == Some("grenade") {
            t.remove("class");
        }
        let ammo_id = format!("{NS}{}", slug(&ammo.name));
        if ammo_ids.insert(ammo_id.clone()) {
            let mut table = Table::new();
            let mut entry = Table::new();
            entry.insert("max".into(), Value::Integer(0));
            table.insert(ammo_id.clone(), Value::Table(entry));
            t.insert("ammo".into(), Value::Table(table));
        }
        if let Some(Value::Table(m)) = t.get_mut("magazine") {
            m.insert("ammo".into(), Value::String(ammo_id));
        }
        patch(&mut t, w, install, mods, meters_per_unit);
        timing_table(&mut t);
        infected_table(&mut t, w);
        new.push((format!("install:{}", w.name), t));
    }
    if skipped > 0 {
        info!("Combat Arms: {skipped} install weapons skipped (no weapon of their class in our data, or no ammo)");
    }
    docs.extend(new);
    docs.into_iter()
        .map(|(n, t)| toml::to_string(&t).map(|s| (n.clone(), s)).map_err(|e| format!("{n}: {e}")))
        .collect()
}

/// A copy's own fire sequence, reload and draw: the `combat_arms:timing`
/// table (`weapons::timing`, read from its view model when built) on a
/// gun with a trigger and a view model, and on a knife (its swings and
/// hit keys); not on a grenade (its own hook times it).
fn timing_table(doc: &mut Table) {
    use super::weapons::timing::TIMING;
    let has_view = doc.get("models").and_then(|m| m.get("view")).is_some();
    let Some(Value::Table(hooks)) = doc.get("hooks") else { return };
    let knife = hooks.contains_key(super::inventory::KNIFE_HOOK);
    if !has_view || !(doc.contains_key("trigger") || knife) || hooks.contains_key(super::grenades::HOOK) {
        return;
    }
    if let Some(Value::Table(hooks)) = doc.get_mut("hooks") {
        hooks.insert(TIMING.into(), Value::Table(Table::new()));
    }
}

/// An infected's weapon (a knife-class copy whose record plays the
/// infected leg set, `LowerAniType` 1 or 2: characters.md 5, the
/// Quarantine hands): the `combat_arms:infected` table (`rungs::INFECTED`,
/// a gun game's infected rung).
fn infected_table(doc: &mut Table, w: &WeaponRecord) {
    if !matches!(w.lower_ani_type, 1 | 2) {
        return;
    }
    if let Some(Value::Table(hooks)) = doc.get_mut("hooks")
        && hooks.contains_key(super::inventory::KNIFE_HOOK)
    {
        hooks.insert(super::rungs::INFECTED.into(), Value::Table(Table::new()));
    }
}

/// The document a new install weapon `w` is copied from: of our matched
/// documents with its `Guntype`, the first whose install weapon's ammo
/// fires the same projectile record (`ProjectileFXName`: a flame weapon
/// copies the flamethrower, a rocket a rocket launcher, whatever comes
/// first in `Guntype` 9), else the first of its `Guntype`.
fn template_for<'a>(
    install: &WeaponsFile,
    w: &WeaponRecord,
    matched: &[Option<usize>],
    docs: &'a [(String, Table)],
) -> Option<&'a Table> {
    let same_class: Vec<(usize, &Table)> = matched
        .iter()
        .zip(docs)
        .filter_map(|(m, (_, t))| m.filter(|j| install.weapons[*j].gun_type == w.gun_type).map(|j| (j, t)))
        .collect();
    let projectile = |r: &WeaponRecord| install.default_ammo(r).map(|a| a.projectile_fx.to_ascii_lowercase()).filter(|p| !p.is_empty());
    let mine = projectile(w);
    same_class
        .iter()
        .find(|(j, _)| mine.is_some() && projectile(&install.weapons[*j]) == mine)
        .or(same_class.first())
        .map(|(_, t)| *t)
}

/// The table of hook `name` in a weapon document, if it has one.
fn hook_table<'a>(doc: &'a Table, name: &str) -> Option<&'a Table> {
    doc.get("hooks")?.get(name)?.as_table()
}

/// `ProgDamageType` as the lasting sphere's kind (burn 3, poison 9,
/// bleeding 1); another type: none.
fn over_time_kind(ty: i32) -> Option<&'static str> {
    match ty {
        3 => Some("burn"),
        9 => Some("poison"),
        1 => Some("bleed"),
        _ => None,
    }
}

/// An impact projectile weapon's install values (special_weapons.md 1.1
/// to 1.3, 4; effects_sounds.md 6, 11): its projectile record's
/// `Velocity`, `Lifetime`, `Gravity`, `CanHitSameKind` and world model
/// (`Model`, `ModelScale`, `Skin#`) and looks in flight (`look_table`) in `combat_arms:projectile`; the
/// ammo's lasting sphere (`ProgDamage*`, made when the ammo has one and
/// the document doesn't: a flame weapon copied from a rocket's) and the
/// `Guntype` 9 shooter exemption; and its sounds: the weapon's `FireSnd`
/// and `FireSndRadius`, the ammo's `ImpactFXName` with that record
/// (`FX.TXT`) in `combat_arms:effects` (made when missing), launched ammo.
fn projectile_patch(doc: &mut Table, w: &WeaponRecord, a: &AmmoRecord, file: &WeaponsFile, length_scale: f64) {
    use super::special::PROJECTILE;
    const EFFECTS: &str = super::effects::data::EFFECTS;
    let mut out = Vec::new();
    if let Some(p) = file.projectile(a) {
        if let Some(v) = p.velocity {
            out.push(set(&["hooks", PROJECTILE, "speed"], v * length_scale));
        }
        if let Some(l) = p.lifetime {
            out.push(set(&["hooks", PROJECTILE, "lifetime"], l));
        }
        if let Some(g) = p.gravity {
            out.push(set(&["hooks", PROJECTILE, "gravity"], g));
        }
        if let Some(h) = p.can_hit_same_kind {
            out.push(set(&["hooks", PROJECTILE, "hits_same_kind"], h));
        }
        if p.model.is_empty() {
            if let Some(Value::Table(t)) = doc.get_mut("hooks").and_then(|h| h.get_mut(PROJECTILE)) {
                for k in ["model", "model_scale", "skins"] {
                    t.remove(k);
                }
            }
        } else {
            let skins = skin_paths(&p.skins).into_iter().map(Value::String).collect::<Vec<_>>();
            out.extend([
                set(&["hooks", PROJECTILE, "model"], super::model::normalize(&p.model)),
                set(&["hooks", PROJECTILE, "model_scale"], floats(p.model_scale)),
                set(&["hooks", PROJECTILE, "skins"], Value::Array(skins)),
            ]);
        }
        out.push(set(&["hooks", PROJECTILE, "look"], Value::Table(look_table(&p.look))));
    }
    out.push(set(&["hooks", PROJECTILE, "shooter_exempt"], w.gun_type == ROCKET_GUNTYPE));
    // The lasting sphere: made for ammo that has one (its numbers are
    // `special_values`').
    if a.prog_damage_lifetime > 0.0
        && let Some(kind) = over_time_kind(a.prog_damage_type)
        && let Some(Value::Table(t)) = doc.get_mut("hooks").and_then(|h| h.get_mut(PROJECTILE))
    {
        let mut ot = Table::new();
        ot.insert("kind".into(), Value::String(kind.into()));
        ot.insert("rate".into(), Value::Float(a.prog_damage));
        ot.insert("radius".into(), Value::Float(a.prog_damage_radius));
        ot.insert("lifetime".into(), Value::Float(a.prog_damage_lifetime));
        ot.insert("duration".into(), Value::Float(a.prog_damage_duration));
        t.insert("over_time".into(), Value::Table(ot));
    }
    if let Some(Value::Table(hooks)) = doc.get_mut("hooks") {
        hooks.entry(EFFECTS).or_insert_with(|| Value::Table(Table::new()));
    }
    out.extend([
        set(&["hooks", EFFECTS, "ammo_kind"], "thrown"),
        set(&["hooks", EFFECTS, "launched"], a.kind != GRENADE_AMMO_TYPE),
        set(&["hooks", EFFECTS, "fire_sound"], super::model::normalize(&w.fire_snd)),
        set(&["hooks", EFFECTS, "fire_radius"], w.fire_snd_radius as f64),
    ]);
    // The launcher's own-muzzle group and casings (special_weapons.md
    // Follow-up 7): its ammo's FireFX record.
    match file.fire(a) {
        Some(r) => {
            out.push(set(&["hooks", EFFECTS, "fire"], r.name.clone()));
            let mut t = Table::new();
            t.insert("fx".into(), Value::String(r.fx_name.clone()));
            t.insert("shells".into(), Value::Boolean(r.eject_shells));
            t.insert("shell_model".into(), Value::String(super::model::normalize(&r.shell_model)));
            t.insert("shell_skin".into(), Value::String(super::model::normalize(&r.shell_skin)));
            t.insert("shell_scale".into(), Value::Float(r.shell_scale));
            t.insert("velocity".into(), floats(r.velocity));
            out.push(set(&["hooks", EFFECTS, "fire_record"], Value::Table(t)));
        }
        None => {
            if let Some(Value::Table(t)) = doc.get_mut("hooks").and_then(|h| h.get_mut(EFFECTS)) {
                t.remove("fire_record");
            }
        }
    }
    match file.impact(a) {
        Some(r) => {
            out.push(set(&["hooks", EFFECTS, "impact"], r.name.clone()));
            out.push(set(&["hooks", EFFECTS, "impact_record"], Value::Table(impact_table(r, a))));
        }
        None => {
            // No record of its own: nothing a template carried.
            out.push(set(&["hooks", EFFECTS, "impact"], ""));
            if let Some(Value::Table(t)) = doc.get_mut("hooks").and_then(|h| h.get_mut(EFFECTS)) {
                t.remove("impact_record");
            }
        }
    }
    for s in out {
        apply(doc, &s);
    }
}

/// The ammo `Type` of grenades: their blasts play the surface's grenade
/// group; other launched ammo the surface's impact group
/// (special_weapons.md Follow-up 8).
const GRENADE_AMMO_TYPE: i32 = 2;

/// A projectile record's looks in flight as `special::ProjectileLook`'s
/// table (special_weapons.md Follow-up (2026-10-30) 1-3): the group,
/// fly sound, flare and smoke trail; sounds and sprites as archive paths.
fn look_table(l: &attributes::ProjectileLookRecord) -> Table {
    let mut t = Table::new();
    t.insert("fx".into(), Value::String(l.fx.clone()));
    t.insert("fx_loop".into(), Value::Boolean(l.fx_loop));
    t.insert("fx_smooth".into(), Value::Boolean(l.fx_smooth));
    t.insert("fly_sound".into(), Value::String(super::model::normalize(&l.fly_sound)));
    t.insert("sound_radius".into(), Value::Float(l.sound_radius));
    if let Some((sprite, scale, dy)) = &l.flare {
        let mut f = Table::new();
        f.insert("sprite".into(), Value::String(super::model::normalize(sprite)));
        f.insert("scale".into(), floats(*scale));
        f.insert("y_offset".into(), Value::Float(*dy));
        t.insert("flare".into(), Value::Table(f));
    }
    if let Some(n) = l.smoke_trail {
        t.insert("smoke_trail".into(), Value::Integer(n.max(0).into()));
    }
    t
}

/// An impact record as our effects data writes one (`effects::data::
/// ImpactFx`): `Surface` as `surface`, files as archive paths, `area`
/// when the ammo does area or lasting damage. Launched burn ammo (`InstDamageType` 3)
/// plays no surface sound (effects_sounds.md 11: "burn → none").
fn impact_table(r: &ImpactRecord, a: &AmmoRecord) -> Table {
    let surface = |v: &str| if v.eq_ignore_ascii_case("surface") { "surface".to_string() } else { super::model::normalize(v) };
    let mut sound = surface(&r.sound);
    if sound == "surface" && a.inst_damage_type == 3 {
        sound.clear();
    }
    let mut t = Table::new();
    t.insert("sound".into(), Value::String(sound));
    t.insert("sound_radius".into(), Value::Float(r.sound_radius));
    t.insert("create_mark".into(), Value::Boolean(r.create_mark));
    t.insert("mark".into(), Value::String(surface(&r.mark)));
    t.insert("mark_scale".into(), Value::Float(r.mark_scale));
    if let Some(c) = r.mark_color_ratio {
        t.insert("mark_color_ratio".into(), Value::Float(c));
    }
    t.insert("surface_fx".into(), Value::Boolean(r.do_surface_fx));
    t.insert("fx".into(), Value::String(r.fx_name.clone()));
    t.insert("area".into(), Value::Boolean(a.area_damage > 0 || a.prog_damage_lifetime > 0.0));
    t
}

/// `Guntype` of the rocket class (special_weapons.md: launchers and the
/// flamethrower): its spheres never burn the shooter.
const ROCKET_GUNTYPE: i32 = 9;

/// A weapon's default ammo record; a weapon naming ammo none of which
/// resolves is logged ("Edge cases").
fn file_default_ammo<'a>(file: &'a WeaponsFile, w: &WeaponRecord) -> Option<&'a AmmoRecord> {
    let a = file.default_ammo(w);
    if a.is_none() && w.names_ammo {
        warn!("Combat Arms: install weapon \"{}\" names no ammo that exists", w.name);
    }
    a
}

/// The install's body types over our movement data's figure:
/// `FigureType0` replaces `character.figure` (the reference body) and
/// every `FigureType#` is `figures` (a vest record's body type).
pub fn merge_movement(data: &mut MovementData, models: &ModelsFile) {
    let Some(first) = models.figure_types.first() else {
        return;
    };
    let figure = |f: &FigureTypeRecord| Figure {
        dash_speed: f.dash_speed as f32,
        dash_gauge: f.dash_gauge as f32,
        gauge_recovery: f.gauge_rec as f32,
        max_stamina: f.max_stamina as f32,
        variable_speed: f.variable_speed as f32,
        variable_weight: f.variable_weight as f32,
        variable_jump: f.variable_jump as f32,
        stiff_duration: f.stiff_duration as f32,
    };
    data.character.figure = figure(first);
    data.figures = models.figure_types.iter().map(figure).collect();
}

/// Combat Arms' data (`CaData`), loaded as the game starts (so a bad file
/// fails at once, not at the first weapon given). Keeps the `CaData` the
/// app already has (a harness's, a minigame's), else inserts the default.
pub struct CaDataPlugin;

impl Plugin for CaDataPlugin {
    fn build(&self, app: &mut App) {
        CaData::of(app).weapon_set();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::data::WeaponSet;

    /// Every file in data/weapons is built in (`WEAPON_DATA`), as it is
    /// on disk.
    #[test]
    fn every_data_file_is_built_in() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weapons");
        let files = WeaponSet::read_dir(&dir).unwrap();
        for (name, text) in &files {
            let built_in = WEAPON_DATA.iter().find(|(n, _)| n == name);
            assert_eq!(built_in.map(|d| d.1), Some(text.as_str()), "{name} not in `WEAPON_DATA`");
        }
        assert_eq!(files.len(), WEAPON_DATA.len());
    }

    /// Every placeholder document says its numbers are placeholders until
    /// the specs replace them.
    #[test]
    fn placeholder_data_says_so() {
        for (name, text) in WEAPON_DATA.iter().filter(|(n, _)| n.starts_with("placeholder_")) {
            assert!(text.contains("PLACEHOLDER"), "{name}: no PLACEHOLDER note");
        }
        // The built-in documents parse on their own and `CaData::builtin()`
        // holds them.
        let builtin = WeaponSet::parse(WEAPON_DATA, &[]).unwrap();
        assert_eq!(builtin.weapons.len(), WEAPON_DATA.len());
        let data = CaData::builtin();
        for id in data.builtin_weapon_ids() {
            assert!(data.weapon_set().get(id).is_some(), "{id}");
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("5.56mm M4A1"), "5.56mm_m4a1");
        assert_eq!(slug("M4A1"), "m4a1");
        assert_eq!(slug("K-2 (Gold)"), "k-2__gold_");
    }
}

