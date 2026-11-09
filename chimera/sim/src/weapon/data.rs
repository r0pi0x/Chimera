//! Weapons as data (docs/plans/active/engine-split.md, "Data-driven
//! first"): a weapon is a TOML document of the parts this module builds
//! (`Trigger`, `Magazine`, `Hitscan`, `DamageEffect`, `Penetration`,
//! `Melee`, `AltModes` with `Zoom`/`Burst`, `WeaponSounds`, ...) plus what
//! the game's other layers read by ID (models, HUD names, animation
//! timing), with behaviour data can't express named as hooks a game
//! registers (`WeaponHooks`, `register_weapon_hook`). Games ship their
//! documents and load them at run time; an app can add more (other games'
//! weapons, new ones) with no Rust: `add_weapon_data`. The format is
//! docs/weapon-data.md; in short:
//!
//! ```toml
//! id = "game:weapon_rifle"         # namespaced
//! base = "game:gun"                # an entry whose tables this one extends
//! slot = 0
//! draw = 1.0                       # s
//! max_speed = 221.0                # length units/s
//! [buy]       price = 2500, team = 1, bot_weight = 8.0
//! [trigger]   automatic = true, cycle = 0.1
//! [magazine]  clip = 30, ammo = "game:762mm", reload = 2.4
//! [hitscan]   range = 8192.0
//! [damage]    amount = 36.0, falloff = 0.98, armor_ratio = 1.55
//! [hooks."game:recoil"]  ...       # parameters the hook reads
//! ```
//!
//! A document holds one weapon (top-level `id`), ammo types (`[ammo.<id>]`)
//! or both. `base` merges the named entry's tables under this one's
//! (recursively; `abstract = true` entries are templates only, never
//! registered; `abstract` and `class` aren't inherited); it and `ammo` may name entries of sets loaded before
//! (`WeaponCatalog`). Lengths and damage are in the document's `units`
//! (`length`: metres per unit, `damage`: health per point; both default
//! 1). Unknown keys, unknown bases, ammo or hooks, and hook parameters the
//! hook can't read are load errors.
//!
//! Loaded sets are leaked (`&'static`: weapon IDs are `&'static str`
//! everywhere); each app's registry, prices and `WeaponCatalog` list the
//! sets it loaded, so two apps in one process can hold different data for
//! the same IDs (code with a world reads `WeaponCatalog::get`). A network
//! client whose set differs from the server's puts the server's documents
//! in its place (`replace_weapon_set`, `server_docs`): same registry
//! position, so indices match the server's.
//!
//! Known shortcuts (docs/tech-debt.md): Weapon data: loaded sets are leaked
//! (one allocation per load; IDs are `&'static str` throughout the weapon
//! code); a game's map loader loads held and view models for its own set only, so
//! a weapon added by an app draws no model unless the game loads it. Why
//! it's OK for now: Loads happen at start;
//! weapons from data play (buy, fire, network) without models.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use bevy::prelude::*;
use serde::{Deserialize, de::DeserializeOwned};

use super::{
    AltModes, AltShot, Burst, DamageEffect, FireTiming, HitgroupScale, Hitscan, Magazine, Melee, ModeTexts, Penetration,
    ShellReload, SpreadShape, Swing, SwitchWeight, Trigger, Weapon, WeaponDef, WeaponRegistry, WeaponSounds, Zoom,
    drop::{PickupTeam, Undroppable},
    economy::{AmmoBox, Prices},
};

fn one() -> f32 {
    1.0
}

fn infinite() -> f32 {
    f32::INFINITY
}

/// What the document's numbers are in: `length` metres per unit (Source's
/// inches: 0.0254), `damage` health per point (hit points of 100: 0.01).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Units {
    #[serde(default = "one")]
    pub length: f32,
    #[serde(default = "one")]
    pub damage: f32,
}

impl Default for Units {
    fn default() -> Self {
        Self {
            length: 1.0,
            damage: 1.0,
        }
    }
}

/// One weapon as its document gives it (after `base` merging), numbers in
/// its `units`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponData {
    /// Namespaced, e.g. `game:weapon_rifle`.
    pub id: String,
    /// The entry it extends (informational once merged).
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default, rename = "abstract")]
    pub is_abstract: bool,
    #[serde(default)]
    pub units: Units,
    /// Slot key (0 = first).
    pub slot: u8,
    /// Seconds from drawing to the first attack.
    pub draw: f32,
    /// Most movement speed while held, length units/s (none: the
    /// movement's own).
    #[serde(default)]
    pub max_speed: Option<f32>,
    /// Auto-switch priority (`SwitchWeight`).
    #[serde(default)]
    pub weight: Option<i32>,
    /// Its class in an automatic gun game (`minigame::auto`: its game
    /// lends it to a ladder); None: not in one.
    #[serde(default)]
    pub class: Option<chimera_core::game_parts::GunClass>,
    #[serde(default)]
    pub undroppable: bool,
    /// The drop key leaves it in hand; it still drops at death
    /// (`drop::KeptOnDrop`, CS:S's grenades).
    #[serde(default)]
    pub kept_on_drop: bool,
    /// Dropped at death beside the best gun, the highest carried
    /// (`drop::DeathDrop`).
    #[serde(default)]
    pub death_drop: Option<i32>,
    /// Only this team picks it up (`PickupTeam`).
    #[serde(default)]
    pub pickup_team: Option<u8>,
    #[serde(default)]
    pub buy: Option<BuyData>,
    #[serde(default)]
    pub trigger: Option<TriggerData>,
    #[serde(default)]
    pub magazine: Option<MagazineData>,
    #[serde(default)]
    pub hitscan: Option<HitscanData>,
    #[serde(default)]
    pub damage: Option<DamageData>,
    #[serde(default)]
    pub penetration: Option<PenetrationData>,
    #[serde(default)]
    pub melee: Option<MeleeData>,
    /// Whom its shots and swings rewind (`lagcomp::RewindChoice`); None:
    /// every other character.
    #[serde(default)]
    pub lag_compensation: Option<LagChoiceData>,
    #[serde(default)]
    pub alt: Option<AltData>,
    /// None: no `WeaponSounds` part.
    #[serde(default)]
    pub sounds: Option<SoundsData>,
    #[serde(default)]
    pub models: ModelsData,
    #[serde(default)]
    pub hud: HudData,
    #[serde(default)]
    pub anim: AnimData,
    /// Hook name -> its parameters (any TOML value; `{}` for none).
    #[serde(default)]
    pub hooks: toml::Table,
    /// The magazine's ammo type, resolved at load.
    #[serde(skip)]
    pub ammo: Option<AmmoData>,
}

/// Buying it (`economy::Prices`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuyData {
    pub price: u32,
    /// Only this team buys it (`core::Team` number).
    #[serde(default)]
    pub team: Option<u8>,
    /// How much computer players like it (`Prices::bot_weights`).
    #[serde(default)]
    pub bot_weight: Option<f32>,
    /// Reserve rounds when given at spawn while ammo is sold
    /// (`Prices::starting_reserve`).
    #[serde(default)]
    pub starting_reserve: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimingData {
    Set,
    Accumulate,
    CarryOver,
}

/// `Trigger`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerData {
    pub automatic: bool,
    /// Seconds between shots.
    pub cycle: f32,
    pub timing: TimingData,
    /// Most shots per pull, 0 for no limit (`Trigger::per_press`).
    #[serde(default)]
    pub per_press: u32,
    /// Updates from entering the firing state to the first round
    /// (`Trigger::latency`; 0: none).
    #[serde(default)]
    pub latency: u32,
    /// Seconds of the closing part after a pull's last cycle
    /// (`Trigger::postfire`; 0: none).
    #[serde(default)]
    pub postfire: f32,
}

/// `Magazine` (and `ShellReload`): the reserve starts full and holds the
/// ammo type's `max`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineData {
    pub clip: u32,
    /// An ammo type's ID (`[ammo.<id>]`).
    pub ammo: String,
    /// Seconds from starting a reload to the swap (one round's for
    /// `shells`).
    pub reload: f32,
    #[serde(default)]
    pub reload_while_held: bool,
    /// An empty clip reloads by itself, held or not (`Magazine::auto_reload`).
    #[serde(default)]
    pub auto_reload: bool,
    /// Rounds go in one at a time: the start, each round (`ShellReload`).
    #[serde(default)]
    pub shells: Option<ShellsData>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShellsData {
    pub start: f32,
    pub insert: f32,
    /// The closing part after the last round, s (`ShellReload::finish`).
    #[serde(default)]
    pub finish: f32,
    /// No tick between rounds (`ShellReload::back_to_back`).
    #[serde(default)]
    pub back_to_back: bool,
    /// Each round goes in as its insert starts (`ShellReload::insert_first`).
    #[serde(default)]
    pub insert_first: bool,
}

/// An ammo type: what `Penetration` passes (`power`, length units of a
/// scale-1 material; `max_distance`), most rounds carried and the box
/// it's bought in.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmmoData {
    #[serde(default)]
    pub power: f32,
    #[serde(default = "infinite")]
    pub max_distance: f32,
    pub max: u32,
    #[serde(default)]
    pub box_price: Option<u32>,
    /// None: no ammo for sale (the reserve stays full when bought).
    #[serde(default)]
    pub box_rounds: Option<u32>,
    /// The HUD's icon for it (a game's name).
    #[serde(default)]
    pub hud_icon: Option<String>,
    /// Defaults to the document's top-level `units`.
    #[serde(default)]
    pub units: Option<Units>,
}

/// `Hitscan`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitscanData {
    pub range: f32,
    pub pellets: u32,
    pub punch_scale: f32,
    /// None: a disc of 0 (a game's hook sets it as it changes).
    #[serde(default)]
    pub spread: Option<SpreadData>,
    /// `Hitscan::tracer_every` (0: none).
    #[serde(default)]
    pub tracer_every: u32,
    /// `Hitscan::redrawn_through_characters`.
    #[serde(default)]
    pub redrawn_through_characters: bool,
    /// `Hitscan::odd_box`: the half size (a length).
    #[serde(default)]
    pub odd_box: Option<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SpreadData {
    Template(f32),
    Disc { inaccuracy: f32, spread: f32 },
    /// Source's generic (HL2) cone (`SpreadShape::Cone`): the spread
    /// vector's component, the same on both axes.
    Cone(f32),
}

/// `DamageEffect` (damage in damage units, `falloff_step` a length,
/// `impulse` mass x length/s).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageData {
    pub amount: f32,
    pub falloff: f32,
    pub falloff_step: f32,
    pub hitgroups: HitgroupData,
    pub impulse: f32,
    #[serde(default)]
    pub quantum: f32,
    #[serde(default)]
    pub armor_ratio: Option<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HitgroupData {
    pub head: f32,
    pub chest: f32,
    pub stomach: f32,
    pub arm: f32,
    pub leg: f32,
}

/// `Penetration`: objects passed; power and distance are the ammo's
/// unless given.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PenetrationData {
    pub objects: u32,
    #[serde(default)]
    pub power: Option<f32>,
    #[serde(default)]
    pub max_distance: Option<f32>,
}

/// `lagcomp::RewindChoice`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LagChoiceData {
    pub fired_within: u32,
    pub reach: f32,
    pub view_cos: f32,
}

/// `Melee`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeleeData {
    pub primary: SwingData,
    #[serde(default)]
    pub secondary: Option<SwingData>,
}

/// A `Swing` (lengths and damage in the units; `force` per hit, mass x
/// length/s).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwingData {
    pub range: f32,
    #[serde(default)]
    pub hull: Option<f32>,
    /// Default: `range`.
    #[serde(default)]
    pub hull_reach: Option<f32>,
    pub facing_cos: f32,
    /// `Swing::refine`.
    #[serde(default)]
    pub refine: bool,
    pub damage: f32,
    /// (window s, damage).
    #[serde(default)]
    pub follow_up: Option<(f32, f32)>,
    #[serde(default)]
    pub backstab: Option<f32>,
    #[serde(default)]
    pub hitgroups: bool,
    pub hit_refire: f32,
    pub miss_refire: f32,
    pub miss_refire_other: f32,
    pub force: f32,
    /// The damage type its hits deal (default melee).
    #[serde(default)]
    pub kind: Option<crate::core::DamageKind>,
    #[serde(default)]
    pub armor_ratio: Option<f32>,
    #[serde(default)]
    pub quantum: f32,
    #[serde(default)]
    pub sound_hit: Option<String>,
    #[serde(default)]
    pub sound_hit_world: Option<String>,
    #[serde(default)]
    pub sound_miss: Option<String>,
}

/// What attack2 does (`AltModes` and its parts).
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AltData {
    /// Zoom levels (`Zoom`): FOV per level, the zoomed speed (none:
    /// unchanged), `step` seconds between steps.
    Scope {
        fov: Vec<f32>,
        #[serde(default)]
        max_speed: Option<f32>,
        #[serde(default)]
        unzoom_after_shot: bool,
        #[serde(default)]
        overlay: bool,
        step: f32,
        #[serde(default)]
        sound: Option<String>,
    },
    /// A silencer: two modes, both attacks wait `time` on a switch.
    Silencer { time: f32 },
    /// Bursts of `count` rounds `interval` apart in mode 1, the next pull
    /// `refire` after the first round; `texts`: what each mode prints.
    Burst {
        interval: f32,
        refire: f32,
        count: u32,
        step: f32,
        #[serde(default)]
        texts: Vec<String>,
        /// One burst per pull (`Burst::once_per_press`).
        #[serde(default)]
        once_per_press: bool,
        /// A step blocks the primary attack for `step` too (Combat Arms'
        /// fire-mode lockout).
        #[serde(default)]
        step_blocks_primary: bool,
    },
    /// Attack2 fires a bigger shot from the clip (`AltShot`: HL2's
    /// shotgun's double shot): `pellets`, `rounds` from the clip, the next
    /// attack `cycle` seconds on.
    Shot { pellets: u32, rounds: u32, cycle: f32 },
}

/// Sound entries (`WeaponSounds`); timed ones as [seconds, entry].
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundsData {
    #[serde(default)]
    pub fire: Option<String>,
    #[serde(default)]
    pub fire_alt: Option<String>,
    #[serde(default)]
    pub empty: Option<String>,
    #[serde(default)]
    pub deploy: Option<String>,
    #[serde(default)]
    pub reload: Vec<(f32, String)>,
    #[serde(default)]
    pub draw: Vec<(f32, String)>,
    #[serde(default)]
    pub shot: Vec<(f32, String)>,
    #[serde(default)]
    pub finish: Vec<(f32, String)>,
    /// [mode, seconds, entry].
    #[serde(default)]
    pub modes: Vec<(u8, f32, String)>,
}

impl SoundsData {
    /// Every entry named.
    pub fn entries(&self) -> impl Iterator<Item = &str> {
        [&self.fire, &self.fire_alt, &self.empty, &self.deploy]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .chain(
                [&self.reload, &self.draw, &self.shot, &self.finish]
                    .into_iter()
                    .flatten()
                    .map(|(_, s)| s.as_str()),
            )
            .chain(self.modes.iter().map(|(_, _, s)| s.as_str()))
    }
}

/// Model files (the game's paths): held (`world`; `world_alt` in an
/// alternate mode, e.g. silenced) and seen in first person (`view`,
/// `view_right_handed`: built for the right hand).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsData {
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub world_alt: Option<String>,
    #[serde(default)]
    pub view: Option<String>,
    #[serde(default)]
    pub view_right_handed: bool,
    /// The view model's skins (textures by the model's texture index), for
    /// models that don't name their own (LithTech's).
    #[serde(default)]
    pub view_skins: Vec<String>,
    /// Where the view model's origin sits in the eye's space, in the
    /// game's own units and axes (its loader converts them).
    #[serde(default)]
    pub view_offset: Option<[f32; 3]>,
}

/// The HUD's names for it: `name` (a string token), `icon` (a kill icon).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HudData {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

/// View-model idling: not sooner than `idle_after` s after an attack (and
/// the draw too with `idle_after_draw`), again `idle_interval` s after
/// the last idle started.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimData {
    #[serde(default)]
    pub idle_after: f32,
    #[serde(default)]
    pub idle_interval: f32,
    #[serde(default)]
    pub idle_after_draw: bool,
    /// What the view model plays, by action (`view_anim::ACTIONS`: draw,
    /// idle, fire, reload, ...) -> sequence names (one at random); empty:
    /// the game's `view_anim::ViewAnimRules` choose.
    #[serde(default)]
    pub sequences: BTreeMap<String, Vec<String>>,
}

impl WeaponData {
    /// Whether the data names this hook.
    pub fn has_hook(&self, name: &str) -> bool {
        self.hooks.contains_key(name)
    }

    /// The parameters it gives a hook, read as `P`.
    pub fn hook<P: DeserializeOwned>(&self, name: &str) -> Option<P> {
        self.hooks.get(name).and_then(|v| v.clone().try_into().ok())
    }
}

// ---------------------------------------------------------------------------
// Sets

/// Weapons and ammo types loaded together, in document order.
#[derive(Debug, Default)]
pub struct WeaponSet {
    pub weapons: Vec<WeaponData>,
    /// `weapons` by id, made on the first look (a ladder's draw looks up
    /// thousands of times in a set of thousands: `get`).
    index: std::sync::OnceLock<bevy::platform::collections::HashMap<String, usize>>,
    pub ammo: BTreeMap<String, AmmoData>,
    /// Every entry (templates too) merged with its bases, for later sets'
    /// `base`.
    tables: BTreeMap<String, toml::Table>,
}

impl WeaponSet {
    pub fn get(&self, id: &str) -> Option<&WeaponData> {
        let index = self.index.get_or_init(|| {
            let mut m = bevy::platform::collections::HashMap::with_capacity(self.weapons.len());
            for (i, w) in self.weapons.iter().enumerate() {
                // The first of an id, as a search from the front finds.
                m.entry(w.id.to_string()).or_insert(i);
            }
            m
        });
        self.weapons.get(*index.get(id)?)
    }

    /// Parse `docs` ((name for errors, TOML text)); `base` and `ammo` may
    /// name entries of `known` (later ones first).
    pub fn parse(docs: &[(&str, &str)], known: &[&WeaponSet]) -> Result<Self, String> {
        let mut raw: Vec<(String, toml::Table)> = Vec::new();
        let mut ammo = BTreeMap::new();
        for (name, text) in docs {
            let mut doc: toml::Table = toml::from_str(text).map_err(|e| format!("{name}: {e}"))?;
            let doc_units: Option<Units> = match doc.get("units") {
                Some(u) => Some(u.clone().try_into().map_err(|e| format!("{name}: units: {e}"))?),
                None => None,
            };
            if let Some(types) = doc.remove("ammo") {
                let toml::Value::Table(types) = types else {
                    return Err(format!("{name}: `ammo` is not a table of ammo types"));
                };
                for (id, t) in types {
                    let mut a: AmmoData = t.try_into().map_err(|e| format!("{name}: ammo {id}: {e}"))?;
                    a.units = a.units.or(doc_units);
                    if ammo.insert(id.clone(), a).is_some() {
                        return Err(format!("{name}: ammo {id} defined twice"));
                    }
                }
            }
            if doc.contains_key("id") {
                let Some(toml::Value::String(id)) = doc.get("id").cloned() else {
                    return Err(format!("{name}: `id` is not a string"));
                };
                if raw.iter().any(|(i, _)| *i == id) {
                    return Err(format!("{name}: {id} defined twice"));
                }
                raw.push((id, doc));
            } else if doc.keys().any(|k| k != "units") {
                return Err(format!("{name}: no `id` (a weapon) or `[ammo.*]` (ammo types)"));
            }
        }
        let mut tables = BTreeMap::new();
        for (id, _) in &raw {
            merged(id, &raw, known, &mut tables, &mut Vec::new())?;
        }
        let mut weapons = Vec::new();
        for (id, _) in &raw {
            let t = tables[id].clone();
            if t.get("abstract").and_then(toml::Value::as_bool) == Some(true) {
                continue;
            }
            let mut w: WeaponData = toml::Value::Table(t).try_into().map_err(|e| format!("{id}: {e}"))?;
            if let Some(m) = &w.magazine {
                let a = ammo
                    .get(&m.ammo)
                    .or_else(|| known.iter().rev().find_map(|s| s.ammo.get(&m.ammo)))
                    .ok_or_else(|| format!("{id}: unknown ammo {}", m.ammo))?;
                w.ammo = Some(a.clone());
            }
            // Without damage the frame only tells each round (`Fired`): a
            // game's hook must deliver it.
            if w.hitscan.is_some() && w.damage.is_none() && w.hooks.is_empty() {
                return Err(format!("{id}: [hitscan] without [damage] or a hook to deliver it"));
            }
            if w.hitscan.is_some() && w.trigger.is_none() {
                return Err(format!("{id}: [hitscan] without [trigger]"));
            }
            let action = |a: &&String| super::view_anim::ACTIONS.contains(&a.as_str());
            if let Some(a) = w.anim.sequences.keys().find(|a| !action(a)) {
                return Err(format!("{id}: [anim.sequences] {a} is not a view-model action"));
            }
            weapons.push(w);
        }
        Ok(Self { weapons, index: Default::default(), ammo, tables })
    }

    /// The `.toml` documents in `dir`, by file name (for `parse`).
    pub fn read_dir(dir: &std::path::Path) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|x| x == "toml") {
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                out.push((path.file_name().unwrap().to_string_lossy().into_owned(), text));
            }
        }
        out.sort();
        Ok(out)
    }
}

/// `id`'s table merged over its bases' (`abstract` and `class` aren't
/// inherited: a launcher's stun variant isn't a gun of a ladder).
fn merged(
    id: &str,
    raw: &[(String, toml::Table)],
    known: &[&WeaponSet],
    done: &mut BTreeMap<String, toml::Table>,
    stack: &mut Vec<String>,
) -> Result<toml::Table, String> {
    if let Some(t) = done.get(id) {
        return Ok(t.clone());
    }
    let Some((_, own)) = raw.iter().find(|(i, _)| i == id) else {
        return known
            .iter()
            .rev()
            .find_map(|s| s.tables.get(id).cloned())
            .ok_or_else(|| format!("unknown base {id}"));
    };
    if stack.iter().any(|s| s == id) {
        return Err(format!("{id}: its bases loop ({})", stack.join(" -> ")));
    }
    let out = match own.get("base") {
        Some(toml::Value::String(base)) => {
            stack.push(id.to_string());
            let mut t = merged(base, raw, known, done, stack).map_err(|e| format!("{id}: {e}"))?;
            stack.pop();
            t.remove("abstract");
            t.remove("class");
            deep_merge(&mut t, own);
            t
        }
        Some(_) => return Err(format!("{id}: `base` is not a string")),
        None => own.clone(),
    };
    done.insert(id.to_string(), out.clone());
    Ok(out)
}

/// `over`'s keys into `base`; tables merge, everything else replaces.
fn deep_merge(base: &mut toml::Table, over: &toml::Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => deep_merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The app

/// A hook's parts, ready to insert.
pub type HookBuild = Arc<dyn Fn(&mut EntityWorldMut) + Send + Sync>;
type Prepare = Arc<dyn Fn(&'static WeaponData, &toml::Value) -> Result<HookBuild, String> + Send + Sync>;

/// Behaviour weapon data names (`hooks."<name>"`), registered by games
/// before their data loads.
#[derive(Resource, Default)]
pub struct WeaponHooks(HashMap<String, Prepare>);

impl WeaponHooks {
    pub fn names(&self) -> Vec<&str> {
        let mut n: Vec<&str> = self.0.keys().map(String::as_str).collect();
        n.sort();
        n
    }
}

/// The weapon sets this app loaded, in order.
#[derive(Resource, Clone, Default)]
pub struct WeaponCatalog {
    pub sets: Vec<&'static WeaponSet>,
}

impl WeaponCatalog {
    pub fn get(&self, id: &str) -> Option<&'static WeaponData> {
        self.sets.iter().rev().find_map(|s| s.get(id))
    }

    /// As `get`, also by the part after the namespace with or without
    /// `weapon_` (`weapon_usp`, `usp`).
    pub fn get_name(&self, name: &str) -> Option<&'static WeaponData> {
        self.get(name).or_else(|| {
            self.sets.iter().rev().find_map(|s| {
                s.weapons.iter().find(|w| {
                    let short = w.id.rsplit(':').next().unwrap_or(&w.id);
                    short == name || short.strip_prefix("weapon_") == Some(name)
                })
            })
        })
    }
}

pub trait WeaponDataAppExt {
    /// Name a behaviour weapon data can ask for: `hook` runs after the
    /// data's parts are on the weapon, with the parameters read as `P`.
    fn register_weapon_hook<P: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        name: &str,
        hook: impl Fn(&mut EntityWorldMut, &'static WeaponData, &P) + Send + Sync + 'static,
    ) -> &mut Self;

    /// Register a parsed set's weapons (replacing same IDs in place), their
    /// prices and catalog entry. Every hook must be registered.
    fn add_weapon_set(&mut self, set: &'static WeaponSet) -> Result<(), String>;

    /// Parse documents against the sets loaded so far and add them.
    fn add_weapon_data(&mut self, docs: &[(&str, &str)]) -> Result<&'static WeaponSet, String>;
}

impl WeaponDataAppExt for App {
    fn register_weapon_hook<P: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        name: &str,
        hook: impl Fn(&mut EntityWorldMut, &'static WeaponData, &P) + Send + Sync + 'static,
    ) -> &mut Self {
        let hook = Arc::new(hook);
        let hook_name = name.to_string();
        let prepare: Prepare = Arc::new(move |d, params| {
            let p: P = params
                .clone()
                .try_into()
                .map_err(|e| format!("{}: hook {hook_name}: {e}", d.id))?;
            let (p, hook) = (Arc::new(p), hook.clone());
            Ok(Arc::new(move |e: &mut EntityWorldMut| hook(e, d, &p)) as HookBuild)
        });
        self.world_mut()
            .get_resource_or_init::<WeaponHooks>()
            .0
            .insert(name.to_string(), prepare);
        self
    }

    fn add_weapon_set(&mut self, set: &'static WeaponSet) -> Result<(), String> {
        let world = self.world_mut();
        let defs = weapon_defs(world, set)?;
        for (d, def) in defs {
            let mut registry = world.get_resource_or_init::<WeaponRegistry>();
            match registry.0.iter_mut().find(|w| w.id == d.id) {
                Some(old) => *old = def,
                None => registry.0.push(def),
            }
            let mut prices = world.get_resource_or_init::<Prices>();
            add_prices(&mut prices, d);
        }
        let mut catalog = world.get_resource_or_init::<WeaponCatalog>();
        if !catalog.sets.iter().any(|s| std::ptr::eq(*s, set)) {
            catalog.sets.push(set);
        }
        Ok(())
    }

    fn add_weapon_data(&mut self, docs: &[(&str, &str)]) -> Result<&'static WeaponSet, String> {
        let known = self.world_mut().get_resource_or_init::<WeaponCatalog>().sets.clone();
        let set: &'static WeaponSet = Box::leak(Box::new(WeaponSet::parse(docs, &known)?));
        self.add_weapon_set(set)?;
        Ok(set)
    }
}

/// `set`'s registry entries (its data's parts, then its hooks'); every
/// hook must be registered.
fn weapon_defs(world: &mut World, set: &'static WeaponSet) -> Result<Vec<(&'static WeaponData, WeaponDef)>, String> {
    let hooks = world.get_resource_or_init::<WeaponHooks>();
    let mut builds = Vec::new();
    for d in &set.weapons {
        let mut parts: Vec<HookBuild> = Vec::new();
        for (name, params) in &d.hooks {
            let prepare = hooks.0.get(name).ok_or_else(|| {
                format!(
                    "{}: unknown hook {name} (registered: {})",
                    d.id,
                    hooks.names().join(", ")
                )
            })?;
            parts.push(prepare(d, params)?);
        }
        builds.push((d, parts));
    }
    Ok(builds
        .into_iter()
        .map(|(d, parts)| {
            let build = move |e: &mut EntityWorldMut| {
                insert_parts(d, e);
                for p in &parts {
                    p(e);
                }
            };
            (
                d,
                WeaponDef {
                    id: d.id.as_str(),
                    build: Arc::new(build),
                },
            )
        })
        .collect())
}

/// Put the set parsed from `docs` in place of `old` (a network client
/// taking the server's documents: `server_docs`): `old`'s weapons leave
/// the registry and the new set's take their place, from the position of
/// `old`'s first (the end if none was there), so the weapons registered
/// after shift as they do on a machine that registered the new set; its
/// prices and catalog entry are replaced too. `docs` may name bases and
/// ammo of the other sets loaded. Err: `docs` don't parse or name a hook
/// this app lacks (nothing changed).
pub fn replace_weapon_set(
    world: &mut World,
    old: &'static WeaponSet,
    docs: &[(String, String)],
) -> Result<&'static WeaponSet, String> {
    let known: Vec<&'static WeaponSet> = world
        .get_resource_or_init::<WeaponCatalog>()
        .sets
        .iter()
        .copied()
        .filter(|s| !std::ptr::eq(*s, old))
        .collect();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set: &'static WeaponSet = Box::leak(Box::new(WeaponSet::parse(&refs, &known)?));
    swap_weapon_set(world, old, set)?;
    Ok(set)
}

/// `replace_weapon_set` with a set already parsed (a set put back: the
/// client's own after the server's).
pub fn swap_weapon_set(world: &mut World, old: &'static WeaponSet, set: &'static WeaponSet) -> Result<(), String> {
    let defs = weapon_defs(world, set)?;
    let gone: std::collections::BTreeSet<&str> = old.weapons.iter().map(|w| w.id.as_str()).collect();
    {
        let mut registry = world.get_resource_or_init::<WeaponRegistry>();
        let at = registry.0.iter().position(|d| gone.contains(d.id)).unwrap_or(registry.0.len());
        let mut kept: Vec<WeaponDef> = std::mem::take(&mut registry.0)
            .into_iter()
            .filter(|d| !gone.contains(d.id))
            .collect();
        let tail = kept.split_off(at);
        kept.extend(defs.iter().map(|(_, def)| WeaponDef {
            id: def.id,
            build: def.build.clone(),
        }));
        kept.extend(tail);
        registry.0 = kept;
    }
    let mut prices = world.get_resource_or_init::<Prices>();
    for id in &gone {
        prices.weapons.remove(id);
        prices.team_only.remove(id);
        prices.bot_weights.remove(id);
        prices.starting_reserve.remove(id);
        prices.ammo.remove(id);
    }
    for (d, _) in &defs {
        add_prices(&mut prices, d);
    }
    let mut catalog = world.get_resource_or_init::<WeaponCatalog>();
    match catalog.sets.iter().position(|s| std::ptr::eq(*s, old)) {
        Some(i) => catalog.sets[i] = set,
        None => catalog.sets.push(set),
    }
    Ok(())
}

fn add_prices(p: &mut Prices, d: &'static WeaponData) {
    let id = d.id.as_str();
    if let Some(b) = &d.buy {
        p.weapons.insert(id, b.price);
        if let Some(team) = b.team {
            p.team_only.insert(id, team);
        }
        if let Some(w) = b.bot_weight {
            p.bot_weights.insert(id, w);
        }
        if let Some(r) = b.starting_reserve {
            p.starting_reserve.insert(id, r);
        }
    }
    if let Some(a) = &d.ammo
        && let Some(rounds) = a.box_rounds
    {
        p.ammo.insert(
            id,
            AmmoBox {
                price: a.box_price.unwrap_or(0),
                rounds,
            },
        );
    }
}

/// The parts `d` describes, on `e`.
pub fn insert_parts(d: &'static WeaponData, e: &mut EntityWorldMut) {
    let len = d.units.length;
    let dmg = d.units.damage;
    e.insert(Weapon {
        id: d.id.as_str(),
        slot: d.slot,
        owner: None,
        draw_time: d.draw,
        max_speed: d.max_speed.map(|s| s * len),
    });
    if let Some(w) = d.weight {
        e.insert(SwitchWeight(w));
    }
    if d.undroppable {
        e.insert(Undroppable);
    }
    if let Some(c) = d.lag_compensation {
        e.insert(super::lagcomp::RewindChoice {
            fired_within: c.fired_within,
            reach: c.reach,
            view_cos: c.view_cos,
        });
    }
    if d.kept_on_drop {
        e.insert(super::drop::KeptOnDrop);
    }
    if let Some(r) = d.death_drop {
        e.insert(super::drop::DeathDrop(r));
    }
    if let Some(t) = d.pickup_team {
        e.insert(PickupTeam(crate::core::Team(t)));
    }
    if let Some(t) = &d.trigger {
        e.insert(Trigger {
            automatic: t.automatic,
            cycle: t.cycle,
            timing: match t.timing {
                TimingData::Set => FireTiming::Set,
                TimingData::Accumulate => FireTiming::Accumulate,
                TimingData::CarryOver => FireTiming::CarryOver,
            },
            per_press: t.per_press,
            latency: t.latency,
            postfire: t.postfire,
        });
    }
    if let (Some(m), Some(a)) = (&d.magazine, &d.ammo) {
        e.insert(Magazine {
            clip: m.clip,
            size: m.clip,
            reserve: a.max,
            reserve_max: a.max,
            reload_time: m.reload,
            reload_while_held: m.reload_while_held,
            auto_reload: m.auto_reload,
        });
        if let Some(s) = m.shells {
            e.insert(ShellReload {
                start: s.start,
                insert: s.insert,
                finish: s.finish,
                back_to_back: s.back_to_back,
                insert_first: s.insert_first,
            });
        }
    }
    if let Some(h) = &d.hitscan {
        e.insert(Hitscan {
            range: h.range * len,
            pellets: h.pellets,
            spread: match h.spread {
                None => SpreadShape::Disc {
                    inaccuracy: 0.0,
                    spread: 0.0,
                },
                Some(SpreadData::Template(s)) => SpreadShape::Template(s),
                Some(SpreadData::Disc { inaccuracy, spread }) => SpreadShape::Disc { inaccuracy, spread },
                Some(SpreadData::Cone(s)) => SpreadShape::Cone(s),
            },
            punch_scale: h.punch_scale,
            tracer_every: h.tracer_every,
            redrawn_through_characters: h.redrawn_through_characters,
            odd_box: h.odd_box.map(|b| b * len),
        });
    }
    if let Some(x) = &d.damage {
        e.insert(DamageEffect {
            amount: x.amount * dmg,
            falloff: x.falloff,
            falloff_step: x.falloff_step * len,
            regions: super::hits::RegionScale::from_hitgroups(&HitgroupScale {
                head: x.hitgroups.head,
                chest: x.hitgroups.chest,
                stomach: x.hitgroups.stomach,
                arm: x.hitgroups.arm,
                leg: x.hitgroups.leg,
            }),
            impulse: x.impulse * len,
            quantum: x.quantum * dmg,
            armor_ratio: x.armor_ratio,
        });
    }
    if let Some(p) = &d.penetration {
        let (a_power, a_dist, a_len) = d.ammo.as_ref().map_or((0.0, f32::INFINITY, len), |a| {
            (a.power, a.max_distance, a.units.unwrap_or_default().length)
        });
        e.insert(Penetration {
            power: p.power.map_or(a_power * a_len, |x| x * len),
            objects: p.objects,
            max_distance: p.max_distance.map_or(a_dist * a_len, |x| x * len),
        });
    }
    if let Some(m) = &d.melee {
        let swing = |s: &SwingData| Swing {
            range: s.range * len,
            hull: s.hull.map(|h| h * len),
            hull_reach: s.hull_reach.unwrap_or(s.range) * len,
            facing_cos: s.facing_cos,
            refine: s.refine,
            damage: s.damage * dmg,
            follow_up: s.follow_up.map(|(t, x)| (t, x * dmg)),
            backstab: s.backstab.map(|x| x * dmg),
            hitgroups: s.hitgroups,
            hit_refire: s.hit_refire,
            miss_refire: s.miss_refire,
            miss_refire_other: s.miss_refire_other,
            force: s.force * len,
            kind: s.kind.unwrap_or(crate::core::DamageKind::Melee),
            armor_ratio: s.armor_ratio,
            quantum: s.quantum * dmg,
            sound_hit: s.sound_hit.clone(),
            sound_hit_world: s.sound_hit_world.clone(),
            sound_miss: s.sound_miss.clone(),
        };
        e.insert(Melee {
            primary: swing(&m.primary),
            secondary: m.secondary.as_ref().map(swing),
        });
    }
    if let Some(s) = &d.sounds {
        e.insert(WeaponSounds {
            fire: s.fire.clone(),
            fire_alt: s.fire_alt.clone(),
            empty: s.empty.clone(),
            deploy: s.deploy.clone(),
            reload: s.reload.clone(),
            draw: s.draw.clone(),
            shot: s.shot.clone(),
            finish: s.finish.clone(),
            modes: s.modes.clone(),
        });
    }
    match &d.alt {
        None => {}
        Some(AltData::Shot { pellets, rounds, cycle }) => {
            e.insert(AltShot {
                pellets: *pellets,
                rounds: *rounds,
                cycle: *cycle,
            });
        }
        Some(AltData::Scope {
            fov,
            max_speed,
            unzoom_after_shot,
            overlay,
            step,
            sound,
        }) => {
            let mut modes = AltModes::new(fov.len() as u8 + 1, *step, false);
            modes.sound = sound.clone();
            e.insert((
                modes,
                Zoom {
                    fov: fov.clone(),
                    max_speed: max_speed.map(|s| s * len),
                    unzoom_after_shot: *unzoom_after_shot,
                    scope: *overlay,
                },
            ));
        }
        Some(AltData::Silencer { time }) => {
            e.insert(AltModes::new(2, *time, true));
        }
        Some(AltData::Burst {
            interval,
            refire,
            count,
            step,
            texts,
            once_per_press,
            step_blocks_primary,
        }) => {
            if !texts.is_empty() {
                e.insert(ModeTexts(texts.iter().map(String::as_str).collect()));
            }
            e.insert((
                AltModes::new(2, *step, *step_blocks_primary),
                Burst {
                    mode: 1,
                    count: *count,
                    interval: *interval,
                    refire: *refire,
                    once_per_press: *once_per_press,
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
        id = "t:base"
        abstract = true
        units = { length = 0.5, damage = 0.01 }
        slot = 0
        draw = 1.0
        [trigger]
        automatic = true
        cycle = 0.1
        timing = "carry_over"
    "#;

    const AMMO: &str = r#"
        units = { length = 0.5 }
        [ammo."t:round"]
        power = 10.0
        max = 90
        box_rounds = 30
    "#;

    #[test]
    fn bases_merge_and_abstract_entries_are_templates() {
        let gun = r#"
            id = "t:gun"
            base = "t:base"
            [trigger]
            cycle = 0.2
            [magazine]
            clip = 30
            ammo = "t:round"
            reload = 2.0
        "#;
        let set = WeaponSet::parse(&[("base", BASE), ("ammo", AMMO), ("gun", gun)], &[]).unwrap();
        assert_eq!(set.weapons.len(), 1, "the template isn't a weapon");
        let g = set.get("t:gun").unwrap();
        let t = g.trigger.as_ref().unwrap();
        assert_eq!((t.automatic, t.cycle, t.timing), (true, 0.2, TimingData::CarryOver));
        assert_eq!(g.units.length, 0.5);
        assert_eq!(g.ammo.as_ref().unwrap().max, 90);
        // A later set builds on this one's entries.
        let set: &'static WeaponSet = Box::leak(Box::new(set));
        let more = r#"
            id = "t:other"
            base = "t:gun"
            slot = 1
        "#;
        let later = WeaponSet::parse(&[("other", more)], &[set]).unwrap();
        assert_eq!(later.get("t:other").unwrap().magazine.as_ref().unwrap().clip, 30);
    }

    #[test]
    fn mistakes_are_load_errors() {
        let err = |docs: &[(&str, &str)]| WeaponSet::parse(docs, &[]).unwrap_err();
        let e = err(&[("a", "id = \"t:a\"\nbase = \"t:none\"\nslot = 0\ndraw = 1.0")]);
        assert!(e.contains("unknown base t:none"), "{e}");
        let e = err(&[("a", "id = \"t:a\"\nslot = 0\ndraw = 1.0\nslott = 1")]);
        assert!(e.contains("slott"), "{e}");
        let e = err(&[(
            "a",
            "id = \"t:a\"\nslot = 0\ndraw = 1.0\n[magazine]\nclip = 1\nammo = \"t:x\"\nreload = 1.0",
        )]);
        assert!(e.contains("unknown ammo t:x"), "{e}");
        let e = err(&[(
            "a",
            "id = \"t:a\"\nslot = 0\ndraw = 1.0\n[anim.sequences]\nshoot = [\"fire\"]",
        )]);
        assert!(e.contains("shoot is not a view-model action"), "{e}");
        let e = err(&[
            ("a", "id = \"t:a\"\nbase = \"t:b\""),
            ("b", "id = \"t:b\"\nbase = \"t:a\""),
        ]);
        assert!(e.contains("loop"), "{e}");
    }

    #[test]
    fn unknown_hooks_and_bad_parameters_are_load_errors() {
        #[derive(Deserialize)]
        struct P {
            #[allow(dead_code)]
            n: u32,
        }
        let mut app = App::new();
        app.register_weapon_hook::<P>("t:hook", |_, _, _| {});
        let doc = |hook: &str| format!("id = \"t:a\"\nslot = 0\ndraw = 1.0\n[hooks.\"{hook}\"]\nn = 1");
        let e = app.add_weapon_data(&[("a", &doc("t:nope"))]).unwrap_err();
        assert!(e.contains("unknown hook t:nope") && e.contains("t:hook"), "{e}");
        let e = app
            .add_weapon_data(&[("a", "id = \"t:a\"\nslot = 0\ndraw = 1.0\n[hooks.\"t:hook\"]\nn = \"x\"")])
            .unwrap_err();
        assert!(e.contains("hook t:hook"), "{e}");
        app.add_weapon_data(&[("a", &doc("t:hook"))]).unwrap();
        assert!(app.world().resource::<WeaponRegistry>().find("t:a").is_some());
    }
}
