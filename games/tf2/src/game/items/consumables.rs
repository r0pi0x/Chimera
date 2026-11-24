//! The consumables (specs/tf2/batch2b_mechanics.md 11-13): one "use and
//! recharge" item, the hook `tf2:consumable` (`ConsumableParams`, the
//! weapon part `Tf2Consumable`), and the conditions they give (Chimera's
//! `status::Conditions`). Three abstract bases carry it
//! (data/weapons/lunchbox.toml, lunchbox_drink.toml, jar.toml); an item's
//! `set_weapon_mode` and class pick its table there (`items::docs`).
//!
//! - **The meter** (`UseState::meter`, 0 to 1, full at the start): using
//!   empties it, and it refills linearly over `recharge` seconds (the
//!   item's `item_meter_charge_rate`: 30 s the Sandvich and the Steak, 10
//!   s the Dalokohs Bar and the Banana; a drink 30 s, a jar 20 s). A
//!   drink's meter drains over its effect (100 to 0 over the 8 s), then
//!   refills. Nothing is used before it is full.
//! - **Eating and drinking** (`use_items`, predicted, before the weapon
//!   switch): attack with a full meter is a taunt of `taunt` seconds
//!   (4.25 eating, 1.2 drinking): no moving, no jumping, no switching
//!   (`HolsterLocked`); then the primary is drawn (the Steak's melee). A
//!   lunch heals `heal` at each of `steps` (1, 2, 3, 4 s; the Banana's ×
//!   `lunchbox_healing_scale`); the Dalokohs Bar's first step adds
//!   `max_health` (+50) to its holder's max (`bonus`, `max_health_of`).
//!   An effect (`effect` s from `effect_from` s in) is the Steak's (from
//!   1 s for 19 s: the mini-crit buff, speed × 1.3 as Chimera's
//!   `SpeedBoost`, melee only: the melee drawn and held; damage taken ×
//!   `energy_buff_dmg_taken_multiplier`), Bonk's (from 0.9 s for 8 s:
//!   phased, no damage reaches him, no attacks) or Crit-a-Cola's (the
//!   mini-crit buff; each attack marks him for death for
//!   `mod_mark_attacker_for_death` s).
//! - **The thrown lunch** (attack2 with a full meter): a medium health kit
//!   (`LUNCH_KIT`, the map's medium kit's model) from the eye at
//!   `throw_speed` along the aim, falling; a teammate (or the Heavy
//!   himself) touching it takes `kit_share` of his max health
//!   (`effects::kits`); the primary is drawn.
//! - **Jars** (the frame throws them as a projectile, the item's own
//!   model: about 1000 forward and 200 up, full gravity; `jars`): the
//!   weapon before is drawn; it breaks on a player or the world, and
//!   every enemy whose centre is within `radius` (230) of the break gets
//!   its `condition` for `duration` (10 s): Jarate's (`JARATE`: hits on
//!   him mini-crit, `effects::crits`) or Mad Milk's (`MILK`: an attacker's
//!   damage on him heals the attacker `MILK_HEAL` of it, rounded down,
//!   `effects::after`); burning teammates in reach are put out.
//!
//! The utility items' gun-game flag is off (data/items.toml): they play
//! in TF2's own modes, never on a ladder.
//!
//! The Dalokohs Bar's bonus ends `BONUS_LIFE` (34.1 s) after the eat, or
//! at death; Bonk's phase is total immunity, his own blast too (both
//! measured, batch3_mechanics.md 6).
//!
//! Known shortcuts (the spec's open questions, provisional): eating at full
//! health eats; after a drink the primary is drawn as after a lunch;
//! the thrown kit is taken by its thrower only `KIT_OWNER_WAIT` after the
//! throw and by no enemy, lasts `effects::KIT_LIFE`, stops where it
//! first touches; a jar's break has no look or sound; the jar's reach
//! is measured to the victim's box centre with no line-of-sight rule;
//! `extinguish reduces cooldown` is not applied; Bonk's after-effect
//! (none measured) and the consumables' view-model sequences (their
//! arms idle) aren't modelled; the drinks' and lunches' own sounds
//! aren't played.

use std::sync::Arc;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::{
    status::{Condition, Conditions, SpeedBoost},
    weapon::{
        HolsterLocked, Inventory, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState,
        data::WeaponSet,
        projectile::{Flyer, FlyerGone, FlyerHit, LaunchRequest},
    },
};

use crate::core::{Damage, Health, Intent, MovementState, NetRole, SimClock, Team};
use crate::game::weapons::{damage::{Tf2Gun, U, round_points}, flame::Burning};

/// The hook.
pub const HOOK: &str = "tf2:consumable";
/// The Dalokohs Bar's + 50 max health lasts this long from the eat (s;
/// batch3_mechanics.md 6, measured).
pub const BONUS_LIFE: f64 = 34.1;
/// Jarate's and Mad Milk's conditions (`status::Condition::Game`).
pub const JARATE: &str = "tf2:jarate";
pub const MILK: &str = "tf2:milk";
/// Mad Milk: the attacker's share of his damage healed (rounded down).
pub const MILK_HEAL: f32 = 0.6;
/// The thrown lunch: its flyer's weapon id and model (shown by the
/// item's key, `kit_key`: a kept item keeps it).
pub const LUNCH_KIT: &str = "tf2:lunch_kit";
pub const LUNCH_KIT_MODEL: &str = "models/items/medkit_medium.mdl";
/// The model key of lunch item `id`'s thrown kit.
pub fn kit_key(id: &str) -> String {
    format!("{id}/kit")
}

/// The thrower takes his own kit only this long after the throw (s;
/// provisional).
pub const KIT_OWNER_WAIT: f64 = 1.0;
/// The slots drawn after use.
pub const PRIMARY_SLOT: u8 = 0;
pub const MELEE_SLOT: u8 = 2;
/// No gain cap on the Steak's speed (units/s).
const NO_CAP: f32 = 1.0e4;

/// Which consumable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Lunch,
    Drink,
    Jar,
}

/// `tf2:consumable`'s parameters (seconds, points, units; module doc).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumableParams {
    pub kind: Kind,
    /// The HUD meter's label.
    #[serde(default)]
    pub label: String,
    pub recharge: f32,
    #[serde(default)]
    pub taunt: f32,
    #[serde(default)]
    pub steps: Vec<f32>,
    #[serde(default)]
    pub heal: f32,
    #[serde(default)]
    pub max_health: f32,
    /// The effect: its start into the taunt and length.
    #[serde(default)]
    pub effect_from: f32,
    #[serde(default)]
    pub effect: f32,
    /// During the effect: the mini-crit buff, phase, speed, melee only.
    #[serde(default)]
    pub buff: bool,
    #[serde(default)]
    pub phase: bool,
    #[serde(default = "one")]
    pub speed: f32,
    #[serde(default)]
    pub melee_only: bool,
    /// Each attack in the effect marks for death this long.
    #[serde(default)]
    pub mark_attacker: f32,
    /// Damage taken in the effect.
    #[serde(default = "one")]
    pub taken_buffed: f32,
    #[serde(default)]
    pub throw_speed: f32,
    #[serde(default)]
    pub kit_share: f32,
    /// The jar's reach, condition and its length.
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub condition: Option<String>,
    /// The tables `items::docs` merges by mode and class (none left on an
    /// item's own document).
    #[serde(default)]
    pub modes: toml::Table,
    #[serde(default)]
    pub by_class: toml::Table,
}

fn one() -> f32 {
    1.0
}

impl ConsumableParams {
    /// The jar's condition, as a static name.
    pub fn condition(&self) -> Option<&'static str> {
        match self.condition.as_deref()? {
            JARATE => Some(JARATE),
            MILK => Some(MILK),
            _ => None,
        }
    }
}

/// The item's state (predicted; module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UseState {
    /// 0 to 1.
    pub meter: f32,
    /// The taunt's start, while it lasts.
    pub started: Option<f64>,
    /// The effect's (from, until).
    pub effect: Option<(f64, f64)>,
    /// The last lunch thrown at (its kit launched by the server once).
    pub thrown: Option<f64>,
    /// The jar's last throw seen.
    pub seen: f64,
    pub prev_fire: bool,
    pub prev_alt: bool,
}

/// The weapon part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Consumable {
    pub params: ConsumableParams,
    pub state: UseState,
}

impl NetPart for Tf2Consumable {
    type Net = UseState;
    fn net(&self) -> UseState {
        self.state
    }
    fn set_net(&mut self, net: UseState) {
        self.state = net;
    }
}

impl Tf2Consumable {
    /// The effect in force at `now` (its end).
    pub fn effect_at(&self, now: f64) -> Option<f64> {
        self.state.effect.filter(|(from, until)| now + 1e-6 >= *from && now < *until).map(|e| e.1)
    }

    /// The HUD meter: its label and fill.
    pub fn meter(&self) -> (&str, f32) {
        (&self.params.label, self.state.meter.clamp(0.0, 1.0))
    }
}

/// `tf2:consumable` on a weapon: full.
pub fn attach(e: &mut EntityWorldMut, p: &ConsumableParams) {
    e.insert(Tf2Consumable { params: p.clone(), state: UseState { meter: 1.0, seen: f64::MIN, ..default() } });
}

/// Every consumable weapon's numbers by id (for a thrown jar).
#[derive(Resource, Clone, Debug, Default)]
pub struct ConsumableRules(pub Vec<(String, Arc<ConsumableParams>)>);

impl ConsumableRules {
    pub fn from_set(set: &WeaponSet) -> Self {
        Self(
            set.weapons
                .iter()
                .filter(|w| !w.is_abstract)
                .filter_map(|w| Some((w.id.clone(), Arc::new(w.hooks.get(HOOK)?.clone().try_into::<ConsumableParams>().ok()?))))
                .collect(),
        )
    }

    pub fn get(&self, weapon: &str) -> Option<&ConsumableParams> {
        self.0.iter().find(|(id, _)| id == weapon).map(|(_, p)| &**p)
    }
}

/// The weapon of `inv` in `slot`.
fn in_slot(inv: &Inventory, weapons: &Query<&Weapon>, slot: u8) -> Option<Entity> {
    inv.weapons.iter().copied().find(|w| weapons.get(*w).is_ok_and(|w| w.slot == slot))
}

/// What the server has done of each lunch's use (by weapon): its taunt's
/// start, the heal steps given, the Dalokohs Bar's bonus, the last throw
/// launched. A resource, the server's (not the predicted part: a
/// client's prediction never changes it).
#[derive(Resource, Clone, Debug, Default)]
pub struct Eaten(pub bevy::platform::collections::HashMap<Entity, Eating>);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Eating {
    pub started: f64,
    pub steps: u8,
    pub bonus: f32,
    /// The bonus's end (sim time).
    pub bonus_until: f64,
    pub thrown: f64,
}

/// The max health `inv`'s consumables add (the Dalokohs Bar's bonus,
/// points).
pub fn max_health_of(inv: &Inventory, eaten: Option<&Eaten>) -> f32 {
    let Some(eaten) = eaten else { return 0.0 };
    inv.weapons.iter().filter_map(|w| eaten.0.get(w)).map(|e| e.bonus).sum()
}

/// Each holder's consumables: the meter, the taunt, the effect's movement
/// and weapon rules, the throws' switch (module doc), predicted, before
/// the weapon switch.
#[allow(clippy::type_complexity)]
pub fn use_items(
    mut owners: Query<(Entity, &mut Intent, &mut Inventory, Option<&Health>), Without<crate::map::interp::NetDrawn>>,
    mut items: Query<(&mut Tf2Consumable, &mut WeaponState, Option<&Tf2Gun>)>,
    slots: Query<&Weapon>,
    locked: Query<(), With<HolsterLocked>>,
    boosts: Query<&SpeedBoost>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let (now, dt) = (clock.now, clock.dt());
    for (owner, mut intent, mut inv, health) in &mut owners {
        if !inv.weapons.iter().any(|w| items.contains(*w)) {
            continue;
        }
        let dead = health.is_some_and(|h| h.current <= 0.0);
        let melee = in_slot(&inv, &slots, MELEE_SLOT);
        let primary = in_slot(&inv, &slots, PRIMARY_SLOT);
        let mut melee_held = false;
        for w in inv.weapons.clone() {
            let Ok((mut item, mut st, gun)) = items.get_mut(w) else { continue };
            let p = item.params.clone();
            let mut s = item.state;
            if dead {
                // Death: full again, nothing in force.
                s = UseState { meter: 1.0, seen: s.seen, thrown: s.thrown, ..default() };
                if locked.contains(w) {
                    commands.entity(w).remove::<HolsterLocked>();
                }
                if item.state != s {
                    item.state = s;
                }
                continue;
            }
            // The meter.
            match s.effect {
                Some((from, until)) if p.kind == Kind::Drink && now >= from && now < until => {
                    s.meter = ((until - now) / (until - from).max(1e-6)) as f32;
                }
                Some((from, _)) if p.kind == Kind::Drink && now < from => {}
                _ => s.meter = (s.meter + dt / p.recharge.max(1e-3)).min(1.0),
            }
            if s.effect.is_some_and(|(_, until)| now >= until) {
                s.effect = None;
            }
            let full = s.meter >= 1.0 - 1e-5;
            let active = inv.active == Some(w);
            let (fire, alt) = (intent.fire, intent.secondary);
            match p.kind {
                Kind::Jar => {
                    if let Some(t) = gun.map(|g| g.last_shot).filter(|t| *t > s.seen) {
                        s.seen = t;
                        s.meter = 0.0;
                        if active {
                            inv.wanted = inv.last.filter(|l| inv.weapons.contains(l) && *l != w);
                        }
                    }
                    if s.meter < 1.0 - 1e-5 {
                        let ready = now + ((1.0 - s.meter) * p.recharge) as f64;
                        if st.next_primary < ready {
                            st.next_primary = ready;
                        }
                    }
                }
                Kind::Lunch | Kind::Drink => {
                    if let Some(t) = s.started {
                        if now + 1e-6 >= t + p.taunt as f64 {
                            s.started = None;
                            commands.entity(w).remove::<HolsterLocked>();
                            let to = if p.melee_only && p.effect > 0.0 { melee } else { primary };
                            if active {
                                inv.wanted = to.filter(|t| *t != w);
                            }
                        } else {
                            intent.move_axis = Vec2::ZERO;
                            intent.jump = false;
                            intent.fire = false;
                            intent.secondary = false;
                            if !locked.contains(w) {
                                commands.entity(w).insert(HolsterLocked);
                            }
                        }
                    } else if active && inv.ready(now) && full && s.effect.is_none() {
                        if fire && !s.prev_fire {
                            s.started = Some(now);
                            if p.kind == Kind::Lunch {
                                s.meter = 0.0;
                            }
                            if p.effect > 0.0 {
                                let from = now + p.effect_from as f64;
                                s.effect = Some((from, from + p.effect as f64));
                            }
                            intent.move_axis = Vec2::ZERO;
                            intent.jump = false;
                            commands.entity(w).insert(HolsterLocked);
                        } else if alt && !s.prev_alt && p.kit_share > 0.0 {
                            s.meter = 0.0;
                            s.thrown = Some(now);
                            inv.wanted = primary.filter(|t| *t != w);
                        }
                    }
                }
            }
            s.prev_fire = fire;
            s.prev_alt = alt;
            // The effect's rules on its holder.
            if let Some((from, until)) = s.effect
                && now + 1e-6 >= from
                && now < until
            {
                if p.speed != 1.0 {
                    let b = SpeedBoost { until, mult: p.speed, cap: NO_CAP };
                    if boosts.get(owner).ok() != Some(&b) {
                        commands.entity(owner).insert(b);
                    }
                }
                if p.melee_only && s.started.is_none() {
                    melee_held = true;
                    if inv.active != melee && inv.wanted.is_none() {
                        inv.wanted = melee;
                    }
                }
                if p.phase && inv.next_attack < until {
                    inv.next_attack = until;
                }
            }
            if item.state != s {
                item.state = s;
            }
        }
        // The Steak's melee stays out while it lasts.
        if let Some(m) = melee {
            match (melee_held, locked.contains(m)) {
                (true, false) if inv.active == Some(m) => {
                    commands.entity(m).insert(HolsterLocked);
                }
                (false, true) => {
                    commands.entity(m).remove::<HolsterLocked>();
                }
                _ => {}
            }
        }
    }
}

/// What the consumables do on the server (module doc): the heal steps
/// and the Dalokohs Bar's bonus, the effects' conditions, Crit-a-Cola's
/// mark on each attack, the thrown lunch.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn effects(
    mut owners: Query<(Entity, &Inventory, &mut Health, &Transform, Option<&Intent>, Option<&MovementState>)>,
    items: Query<(&Tf2Consumable, &Weapon)>,
    mut events: MessageReader<WeaponEvent>,
    mut conditions: ResMut<Conditions>,
    mut eaten: ResMut<Eaten>,
    mut launch: MessageWriter<LaunchRequest>,
    weapons: Query<(), With<Weapon>>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    eaten.0.retain(|w, _| weapons.contains(*w));
    let attacked: Vec<Entity> = events
        .read()
        .filter(|e| !e.replay && matches!(e.kind, WeaponEventKind::ShotMade { .. } | WeaponEventKind::Swing { .. }))
        .map(|e| e.owner)
        .collect();
    for (owner, inv, mut h, at, intent, state) in &mut owners {
        if h.current <= 0.0 {
            // A death ends the bonus.
            for w in &inv.weapons {
                eaten.0.remove(w);
            }
            continue;
        }
        for &w in &inv.weapons {
            let Ok((item, weapon)) = items.get(w) else { continue };
            let p = &item.params;
            let s = item.state;
            let mut e = eaten.0.get(&w).copied().unwrap_or(Eating { started: f64::MIN, thrown: f64::MIN, ..default() });
            // The heal steps.
            if let Some(t) = s.started {
                if e.started != t {
                    e.started = t;
                    e.steps = 0;
                }
                while (e.steps as usize) < p.steps.len() && now + 1e-6 >= t + p.steps[e.steps as usize] as f64 {
                    e.steps += 1;
                    // The bonus goes on the max at once (the class layer
                    // then counts it: `max_health_of`), the health kept.
                    if p.max_health > 0.0 && e.bonus == 0.0 {
                        e.bonus = p.max_health;
                        h.max += p.max_health / 100.0;
                    }
                    if p.max_health > 0.0 {
                        e.bonus_until = t + BONUS_LIFE;
                    }
                    if p.heal > 0.0 && h.current < h.max {
                        h.current = (h.current + p.heal / 100.0).min(h.max);
                    }
                }
            }
            // The bonus ends `BONUS_LIFE` after the eat (the class layer
            // then lowers the max: `max_health_of`).
            if e.bonus > 0.0 && now + 1e-6 >= e.bonus_until {
                e.bonus = 0.0;
            }
            // The effect's conditions.
            if let Some((from, until)) = s.effect
                && now + 1e-6 >= from
                && now < until
            {
                if p.buff {
                    conditions.give(owner, Condition::MiniCritBuff, until, Some(owner));
                    if p.mark_attacker > 0.0 && attacked.contains(&owner) {
                        conditions.give(owner, Condition::MarkedForDeath, now + p.mark_attacker as f64, Some(owner));
                    }
                }
                if p.phase {
                    conditions.give(owner, Condition::Phased, until, Some(owner));
                }
            }
            // The thrown lunch, once.
            if let Some(t) = s.thrown
                && t != e.thrown
            {
                e.thrown = t;
                let eye = at.translation + state.map_or(Vec3::ZERO, |m| m.eye_offset);
                let dir = intent.map_or(at.rotation, |i| chimera_sim::weapon::aim_of(i.yaw, i.pitch)) * Vec3::NEG_Z;
                let mut f = Flyer::new(LUNCH_KIT, Some(owner), dir * p.throw_speed * U, &clock);
                f.gravity = 800.0 * U;
                f.half = Vec3::splat(4.0 * U);
                f.expires = Some(now + super::effects::KIT_LIFE as f64);
                f.pass_teammates = false;
                launch.write(LaunchRequest { flyer: f, start: eye, shown: Some(kit_key(weapon.id)), tag: None });
            }
            if eaten.0.get(&w) != Some(&e) {
                eaten.0.insert(w, e);
            }
        }
    }
}

/// Damage on a phased player is none; on a holder in a buffed effect ×
/// `taken_buffed` (module doc); before the damage lands.
pub fn taken(
    mut damage: MessageMutator<Damage>,
    conditions: Res<Conditions>,
    holders: Query<&Inventory>,
    items: Query<&Tf2Consumable>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for d in damage.read() {
        if conditions.has(d.target, Condition::Phased, now) {
            d.amount = 0.0;
            continue;
        }
        let Ok(inv) = holders.get(d.target) else { continue };
        let f: f32 = inv
            .weapons
            .iter()
            .filter_map(|w| items.get(*w).ok())
            .filter(|c| c.params.taken_buffed != 1.0 && c.effect_at(now).is_some())
            .map(|c| c.params.taken_buffed)
            .product();
        if f != 1.0 {
            d.amount = round_points(d.amount * f);
        }
    }
}

/// A jar breaking (module doc): its condition on the enemies in reach,
/// burning teammates put out.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn jars(
    mut hits: MessageReader<FlyerHit>,
    rules: Option<Res<ConsumableRules>>,
    chars: Query<(Entity, &Transform, Option<&avian3d::prelude::ColliderAabb>, Option<&MovementState>, &Health, Option<&Team>), With<Intent>>,
    burning: Query<(), With<Burning>>,
    friendly: Option<Res<crate::core::FriendlyFire>>,
    mut conditions: ResMut<Conditions>,
    mut gone: MessageWriter<FlyerGone>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let Some(rules) = rules else {
        hits.clear();
        return;
    };
    let now = clock.now;
    let ff = friendly.is_some_and(|f| f.0 != 0);
    let team = |e: Entity| chars.get(e).ok().and_then(|c| c.5.copied()).filter(|t| t.0 != 0);
    for hit in hits.read() {
        let Some(p) = rules.get(hit.weapon).filter(|p| p.kind == Kind::Jar) else { continue };
        let reach = p.radius * U;
        for (e, t, aabb, state, h, _) in &chars {
            if h.current <= 0.0 {
                continue;
            }
            let (min, max) = crate::game::weapons::blast::boxed(t, aabb, state);
            if ((min + max) / 2.0).distance(hit.point) > reach {
                continue;
            }
            let mate = Some(e) == hit.owner || (!ff && team(e).is_some() && hit.owner.and_then(team) == team(e));
            if mate {
                if burning.contains(e) {
                    commands.entity(e).remove::<Burning>();
                }
            } else if let Some(c) = p.condition() {
                conditions.give(e, Condition::Game(c), now + p.duration as f64, hit.owner);
            }
        }
        gone.write(FlyerGone { flyer: hit.flyer, weapon: hit.weapon, owner: hit.owner, at: hit.point, expired: false });
        commands.entity(hit.flyer).despawn();
    }
}

/// Mad Milk's heal for a hit of `points` on a milked victim (points,
/// rounded down).
pub fn milk_heal(points: f32) -> f32 {
    (points * MILK_HEAL + 1e-4).floor()
}

/// Whether this machine runs the server's part (no network client).
pub fn serving(role: Option<&NetRole>) -> bool {
    role.is_none_or(|r| *r != NetRole::Client)
}

/// The hook, the part and the systems (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_sim::weapon::{CarriedAppExt, SelectWeapons, data::WeaponDataAppExt, projectile::FlyerSet};
    app.register_weapon_hook::<ConsumableParams>(HOOK, |e, _, p| attach(e, p))
        .init_resource::<Eaten>()
        .predicted_part::<Tf2Consumable>()
        .add_systems(crate::core::Predict::Select, use_items.before(SelectWeapons))
        .add_systems(
            FixedUpdate,
            (
                effects.after(crate::core::SimSet::Weapons).before(crate::core::apply_damage),
                taken.after(super::mods::taken).before(crate::core::apply_damage),
                jars.in_set(FlyerSet::Touch),
            )
                .run_if(crate::core::authoritative),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mad_milk_heals_sixty_per_cent_rounded_down() {
        assert_eq!(milk_heal(121.0), 72.0);
        assert_eq!(milk_heal(50.0), 30.0);
    }
}
