//! Who may wear a rung's gun (docs/DESIGN.md "Gun game rules":
//! characters follow the gun): a game's mapping between its weapons and
//! the characters allowed to hold them, which its rung extras' draw
//! (`ladder::RungExtrasHook`) picks the rung's character from, at random
//! among those allowed, kept for the rung.
//!
//! The mapping is data, the game's own (its `rung_characters.toml`):
//!
//! ```toml
//! # What the game's data says of a weapon (the game derives it: CS:S's
//! # buy team, Combat Arms' infected leg set) -> the group allowed.
//! default = "any"            # a weapon nothing is derived for
//! [derived]
//! "team:1" = "t"
//! [groups]                   # a group's characters, in the game's own words
//! t = ["1", "2", "3", "4"]   # (ids, record names, `name*` patterns, `@sources`,
//! any = ["t", "ct"]          #  other groups; `!x` takes x out again)
//! [weapons]                  # explicit overrides, over what is derived:
//! "cs_source:weapon_ak47" = ["ct"]   # a weapon id, or a name pattern
//! ```
//!
//! A definition overrides it in turn (`[rules] rung_extras = { character
//! = { weapons = { "combat_arms:m4a1" = ["@players"] }, groups = { ... }
//! } }`, `Holders::tuned`). What a character string means is the game's
//! (`Holders::of` gives the strings with the groups expanded; the game
//! resolves the rest): only a game's hook reads its own.
//!
//! Known shortcuts: patterns are `*` wildcards only, matched without case.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::ladder::RungOptions;

/// The option key a definition tunes a game's mapping under (the rung
/// extras' `character`).
pub const CHARACTER_KEY: &str = "character";

/// How deep groups may name groups.
const DEPTH: usize = 8;

/// A game's weapons-to-characters mapping (module doc).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Holders {
    /// The group of a weapon nothing is derived for and no override names.
    #[serde(default)]
    pub default: Option<String>,
    /// A derived class (the game's own key) -> its group.
    #[serde(default)]
    pub derived: BTreeMap<String, String>,
    /// Groups by name: characters, other groups, `!x` exclusions.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
    /// Overrides by weapon id or name pattern: groups or characters.
    #[serde(default)]
    pub weapons: BTreeMap<String, Vec<String>>,
}

impl Holders {
    /// `text` (a game's `rung_characters.toml`).
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// This with a definition's overrides on top (`rung_extras = {
    /// character = { weapons = .., groups = .., derived = .., default = .. } }`):
    /// each key it names replaces this one's. A table that doesn't read:
    /// logged, this unchanged.
    pub fn tuned(&self, options: &RungOptions) -> Self {
        let Some(t) = options.table(CHARACTER_KEY) else { return self.clone() };
        let theirs: Holders = match toml::Value::Table(t.clone()).try_into() {
            Ok(h) => h,
            Err(e) => {
                bevy::log::warn!("[rules] rung_extras.character: {e}; the game's mapping");
                return self.clone();
            }
        };
        let mut out = self.clone();
        if theirs.default.is_some() {
            out.default = theirs.default;
        }
        out.derived.extend(theirs.derived);
        out.groups.extend(theirs.groups);
        out.weapons.extend(theirs.weapons);
        out
    }

    /// The override naming `weapon` (its full id, or its name after the
    /// game's prefix; exact before a pattern; without case), if any.
    pub fn override_for(&self, weapon: &str) -> Option<&[String]> {
        let name = weapon.split_once(':').map_or(weapon, |(_, n)| n);
        let exact = self.weapons.iter().find(|(k, _)| k.eq_ignore_ascii_case(weapon) || k.eq_ignore_ascii_case(name));
        exact
            .or_else(|| self.weapons.iter().find(|(k, _)| k.contains('*') && (matches(k, weapon) || matches(k, name))))
            .map(|(_, v)| v.as_slice())
    }

    /// The group (or override) that decides `weapon`'s holders: its
    /// override, else the group of what the game derived for it
    /// (`derived`), else `default`.
    pub fn rule_for(&self, weapon: &str, derived: Option<&str>) -> Vec<String> {
        if let Some(o) = self.override_for(weapon) {
            return o.to_vec();
        }
        let group = derived.and_then(|d| self.derived.get(d)).or(self.default.as_ref());
        group.map(|g| vec![g.clone()]).unwrap_or_default()
    }

    /// The characters allowed to hold `weapon` (the game's own strings,
    /// groups expanded, in order, each once): `rule_for`'s names expanded.
    /// Empty: nothing says (the game keeps its default look).
    pub fn of(&self, weapon: &str, derived: Option<&str>) -> Vec<String> {
        self.expand(&self.rule_for(weapon, derived))
    }

    /// `names` with every group expanded (`!x` removing x, or x's
    /// characters, from what came before).
    pub fn expand(&self, names: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        self.expand_into(names, &mut out, 0);
        out
    }

    fn expand_into(&self, names: &[String], out: &mut Vec<String>, depth: usize) {
        for n in names {
            if let Some(gone) = n.strip_prefix('!') {
                let mut drop = Vec::new();
                self.expand_into(&[gone.to_string()], &mut drop, depth + 1);
                out.retain(|c| !drop.iter().any(|d| d.eq_ignore_ascii_case(c)));
                continue;
            }
            match self.groups.get(n.as_str()) {
                Some(g) if depth < DEPTH => self.expand_into(g, out, depth + 1),
                Some(_) => bevy::log::warn!("rung characters: group `{n}` nests too deep"),
                None => {
                    if !out.iter().any(|c| c.eq_ignore_ascii_case(n)) {
                        out.push(n.clone());
                    }
                }
            }
        }
    }
}

/// Whether `text` matches `pattern` (`*` any run of characters), without
/// case.
pub fn matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    // Iterative wildcard match.
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && p[pi] != '*' && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"
default = "any"
[derived]
"team:1" = "t"
"team:2" = "ct"
[groups]
t = ["1", "2"]
ct = ["3", "4"]
any = ["t", "ct"]
[weapons]
"x:special_*" = ["4"]
"x:odd" = ["any", "!3"]
"#;

    #[test]
    fn derived_groups_overrides_and_patterns() {
        let h = Holders::parse(DATA).unwrap();
        assert_eq!(h.of("x:ak", Some("team:1")), ["1", "2"]);
        assert_eq!(h.of("x:m4", Some("team:2")), ["3", "4"]);
        assert_eq!(h.of("x:deagle", None), ["1", "2", "3", "4"], "nothing derived: the default");
        assert_eq!(h.of("x:deagle", Some("team:9")), ["1", "2", "3", "4"], "an unknown class: the default");
        assert_eq!(h.of("x:special_gun", Some("team:1")), ["4"], "an override over what is derived");
        assert_eq!(h.of("x:SPECIAL_GUN", None), ["4"], "without case");
        assert_eq!(h.of("x:odd", None), ["1", "2", "4"], "an exclusion");
        assert!(matches("powered_suit_boss_*", "Powered_Suit_Boss_Launcher"));
        assert!(!matches("boss_*", "powered_suit_boss_x"));
        assert!(matches("*boss*", "powered_suit_boss_x"));
    }

    #[test]
    fn a_definition_tunes_the_mapping() {
        let h = Holders::parse(DATA).unwrap();
        let t: toml::Table = toml::from_str("weapons = { \"x:ak\" = [\"ct\"] }\ngroups = { t = [\"9\"] }").unwrap();
        let mut o = RungOptions::default();
        o.keys.insert(CHARACTER_KEY.into(), true);
        o.tables.insert(CHARACTER_KEY.into(), t);
        let tuned = h.tuned(&o);
        assert_eq!(tuned.of("x:ak", Some("team:1")), ["3", "4"]);
        assert_eq!(tuned.of("x:galil", Some("team:1")), ["9"]);
        assert_eq!(h.tuned(&RungOptions::default()), h, "no table: unchanged");
    }
}
