//! Combat Arms' gun families (docs/plans/active/guns-picker-v2.md, step 1):
//! the install's weapons that are one gun in hand, its skins, editions and
//! levels (about 2,000 records of some 500 guns), for the host's Guns rules
//! (`chimera_sim::minigame::auto::GunRules`: a family is banned, allowed and
//! drawn as one, its skin drawn at the deal).
//!
//! The signal is the record's first-person model (`PVModelNormal`), not its
//! name: records drawing the same model are one gun. The family id is
//! `combat_arms:<the model's file stem, lower case>`, stable across installs
//! and builds; a family whose members name different classes splits by
//! class (`<stem>.<class>`), so a family never spans two classes. A record
//! without a model is its own family (no `family` key).
//!
//! Made by `merge_weapon_docs` with the documents (`family` in each), part
//! of the converter's sources (games/combat_arms/build.rs), so the data is
//! generated and versioned with it.

use toml::{Table, Value};

use super::slug;

/// The family stem of a first-person model path (`Models\Weapons\PV\AK47.ltb`
/// is `ak47`); None for no model.
pub fn model_stem(pv_model: &str) -> Option<String> {
    let file = pv_model.rsplit(['/', '\\']).next()?.trim();
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    (!stem.is_empty()).then(|| slug(stem))
}

/// Sets `family` in each document of `docs` (each with its record's
/// first-person model, None: none): `combat_arms:<stem>`, or
/// `combat_arms:<stem>.<class>` when the model's documents name more than
/// one class.
pub fn assign(docs: &mut [(&mut Table, Option<String>)]) {
    use std::collections::{BTreeMap, BTreeSet};
    let class = |t: &Table| t.get("class").and_then(Value::as_str).unwrap_or("none").to_string();
    let mut classes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (t, stem) in docs.iter() {
        if let Some(s) = stem {
            classes.entry(s.clone()).or_default().insert(class(t));
        }
    }
    for (t, stem) in docs.iter_mut() {
        let Some(s) = stem else { continue };
        let split = classes.get(s).is_some_and(|c| c.len() > 1);
        let id = if split { format!("{}{s}.{}", super::NS, class(t)) } else { format!("{}{s}", super::NS) };
        t.insert("family".into(), Value::String(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(class: Option<&str>) -> Table {
        let mut t = Table::new();
        if let Some(c) = class {
            t.insert("class".into(), Value::String(c.into()));
        }
        t
    }

    #[test]
    fn a_model_path_gives_its_lower_case_stem() {
        assert_eq!(model_stem("Models\\Weapons\\PV\\AK47_Gold.ltb").as_deref(), Some("ak47_gold"));
        assert_eq!(model_stem("models/pv/m4a1.LTB").as_deref(), Some("m4a1"));
        assert_eq!(model_stem(""), None);
    }

    #[test]
    fn records_of_one_model_are_one_family_split_only_by_class() {
        let mut docs = vec![
            (doc(Some("rifle")), Some("ak47".to_string())),
            (doc(Some("rifle")), Some("ak47".to_string())),
            (doc(Some("pistol")), Some("m9".to_string())),
            (doc(Some("smg")), Some("m9".to_string())),
            (doc(Some("rifle")), None),
        ];
        let mut refs: Vec<(&mut Table, Option<String>)> = docs.iter_mut().map(|(t, s)| (t, s.clone())).collect();
        assign(&mut refs);
        let fam: Vec<Option<&str>> = docs.iter().map(|(t, _)| t.get("family").and_then(Value::as_str)).collect();
        assert_eq!(
            fam,
            [
                Some("combat_arms:ak47"),
                Some("combat_arms:ak47"),
                Some("combat_arms:m9.pistol"),
                Some("combat_arms:m9.smg"),
                None
            ]
        );
    }
}
