//! Combat Arms' rung extras (docs/DESIGN.md "Gun game rules"; Chimera's
//! `minigame::ladder::RungExtrasHook`, registered by `CaRungsPlugin`):
//! when a ladder is made, each Combat Arms rung draws, from the ladder's
//! seeded generator, what every player who reaches it gets alike:
//!
//! - `attachments`: a fitted set from the gun's accepted parts
//!   (`draw_parts_with`): per slot, the parts the gun's list names that
//!   can go on a weapon (`ModRecord::weapon_part`) in their own `Kinds`
//!   slot, but none in a slot the gun's modification record closes
//!   (`AttachParams::slot_options` 0, or 2: a built-in part; the install's
//!   FMG-9 lists sights and magazines but takes neither), nor in a slot
//!   with a default part (a sniper's scope, an event weapon's built-in
//!   parts: kept, `slot_candidates`); each slot gets one of them at its odds
//!   (`RungOdds`: data/attachments/rung_odds.toml, tuned by a definition's
//!   `rung_extras = { attachments = { sight = 0.5, max_parts = 1 } }`;
//!   sights most often, power parts least, at most two; Gold and Red
//!   are grades of a part, drawn like it), else keeps its
//!   stock part, drawn alike; given to the rung's gun as
//!   `attachments::CaRungParts` (fitting, `fit_given`, takes it over the
//!   player's own choice), named in the HUD's rung line by display name
//!   (`label`: the install's, `CaPartNames`, shortened, else readable
//!   record names; at most `LABEL_MAX` characters);
//! - `character`: a body for everyone on the rung
//!   (`characters::CaRungLook`, the look `wear_looks` dresses them in
//!   over their own, a bot's and a client's too), drawn from every player
//!   body the install has, specialists and mercenaries included
//!   (`CharacterData::rung_bodies`; without the install's models file the
//!   stock six), with that body's default face; it plays with the
//!   baseline vest and its own body type only (`gear::vest_of`: no
//!   drawn armour or speed); a body no stock group
//!   has loads when first worn (`characters::bodies_on_demand`) and poses
//!   on its skeleton's stock rig meanwhile (`models::alias_rig`);
//! - `melee`: a Combat Arms melee weapon (every weapon of ours with the
//!   knife's data, `inventory::KNIFE_HOOK`, in a set the definition allows: knives, the shovel, the
//!   machete, ...) in place of the kit's knife on the rung (Chimera's
//!   `runtime::KitMelee`). The knife finale's rung draws none of these: it
//!   keeps its knife, or is the infected's.
//!
//! - `infected`: a melee rung may become the infected's (game_modes.md
//!   M8's infected, tied to their hands; Chimera's
//!   `ladder::RungCharacter`): the infected are one choice in the melee
//!   pool, so only a rung of the ladder's melee band (its last rungs whose
//!   weapon is a melee weapon: the knife finale's, whichever game's knife
//!   it is; `RungDraw::melee`) can be theirs, at the odds of
//!   data/rung_melee.toml (`infected`, 0.25: `InfectedOdds`; tuned by a
//!   definition's `rung_extras = { infected = { odds = 0.5 } }`, 0: never),
//!   else it keeps its knife; with the infected's weapon in the data (a
//!   knife-class weapon with the `combat_arms:infected` table,
//!   `INFECTED`: the install's Quarantine hands, `data::infected_table`;
//!   the stock ones, records 191 and 190 of missions.md F4.1, first). A
//!   rung whose own weapon is such a hand is the infected's too. On it
//!   everyone holds the hands alone (no gun, no kit) and wears an
//!   infected body (`CharacterData::infected_bodies`, its override's
//!   face: `infected_look`); the next rung, up or down, makes them
//!   themselves again. They keep the gun game's baseline (the rung
//!   vest's armour and health, `gear::vest_of`; no `movement::CaInfected`,
//!   so no knock-back from bullets: S14). Arriving and leaving are
//!   observers of Chimera's `ladder::OnRung` on every machine (a network
//!   client's from the replicated rung): `CaRungInfected` on and off
//!   (`arrive`, `leave`), and becoming one is the infection's moment
//!   (`infection_moment`, where effects are shown; not for a player a
//!   machine first sees on the rung, `OnRung::from`): the infection sound
//!   (data/effects.toml `infection`, by the body's sex) and the change
//!   effect (`INFECTION_FX`, M8's `Quarantine_Exp_01`) at the player. A
//!   gun rung's melee draw never gives a hand.
//!
//! Each is on by default and turned off in the definition's `[rules]`
//! (`rung_extras = { attachments = false, character = false, melee =
//! false, infected = false }`, or `rung_extras = false` for all); off,
//! that part is as without this module. The draws are the server's; what they put on a
//! player replicates as the look, the fitted parts and the inventory
//! always do.
//!
//! Known shortcuts (docs/tech-debt.md): the attachment data states no
//! requirements or conflicts between parts (none to respect); the slot
//! options are the lobby's (gameplay checks only the list), read here so
//! a rung gives only what the game would let a player fit; a slot's
//! "none" is the weapon's stock part (fitting can't leave a slot with a
//! default empty); the odds don't weigh parts by grade (Gold and Red as
//! often as base); on a dedicated server (no front end) the rung line
//! names parts by readable record names; the rung's character keeps camouflage 0 and the
//! body's default face and head piece; a player keeps the last rung look
//! (and an infected its marker) if the ladder stops for a minigame of
//! other rules; the infected's override render styles (the virus blend,
//! the glow) and first-person hands aren't drawn (their eyes are: the
//! override face's `ClientFX#` groups, `effects::attached`); the infection sound's files
//! and radius are ours (the spec's auto-cast record isn't read), played
//! on each machine (the spec's is a server sound), one at a time on it.

use bevy::prelude::*;
use serde::Deserialize;
use chimera_sim::minigame::{
    auto::Rng,
    ladder::{OnRung, RungCharacter, RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook, RungOptions},
};

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};

use super::{
    GAME,
    inventory::UNIT,
    attachments::{ATTACHMENTS, AttachParams, BULLET, CaModTable, CaRungParts, MAGAZINE, MECH_SIGHT, MUZZLE, SIGHT, SLOTS},
    characters::{self, CaRungLook, models, records::Selection},
    data::CaData,
    effects::{CaFxRequest, FxKind, data::CaFxTables, sounds::CaSound},
    inventory::KNIFE_HOOK,
};
use crate::weapon::data::WeaponCatalog;

/// The option keys (`rung_extras = { <key> = false }`).
pub const ATTACHMENTS_KEY: &str = "attachments";
pub const CHARACTER_KEY: &str = "character";
pub const MELEE_KEY: &str = "melee";
pub const INFECTED_KEY: &str = "infected";

/// The weapon table marking an infected's weapon (`data::infected_table`).
pub const INFECTED: &str = "combat_arms:infected";
/// The infected as a rung's character (`ladder::RungCharacter::id`).
pub const INFECTED_CHARACTER: &str = "combat_arms:infected";
/// The infected's weapons of the stock Quarantine mission (missions.md
/// F4.1: infected 191, hosts 190), preferred when the install has them.
pub const STOCK_HANDS: [i32; 2] = [191, 190];
/// The change effect group (game_modes.md M8, the mission's
/// `PlayerToInfected` stock value).
pub const INFECTION_FX: &str = "Quarantine_Exp_01";

/// The extras' values: the rung's gun, its parts (comma-separated), the
/// body record, the infected's rung ("1").
pub const WEAPON: &str = "weapon";
pub const PARTS: &str = "parts";
pub const MODEL: &str = "model";
pub const INFECTED_VALUE: &str = "infected";

/// The hook.
pub const HOOK: RungExtrasHook = RungExtrasHook {
    game: GAME,
    options: &[ATTACHMENTS_KEY, CHARACTER_KEY, MELEE_KEY, INFECTED_KEY],
    melee: knives,
    characters,
    draw,
    apply,
};

/// A player who is a ladder rung's infected, on every machine (put on
/// and off by `arrive` and `leave` from Chimera's `ladder::OnRung`). Not
/// `movement::CaInfected`: no knock-back, no infected rules.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaRungInfected;

/// Combat Arms' rung extras on the ladder.
pub struct CaRungsPlugin;

impl Plugin for CaRungsPlugin {
    fn build(&self, app: &mut App) {
        app.add_rung_extras(HOOK).add_observer(arrive).add_observer(leave);
    }
}

/// Whether rung `on` makes its players the infected.
pub fn infected_rung(on: &OnRung) -> bool {
    on.character().is_some_and(|c| c.id == INFECTED_CHARACTER)
}

/// A player arrived on a rung: the infected's marker on, on theirs.
fn arrive(on: On<Insert<OnRung>>, rungs: Query<&OnRung>, mut commands: Commands) {
    if rungs.get(on.event_target()).is_ok_and(infected_rung) {
        commands.entity(on.event_target()).insert(CaRungInfected);
    }
}

/// A player left a rung (or is gone): the infected's marker off.
fn leave(on: On<Discard<OnRung>>, rungs: Query<&OnRung>, mut commands: Commands) {
    if rungs.get(on.event_target()).is_ok_and(infected_rung) {
        commands.entity(on.event_target()).try_remove::<CaRungInfected>();
    }
}

/// How often a melee rung is the infected's (data/rung_melee.toml,
/// `rung_extras = { infected = { odds = .. } }` in a definition).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InfectedOdds(pub f64);

impl InfectedOdds {
    /// The built-in odds (data/rung_melee.toml).
    pub const DATA: &str = include_str!("../../data/rung_melee.toml");

    /// The definition's (its `infected` table's `odds`, else the data's;
    /// 0 with the option off), within 0 and 1.
    pub fn of(options: &RungOptions) -> InfectedOdds {
        if !options.on(INFECTED_KEY) {
            return InfectedOdds(0.0);
        }
        let number = |v: &toml::Value| v.as_float().or_else(|| v.as_integer().map(|i| i as f64));
        let tuned = options.table(INFECTED_KEY).and_then(|t| t.get("odds")).and_then(number);
        let data = toml::from_str::<toml::Table>(Self::DATA)
            .unwrap_or_else(|e| panic!("rung_melee.toml: {e}"))
            .get("infected")
            .and_then(number)
            .unwrap_or_else(|| panic!("rung_melee.toml: no `infected` odds"));
        InfectedOdds(tuned.unwrap_or(data).clamp(0.0, 1.0))
    }
}

/// Whether weapon `id` is an infected's (its `INFECTED` table).
pub fn is_infected_weapon(catalog: &WeaponCatalog, id: &str) -> bool {
    catalog.get(id).is_some_and(|w| w.has_hook(INFECTED))
}

/// The infected's weapons a rung may give: every infected's weapon of
/// ours the definition allows, the stock Quarantine hands only when the
/// data has them (`STOCK_HANDS`).
pub fn infected_hands(world: &World, d: &RungDraw) -> Vec<&'static str> {
    let Some(catalog) = world.get_resource::<WeaponCatalog>() else { return Vec::new() };
    let prefix = format!("{GAME}:");
    let mut pool: Vec<&'static str> = Vec::new();
    for set in &catalog.sets {
        for w in &set.weapons {
            let Some(data) = catalog.get(&w.id) else { continue };
            let id: &'static str = &data.id;
            if id.starts_with(&prefix) && data.has_hook(INFECTED) && data.has_hook(KNIFE_HOOK) && d.def.allows_weapon(id) && !pool.contains(&id) {
                pool.push(id);
            }
        }
    }
    let stock: Vec<&'static str> = world
        .get_resource::<CaData>()
        .map(|data| STOCK_HANDS.iter().filter_map(|i| data.weapon_for_index(*i)).filter(|id| pool.contains(id)).collect())
        .unwrap_or_default();
    if stock.is_empty() { pool } else { stock }
}

/// Every knife-class weapon of ours (`inventory::KNIFE_HOOK`; the infected's
/// hands among them): melee, though the catalogue has no `melee` data for
/// them.
fn knives(catalog: &WeaponCatalog) -> Vec<&'static str> {
    let prefix = format!("{GAME}:");
    let mut out: Vec<&'static str> = Vec::new();
    for set in &catalog.sets {
        for w in &set.weapons {
            if let Some(data) = catalog.get(&w.id)
                && data.id.starts_with(&prefix)
                && data.has_hook(KNIFE_HOOK)
                && !out.contains(&data.id.as_str())
            {
                out.push(&data.id);
            }
        }
    }
    out
}

/// Every infected's weapon of ours (the hands): no melee rung's weapon of
/// their own, only the infected character's.
fn characters(catalog: &WeaponCatalog) -> Vec<&'static str> {
    let prefix = format!("{GAME}:");
    let mut out: Vec<&'static str> = Vec::new();
    for set in &catalog.sets {
        for w in &set.weapons {
            if let Some(data) = catalog.get(&w.id)
                && data.id.starts_with(&prefix)
                && data.has_hook(INFECTED)
                && !out.contains(&data.id.as_str())
            {
                out.push(&data.id);
            }
        }
    }
    out
}

/// A Combat Arms rung's draw.
fn draw(world: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    let mut x = RungExtras::default();
    x.values.push((WEAPON.into(), d.weapon.into()));
    // The infected's rung: its own weapon a hand, or (a melee rung) one
    // drawn at the infected's odds.
    let own = world.get_resource::<WeaponCatalog>().is_some_and(|c| is_infected_weapon(c, d.weapon));
    let hands = if own {
        Some(d.weapon)
    } else if d.melee {
        let odds = InfectedOdds::of(d.options).0;
        let pool = if odds > 0.0 { infected_hands(world, d) } else { Vec::new() };
        (!pool.is_empty() && rng.unit() < odds).then(|| pool[rng.below(pool.len())])
    } else {
        None
    };
    // Any other melee rung keeps its own weapon: nothing drawn.
    if d.melee && hands.is_none() {
        return None;
    }
    if let Some(hands) = hands {
        x.values.push((INFECTED_VALUE.into(), "1".into()));
        if let Some(m) = draw_infected_body(world, rng) {
            x.values.push((MODEL.into(), m.to_string()));
        }
        x.character = Some(RungCharacter {
            id: INFECTED_CHARACTER.into(),
            weapon: hands.into(),
        });
        return Some(x);
    }
    if d.options.on(ATTACHMENTS_KEY) {
        let parts = draw_parts(world, d.weapon, &RungOdds::of(d.options), rng);
        if !parts.is_empty() {
            x.label = Some(label(world.get_resource::<CaPartNames>(), &parts));
            x.values.push((PARTS.into(), parts.join(",")));
        }
    }
    if d.options.on(CHARACTER_KEY)
        && let Some(m) = draw_body(world, rng)
    {
        x.values.push((MODEL.into(), m.to_string()));
    }
    if d.options.on(MELEE_KEY) {
        x.melee = draw_melee(world, d, rng).map(str::to_string);
    }
    Some(x)
}

/// The odds of a rung's fitted parts (`RungOdds::DATA`, tuned by a
/// definition's `rung_extras = { attachments = { ... } }`): per slot the
/// chance of a part other than the gun's stock one, and the most such
/// parts a gun gets.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RungOdds {
    pub muzzle: f64,
    pub sight: f64,
    pub magazine: f64,
    pub bullet: f64,
    pub mech_sight: f64,
    pub max_parts: usize,
}

impl Default for RungOdds {
    fn default() -> Self {
        Self::built_in()
    }
}

impl RungOdds {
    /// The built-in odds (data/attachments/rung_odds.toml).
    pub const DATA: &str = include_str!("../../data/attachments/rung_odds.toml");

    /// `DATA` parsed (bad built-in data panics; the tests load it).
    pub fn built_in() -> RungOdds {
        Self::with(&toml::Table::new()).unwrap_or_else(|e| panic!("rung_odds.toml: {e}"))
    }

    /// `DATA` with every value `t` names in its place (an integer chance,
    /// `sight = 1`, reads as a number).
    pub fn with(t: &toml::Table) -> Result<Self, String> {
        let mut base: toml::Table = toml::from_str(Self::DATA).map_err(|e| e.to_string())?;
        for (k, v) in t {
            base.insert(k.clone(), v.clone());
        }
        for (k, v) in base.iter_mut() {
            if let toml::Value::Integer(i) = v
                && k != "max_parts"
            {
                *v = toml::Value::Float(*i as f64);
            }
        }
        toml::Value::Table(base).try_into().map_err(|e: toml::de::Error| e.to_string())
    }

    /// The odds a definition's `[rules]` give (`rung_extras = {
    /// attachments = { ... } }` over the built-in; bad values: logged, the
    /// built-in).
    pub fn of(options: &RungOptions) -> Self {
        let Some(t) = options.table(ATTACHMENTS_KEY) else { return Self::default() };
        Self::with(t).unwrap_or_else(|e| {
            warn!("[rules] rung_extras.attachments: {e}; the built-in odds");
            Self::default()
        })
    }

    /// Slot `slot`'s chance of a non-stock part.
    pub fn chance(&self, slot: usize) -> f64 {
        match slot {
            MUZZLE => self.muzzle,
            SIGHT => self.sight,
            MAGAZINE => self.magazine,
            BULLET => self.bullet,
            MECH_SIGHT => self.mech_sight,
            _ => 0.0,
        }
    }
}

/// The parts a rung fits to `weapon` (`draw_parts_with` on its data and
/// the app's attachment table).
pub fn draw_parts(world: &World, weapon: &str, odds: &RungOdds, rng: &mut Rng) -> Vec<String> {
    let Some(params) = world
        .get_resource::<WeaponCatalog>()
        .and_then(|c| c.get(weapon))
        .and_then(|w| w.hook::<AttachParams>(ATTACHMENTS))
    else {
        return Vec::new();
    };
    let table = world.get_resource::<CaModTable>().unwrap_or(CaModTable::built_in());
    draw_parts_with(table, &params, odds, rng)
}

/// The parts a slot of a weapon may take on a rung: those its accepted
/// list names that can go on a weapon (`ModRecord::weapon_part`) and
/// whose own `Kinds` is the slot (each once), but none when the weapon's
/// modification record says the slot takes no part or a built-in one
/// (`AttachParams::slot_options` 0 or 2: a sniper's own scope), nor when
/// the weapon names a default part for it (`DefaultScopeAttach` and
/// friends: a sniper's scope, an event weapon's built-in parts), which is
/// its stock and stays.
pub fn slot_candidates<'a>(table: &'a CaModTable, params: &AttachParams, slot: usize) -> Vec<&'a str> {
    if matches!(params.slot_options.get(slot), Some(0 | 2)) || params.defaults.get(slot).is_some() {
        return Vec::new();
    }
    let mut out: Vec<&str> = Vec::new();
    for a in &params.accepts {
        let Some(r) = table.find(a.name()).and_then(|i| table.get(i)) else { continue };
        if r.slot() != Some(slot) || !r.weapon_part() || out.iter().any(|c| c.eq_ignore_ascii_case(&r.name)) {
            continue;
        }
        out.push(&r.name);
    }
    out
}

/// A rung's parts for a weapon of `params` ("Which weapon accepts which",
/// `slot_candidates`), drawn alike from `rng`: the slots in the order of
/// their chance (highest first, then slot order), each with parts to take
/// getting one of them, evenly, at its chance (`RungOdds::chance`) while
/// fewer than `max_parts` are drawn, else keeping its stock part (the
/// weapon's default, or nothing). The parts in slot order.
pub fn draw_parts_with(table: &CaModTable, params: &AttachParams, odds: &RungOdds, rng: &mut Rng) -> Vec<String> {
    let mut order: Vec<usize> = (0..SLOTS).collect();
    order.sort_by(|a, b| odds.chance(*b).total_cmp(&odds.chance(*a)).then(a.cmp(b)));
    let mut picked: Vec<(usize, String)> = Vec::new();
    for slot in order {
        let candidates = slot_candidates(table, params, slot);
        if candidates.is_empty() {
            continue;
        }
        // Drawn whatever the cap, so one slot's draw doesn't move the
        // next one's.
        let roll = rng.unit();
        let pick = rng.below(candidates.len());
        if picked.len() < odds.max_parts && roll < odds.chance(slot) {
            picked.push((slot, candidates[pick].to_string()));
        }
    }
    picked.sort_by_key(|(slot, _)| *slot);
    picked.into_iter().map(|(_, n)| n).collect()
}

/// The attachments' display names by record `Name` (the install's
/// string table, `ui::install::part_names`; put here by the front end),
/// each shortened (`short_name`).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaPartNames(pub std::collections::BTreeMap<String, String>);

impl CaPartNames {
    /// From (record name, display name) pairs.
    pub fn new(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self(pairs.into_iter().map(|(k, v)| (k.to_lowercase(), short_name(&v))).collect())
    }

    /// Part `name`'s display name, shortened; without one, its record name
    /// made readable (`readable`).
    pub fn of(&self, name: &str) -> String {
        self.0.get(&name.to_lowercase()).cloned().unwrap_or_else(|| readable(name))
    }
}

/// The most characters of a rung's label (its parts, after the gun in the
/// HUD's rung line); past it, cut with an ellipsis.
pub const LABEL_MAX: usize = 48;

/// A rung's label: its parts' display names (`names`, else readable
/// record names), comma-separated, cut at `LABEL_MAX` characters.
pub fn label(names: Option<&CaPartNames>, parts: &[String]) -> String {
    let fallback = CaPartNames::default();
    let names = names.unwrap_or(&fallback);
    let text = parts.iter().map(|p| names.of(p)).collect::<Vec<_>>().join(", ");
    if text.chars().count() <= LABEL_MAX {
        return text;
    }
    let cut: String = text.chars().take(LABEL_MAX - 1).collect();
    format!("{}…", cut.trim_end_matches([',', ' ']))
}

/// A display name without its weapon-class suffix (`S1 Suppressor I (AR)`
/// is `S1 Suppressor I`).
pub fn short_name(text: &str) -> String {
    let t = text.trim();
    let t = match t.rfind(" (") {
        Some(i) if t.ends_with(')') => &t[..i],
        _ => t,
    };
    t.trim().to_string()
}

/// The weapon classes' tokens in record names (`Silencer_A_SMG_Gold`).
const CLASS_TOKENS: &[&str] = &["AR", "SMG", "SR", "MG", "Pistol", "SG", "Shotgun"];

/// A record name made readable when no display name is known:
/// `Silencer_A_SMG_Gold` is `Silencer A Gold`, `DotSight_Holosight_SMG`
/// `Holosight`, `MechSight_Aim_III_SMG` `Aim III`, `Bullet_PVP_Stun_I_SMG`
/// `Stun I bullet`.
pub fn readable(name: &str) -> String {
    let tokens: Vec<&str> = name.split('_').filter(|t| !CLASS_TOKENS.iter().any(|c| c.eq_ignore_ascii_case(t))).collect();
    let (head, rest) = tokens.split_first().map_or(("", &[][..]), |(h, r)| (*h, r));
    let rest: Vec<&str> = rest.iter().copied().filter(|t| !t.eq_ignore_ascii_case("PVP")).collect();
    match head.to_lowercase().as_str() {
        "dotsight" if !rest.is_empty() && rest[0].len() > 1 => rest.join(" "),
        "dotsight" => format!("Red dot {}", rest.join(" ")).trim().to_string(),
        "mechsight" => rest.join(" "),
        "bullet" => format!("{} bullet", rest.join(" ")).trim().to_string(),
        "magazine" => format!("Mag {}", rest.join(" ")).trim().to_string(),
        _ => tokens.join(" "),
    }
}

/// A body record from the rungs' pool (`CharacterData::rung_bodies`).
pub fn draw_body(world: &World, rng: &mut Rng) -> Option<i32> {
    let pool = world.get_resource::<CaData>()?.characters().rung_bodies();
    (!pool.is_empty()).then(|| pool[rng.below(pool.len())])
}

/// A body for the infected's rung (`CharacterData::infected_bodies`).
pub fn draw_infected_body(world: &World, rng: &mut Rng) -> Option<i32> {
    let pool = world.get_resource::<CaData>()?.characters().infected_bodies();
    (!pool.is_empty()).then(|| pool[rng.below(pool.len())])
}

/// A melee weapon of ours (a knife-class weapon: `inventory::KNIFE_HOOK`)
/// in a set the definition allows.
fn draw_melee(world: &World, d: &RungDraw, rng: &mut Rng) -> Option<&'static str> {
    let catalog = world.get_resource::<WeaponCatalog>()?;
    let prefix = format!("{GAME}:");
    let mut pool: Vec<&'static str> = Vec::new();
    for set in &catalog.sets {
        for w in &set.weapons {
            let Some(data) = catalog.get(&w.id) else { continue };
            let id: &'static str = &data.id;
            let knife = data.has_hook(KNIFE_HOOK) && !data.has_hook(INFECTED);
            if id.starts_with(&prefix) && knife && d.def.allows_weapon(id) && !pool.contains(&id) {
                pool.push(id);
            }
        }
    }
    (!pool.is_empty()).then(|| pool[rng.below(pool.len())])
}

/// The look a rung's body record `id` gives: that body, its default face.
pub fn look_of(world: &World, id: i32) -> Selection {
    let face = world.get_resource::<CaData>().map_or(0, |d| d.characters().default_face(id));
    Selection {
        model: id,
        face,
        ..Selection::default()
    }
}

/// `e` armed for a rung: a Combat Arms rung's parts and look on it, or
/// (another game's rung, or the option off) taken off: a bot back in its
/// own look.
fn apply(world: &mut World, e: Entity, extras: Option<&RungExtras>) {
    let parts = extras.and_then(|x| Some((x.value(WEAPON)?, x.value(PARTS)?)));
    match parts {
        Some((weapon, parts)) => {
            let parts = CaRungParts {
                weapon: weapon.to_string(),
                parts: parts.split(',').filter(|p| !p.is_empty()).map(str::to_string).collect(),
            };
            world.entity_mut(e).insert(parts);
        }
        None => {
            world.entity_mut(e).remove::<CaRungParts>();
        }
    }
    let model = extras.and_then(|x| x.value(MODEL)?.parse::<i32>().ok());
    let infected = extras.is_some_and(|x| x.value(INFECTED_VALUE).is_some());
    match model {
        Some(id) => {
            let look = match (infected, world.get_resource::<CaData>()) {
                (true, Some(data)) => data.characters().infected_look(id),
                _ => look_of(world, id),
            };
            if let Some(data) = world.get_resource::<CaData>() {
                models::alias_rig(data.characters(), id);
            }
            // Worn at once (not at `wear_looks`' next tick): the rung's
            // swap restarts the player's prediction (`core::rebase`) with
            // the look's gear in it.
            world.entity_mut(e).insert((CaRungLook(look), look));
        }
        None => {
            if world.entity_mut(e).take::<CaRungLook>().is_some() && world.get::<crate::core::Bot>(e).is_some() {
                let look = characters::bot_look_of(world.get::<Name>(e), e);
                world.entity_mut(e).insert(look);
            }
        }
    }
}

/// The infection's moment (an observer where effects are shown): a
/// player who arrives on the infected's rung from another rung
/// (`OnRung::from`: not one this machine first sees there) and wasn't the
/// infected gets the infection sound (data/effects.toml `infection`, the
/// female list for a female body: one file drawn uniformly, 3D at the
/// infection point, and only when the last one has finished:
/// `InfectionSound`) and the change effect (`INFECTION_FX`) at the
/// infection point: the floor straight below the player + 0.1 u, up +Y
/// (game_modes.md Corrections (2026-10-30) 1 and 2, `infection_point`).
#[allow(clippy::too_many_arguments)]
pub fn infection_moment(
    on: On<Insert<OnRung>>,
    players: Query<(&OnRung, &GlobalTransform, Option<&Selection>, Has<CaRungInfected>)>,
    tables: Option<Res<CaFxTables>>,
    data: Option<Res<CaData>>,
    (time, lengths, spatial): (Res<Time>, Option<Res<super::effects::CaSoundLengths>>, SpatialQuery),
    (mut playing, mut rng): (ResMut<InfectionSound>, Local<u32>),
    mut sounds: MessageWriter<CaSound>,
    mut fx: MessageWriter<CaFxRequest>,
) {
    let player = on.event_target();
    let Ok((rung, at, look, was)) = players.get(player) else { return };
    let Some(tables) = tables else { return };
    if !infected_rung(rung) || rung.from.is_none() || was {
        return;
    }
    let origin = at.translation();
    let filter = SpatialQueryFilter::from_excluded_entities([player])
        .with_mask(crate::core::SOLID_LAYERS.0 & !crate::core::ITEM_LAYER.0);
    let below = spatial.cast_ray(origin, Dir3::NEG_Y, 1000.0 * UNIT, true, &filter).map(|h| h.distance);
    let at = infection_point(origin, below);
    let female = look
        .zip(data.as_deref())
        .and_then(|(l, d)| d.characters().model(l.model).map(|m| m.figure == 1))
        .unwrap_or(false);
    let s = &tables.sounds;
    let files = if female && !s.infection_female.is_empty() { &s.infection_female } else { &s.infection };
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let roll = (*rng >> 8) as f32 / (1u32 << 24) as f32;
    if let Some(file) = super::effects::rules::draw_any(files, roll) {
        let length = lengths.as_deref().and_then(|l| l.0.get(file).copied()).unwrap_or(0.0);
        if playing.try_start(time.elapsed_secs_f64(), f64::from(length)) {
            sounds.write(CaSound::at(file.to_string(), at, s.infection_radius));
        }
    }
    fx.write(CaFxRequest {
        group: INFECTION_FX.into(),
        at,
        normal: Vec3::Y,
        kind: FxKind::Ammo,
    });
}

/// Where an infection's change effect and sound play (game_modes.md
/// Corrections (2026-10-30) 2): the player's position `origin` (m)
/// dropped by `below` (m: the distance to the first solid surface
/// straight below; None: none found, the position itself) plus 0.1 u.
pub fn infection_point(origin: Vec3, below: Option<f32>) -> Vec3 {
    match below {
        Some(d) => origin - Vec3::Y * d + Vec3::Y * (0.1 * UNIT),
        None => origin,
    }
}

/// The infection sound playing (until when, s): at most one at a time
/// (game_modes.md Corrections (2026-10-30) 1: like the hurt sounds, a new
/// one plays only once the last has finished).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct InfectionSound(pub f64);

impl InfectionSound {
    /// Start one of `length` s at `now` unless the last still plays.
    pub fn try_start(&mut self, now: f64, length: f64) -> bool {
        if now < self.0 {
            return false;
        }
        self.0 = now + length;
        true
    }
}
