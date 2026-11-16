//! The cross-game balance table (`Balance`): what each game's weapons deal
//! to other games' bodies (`outgoing`), what each game's bodies take from
//! other games' weapons (`incoming`), a body's own incoming (by the body
//! id the sandbox uses, `halo_ce:elite`, `tf2:heavy`), and the per-pair
//! exceptions; the host's switch (`on`: off is a "raw" match, every
//! weapon and body with its own game's numbers). An app made of several
//! games puts it in (the party apps: party_kit's data/party_balance.toml
//! and its `mp_balance`); without it nothing is scaled. The engine knows
//! no game here: games are the table's keys.
//!
//! **A hit's scale** (`Balance::pair`): with the switch off, 1; a pair
//! exception, its value; the same game on both sides, 1; else the
//! attacker's outgoing times the victim body's incoming (its own entry,
//! else its game's).
//!
//! **Where it is applied: one place.** `HitArmor::game_scale` (every
//! weapon hit through the shared armour step: rounds, swings, blasts,
//! projectiles, and a game's own seam naming its game, Halo's) scales the
//! hit before armour by `Balance::before_armour`: the pair's scale over
//! what the victim's body already holds (`core::DamageScale`, the
//! incoming a body keeps as its mechanism: Halo's shield counts it in its
//! armour unit and its health in `core::apply_damage`, so a hit that
//! takes armour in a game's own step, Combat Arms', counts it too). The
//! product is the pair's scale, once. Halo against Halo never comes here
//! (Halo's seam, written divided by the body's scale), so it is never
//! changed. The map's damage (`HitArmor::apply_environment`) is no
//! game's.
//!
//! Known shortcuts:
//! - Combat Arms' guns take armour in their own step
//!   (`combat_arms::game::weapons::armour_and_zone`), which reads no table:
//!   their outgoing, a pair exception with them, and a non-Halo body's own
//!   incoming against them count 1 (a Halo body's incoming still counts:
//!   it is held by the body).
//! - A body id is known for the sandbox's targets (`minigame::targets::
//!   Target::character`) and Halo's bodies (Halo reads its own); a
//!   player's TF2 class is not looked up (its game's entry counts).
//! - The ladder's power curve counts the outgoing column as data
//!   (`power_table`), whatever the switch.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::Deserialize;

use super::GameDamage;

/// One game's row.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Sides {
    /// What its weapons deal to another game's body, times.
    pub outgoing: f32,
    /// What its bodies take from another game's weapon, times.
    pub incoming: f32,
}

impl Default for Sides {
    fn default() -> Self {
        Self { outgoing: 1.0, incoming: 1.0 }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyRow {
    incoming: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairRow {
    attacker: String,
    victim: String,
    scale: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    games: BTreeMap<String, Sides>,
    #[serde(default)]
    bodies: BTreeMap<String, BodyRow>,
    #[serde(default)]
    pair: Vec<PairRow>,
}

/// The table and the switch (module doc).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Balance {
    /// The switch in force: on, the table; off, a raw match.
    pub on: bool,
    /// The host's setting (`mp_balance`: 1 on, 0 raw), put in force when
    /// the next match starts (the app's rule: party_kit's `balance`).
    pub switch: u8,
    pub games: BTreeMap<String, Sides>,
    /// A body's own incoming, over its game's.
    pub bodies: BTreeMap<String, f32>,
    /// (attacker game, victim game): the whole scale.
    pub pairs: BTreeMap<(String, String), f32>,
}

impl Default for Balance {
    /// An empty table, on: everything 1.
    fn default() -> Self {
        Self { on: true, switch: 1, games: BTreeMap::new(), bodies: BTreeMap::new(), pairs: BTreeMap::new() }
    }
}

/// The game of an id (`cs_source` of `cs_source:weapon_ak47`), or the
/// game's own name.
pub fn game_of(id: &str) -> &str {
    id.split_once(':').map_or(id, |(g, _)| g)
}

impl Balance {
    /// The table's TOML (`[games.<game>]` outgoing and incoming,
    /// `[bodies]` `"<game>:<body>" = { incoming = k }`, `[[pair]]`
    /// attacker, victim, scale), on. Every number finite; an incoming
    /// above 0, the others at least 0.
    pub fn parse(text: &str) -> Result<Self, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        // An incoming divides (`before_armour`): above 0.
        let ok = |v: f32, incoming: bool, what: &str| {
            if v.is_finite() && (v > 0.0 || (!incoming && v == 0.0)) {
                Ok(v)
            } else {
                Err(format!("{what} = {v}: a scale is a finite number, {}", if incoming { "above 0" } else { "at least 0" }))
            }
        };
        for (g, s) in &f.games {
            ok(s.outgoing, false, &format!("games.{g}.outgoing"))?;
            ok(s.incoming, true, &format!("games.{g}.incoming"))?;
        }
        let mut bodies = BTreeMap::new();
        for (id, b) in f.bodies {
            if let Some(k) = b.incoming {
                bodies.insert(id.clone(), ok(k, true, &format!("bodies.\"{id}\".incoming"))?);
            }
        }
        let mut pairs = BTreeMap::new();
        for p in f.pair {
            let k = ok(p.scale, false, &format!("pair {} on {}: scale", p.attacker, p.victim))?;
            pairs.insert((p.attacker, p.victim), k);
        }
        Ok(Self { games: f.games, bodies, pairs, ..Default::default() })
    }

    /// The same table with the switch off (a raw match), now.
    pub fn raw(mut self) -> Self {
        self.on = false;
        self.switch = 0;
        self
    }

    /// Put the host's setting in force (a match starting).
    pub fn take_switch(&mut self) {
        let on = self.switch != 0;
        if self.on != on {
            self.on = on;
        }
    }

    /// `game`'s outgoing in force (1: off, or not named).
    pub fn outgoing(&self, game: &str) -> f32 {
        if !self.on {
            return 1.0;
        }
        self.games.get(game).map_or(1.0, |s| s.outgoing)
    }

    /// The incoming in force of body `body` (its id; None: its game's
    /// own) of game `game` (None: the body id's): its own entry, else its
    /// game's (1: off, or not named).
    pub fn incoming(&self, game: Option<&str>, body: Option<&str>) -> f32 {
        if !self.on {
            return 1.0;
        }
        if let Some(k) = body.and_then(|b| self.bodies.get(b)) {
            return *k;
        }
        game.or(body.map(game_of)).and_then(|g| self.games.get(g)).map_or(1.0, |s| s.incoming)
    }

    /// The scale of a hit by game `attacker`'s weapon on a body of game
    /// `victim` (None: unknown, another game's) and id `body` (module doc).
    pub fn pair(&self, attacker: &str, victim: Option<&str>, body: Option<&str>) -> f32 {
        if !self.on {
            return 1.0;
        }
        let victim = victim.or(body.map(game_of));
        if let Some(v) = victim {
            if let Some(k) = self.pairs.get(&(attacker.to_string(), v.to_string())) {
                return *k;
            }
            if v == attacker {
                return 1.0;
            }
        }
        self.outgoing(attacker) * self.incoming(victim, body)
    }

    /// What `HitArmor` scales a hit by before armour: `pair` over the
    /// incoming the victim's body holds (`held`, its `core::DamageScale`;
    /// 1 without).
    pub fn before_armour(&self, attacker: &str, victim: Option<&str>, body: Option<&str>, held: f32) -> f32 {
        let k = self.pair(attacker, victim, body);
        if held > 0.0 && held != 1.0 { k / held } else { k }
    }

    /// The outgoing column as the power curve counts it
    /// (`minigame::power::PowerReaders::game_damage`): the table's, the
    /// switch aside (module doc).
    pub fn power_table(&self) -> GameDamage {
        GameDamage(self.games.iter().filter(|(_, s)| s.outgoing != 1.0).map(|(g, s)| (g.clone(), s.outgoing)).collect())
    }

    /// The switch for a status line.
    pub fn status_line(&self) -> String {
        let now = if self.on { "on (the cross-game table)" } else { "off (raw: each game's own numbers)" };
        if (self.switch != 0) != self.on {
            format!("{now}; mp_balance {} from the next match", self.switch)
        } else {
            now.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str = "[games.a]\noutgoing = 0.5\nincoming = 2.0\n[games.b]\noutgoing = 3.0\n\
                     [bodies]\n\"a:big\" = { incoming = 4.0 }\n\"a:plain\" = {}\n";

    #[test]
    fn a_body_s_own_incoming_over_its_game_s() {
        let t = Balance::parse(T).unwrap();
        assert_eq!(t.incoming(Some("a"), None), 2.0);
        assert_eq!(t.incoming(None, Some("a:big")), 4.0);
        assert_eq!(t.incoming(Some("a"), Some("a:plain")), 2.0, "no number: its game's");
        assert_eq!(t.pair("b", None, Some("a:big")), 12.0);
        assert_eq!(t.pair("b", Some("a"), None), 6.0);
        assert_eq!(t.pair("a", Some("a"), Some("a:big")), 1.0, "the same game");
        // What the armour step scales by when the body holds its incoming.
        assert_eq!(t.before_armour("b", Some("a"), None, 2.0), 3.0);
        let raw = t.clone().raw();
        assert_eq!(raw.pair("b", None, Some("a:big")), 1.0);
        assert!(raw.status_line().starts_with("off"));
        let mut next = t.clone();
        next.switch = 0;
        assert!(next.status_line().contains("mp_balance 0 from the next match"));
        next.take_switch();
        assert!(!next.on && next.pair("b", Some("a"), None) == 1.0);
        assert_eq!(t.power_table().0.len(), 2, "the outgoing column, whatever the switch");
    }
}
