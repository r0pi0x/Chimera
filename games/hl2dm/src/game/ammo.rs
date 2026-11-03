//! HL2:DM's ammunition outside its weapons: each type's carry limit
//! (`AmmoLimits`, from the weapon data's ammo types), giving a player
//! rounds of a type (`give`: into the reserve of a carried weapon that
//! uses it, else the count waiting in `Hl2Ammo`, capped by the limit; no
//! weapon of it needed) and the boxes and crates as data
//! (data/items.toml: `ItemData`, specs/hl2dm/maps_entities.md 1, 3).

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use super::weapons::inventory::{Hl2Ammo, Hl2Item};
use chimera_sim::weapon::{Inventory, Magazine};

/// The most of each ammo type a player carries (by ammo id).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AmmoLimits(pub BTreeMap<String, u32>);

impl AmmoLimits {
    /// HL2:DM's, from its weapon data's ammo types.
    pub fn hl2() -> Self {
        Self(
            super::weapons::hl2_weapons()
                .ammo
                .into_iter()
                .map(|(id, a)| (id, a.max))
                .collect(),
        )
    }
}

/// A walk-over ammo box (data/items.toml `[[box]]`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AmmoBox {
    pub class: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub model: String,
    pub ammo: String,
    pub count: u32,
}

/// An ammo crate's type (data/items.toml `[[crate]]`, by `AmmoType`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CrateType {
    pub model: String,
    pub ammo: String,
    pub count: u32,
    /// Given first when the user has none (type 5's frag).
    #[serde(default)]
    pub weapon: Option<String>,
}

/// data/items.toml.
#[derive(Resource, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemData {
    #[serde(rename = "box")]
    pub boxes: Vec<AmmoBox>,
    #[serde(rename = "crate")]
    pub crates: Vec<CrateType>,
}

pub const ITEMS: &str = include_str!("../../data/items.toml");

impl ItemData {
    pub fn hl2() -> Self {
        toml::from_str(ITEMS).unwrap_or_else(|e| panic!("data/items.toml: {e}"))
    }

    /// The box of map class `class` (or one of its aliases).
    pub fn ammo_box(&self, class: &str) -> Option<&AmmoBox> {
        self.boxes
            .iter()
            .find(|b| b.class.eq_ignore_ascii_case(class) || b.aliases.iter().any(|a| a.eq_ignore_ascii_case(class)))
    }
}

/// How many rounds of `ammo` `player` carries (a weapon's reserve, else
/// its waiting count).
pub fn carried(world: &World, player: Entity, ammo: &str) -> u32 {
    if let Some(w) = weapon_of(world, player, ammo) {
        return world.get::<Magazine>(w).map_or(0, |m| m.reserve);
    }
    world
        .get::<Hl2Ammo>(player)
        .and_then(|a| a.0.iter().find(|(t, _)| t == ammo).map(|(_, n)| *n))
        .unwrap_or(0)
}

/// The carried weapon whose reserve holds `ammo`.
fn weapon_of(world: &World, player: Entity, ammo: &str) -> Option<Entity> {
    let inv = world.get::<Inventory>(player)?;
    inv.weapons
        .iter()
        .copied()
        .find(|w| world.get::<Hl2Item>(*w).and_then(|i| i.ammo) == Some(ammo) && world.get::<Magazine>(*w).is_some())
}

/// Give `player` up to `count` rounds of `ammo`, capped by its limit: the
/// rounds added (0: full, or a type HL2:DM lacks).
pub fn give(world: &mut World, player: Entity, ammo: &str, count: u32) -> u32 {
    let Some(limit) = world.get_resource::<AmmoLimits>().and_then(|l| l.0.get(ammo).copied()) else {
        return 0;
    };
    if let Some(w) = weapon_of(world, player, ammo) {
        let Some(mut m) = world.get_mut::<Magazine>(w) else { return 0 };
        let max = m.reserve_max.min(limit).max(m.reserve);
        let add = count.min(max - m.reserve);
        m.reserve += add;
        return add;
    }
    let Ok(mut e) = world.get_entity_mut(player) else { return 0 };
    if e.get::<Hl2Ammo>().is_none() {
        e.insert(Hl2Ammo::default());
    }
    let mut pools = e.get_mut::<Hl2Ammo>().unwrap();
    let i = match pools.0.iter().position(|(t, _)| t == ammo) {
        Some(i) => i,
        None => {
            pools.0.push((ammo.to_string(), 0));
            pools.0.len() - 1
        }
    };
    let have = pools.0[i].1;
    let add = count.min(limit.saturating_sub(have));
    pools.0[i].1 += add;
    add
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_item_data_loads() {
        let d = ItemData::hl2();
        assert_eq!(d.boxes.len(), 16);
        assert_eq!(d.crates.len(), 10);
        assert_eq!(d.ammo_box("item_box_srounds").map(|b| b.count), Some(20));
        assert_eq!(d.ammo_box("ITEM_AMMO_SMG1_LARGE").map(|b| b.count), Some(225));
        let limits = AmmoLimits::hl2();
        for b in &d.boxes {
            assert_eq!(limits.0.contains_key(&b.ammo), !b.ammo.contains("flare") && !b.ammo.contains("sniper"), "{}", b.class);
        }
        for c in &d.crates {
            assert!(limits.0.contains_key(&c.ammo), "{}", c.ammo);
        }
    }
}
