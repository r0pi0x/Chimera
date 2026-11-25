//! How bullets pass things, as data (docs/weapon-data.md, "Penetration
//! by material"): a game's document gives `PassMaterials`, the scale and
//! damage factor per surface material class (the letter a map's surface
//! names, `MapSurface::game_material`, which comes from the game's
//! install), for classes not listed, and for characters. Each ammo type's
//! power and reach are its weapon data's (`[ammo."<id>"]`).
//!
//! ```toml
//! units = { length = 0.0254 }     # metres per length unit (default 1)
//! [[material]]
//! class = "W"                     # one letter
//! scale = 2.0                     # material passed per unit of power
//! damage = 0.6                    # damage carried on, as a factor
//! [default]                       # other classes (default 1.0, 0.5)
//! scale = 1.0
//! damage = 0.5
//! [character]                     # (default: never passed)
//! cost = 21.5                     # power one character uses, length units
//! damage = 0.5
//! range_scale = 0.5               # range left beyond its exit, as a factor
//! [entity]                        # (default: never)
//! twice_from = 24.0               # an entity passed this far or farther
//!                                 # scales the damage twice, length units
//! ```
//!
//! Unknown keys, a class that isn't one letter and a class given twice
//! are load errors.
//!
//! GoldSrc's rule (`PowerRules`, `PowerPenetration`; specs/cs16/damage.md,
//! "Wall penetration") is a document of its own (`PowerRules::parse`):
//!
//! ```toml
//! units = { length = 0.0254 }
//! start_factor = 0.5              # the damage factor until a material sets one
//! wall_distance = 0.5             # the range left after a wall, as a factor
//! [character]                     # after a character or the last hit
//! power = 42.0                    # the power from then on, length units
//! factor = 0.75                   # the damage factor
//! distance = 0.75                 # the range left, as a factor
//! [[material]]
//! class = "M"                     # one letter
//! names = ["metal"]               # surface names that stand for it
//! power = 0.15                    # the power's scale
//! factor = 0.2                    # the damage factor it sets (none: kept)
//! ```

use serde::Deserialize;

use super::{CharacterPass, PassMaterial, PassMaterials, PowerMaterial, PowerRules};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PassDoc {
    #[serde(default)]
    units: Option<LengthUnit>,
    #[serde(default)]
    material: Vec<MaterialData>,
    #[serde(default)]
    default: Option<FactorData>,
    #[serde(default)]
    character: Option<CharacterData>,
    #[serde(default)]
    entity: Option<EntityData>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityData {
    twice_from: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LengthUnit {
    length: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialData {
    class: String,
    scale: f32,
    damage: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactorData {
    scale: f32,
    damage: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterData {
    cost: f32,
    damage: f32,
    range_scale: f32,
}

impl PassMaterials {
    /// Parse a penetration document (`name` for errors).
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let doc: PassDoc = toml::from_str(text).map_err(|e| format!("{name}: {e}"))?;
        let len = doc.units.map_or(1.0, |u| u.length);
        let mut out = Self::default();
        for m in &doc.material {
            let mut chars = m.class.chars();
            let (Some(class), None) = (chars.next(), chars.next()) else {
                return Err(format!("{name}: material class {:?} isn't one letter", m.class));
            };
            if out.by_class.iter().any(|(c, _)| *c == class) {
                return Err(format!("{name}: material class {class} given twice"));
            }
            out.by_class.push((
                class,
                PassMaterial {
                    scale: m.scale,
                    damage: m.damage,
                },
            ));
        }
        if let Some(d) = doc.default {
            out.default = PassMaterial {
                scale: d.scale,
                damage: d.damage,
            };
        }
        if let Some(c) = doc.character {
            out.character = CharacterPass {
                cost: c.cost * len,
                damage: c.damage,
                range_scale: c.range_scale,
            };
        }
        if let Some(e) = doc.entity {
            out.entity_twice_from = e.twice_from * len;
        }
        Ok(out)
    }
}

/// A power rule's document (`PowerRules::parse`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PowerDoc {
    #[serde(default)]
    units: Option<LengthUnit>,
    start_factor: f32,
    wall_distance: f32,
    character: PowerCharacterData,
    #[serde(default)]
    material: Vec<PowerMaterialData>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PowerCharacterData {
    power: f32,
    factor: f32,
    distance: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PowerMaterialData {
    class: String,
    #[serde(default)]
    names: Vec<String>,
    power: f32,
    #[serde(default)]
    factor: Option<f32>,
}

impl PowerRules {
    /// Parse a power rule's document (`name` for errors): docs/weapon-data.md,
    /// "Penetration by power".
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let doc: PowerDoc = toml::from_str(text).map_err(|e| format!("{name}: {e}"))?;
        let len = doc.units.map_or(1.0, |u| u.length);
        let mut out = Self {
            materials: Vec::new(),
            names: Vec::new(),
            character_power: doc.character.power * len,
            character_factor: doc.character.factor,
            character_distance: doc.character.distance,
            wall_distance: doc.wall_distance,
            start_factor: doc.start_factor,
            unit: len,
        };
        for m in &doc.material {
            let mut chars = m.class.chars();
            let (Some(class), None) = (chars.next(), chars.next()) else {
                return Err(format!("{name}: material class {:?} isn't one letter", m.class));
            };
            if out.materials.iter().any(|x| x.class == class) {
                return Err(format!("{name}: material class {class} given twice"));
            }
            out.materials.push(PowerMaterial { class, power: m.power, factor: m.factor });
            out.names.extend(m.names.iter().map(|n| (n.to_ascii_lowercase(), class)));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_gives_the_table_in_its_units() {
        let m = PassMaterials::parse(
            "p.toml",
            "units = { length = 0.5 }\n[[material]]\nclass = \"W\"\nscale = 2.0\ndamage = 0.6\n\
             [character]\ncost = 10.0\ndamage = 0.5\nrange_scale = 0.5",
        )
        .unwrap();
        assert_eq!(
            m.get(Some('W')),
            PassMaterial {
                scale: 2.0,
                damage: 0.6
            }
        );
        assert_eq!(m.get(Some('X')), PassMaterials::default().default);
        assert_eq!(m.character.cost, 5.0);
        for (text, why) in [
            ("[[material]]\nclass = \"WD\"\nscale = 1.0\ndamage = 1.0", "one letter"),
            (
                "[[material]]\nclass = \"W\"\nscale = 1.0\ndamage = 1.0\n[[material]]\nclass = \"W\"\nscale = 1.0\ndamage = 1.0",
                "twice",
            ),
            ("[default]\nscale = 1.0\ndamage = 1.0\nhardness = 2", "unknown field"),
        ] {
            let e = PassMaterials::parse("p.toml", text).unwrap_err();
            assert!(e.starts_with("p.toml: ") && e.contains(why), "{e}");
        }
    }

    #[test]
    fn a_power_document_gives_its_rules_in_its_units() {
        let r = PowerRules::parse(
            "p.toml",
            "units = { length = 0.5 }\nstart_factor = 0.5\nwall_distance = 0.5\n\
             [character]\npower = 42.0\nfactor = 0.75\ndistance = 0.75\n\
             [[material]]\nclass = \"M\"\nnames = [\"Metal\"]\npower = 0.15\nfactor = 0.2\n\
             [[material]]\nclass = \"C\"\npower = 0.25",
        )
        .unwrap();
        assert_eq!(r.character_power, 21.0);
        assert_eq!(r.get(Some('M')), Some(&PowerMaterial { class: 'M', power: 0.15, factor: Some(0.2) }));
        assert_eq!(r.get(Some('C')).and_then(|m| m.factor), None, "the factor stays");
        assert_eq!(r.get(Some('X')), None);
        assert_eq!(r.class_of("metal"), Some('M'));
        assert!(PowerRules::parse("p.toml", "start_factor = 0.5").unwrap_err().starts_with("p.toml: "));
    }
}
