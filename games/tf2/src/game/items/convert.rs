//! The item converter `tf2:item_stats` (specs/tf2/item_attributes.md 1,
//! 2): TF2's item schema (`scripts/items/items_game.txt`) and its item
//! classes' weapon scripts in, one record per weapon item out
//! (`ItemRecord`, written as `items.toml` into the TF2 pack by
//! `chimera_pack sets tf2 --only items`; never committed, never read
//! from the install at run time).
//!
//! What it does, per item of the schema's `items` block whose item class
//! is a weapon's (`tf_weapon_*`) or whose slot is a weapon slot
//! (`WEAPON_SLOTS`: the Sniper's wearables are secondaries):
//!
//! 1. **Prefabs**: the schema's `default` item, then each prefab the
//!    item's `prefab` names (space-separated, each with its own chain,
//!    left to right), then the item itself, merged key by key (blocks
//!    merged, values replaced): its `item_class`, `item_slot`, `name`,
//!    `model_player`, `model_world`, `used_by_classes`, `attributes` and
//!    `static_attrs`.
//! 2. **Attributes**: each of `attributes` (`"<name>" { "attribute_class"
//!    ... "value" ... }`, or a bare value) and of `static_attrs`
//!    (`"<name>" "<value>"`), looked up by name in the schema's
//!    `attributes` definitions for its class and `description_format`; a
//!    name the definitions lack keeps the class the item gives, or is
//!    listed in `unknown`. `IGNORED` names (counters, looks) are left out.
//! 3. **Folding** (§1): every attribute of one class folds into one
//!    number by its format's operation (`Op`): percentages multiply
//!    (identity 1), additive values add (0), flags OR (as integers),
//!    lookup values replace. Order never matters for these.
//! 4. **Base and effective numbers**: the item class's script (through
//!    `script`, the host's decrypting mount with the key; never printed)
//!    gives `Damage`, `TimeFireDelay`, `clip_size`, `BulletsPerShot` and
//!    `Spread` (`base`); `effective` is those and the spec's constants
//!    (`constants`) with the folded attributes applied, integers rounded
//!    once at the end (§1: clip 12 × 0.4 → 5). The game derives its own
//!    numbers from `attrs` over its weapon data (`super::docs`);
//!    `effective` is for reading and for the converted-install test.
//!
//! The output is deterministic (sorted maps, no time, no paths): the same
//! inputs give the same bytes (`super::VERSION` names the rules).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use source_formats::keyvalues::{self, Kv};

/// The item slots whose items are weapons (or a weapon slot's wearable).
pub const WEAPON_SLOTS: &[&str] = &["primary", "secondary", "melee", "pda", "pda2", "building"];

/// Attributes with no gameplay effect for us (§2.3), by name: left out.
pub const IGNORED: &[&str] = &[
    "kill eater score type",
    "kill eater score type 2",
    "kill eater kill type",
    "special taunt",
    "disable fancy class select anim",
    "vision opt in flags",
    "pyrovision only DISPLAY ONLY",
    "pyrovision opt in DISPLAY ONLY",
    "custom projectile model",
    "allowed in medieval mode",
    "limited quantity item",
    "min_viewmodel_offset",
    "weapon_allow_inspect",
    "weapon_uses_stattrak_module",
    "weapon_stattrak_module_scale",
];

/// How an attribute's value combines (§1, by `description_format`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Multiply,
    Add,
    Or,
    Set,
}

impl Op {
    /// The operation a `description_format` names; an unknown or absent
    /// one adds (the start is 0: a flag's answer).
    pub fn of(format: Option<&str>) -> Op {
        match format.map(str::trim) {
            Some("value_is_percentage" | "value_is_inverted_percentage") => Op::Multiply,
            Some("value_is_or") => Op::Or,
            Some("value_is_from_lookup_table") => Op::Set,
            _ => Op::Add,
        }
    }

    fn identity(self) -> f64 {
        match self {
            Op::Multiply => 1.0,
            _ => 0.0,
        }
    }

    fn fold(self, acc: f64, v: f64) -> f64 {
        match self {
            Op::Multiply => acc * v,
            Op::Add => acc + v,
            Op::Or => ((acc as i64) | (v as i64)) as f64,
            Op::Set => v,
        }
    }
}

/// One weapon item, converted.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemRecord {
    /// Its definition index (the schema's key).
    pub index: u32,
    /// Its schema name (`name`).
    #[serde(default)]
    pub name: String,
    /// Its item class (`tf_weapon_handgun_scout_secondary`, `tf_wearable`).
    pub class: String,
    #[serde(default)]
    pub slot: Option<String>,
    /// The classes that use it (`used_by_classes`), lower case, sorted.
    #[serde(default)]
    pub classes: Vec<String>,
    /// Its models (`model_player`, `model_world`).
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub world_model: Option<String>,
    /// Attribute class → folded value (identities left out).
    #[serde(default)]
    pub attrs: BTreeMap<String, f64>,
    /// Attribute names with no definition and no class of their own.
    #[serde(default)]
    pub unknown: Vec<String>,
    /// The item class's script numbers (`damage`, `interval`, `clip`,
    /// `bullets`, `spread`), when it was read.
    #[serde(default)]
    pub base: BTreeMap<String, f64>,
    /// The numbers with the attributes applied (module doc, step 4).
    #[serde(default)]
    pub effective: BTreeMap<String, f64>,
}

impl ItemRecord {
    /// A folded attribute class's value, or `default` (its identity).
    pub fn attr(&self, class: &str, default: f64) -> f64 {
        self.attrs.get(class).copied().unwrap_or(default)
    }

    /// Whether flag class `class` is set (non-zero).
    pub fn flag(&self, class: &str) -> bool {
        self.attrs.get(class).is_some_and(|v| *v != 0.0)
    }
}

/// The converter's output: every weapon item, by index.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemPack {
    pub converter: String,
    pub version: u32,
    /// The schema's fingerprint (`fingerprint`: FNV-1a 64 of its bytes).
    pub input: String,
    /// Item classes whose script was read (sorted).
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default, rename = "item")]
    pub items: Vec<ItemRecord>,
}

impl ItemPack {
    pub fn get(&self, index: u32) -> Option<&ItemRecord> {
        self.items.iter().find(|i| i.index == index)
    }

    /// The pack as its file's text.
    pub fn to_text(&self) -> String {
        toml::to_string(self).expect("an item pack serializes")
    }

    pub fn parse(text: &str) -> Result<ItemPack, String> {
        toml::from_str(text).map_err(|e| format!("items pack: {e}"))
    }
}

/// FNV-1a 64 of `bytes`, hex: the schema's fingerprint in the pack.
pub fn fingerprint(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// `b` merged into `a`: blocks merged key by key (any case), values
/// replaced, new keys appended.
fn merge(a: &mut Vec<(String, Kv)>, b: &[(String, Kv)]) {
    for (k, v) in b {
        match a.iter_mut().find(|(x, _)| x.eq_ignore_ascii_case(k)) {
            Some((_, Kv::Block(old))) if matches!(v, Kv::Block(_)) => merge(old, v.items()),
            Some((_, old)) => *old = v.clone(),
            None => a.push((k.clone(), v.clone())),
        }
    }
}

/// `prefab`'s merged entries (its own chain first), `seen` guarding loops.
fn prefab_entries(prefabs: &Kv, name: &str, seen: &mut Vec<String>) -> Vec<(String, Kv)> {
    if seen.iter().any(|s| s.eq_ignore_ascii_case(name)) {
        return Vec::new();
    }
    seen.push(name.to_string());
    let Some(p) = prefabs.get(name) else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(chain) = p.str("prefab") {
        for n in chain.split_whitespace() {
            let inner = prefab_entries(prefabs, n, seen);
            merge(&mut out, &inner);
        }
    }
    merge(&mut out, p.items());
    out
}

/// An item's entries with its prefabs and the `default` item under it.
fn resolved(items: &Kv, prefabs: &Kv, item: &Kv) -> Vec<(String, Kv)> {
    let mut out = Vec::new();
    if let Some(d) = items.get("default") {
        merge(&mut out, d.items());
    }
    if let Some(chain) = item.str("prefab") {
        for n in chain.split_whitespace() {
            let p = prefab_entries(prefabs, n, &mut Vec::new());
            merge(&mut out, &p);
        }
    }
    merge(&mut out, item.items());
    out
}

/// An attribute definition: its class and operation.
#[derive(Clone, Debug)]
struct Def {
    class: String,
    op: Op,
}

/// The schema's attribute definitions by lower-case name.
fn definitions(root: &Kv) -> BTreeMap<String, Def> {
    let mut out = BTreeMap::new();
    for (_, a) in root.get("attributes").map(Kv::items).unwrap_or_default() {
        let (Some(name), Some(class)) = (a.str("name"), a.str("attribute_class")) else { continue };
        out.insert(name.trim().to_ascii_lowercase(), Def { class: class.trim().to_string(), op: Op::of(a.str("description_format")) });
    }
    out
}

/// The schema's root block (`items_game`, or the text's top level).
fn schema_root(kv: &Kv) -> &Kv {
    kv.get("items_game").unwrap_or(kv)
}

/// The numbers of an item class's script (its first block): `damage`,
/// `interval`, `clip`, `bullets`, `spread`.
pub fn script_base(script: &Kv) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    let Some(data) = script.items().iter().find(|(_, v)| matches!(v, Kv::Block(_))).map(|(_, v)| v) else { return out };
    for (key, ours) in [("Damage", "damage"), ("TimeFireDelay", "interval"), ("clip_size", "clip"), ("BulletsPerShot", "bullets"), ("Spread", "spread")] {
        if let Some(x) = data.str(key).and_then(|t| t.trim().parse::<f64>().ok()).filter(|x| x.is_finite()) {
            out.insert(ours.to_string(), x);
        }
    }
    out
}

/// The spec's constants the effective numbers start from (item_attributes.md
/// "Constants"), by number name.
pub mod constants {
    pub const ROCKET_SPEED: f64 = 1100.0;
    pub const BLAST_RADIUS: f64 = 146.0;
    pub const GRENADE_SPEED: f64 = 1216.0;
    pub const GRENADE_FUSE: f64 = 2.0;
    pub const STICKY_ARM: f64 = 0.8;
    pub const STICKY_CHARGE: f64 = 4.0;
    pub const STICKY_MAX: f64 = 8.0;
    pub const SPINUP: f64 = 0.75;
    pub const SPUN_SPEED: f64 = 110.0;
    pub const JUMP: f64 = 289.0;
    pub const MELEE_RANGE: f64 = 48.0;
    pub const SWORD_RANGE: f64 = 72.0;
    pub const SWITCH: f64 = 0.5;
    pub const SWORD_SWITCH: f64 = 1.75;
}

/// The effective numbers of `r` (module doc, step 4).
pub fn effective(r: &ItemRecord) -> BTreeMap<String, f64> {
    use constants::*;
    let mut out = BTreeMap::new();
    let class = r.class.as_str();
    let a = |c: &str| r.attr(c, 1.0);
    let add = |c: &str| r.attr(c, 0.0);
    if let Some(d) = r.base.get("damage") {
        out.insert("damage".into(), d * a("mult_dmg"));
    }
    if let Some(i) = r.base.get("interval") {
        out.insert("interval".into(), i * a("mult_postfiredelay"));
    }
    if let Some(c) = r.base.get("clip").filter(|c| **c >= 0.0) {
        out.insert("clip".into(), (c * a("mult_clipsize")).round());
    }
    if let Some(s) = r.base.get("spread") {
        out.insert("spread".into(), s * a("mult_spread_scale"));
    }
    if class.starts_with("tf_weapon_rocketlauncher") {
        out.insert("projectile_speed".into(), ROCKET_SPEED * a("mult_projectile_speed"));
        out.insert("blast_radius".into(), BLAST_RADIUS * a("mult_explosion_radius"));
    }
    if class == "tf_weapon_grenadelauncher" {
        out.insert("projectile_speed".into(), GRENADE_SPEED * a("mult_projectile_speed"));
        out.insert("blast_radius".into(), BLAST_RADIUS * a("mult_explosion_radius"));
        out.insert("fuse".into(), GRENADE_FUSE * a("fuse_mult"));
    }
    if class == "tf_weapon_pipebomblauncher" {
        out.insert("blast_radius".into(), BLAST_RADIUS * a("mult_explosion_radius"));
        out.insert("arm".into(), STICKY_ARM + add("sticky_arm_time"));
        out.insert("charge".into(), STICKY_CHARGE * a("stickybomb_charge_rate"));
        out.insert("max_stickies".into(), (STICKY_MAX + add("add_max_pipebombs")).round());
    }
    if class == "tf_weapon_minigun" {
        out.insert("spinup".into(), SPINUP * a("mult_minigun_spinup_time"));
        out.insert("spun_speed".into(), SPUN_SPEED * a("mult_player_aiming_movespeed"));
    }
    if r.attrs.contains_key("mod_jump_height_from_weapon") {
        out.insert("jump".into(), JUMP * a("mod_jump_height_from_weapon"));
    }
    if r.flag("is_a_sword") {
        out.insert("melee_range".into(), SWORD_RANGE);
        out.insert("switch".into(), SWITCH * SWORD_SWITCH);
    }
    out
}

/// What the converter read and wrote: the pack, and notes worth a line.
#[derive(Clone, Debug, Default)]
pub struct Converted {
    pub pack: ItemPack,
    pub notes: Vec<String>,
}

/// Convert `schema` (items_game's text) with each item class's script
/// from `script` (None: not read; the record has no `base`).
pub fn convert(schema: &str, script: &dyn Fn(&str) -> Option<Kv>) -> Converted {
    let kv = keyvalues::parse_pc(schema);
    let root = schema_root(&kv);
    let empty = Kv::Block(Vec::new());
    let items = root.get("items").unwrap_or(&empty);
    let prefabs = root.get("prefabs").unwrap_or(&empty);
    let defs = definitions(root);
    let mut notes = Vec::new();
    let mut scripts: BTreeMap<String, Option<BTreeMap<String, f64>>> = BTreeMap::new();
    let mut out = Vec::new();
    for (key, item) in items.items() {
        let Ok(index) = key.trim().parse::<u32>() else { continue };
        let e = Kv::Block(resolved(items, prefabs, item));
        let Some(class) = e.str("item_class").map(|c| c.trim().to_string()) else { continue };
        let slot = e.str("item_slot").map(|s| s.trim().to_ascii_lowercase());
        let weapon = class.starts_with("tf_weapon_") || slot.as_deref().is_some_and(|s| WEAPON_SLOTS.contains(&s));
        if !weapon {
            continue;
        }
        let mut classes: Vec<String> = e
            .get("used_by_classes")
            .map(Kv::items)
            .unwrap_or_default()
            .iter()
            .filter(|(_, v)| v.str_value().is_none_or(|t| t.trim() != "0"))
            .map(|(k, _)| k.trim().to_ascii_lowercase())
            .collect();
        classes.sort();
        classes.dedup();
        // Attribute (name, class given, value).
        let mut listed: Vec<(String, Option<String>, f64)> = Vec::new();
        for (name, v) in e.get("attributes").map(Kv::items).unwrap_or_default() {
            let (given, value) = match v {
                Kv::Block(_) => (v.str("attribute_class").map(|c| c.trim().to_string()), v.str("value")),
                Kv::Value(t) => (None, Some(t.as_str())),
            };
            if let Some(x) = value.and_then(|t| t.trim().parse::<f64>().ok()) {
                listed.push((name.clone(), given, x));
            }
        }
        for (name, v) in e.get("static_attrs").map(Kv::items).unwrap_or_default() {
            if let Some(x) = v.str_value().and_then(|t| t.trim().parse::<f64>().ok()) {
                listed.push((name.clone(), None, x));
            }
        }
        let mut folded: BTreeMap<String, (Op, f64)> = BTreeMap::new();
        let mut unknown = Vec::new();
        for (name, given, x) in listed {
            if IGNORED.iter().any(|i| i.eq_ignore_ascii_case(name.trim())) {
                continue;
            }
            let def = defs.get(&name.trim().to_ascii_lowercase());
            let (class, op) = match (def, given) {
                (Some(d), _) => (d.class.clone(), d.op),
                (None, Some(c)) => (c, Op::Add),
                (None, None) => {
                    unknown.push(name.trim().to_string());
                    continue;
                }
            };
            let slot = folded.entry(class).or_insert((op, op.identity()));
            slot.1 = slot.0.fold(slot.1, x);
        }
        let attrs: BTreeMap<String, f64> = folded.into_iter().filter(|(_, (op, v))| *v != op.identity()).map(|(c, (_, v))| (c, v)).collect();
        unknown.sort();
        unknown.dedup();
        let base = scripts
            .entry(class.clone())
            .or_insert_with(|| {
                let read = script(&class).map(|s| script_base(&s));
                if read.is_none() && class.starts_with("tf_weapon_") {
                    notes.push(format!("{class}: no script read (no key, or not in the install)"));
                }
                read
            })
            .clone()
            .unwrap_or_default();
        let mut r = ItemRecord {
            index,
            name: e.str("name").map(|n| n.trim().to_string()).unwrap_or_default(),
            class,
            slot,
            classes,
            model: e.str("model_player").map(|m| m.trim().to_string()).filter(|m| !m.is_empty()),
            world_model: e.str("model_world").map(|m| m.trim().to_string()).filter(|m| !m.is_empty()),
            attrs,
            unknown,
            base,
            effective: BTreeMap::new(),
        };
        r.effective = effective(&r);
        out.push(r);
    }
    out.sort_by_key(|r| r.index);
    out.dedup_by_key(|r| r.index);
    let read: Vec<String> = scripts.iter().filter(|(_, s)| s.is_some()).map(|(c, _)| c.clone()).collect();
    Converted {
        pack: ItemPack { converter: super::CONVERTER.into(), version: super::VERSION, input: fingerprint(schema.as_bytes()), scripts: read, items: out },
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up schema in items_game's shape (written here, not copied
    /// from the game): a default item, a prefab chain, attribute
    /// definitions of each format, one item per rule.
    const SCHEMA: &str = r#"
"items_game"
{
    "prefabs"
    {
        "weapon_base" { "item_quality" "unique" "attributes" { "made up flag" { "attribute_class" "made_up_flag" "value" "1" } } }
        "pistol_kind" { "prefab" "weapon_base" "item_class" "tf_weapon_handgun_scout_secondary" "item_slot" "secondary" }
        "looks" { "model_player" "models/made/up.mdl" }
    }
    "items"
    {
        "default" { "item_slot" "misc" }
        "10"
        {
            "name" "Test Pistol"
            "prefab" "pistol_kind looks"
            "used_by_classes" { "scout" "1" "engineer" "1" }
            "attributes"
            {
                "clip size penalty" { "attribute_class" "mult_clipsize" "value" "0.4" }
                "damage bonus" { "attribute_class" "mult_dmg" "value" "1.15" }
                "damage penalty" { "attribute_class" "mult_dmg" "value" "0.5" }
                "kill eater score type" { "attribute_class" "kill_eater_score_type" "value" "3" }
            }
            "static_attrs" { "sticky arm time bonus" "-0.2" "sticky arm time penalty" "0.05" "flags a" "1" "flags b" "4" }
        }
        "11" { "name" "A Hat" "item_class" "tf_wearable" "item_slot" "head" }
        "12" { "name" "A Shield" "item_class" "tf_wearable" "item_slot" "secondary" "static_attrs" { "no such attribute" "2" } }
        "13" { "name" "Plain Club" "prefab" "weapon_base" "item_class" "tf_weapon_club" "item_slot" "melee" }
    }
    "attributes"
    {
        "1" { "name" "damage penalty" "attribute_class" "mult_dmg" "description_format" "value_is_percentage" }
        "2" { "name" "damage bonus" "attribute_class" "mult_dmg" "description_format" "value_is_percentage" }
        "3" { "name" "clip size penalty" "attribute_class" "mult_clipsize" "description_format" "value_is_inverted_percentage" }
        "126" { "name" "sticky arm time bonus" "attribute_class" "sticky_arm_time" "description_format" "value_is_additive" }
        "127" { "name" "sticky arm time penalty" "attribute_class" "sticky_arm_time" "description_format" "value_is_additive" }
        "200" { "name" "flags a" "attribute_class" "made_up_bits" "description_format" "value_is_or" }
        "201" { "name" "flags b" "attribute_class" "made_up_bits" "description_format" "value_is_or" }
    }
}
"#;

    fn script(class: &str) -> Option<Kv> {
        (class == "tf_weapon_handgun_scout_secondary").then(|| {
            keyvalues::parse_pc(r#"WeaponData { "Damage" "15" "TimeFireDelay" "0.15" "clip_size" "12" "Spread" "0.04" }"#)
        })
    }

    #[test]
    fn items_resolve_prefabs_and_fold_attributes_by_format() {
        let c = convert(SCHEMA, &script);
        let p = &c.pack;
        assert_eq!((p.converter.as_str(), p.version), (super::super::CONVERTER, super::super::VERSION));
        // The hat is no weapon; the shield is (a weapon slot's wearable).
        assert_eq!(p.items.iter().map(|i| i.index).collect::<Vec<_>>(), [10, 12, 13]);
        let r = p.get(10).unwrap();
        assert_eq!(r.class, "tf_weapon_handgun_scout_secondary", "from the prefab chain");
        assert_eq!(r.slot.as_deref(), Some("secondary"), "the prefab's over the default item's");
        assert_eq!(r.classes, ["engineer", "scout"]);
        assert_eq!(r.model.as_deref(), Some("models/made/up.mdl"), "the second prefab's");
        // Percentages multiply, additive values add, flags OR.
        assert!((r.attr("mult_dmg", 1.0) - 0.575).abs() < 1e-12);
        assert_eq!(r.attr("mult_clipsize", 1.0), 0.4);
        assert!((r.attr("sticky_arm_time", 0.0) + 0.15).abs() < 1e-12);
        assert_eq!(r.attr("made_up_bits", 0.0), 5.0);
        assert_eq!(r.attr("made_up_flag", 0.0), 1.0, "a prefab's attributes reach the item");
        assert!(!r.attrs.contains_key("kill_eater_score_type"), "counters ignored");
        // The script's base, the effective numbers rounded once.
        assert_eq!(r.base.get("clip"), Some(&12.0));
        assert_eq!(r.effective.get("clip"), Some(&5.0), "12 x 0.4 = 4.8 -> 5");
        assert!((r.effective["damage"] - 15.0 * 0.575).abs() < 1e-9);
        assert_eq!(p.get(12).unwrap().unknown, ["no such attribute"]);
        assert!(p.get(13).unwrap().base.is_empty(), "no script for the club");
        assert_eq!(c.notes.len(), 1, "{:?}", c.notes);
        assert_eq!(p.scripts, ["tf_weapon_handgun_scout_secondary"]);
    }

    #[test]
    fn the_pack_is_deterministic_and_reads_back() {
        let a = convert(SCHEMA, &script).pack.to_text();
        let b = convert(SCHEMA, &script).pack.to_text();
        assert_eq!(a, b);
        assert_eq!(ItemPack::parse(&a).unwrap(), convert(SCHEMA, &script).pack);
        assert_ne!(fingerprint(SCHEMA.as_bytes()), fingerprint(b"other"));
    }

    #[test]
    fn identity_values_are_left_out_and_unknown_formats_add() {
        assert_eq!(Op::of(Some("value_is_additive_percentage")), Op::Add);
        assert_eq!(Op::of(None), Op::Add);
        assert_eq!(Op::of(Some("value_is_from_lookup_table")).fold(3.0, 2.0), 2.0);
        let c = convert(
            r#""items_game" { "items" { "1" { "item_class" "tf_weapon_bat" "attributes" { "x" { "attribute_class" "mult_dmg" "value" "1" } } } }
               "attributes" { "9" { "name" "x" "attribute_class" "mult_dmg" "description_format" "value_is_percentage" } } }"#,
            &|_| None,
        );
        assert!(c.pack.items[0].attrs.is_empty());
    }
}
