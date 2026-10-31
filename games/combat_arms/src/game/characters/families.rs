//! Body families (specs/combat_arms/characters.md 13.4, 13.7, 13.8; our
//! data: data/npc_bodies.toml): one bone tree and animation library each,
//! and how the player driver runs on it. A body record's family is the
//! one naming its body file, else the one naming its hit table
//! (`Skeleton`), else the male's (`Families::of`).
//!
//! - The male and female families are the player bodies' trees and
//!   libraries: the driver runs unchanged (814 of the 947 world NPCs).
//! - The infected (Tanker, Assassin) play the infected-hand class 39 on
//!   the `V` legs whatever is held, `VD_HIT` / `VD_HITT` for hits and
//!   `VDIE` for death (`Family::class`, `legs`, `hits`, `death`).
//! - The powered suits play the robot classes (`RobotClass`: the names
//!   per posture and direction, the right strafe on the left names,
//!   Quirk 15); a gun of no suit class plays `other_class`.
//! - The bosses play a minimal mapping (`BossNames`) on every tracker.
//!
//! Known shortcuts (docs/tech-debt.md): a gun of no suit class on the big
//! suit plays its machine gun's class (the spec proposes a class only for
//! the small suit, Open question 36); the EOD boss's `AL` pulse (its weak
//! point's third tracker) isn't played; the Shining's body has no weight
//! sets (Quirk 13), so its trackers but the main one pose nothing.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The data (data/npc_bodies.toml).
pub const DATA: &str = include_str!("../../../data/npc_bodies.toml");

/// A boss's minimal mapping (13.8): the names played on every tracker.
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BossNames {
    pub idle: String,
    #[serde(default)]
    pub walk: Option<String>,
    /// One name looping while fire is held, or start, loop and end.
    #[serde(default)]
    pub fire: Vec<String>,
    #[serde(default)]
    pub melee: Option<String>,
    #[serde(default)]
    pub hit: Option<String>,
    #[serde(default)]
    pub death: Option<String>,
}

/// One family (module doc; the data file's comment says each field).
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Family {
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub tables: Vec<usize>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub aim: Option<usize>,
    #[serde(default)]
    pub stock_rig: Option<usize>,
    #[serde(default = "yes")]
    pub wearable: bool,
    #[serde(default)]
    pub class: Option<i32>,
    #[serde(default)]
    pub legs: Option<i32>,
    #[serde(default)]
    pub classes: Vec<i32>,
    #[serde(default)]
    pub other_class: Option<i32>,
    #[serde(default)]
    pub other_class_by_record: BTreeMap<String, i32>,
    #[serde(default)]
    pub hits: Option<[String; 2]>,
    #[serde(default)]
    pub death: Option<String>,
    #[serde(default)]
    pub boss: Option<BossNames>,
}

fn yes() -> bool {
    true
}

impl Family {
    /// The upper class and leg set a body of this family (record
    /// `record`) plays for a weapon of class `class` and leg set `legs`.
    pub fn played(&self, record: &str, class: i32, legs: i32) -> (i32, i32) {
        let legs = self.legs.unwrap_or(legs);
        if let Some(c) = self.class {
            return (c, legs);
        }
        if self.classes.is_empty() || self.classes.contains(&class) {
            return (class, legs);
        }
        let by_record = self.other_class_by_record.iter().find(|(n, _)| n.eq_ignore_ascii_case(record)).map(|(_, c)| *c);
        (by_record.or(self.other_class).unwrap_or(class), legs)
    }

    /// Whether it is one of the player trees (the male or female family).
    pub fn player_tree(&self) -> bool {
        self.stock_rig.is_some()
    }
}

/// A robot class's names for one posture: five (none, F, B, L, R).
pub type Row = [String; 5];

/// Its sprint names.
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sprint {
    pub aim: String,
    pub fire: String,
    pub select: String,
    pub reload: String,
}

/// A robot class's upper names (13.7; the data file's comment says how
/// they read).
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RobotClass {
    pub aim: Row,
    pub fire: Row,
    pub select: Row,
    pub reload: Row,
    /// None: the aim names.
    #[serde(default)]
    pub alt_fire: Option<Row>,
    pub sprint: Sprint,
}

/// The run and walk forms of a name template (`{s}`, `run|walk`).
pub fn forms(template: &str) -> (String, String) {
    match template.split_once('|') {
        Some((run, walk)) => (run.to_string(), walk.to_string()),
        None => (template.replace("{s}", "R"), template.replace("{s}", "W")),
    }
}

/// The whole file.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Families {
    pub families: BTreeMap<String, Family>,
    #[serde(default)]
    pub robot_classes: BTreeMap<String, RobotClass>,
}

/// A body file's name after its folder, lower case.
fn base(file: &str) -> String {
    file.rsplit(['\\', '/']).next().unwrap_or(file).to_ascii_lowercase()
}

impl Families {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut f: Families = toml::from_str(text).map_err(|e| e.to_string())?;
        for (name, family) in &mut f.families {
            family.name = name.clone();
        }
        Ok(f)
    }

    /// The family of a body with file `file` on hit table `table`.
    pub fn of(&self, file: &str, table: usize) -> &Family {
        let b = base(file);
        self.families
            .values()
            .find(|f| f.files.iter().any(|x| x.eq_ignore_ascii_case(&b)))
            .or_else(|| self.families.values().find(|f| f.tables.contains(&table)))
            .or_else(|| self.families.get("male"))
            .expect("npc_bodies.toml: a male family")
    }

    /// Robot class `class`'s names, if it is one.
    pub fn robot(&self, class: i32) -> Option<&RobotClass> {
        self.robot_classes.get(&class.to_string())
    }
}

/// The built-in families (the built-in character data's:
/// `CharacterData::families`).
pub fn families() -> &'static Families {
    &super::records::builtin().families
}

/// `DATA` parsed (bad built-in data panics; the tests load it).
pub fn built_in() -> Families {
    Families::parse(DATA).unwrap_or_else(|e| panic!("npc_bodies.toml: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_reads() {
        let f = families();
        assert_eq!(f.of("CHARS_M_BODY\\MT_AR_BODY.ltb", 0).name, "male");
        assert_eq!(f.of("CHARS_M_BODY\\Light_W_vest.ltb", 0).name, "female");
        assert_eq!(f.of("x\\GXL-4209.ltb", 7).name, "gxl_4209");
        assert_eq!(f.of("x\\PV_CabinFever2_EOD_Boss_KE.ltb", 7).name, "eod_boss");
        for c in [58, 59, 60, 61, 62, 63, 64, 66, 67] {
            assert!(f.robot(c).is_some(), "{c}");
        }
        for (name, fam) in &f.families {
            if let Some(b) = &fam.boss {
                assert!(matches!(b.fire.len(), 0 | 1 | 3), "{name}");
            }
        }
    }
}
