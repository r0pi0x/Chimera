//! TF2's unlocks (docs/plans/active/tf2-weapons.md, batches 0 and 1;
//! specs/tf2/item_attributes.md): each one a stock weapon class plus
//! attributes, read from a converted pack, never from the game at run time.
//!
//! **The converter** (`convert`, `CONVERTER` version `VERSION`): host-side,
//! `chimera_pack sets tf2 --only items` (`generated`) reads the install's
//! item schema (`scripts/items/items_game.txt`) and each item class's
//! weapon script (decrypted with the host's key through
//! `scripts::decrypting`; the key never printed) and writes one file,
//! `PACK_PATH` (`chimera/gen/tf2.items/v<VERSION>/items.toml`), into the
//! install build's manifest as set `items`: a record per weapon item
//! (its class, slot, classes, models, folded attributes, base and
//! effective numbers). Deterministic; `VERSION` is bumped whenever an
//! output byte could change. Nothing of the schema is committed.
//!
//! **In the game** (`load`, `ItemWeapon`): the pack is read from the
//! game's mount (the asset store's set, layered under the install or
//! standing in for it); data/items.toml (ours) names the items we play,
//! by definition index, with the weapon of ours each item class starts
//! from (`[bases]`). Each becomes a weapon document (`docs::item_doc`)
//! registered with the stock ones (`weapons::script_numbers::GunNumbers`,
//! so the server's documents carry them to clients), named `tf2:<name>`
//! (`tf2:winger`), its classes added to the class table
//! (`classes::Classes::add_gun`) and its holder's numbers on its
//! `tf2:item` hook (`mods`). A ladder draws one that deals damage
//! (its base's `class` kept); the jumpers don't. Without the pack (no
//! store set, an older store) no item is offered and stock plays on.
//! Wearables (`wearable`) make no weapon.
//!
//! **Its looks** (`docs::item_doc`, the record's `model` and `visuals`):
//! its own `c_` model as its view and held model (on its class's arms,
//! bone-merged as the stock ones: `view::load`, `maps::add_guns`, which
//! take the items on the mount from `on_mount`), in its own skin, its own
//! kill icon (`hud::kill_icon_pictures`, the base's where the sheet lacks
//! it), the sound entries its `visuals` name in place of its base's
//! (`maps::weapon_sounds` loads them). What is keyed by the stock
//! weapons (view-model sequences, the third-person stance, the effects,
//! the projectile's model) is its base's: `base_in` (`base` on the item's
//! `tf2:item` hook).
//! Switching from an item weapon (`mods::holster`) applies its holster
//! multiplier and the sword's ×1.75 (§4.9).
//!
//! **Its files on the asset service**: each item weapon has a set,
//! `weapon/<name>` (`weapon/direct_hit`; `sets::item_pieces`, recorded by
//! `chimera_pack sets tf2 --only weapon/ --pack`): what loading it as a
//! match does reads (its `c_` model, skin's materials, held model) with
//! the waves of its own sound entries, less what other sets hold (`base`,
//! `common`, `players`, its classes' `players/<class>`: the arms and
//! their sequences; its base's `weapon/<id>`). A match's offer names it
//! as any gun's (`chimera_net::net::assets::weapon_set`), the sandbox
//! fetches it when the item is picked, so a player with no install sees
//! the item's own model. A map's dressing takes only the items whose
//! models are here (`on_mount_here`: a load never fetches every item's
//! set); one whose set comes later is loaded then (`late_items`).
//!
//! **Batch 2** (specs/tf2/batch2_mechanics.md): each mechanic built once
//! and named by the item's attributes (`effects`: crits and mini-crits,
//! bleed, heals on hit and kill, heads, the dropped kit, regeneration,
//! overheal, speed buffs; `weapons::flare`: the flare guns, whose class
//! has no stock weapon: an abstract base document, data/weapons/
//! flaregun.toml). Part 2 (specs/tf2/batch2b_mechanics.md): the
//! consumables (`consumables`: lunches, drinks, jars on three abstract
//! bases), the meters (`meters`: hype, boost, crikey, heads), the sniper
//! rifles' variants (`weapons::sniper`) and the revolvers' (`effects`).
//! A jar's thrown projectile shows the item's own model
//! (`ItemLooks::thrown`, `docs::thrown_key`).
//!
//! **Batch 3** (specs/tf2/batch3_mechanics.md 1, 2): the Demoman's shields
//! and their charge (`shields`, on the abstract base data/weapons/
//! demoshield.toml; the boots on data/weapons/wearable.toml: worn items,
//! carried, never drawn) and the Soldier's banners (`meters`' rage, on
//! data/weapons/buff_item.toml). An item's data may set hook values its
//! attributes don't give (`ItemDef::hooks`: the Tide Turner's mini-crit
//! cap).
//!
//! **Batch 3, the Spy** (specs/tf2/batch3_mechanics.md 3-5; `spy`): the
//! watches (cloak, the Cloak and Dagger, the Dead Ringer's fake death) on
//! an abstract base (data/weapons/invis.toml), the disguise, and the
//! knives' rules (Your Eternal Reward, Big Earner, Spy-cicle) and the
//! Enforcer's disguise bonus on `tf2:item`, each item's numbers in its
//! data/items.toml `hooks` (no attribute of the spec names them).
//!
//! **Batch 3b** (specs/tf2/batch3b_mechanics.md 2-12): the fire family
//! (`fire`: the crit boost, the Manmelter, the Gas Passer; the
//! Phlogistinator's rage in `meters`, the Backburner's back crits in
//! `weapons::flame`, the Dragon's Fury's fireball in `weapons::shot`,
//! the airblast's costs and heals in `docs` and `weapons::airblast`), the
//! movement kit (`kit`: the B.A.S.E. Jumper's chute, the stomp, the
//! Mantreads' resistances, the Thermal Thruster) and `misc` (the Cow
//! Mangler's charged shot, the Razorback); the Heatmaker's focus is
//! `meters`'. Their classes with no stock weapon are abstract bases
//! (data/weapons/rocketlauncher_fireball, flaregun_revenge, parachute,
//! rocketpack, particle_cannon.toml; the gas jar a class table of
//! jar.toml, the Razorback the worn item's).
//!
//! **Bots** (`bots`, docs/plans/active/rung-passives.md P3): the keyed
//! items a rung may carry (shields, watches, the chute, the thruster)
//! used by any bot through Chimera's `core::BotItems` and `BotFocus`.
//!
//! Known shortcuts:
//! - An item's `anim_slot` and `animation_replacement` (on `tf2:item`)
//!   pick its sequences only for a melee unlock (`view::item_activity`:
//!   one that remaps its draw, idle or swings, names a slot, or is over
//!   the Fists, the Heavy's gloves; the third-person stance takes any
//!   item's slot, `player_anim::Kind::of_slot`); a gun plays its base's
//!   view-model sequences; its `muzzle_flash`,
//!   `tracer_effect` and `particle_effect` are recorded, the base's
//!   effects play; a one-piece view model (`attach_to_hands` 0) is still
//!   bone-merged; of its sounds the shot, crit shot (Minigun), empty and
//!   melee hit/miss entries are used, the reload and deploy entries not.

pub mod bots;
pub mod consumables;
pub mod convert;
pub mod docs;
pub mod effects;
pub mod fire;
pub mod kit;
pub mod meters;
pub mod misc;
pub mod mods;
pub mod shields;
pub mod spy;

use std::collections::BTreeMap;

use serde::Deserialize;

pub use convert::{ItemPack, ItemRecord};

/// Batch 3b's server state in one resource (a resource is an entity of
/// its own, an archetype each: the UGG sessions' archetype bounds): the
/// Manmelter's crit shots, the gas clouds, the crit boost's looks put on,
/// the stomps' falls, the thruster's landings, the Cow Mangler's charged
/// shots and the Razorbacks' breaks.
#[derive(bevy::prelude::Resource, Clone, Debug, Default)]
pub struct ItemState {
    pub crit_shots: fire::CritShots,
    pub gas: fire::GasClouds,
    pub boost_shown: fire::BoostShown,
    pub falls: kit::Falls,
    pub landings: Vec<kit::ThrusterLanded>,
    pub charged: misc::ChargedShots,
    pub razorbacks: misc::Razorbacks,
}

use crate::mount::Mount;

/// The converter's name (docs/formats.md).
pub const CONVERTER: &str = "tf2:item_stats";

/// Its version: bumped by hand whenever any output byte could change.
pub const VERSION: u32 = 4;

/// Where its output lives in TF2's manifest.
pub const GEN_ROOT: &str = "chimera/gen/tf2.items";

/// The pack's file.
pub const PACK_PATH: &str = "chimera/gen/tf2.items/v4/items.toml";

/// The pack of converter version 3, read when the mount has no newer (an
/// older store): the same records without their scripts' `SoundData`
/// (`ItemRecord::script_sounds`: the Huntsman fires silently).
pub const PACK_PATH_V3: &str = "chimera/gen/tf2.items/v3/items.toml";

/// The pack of converter version 2, read when the mount has no newer (an
/// older store): the same records, but a class with no script of its own
/// (the Shotgun items) has no base numbers.
pub const PACK_PATH_V2: &str = "chimera/gen/tf2.items/v2/items.toml";

/// The generated set holding it.
pub const SET: &str = "items";

/// The item schema in the install.
pub const SCHEMA: &str = "scripts/items/items_game.txt";

/// The hook an item weapon's player numbers ride on.
pub const ITEM_HOOK: &str = "tf2:item";

/// data/items.toml (ours).
pub const DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/items.toml");

/// One item of data/items.toml.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub index: u32,
    pub batch: u32,
    #[serde(default)]
    pub gun_game: Option<bool>,
    #[serde(default)]
    pub wearable: bool,
    #[serde(default)]
    pub note: Option<String>,
    /// Hook values no attribute of the item gives (the Tide Turner's
    /// mini-crit cap, measured), merged over its document's `hooks`.
    #[serde(default)]
    pub hooks: Option<toml::Table>,
}

/// data/items.toml.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemsData {
    pub bases: BTreeMap<String, String>,
    pub items: BTreeMap<String, ItemDef>,
}

impl ItemsData {
    pub fn builtin() -> Self {
        toml::from_str(DATA.text()).unwrap_or_else(|e| panic!("tf2 data/items.toml: {e}"))
    }
}

/// The pack's file of converter version 1, read when the mount has no
/// newer (an older store): its records have no `visuals`.
pub const PACK_PATH_V1: &str = "chimera/gen/tf2.items/v1/items.toml";

/// An item weapon made from the pack.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemWeapon {
    /// Our id (`tf2:winger`).
    pub id: String,
    pub index: u32,
    /// Its name (the pack's, "The " left off).
    pub name: String,
    /// The weapon of ours it starts from.
    pub base: String,
    /// The classes that use it.
    pub classes: Vec<String>,
    /// Its document (TOML text).
    pub doc: String,
    /// Its looks, as its document gives them (`docs::item_doc`).
    pub looks: ItemLooks,
}

/// What an item weapon shows and plays (from its document).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemLooks {
    /// The class whose arms hold it (`tf2:gun`'s or `tf2:melee`'s `arms`).
    pub arms: String,
    /// Its view and held models (`[models]`).
    pub view: Option<String>,
    pub world: Option<String>,
    pub view_offset: Option<[f32; 3]>,
    /// Its model's skin (`tf2:item` `skin`).
    pub skin: Option<i32>,
    /// Its kill icon (`[hud] icon`).
    pub icon: Option<String>,
    /// The sound entries its `visuals` name and those of its class
    /// script it plays (script names, no prefix).
    pub sounds: Vec<String>,
    /// The model key its thrown projectile shows its own held model by
    /// (a jar's, `docs::thrown_key`; None: its base's projectile).
    pub thrown: Option<String>,
}

impl ItemLooks {
    /// The looks document `doc` gives, with record `r`'s sounds.
    pub fn of(doc: &toml::Table, r: &ItemRecord) -> Self {
        let s = |v: Option<&toml::Value>| v.and_then(toml::Value::as_str).map(str::to_string);
        let models = doc.get("models");
        let hooks = doc.get("hooks");
        let arms = [crate::game::weapons::GUN_HOOK, crate::game::weapons::MELEE_HOOK]
            .iter()
            .find_map(|h| s(hooks?.get(h)?.get("arms")))
            .unwrap_or_default();
        let offset = models.and_then(|m| m.get("view_offset")).and_then(|o| {
            let a = o.as_array()?;
            let n = |i: usize| a.get(i).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map(|f| f as f32);
            Some([n(0)?, n(1)?, n(2)?])
        });
        let mut sounds: Vec<String> = r.visuals.sounds.values().cloned().collect();
        // Its class script's entries its document plays (`docs::looks`:
        // where its base names none, the Huntsman's shot).
        let played: Vec<&str> = doc.get("sounds").and_then(toml::Value::as_table).map_or(Vec::new(), |t| t.values().filter_map(toml::Value::as_str).collect());
        sounds.extend(r.script_sounds.values().filter(|n| played.contains(&crate::game::maps::own_sound(n).as_str())).cloned());
        sounds.sort();
        sounds.dedup();
        Self {
            arms,
            view: s(models.and_then(|m| m.get("view"))),
            world: s(models.and_then(|m| m.get("world"))),
            view_offset: offset,
            skin: hooks.and_then(|h| h.get(ITEM_HOOK)?.get("skin")?.as_integer()).and_then(|i| i32::try_from(i).ok()),
            icon: s(doc.get("hud").and_then(|h| h.get("icon"))),
            sounds,
            thrown: s(hooks.and_then(|h| h.get(crate::game::weapons::PROJECTILE_HOOK)?.get("shown"))).filter(|k| k.ends_with("/thrown")),
        }
    }
}

/// The stock weapon an item weapon's data names as its base (its
/// `tf2:item` hook's `base`; module doc: its sequences, stance and
/// effects are that weapon's), None for a stock weapon.
pub fn base_in(data: &chimera_sim::weapon::data::WeaponData) -> Option<&str> {
    data.hooks.get(ITEM_HOOK)?.get("base")?.as_str().filter(|b| !b.is_empty())
}

/// The item weapons of the pack on `mount` (`weapons` over the committed
/// documents: their looks don't depend on the scripts). Reads the pack
/// only when it is here (`Mount::held`): never fetches.
pub fn on_mount(mount: &Mount) -> Vec<ItemWeapon> {
    let Some(bytes) = [PACK_PATH, PACK_PATH_V3, PACK_PATH_V2, PACK_PATH_V1].iter().filter(|p| mount.held(p)).find_map(|p| mount.read(p).ok()) else {
        return Vec::new();
    };
    let Ok(pack) = ItemPack::parse(&String::from_utf8_lossy(&bytes)) else { return Vec::new() };
    let docs = crate::game::weapons::script_numbers::GunNumbers::builtin().docs;
    weapons(&pack, &ItemsData::builtin(), &docs).0
}

/// The item weapons of `on_mount` whose own models are here
/// (`ItemWeapon::models_here`): what a map's dressing loads, so a load
/// never fetches an item's set; one whose set comes later is loaded
/// then (`late_items`).
pub fn on_mount_here(mount: &Mount) -> Vec<ItemWeapon> {
    on_mount(mount).into_iter().filter(|i| i.models_here(mount)).collect()
}

impl ItemWeapon {
    /// Whether its own view and held models are on `mount` without a
    /// fetch (`Mount::held`; one it hasn't is its base's, `base`'s files).
    pub fn models_here(&self, mount: &Mount) -> bool {
        [&self.looks.view, &self.looks.world].iter().all(|m| m.as_deref().is_none_or(|p| mount.held(p)))
    }
}

/// The pack's display name for `r`.
pub fn display_name(r: &ItemRecord) -> String {
    let n = r.name.trim();
    n.strip_prefix("The ").unwrap_or(n).to_string()
}

/// The item weapons of `pack` that data/items.toml names, built over
/// `docs` (the stock documents, file name and text) in data order. Items
/// the pack lacks, wearables and items with no base are left out (said
/// in the second list).
pub fn weapons(pack: &ItemPack, data: &ItemsData, docs: &[(String, String)]) -> (Vec<ItemWeapon>, Vec<String>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    let by_id: BTreeMap<String, toml::Table> = docs
        .iter()
        .filter_map(|(_, t)| {
            let t = t.parse::<toml::Table>().ok()?;
            Some((t.get("id")?.as_str()?.to_string(), t))
        })
        .collect();
    let mut defs: Vec<(&String, &ItemDef)> = data.items.iter().collect();
    defs.sort_by_key(|(_, d)| d.index);
    for (name, def) in defs {
        if def.wearable {
            continue;
        }
        let Some(r) = pack.get(def.index) else {
            notes.push(format!("{name} ({}): not in the pack", def.index));
            continue;
        };
        let Some(base) = data.bases.get(&r.class) else {
            notes.push(format!("{name} ({}): no base for {}", def.index, r.class));
            continue;
        };
        let Some(text) = flat(base, &by_id, 0) else {
            notes.push(format!("{name}: no document {base}"));
            continue;
        };
        let id = format!("tf2:{name}");
        let shown = display_name(r);
        let mut doc = docs::item_doc(text, &id, &shown, base, r, def.gun_game);
        if let Some(extra) = &def.hooks
            && let Some(toml::Value::Table(hooks)) = doc.get_mut("hooks")
        {
            for (name, v) in extra {
                match (hooks.get_mut(name), v) {
                    (Some(toml::Value::Table(t)), toml::Value::Table(x)) => t.extend(x.clone()),
                    _ => {
                        hooks.insert(name.clone(), v.clone());
                    }
                }
            }
        }
        out.push(ItemWeapon {
            looks: ItemLooks::of(&doc, r),
            id,
            index: def.index,
            name: shown,
            base: base.clone(),
            classes: r.classes.clone(),
            doc: toml::to_string(&doc).expect("a document serializes"),
        });
    }
    (out, notes)
}

/// Document `id` with its bases merged in (as the weapon data merges
/// them: tables key by key, `class` and `abstract` not inherited), its
/// `base` gone: an item's document stands alone.
/// `b` merged over `a`: tables key by key, other values replaced.
fn merge(a: &mut toml::Table, b: &toml::Table) {
    for (k, v) in b {
        match (a.get_mut(k), v) {
            (Some(toml::Value::Table(x)), toml::Value::Table(y)) => merge(x, y),
            _ => {
                a.insert(k.clone(), v.clone());
            }
        }
    }
}

fn flat(id: &str, docs: &BTreeMap<String, toml::Table>, depth: usize) -> Option<toml::Table> {
    let own = docs.get(id)?;
    let mut t = match own.get("base").and_then(toml::Value::as_str) {
        Some(b) if depth < 8 => {
            let mut t = flat(b, docs, depth + 1)?;
            t.remove("abstract");
            t.remove("class");
            t
        }
        _ => toml::Table::new(),
    };
    merge(&mut t, own);
    t.remove("base");
    Some(t)
}

/// The pack on `mount`, if it has one (a malformed one said); an older
/// store's version-1 pack when it has no newer.
pub fn read_pack(mount: &Mount) -> Option<ItemPack> {
    let bytes = [PACK_PATH, PACK_PATH_V3, PACK_PATH_V2, PACK_PATH_V1].iter().find_map(|p| mount.read(p).ok())?;
    let text = String::from_utf8_lossy(&bytes);
    ItemPack::parse(&text).map_err(|e| bevy::log::warn!("tf2: {PACK_PATH}: {e}")).ok()
}

/// The generated sets for `chimera_pack sets tf2 --only items`: the pack
/// converted from `mount` (the install, `.ctx` scripts decrypted with the
/// host's key), when `only` asks for it.
pub fn generated(mount: &Mount, only: &str) -> Vec<(String, Vec<(String, Vec<u8>)>)> {
    if !(SET.starts_with(only) || only.starts_with(SET)) {
        return Vec::new();
    }
    let schema = match mount.read(SCHEMA) {
        Ok(b) => b,
        Err(e) => {
            println!("{SET}: {SCHEMA}: {e}");
            return Vec::new();
        }
    };
    let text = String::from_utf8_lossy(&schema);
    // A class with no script of its own (the Shotgun's `tf_weapon_shotgun`:
    // the install has the `_primary`, `_soldier`, `_hwg` and `_pyro` ones)
    // reads its base weapon's (data/items.toml `[bases]`).
    let bases = ItemsData::builtin().bases;
    let script = |class: &str| {
        let read = |name: &str| {
            let s = super::scripts::read(mount, name, None);
            (s.source != super::scripts::ScriptSource::Missing).then_some(s.root)
        };
        read(class).or_else(|| bases.get(class).and_then(|b| read(b.strip_prefix("tf2:").unwrap_or(b))))
    };
    let c = convert::convert(&text, &script);
    for n in &c.notes {
        println!("{SET}: {n}");
    }
    println!("{SET}: {} weapon items, {} item class scripts read", c.pack.items.len(), c.pack.scripts.len());
    vec![(SET.to_string(), vec![(PACK_PATH.to_string(), c.pack.to_text().into_bytes())])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_reads_and_names_batch_one() {
        let d = ItemsData::builtin();
        assert_eq!(d.items.values().filter(|i| i.batch == 1).count(), 33);
        assert_eq!(d.items.values().filter(|i| i.batch == 2).count(), 54);
        let mut idx: Vec<u32> = d.items.values().map(|i| i.index).collect();
        idx.sort();
        idx.dedup();
        assert_eq!(idx.len(), d.items.len(), "an index once");
        let stock: Vec<String> =
            crate::game::weapons::tf2_weapons().weapons.into_iter().map(|w| w.id).collect();
        for b in d.bases.values() {
            // Classes with no stock weapon: abstract bases.
            assert!(stock.contains(b) || crate::game::weapons::ABSTRACT_BASES.contains(&b.as_str()), "{b}");
        }
        assert!(PACK_PATH.starts_with(&format!("{GEN_ROOT}/v{VERSION}/")));
    }

    /// Made-up items (written here, not the game's) with looks: a Minigun
    /// with its own shot and crit shot, a club with its own swings, a
    /// rocket launcher with its own model, icon and skin, one without looks.
    const LOOKS: &str = r#"
"items_game" { "items" {
    "312" { "name" "The Made Beast" "item_class" "tf_weapon_minigun" "item_slot" "primary" "used_by_classes" { "heavy" "1" }
            "model_player" "models/made/c_beast.mdl"
            "visuals" { "sound_single_shot" "Made_Beast.Fire" "sound_burst" "Made_Beast.FireCrit" "sound_deploy" "Made_Beast.Draw" } }
    "172" { "name" "Made Cutter" "item_class" "tf_weapon_sword" "item_slot" "melee" "used_by_classes" { "demoman" "1" }
            "model_player_per_class" { "demoman" "models/made/c_cutter.mdl" }
            "visuals" { "sound_melee_hit" "Made_Sword.HitFlesh" "sound_melee_miss" "Made_Sword.Swing" "sound_melee_hit_world" "Made_Sword.HitWorld" } }
    "127" { "name" "The Made Hit" "item_class" "tf_weapon_rocketlauncher_directhit" "item_slot" "primary" "used_by_classes" { "soldier" "1" }
            "model_player" "models/made/c_hit.mdl" "item_iconname" "made_hit" "visuals" { "skin" "2" "sound_single_shot" "Made_Hit.Single" } }
    "414" { "name" "Plain Launcher" "item_class" "tf_weapon_rocketlauncher" "item_slot" "primary" "used_by_classes" { "soldier" "1" } }
} }
"#;

    /// Report d1d9bdae: the Huntsman's class has no stock weapon of ours,
    /// so its abstract base names no shot; the converter keeps the class
    /// script's `SoundData` (made-up entries here) and the item's
    /// document plays its `single_shot` as its shot, loaded with the map.
    #[test]
    fn a_class_without_a_stock_weapon_shoots_with_its_scripts_sound() {
        const BOW: &str = r#"
"items_game" { "items" {
    "56" { "name" "The Made Bow" "item_class" "tf_weapon_compound_bow" "item_slot" "primary" "used_by_classes" { "sniper" "1" } "anim_slot" "item2" }
} }
"#;
        let script = |class: &str| {
            (class == "tf_weapon_compound_bow").then(|| {
                source_formats::keyvalues::parse_pc(r#"WeaponData { "Damage" "50" "SoundData" { "single_shot" "Made_Bow.Fire" "special1" "Made_Bow.Pull" } }"#)
            })
        };
        let pack = convert::convert(BOW, &script).pack;
        let r = pack.get(56).expect("the bow");
        assert_eq!(r.script_sounds.get("single_shot").map(String::as_str), Some("Made_Bow.Fire"));
        assert_eq!(r.visuals.anim_slot.as_deref(), Some("item2"));
        let stock = crate::game::weapons::script_numbers::GunNumbers::builtin();
        let (items, _) = weapons(&pack, &ItemsData::builtin(), &stock.docs);
        let set = stock.clone().with_pack(&pack).parse().expect("the items parse");
        let bow = set.get("tf2:huntsman").expect("the Huntsman");
        assert_eq!(bow.sounds.as_ref().and_then(|s| s.fire.as_deref()), Some("tf2:Made_Bow.Fire"));
        let looks = &items.iter().find(|i| i.id == "tf2:huntsman").expect("its item").looks;
        assert_eq!(looks.sounds, ["Made_Bow.Fire"], "loaded with the map");
    }

    #[test]
    fn items_play_their_own_sounds_and_show_their_own_looks_else_the_base_s() {
        let pack = convert::convert(LOOKS, &|_| None).pack;
        let stock = crate::game::weapons::script_numbers::GunNumbers::builtin();
        let (items, _) = weapons(&pack, &ItemsData::builtin(), &stock.docs);
        let numbers = stock.clone().with_pack(&pack);
        let set = numbers.parse().expect("the items parse");
        let get = |id: &str| set.get(id).unwrap_or_else(|| panic!("{id}"));
        let item = |id: &str| items.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("{id}"));
        let hook = |id: &str, h: &str, k: &str| get(id).hooks[h][k].as_str().unwrap().to_string();

        // The Minigun: its shot and crit shot; its deploy entry unused; the
        // rest its base's.
        let beast = "tf2:brass_beast";
        assert_eq!(hook(beast, "tf2:minigun", "fire_sound"), "tf2:Made_Beast.Fire");
        assert_eq!(hook(beast, "tf2:minigun", "fire_crit_sound"), "tf2:Made_Beast.FireCrit");
        assert_eq!(hook(beast, "tf2:minigun", "spin_sound"), hook(crate::game::weapons::MINIGUN, "tf2:minigun", "spin_sound"));
        assert_eq!(get(beast).models.view.as_deref(), Some("models/made/c_beast.mdl"));
        assert_eq!(get(beast).models.world.as_deref(), Some("models/made/c_beast.mdl"));
        // The sword: every swing's hit, world hit and miss; its class's model.
        let cutter = get("tf2:scotsmans_skullcutter");
        let m = cutter.melee.as_ref().and_then(|m| m.primary.as_ref()).expect("a melee weapon");
        assert_eq!(m.sound_hit.as_deref(), Some("tf2:Made_Sword.HitFlesh"));
        assert_eq!(m.sound_hit_world.as_deref(), Some("tf2:Made_Sword.HitWorld"));
        assert_eq!(m.sound_swing.as_deref(), Some("tf2:Made_Sword.Swing"));
        assert_eq!(cutter.models.view.as_deref(), Some("models/made/c_cutter.mdl"), "model_player_per_class");
        // The launcher: its shot, icon and skin; the plain one its base's.
        let hit = get("tf2:direct_hit");
        assert_eq!(hit.sounds.as_ref().and_then(|s| s.fire.as_deref()), Some("tf2:Made_Hit.Single"));
        assert_eq!(hit.hud.icon.as_deref(), Some("made_hit"));
        assert_eq!(item("tf2:direct_hit").looks.skin, Some(2));
        assert_eq!(item("tf2:direct_hit").looks.arms, "soldier");
        let plain = get("tf2:liberty_launcher");
        let base = get(crate::game::weapons::ROCKET_LAUNCHER);
        assert_eq!(plain.sounds.as_ref().and_then(|s| s.fire.clone()), base.sounds.as_ref().and_then(|s| s.fire.clone()));
        assert_eq!(plain.hud.icon, base.hud.icon);
        assert_eq!(plain.models.view, base.models.view, "no model of its own: its base's");
        // The sounds the maps load: the visuals' entries.
        assert_eq!(item(beast).looks.sounds, ["Made_Beast.Draw", "Made_Beast.Fire", "Made_Beast.FireCrit"]);
        // Sequences, stance and effects: the base's.
        assert_eq!(base_in(hit), Some(crate::game::weapons::ROCKET_LAUNCHER));
        assert_eq!(base_in(get(crate::game::weapons::MINIGUN)), None);
        assert_eq!(crate::game::weapons::looks::GunLooks::from_set(&set).of("tf2:direct_hit").map(|l| l.explosion.is_some()), Some(true));
    }
}
