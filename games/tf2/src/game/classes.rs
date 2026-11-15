//! TF2's nine classes as data (games/tf2/data/classes.toml, built in;
//! specs/tf2/classes.md and movement.md): each class's index, max health,
//! top speed, standing eye height, air jumps, player model and arms, and
//! which class each gun makes its holder (`class_of_gun`: the file's
//! `[guns]` table, else the gun's `tf2:gun` hook's `arms`). A player
//! holding a TF2 gun is that gun's class (`Tf2Class`): its speed, eyes
//! and air jumps (`super::movement`), its health (`super::movement::
//! class_health`), its body (`super::bodies`) and its HUD's numbers.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

/// The class table and the guns' classes, as written.
pub const DATA: &str = include_str!("../../data/classes.toml");

/// One class (classes.md, "The class table").
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Class {
    /// The game's class index (1 Scout ... 9 Engineer).
    pub index: u8,
    /// Max health, points.
    pub health: f32,
    /// Top speed, units/s.
    pub speed: f32,
    /// Standing eye height, units.
    pub eye: f32,
    #[serde(default)]
    pub air_jumps: u8,
    pub model: String,
    /// The first-person arms' class (`c_<arms>_arms.mdl`).
    pub arms: String,
    /// Its name (the table's key), filled in after reading.
    #[serde(skip)]
    pub name: String,
}

#[derive(Deserialize)]
struct Doc {
    classes: BTreeMap<String, Class>,
    #[serde(default)]
    guns: BTreeMap<String, String>,
}

/// The classes and the guns' classes: a resource of TF2's movement
/// (`super::movement::add_movement` inserts the built-in table).
#[derive(Resource, Clone)]
pub struct Classes {
    /// By index order.
    pub classes: Vec<Class>,
    guns: BTreeMap<String, String>,
    arms: BTreeMap<String, String>,
}

impl Classes {
    /// Parse `text` (the format of `DATA`), with the guns' `arms` from
    /// TF2's weapon data for guns the table leaves out.
    pub fn parse(text: &str) -> Result<Classes, String> {
        let doc: Doc = toml::from_str(text).map_err(|e| format!("classes.toml: {e}"))?;
        let mut classes: Vec<Class> = doc
            .classes
            .into_iter()
            .map(|(name, mut c)| {
                c.name = name;
                c
            })
            .collect();
        classes.sort_by_key(|c| c.index);
        for (gun, class) in &doc.guns {
            if !classes.iter().any(|c| &c.name == class) {
                return Err(format!("classes.toml: {gun} names no class {class}"));
            }
        }
        let arms = super::weapons::tf2_weapons()
            .weapons
            .into_iter()
            .filter_map(|w| {
                let arms = w.hooks.get(super::weapons::GUN_HOOK)?.get("arms")?.as_str()?.to_string();
                Some((w.id, arms))
            })
            .collect();
        Ok(Classes { classes, guns: doc.guns, arms })
    }

    /// The class named `name` (`scout`).
    pub fn get(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.name == name)
    }

    /// The class of index `index`.
    pub fn by_index(&self, index: u8) -> Option<&Class> {
        self.classes.iter().find(|c| c.index == index)
    }

    /// The class gun `id` makes its holder: the `[guns]` table's, else
    /// the class whose arms hold it.
    pub fn of_gun(&self, id: &str) -> Option<&Class> {
        if let Some(c) = self.guns.get(id) {
            return self.get(c);
        }
        let arms = self.arms.get(id)?;
        self.classes.iter().find(|c| &c.arms == arms || &c.name == arms)
    }
}

/// The built-in table, parsed (a fresh copy each call: systems read the
/// `Classes` resource).
pub fn classes() -> Classes {
    Classes::parse(DATA).unwrap_or_else(|e| panic!("{e}"))
}

impl Default for Classes {
    fn default() -> Self {
        classes()
    }
}

/// The class a player without a TF2 gun in hand is until it holds one
/// (the Soldier).
pub const DEFAULT: &str = "soldier";

/// A TF2 player's class (its index), from the gun in hand
/// (`super::movement::select_class`, predicted: every machine works it out
/// from the same inventory). The default is the Soldier (index 3).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tf2Class(pub u8);

impl Tf2Class {
    /// Its row of `table` (the default class's when the index has none).
    pub fn class<'a>(&self, table: &'a Classes) -> &'a Class {
        table
            .by_index(self.0)
            .or_else(|| table.get(DEFAULT))
            .unwrap_or_else(|| table.classes.first().expect("a class"))
    }
}

impl Default for Tf2Class {
    fn default() -> Self {
        Tf2Class(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is classes.md's (test case 1) and every gun the brief
    /// names has its class.
    #[test]
    fn the_table_is_the_specs() {
        let c = classes();
        let row = |n: &str| {
            let c = c.get(n).unwrap();
            (c.health, c.speed, c.eye)
        };
        assert_eq!(row("scout"), (125.0, 400.0, 65.0));
        assert_eq!(row("soldier"), (200.0, 240.0, 68.0));
        assert_eq!(row("pyro"), (175.0, 300.0, 68.0));
        assert_eq!(row("demoman"), (175.0, 280.0, 68.0));
        assert_eq!(row("heavy"), (300.0, 230.0, 75.0));
        assert_eq!(row("engineer"), (125.0, 300.0, 68.0));
        assert_eq!(row("medic"), (150.0, 320.0, 75.0));
        assert_eq!(row("sniper"), (125.0, 300.0, 75.0));
        assert_eq!(row("spy"), (125.0, 320.0, 75.0));
        assert_eq!(c.classes.iter().map(|c| c.index).collect::<Vec<_>>(), (1..=9).collect::<Vec<_>>());
        let of = |g: &str| c.of_gun(g).map(|c| c.name.as_str());
        assert_eq!(of("tf2:tf_weapon_scattergun"), Some("scout"));
        assert_eq!(of("tf2:tf_weapon_rocketlauncher"), Some("soldier"));
        assert_eq!(of("tf2:tf_weapon_minigun"), Some("heavy"));
        assert_eq!(of("tf2:tf_weapon_shotgun_primary"), Some("engineer"));
        assert_eq!(of("tf2:tf_weapon_pistol"), Some("scout"));
        assert_eq!(of("tf2:tf_weapon_sniperrifle"), Some("sniper"));
        assert_eq!(of("tf2:tf_weapon_flamethrower"), Some("pyro"));
        assert_eq!(of("cs_source:weapon_ak47"), None);
        assert_eq!(c.get("scout").unwrap().air_jumps, 1);
        assert_eq!(c.get("heavy").unwrap().air_jumps, 0);
        assert_eq!(Tf2Class::default().class(&c).name, DEFAULT);
    }
}
