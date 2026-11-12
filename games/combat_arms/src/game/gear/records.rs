//! The gear and perk records (specs/combat_arms/gear_perks.md sections
//! 1 to 8, 12 to 14): our defaults (data/gear.toml) and, when the install
//! can be read, its records over them by id (`GearInstall`, read from
//! WEAPONS.TXT `Gear#`, MODELBUTES.TXT `Model#` and `Hair#`,
//! ATTACHMENTS.TXT `Goggle#` and `Backpack#`, MOD.TXT gear mods,
//! PERKSKILLINFO.TXT and SETITEM.TXT).

use std::collections::{BTreeMap, HashMap};

use lithtech_formats::attributes::{Document, Section};
use serde::{Deserialize, Serialize};

use crate::game::weapons::{Gear, Protection};

fn yes() -> bool {
    true
}
fn minus_one() -> i32 {
    -1
}
fn hundred() -> f32 {
    100.0
}
fn dash() -> f32 {
    0.4
}
fn standing_mass() -> f32 {
    2000.0
}

/// One entry of a piece (`ProtectionType{i}` with its four fractions).
#[derive(Deserialize, Clone, Copy, Debug, Default, Serialize)]
#[serde(deny_unknown_fields)]
struct EntryDef {
    ty: i32,
    #[serde(default)]
    head: f32,
    #[serde(default)]
    torso: f32,
    #[serde(default)]
    arm: f32,
    #[serde(default)]
    leg: f32,
    /// Sets the four.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    all: Option<f32>,
    #[serde(default = "yes")]
    consumable: bool,
}

impl EntryDef {
    fn protection(&self) -> Protection {
        let all = self.all;
        let at = |v: f32| all.unwrap_or(v).clamp(0.0, 1.0);
        Protection {
            damage_type: self.ty,
            head: at(self.head),
            torso: at(self.torso),
            arm: at(self.arm),
            leg: at(self.leg),
            consumable: self.consumable,
        }
    }
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct PieceDef {
    id: i32,
    name: String,
    #[serde(default)]
    armor: f32,
    #[serde(default)]
    health: f32,
    #[serde(default)]
    protects: Vec<EntryDef>,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct VestDef {
    id: i32,
    name: String,
    gear: i32,
    #[serde(default = "hundred")]
    max_armor: f32,
    #[serde(default)]
    max_hp: f32,
    #[serde(default)]
    move_penalty: f32,
    #[serde(default = "dash")]
    dash_speed: f32,
    #[serde(default)]
    max_stamina: f32,
    #[serde(default = "standing_mass")]
    mass: f32,
    #[serde(default)]
    figure: usize,
    /// `ReloadAniRate` and `SelectAniRate`: the vest's animation rate
    /// bonuses (view_models.md Corrections 6).
    #[serde(default)]
    reload_ani_rate: f32,
    #[serde(default)]
    select_ani_rate: f32,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct WornDef {
    id: i32,
    #[serde(default)]
    name: String,
    #[serde(default = "minus_one")]
    gear: i32,
    #[serde(default)]
    move_penalty: f32,
    #[serde(default)]
    dash_speed: f32,
    #[serde(default)]
    max_stamina: f32,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct MaskDef {
    ids: Vec<i32>,
    gear: i32,
    #[serde(default)]
    move_penalty: f32,
    #[serde(default)]
    max_stamina: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModSlot {
    Head,
    Face,
    Vest,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ModDef {
    id: i32,
    name: String,
    slot: ModSlot,
    #[serde(default = "minus_one")]
    gear: i32,
    #[serde(default)]
    move_penalty: f32,
    #[serde(default)]
    dash_speed: f32,
    #[serde(default)]
    max_stamina: f32,
}

/// A perk's kind (section 13).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PerkKind {
    Passive,
    Active,
    Mode,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct PerkDef {
    #[serde(rename = "type")]
    ty: i32,
    name: String,
    slots: u8,
    items: Vec<i32>,
    kind: PerkKind,
    #[serde(default)]
    values: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ItemDef {
    no: i32,
    weapon: String,
}

/// One effect of a set tier (section 12).
#[derive(Deserialize, Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetEffect {
    /// `EffectType`.
    pub kind: i32,
    pub value: f32,
    /// The condition list (`_ItemNo#`).
    #[serde(default)]
    pub only: Vec<i32>,
    /// Kind 4: the locations it covers (1 head, 2 torso, 3 arm, 4 leg);
    /// none: everywhere.
    #[serde(default)]
    pub locations: Vec<u8>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetTier {
    /// `ItemCount{j}`: the piece count that earns it.
    pub count: usize,
    #[serde(default)]
    pub effect: Vec<SetEffect>,
}

/// `SetItem#`: its items and tiers.
#[derive(Deserialize, Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetDef {
    #[serde(default)]
    pub id: i32,
    pub items: Vec<i32>,
    #[serde(default)]
    pub tier: Vec<SetTier>,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct File {
    /// How many gear records the data has (`GearData::piece_count`);
    /// absent: the highest piece id + 1. The written document has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    piece_count: Option<usize>,
    piece: Vec<PieceDef>,
    vest: Vec<VestDef>,
    head: Vec<WornDef>,
    goggle: Vec<WornDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worldcup_mask: Option<MaskDef>,
    backpack: Vec<WornDef>,
    gear_mod: Vec<ModDef>,
    perk: Vec<PerkDef>,
    #[serde(default)]
    item: Vec<ItemDef>,
    #[serde(default)]
    set: Vec<SetDef>,
}

/// A gear piece (`Gear#`).
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub name: String,
    /// Repairs the armour pool by this much when added to a living player.
    pub armor: f32,
    /// Heals instead when `armor` is 0.
    pub health: f32,
    pub gear: Gear,
}

/// A vest (`Model#`).
#[derive(Clone, Debug, PartialEq)]
pub struct Vest {
    pub name: String,
    pub gear: i32,
    pub max_armor: f32,
    /// 0: the figure's `HP`.
    pub max_hp: f32,
    pub move_penalty: f32,
    pub dash_speed: f32,
    pub max_stamina: f32,
    pub mass: f32,
    pub figure: usize,
    pub reload_ani_rate: f32,
    pub select_ani_rate: f32,
}

/// A head piece, goggles or a backpack (`Hair#`, `Goggle#`, `Backpack#`).
#[derive(Clone, Debug, PartialEq)]
pub struct Worn {
    pub name: String,
    pub gear: i32,
    pub move_penalty: f32,
    pub dash_speed: f32,
    pub max_stamina: f32,
}

/// A gear mod (`Mod#`, `Kinds` 5 to 7).
#[derive(Clone, Debug, PartialEq)]
pub struct GearMod {
    pub name: String,
    pub slot: ModSlot,
    pub gear: i32,
    pub move_penalty: f32,
    pub dash_speed: f32,
    pub max_stamina: f32,
}

/// A perk (`PerkSkill_N`, section 14): its items, slots and numbers by
/// level (0 basic, 1 Expert).
#[derive(Clone, Debug, PartialEq)]
pub struct PerkRecord {
    pub ty: i32,
    pub name: String,
    pub slots: u8,
    /// The shop items: basic, then Expert.
    pub items: Vec<i32>,
    pub kind: PerkKind,
    pub values: BTreeMap<String, [f32; 2]>,
}

impl PerkRecord {
    /// A number at `level` (0 basic, 1 Expert); 0 when the perk has none.
    pub fn value(&self, key: &str, level: usize) -> f32 {
        self.values.get(key).map_or(0.0, |v| v[level.min(1)])
    }

    /// The level an owned item gives (0 basic, 1 Expert).
    pub fn level_of(&self, item: i32) -> Option<usize> {
        self.items.iter().position(|&i| i == item).map(|p| p.min(1))
    }
}

/// Every record of the gear and perk systems.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearData {
    pub pieces: HashMap<i32, Piece>,
    pub vests: HashMap<i32, Vest>,
    pub heads: HashMap<i32, Worn>,
    pub goggles: HashMap<i32, Worn>,
    pub backpacks: HashMap<i32, Worn>,
    pub mods: HashMap<i32, GearMod>,
    pub perks: Vec<PerkRecord>,
    /// Item number to the weapon id it is (per-weapon set bonuses).
    pub items: HashMap<i32, String>,
    pub sets: Vec<SetDef>,
    /// How many gear records the data has (the install's, else ours):
    /// quirk 1 applies above 255.
    pub piece_count: usize,
}

/// The built-in data (data/gear.toml).
const BUILT_IN: &str = include_str!("../../../data/gear.toml");

fn number(v: &toml::Value) -> Option<f32> {
    v.as_float().map(|f| f as f32).or_else(|| v.as_integer().map(|i| i as f32))
}

impl GearData {
    /// Vest record `model`'s animation rate bonuses (`ReloadAniRate`,
    /// `SelectAniRate`; an unknown record: model 0's).
    pub fn vest_rates(&self, model: i32) -> (f32, f32) {
        self.vests
            .get(&model)
            .or(self.vests.get(&0))
            .map_or((0.0, 0.0), |v| (v.reload_ani_rate, v.select_ani_rate))
    }

    /// The document's format (`chimera_sim::data_format`).
    pub fn format() -> String {
        chimera_sim::data_format::describe::<File>()
    }

    /// Parse a gear document (data/gear.toml's format).
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut data = GearData::default();
        for p in &file.piece {
            let entries: Vec<Protection> = p.protects.iter().map(EntryDef::protection).collect();
            data.pieces.insert(
                p.id,
                Piece {
                    name: p.name.clone(),
                    armor: p.armor,
                    health: p.health,
                    gear: Gear::new(&entries),
                },
            );
        }
        for v in &file.vest {
            data.vests.insert(
                v.id,
                Vest {
                    name: v.name.clone(),
                    gear: v.gear,
                    max_armor: v.max_armor,
                    max_hp: v.max_hp,
                    move_penalty: v.move_penalty,
                    dash_speed: v.dash_speed,
                    max_stamina: v.max_stamina,
                    mass: v.mass,
                    figure: v.figure,
                    reload_ani_rate: v.reload_ani_rate,
                    select_ani_rate: v.select_ani_rate,
                },
            );
        }
        let worn = |w: &WornDef| Worn {
            name: w.name.clone(),
            gear: w.gear,
            move_penalty: w.move_penalty,
            dash_speed: w.dash_speed,
            max_stamina: w.max_stamina,
        };
        for w in &file.head {
            data.heads.insert(w.id, worn(w));
        }
        for w in &file.goggle {
            data.goggles.insert(w.id, worn(w));
        }
        if let Some(m) = &file.worldcup_mask {
            for &id in &m.ids {
                data.goggles.entry(id).or_insert_with(|| Worn {
                    name: format!("Worldcup_Skull_Mask_{id}"),
                    gear: m.gear,
                    move_penalty: m.move_penalty,
                    dash_speed: 0.0,
                    max_stamina: m.max_stamina,
                });
            }
        }
        for w in &file.backpack {
            data.backpacks.insert(w.id, worn(w));
        }
        for m in &file.gear_mod {
            data.mods.insert(
                m.id,
                GearMod {
                    name: m.name.clone(),
                    slot: m.slot,
                    gear: m.gear,
                    move_penalty: m.move_penalty,
                    dash_speed: m.dash_speed,
                    max_stamina: m.max_stamina,
                },
            );
        }
        for p in &file.perk {
            let values = p
                .values
                .iter()
                .filter_map(|(k, v)| {
                    let pair = match v.as_array() {
                        Some(a) if a.len() >= 2 => [number(&a[0])?, number(&a[1])?],
                        _ => {
                            let n = number(v)?;
                            [n, n]
                        }
                    };
                    Some((k.clone(), pair))
                })
                .collect();
            data.perks.push(PerkRecord {
                ty: p.ty,
                name: p.name.clone(),
                slots: p.slots,
                items: p.items.clone(),
                kind: p.kind,
                values,
            });
        }
        for i in &file.item {
            data.items.insert(i.no, i.weapon.clone());
        }
        data.sets = file.set;
        data.piece_count = file
            .piece_count
            .unwrap_or_else(|| data.pieces.keys().max().map_or(0, |&m| m as usize + 1));
        Ok(data)
    }

    /// This data as one gear document (data/gear.toml's format, records
    /// by id): `parse` reads it back as this data. What a server sends a
    /// client whose own gear data differs (`data::served::GEAR`); the same
    /// data writes the same text on every machine.
    pub fn to_doc(&self) -> String {
        fn sorted<'a, T: 'a>(m: impl IntoIterator<Item = (&'a i32, &'a T)>) -> Vec<(i32, &'a T)> {
            let mut v: Vec<(i32, &T)> = m.into_iter().map(|(k, v)| (*k, v)).collect();
            v.sort_by_key(|(k, _)| *k);
            v
        }
        let worn = |(id, w): (i32, &Worn)| WornDef {
            id,
            name: w.name.clone(),
            gear: w.gear,
            move_penalty: w.move_penalty,
            dash_speed: w.dash_speed,
            max_stamina: w.max_stamina,
        };
        let mut items: Vec<ItemDef> = self
            .items
            .iter()
            .map(|(no, weapon)| ItemDef {
                no: *no,
                weapon: weapon.clone(),
            })
            .collect();
        items.sort_by_key(|i| i.no);
        let file = File {
            piece: sorted(&self.pieces)
                .into_iter()
                .map(|(id, p)| PieceDef {
                    id,
                    name: p.name.clone(),
                    armor: p.armor,
                    health: p.health,
                    protects: p
                        .gear
                        .protects
                        .iter()
                        .map(|e| EntryDef {
                            ty: e.damage_type,
                            head: e.head,
                            torso: e.torso,
                            arm: e.arm,
                            leg: e.leg,
                            all: None,
                            consumable: e.consumable,
                        })
                        .collect(),
                })
                .collect(),
            vest: sorted(&self.vests)
                .into_iter()
                .map(|(id, v)| VestDef {
                    id,
                    name: v.name.clone(),
                    gear: v.gear,
                    max_armor: v.max_armor,
                    max_hp: v.max_hp,
                    move_penalty: v.move_penalty,
                    dash_speed: v.dash_speed,
                    max_stamina: v.max_stamina,
                    mass: v.mass,
                    figure: v.figure,
                    reload_ani_rate: v.reload_ani_rate,
                    select_ani_rate: v.select_ani_rate,
                })
                .collect(),
            head: sorted(&self.heads).into_iter().map(worn).collect(),
            goggle: sorted(&self.goggles).into_iter().map(worn).collect(),
            worldcup_mask: None,
            backpack: sorted(&self.backpacks).into_iter().map(worn).collect(),
            gear_mod: sorted(&self.mods)
                .into_iter()
                .map(|(id, m)| ModDef {
                    id,
                    name: m.name.clone(),
                    slot: m.slot,
                    gear: m.gear,
                    move_penalty: m.move_penalty,
                    dash_speed: m.dash_speed,
                    max_stamina: m.max_stamina,
                })
                .collect(),
            perk: self
                .perks
                .iter()
                .map(|p| PerkDef {
                    ty: p.ty,
                    name: p.name.clone(),
                    slots: p.slots,
                    items: p.items.clone(),
                    kind: p.kind,
                    values: p
                        .values
                        .iter()
                        .map(|(k, [a, b])| {
                            let pair = vec![toml::Value::Float(f64::from(*a)), toml::Value::Float(f64::from(*b))];
                            (k.clone(), toml::Value::Array(pair))
                        })
                        .collect(),
                })
                .collect(),
            item: items,
            set: self.sets.clone(),
            piece_count: Some(self.piece_count),
        };
        toml::to_string(&file).expect("gear records serialize")
    }

    /// The built-in data.
    pub fn built_in() -> Self {
        Self::parse(BUILT_IN).unwrap_or_else(|e| panic!("data/gear.toml: {e}"))
    }

    /// The install's records over this data, by id (a record the install
    /// has replaces ours; ours it lacks stay).
    pub fn with_install(mut self, install: &GearInstall) -> Self {
        for (id, p) in install.pieces.iter().enumerate() {
            self.pieces.insert(id as i32, p.clone());
        }
        if !install.pieces.is_empty() {
            self.piece_count = install.pieces.len();
        }
        for (id, v) in install.vests.iter().enumerate() {
            self.vests.insert(id as i32, v.clone());
        }
        for (id, w) in install.heads.iter().enumerate() {
            self.heads.insert(id as i32, w.clone());
        }
        for (id, w) in install.goggles.iter().enumerate() {
            self.goggles.insert(id as i32, w.clone());
        }
        for (id, w) in install.backpacks.iter().enumerate() {
            self.backpacks.insert(id as i32, w.clone());
        }
        for (id, m) in install.mods.iter() {
            self.mods.insert(*id, m.clone());
        }
        for p in &mut self.perks {
            if let Some(record) = install.perks.iter().find(|r| r.ty == p.ty) {
                for (key, value) in p.values.iter_mut() {
                    if let Some(v) = record.values(key) {
                        *value = v;
                    }
                }
                if let Some(items) = record.items() {
                    p.items = items;
                }
            }
        }
        if !install.sets.is_empty() {
            self.sets = install.sets.clone();
        }
        self
    }
}

// ---------------------------------------------------------------------------
// The install's records

/// A `PerkSkill_N` record's section, to read a perk's numbers by level
/// (Open question 1: a key with the suffix `_0` or `_1` belongs to that
/// level, an unsuffixed one to both).
#[derive(Clone, Debug, PartialEq)]
pub struct PerkInstall {
    pub ty: i32,
    keys: HashMap<String, f64>,
}

impl PerkInstall {
    fn read(s: &Section) -> Self {
        let keys = s
            .iter()
            .filter_map(|(k, _)| s.number_opt(k).map(|n| (k.to_ascii_lowercase(), n)))
            .collect();
        Self { ty: s.int("Type", -1), keys }
    }

    /// `key`'s basic and Expert values, if the record has it.
    fn values(&self, key: &str) -> Option<[f32; 2]> {
        let key = key.to_ascii_lowercase();
        let plain = self.keys.get(&key).copied();
        let level = |i: u8| self.keys.get(&format!("{key}_{i}")).copied().or(plain).map(|v| v as f32);
        match (level(0), level(1)) {
            (None, None) => None,
            (a, b) => Some([a.or(b)?, b.or(a)?]),
        }
    }

    /// `ItemNo_0` and `ItemNo_1`.
    fn items(&self) -> Option<Vec<i32>> {
        let a = self.keys.get("itemno_0").copied()? as i32;
        let mut items = vec![a];
        items.extend(self.keys.get("itemno_1").map(|v| *v as i32));
        Some(items)
    }
}

/// What the install's attribute files give the gear system, by record
/// index (the ids of the game).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearInstall {
    pub pieces: Vec<Piece>,
    pub vests: Vec<Vest>,
    pub heads: Vec<Worn>,
    pub goggles: Vec<Worn>,
    pub backpacks: Vec<Worn>,
    pub mods: Vec<(i32, GearMod)>,
    pub perks: Vec<PerkInstall>,
    pub sets: Vec<SetDef>,
}

fn name_of(s: &Section) -> String {
    s.fixed_string("Name", "")
}

/// A gear id: missing is -1 (no piece).
fn gear_id(s: &Section) -> i32 {
    s.int("GearId", s.int("GearID", -1))
}

fn read_worn(s: &Section) -> Worn {
    Worn {
        name: name_of(s),
        gear: gear_id(s),
        move_penalty: s.number("MovePenalty", 0.0) as f32,
        dash_speed: s.number("DashSpeed", 0.0) as f32,
        max_stamina: s.number("MaxStamina", s.number("Maxstamina", 0.0)) as f32,
    }
}

impl GearInstall {
    /// Read the records from the attribute documents that were found.
    pub fn read(
        weapons: Option<&Document>,
        models: Option<&Document>,
        attachments: Option<&Document>,
        mods: Option<&Document>,
        perks: Option<&Document>,
        sets: Option<&Document>,
    ) -> Self {
        let mut out = GearInstall::default();
        if let Some(d) = weapons {
            for s in d.records("Gear") {
                out.pieces.push(read_piece(s));
            }
        }
        if let Some(d) = models {
            for s in d.records("Model") {
                out.vests.push(Vest {
                    name: name_of(s),
                    gear: gear_id(s),
                    max_armor: s.number("MaxArmor", 0.0) as f32,
                    max_hp: s.number("MaxHitPoints", 0.0) as f32,
                    move_penalty: s.number("MovePenalty", 0.0) as f32,
                    dash_speed: s.number("DashSpeed", 0.0) as f32,
                    max_stamina: s.number("MaxStamina", s.number("Maxstamina", 0.0)) as f32,
                    mass: s.number("Mass", 2000.0) as f32,
                    figure: s.int("FigureType", 0).max(0) as usize,
                    reload_ani_rate: s.number("ReloadAniRate", 0.0) as f32,
                    select_ani_rate: s.number("SelectAniRate", 0.0) as f32,
                });
            }
            for s in d.records("Hair") {
                out.heads.push(read_worn(s));
            }
        }
        if let Some(d) = attachments {
            out.goggles = d.records("Goggle").into_iter().map(read_worn).collect();
            out.backpacks = d.records("Backpack").into_iter().map(read_worn).collect();
        }
        if let Some(d) = mods {
            for (id, s) in d.records("Mod").into_iter().enumerate() {
                let slot = match s.int("Kinds", -1) {
                    5 => ModSlot::Head,
                    6 => ModSlot::Face,
                    7 => ModSlot::Vest,
                    _ => continue,
                };
                let w = read_worn(s);
                out.mods.push((
                    id as i32,
                    GearMod {
                        name: w.name,
                        slot,
                        gear: s.int("GearID", s.int("GearId", -1)),
                        move_penalty: w.move_penalty,
                        dash_speed: w.dash_speed,
                        max_stamina: w.max_stamina,
                    },
                ));
            }
        }
        if let Some(d) = perks {
            out.perks = d.records("PerkSkill_").into_iter().map(PerkInstall::read).collect();
        }
        if let Some(d) = sets {
            out.sets = read_sets(d);
        }
        out
    }
}

fn read_piece(s: &Section) -> Piece {
    let types = s.int_list("ProtectionType");
    let list = |k: &str| s.number_list(k);
    let (head, torso, arm, leg) = (
        list("NodeHeadProtection"),
        list("NodeTorsoProtection"),
        list("NodeArmProtection"),
        list("NodelegProtection"),
    );
    let consumable = s.int_list("Consumable");
    let at = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) as f32;
    let entries: Vec<Protection> = types
        .iter()
        .enumerate()
        .map(|(i, &ty)| Protection {
            damage_type: ty,
            head: at(&head, i),
            torso: at(&torso, i),
            arm: at(&arm, i),
            leg: at(&leg, i),
            // Missing: consumable.
            consumable: consumable.get(i).is_none_or(|&c| c != 0),
        })
        .collect();
    Piece {
        name: name_of(s),
        armor: s.number("Armor", 0.0) as f32,
        health: s.number("Health", 0.0) as f32,
        gear: Gear::new(&entries),
    }
}

/// `Effect0..` and `SetItem0..` (section 12): the effects by kind, the
/// sets with their items, tiers and effects.
fn read_sets(d: &Document) -> Vec<SetDef> {
    let kinds: Vec<i32> = d.records("Effect").into_iter().map(|s| s.int("EffectType", -1)).collect();
    d.records("SetItem")
        .into_iter()
        .enumerate()
        .map(|(id, s)| {
            let items = s.int_list("ItemNo");
            let mut tier = Vec::new();
            for j in 0.. {
                let Some(count) = s.get(&format!("ItemCount{j}")).map(|_| s.int(&format!("ItemCount{j}"), 0)) else {
                    break;
                };
                let mut effect = Vec::new();
                for k in 0.. {
                    let prefix = format!("ItemCount{j}_Effect{k}");
                    if s.get(&format!("{prefix}_ID")).is_none() {
                        break;
                    }
                    let effect_id = s.int(&format!("{prefix}_ID"), -1);
                    let kind = kinds.get(effect_id.max(0) as usize).copied().unwrap_or(-1);
                    effect.push(SetEffect {
                        kind,
                        value: s.number(&format!("{prefix}_Value"), 0.0) as f32,
                        only: s.int_list(&format!("{prefix}_ItemNo")),
                        // Effect records 14 and 15 are protection kept
                        // to the head and the torso.
                        locations: match effect_id {
                            14 => vec![1],
                            15 => vec![2],
                            _ => Vec::new(),
                        },
                    });
                }
                tier.push(SetTier {
                    count: count.max(0) as usize,
                    effect,
                });
            }
            SetDef {
                id: id as i32,
                items,
                tier,
            }
        })
        .collect()
}
