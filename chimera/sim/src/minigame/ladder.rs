//! `chimera:ladder`, the weapon ladder (Gun Game; Combat Arms' Arms Race
//! is its first user in shape, architecture-review.md D2): every player a
//! team of their own, each on a rung of the definition's ladder of weapons
//! (guns, or a grenade), holding that rung's weapon (with the definition's
//! kit). An enemy kill (any weapon's, the grenade's blast too)
//! moves the killer a rung up, its weapon swapped at once; a kill on the
//! last rung wins the match. With `demote_on = "melee_death"` a player
//! killed by an enemy's melee swing goes a rung down (not below the first):
//! melee damage, or any damage from a melee weapon (CS:S's knife deals
//! bullet damage); the knifer still climbs, so a knife kill steals a rung.
//! A death of one's own doing costs a rung too (`demote_on_suicide`, on
//! by default; never below the first): a suicide (one's own grenade or
//! blast, `kill`) or a death with no killer (a fall, the world), so `kill`
//! is no free way out of a bad spawn. It is no kill for anyone: the
//! announcer's streaks count enemy kills only (`announce`).
//! A player who joins a live match starts on the lowest rung of the
//! others (`join_rung = "lowest"`, the default), their average rounded
//! down (`"average"`) or the first (`"first"`), never the last rung; at a
//! match's start everyone is on the first. With `knife_finale` the
//! ladder ends with melee rungs, knife-only: after the last gun the
//! player holds only the rung's melee weapon, and the last one's kill
//! wins (off by default in the module; both gun games turn it on). There
//! are `melee_rungs` of them (`"auto"`, the default: the power curve's
//! melee band, `melee_share` of the whole ladder, 7% by default
//! (`data/power_curve.toml`, `power::melee_rungs_for`: 1 for 10 or 20
//! guns, 2 for 30), at least 1, `auto_melee_rungs`; a drawn ladder's own
//! band; or a number), each drawn
//! from the ladder's seed (`finale`) from the melee weapons of the
//! catalogue the definition allows, none twice while the pool has enough
//! and never a rung character's hands (`RungExtrasHook::characters`);
//! `knife_finale = "<id>"` names the last rung's. With `ladder = "auto"`
//! the guns are drawn too, at each match's start, on the power curve
//! (`drawn_ladder`: `auto::ladder` over the composed games' guns the
//! definition allows, `rungs` in all, `power_curve` tuning the curve), so
//! a hand-written definition can opt in. The host's draw is replicated
//! with the match (`NetMinigame::rungs`, `melee_rungs`).
//! Combat Arms' own Arms Race (`games/combat_arms`, its rules) is not
//! this module and keeps its spec's rules (no cost for a suicide).
//!
//! **Rung extras** (`RungExtras`): when the ladder is made (each match's
//! start on the host: every map, every new ladder) each rung whose
//! weapon's game registered a hook (`RungExtrasHook`,
//! `RungExtrasAppExt::add_rung_extras`) gets that game's draw for it,
//! from the match's seeded generator (`RungSeed`, the definition's id and
//! the host's count of ladders made): the same for every player on the
//! rung, kept in `Ladder::extras` (replicated with the match,
//! `chimera_net::minigame_net`), so a late joiner, a respawn and a
//! network client get the same. What they hold is the game's (its own
//! key/value pairs, which only its hook reads), but for two things
//! Chimera does itself: a melee weapon in place of the kit's
//! (`RungExtras::melee`, `runtime::KitMelee`) and a label after the
//! rung's weapon in the HUD's rung line. A game's character for a rung
//! comes from those allowed to hold its weapon (`holders::Holders`, the
//! game's data; characters follow the gun). A player is armed for its rung
//! (`arm`: the melee, then every hook's `apply`, with its own extras or
//! none) before a life's weapons (its observer of `minigame::Arming`)
//! and at each rung swap. A rung of a game without a hook draws none. The ladder's
//! **melee band** (its last rungs whose weapon is a melee weapon: the
//! knife finale's, the power curve's melee finale) draws otherwise: such
//! a rung keeps its own weapon unless a game's draw makes it a
//! character's (below); every hook is asked in turn (`RungDraw::melee`),
//! the first with a character wins, at that game's odds (data, tuned in
//! the rules: Combat Arms' infected, `infected = { odds = 0.25 }`), but
//! one character a ladder at most.
//! `rung_extras = false`
//! turns them all off; a table turns a hook's options off by key
//! (`rung_extras = { attachments = false }`; every key is on by default;
//! a key no hook reads is warned about at the start), or tunes one with
//! a table of its own, which only its hook reads (`RungOptions::table`:
//! `attachments = { sight = 0.4 }`; on).
//! **Rung characters** (`RungExtras::character`, `RungCharacter`): a
//! game's draw may make a rung a special character's, tied to its one
//! weapon (Combat Arms' infected and their hands): on that rung a player
//! holds the character's weapon alone (no gun, no kit: `no_kit`; the HUD
//! names it as the rung's weapon, `Ladder::weapon_at`), and the next rung,
//! up or down, takes it back and gives the kit again.
//! **Arriving on a rung and leaving it** (`OnRung`, a component): arming
//! puts the player's rung and its extras on it, replacing it only when
//! they change (a rung swap, a joiner's first rung, a new ladder), so a
//! respawn on the same rung changes nothing. Games react with observers,
//! on every rung: `On<Insert<OnRung>>` (arrived), `On<Discard<OnRung>>`
//! (left: the old value still readable; a despawn too). A network client
//! puts the same component on from the replicated `Rung` and the
//! server's extras (`follow_rungs`, once they are in), so its observers
//! fire there too (`OnRung` itself isn't replicated). Combat Arms' are the
//! infection's moments.
//! The ladder gives the weapons, so nobody drops one at death
//! (`weapon::drop::NoDeathDrops`).
//! At the time limit the highest rung wins, then the most kills; a tie is
//! a draw.
//!
//! ```toml
//! [rules]
//! module = "chimera:ladder"
//! ladder = ["cs_source:weapon_glock", "combat_arms:m4a1"]   # the rungs, in order, or
//!                                  # "auto": drawn on the power curve at each match's start
//! rungs = 13                       # with "auto": rungs in all, melee too (default 13)
//! power_curve = { shares = { peak = 0.3 } }   # with "auto": over data/power_curve.toml
//! advance_on = "kill"              # the only one so far
//! demote_on = "melee_death"        # or "none" (the default)
//! demote_on_suicide = true         # a suicide, fall or `kill` costs a rung (default)
//! join_rung = "lowest"             # a late joiner's rung: "lowest" (default), "average", "first"
//! knife_finale = false             # true (drawn knives) or a weapon id (the last
//!                                  # rung's): knife-only last rungs (default: false)
//! melee_rungs = "auto"             # how many: "auto" (a share of the ladder) or a number
//! melee_share = 0.07               # the share of the ladder "auto" takes (default: the
//!                                  # power curve's melee share, 0.07; at least 1 rung)
//! refill_ammo = true               # a rung's gun never runs dry, its grenade comes
//!                                  # back once thrown (default: false)
//! rung_extras = true               # the games' per-rung draws (default: on); or by
//!                                  # option: { attachments = false, character = true },
//!                                  # or tuned: { attachments = { sight = 0.4 } }
//! win = { kind = "ladder_end" }    # the only one so far
//! ```
//!
//! Events (`MinigameEvent`): `rung_up`, `rung_down` (knifed) and
//! `suicide_down` (one's own death) with the new rung,
//! `last_rung` (a player reached the last rung), and the lead (the
//! highest rung, then the most kills; nobody leads before a kill):
//! `lead_taken` (alone in it), `lead_lost` (no longer in it),
//! `lead_tied` (newly sharing it, or a sole leader caught up), each with
//! the player's rung. The viewer's callouts (`callout`):
//! "Level up: <gun>", "Knifed! Level down: <gun>", "Suicide! Level down:
//! <gun>", "Last level!", "<name> is on
//! the last level!", "You took the lead", "You lost the lead", "Tied for
//! the lead"; the HUD's notices (`notice`) stay the small text.

use std::collections::BTreeMap;

use bevy::{
    ecs::{schedule::ScheduleConfigs, system::ScheduleSystem},
    prelude::*,
};
use serde::{Deserialize, Serialize};

use super::{
    ActiveMinigame, MinigameDef, MinigameEvent, RulesModule,
    auto::Rng,
    hud::{Callout, HudLine},
    runtime,
};
use crate::{
    core::{DamageKind, Died, Score, Team},
    mode::{Death, DeathKind, Hooks, Match, Mode, NO_HOOKS, Teams, mode_is},
    weapon::{
        Inventory, Magazine, Weapon,
        data::WeaponCatalog,
        give,
        grenade::{KeepSpent, Throwable},
    },
};

/// The module's name.
pub const LADDER: &str = "chimera:ladder";

/// The rules module.
pub const RULES: RulesModule = RulesModule {
    mode: &MODE,
    check,
    start,
    loadout,
    no_kit,
    notice,
    callout,
};

/// The mode on the runtime.
pub static MODE: Mode = Mode {
    id: LADDER,
    key: LADDER,
    shown_as: "Weapon ladder",
    description: "every player for themselves up a ladder of weapons: a kill a rung up, the last rung's kill wins",
    respawns: true,
    warmup: false,
    warmup_revives: false,
    forced_setup: Some(0.0),
    teams: Teams::FreeForAll,
    on_death,
    time_out,
    match_end: |w| Some(w.resource::<Match>().winner),
    hooks: Hooks {
        before_spawns,
        ..NO_HOOKS
    },
};

/// The `[rules]` keys.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    #[allow(dead_code)]
    module: String,
    ladder: LadderParam,
    /// With `ladder = "auto"`: rungs in all, the melee ones too.
    #[serde(default = "auto_rungs")]
    rungs: usize,
    /// With `ladder = "auto"`: the power curve over the shipped one.
    #[serde(default)]
    power_curve: Option<toml::Table>,
    #[serde(default)]
    advance_on: Advance,
    #[serde(default)]
    demote_on: Demote,
    #[serde(default)]
    win: Win,
    #[serde(default)]
    refill_ammo: bool,
    #[serde(default = "yes")]
    demote_on_suicide: bool,
    #[serde(default)]
    join_rung: JoinRung,
    #[serde(default)]
    knife_finale: Finale,
    #[serde(default)]
    melee_rungs: MeleeRungs,
    #[serde(default = "melee_share")]
    melee_share: f64,
    #[serde(default)]
    rung_extras: ExtrasParam,
}

/// `ladder`: the rungs' weapons, or `"auto"`: drawn on the power curve
/// when the match starts (`auto::ladder` over the composed games' guns
/// the definition allows).
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum LadderParam {
    List(Vec<String>),
    Auto(String),
}

/// A drawn ladder's rungs by default (`rungs`).
pub const AUTO_RUNGS: usize = 13;

fn auto_rungs() -> usize {
    AUTO_RUNGS
}

/// `melee_rungs`: how many trailing melee rungs the finale has.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum MeleeRungs {
    /// A number (at least 1).
    Count(usize),
    /// `"auto"`: the band's share (`melee_share`, 7%) of the gun rungs.
    Auto(String),
}

impl Default for MeleeRungs {
    fn default() -> Self {
        Self::Auto("auto".into())
    }
}

/// The melee rungs a ladder of `guns` gun rungs gets by default: the
/// melee band's `share` of the whole ladder, as the power curve splits it
/// (`power::melee_rungs_for`: 7%, 1 for 10 or 20 guns, 2 for 30).
pub fn auto_melee_rungs(guns: usize, share: f64) -> usize {
    super::power::melee_rungs_for(guns, share)
}

/// The melee band's share of the ladder: the power curve's
/// (`data/power_curve.toml`), the one number for both.
fn melee_share() -> f64 {
    super::power::PowerCurve::shipped().melee_share()
}

fn yes() -> bool {
    true
}

/// A late joiner's rung (`join_rung`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinRung {
    /// The lowest rung of the others.
    #[default]
    Lowest,
    /// The others' average, rounded down.
    Average,
    /// The first rung.
    First,
}

/// `rung_extras`: all on or off, or by option key (on or off, or a
/// table: on, tuned).
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum ExtrasParam {
    All(bool),
    Keys(BTreeMap<String, ExtrasKey>),
}

/// One option key's value.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum ExtrasKey {
    On(bool),
    Tuned(toml::Table),
}

impl Default for ExtrasParam {
    fn default() -> Self {
        Self::All(true)
    }
}

/// Which rung extras a definition wants (`rung_extras`): every option
/// key a hook reads is on unless turned off, none when all are off.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RungOptions {
    /// `rung_extras = false`.
    pub off: bool,
    /// Each key turned on or off (a tuned one: on).
    pub keys: BTreeMap<String, bool>,
    /// The tuned keys' tables (`{ attachments = { ... } }`), which only
    /// the hook that reads the key reads.
    pub tables: BTreeMap<String, toml::Table>,
}

impl RungOptions {
    /// Whether option `key` is on.
    pub fn on(&self, key: &str) -> bool {
        !self.off && self.keys.get(key).copied().unwrap_or(true)
    }

    /// Option `key`'s table, when the definition tunes it.
    pub fn table(&self, key: &str) -> Option<&toml::Table> {
        self.tables.get(key)
    }

    fn of(p: &ExtrasParam) -> Self {
        match p {
            ExtrasParam::All(on) => Self {
                off: !on,
                ..Self::default()
            },
            ExtrasParam::Keys(keys) => {
                let mut out = Self::default();
                for (k, v) in keys {
                    match v {
                        ExtrasKey::On(on) => {
                            out.keys.insert(k.clone(), *on);
                        }
                        ExtrasKey::Tuned(t) => {
                            out.keys.insert(k.clone(), true);
                            out.tables.insert(k.clone(), t.clone());
                        }
                    }
                }
                out
            }
        }
    }
}

/// What a game adds to one rung (its `RungExtrasHook`'s draw), the same
/// for every player on it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RungExtras {
    /// The game whose hook drew it (only its `apply` gets it).
    pub game: String,
    /// A melee weapon in place of the kit's on this rung: the kit's
    /// weapon of its slot (the knife's) becomes it (a weapon id in a set
    /// the definition allows; anything else is dropped).
    pub melee: Option<String>,
    /// The game's own values, which only its hook reads.
    pub values: Vec<(String, String)>,
    /// Shown after the rung's weapon in the HUD's rung line.
    pub label: Option<String>,
    /// A character every player on the rung becomes (`RungCharacter`):
    /// they hold its weapon alone, in place of the rung's gun.
    #[serde(default)]
    pub character: Option<RungCharacter>,
}

/// A special character a rung makes its players (a game's: Combat Arms'
/// infected), tied to its one weapon: on the rung a player holds that
/// weapon alone (no kit, no gun) and is the character; the next rung
/// (up or down) makes them themselves again. Its game plays the moments
/// of becoming and leaving it (observers of `OnRung`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RungCharacter {
    /// `game:name` (its game: the prefix).
    pub id: String,
    /// The weapon it holds (a weapon id the definition allows).
    pub weapon: String,
}

impl RungCharacter {
    /// Its game (the id's prefix).
    pub fn game(&self) -> &str {
        self.id.split_once(':').map_or(self.id.as_str(), |(g, _)| g)
    }
}

/// The rung player `e` is on as this machine knows it, with the rung's
/// extras (`RungExtras`; none drawn: none): put on by arming (`arm`, the
/// host's) or, on a network client, from the replicated `Rung` and the
/// server's extras (`follow_rungs`). Replaced only when the rung or its
/// extras change, never by a respawn on the same rung: games react to
/// arriving with `On<Insert<OnRung>>` and to leaving with
/// `On<Discard<OnRung>>` (the old value still there; a removal or a
/// despawn fires it too), on every rung.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct OnRung {
    /// The rung (0: the first).
    pub index: usize,
    /// What the rung holds.
    pub extras: Option<RungExtras>,
    /// The rung the player came from on this machine (none: the first
    /// rung this machine knew it on: a new player, a joiner, a player a
    /// client first sees), so a game can tell an arrival from a sighting.
    pub from: Option<usize>,
}

impl OnRung {
    /// The rung's character, if it makes its players one.
    pub fn character(&self) -> Option<&RungCharacter> {
        self.extras.as_ref()?.character.as_ref()
    }

    /// What `have` becomes on `ladder`'s rung `index`: none when it is
    /// that already (the rung and its extras).
    fn next(have: Option<&OnRung>, ladder: &Ladder, index: usize) -> Option<OnRung> {
        let extras = ladder.extras(index);
        if have.is_some_and(|o| o.index == index && o.extras.as_ref() == extras) {
            return None;
        }
        Some(OnRung {
            index,
            extras: extras.cloned(),
            from: have.map(|o| o.index),
        })
    }
}

impl RungExtras {
    /// Value `key`'s (the first).
    pub fn value(&self, key: &str) -> Option<&str> {
        self.values.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// A rung being drawn (`RungExtrasHook::draw`).
pub struct RungDraw<'a> {
    pub def: &'a MinigameDef,
    /// The rung (0: the first) and its weapon.
    pub rung: usize,
    pub weapon: &'static str,
    pub options: &'a RungOptions,
    /// Every rung's weapon, in order (the knife finale's last).
    pub rungs: &'a [&'static str],
    /// The extras drawn for the rungs before this one.
    pub drawn: &'a [Option<RungExtras>],
    /// The rung is in the ladder's melee band (its last rungs whose
    /// weapon is a melee weapon, the knife finale's among them): every
    /// hook is asked, whatever the weapon's game; a draw with a character
    /// makes it that character's, any other keeps the rung's own weapon
    /// (nothing drawn).
    pub melee: bool,
}

/// A game's per-rung draw for the ladder (registered with
/// `RungExtrasAppExt::add_rung_extras`): for the rungs whose weapon is
/// its own.
#[derive(Clone, Copy)]
pub struct RungExtrasHook {
    /// The game (a weapon id's prefix).
    pub game: &'static str,
    /// The option keys it reads (`rung_extras = { <key> = false }`).
    pub options: &'static [&'static str],
    /// The melee weapons of its that carry no `melee` data of the catalogue
    /// (Combat Arms' knives, a hook of its own): melee rungs all the same.
    pub melee: fn(&WeaponCatalog) -> Vec<&'static str>,
    /// The weapons of its that are rung characters' (Combat Arms' infected
    /// hands): they are no melee rung's weapon of their own, only a
    /// character's, at the odds of `draw`.
    pub characters: fn(&WeaponCatalog) -> Vec<&'static str>,
    /// The rung's extras (the host's, once per ladder), drawn on `rng`:
    /// for its own rungs, and on the melee band's (`RungDraw::melee`) a
    /// character or nothing.
    pub draw: fn(&World, &RungDraw, &mut Rng) -> Option<RungExtras>,
    /// Player `e` is armed for a rung: what this hook drew for it, or
    /// none (another game's rung, or nothing drawn: what it set comes
    /// off). The host's; before the rung's weapons are given.
    pub apply: fn(&mut World, Entity, Option<&RungExtras>),
}

/// The registered `RungExtrasHook`s.
#[derive(Resource, Clone, Default)]
pub struct RungExtrasHooks(pub Vec<RungExtrasHook>);

/// Registering a game's `RungExtrasHook`.
pub trait RungExtrasAppExt {
    /// `hook` (one per game: a second replaces the first).
    fn add_rung_extras(&mut self, hook: RungExtrasHook) -> &mut Self;
}

impl RungExtrasAppExt for App {
    fn add_rung_extras(&mut self, hook: RungExtrasHook) -> &mut Self {
        let mut hooks = self.world_mut().get_resource_or_init::<RungExtrasHooks>();
        hooks.0.retain(|h| h.game != hook.game);
        hooks.0.push(hook);
        self
    }
}

/// The seed rung extras are drawn from (an app's launch seed; none: 0,
/// the tests'). Each ladder made draws anew from it, the definition's id
/// and how many ladders this host made (`LaddersMade`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RungSeed(pub u64);

/// How many ladders this host made (its rung extras' draws).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LaddersMade(pub u64);

/// `knife_finale`: off, the kit's knife, or a named melee weapon.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum Finale {
    On(bool),
    Weapon(String),
}

impl Default for Finale {
    fn default() -> Self {
        Self::On(false)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Advance {
    #[default]
    Kill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Demote {
    #[default]
    None,
    MeleeDeath,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Win {
    #[serde(default)]
    kind: WinKind,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WinKind {
    #[default]
    LadderEnd,
}

/// The ladder playing.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Ladder {
    pub rungs: Vec<&'static str>,
    pub demote_on_melee: bool,
    /// The rungs' guns keep a full reserve (a ladder has no ammo to find).
    pub refill_ammo: bool,
    /// A suicide or a death with no killer costs a rung.
    pub demote_on_suicide: bool,
    /// A late joiner's rung.
    pub join_rung: JoinRung,
    /// The last rung is a knife alone (`knife_finale`): its weapon is the
    /// last of `rungs`, held without the kit.
    pub knife_finale: bool,
    /// How many trailing rungs are melee rungs (`melee_rungs`; 0 without
    /// the knife finale).
    pub melee_rungs: usize,
    /// Each rung's extras (`RungExtras`), one per rung (the host's, none
    /// for a rung that drew nothing); a network client's stays empty until
    /// the server's come in.
    pub extras: Vec<Option<RungExtras>>,
    /// The definition's `rung_extras`.
    pub options: RungOptions,
}

impl Ladder {
    /// Whether `id` is one of the ladder's guns (a rung's weapon, but not
    /// the knife finale's, which is a kit weapon).
    pub fn is_gun(&self, id: &str) -> bool {
        let guns = self.rungs.len().saturating_sub(self.melee_rungs);
        self.rungs[..guns].contains(&id)
    }

    /// Whether `rung` is the knife-only last rung.
    pub fn knife_only(&self, rung: usize) -> bool {
        self.melee_rungs > 0 && rung + self.melee_rungs >= self.rungs.len()
    }

    /// Rung `rung`'s extras.
    pub fn extras(&self, rung: usize) -> Option<&RungExtras> {
        self.extras.get(rung).and_then(Option::as_ref)
    }

    /// Rung `rung`'s character (`RungExtras::character`).
    pub fn character(&self, rung: usize) -> Option<&RungCharacter> {
        self.extras(rung)?.character.as_ref()
    }

    /// What a player on `rung` holds as the rung's weapon: its
    /// character's weapon (as the catalogue has it), else the rung's gun.
    pub fn weapon_at(&self, rung: usize, catalog: Option<&WeaponCatalog>) -> Option<&'static str> {
        if let Some(c) = self.character(rung) {
            return catalog?.get(&c.weapon).map(|w| w.id.as_str());
        }
        self.rungs.get(rung).copied()
    }

    /// Every rung character's weapon, as the catalogue has them.
    fn character_weapons(&self, catalog: &WeaponCatalog) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for c in self.extras.iter().flatten().filter_map(|x| x.character.as_ref()) {
            if let Some(w) = catalog.get(&c.weapon)
                && !out.contains(&w.id.as_str())
            {
                out.push(&w.id);
            }
        }
        out
    }

    /// Every rung's melee weapon (`RungExtras::melee`), as the catalogue
    /// has them.
    fn melees(&self, catalog: &WeaponCatalog) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for m in self.extras.iter().flatten().filter_map(|x| x.melee.as_deref()) {
            if let Some(w) = catalog.get(m)
                && !out.contains(&w.id.as_str())
            {
                out.push(&w.id);
            }
        }
        out
    }
}

/// A player's rung (0: the first). Replicated to clients (`minigame_net`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rung(pub usize);

fn params(def: &MinigameDef) -> Result<Params, String> {
    let mut table = def.rules.params.clone();
    table.insert("module".into(), toml::Value::String(def.rules.module.clone()));
    let p: Params = table.try_into().map_err(|e| format!("[rules]: {e}"))?;
    let (Advance::Kill, WinKind::LadderEnd) = (p.advance_on, p.win.kind);
    Ok(p)
}

fn check(def: &MinigameDef, catalog: &WeaponCatalog) -> Result<(), String> {
    let p = params(def)?;
    let guns = match &p.ladder {
        LadderParam::List(ids) => {
            if ids.is_empty() {
                return Err("[rules] ladder: no rungs".into());
            }
            for id in ids {
                super::weapon(def, catalog, id)?;
            }
            ids.len()
        }
        LadderParam::Auto(a) if a == "auto" => {
            if p.rungs < 2 {
                return Err("[rules] rungs: at least 2 for a drawn ladder".into());
            }
            curve(&p)?;
            p.rungs
        }
        LadderParam::Auto(a) => return Err(format!("[rules] ladder: `{a}` is not a list of weapons or \"auto\"")),
    };
    // The games' hooks name more melee weapons than the catalogue's data
    // does (Combat Arms' knives), so an empty pool is no error here: the
    // match starts without a finale (and says so).
    match finale(def, catalog, &[], &p, auto_melee_rungs(guns, p.melee_share.clamp(0.0, 1.0)), &mut Rng::new(0)) {
        Err(e) if e == NO_MELEE => Ok(()),
        r => r.map(|_| ()),
    }
}

/// The melee weapons a finale may draw: every melee weapon of the
/// catalogue the definition allows (CS:S's knife, Combat Arms' knife-class
/// weapons, ...), but a hook's rung characters' (`characters`: the
/// infected's hands come only as a character, at their odds), in the
/// catalogue's order.
fn melee_pool(def: &MinigameDef, catalog: &WeaponCatalog, hooks: &[RungExtrasHook]) -> Vec<&'static str> {
    let hands: Vec<&'static str> = hooks.iter().flat_map(|h| (h.characters)(catalog)).collect();
    let knives: Vec<&'static str> = hooks.iter().flat_map(|h| (h.melee)(catalog)).collect();
    let mut pool: Vec<&'static str> = Vec::new();
    for set in &catalog.sets {
        for w in &set.weapons {
            let Some(data) = catalog.get(&w.id) else { continue };
            let id: &'static str = &data.id;
            if (data.melee.is_some() || knives.contains(&id)) && def.allows_weapon(id) && !hands.contains(&id) && !pool.contains(&id) {
                pool.push(id);
            }
        }
    }
    pool
}

/// Whether weapon `id` is a melee weapon: it has `melee` data, or a hook
/// names it (`RungExtrasHook::melee`).
pub fn is_melee(catalog: &WeaponCatalog, hooks: &[RungExtrasHook], id: &str) -> bool {
    catalog.get(id).is_some_and(|w| w.melee.is_some()) || hooks.iter().any(|h| (h.melee)(catalog).contains(&id))
}

const NO_MELEE: &str = "[rules] knife_finale: the games have no melee weapon for it";

/// The definition's power curve (`power_curve` over the shipped one).
fn curve(p: &Params) -> Result<super::power::PowerCurve, String> {
    match &p.power_curve {
        None => Ok(super::power::PowerCurve::shipped()),
        Some(t) => super::power::PowerCurve::tuned(t).map_err(|e| format!("[rules] {e}")),
    }
}

/// The knife finale's melee rungs' weapons, in order (none: off): as many
/// as `melee_rungs` says (`"auto"`, the default: `auto`, the melee band's
/// count: `auto_melee_rungs` of a written ladder's guns, a drawn ladder's
/// own), each drawn on `rng` from the melee pool, none twice while the
/// pool has enough; a named weapon (`knife_finale = "<id>"`) is the last
/// rung's.
fn finale(
    def: &MinigameDef,
    catalog: &WeaponCatalog,
    hooks: &[RungExtrasHook],
    p: &Params,
    auto: usize,
    rng: &mut Rng,
) -> Result<Vec<&'static str>, String> {
    let named = match &p.knife_finale {
        Finale::On(false) => return Ok(Vec::new()),
        Finale::On(true) => None,
        Finale::Weapon(id) => {
            let id = super::weapon(def, catalog, id)?;
            if !is_melee(catalog, hooks, id) {
                return Err(format!("[rules] knife_finale: {id} is not a melee weapon"));
            }
            Some(id)
        }
    };
    let want = match &p.melee_rungs {
        MeleeRungs::Count(0) => return Err("[rules] melee_rungs: at least 1 (or \"auto\")".into()),
        MeleeRungs::Count(n) => *n,
        MeleeRungs::Auto(a) if a == "auto" => auto.max(1),
        MeleeRungs::Auto(a) => return Err(format!("[rules] melee_rungs: `{a}` is not a number or \"auto\"")),
    };
    let mut pool = melee_pool(def, catalog, hooks);
    pool.retain(|id| Some(*id) != named);
    if pool.is_empty() && named.is_none() {
        return Err(NO_MELEE.into());
    }
    let mut out: Vec<&'static str> = Vec::new();
    // Seeded shuffle, taken in turn (a pool shorter than the finale
    // repeats, in a new order).
    let mut bag: Vec<&'static str> = Vec::new();
    while out.len() < want - usize::from(named.is_some()) && !(pool.is_empty()) {
        if bag.is_empty() {
            bag = pool.clone();
            for i in (1..bag.len()).rev() {
                bag.swap(i, rng.below(i + 1));
            }
        }
        out.push(bag.pop().expect("a refilled bag"));
    }
    out.extend(named);
    Ok(out)
}

fn start(world: &mut World, def: &MinigameDef) {
    let Ok(p) = params(def) else { return };
    let catalog = world.get_resource::<WeaponCatalog>().cloned().unwrap_or_default();
    let hooks = world.get_resource::<RungExtrasHooks>().cloned().unwrap_or_default();
    // The melee rungs' weapons (and a drawn ladder's guns) are the host's
    // draw (from this ladder's seed); a network client takes the server's
    // rungs once they are in (`minigame_net`), its own draw from seed 0
    // until then.
    let client = world.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client);
    let (mut rungs, auto) = match &p.ladder {
        LadderParam::List(ids) => {
            let rungs: Vec<&'static str> = ids.iter().filter_map(|id| super::weapon(def, &catalog, id).ok()).collect();
            let auto = auto_melee_rungs(rungs.len(), p.melee_share.clamp(0.0, 1.0));
            (rungs, auto)
        }
        LadderParam::Auto(_) => {
            let seed = if client { 0 } else { ladder_rng(world, def, 0x4155_544F).next() };
            let drawn = drawn_ladder(world, def, &p, seed);
            (drawn.rungs, drawn.melee)
        }
    };
    let mut rng = ladder_rng(world, def, 0x4D45_4C45_4521);
    if client {
        rng = Rng::new(0);
    }
    let melee = finale(def, &catalog, &hooks.0, &p, auto, &mut rng).unwrap_or_else(|e| {
        warn!("{}: {e}; no finale", def.id);
        Vec::new()
    });
    let melee_rungs = melee.len();
    rungs.extend(melee);
    world.insert_resource(crate::weapon::drop::NoDeathDrops);
    let options = RungOptions::of(&p.rung_extras);
    let extras = draw_extras(world, def, &rungs, &options);
    world.insert_resource(Ladder {
        rungs,
        demote_on_melee: p.demote_on == Demote::MeleeDeath,
        refill_ammo: p.refill_ammo,
        demote_on_suicide: p.demote_on_suicide,
        join_rung: p.join_rung,
        knife_finale: melee_rungs > 0,
        melee_rungs,
        extras,
        options,
    });
}

/// `ladder = "auto"`: a ladder of `rungs` drawn on the definition's power
/// curve from `seed` (`auto::ladder`) over the composed games' guns and
/// grenades the definition allows.
fn drawn_ladder(world: &World, def: &MinigameDef, p: &Params, seed: u64) -> super::auto::Drawn {
    use super::{auto, power};
    let curve = curve(p).unwrap_or_else(|_| power::PowerCurve::shipped());
    let mut caps = auto::report(world);
    for c in &mut caps {
        c.guns.retain(|g| def.allows_weapon(g.id));
        c.grenades.retain(|g| def.allows_weapon(g));
    }
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    let readers = world.get_resource::<power::PowerReaders>().cloned().unwrap_or_default();
    let powers = auto::powers(&caps, catalog, &readers, &curve);
    let refs: Vec<&auto::Capabilities> = caps.iter().collect();
    let drawn = auto::ladder(&refs, &powers, &curve, p.rungs, seed);
    if drawn.rungs.is_empty() {
        warn!("{}: [rules] ladder = \"auto\": no game lends guns the definition allows", def.id);
    }
    drawn
}

/// The generator of the ladder about to be made: the app's seed, the
/// definition's id and the host's count of ladders (`salt` apart for each
/// use), so each ladder draws anew.
fn ladder_rng(world: &World, def: &MinigameDef, salt: u64) -> Rng {
    let made = world.get_resource::<LaddersMade>().map_or(0, |n| n.0) + 1;
    let seed = world.get_resource::<RungSeed>().map_or(0, |s| s.0);
    let id = def.id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    Rng::new(seed ^ id ^ salt ^ made.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// The rungs' extras (`RungExtras`), one per rung: each rung whose
/// weapon's game has a hook drawn by it from this ladder's seeded
/// generator; the melee band's (`melee_band`) by the first hook whose draw
/// gives it a character, else none. All none with `rung_extras = false`;
/// empty on a network client (it takes the server's).
fn draw_extras(world: &mut World, def: &MinigameDef, rungs: &[&'static str], options: &RungOptions) -> Vec<Option<RungExtras>> {
    let client = world.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client);
    if client {
        return Vec::new();
    }
    let hooks = world.get_resource::<RungExtrasHooks>().cloned().unwrap_or_default();
    if options.off || hooks.0.is_empty() {
        // Still a ladder made (the melee rungs' draw moves on).
        world.get_resource_or_init::<LaddersMade>().0 += 1;
        return vec![None; rungs.len()];
    }
    for key in options.keys.keys() {
        if !hooks.0.iter().any(|h| h.options.contains(&key.as_str())) {
            warn!("{}: [rules] rung_extras: no game reads `{key}`", def.id);
        }
    }
    let made = {
        let mut n = world.get_resource_or_init::<LaddersMade>();
        n.0 += 1;
        n.0
    };
    let seed = world.get_resource::<RungSeed>().map_or(0, |s| s.0);
    let id = def.id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let mut rng = Rng::new(seed ^ id ^ made.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let catalog = world.get_resource::<WeaponCatalog>().cloned().unwrap_or_default();
    let melee = |m: &str| super::weapon(def, &catalog, m).ok();
    let band = melee_band(rungs, &catalog, &hooks.0);
    let world: &World = world;
    let mut drawn: Vec<Option<RungExtras>> = Vec::new();
    for (rung, weapon) in rungs.iter().enumerate() {
        let in_band = rung >= band;
        let draw = RungDraw {
            def,
            rung,
            weapon,
            options,
            rungs,
            drawn: &drawn,
            melee: in_band,
        };
        let game = weapon.split_once(':').map(|(g, _)| g);
        let mut x = if in_band {
            // Every game may make it its character's (the first that does),
            // but one character a ladder at most.
            let taken = drawn.iter().flatten().any(|x| x.character.is_some());
            hooks.0.iter().filter(|_| !taken).find_map(|h| {
                let mut x = (h.draw)(world, &draw, &mut rng).filter(|x| x.character.is_some())?;
                x.game = h.game.to_string();
                Some(x)
            })
        } else {
            game.and_then(|g| hooks.0.iter().find(|h| h.game == g)).and_then(|h| {
                let mut x = (h.draw)(world, &draw, &mut rng)?;
                x.game = h.game.to_string();
                Some(x)
            })
        };
        if let Some(x) = x.as_mut() {
            x.melee = x.melee.as_deref().and_then(melee).map(str::to_string);
            // A character whose weapon the definition doesn't allow: none.
            x.character = x.character.take().filter(|c| melee(&c.weapon).is_some());
        }
        // A melee rung without its character keeps its own weapon alone.
        if in_band && x.as_ref().is_some_and(|x| x.character.is_none()) {
            x = None;
        }
        drawn.push(x);
    }
    drawn
}

/// Where the ladder's melee band starts: its last rungs whose weapon is a
/// melee weapon (the knife finale's among them; `rungs.len()`: none).
pub fn melee_band(rungs: &[&str], catalog: &WeaponCatalog, hooks: &[RungExtrasHook]) -> usize {
    let melee = |id: &&&str| is_melee(catalog, hooks, id);
    rungs.len() - rungs.iter().rev().take_while(melee).count()
}

/// A network client takes the server's rungs (the melee rungs' draw is
/// the host's, and a drawn ladder's guns, `ladder = "auto"`) and its
/// count of melee rungs: `ids` in place of the ladder's own; when the
/// lengths agree an id this catalogue lacks stays the client's, else
/// (a drawn ladder of another length) every id must be in it.
pub fn take_rungs(world: &mut World, ids: &[String], melee_rungs: usize) {
    let Some(catalog) = world.get_resource::<WeaponCatalog>().cloned() else { return };
    let Some(mut ladder) = world.get_resource_mut::<Ladder>() else { return };
    if ids.is_empty() || melee_rungs > ids.len() {
        return;
    }
    let same = ids.len() == ladder.rungs.len() && ladder.rungs.iter().zip(ids).all(|(a, b)| a == b);
    if !same {
        let taken: Option<Vec<&'static str>> = if ids.len() == ladder.rungs.len() {
            Some(ids.iter().zip(&ladder.rungs).map(|(b, a)| catalog.get(b).map_or(*a, |w| w.id.as_str())).collect())
        } else {
            ids.iter().map(|b| catalog.get(b).map(|w| w.id.as_str())).collect()
        };
        let Some(taken) = taken else { return };
        ladder.rungs = taken;
    }
    if ladder.melee_rungs != melee_rungs {
        ladder.melee_rungs = melee_rungs;
        ladder.knife_finale = melee_rungs > 0;
    }
}

/// `e` armed for `rung` (the host's, before its weapons are given): the
/// rung's melee weapon in place of the kit's (`runtime::KitMelee`), every
/// hook's `apply` with its own extras for the rung, or none, then its
/// `OnRung` (replaced only when the rung or its extras changed: the
/// games' observers of arriving and leaving fire).
pub fn arm(world: &mut World, e: Entity, ladder: &Ladder, rung: usize) {
    let extras = ladder.extras(rung).cloned();
    let melee = extras
        .as_ref()
        .and_then(|x| x.melee.as_deref())
        .and_then(|m| world.get_resource::<WeaponCatalog>()?.get(m).map(|w| w.id.as_str()));
    match melee {
        Some(m) => {
            world.entity_mut(e).insert(runtime::KitMelee(m));
        }
        None => {
            world.entity_mut(e).remove::<runtime::KitMelee>();
        }
    }
    let hooks = world.get_resource::<RungExtrasHooks>().cloned().unwrap_or_default();
    for h in &hooks.0 {
        (h.apply)(world, e, extras.as_ref().filter(|x| x.game == h.game));
    }
    if let Some(on) = OnRung::next(world.get::<OnRung>(e), ladder, rung) {
        world.entity_mut(e).insert(on);
    }
}

/// A network client's `OnRung`s (each frame, after the match's state
/// came in): every player with a replicated `Rung` gets its rung and the
/// server's extras for it, once those are in (the ladder's extras, one
/// per rung), replaced only when they change, so the games' observers
/// fire here as on the host.
pub fn follow_rungs(ladder: Option<Res<Ladder>>, players: Query<(Entity, &Rung, Option<&OnRung>)>, mut commands: Commands) {
    let Some(ladder) = ladder else { return };
    if ladder.rungs.is_empty() || ladder.extras.len() != ladder.rungs.len() {
        return;
    }
    for (e, rung, have) in &players {
        let index = rung.0.min(ladder.rungs.len() - 1);
        if let Some(on) = OnRung::next(have, &ladder, index) {
            commands.entity(e).insert(on);
        }
    }
}

/// A life's arming (`minigame::Arming`, the ladder's observer): for the
/// player's rung (`arm`: the games' hooks take the world, so queued, and
/// applied by the runtime's flush before the life's weapons).
pub(super) fn arming(arming: On<super::Arming>, mut commands: Commands) {
    if arming.rules != LADDER {
        return;
    }
    let e = arming.player;
    commands.queue(move |world: &mut World| {
        let Some(ladder) = world.get_resource::<Ladder>().cloned() else { return };
        let rung = world.get::<Rung>(e).map_or(0, |r| r.0).min(ladder.rungs.len().saturating_sub(1));
        arm(world, e, &ladder, rung);
    });
}

/// The knife-only last rung's life has no kit (the knife is its
/// weapon), nor does a character's rung (its weapon alone).
fn no_kit(world: &World, e: Entity) -> bool {
    let Some(ladder) = world.get_resource::<Ladder>() else { return false };
    let rung = world.get::<Rung>(e).map_or(0, |r| r.0);
    ladder.knife_only(rung) || ladder.character(rung).is_some()
}

/// A joiner's rung (`minigame::Joined`, the ladder's observer): in a live
/// match the others' lowest (or average, or the first: `join_rung`), never
/// the last; else the first (the match's start puts everyone there
/// anyway). Applied by the runtime's flush, before the joiner's first
/// spawn.
pub(super) fn joined(
    join: On<super::Joined>,
    ladder: Option<Res<Ladder>>,
    game: Option<Res<Match>>,
    players: Query<(Entity, &Team, Option<&Rung>), runtime::Playing>,
    mut commands: Commands,
) {
    let Some(ladder) = ladder.filter(|_| join.rules == LADDER) else { return };
    let e = join.player;
    let rung = if game.is_some_and(|m| m.counting()) {
        let others: Vec<usize> = players
            .iter()
            .filter(|(p, t, _)| *p != e && crate::mode::plays(t.0))
            .map(|(_, _, r)| r.map_or(0, |r| r.0))
            .collect();
        join_rung(&ladder, &others)
    } else {
        0
    };
    commands.entity(e).insert(Rung(rung));
}

/// The rung a joiner gets with the others on `others`.
pub fn join_rung(ladder: &Ladder, others: &[usize]) -> usize {
    let rung = match (ladder.join_rung, others.is_empty()) {
        (_, true) | (JoinRung::First, _) => 0,
        (JoinRung::Lowest, false) => others.iter().copied().min().unwrap_or(0),
        (JoinRung::Average, false) => others.iter().sum::<usize>() / others.len(),
    };
    rung.min(ladder.rungs.len().saturating_sub(2))
}

/// The rung's weapon (its character's, on a character's rung).
fn loadout(world: &World, e: Entity) -> Vec<&'static str> {
    let Some(ladder) = world.get_resource::<Ladder>() else { return Vec::new() };
    let rung = world.get::<Rung>(e).map_or(0, |r| r.0).min(ladder.rungs.len().saturating_sub(1));
    ladder.weapon_at(rung, world.get_resource::<WeaponCatalog>()).into_iter().collect()
}

/// The ladder's tick (the server's, while it is the mode playing,
/// `mode::mode_is`): the guns refilled, then the lead followed, chained
/// where the minigame runtime runs its mode's per-tick work (after `join`,
/// before the phases step). Normal systems: what they insert goes through
/// `Commands`, applied at the sync point before the step (O1).
pub(super) fn per_tick() -> ScheduleConfigs<ScheduleSystem> {
    (refill, follow_lead).chain().distributive_run_if(mode_is(LADDER))
}

/// Who leads (the players at the highest rung, then most kills), as last
/// told: `lead_taken`, `lead_lost`, `lead_tied` when it changes.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Leaders(pub Vec<Entity>);

/// The players in the minigame (`runtime::players`' filter), with their
/// rung and kills.
type Climbers<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Team, Option<&'static Rung>, Option<&'static Score>),
    (
        With<crate::core::Intent>,
        With<crate::core::Health>,
        With<runtime::InMinigame>,
        Without<crate::core::Connecting>,
        Without<crate::core::Spectating>,
        Without<crate::core::Npc>,
    ),
>;

/// The players in the lead now (none before anyone has a rung or a kill),
/// sorted.
fn leaders_of(climbers: &Climbers) -> Vec<Entity> {
    let keyed: Vec<((usize, i32), Entity)> = climbers
        .iter()
        .filter(|(_, t, ..)| crate::mode::plays(t.0))
        .map(|(e, _, r, s)| ((r.map_or(0, |r| r.0), s.map_or(0, |s| s.kills)), e))
        .collect();
    let Some(best) = keyed.iter().map(|(k, _)| *k).max() else { return Vec::new() };
    if best <= (0, 0) {
        return Vec::new();
    }
    let mut out: Vec<Entity> = keyed.into_iter().filter(|(k, _)| *k == best).map(|(_, e)| e).collect();
    out.sort();
    out
}

/// The lead's events from what changed since `Leaders`.
fn follow_lead(
    m: Res<Match>,
    climbers: Climbers,
    rungs: Query<&Rung>,
    leaders: Option<ResMut<Leaders>>,
    mut events: MessageWriter<MinigameEvent>,
    mut commands: Commands,
) {
    if !m.counting() {
        return;
    }
    let now = leaders_of(&climbers);
    let before = leaders.as_ref().map(|l| l.0.clone()).unwrap_or_default();
    if now == before {
        return;
    }
    let mut out = Vec::new();
    for e in before.iter().filter(|e| !now.contains(e)) {
        out.push(("lead_lost", *e));
    }
    match now.as_slice() {
        [] => {}
        [one] => out.push(("lead_taken", *one)),
        many => {
            // The newcomers, and a sole leader now sharing.
            let alone = before.len() == 1;
            for e in many.iter().filter(|e| !before.contains(e) || alone) {
                out.push(("lead_tied", *e));
            }
        }
    }
    for (name, e) in out {
        let value = rungs.get(e).map_or(0, |r| r.0) as i32;
        events.write(MinigameEvent {
            name,
            player: Some(e),
            value,
        });
    }
    match leaders {
        Some(mut l) => l.0 = now,
        None => commands.insert_resource(Leaders(now)),
    }
}

/// With `refill_ammo`, a held rung gun's reserve is topped up every tick:
/// the ladder gives each gun once, and a 200-round machine gun used up
/// (misses count) would otherwise end the climb; a rung's grenade is
/// given back once thrown.
fn refill(
    ladder: Option<Res<Ladder>>,
    mut guns: Query<(&Weapon, &mut Magazine)>,
    mut grenades: Query<(Entity, &Weapon, &mut Throwable, Has<KeepSpent>)>,
    mut commands: Commands,
) {
    let Some(ladder) = ladder else { return };
    if !ladder.refill_ammo {
        return;
    }
    let rungs = &ladder.rungs;
    for (w, mut m) in &mut guns {
        if w.owner.is_some() && m.reserve < m.reserve_max && rungs.contains(&w.id) {
            m.reserve = m.reserve_max;
        }
    }
    // A grenade rung's grenade comes back once thrown (kept in the hand
    // meanwhile: `KeepSpent`), so a player throws until a kill.
    for (e, w, mut t, kept) in &mut grenades {
        if w.owner.is_none() || !rungs.contains(&w.id) {
            continue;
        }
        if !kept {
            commands.entity(e).insert(KeepSpent);
        }
        let full = t.max.max(1);
        if t.count < full {
            t.count = full;
        }
    }
}

/// Everyone back to the first rung as the match starts.
fn before_spawns(world: &mut World) {
    for (e, _) in runtime::players(world) {
        world.entity_mut(e).insert(Rung(0));
    }
    world.insert_resource(Leaders::default());
}

/// An enemy kill: the killer a rung up (its weapon swapped), or the match
/// won from the last rung; one's own death (a suicide, a fall): a rung
/// down with `demote_on_suicide`.
fn on_death(world: &mut World, d: &Death) {
    match d.kind {
        DeathKind::Enemy => {}
        DeathKind::Suicide | DeathKind::NoKiller => return own_death(world, d.victim),
        DeathKind::TeamKill => return,
    }
    let Some(k) = d.killer else { return };
    let Some(ladder) = world.get_resource::<Ladder>().cloned() else { return };
    let rung = world.get::<Rung>(k).map_or(0, |r| r.0) + 1;
    if rung >= ladder.rungs.len() {
        if let Some(team) = world.get::<Team>(k).map(|t| t.0) {
            world.resource_mut::<Match>().win(team);
        }
        return;
    }
    set_rung(world, k, rung, &ladder);
    world.write_message(MinigameEvent {
        name: "rung_up",
        player: Some(k),
        value: rung as i32,
    });
    if rung + 1 == ladder.rungs.len() {
        world.write_message(MinigameEvent {
            name: "last_rung",
            player: Some(k),
            value: rung as i32,
        });
    }
}

/// `victim` died by its own doing (a suicide, a fall, the world): a rung
/// down (not below the first; dead, the weapon is the next life's).
fn own_death(world: &mut World, victim: Entity) {
    let Some(ladder) = world.get_resource::<Ladder>() else { return };
    if !ladder.demote_on_suicide {
        return;
    }
    let rung = world.get::<Rung>(victim).map_or(0, |r| r.0);
    if rung == 0 {
        return;
    }
    world.entity_mut(victim).insert(Rung(rung - 1));
    world.write_message(MinigameEvent {
        name: "suicide_down",
        player: Some(victim),
        value: rung as i32 - 1,
    });
}

/// A melee death costs the victim a rung (`demote_on = "melee_death"`,
/// while the match counts, never below the first): the ladder's
/// `ModeDeath` observer, run where the death step triggers it (after the
/// counting, death by death).
#[allow(clippy::too_many_arguments)]
pub(super) fn died(
    death: On<crate::mode::ModeDeath>,
    ladder: Option<Res<Ladder>>,
    game: Option<Res<Match>>,
    teams: Query<&Team>,
    rungs: Query<&Rung>,
    held: Query<&Inventory>,
    weapons: Query<&Weapon>,
    catalog: Option<Res<WeaponCatalog>>,
    mut events: MessageWriter<MinigameEvent>,
    mut commands: Commands,
) {
    let d = &death.died;
    if death.mode != MODE.id || !ladder.is_some_and(|l| l.demote_on_melee) || !game.is_some_and(|m| m.counting()) {
        return;
    }
    if !melee_kill(d, &held, &weapons, catalog.as_deref()) {
        return;
    }
    let team = |e: Entity| teams.get(e).ok().map(|t| t.0);
    let Some(k) = d.attacker.filter(|k| *k != d.entity) else { return };
    if team(k) == team(d.entity) {
        return;
    }
    let rung = rungs.get(d.entity).map_or(0, |r| r.0);
    if rung == 0 {
        return;
    }
    // Dead: the weapon is the next life's.
    commands.entity(d.entity).insert(Rung(rung - 1));
    events.write(MinigameEvent {
        name: "rung_down",
        player: Some(d.entity),
        value: rung as i32 - 1,
    });
}

/// Whether `d` was a melee kill: melee damage, or a melee weapon's (the
/// damage's weapon, else what the attacker holds; CS:S's knife deals
/// bullet damage).
fn melee_kill(d: &Died, held: &Query<&Inventory>, weapons: &Query<&Weapon>, catalog: Option<&WeaponCatalog>) -> bool {
    if d.damage.kind == DamageKind::Melee {
        return true;
    }
    let id = d.damage.weapon.or_else(|| {
        let inv = held.get(d.attacker?).ok()?;
        weapons.get(inv.active?).ok().map(|w| w.id)
    });
    let Some(id) = id else { return false };
    catalog.and_then(|c| c.get(id)).is_some_and(|w| w.melee.is_some())
}

/// `e` on `rung`, holding its weapon in place of the ladder's others (on
/// the knife-only last rung: that knife alone).
pub fn set_rung(world: &mut World, e: Entity, rung: usize, ladder: &Ladder) {
    world.entity_mut(e).insert(Rung(rung));
    arm(world, e, ladder, rung);
    // A character's weapon alone (on the knife finale too), else the
    // knife finale's knife alone.
    if ladder.character(rung).is_some()
        && let Some(w) = ladder.weapon_at(rung, world.get_resource::<WeaponCatalog>())
    {
        return knife_alone(world, e, w);
    }
    if ladder.knife_only(rung) {
        return knife_alone(world, e, ladder.rungs[rung]);
    }
    let before = carried_slots(world, e, ladder);
    let old: Vec<Entity> = world
        .get::<Inventory>(e)
        .map(|i| {
            i.weapons
                .iter()
                .copied()
                .filter(|w| world.get::<Weapon>(*w).is_some_and(|w| ladder.is_gun(w.id)))
                .collect()
        })
        .unwrap_or_default();
    for w in old {
        runtime::take_weapon(world, e, w);
    }
    // A kit by game (the gun game's knife) goes with the new gun's game,
    // a rung's melee weapon in place of the kit's.
    // The rungs' melee weapons and characters' weapons: taken when not
    // this rung's (the kit given back after a character's rung).
    let melees = world
        .get_resource::<WeaponCatalog>()
        .map(|c| {
            let mut m = ladder.melees(c);
            m.extend(ladder.character_weapons(c));
            m
        })
        .unwrap_or_default();
    runtime::refit_kit(world, e, ladder.rungs[rung], &melees);
    give(world, e, ladder.rungs[rung]);
    keep_slot_order(world, e, &before, ladder);
    // No command of the player's did this: its client takes the state
    // as a restart, not a misprediction.
    crate::core::rebase(world, e);
}

/// `e` holding `knife` alone, drawn: every other weapon taken (the gun,
/// the kit's others), the knife kept if carried, else given.
fn knife_alone(world: &mut World, e: Entity, knife: &'static str) {
    let held: Vec<(Entity, &'static str)> = world
        .get::<Inventory>(e)
        .map(|i| i.weapons.iter().filter_map(|w| world.get::<Weapon>(*w).map(|x| (*w, x.id))).collect())
        .unwrap_or_default();
    let mut kept = None;
    for (w, id) in held {
        if id == knife && kept.is_none() {
            kept = Some(w);
        } else {
            runtime::take_weapon(world, e, w);
        }
    }
    let knife = kept.or_else(|| give(world, e, knife));
    if let (Some(k), Some(mut inv)) = (knife, world.get_mut::<Inventory>(e)) {
        inv.wanted = Some(k);
    }
    crate::core::rebase(world, e);
}

/// What `e` carries as (weapon, slot, is one of the ladder's guns: else
/// the kit's, the knife).
fn carried_slots(world: &World, e: Entity, ladder: &Ladder) -> Vec<(Entity, u8, bool)> {
    let Some(inv) = world.get::<Inventory>(e) else { return Vec::new() };
    inv.weapons
        .iter()
        .filter_map(|w| {
            let weapon = world.get::<Weapon>(*w)?;
            Some((*w, weapon.slot, ladder.is_gun(weapon.id)))
        })
        .collect()
}

/// `e`'s slot order for `lastinv` (`Inventory::slot_mru`) across a swap:
/// the weapons it no longer holds since `before` (the old gun, a knife of
/// the other game) hand their slot's place to the ones that came in (the
/// new gun, the other game's knife), a gun to a gun and a kit weapon to a
/// kit weapon, so Q still goes gun <-> knife whichever slots they are in.
fn keep_slot_order(world: &mut World, e: Entity, before: &[(Entity, u8, bool)], ladder: &Ladder) {
    let now = carried_slots(world, e, ladder);
    for gun in [true, false] {
        let gone = before.iter().filter(|(w, _, g)| *g == gun && !now.iter().any(|(n, ..)| n == w));
        let came = now.iter().filter(|(w, _, g)| *g == gun && !before.iter().any(|(b, ..)| b == w));
        let pairs: Vec<(u8, u8)> = gone.zip(came).map(|(g, c)| (g.1, c.1)).collect();
        if let Some(mut inv) = world.get_mut::<Inventory>(e) {
            // Through a spare number first: two slots may trade places.
            for (i, (old, _)) in pairs.iter().enumerate() {
                inv.rename_slot(*old, 200 + i as u8);
            }
            for (i, (_, new)) in pairs.iter().enumerate() {
                inv.rename_slot(200 + i as u8, *new);
            }
        }
    }
}

/// The time limit: the highest rung, then the most kills; a tie, a draw.
fn time_out(world: &mut World) {
    let mut best: Option<((usize, i32), u8)> = None;
    let mut tie = false;
    for (e, team) in runtime::players(world) {
        let key = (world.get::<Rung>(e).map_or(0, |r| r.0), world.get::<Score>(e).map_or(0, |s| s.kills));
        match best {
            Some((b, _)) if key == b => tie = true,
            Some((b, _)) if key < b => {}
            _ => {
                best = Some((key, team));
                tie = false;
            }
        }
    }
    if let Some((_, team)) = best.filter(|_| !tie) {
        world.resource_mut::<Match>().winner = Some(team);
    }
}

/// `e`'s place on the ladder, for `minigame_status`.
pub fn standing(world: &World, e: Entity) -> Option<String> {
    let active = world.get_resource::<ActiveMinigame>()?;
    if active.rules.mode.id != LADDER {
        return None;
    }
    let ladder = world.get_resource::<Ladder>()?;
    let rung = world.get::<Rung>(e).map_or(0, |r| r.0);
    Some(format!(", rung {}/{} ({})", rung + 1, ladder.rungs.len(), ladder.rungs.get(rung).unwrap_or(&"?")))
}

/// Whether the ladder is the rules playing (the host's and a client's
/// alike: `ActiveMinigame`).
fn ladder_plays(active: Option<Res<ActiveMinigame>>) -> bool {
    active.is_some_and(|a| a.rules.mode.id == LADDER)
}

/// Each player's HUD lines (`super::hud::MinigameStatus`): their rung and gun,
/// the next one, who leads (the highest rung, then the most kills;
/// "tied" when level). Written only when a line changes; every game's
/// HUD reads them through `hud::update`. Runs on the host and on a
/// client (its own copy from the replicated rungs and scores), before
/// `hud::update`.
pub(super) fn write_status(
    ladder: Option<Res<Ladder>>,
    catalog: Option<Res<WeaponCatalog>>,
    climbers: Climbers,
    local: Query<Entity, With<crate::core::LocalPlayer>>,
    mut status: Query<&mut super::hud::MinigameStatus>,
    rungs: Query<&Rung>,
    mut commands: Commands,
) {
    let Some(ladder) = ladder else { return };
    let total = ladder.rungs.len();
    let mut best: Option<((usize, i32), Entity)> = None;
    let mut tied = false;
    for (e, t, r, s) in &climbers {
        if !crate::mode::plays(t.0) {
            continue;
        }
        let key = (r.map_or(0, |r| r.0), s.map_or(0, |s| s.kills));
        match best {
            Some((b, _)) if key == b => tied = true,
            Some((b, _)) if key < b => {}
            _ => {
                best = Some((key, e));
                tied = false;
            }
        }
    }
    let catalog = catalog.as_deref();
    let viewers = climbers.iter().map(|(e, ..)| e).chain(local.iter());
    for viewer in viewers {
        let rung = rungs.get(viewer).map_or(0, |r| r.0).min(total.saturating_sub(1));
        let mut lines = Vec::new();
        let (now, next) = (ladder.weapon_at(rung, catalog), ladder.weapon_at(rung + 1, catalog));
        if let Some(w) = &now {
            // The rung's label (its fitted parts) after the weapon.
            let label = ladder.extras(rung).and_then(|x| x.label.as_deref()).map(|l| format!(" ({})", l.replace(['{', '}'], "")));
            lines.push(HudLine::new(format!("Rung {}/{total}: {{weapon}}{}", rung + 1, label.unwrap_or_default())).weapon(w));
        }
        match next {
            Some(w) => lines.push(HudLine::new("Next: {weapon}").weapon(w)),
            None => lines.push(HudLine::new("Last rung: a kill wins")),
        }
        if let Some(((r, _), who)) = best {
            lines.push(if tied {
                HudLine::new("Tied for the lead")
            } else if who == viewer {
                HudLine::new("You lead")
            } else {
                HudLine::new(format!("Leader: {{player}}, rung {}/{total}", r + 1)).player(who)
            });
        }
        match status.get_mut(viewer) {
            Ok(mut s) => {
                s.set_if_neq(super::hud::MinigameStatus(lines));
            }
            Err(_) => {
                commands.entity(viewer).insert(super::hud::MinigameStatus(lines));
            }
        }
    }
}

/// The status lines go when the ladder is not the rules playing.
pub(super) fn clear_status(status: Query<Entity, With<super::hud::MinigameStatus>>, mut commands: Commands) {
    for e in &status {
        commands.entity(e).remove::<super::hud::MinigameStatus>();
    }
}

/// The two status systems, for the plugin.
pub(super) fn status_systems() -> ScheduleConfigs<ScheduleSystem> {
    (write_status.run_if(ladder_plays), clear_status.run_if(not(ladder_plays))).before(super::hud::update)
}

/// The viewer's own rung changes (`rung_up`, `rung_down`, `suicide_down`).
fn notice(world: &World, viewer: Entity, event: &MinigameEvent) -> Option<HudLine> {
    if event.player != Some(viewer) {
        return None;
    }
    let ladder = world.get_resource::<Ladder>()?;
    let total = ladder.rungs.len();
    let weapon = ladder.weapon_at(usize::try_from(event.value).ok()?, world.get_resource::<WeaponCatalog>())?;
    let text = match event.name {
        "rung_up" => format!("Rung up! {}/{total}: {{weapon}}", event.value + 1),
        "rung_down" => format!("Knifed! Down to {}/{total}: {{weapon}}", event.value + 1),
        "suicide_down" => format!("Suicide! Down to {}/{total}: {{weapon}}", event.value + 1),
        _ => return None,
    };
    Some(HudLine::new(text).weapon(weapon))
}

/// The viewer's big callouts (`hud::Callout`): its own rung changes, the
/// last rung (its own and others'), its lead.
fn callout(world: &World, viewer: Entity, event: &MinigameEvent) -> Option<Callout> {
    let ladder = world.get_resource::<Ladder>()?;
    let total = ladder.rungs.len();
    let rung = usize::try_from(event.value).ok()?;
    let weapon = ladder.weapon_at(rung, world.get_resource::<WeaponCatalog>());
    let own = event.player == Some(viewer);
    let level_line = || HudLine::new(format!("Level {}/{total}", rung + 1));
    let gun = |text: &str| {
        let line = HudLine::new(text);
        match weapon {
            Some(w) => line.weapon(w),
            None => line,
        }
    };
    Some(match (event.name, own) {
        // The last rung's own callout says it.
        ("rung_up", true) if rung + 1 < total => Callout::new(2, "rung_up", vec![gun("Level up: {weapon}"), level_line()]),
        ("rung_down", true) => Callout::new(3, "rung_down", vec![gun("Knifed! Level down: {weapon}"), level_line()]),
        // The same denied cue as a knifing.
        ("suicide_down", true) => Callout::new(3, "rung_down", vec![gun("Suicide! Level down: {weapon}"), level_line()]),
        ("last_rung", true) => Callout::new(4, "last_rung", vec![HudLine::new("Last level!"), gun("{weapon}: a kill wins")]),
        ("last_rung", false) => Callout::new(
            4,
            "last_rung_other",
            vec![HudLine::new("{player} is on the last level!").player(event.player?)],
        ),
        ("lead_taken", true) => Callout::new(1, "lead_taken", vec![HudLine::new("You took the lead")]),
        ("lead_lost", true) => Callout::new(1, "lead_lost", vec![HudLine::new("You lost the lead")]),
        ("lead_tied", true) => Callout::new(1, "lead_tied", vec![HudLine::new("Tied for the lead")]),
        _ => return None,
    })
}
