//! The lobby's item list (specs/combat_arms/lobby.md E, P): what a player
//! may equip, as data (`data/lobby_items.toml` for weapons; gear built
//! from `data/gear.toml` by item number), and the shop's item lifetimes
//! with what an expired item does to the sets.

use std::{collections::BTreeMap, sync::Arc};

use serde::Deserialize;

use crate::game::gear::records::GearData;

/// data/lobby_items.toml, built in.
pub const DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/lobby_items.toml");

/// Item numbers by type (E, "Item numbers").
pub const HEAD_BASE: i32 = 4000;
pub const FACE_BASE: i32 = 5000;
pub const BACKPACK_BASE: i32 = 7000;
pub const CHARACTER_BASE: i32 = 10000;
/// A rental copy of an item is its number plus this.
pub const RENTAL_OFFSET: i32 = 100_000_000;
/// The shop period that means "never expires".
pub const PERMANENT: u32 = 999;

/// What an item is.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Knife,
    Throwable,
    Assault,
    Smg,
    Sniper,
    Mg,
    Rocket,
    Flame,
    /// A primary shotgun.
    Shotgun,
    Pistol,
    /// The item type listed as shotgun (type 5): a secondary.
    Shotgun2,
    Special,
    Assassin,
    Crossbow,
    // Gear.
    Head,
    Face,
    Uniform,
    Vest,
    Backpack,
    Character,
}

impl Kind {
    /// Goes in the primary slot (E, "What may go where").
    pub fn primary(self) -> bool {
        matches!(
            self,
            Kind::Assault | Kind::Smg | Kind::Sniper | Kind::Mg | Kind::Rocket | Kind::Flame | Kind::Shotgun
        )
    }

    pub fn secondary(self) -> bool {
        matches!(self, Kind::Pistol | Kind::Shotgun2)
    }

    pub fn weapon(self) -> bool {
        self.primary() || self.secondary() || matches!(self, Kind::Knife | Kind::Throwable | Kind::Special | Kind::Assassin | Kind::Crossbow)
    }

    /// Special and assassin items go only in a backpack slot.
    pub fn backpack_only(self) -> bool {
        matches!(self, Kind::Special | Kind::Assassin)
    }

    /// The expiry fallback's type priority (P): lower first.
    pub fn priority(self) -> u32 {
        match self {
            Kind::Assault => 0,
            Kind::Smg => 1,
            Kind::Sniper => 2,
            Kind::Shotgun => 3,
            Kind::Mg => 4,
            Kind::Rocket => 5,
            Kind::Crossbow => 6,
            Kind::Special => 7,
            Kind::Assassin => 8,
            Kind::Throwable => 9,
            Kind::Pistol => 10,
            _ => 11,
        }
    }
}

/// An item of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub no: i32,
    pub name: String,
    pub kind: Kind,
    /// The weapon document it is (our weapon the item stands on).
    pub weapon: Option<String>,
    /// Owned by everyone, never expiring, never released.
    pub base: bool,
    pub explosive: bool,
    pub grade: u32,
    /// Two of it may be equipped at once.
    pub duplicates: bool,
    /// `CantWithEquip`: items that cannot be equipped with this one.
    pub cant_with: Vec<i32>,
    /// A backpack's `Slot`: weapons it carries.
    pub slots: u8,
    /// On the Pure Combat list.
    pub pure_combat: bool,
}

#[derive(Deserialize)]
struct ItemDef {
    no: i32,
    name: String,
    kind: Kind,
    weapon: Option<String>,
    #[serde(default)]
    base: bool,
    #[serde(default)]
    explosive: bool,
    #[serde(default)]
    grade: u32,
}

#[derive(Deserialize)]
struct Base {
    head: i32,
    backpack: i32,
}

#[derive(Deserialize)]
struct File {
    item: Vec<ItemDef>,
    base: Base,
}

/// The whole list.
#[derive(Clone, Debug)]
pub struct Catalog {
    pub items: BTreeMap<i32, Item>,
    /// The gear data the list stands on (abilities, perks).
    pub gear: Arc<GearData>,
    /// The base head and backpack (the new player's).
    pub base_head: i32,
    pub base_backpack: i32,
}

impl Catalog {
    /// The list from its document and the gear data.
    pub fn build(text: &str, gear: &GearData) -> Result<Self, String> {
        let file: File = toml::from_str(text).map_err(|e| format!("lobby_items.toml: {e}"))?;
        let mut items = BTreeMap::new();
        let mut add = |i: Item| {
            items.insert(i.no, i);
        };
        for d in file.item {
            add(Item {
                no: d.no,
                name: d.name,
                kind: d.kind,
                weapon: d.weapon,
                base: d.base,
                explosive: d.explosive,
                grade: d.grade,
                duplicates: false,
                cant_with: Vec::new(),
                slots: 0,
                pure_combat: d.base || d.grade <= 2,
            });
        }
        let gear_item = |no: i32, name: &str, kind: Kind, slots: u8, base: bool| Item {
            no,
            name: name.to_string(),
            kind,
            weapon: None,
            base,
            explosive: false,
            grade: 0,
            duplicates: false,
            cant_with: Vec::new(),
            slots,
            pure_combat: true,
        };
        for (id, w) in &gear.heads {
            add(gear_item(HEAD_BASE + id, &w.name, Kind::Head, 0, HEAD_BASE + id == file.base.head));
        }
        for (id, w) in &gear.goggles {
            add(gear_item(FACE_BASE + id, &w.name, Kind::Face, 0, false));
        }
        for (id, w) in &gear.backpacks {
            add(gear_item(
                BACKPACK_BASE + id,
                &w.name,
                Kind::Backpack,
                (*id).clamp(0, 4) as u8,
                BACKPACK_BASE + id == file.base.backpack,
            ));
        }
        for (id, v) in &gear.vests {
            add(gear_item(CHARACTER_BASE + id, &v.name, Kind::Character, 0, *id == 0));
        }
        // The spec's base head and backpack when the gear data lacks them.
        if !items.contains_key(&file.base.head) {
            items.insert(file.base.head, gear_item(file.base.head, "Black Cap", Kind::Head, 0, true));
        }
        if !items.contains_key(&file.base.backpack) {
            items.insert(file.base.backpack, gear_item(file.base.backpack, "Standard Backpack", Kind::Backpack, 1, true));
        }
        Ok(Self {
            items,
            gear: Arc::new(gear.clone()),
            base_head: file.base.head,
            base_backpack: file.base.backpack,
        })
    }

    /// The built-in list over our own gear data (tests, defaults).
    pub fn built_in() -> Arc<Self> {
        Self::over(crate::game::gear::gear_data())
    }

    /// The built-in list over `gear`: the app's (`CaData::gear`, the
    /// install's records over ours).
    pub fn over(gear: &GearData) -> Arc<Self> {
        Arc::new(Self::build(&DATA, gear).expect("data/lobby_items.toml parses"))
    }

    pub fn get(&self, no: i32) -> Option<&Item> {
        self.items.get(&no)
    }

    /// The base item for a weapon slot kind (M16A3, Beretta, M9, M67).
    pub fn base_for(&self, pick: impl Fn(&Item) -> bool) -> Option<&Item> {
        self.items.values().find(|i| i.base && pick(i))
    }
}

// ---------------------------------------------------------------------------
// P: item lifetimes

/// How a product counts its period (`Consumetype0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consume {
    /// Days; 999 never expires.
    Days,
    /// A number of uses.
    Count,
    Hours,
    Minutes,
}

impl Consume {
    /// The shop file's `Consumetype0`: 0 days, 3 count, 5 hours, 7
    /// minutes; other values (boxes, GP, coupons) are not equipment
    /// durations.
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Consume::Days),
            3 => Some(Consume::Count),
            5 => Some(Consume::Hours),
            7 => Some(Consume::Minutes),
            _ => None,
        }
    }
}

/// When an item bought at `bought` (days on the service's clock) expires:
/// None for a permanent one or a use count.
pub fn expires_at(bought: f64, consume: Consume, period: u32) -> Option<f64> {
    match consume {
        Consume::Days if period == PERMANENT => None,
        Consume::Days => Some(bought + period as f64),
        Consume::Hours => Some(bought + period as f64 / 24.0),
        Consume::Minutes => Some(bought + period as f64 / 1440.0),
        Consume::Count => None,
    }
}

/// The items a player owns, with their expiry (days); None: every item of
/// the list (an offline profile has no shop).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Owned {
    /// Item number to expiry (None: permanent).
    pub items: Option<BTreeMap<i32, Option<f64>>>,
}

impl Owned {
    /// Everything owned, nothing expiring.
    pub fn all() -> Self {
        Self { items: None }
    }

    /// Only the base items (a fresh account) until more are bought.
    pub fn base_only(catalog: &Catalog) -> Self {
        Self {
            items: Some(catalog.items.values().filter(|i| i.base).map(|i| (i.no, None)).collect()),
        }
    }

    pub fn owns(&self, catalog: &Catalog, no: i32) -> bool {
        match &self.items {
            None => catalog.get(no).is_some(),
            Some(m) => m.contains_key(&no) || catalog.get(no).is_some_and(|i| i.base),
        }
    }

    pub fn grant(&mut self, no: i32, expires: Option<f64>) {
        if let Some(m) = &mut self.items {
            m.insert(no, expires);
        }
    }

    /// Remove the items past their time at `now` (days): the checked at
    /// login, after a match and after single play. The expired item
    /// numbers.
    pub fn expire(&mut self, now: f64) -> Vec<i32> {
        let Some(m) = &mut self.items else { return Vec::new() };
        let gone: Vec<i32> = m.iter().filter(|(_, e)| e.is_some_and(|e| e <= now)).map(|(n, _)| *n).collect();
        for n in &gone {
            m.remove(n);
        }
        gone
    }
}
