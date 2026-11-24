//! Rung passives (docs/plans/active/rung-passives.md): an item that isn't
//! a rung's weapon but rides with it, worn (boots, a backpack) or carried
//! and switched to. A rung extra (`ladder::RungExtras::passives`, item
//! weapon ids), drawn per rung with the ladder from its seeded generator,
//! the same for everyone on the rung, replicated with the match's extras.
//! Chimera names no game: it owns the field, the draw (`draw`), the
//! options, the conflict rule and the giving helper (`wear`); each game
//! owns its table (`PassiveTable`, its `rung_passives.toml`) and calls
//! `draw` from its hook's draw and `wear` from its hook's `apply`.
//!
//! - **Off by default** (ours, not any game's: parity first). On with
//!   `rung_extras = { passives = true }` or a table (`passives = { odds =
//!   0.5, max = 2 }`; `on = false` in it turns them off again), so the host
//!   settings' rows (`rules.rung_extras.passives.on`,
//!   `rules.rung_extras.passives.odds`) set one key each. `rung_extras =
//!   false` turns them off with the rest.
//! - **The draw** (`draw`): a melee band rung (the knife finale's) gets
//!   none. Else the rung's odds are rolled (the table's `odds`, the
//!   definition's over them); a miss: none. The candidates are the
//!   table's items for the rung's class (`"*"`: any; a game without
//!   classes uses `"*"`) that the definition's sets can give, minus those
//!   in conflict with the rung's gun: an item whose `slot` is the gun's
//!   for that class (`PassiveTable::gun_slot`) or that takes a key the
//!   gun uses (`PassiveTable::gun_keys`). One is picked by weight; with
//!   `max` above 1 the odds are rolled again for each next one, from a
//!   different `group` (one pair of boots). Nothing is rolled while off,
//!   so a ladder without passives draws as before.
//! - **Giving** (`wear`, a game's `apply`, the host's): the rung's items
//!   of that game given (`RungPassive` on each; a worn item's own hook
//!   makes it unselectable), the others of that game it gave taken; what
//!   the player was switching to is kept. Inventory replicates, so a
//!   client has them.
//! - **Kills** by a passive (a stomp, a bash) climb as any kill and never
//!   cost the victim a rung with `demote_on = "melee_death"`
//!   (`ladder::Ladder::is_passive`).
//!
//! - **Bots** use a keyed item through the game's hook in
//!   `core::BotItems` (reading `core::BotFocus`, the brain's target); a
//!   brain leaves `RungPassive` weapons out of its guns.
//!
//! Known shortcuts: no HUD yet (P2: the user's pick of the mockup); the odds row of the host
//! settings shows whether passives are on or not (no progressive
//! disclosure yet), and setting it alone turns them on.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use super::{
    auto::Rng,
    ladder::{RungDraw, RungOptions},
};
use crate::weapon::{Inventory, Weapon, WeaponRegistry, give};

/// The option key (`rung_extras = { passives = ... }`).
pub const OPTION: &str = "passives";

/// A game's rung passives table (its `rung_passives.toml`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassiveTable {
    /// A rung's chance of a passive when the host has them on (0 to 1).
    #[serde(default = "default_odds")]
    pub odds: f64,
    /// At most this many on a rung.
    #[serde(default = "one")]
    pub max: usize,
    /// A weapon's data slot (0, 1, ...) by name (`["primary", "secondary",
    /// "melee"]`), the names `slot` and `slots` use.
    #[serde(default)]
    pub slot_names: Vec<String>,
    /// The slot a gun takes for a class where it isn't its data's: gun id
    /// -> class (`"*"`: any) -> slot name.
    #[serde(default)]
    pub slots: BTreeMap<String, BTreeMap<String, String>>,
    /// The keys a gun uses beyond fire (`attack2`): gun id -> keys.
    #[serde(default)]
    pub gun_keys: BTreeMap<String, Vec<String>>,
    /// The items.
    #[serde(default, rename = "passive")]
    pub passives: Vec<Passive>,
}

fn default_odds() -> f64 {
    0.25
}

fn one() -> usize {
    1
}

fn any_class() -> Vec<String> {
    vec!["*".into()]
}

fn weight() -> f64 {
    1.0
}

/// How a passive is held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassiveKind {
    /// Worn: never drawn, it changes its wearer.
    #[default]
    Worn,
    /// Carried: switched to as a weapon in its slot.
    Carried,
}

/// One item of a table.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Passive {
    /// Its weapon id.
    pub item: String,
    /// The classes it may go with (`"*"`: any).
    #[serde(default = "any_class")]
    pub classes: Vec<String>,
    /// The loadout slot it takes (none: no slot conflict).
    #[serde(default)]
    pub slot: String,
    /// One item of a group on a rung.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default = "weight")]
    pub weight: f64,
    #[serde(default)]
    pub kind: PassiveKind,
    /// The keys it takes from any weapon (`attack2`).
    #[serde(default)]
    pub keys: Vec<String>,
    /// One line of what it does (the HUD's hover card).
    #[serde(default)]
    pub summary: Option<String>,
}

impl PassiveTable {
    /// `text` (a game's `rung_passives.toml`).
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The slot gun `gun` (data slot `data_slot`) takes for `class`.
    pub fn gun_slot(&self, gun: &str, class: &str, data_slot: u8) -> String {
        if let Some(by) = self.slots.get(gun)
            && let Some(s) = by.get(class).or_else(|| by.get("*"))
        {
            return s.clone();
        }
        self.slot_names.get(data_slot as usize).cloned().unwrap_or_else(|| data_slot.to_string())
    }

    /// The keys gun `gun` uses beyond fire.
    pub fn gun_keys(&self, gun: &str) -> &[String] {
        self.gun_keys.get(gun).map_or(&[], Vec::as_slice)
    }

    /// Item `id`'s row.
    pub fn get(&self, id: &str) -> Option<&Passive> {
        self.passives.iter().find(|p| p.item == id)
    }

    /// The items a rung of `class` on a gun in slot `slot` using `keys`
    /// may draw (the conflict rule), of those `available`.
    pub fn candidates(&self, class: &str, slot: &str, keys: &[String], available: &dyn Fn(&str) -> bool) -> Vec<&Passive> {
        self.passives
            .iter()
            .filter(|p| p.weight > 0.0)
            .filter(|p| p.classes.iter().any(|c| c == "*" || c == class))
            .filter(|p| p.slot.is_empty() || p.slot != slot)
            .filter(|p| !p.keys.iter().any(|k| keys.contains(k)))
            .filter(|p| available(&p.item))
            .collect()
    }
}

/// What a definition wants of the passives: on, at these odds, at most
/// `max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassiveSettings {
    pub odds: f64,
    pub max: usize,
}

/// The passives' settings for `options` over `table`'s, or none (off: the
/// default).
pub fn settings(options: &RungOptions, table: &PassiveTable) -> Option<PassiveSettings> {
    if options.off || options.keys.get(OPTION) != Some(&true) {
        return None;
    }
    let tuned = options.table(OPTION);
    let num = |k: &str| {
        let v = tuned?.get(k)?;
        v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
    };
    if tuned.and_then(|t| t.get("on")).and_then(|v| v.as_bool()) == Some(false) {
        return None;
    }
    Some(PassiveSettings {
        odds: num("odds").unwrap_or(table.odds).clamp(0.0, 1.0),
        max: num("max").map_or(table.max, |m| m.max(0.0) as usize),
    })
}

/// The rung's gun as the conflict rule sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RungGun<'a> {
    /// The rung's class (`"*"`: a game without classes).
    pub class: &'a str,
    /// The gun's slot for that class (`PassiveTable::gun_slot`).
    pub slot: String,
    /// The keys it uses beyond fire.
    pub keys: &'a [String],
}

/// Rung `d`'s passives from `table` (module doc), drawn on `rng`; none
/// while off (nothing rolled), on the melee band or at a missed roll.
/// `available`: an item the game can give now (its weapon is registered).
pub fn draw(table: &PassiveTable, d: &RungDraw, gun: &RungGun, rng: &mut Rng, available: &dyn Fn(&str) -> bool) -> Vec<String> {
    let Some(s) = settings(d.options, table) else { return Vec::new() };
    if d.melee {
        return Vec::new();
    }
    let mut pool = table.candidates(gun.class, &gun.slot, gun.keys, available);
    let mut out = Vec::new();
    while out.len() < s.max && !pool.is_empty() {
        if rng.unit() >= s.odds {
            break;
        }
        let total: f64 = pool.iter().map(|p| p.weight).sum();
        let mut at = rng.unit() * total;
        let i = pool
            .iter()
            .position(|p| {
                at -= p.weight;
                at < 0.0
            })
            .unwrap_or(pool.len() - 1);
        let picked = pool.remove(i);
        pool.retain(|p| p.item != picked.item && (picked.group.is_none() || p.group != picked.group));
        out.push(picked.item.clone());
    }
    out
}

/// Whether the registry can make weapon `id` (an item a pack lacks: no).
pub fn registered(world: &World, id: &str) -> bool {
    world.get_resource::<WeaponRegistry>().is_some_and(|r| r.find(id).is_some())
}

/// On a weapon a rung passive gave (`wear`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RungPassive;

/// Player `e` wears `ids`, game `game`'s rung passives (module doc): those
/// missing given, marked `RungPassive`; the game's others it wears taken.
/// What it was switching to stays wanted.
pub fn wear(world: &mut World, e: Entity, game: &str, ids: &[String]) {
    let Some(inv) = world.get::<Inventory>(e) else {
        if ids.is_empty() {
            return;
        }
        world.entity_mut(e).insert(Inventory::default());
        return wear(world, e, game, ids);
    };
    let ours = |id: &str| id.split_once(':').is_some_and(|(g, _)| g == game);
    let have: Vec<(Entity, &'static str)> = inv
        .weapons
        .iter()
        .filter(|w| world.get::<RungPassive>(**w).is_some())
        .filter_map(|w| world.get::<Weapon>(*w).map(|x| (*w, x.id)))
        .filter(|(_, id)| ours(id))
        .collect();
    for (w, id) in &have {
        if !ids.iter().any(|i| i == id) {
            super::runtime::take_weapon(world, e, *w);
        }
    }
    for id in ids.iter().filter(|i| ours(i) && !have.iter().any(|(_, h)| h == i)) {
        let wanted = world.get::<Inventory>(e).and_then(|i| i.wanted);
        let Some(w) = give(world, e, id) else {
            warn!("rung passive {id}: no such weapon");
            continue;
        };
        world.entity_mut(w).insert(RungPassive);
        if let Some(mut inv) = world.get_mut::<Inventory>(e) {
            inv.wanted = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r#"
odds = 0.25
slot_names = ["primary", "secondary", "melee"]
[slots."g:shotgun"]
"*" = "secondary"
engineer = "primary"
[gun_keys]
"g:flamer" = ["attack2"]
[[passive]]
item = "g:boots"
classes = ["soldier"]
slot = "secondary"
group = "feet"
[[passive]]
item = "g:shield"
classes = ["soldier", "pyro"]
slot = "secondary"
keys = ["attack2"]
"#;

    #[test]
    fn the_gun_slot_follows_the_class_then_the_data() {
        let t = PassiveTable::parse(TABLE).unwrap();
        assert_eq!(t.gun_slot("g:shotgun", "soldier", 0), "secondary");
        assert_eq!(t.gun_slot("g:shotgun", "engineer", 0), "primary");
        assert_eq!(t.gun_slot("g:rocket", "soldier", 0), "primary");
        assert_eq!(t.gun_slot("g:x", "soldier", 7), "7");
    }

    #[test]
    fn the_conflict_rule_takes_the_guns_slot_and_keys() {
        let t = PassiveTable::parse(TABLE).unwrap();
        let all = |_: &str| true;
        fn ids<'a>(v: Vec<&'a Passive>) -> Vec<&'a str> {
            v.into_iter().map(|p| p.item.as_str()).collect()
        }
        assert_eq!(ids(t.candidates("soldier", "primary", &[], &all)), ["g:boots", "g:shield"]);
        assert!(t.candidates("soldier", "secondary", &[], &all).is_empty(), "a secondary gun: no secondary item");
        let keys = t.gun_keys("g:flamer").to_vec();
        assert!(t.candidates("pyro", "primary", &keys, &all).is_empty(), "attack2 taken");
        assert!(t.candidates("heavy", "primary", &[], &all).is_empty(), "no item of the class");
        assert!(t.candidates("soldier", "primary", &[], &|id| id != "g:boots").iter().all(|p| p.item != "g:boots"));
    }
}
