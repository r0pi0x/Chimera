//! What a player's gear changes (specs/combat_arms/gear_perks.md; section
//! numbers below are its): the vest (the selection's body record), helmet,
//! goggles or mask, backpack and gear mods give an armour pool and a list
//! of protection pieces (3, 4, 9: `weapon::Armor` with `weapon::ArmorPieces`,
//! `weapons::gear_armour`, whose armour step
//! the weapons and grenades already run), a speed penalty, sprint and
//! stamina bonuses (10: `movement::CaCharacter`) and, with the set items
//! and Fighting Chance, extra ammunition and damage (11, 12, 15).
//!
//! - `records`: the data (data/gear.toml, our defaults; the install's
//!   `Gear#`, `Model#`, `Hair#`, `Goggle#`, `Backpack#`, gear mods,
//!   `PerkSkill_N` and `SetItem#` records over them by id).
//! - The loadout (17): a player's gear choice is its `characters::
//!   CaLook` (`Selection`: body, head piece, goggle, backpack, which also
//!   dress the body) and its `CaLoadout` (gear mods, equipped items for
//!   set tiers, perks, the fast-respawn allowance). The local player's
//!   comes from the console (`CaOwnGear`: `ca_perks`, `ca_items`,
//!   `ca_head_mods`, `ca_face_mods`, `ca_vest_mods`, `ca_fast_respawns`,
//!   `ca_fast_respawns_max`, `ca_fast_pass`, `ca_skill_extender`,
//!   `ca_infected_item`), a client's arrives as `CaLoadoutRequest` (when
//!   its player appears and whenever it changes) and the server checks it
//!   (`validate`: unknown items dropped, perks that don't fit the slots
//!   refused, at most 3 mods a slot), keeps it (`CaClientLoadout`) and
//!   sends each player's to every client (`CaLoadout`, replicated).
//! - A ladder rung's character (`characters::CaRungLook`, replicated)
//!   keeps only what is intrinsic to its body, its body type (the
//!   record's `FigureType`, section 2: sprint, stamina and variable
//!   terms; `body_figure`); its vest (armour pool, protection piece,
//!   penalty, sprint and stamina bonuses, hit points, animation rates:
//!   section 3, the body record itself) is `RUNG_VEST`, the reference
//!   character's (`vest_of`, `gear_view`), so a drawn specialist or
//!   mercenary is no harder to kill and no faster than anyone else. The
//!   install's two body types have the same numbers, so every rung body
//!   plays as the stock one there.
//! - Every side derives the rest from the replicated look and loadout,
//!   so a client predicts its own player with the same numbers
//!   (`derive`): `CaCharacter` (the body type's figure and every worn item's
//!   penalty, sprint and stamina terms, set speed and stamina, Fighting
//!   Chance's 1.03) and `CaGearEffects` (sets, Fighting Chance).
//! - The animation rates (`CaGearRates`, `weapon_rates`): the vest's
//!   `ReloadAniRate` and `SelectAniRate` and the sets' fire, reload and
//!   draw rates go into each held weapon's own reload, draw and cycle
//!   (attachments.md "Animation rates"), on every side from the
//!   replicated look and loadout.
//! - The records go to network clients whose own differ (`data::served`
//!   `GEAR`: `GearData::to_doc`), so both sides derive the same speed
//!   penalties and rates.
//! - The server builds the armour (`armour`): at every spawn the pool is
//!   the vest's `MaxArmor` and the piece list is vest, head, face,
//!   goggle, then the head, face and vest mods' pieces, the set
//!   protection first; a changed look or loadout while alive refills the
//!   list and repairs the pool by each piece's `Armor` (capped; case 24).
//!   A character wearing an untouched default armour gets its gear's.
//! - Ammunition (`ammo`): a weapon's maximum total is raised by Fighting
//!   Chance and the sets' extra magazines; its starting total is raised by
//!   Fighting Chance only (Corrections 2026-10-22, 1: the server counts
//!   `SpawnedAmount`; the original's client shows the maximum, so extra
//!   magazines are shown but unreal there. We show the real count, the
//!   spec's recommendation; the M-23F and G7A likewise stay 1 and 1).
//!   `max_armour` is the vest's `MaxArmor` (Resourceful's medikit).
//!
//! Known shortcuts (docs/tech-debt.md; the most literal reading where the
//! spec is unclear):
//! - Our gear data is gear.toml (the spec's stock tables; the mercenary
//!   bodies and mod ids are examples, the Worldcup masks one template);
//!   the install's records replace ours by id when the V1 key is set, with
//!   the shop table (`ITEMS.TXT`) unread: perk items, slots and the
//!   function items come from our data (open question 1: the Expert item
//!   wins when both levels are owned).
//! - The loadout is console settings until the lobby exists: no rental
//!   expiry, no account flags, and the server trusts the client's
//!   fast-respawn counts up to a sanity cap (the lobby's counter would
//!   be the server's).
//! - Set items: the effect kinds 0-4, 6, 9-15 are applied (damage,
//!   protection, speed, extra magazines, animation rates, stamina,
//!   spread's max); kind 5 (protection against AI attackers) counts as 4;
//!   the rewards (7, 8) are read and unused. A per-weapon bonus
//!   names its weapon through `[[item]]` (ours; the stock sets are the
//!   install's). Set speed and stamina fold into the worn items' terms
//!   (the penalty clamp comes after the set's speed rather than before).
//! - The face piece (`Face#` `GearId`) is none: no stock face has one.
//!   Infection overrides' gear (`Overidable_Model#`) isn't read; gear mods
//!   are dropped while infected.
//! - Mass isn't used (every stock body is 2000). Health: a vest's
//!   `MaxHitPoints` sets the player's maximum at spawn.
//! - Quirk 1 (ids sent as bytes) is `CaGearQuirks::byte_ids` (the
//!   console's `ca_gear_byte_ids`, off): the vest, face and goggle
//!   piece ids wrap to a byte and "none" is 255, only when the data has
//!   more than 255 pieces (the US data's 258; ours has pieces 255 and
//!   257 for it).

pub mod records;

use std::sync::Arc;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use self::records::{GearData, GearMod, ModSlot, SetDef, SetEffect};
use super::{
    characters::{CaRungLook, records::Selection},
    data::CaData,
    movement::{CaCharacter, Figure, Gear as WornGear, MovementData},
    rules::CaMatchView,
    weapons::{Gear, HIT_POINTS, Protection, default_armour, gear_armour},
};
use crate::{
    core::{self, Health, Intent, LocalPlayer, MovementState, NetRole},
    net::{NetAppExt, ordered::OrderedAppExt, server::Player},
    weapon::{Armor, ArmorPieces, Magazine, Weapon},
};

/// Damage types (`InstDamageType`) the armour step sees; a set's
/// protection covers each.
const PROTECTED_TYPES: [i32; 9] = [2, 3, 7, 9, 13, 34, 38, 40, 41];
/// The room key of the infection mode (Fighting Chance is its).
pub const QUARANTINE: &str = "QRT";
/// Fighting Chance's rates (15): ammunition, damage, speed, stamina.
pub const FC_AMMO: f32 = 1.25;
pub const FC_DAMAGE: f32 = 1.03;
pub const FC_SPEED: f32 = 1.03;
pub const FC_STAMINA: f32 = 1.03;
/// Calloused's heal rate (15): `HPRecoverRate`.
pub const CALLOUSED_RATE: f32 = 2.0;
/// Most gear mods a slot holds (8).
pub const MODS_PER_SLOT: usize = 3;
/// Perk slots: 3, 4 with the extender (13).
pub const PERK_SLOTS: u8 = 3;
/// The server's sanity cap on a client's fast-respawn counts.
pub const FAST_CAP: u32 = 999;

/// Our own gear data (a read-only table): the lobby's defaults and the
/// tests'. An app's is `CaGearData`, from its `CaData`.
pub fn gear_data() -> &'static GearData {
    CaData::builtin_static().gear()
}

/// The gear data the systems use (replaceable: tests give it sets).
#[derive(Resource, Clone)]
pub struct CaGearData(pub Arc<GearData>);

/// Quirk 1: the vest, face and goggle piece ids go to the client as one
/// byte each (off by default; section 18's faithful mode).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaGearQuirks {
    pub byte_ids: bool,
}

/// What a player equipped besides the selection (17): the lobby's item
/// hand-over, as ours from the console.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct CaLoadout {
    /// Perk items (`ITEMS.TXT` numbers), in the order equipped.
    pub perks: Vec<i32>,
    /// Gear mods (`Mod#` ids) on the helmet, the face piece and the vest.
    pub head_mods: Vec<i32>,
    pub face_mods: Vec<i32>,
    pub vest_mods: Vec<i32>,
    /// Every equipped item number, for set tiers.
    pub items: Vec<i32>,
    /// Fast respawns left, the account's most, and the unlimited pass (16).
    pub fast_left: u32,
    pub fast_max: u32,
    pub premium: bool,
    /// The skill-slot extender function item (4 perk slots).
    pub extender: bool,
    /// An infected-mercenary item (Fighting Chance, 15).
    pub infected_item: bool,
}

impl CaLoadout {
    /// The perk slots this loadout has.
    pub fn perk_slots(&self) -> u8 {
        PERK_SLOTS + self.extender as u8
    }
}

/// The local player's loadout: the console's settings (`ca_perks` ...).
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct CaOwnGear {
    pub perks: String,
    pub items: String,
    pub head_mods: String,
    pub face_mods: String,
    pub vest_mods: String,
    pub fast_respawns: i32,
    pub fast_respawns_max: i32,
    pub fast_pass: i32,
    pub skill_extender: i32,
    pub infected_item: i32,
}

impl Default for CaOwnGear {
    fn default() -> Self {
        Self {
            perks: String::new(),
            items: String::new(),
            head_mods: String::new(),
            face_mods: String::new(),
            vest_mods: String::new(),
            fast_respawns: 0,
            fast_respawns_max: 0,
            fast_pass: 0,
            skill_extender: 0,
            infected_item: 0,
        }
    }
}

/// Item numbers from text ("16001 16010", commas too); bad words are
/// skipped.
pub fn parse_numbers(text: &str) -> Vec<i32> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .filter_map(|w| w.parse().ok())
        .collect()
}

impl CaOwnGear {
    /// The settings as a loadout (unchecked: `validate` checks it).
    pub fn loadout(&self) -> CaLoadout {
        CaLoadout {
            perks: parse_numbers(&self.perks),
            head_mods: parse_numbers(&self.head_mods),
            face_mods: parse_numbers(&self.face_mods),
            vest_mods: parse_numbers(&self.vest_mods),
            items: parse_numbers(&self.items),
            fast_left: self.fast_respawns.max(0) as u32,
            fast_max: self.fast_respawns_max.max(0) as u32,
            premium: self.fast_pass != 0,
            extender: self.skill_extender != 0,
            infected_item: self.infected_item != 0,
        }
    }
}

/// A client's loadout request (17, "Join"): sent when its player appears
/// and whenever it changes.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CaLoadoutRequest(pub CaLoadout);

/// The loadout a connected client sent (the server's, on the client's
/// entity, validated).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CaClientLoadout(pub CaLoadout);

/// The server's check of a loadout (17, "Validation"): unknown perks and
/// gear mods dropped, one entry an ability, perks that don't fit the
/// slots refused (the earlier ones stay), at most `MODS_PER_SLOT` mods
/// a slot and of the right slot, fast-respawn counts within the cap and
/// the account's most.
pub fn validate(data: &GearData, l: &CaLoadout) -> CaLoadout {
    let mut perks: Vec<i32> = Vec::new();
    let mut used = 0u8;
    let mut kinds: Vec<i32> = Vec::new();
    for &item in &l.perks {
        let Some(p) = data.perks.iter().find(|p| p.level_of(item).is_some()) else { continue };
        if let Some(at) = kinds.iter().position(|&t| t == p.ty) {
            // One entry an ability; owning both levels, the Expert item
            // wins (open question 1: the data doesn't say).
            if p.level_of(item) > p.level_of(perks[at]) {
                perks[at] = item;
            }
            continue;
        }
        if used + p.slots > l.perk_slots() {
            continue;
        }
        used += p.slots;
        kinds.push(p.ty);
        perks.push(item);
    }
    let mods = |list: &[i32], slot: ModSlot| -> Vec<i32> {
        list.iter()
            .copied()
            .filter(|id| data.mods.get(id).is_some_and(|m| m.slot == slot))
            .take(MODS_PER_SLOT)
            .collect()
    };
    let mut items: Vec<i32> = Vec::new();
    for &i in l.items.iter().take(64) {
        if !items.contains(&i) {
            items.push(i);
        }
    }
    let fast_max = l.fast_max.min(FAST_CAP);
    CaLoadout {
        perks,
        head_mods: mods(&l.head_mods, ModSlot::Head),
        face_mods: mods(&l.face_mods, ModSlot::Face),
        vest_mods: mods(&l.vest_mods, ModSlot::Vest),
        items,
        fast_left: l.fast_left.min(fast_max),
        fast_max,
        premium: l.premium,
        extender: l.extender,
        infected_item: l.infected_item,
    }
}

/// Whether `perks` (item numbers) fit in `slots` perk slots (section 13;
/// test case 52).
pub fn perks_fit(data: &GearData, perks: &[i32], slots: u8) -> bool {
    let mut used = 0u8;
    for &item in perks {
        let Some(p) = data.perks.iter().find(|p| p.level_of(item).is_some()) else {
            return false;
        };
        used += p.slots;
    }
    used <= slots
}

// ---------------------------------------------------------------------------
// Set items (12)

/// What the worn set items give (the tiers reached, summed).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetBonus {
    /// Damage dealt +v (kinds 0-3), player-wide.
    pub damage: f32,
    /// Protection (kinds 4, 5) with the locations it covers (none:
    /// everywhere).
    pub protection: Vec<(f32, Vec<u8>)>,
    /// Speed +v (6), stamina +v (13), extra magazines (9), spread's
    /// maximum cut (14).
    pub speed: f32,
    pub stamina: f32,
    pub magazines: f32,
    pub spread: f32,
    /// Fire, reload and draw animation rates +v (10, 11, 12): the
    /// attachments' `f_set`, `r_set`, `s_set`.
    pub fire_rate: f32,
    pub reload_rate: f32,
    pub select_rate: f32,
    /// The bonuses of a condition list that name a weapon's item: (item
    /// number, effect kind, value).
    pub per_item: Vec<(i32, i32, f32)>,
}

impl SetBonus {
    /// The damage bonus for a weapon (kinds 0-3 named on its item).
    pub fn weapon_damage(&self, data: &GearData, weapon: &str) -> f32 {
        self.per_item
            .iter()
            .filter(|(item, kind, _)| (0..=3).contains(kind) && data.items.get(item).is_some_and(|w| w == weapon))
            .map(|(_, _, v)| v)
            .sum()
    }

    /// The extra magazines for a weapon: the player-wide sum and the
    /// weapon's own (9).
    pub fn weapon_magazines(&self, data: &GearData, weapon: &str) -> f32 {
        self.magazines
            + self
                .per_item
                .iter()
                .filter(|(item, kind, _)| *kind == 9 && data.items.get(item).is_some_and(|w| w == weapon))
                .map(|(_, _, v)| v)
                .sum::<f32>()
    }

    /// The fire, reload and draw rate bonuses for a weapon (10, 11, 12):
    /// the player-wide sums and the weapon's own.
    pub fn weapon_rates(&self, data: &GearData, weapon: &str) -> (f32, f32, f32) {
        let own = |kind: i32| -> f32 {
            self.per_item
                .iter()
                .filter(|(item, k, _)| *k == kind && data.items.get(item).is_some_and(|w| w == weapon))
                .map(|(_, _, v)| v)
                .sum()
        };
        (
            self.fire_rate + own(10),
            self.reload_rate + own(11),
            self.select_rate + own(12),
        )
    }
}

/// The gear's animation rate bonuses on a weapon (attachments.md
/// "Animation rates": the character model's `ReloadAniRate` and
/// `SelectAniRate`, the sets' fire, reload and draw rates): its owner's
/// vest (`Selection::model`) and set tiers (`CaGearEffects`), derived on
/// every side each predicted tick (`weapon_rates`) from the replicated
/// look and loadout, so a client predicts its own reload and draw on the
/// server's numbers. The attachments add them to the parts' sums
/// (`attachments::apply_fitted`, `apply_draw`); the view model plays at
/// the same rates.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaGearRates {
    pub fire: f32,
    pub reload: f32,
    pub select: f32,
}

impl CaGearRates {
    /// The rates a weapon `weapon` held by a player of `look` (None: the
    /// default body; `rung`: a rung's character, the baseline vest's,
    /// `vest_of`) with set bonuses `effects` gets.
    pub fn of(data: &GearData, look: Option<&Selection>, rung: bool, effects: Option<&CaGearEffects>, weapon: &str) -> Self {
        let (reload, select) = data.vest_rates(look.map_or(0, |l| vest_of(l, rung)));
        let (f, r, s) = effects.map_or((0.0, 0.0, 0.0), |e| e.set.weapon_rates(data, weapon));
        CaGearRates {
            fire: f,
            reload: reload + r,
            select: select + s,
        }
    }
}

/// Each weapon's `CaGearRates` from its owner (none: 0); in the predicted
/// tick, before the attachments apply them.
pub fn weapon_rates(
    data: Option<Res<CaGearData>>,
    owners: Query<(Option<&Selection>, Has<CaRungLook>, Option<&CaGearEffects>)>,
    mut weapons: Query<(&Weapon, &mut CaGearRates)>,
) {
    let Some(data) = data else { return };
    for (w, mut rates) in &mut weapons {
        let owner = w.owner.filter(|o| *o != Entity::PLACEHOLDER).and_then(|o| owners.get(o).ok());
        let want = owner.map_or_else(CaGearRates::default, |(look, rung, effects)| {
            CaGearRates::of(&data.0, look, rung, effects, w.id)
        });
        if *rates != want {
            *rates = want;
        }
    }
}

/// The tiers the equipped `items` reach (12): per set the last tier whose
/// threshold the count of different listed items meets; an effect with a
/// condition list is player-wide when one of its items is worn gear (not
/// a weapon) and a per-weapon bonus for each of its weapon items.
pub fn resolve_sets(data: &GearData, equipped: &[i32]) -> SetBonus {
    let mut out = SetBonus::default();
    for set in &data.sets {
        let n = set.items.iter().filter(|i| equipped.contains(i)).count();
        let Some(tier) = set.tier.iter().rev().find(|t| t.count <= n && t.count > 0) else {
            continue;
        };
        for e in &tier.effect {
            apply_effect(data, equipped, e, &mut out);
        }
    }
    out
}

fn apply_effect(data: &GearData, equipped: &[i32], e: &SetEffect, out: &mut SetBonus) {
    let is_weapon = |i: &i32| data.items.contains_key(i);
    let player_wide = e.only.is_empty() || e.only.iter().any(|i| equipped.contains(i) && !is_weapon(i));
    if player_wide {
        match e.kind {
            0..=3 => out.damage += e.value,
            4 | 5 => out.protection.push((e.value, e.locations.clone())),
            6 => out.speed += e.value,
            9 => out.magazines += e.value,
            10 => out.fire_rate += e.value,
            11 => out.reload_rate += e.value,
            12 => out.select_rate += e.value,
            13 => out.stamina += e.value,
            14 => out.spread += e.value,
            _ => {}
        }
    }
    for item in e.only.iter().filter(|i| is_weapon(i)) {
        out.per_item.push((*item, e.kind, e.value));
    }
}

/// What a player's loadout does besides gear (derived on every side).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaGearEffects {
    /// Fighting Chance applies (a human of Quarantine with an
    /// infected-mercenary item).
    pub fighting_chance: bool,
    pub set: SetBonus,
}

impl CaGearEffects {
    /// A hit's damage (hit points) after Fighting Chance and the sets'
    /// bonus for `weapon` (per-tick order: attacker side first, 15, 12).
    pub fn dealt(&self, data: &GearData, weapon: &str, d: f32) -> f32 {
        let mut d = d;
        if self.fighting_chance && d > 0.0 {
            d += (FC_DAMAGE * d - d).max(1.0);
        }
        d * (1.0 + self.set.damage + self.set.weapon_damage(data, weapon))
    }
}

/// A weapon's maximum total after the gear (11): Fighting Chance, then
/// the sets' extra magazines (`magazines`, truncated, of `clip` rounds).
pub fn max_total(n: u32, clip: u32, fighting_chance: bool, magazines: f32) -> u32 {
    let mut n = n;
    if fighting_chance {
        let raised = (FC_AMMO * n as f32).floor() as u32;
        n += raised.saturating_sub(n).max(1);
    }
    n + magazines.max(0.0).trunc() as u32 * clip
}

// ---------------------------------------------------------------------------
// Resolving a look and a loadout

/// The vest every ladder rung's character wears (docs/DESIGN.md "Gun game
/// rules"): a fresh selection's, model 0's `Normal` (section 18: the
/// reference character, what the party's players wear without gear), so
/// a drawn body is no harder to kill and moves no differently than
/// anyone else.
pub const RUNG_VEST: i32 = 0;

/// The vest record `look`'s gear reads: its model (section 3: the body
/// record is the vest; a mercenary is a full-body vest), or `RUNG_VEST`
/// for a ladder rung's character (`rung`: `characters::CaRungLook`),
/// whose drawn body keeps only what is its own, its body type
/// (`body_figure`).
pub fn vest_of(look: &Selection, rung: bool) -> i32 {
    if rung { RUNG_VEST } else { look.model }
}

/// The body type `look`'s body has (section 2: its record's
/// `FigureType`; the install's `FigureType#`, `MovementData::figures`):
/// what is intrinsic to a body, kept on a rung too.
pub fn body_figure<'a>(data: &GearData, movement: &'a MovementData, look: &Selection) -> &'a Figure {
    let figure = data.vests.get(&look.model).or(data.vests.get(&0)).map_or(0, |v| v.figure);
    movement.figure_of(figure)
}

/// What `look` resolves with: the look its vest is read from (`vest_of`)
/// and its body type (`body_figure`).
pub fn gear_view<'a>(data: &GearData, movement: &'a MovementData, look: &Selection, rung: bool) -> (Selection, &'a Figure) {
    let figure = body_figure(data, movement, look);
    let vest = if rung {
        // A rung's pieces are drawn, not worn as gear (an NPC body's
        // head piece and goggles: characters.md 13.2): the baseline's.
        let d = Selection::default();
        Selection {
            model: vest_of(look, rung),
            head: d.head,
            goggle: d.goggle,
            backpack: d.backpack,
            ..*look
        }
    } else {
        Selection {
            model: vest_of(look, rung),
            ..*look
        }
    };
    (vest, figure)
}

/// What a selection and loadout come to.
#[derive(Clone, Debug)]
pub struct Resolved {
    /// The movement's figure and worn terms.
    pub character: CaCharacter,
    /// The armour pieces in the order they are added (the set's
    /// protection first).
    pub pieces: Vec<Gear>,
    /// The piece ids added (the set's protection has none).
    pub ids: Vec<i32>,
    /// The vest's `MaxArmor`.
    pub max_armor: f32,
    /// Hit points.
    pub max_hp: f32,
    /// The sum of the added pieces' `Armor` (what adding them repairs).
    pub repair: f32,
}

fn worn(name: &str, w: &records::Worn) -> WornGear {
    WornGear {
        name: name.to_string(),
        dash_speed: w.dash_speed,
        max_stamina: w.max_stamina,
        move_penalty: w.move_penalty,
        jump: 0.0,
    }
}

/// The set's protection as one piece (consumable, every damage type that
/// goes through the armour step); None without any.
fn set_piece(set: &SetBonus) -> Option<Gear> {
    if set.protection.is_empty() {
        return None;
    }
    let at = |loc: u8| -> f32 {
        set.protection
            .iter()
            .filter(|(_, l)| l.is_empty() || l.contains(&loc))
            .map(|(v, _)| v)
            .sum::<f32>()
            .clamp(0.0, 1.0)
    };
    let (head, torso, arm, leg) = (at(1), at(2), at(3), at(4));
    let entries: Vec<Protection> = PROTECTED_TYPES
        .iter()
        .map(|&ty| Protection {
            damage_type: ty,
            head,
            torso,
            arm,
            leg,
            consumable: true,
        })
        .collect();
    Some(Gear::new(&entries))
}

/// Quirk 1's piece ids (section 3 of Quirks): the vest, face and goggle
/// ids as bytes (none: 255), only when the data has more than 255 pieces.
fn quirk_id(data: &GearData, quirks: CaGearQuirks, id: i32) -> i32 {
    if quirks.byte_ids && data.piece_count > 255 {
        (id & 0xff) as i32
    } else {
        id
    }
}

/// A player's gear from its look and loadout.
pub fn resolve(
    data: &GearData,
    figure: &Figure,
    quirks: CaGearQuirks,
    look: &Selection,
    loadout: &CaLoadout,
    effects: &CaGearEffects,
    infected: bool,
) -> Resolved {
    let fallback = data.vests.get(&0);
    let vest = data.vests.get(&look.model).or(fallback);
    let head = data.heads.get(&look.head);
    let goggle = data.goggles.get(&look.goggle);
    let pack = data.backpacks.get(&look.backpack);
    let mut ids: Vec<i32> = Vec::new();
    // Vest, head, face, goggle (the byte path: vest, face and goggle).
    ids.push(quirk_id(data, quirks, vest.map_or(-1, |v| v.gear)));
    ids.push(head.map_or(-1, |h| h.gear));
    ids.push(quirk_id(data, quirks, -1));
    ids.push(quirk_id(data, quirks, goggle.map_or(-1, |g| g.gear)));
    let mods: Vec<&GearMod> = if infected {
        Vec::new()
    } else {
        let take = |list: &[i32], slot: ModSlot| -> Vec<&GearMod> {
            list.iter()
                .filter_map(|id| data.mods.get(id))
                .filter(|m| m.slot == slot)
                .take(MODS_PER_SLOT)
                .collect()
        };
        let mut all = take(&loadout.head_mods, ModSlot::Head);
        all.extend(take(&loadout.face_mods, ModSlot::Face));
        all.extend(take(&loadout.vest_mods, ModSlot::Vest));
        all
    };
    ids.extend(mods.iter().map(|m| m.gear));
    let mut pieces = Vec::new();
    let mut added = Vec::new();
    let mut repair = 0.0;
    if let Some(p) = set_piece(&effects.set) {
        pieces.push(p);
    }
    for id in ids {
        if let Some(p) = data.pieces.get(&id) {
            pieces.push(p.gear);
            added.push(id);
            repair += p.armor;
        }
    }
    // The movement's terms: the vest, then each worn item.
    let mut gear: Vec<WornGear> = Vec::new();
    if let Some(v) = vest {
        gear.push(WornGear {
            name: v.name.clone(),
            dash_speed: v.dash_speed,
            max_stamina: v.max_stamina,
            move_penalty: v.move_penalty,
            jump: 0.0,
        });
    }
    for (w, label) in [(head, "head"), (goggle, "goggle"), (pack, "backpack")] {
        if let Some(w) = w {
            gear.push(worn(if w.name.is_empty() { label } else { &w.name }, w));
        }
    }
    for m in &mods {
        gear.push(WornGear {
            name: m.name.clone(),
            dash_speed: m.dash_speed,
            max_stamina: m.max_stamina,
            move_penalty: m.move_penalty,
            jump: 0.0,
        });
    }
    // The set's speed (S_set adds to 1 - P) and stamina (M_set).
    if effects.set.speed != 0.0 || effects.set.stamina != 0.0 {
        gear.push(WornGear {
            name: "set".into(),
            dash_speed: 0.0,
            max_stamina: effects.set.stamina,
            move_penalty: -effects.set.speed,
            jump: 0.0,
        });
    }
    let figure: Figure = figure.clone();
    let max_hp = match vest {
        Some(v) if v.max_hp > 0.1 => v.max_hp,
        _ => figure_hp(),
    };
    let character = CaCharacter {
        figure,
        gear,
        speed_scale: if effects.fighting_chance { FC_SPEED } else { 1.0 },
        stamina_scale: if effects.fighting_chance { FC_STAMINA } else { 1.0 },
    };
    Resolved {
        character,
        pieces,
        ids: added,
        max_armor: vest.map_or(100.0, |v| v.max_armor),
        max_hp,
        repair,
    }
}

/// The figure types' `HP` (100 for both stock bodies).
fn figure_hp() -> f32 {
    100.0
}

/// The armour a freshly spawned player has: the pool at the vest's
/// `MaxArmor`, then the pieces added (their repair changes nothing).
pub fn spawn_armour(resolved: &Resolved) -> (Armor, ArmorPieces) {
    gear_armour(resolved.max_armor, &resolved.pieces)
}

/// A living player's armour after its gear changed: the list is
/// refilled and the pool repaired by the added pieces' `Armor`, capped
/// at the vest's maximum (section 9; test case 24).
pub fn rebuild_armour(old: &Armor, resolved: &Resolved) -> (Armor, ArmorPieces) {
    let points = old.amount * HIT_POINTS;
    gear_armour((points + resolved.repair).min(resolved.max_armor.max(points)), &resolved.pieces)
}

// ---------------------------------------------------------------------------
// The plugin

/// Marks the look and loadout a character's gear was last built from.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CaGearKey {
    look: Selection,
    /// A rung's character (`vest_of`).
    rung: bool,
    loadout: CaLoadout,
    effects: CaGearEffects,
    infected: bool,
}

/// Raised ammunition: what a weapon was given (11).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaAmmoRaised(pub u32);

/// Combat Arms' gear (a part of the network, `combat_arms_gear`).
pub struct CaGearPlugin;

impl Plugin for CaGearPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<CaGearData>() {
            let data = CaData::of(app).gear().clone();
            app.insert_resource(CaGearData(Arc::new(data)));
        }
        // A network client whose gear records differ takes the server's.
        let doc = super::data::served::gear_doc(&app.world().resource::<CaGearData>().0);
        app.server_docs(
            super::data::served::GEAR,
            doc,
            chimera_sim::server_docs::DocApply::With(super::data::served::apply_gear),
        );
        app.init_resource::<CaGearQuirks>()
            .init_resource::<CaOwnGear>()
            .net_part("combat_arms_gear", register)
            .add_systems(
                FixedUpdate,
                (
                    wear_loadouts.run_if(core::authoritative),
                    derive,
                    armour.run_if(core::authoritative),
                    ammo,
                )
                    .chain()
                    .after(core::SimSet::Weapons),
            );
        gear_cvars(app);
    }
}

/// The console's gear settings (`CaOwnGear`, `CaGearQuirks`).
pub fn gear_cvars(app: &mut App) {
    use crate::console::resource_cvar;
    resource_cvar::<CaOwnGear, String>(app, "combat_arms:ca_perks", "Your perk items, e.g. \"16001 16012\".", |g| &mut g.perks);
    resource_cvar::<CaOwnGear, String>(app, "combat_arms:ca_items", "Your equipped item numbers (set tiers).", |g| &mut g.items);
    resource_cvar::<CaOwnGear, String>(app, "combat_arms:ca_head_mods", "Gear mods on your helmet (up to 3).", |g| &mut g.head_mods);
    resource_cvar::<CaOwnGear, String>(app, "combat_arms:ca_face_mods", "Gear mods on your goggles or mask (up to 3).", |g| {
        &mut g.face_mods
    });
    resource_cvar::<CaOwnGear, String>(app, "combat_arms:ca_vest_mods", "Gear mods on your vest (up to 3).", |g| &mut g.vest_mods);
    resource_cvar::<CaOwnGear, i32>(app, "combat_arms:ca_fast_respawns", "Your fast respawns left (F7).", |g| &mut g.fast_respawns);
    resource_cvar::<CaOwnGear, i32>(app, "combat_arms:ca_fast_respawns_max", "Your fast respawns at most.", |g| {
        &mut g.fast_respawns_max
    });
    resource_cvar::<CaOwnGear, i32>(app, "combat_arms:ca_fast_pass", "1: unlimited fast respawns (the monthly pass).", |g| {
        &mut g.fast_pass
    });
    resource_cvar::<CaOwnGear, i32>(app, "combat_arms:ca_skill_extender", "1: a fourth perk slot.", |g| &mut g.skill_extender);
    resource_cvar::<CaOwnGear, i32>(app, "combat_arms:ca_infected_item", "1: an infected-mercenary item (Fighting Chance).", |g| {
        &mut g.infected_item
    });
    resource_cvar::<CaGearQuirks, bool>(
        app,
        "combat_arms:ca_gear_byte_ids",
        "1: the reference's byte-wide gear ids (quirk 1: everyone without a face or goggle piece gets piece 255).",
        |q| &mut q.byte_ids,
    );
}

fn register(app: &mut App) {
    app.replicate::<CaLoadout>()
        .ordered::<CaLoadout>()
        .add_client_message::<CaLoadoutRequest>(Channel::Ordered)
        .add_systems(
            PreUpdate,
            receive_loadouts
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            PostUpdate,
            send_loadout.before(ClientSystems::Send).run_if(resource_equals(NetRole::Client)),
        );
}

/// A client sends its loadout when its player appears and whenever it
/// changes.
fn send_loadout(
    own: Res<CaOwnGear>,
    me: Query<Entity, With<LocalPlayer>>,
    mut sent: Local<Option<(Entity, CaOwnGear)>>,
    mut out: MessageWriter<CaLoadoutRequest>,
) {
    let Ok(e) = me.single() else {
        *sent = None;
        return;
    };
    if sent.as_ref().is_none_or(|(s, g)| *s != e || *g != *own) {
        *sent = Some((e, own.clone()));
        out.write(CaLoadoutRequest(own.loadout()));
    }
}

/// The server keeps each client's loadout, checked.
fn receive_loadouts(
    mut requests: MessageReader<FromClient<CaLoadoutRequest>>,
    clients: Query<(), With<Player>>,
    data: Res<CaGearData>,
    mut commands: Commands,
) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        if clients.get(client).is_err() {
            continue;
        }
        commands.entity(client).insert(CaClientLoadout(validate(&data.0, &r.message.0)));
    }
}

/// Loadouts (the server's): the local player wears the console's, a
/// client's player the one its client sent; others (bots) have none.
#[allow(clippy::type_complexity)]
fn wear_loadouts(
    own: Res<CaOwnGear>,
    data: Res<CaGearData>,
    clients: Query<(&Player, &CaClientLoadout)>,
    characters: Query<(Entity, Option<&CaLoadout>, Has<LocalPlayer>), (With<Intent>, With<MovementState>)>,
    mut commands: Commands,
) {
    for (e, have, local) in &characters {
        let want = if local {
            Some(validate(&data.0, &own.loadout()))
        } else {
            clients.iter().find(|(p, _)| p.character == e).map(|(_, l)| l.0.clone())
        };
        let Some(want) = want else { continue };
        if have != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// The room key playing (the match view's).
fn mode_key(view: &CaMatchView) -> &str {
    &view.mode
}

/// Gear's derived state on every side: `CaGearEffects` and `CaCharacter`
/// from the look and loadout.
#[allow(clippy::type_complexity)]
fn derive(
    data: Res<CaGearData>,
    movement: Res<MovementData>,
    quirks: Res<CaGearQuirks>,
    view: Res<CaMatchView>,
    characters: Query<
        (
            Entity,
            &Selection,
            Option<&CaLoadout>,
            Option<&CaGearKey>,
            Option<&CaGearEffects>,
            Has<super::movement::CaInfected>,
            Has<CaCharacter>,
            Has<CaRungLook>,
        ),
        (With<Intent>, With<MovementState>),
    >,
    mut commands: Commands,
) {
    let quarantine = mode_key(&view) == QUARANTINE;
    for (e, look, loadout, key, effects, infected, has_character, rung) in &characters {
        let empty = CaLoadout::default();
        let loadout = loadout.unwrap_or(&empty);
        let want = CaGearEffects {
            fighting_chance: loadout.infected_item && quarantine && !infected,
            set: resolve_sets(&data.0, &loadout.items),
        };
        let now = CaGearKey {
            look: *look,
            rung,
            loadout: loadout.clone(),
            effects: want.clone(),
            infected,
        };
        if key == Some(&now) && effects == Some(&want) {
            continue;
        }
        let mut entity = commands.entity(e);
        if effects != Some(&want) {
            entity.insert(want.clone());
        }
        // A character that already had its own figure and gear when it
        // was first seen keeps it (the tests', the rules').
        if key.is_some() || !has_character {
            let (vest, figure) = gear_view(&data.0, &movement, look, rung);
            let resolved = resolve(&data.0, figure, *quirks, &vest, loadout, &want, infected);
            entity.insert(resolved.character);
        }
        entity.insert(now);
    }
}

/// The look and loadout a character's armour was last built from.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct CaGearBuilt(pub CaGearKey);

/// The armour (the server's): a character wearing an untouched default
/// armour gets its gear's, and a changed look or loadout refills the
/// list (section 9). Spawns do it through `spawn_armour`.
#[allow(clippy::type_complexity)]
fn armour(
    data: Res<CaGearData>,
    movement: Res<MovementData>,
    quirks: Res<CaGearQuirks>,
    mut characters: Query<(
        Entity,
        &Selection,
        &CaGearKey,
        &CaGearEffects,
        (&mut Armor, &mut ArmorPieces),
        Option<&mut Health>,
        Option<&CaGearBuilt>,
    )>,
    mut commands: Commands,
) {
    for (e, look, key, effects, (mut armor, mut pieces), health, built) in &mut characters {
        if built.is_some_and(|b| b.0 == *key) {
            continue;
        }
        commands.entity(e).insert(CaGearBuilt(key.clone()));
        let (vest, figure) = gear_view(&data.0, &movement, look, key.rung);
        let resolved = resolve(&data.0, figure, *quirks, &vest, &key.loadout, effects, key.infected);
        match built {
            // First seen: an untouched default armour becomes the gear's.
            None => {
                if (*armor, pieces.clone()) == default_armour() {
                    (*armor, *pieces) = spawn_armour(&resolved);
                    if let Some(mut h) = health
                        && (resolved.max_hp / 100.0 - h.max).abs() > 1e-6
                    {
                        h.max = resolved.max_hp / 100.0;
                        h.current = h.current.min(h.max);
                    }
                }
            }
            Some(_) => (*armor, *pieces) = rebuild_armour(&armor, &resolved),
        }
    }
}


/// Weapons raised to the gear's ammunition maximum when they first have
/// an owner with its gear derived (11).
#[allow(clippy::type_complexity)]
fn ammo(
    data: Res<CaGearData>,
    owners: Query<&CaGearEffects>,
    mut weapons: Query<(Entity, &Weapon, &mut Magazine, Option<&CaAmmoRaised>)>,
    mut commands: Commands,
) {
    for (e, w, mut mag, raised) in &mut weapons {
        let Some(owner) = w.owner else { continue };
        let Ok(effects) = owners.get(owner) else { continue };
        let total = mag.reserve_max + mag.size;
        let magazines = effects.set.weapon_magazines(&data.0, w.id);
        let delta = match raised {
            Some(r) => {
                // A change while the weapon is held: the cap moves, the
                // total waits for the next spawn or refill.
                let base = total - r.0.min(total);
                let want = max_total(base, mag.size, effects.fighting_chance, magazines) - base;
                if want != r.0 {
                    mag.reserve_max = (mag.reserve_max + want).saturating_sub(r.0);
                    commands.entity(e).insert(CaAmmoRaised(want));
                }
                continue;
            }
            None => max_total(total, mag.size, effects.fighting_chance, magazines) - total,
        };
        // The server's starting total is `SpawnedAmount` (the weapon's
        // own count here) after Fighting Chance only: the sets' extra
        // magazines raise the cap, not the count (Corrections
        // 2026-10-22, 1; the original's client shows the cap, a
        // fidelity option we leave out).
        let held = mag.clip + mag.reserve;
        let start = max_total(held, mag.size, effects.fighting_chance, 0.0).min(total + delta).saturating_sub(held);
        mag.reserve_max += delta;
        mag.reserve += start;
        commands.entity(e).insert(CaAmmoRaised(delta));
    }
}

/// A set definition, for tests and tools.
pub fn set(items: &[i32], tiers: Vec<(usize, Vec<SetEffect>)>) -> SetDef {
    SetDef {
        id: 0,
        items: items.to_vec(),
        tier: tiers
            .into_iter()
            .map(|(count, effect)| records::SetTier { count, effect })
            .collect(),
    }
}

/// A spawn's gear for `e` (section 9): the pool at the vest's `MaxArmor`,
/// the piece list refilled, and the vest's `MaxHitPoints` as the maximum
/// health (full). Without a look the default armour.
pub fn spawn_gear(world: &mut World, e: Entity) {
    let Some(look) = world.get::<Selection>(e).copied() else {
        world.entity_mut(e).insert(default_armour());
        return;
    };
    let loadout = world.get::<CaLoadout>(e).cloned().unwrap_or_default();
    let effects = world.get::<CaGearEffects>(e).cloned().unwrap_or_default();
    let infected = world.get::<super::movement::CaInfected>(e).is_some();
    let quirks = world.get_resource::<CaGearQuirks>().copied().unwrap_or_default();
    let Some(data) = world.get_resource::<CaGearData>().map(|d| d.0.clone()) else {
        world.entity_mut(e).insert(default_armour());
        return;
    };
    let Some(movement) = world.get_resource::<MovementData>() else {
        world.entity_mut(e).insert(default_armour());
        return;
    };
    let (vest, figure) = gear_view(&data, movement, &look, world.get::<CaRungLook>(e).is_some());
    let resolved = resolve(&data, figure, quirks, &vest, &loadout, &effects, infected);
    let mut ent = world.entity_mut(e);
    ent.insert(spawn_armour(&resolved));
    if let Some(mut h) = ent.get_mut::<Health>()
        && (resolved.max_hp / 100.0 - h.max).abs() > 1e-6
    {
        h.max = resolved.max_hp / 100.0;
        h.current = h.max;
    }
}

/// `e`'s armour pool maximum, the vest's `MaxArmor` (section 9): what
/// Resourceful's medikit repairs to. 100 without a look or gear data.
pub fn max_armour(world: &World, e: Entity) -> f32 {
    let Some(look) = world.get::<Selection>(e) else { return 100.0 };
    let loadout = world.get::<CaLoadout>(e).cloned().unwrap_or_default();
    let effects = world.get::<CaGearEffects>(e).cloned().unwrap_or_default();
    let infected = world.get::<super::movement::CaInfected>(e).is_some();
    let quirks = world.get_resource::<CaGearQuirks>().copied().unwrap_or_default();
    let (Some(data), Some(movement)) = (world.get_resource::<CaGearData>(), world.get_resource::<MovementData>()) else {
        return 100.0;
    };
    let (vest, figure) = gear_view(&data.0, movement, look, world.get::<CaRungLook>(e).is_some());
    resolve(&data.0, figure, quirks, &vest, &loadout, &effects, infected).max_armor
}
