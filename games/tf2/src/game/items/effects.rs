//! The batch-2 unlocks' rules (specs/tf2/batch2_mechanics.md), each built
//! once and named by an item's attributes on its `tf2:item` hook
//! (`super::mods::ItemParams`): a weapon is its data plus these.
//!
//! - **Crits and mini-crits** (§2; `crits`, on every hit before the
//!   damage-taken factors, `Conditions`): a mini-crit × 1.35, a crit × 3,
//!   on the hit as dealt, rounded. A hit mini-crits a victim whose active
//!   weapon marks him (`mark_self`: the Escape Plan), one in the air after
//!   another's blast pushed him (`mini_crit_airborne`: the Direct Hit,
//!   the Reserve Shooter; `blast::BlastLaunched`), one hit from behind and
//!   close (`back_attack`: within `BACK_RANGE` of the shooter's origin,
//!   the victim's facing within `BACK_ANGLE` of the line from the shooter
//!   to him: the Back Scatter), a burning one (`minicrit_burning`: the
//!   Detonator, the Scorch Shot); it crits a burning one (`crit_burning`:
//!   the Flare Gun's class rule) and anyone while its holder blast jumps
//!   (`crit_airborne`: the Market Gardener). `minicrits_crit` (the
//!   Bushwacka) makes every mini-crit a crit. The Axtinguisher's family
//!   (`consume_burning`) mini-crits a burning victim with its damage
//!   before the mini-crit `base_damage` × (`AXT_A` + `AXT_B` R), R the
//!   burn's seconds left, and puts the burn out; the Equalizer's
//!   (`damage_by_health`) is `base_damage` × (1.65 - 1.15 health / max).
//!   A blast asks `Conditions::blast_mini_crit` per victim (`blast`).
//!   A crit plays `TFPlayer.CritHit` at its victim (weapons_stock.md
//!   8.3; a melee weapon's own crit swing plays it itself).
//! - **After a hit** (`after`, once the damage landed): a bleeding hit
//!   (`bleed` s; §3) wounds its victim (Chimera's `status::Bleeding`,
//!   `BLEED_TICK` every `DOT_PERIOD`); a swing at nothing with
//!   `hit_self_on_miss` hits its holder for half its damage and wounds
//!   him; an enemy hit with `boost_on_hit` boosts its holder (Chimera's
//!   `status::SpeedBoost`, × `BOOST_MULT` capped at + `BOOST_CAP`); a
//!   swing on a teammate with `buff_ally` boosts both (`ALLY_BOOST`,
//!   `SELF_BOOST`); a flare's hit or explosion sets alight
//!   (`weapons::flare`'s burn, from its tick).
//! - **Health** (§1): the Black Box's heal per rocket from the enemy
//!   damage it dealt (`heal_on_radius` × min(1, D / its rocket's base),
//!   whole points down, to max); the Kunai's backstab kill
//!   (`sanguisuge`): the victim's health before the stab, up to
//!   `KUNAI_CAP` × max; a head per kill (`decapitate`, `Heads`): 15
//!   health past max, and the first `HEAD_CAP` each 15 max health and 8 %
//!   of the class's speed; the Candy Cane's small kit (`drop_kit`,
//!   `kits`): ⌈max × 0.2⌉ to whoever reaches it, curing burns and bleeds
//!   (classes.md, "Health kits"), gone `KIT_LIFE` after the kill.
//! - **Regeneration** (`regen`; classes.md, item_attributes.md 4.8): once
//!   a second the Medic's 3 to 6 and the items' `regen` × 0.25 to 1.0
//!   by the time since he was hurt, the fraction carried, to max.
//! - **Overheal** (`overheal`, `Overhealed`): health the Kunai or a head
//!   put above max falls at the normal overheal's rate for that max
//!   ((⌊1.5 max / 5⌋ × 5 - max) / 15 a second), a head's excess at
//!   `HEAD_DRAIN`.
//! - **Speeds** (`speed_factor`, `movement::select_class`): the speed
//!   boost, the Escape Plan's tiers by health, the heads.
//!
//! Known shortcuts (provisional numbers of the spec's open questions):
//! - the Axtinguisher's law is fitted on flare burns (7.5 s); the head's
//!   excess drains at the measured 28 a second (`HEAD_DRAIN`); heads are
//!   lost at death (unmeasured); the Black Box takes the whole rocket's
//!   damage once (not each victim's rounded share);
//! - the kit's life is `KIT_LIFE` (32 s, measured 31.3 to 33.3) and it is
//!   taken by a body reaching within `KIT_REACH` of it (no pickup box
//!   was measured); it shows the small kit's model;
//! - a mini-crit scales the hit as dealt, after the distance modifier (a
//!   bullet's past 512 units keeps its fall-off; the Back Scatter's reach
//!   ends at 512, where the modifier is 1); a mini-crit makes no sound
//!   and neither shows a text (no spec names them: the plan's spec
//!   list); the Escape Plan's mark has no icon;
//!   healing from Medics (×0.1) waits on a Medi Gun;
//! - an enemy Pyro hit by a flare burns `pyro_burn` (0.25 s) in look
//!   only, as from the Flame Thrower (weapons_more.md 7.3; that look
//!   isn't burning for the flares' crits: unspecified), and a Pyro in his
//!   own explosion takes one afterburn tick (the measured one; §4.3 says
//!   no more).

use bevy::{ecs::system::SystemParam, platform::collections::HashMap, prelude::*};

use super::mods::{ItemParams, Tf2Item};
use crate::core::{Damage, DamageKind, DamageTaken, Died, Health, Hitgroup, Intent, MovementState, SimClock, Team};
use crate::game::weapons::{
    blast::{BlastJumping, BlastLaunched, MINI_CRIT},
    damage::round_points,
    flame::Burning,
    melee::{BACKSTAB, CRIT_HIT, CRIT_MULT, Tf2Melee},
};
use crate::map::{GameSound, PlaySound};
use chimera_sim::{
    status::{Bleeding, SpeedBoost},
    weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind},
};

/// The speed boost (§5.1): × 1.4, at most + 105 of the class's speed.
pub const BOOST_MULT: f32 = 1.4;
pub const BOOST_CAP: f32 = 105.0;
/// The Axtinguisher's kill on a burning player (§2.5), the Disciplinary
/// Action's teammate and holder (§5.4): seconds of boost.
pub const AXT_BOOST: f32 = 3.0;
pub const ALLY_BOOST: f32 = 2.0;
pub const SELF_BOOST: f32 = 3.6;
/// Bleed (§3): points per tick, the period (whole ticks: 0.51 s).
pub const BLEED_TICK: f32 = 4.0;
pub const DOT_PERIOD: f32 = 0.5;
/// The Back Scatter (§2.3): reach (units, origin to origin) and angle.
pub const BACK_RANGE: f32 = 512.0;
pub const BACK_ANGLE: f32 = 75.0;
/// The Axtinguisher's fitted law (§2.5): × (A + B R) on its own damage.
pub const AXT_A: f32 = 1.389;
pub const AXT_B: f32 = 0.1447;
/// The Equalizer's law (§5.3).
pub const EQUALIZER_A: f32 = 1.65;
pub const EQUALIZER_B: f32 = 1.15;
/// Eyelander heads (§1.6).
pub const HEAD_HEALTH: f32 = 15.0;
pub const HEAD_SPEED: f32 = 0.08;
pub const HEAD_CAP: u32 = 4;
/// A head's excess over max falls this fast (points/s; provisional).
pub const HEAD_DRAIN: f32 = 28.0;
/// The Kunai's overheal cap (× max, §1.4).
pub const KUNAI_CAP: f32 = 3.0;
/// The small kit (classes.md; §1.5): its share of max health, its life
/// (provisional) and how near a body takes it (units, provisional).
pub const KIT_SHARE: f32 = 0.2;
pub const KIT_LIFE: f32 = 32.0;
pub const KIT_REACH: f32 = 24.0;
/// The dropped kit's flyer id and its model key and model.
pub const KIT: &str = "tf2:candy_cane";
pub const KIT_SHOWN: &str = "tf2:candy_cane/kit";
pub const KIT_MODEL: &str = "models/items/medkit_small.mdl";
/// The Medic's regeneration (classes.md): 3 a second, rising to 6 from 5
/// to 10 s after he was hurt.
pub const MEDIC: &str = "medic";
pub const MEDIC_REGEN: f32 = 3.0;

/// Every item weapon's numbers by id (for a hit whose weapon is no longer
/// in hand: a rocket, a flare).
#[derive(Resource, Clone, Debug, Default)]
pub struct ItemRules(pub Vec<(String, ItemParams)>);

impl ItemRules {
    pub fn from_set(set: &chimera_sim::weapon::data::WeaponSet) -> Self {
        Self(
            set.weapons
                .iter()
                .filter_map(|w| Some((w.id.clone(), w.hooks.get(super::ITEM_HOOK)?.clone().try_into::<ItemParams>().ok()?)))
                .collect(),
        )
    }

    pub fn get(&self, weapon: &str) -> Option<&ItemParams> {
        self.0.iter().find(|(id, _)| id == weapon).map(|(_, p)| p)
    }
}

/// What a hit's conditions read of the characters (module doc).
#[derive(SystemParam)]
pub struct Conditions<'w, 's> {
    chars: Query<
        'w,
        's,
        (
            Option<&'static MovementState>,
            Has<BlastJumping>,
            Has<BlastLaunched>,
            Option<&'static Burning>,
            Option<&'static Inventory>,
            &'static Transform,
            Option<&'static Intent>,
        ),
    >,
    items: Query<'w, 's, &'static Tf2Item>,
    rules: Option<Res<'w, ItemRules>>,
    donks: Option<Res<'w, Donks>>,
    clock: Res<'w, SimClock>,
}

impl Conditions<'_, '_> {
    /// An item weapon's numbers by id.
    pub fn item(&self, weapon: &str) -> Option<&ItemParams> {
        self.rules.as_deref()?.get(weapon)
    }

    /// `e`'s active weapon marks him (every hit on him mini-crits).
    pub fn marked(&self, e: Entity) -> bool {
        let Ok((.., Some(inv), _, _)) = self.chars.get(e) else { return false };
        inv.active.and_then(|w| self.items.get(w).ok()).is_some_and(|i| i.0.mark_self)
    }

    /// Cannonball `flyer` has hit `victim` already (`Donks`).
    pub fn donked(&self, flyer: Entity, victim: Entity) -> bool {
        self.donks.as_deref().is_some_and(|d| d.0.iter().any(|d| d.flyer == flyer && d.victim == victim))
    }

    /// `e` is in the air after another's blast (or his own).
    pub fn launched(&self, e: Entity) -> bool {
        self.chars.get(e).is_ok_and(|(s, jumping, launched, ..)| (jumping || launched) && s.is_some_and(|s| !s.on_ground))
    }

    /// `e` blast jumps on his own blast, in the air.
    pub fn blast_jumping(&self, e: Entity) -> bool {
        self.chars.get(e).is_ok_and(|(s, jumping, ..)| jumping && s.is_some_and(|s| !s.on_ground))
    }

    /// `e`'s burn (a harmful one with time left).
    pub fn burn(&self, e: Entity) -> Option<&Burning> {
        self.chars.get(e).ok()?.3.filter(|b| !b.harmless && b.left > 0.0)
    }

    /// `victim` is behind and close to `shooter` (§2.3).
    pub fn behind(&self, shooter: Entity, victim: Entity) -> bool {
        let (Ok(s), Ok(v)) = (self.chars.get(shooter), self.chars.get(victim)) else { return false };
        let to = v.5.translation - s.5.translation;
        if to.length() > BACK_RANGE * crate::game::weapons::damage::U {
            return false;
        }
        let facing = v.6.map_or(v.5.rotation, |i| i.yaw_rotation()) * Vec3::NEG_Z;
        let (a, b) = (facing.with_y(0.0).normalize_or_zero(), to.with_y(0.0).normalize_or_zero());
        a != Vec3::ZERO && b != Vec3::ZERO && a.angle_between(b).to_degrees() < BACK_ANGLE
    }

    /// Whether a blast of `weapon` by `owner` mini-crits `victim`.
    pub fn blast_mini_crit(&self, weapon: &str, owner: Option<Entity>, victim: Entity) -> bool {
        if Some(victim) == owner {
            return false;
        }
        if self.marked(victim) {
            return true;
        }
        let now = self.clock.now;
        if self.donks.as_deref().is_some_and(|d| d.0.iter().any(|d| d.victim == victim && d.owner == owner && d.weapon == weapon && now <= d.until + 1e-6)) {
            return true;
        }
        let Some(p) = self.item(weapon) else { return false };
        (p.mini_crit_airborne && self.launched(victim)) || ((p.minicrit_burning || p.crit_burning) && self.burn(victim).is_some())
    }
}

/// A cannonball's impact on a player (batch2b_mechanics.md 7: the double
/// donk): its ball's explosion within the window mini-crits him.
#[derive(Clone, Copy, Debug)]
pub struct Donk {
    pub flyer: Entity,
    pub owner: Option<Entity>,
    pub weapon: &'static str,
    pub victim: Entity,
    pub until: f64,
}

/// The impacts still in their windows (`weapons::grenade::touch`).
#[derive(Resource, Clone, Debug, Default)]
pub struct Donks(pub Vec<Donk>);

/// Each victim's health before a backstab this tick (the Kunai's heal).
#[derive(Resource, Clone, Debug, Default)]
pub struct HitHealth(pub HashMap<Entity, f32>);

/// The Axtinguisher put this victim's burn out with `by`'s hit at `at`.
#[derive(Component, Clone, Copy, Debug)]
pub struct BurnConsumed {
    pub by: Entity,
    pub at: f64,
}

/// A head holder's count (§1.6): predicted state the server sends its
/// holder's client (`predicted_net`), whose movement then runs at the
/// heads' speed.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Heads(pub u32);

/// Health an item put above max (the Kunai's, a head's): it falls at
/// `rate` (points/s; None: the normal overheal's for the max) until max.
#[derive(Component, Clone, Copy, Debug)]
pub struct Overhealed {
    pub rate: Option<f32>,
}

/// The item behind hit `d`: its attacker's active weapon's when it dealt
/// it (or named none), else the weapon it names (a flyer's).
fn item_of(d: &Damage, holders: &Query<&Inventory>, weapons: &Query<(&Weapon, &Tf2Item)>, cond: &Conditions) -> Option<ItemParams> {
    let a = d.attacker?;
    if let Some(w) = holders.get(a).ok().and_then(|i| i.active)
        && let Ok((weapon, item)) = weapons.get(w)
        && d.weapon.is_none_or(|n| n == weapon.id)
    {
        return Some(item.0.clone());
    }
    d.weapon.and_then(|n| cond.item(n)).cloned()
}

/// The hit's crit or mini-crit (module doc), before the damage-taken
/// factors (`mods::taken`).
#[allow(clippy::too_many_arguments)]
pub fn crits(
    mut damage: MessageMutator<Damage>,
    cond: Conditions,
    health: Query<&Health>,
    holders: Query<&Inventory>,
    weapons: Query<(&Weapon, &Tf2Item)>,
    clock: Res<SimClock>,
    mut before: ResMut<HitHealth>,
    (melees, mut sounds): (Query<&Tf2Melee>, MessageWriter<GameSound>),
    mut commands: Commands,
) {
    before.0.clear();
    for d in damage.read() {
        if matches!(d.kind, DamageKind::Burn | DamageKind::Bleed) || d.kind.is_blast() {
            continue;
        }
        let Some(a) = d.attacker.filter(|a| *a != d.target) else { continue };
        if d.weapon == Some(BACKSTAB) {
            if let Ok(h) = health.get(d.target) {
                before.0.insert(d.target, h.current);
            }
            continue;
        }
        let v = d.target;
        let mut mini = cond.marked(v);
        let mut crit = false;
        if let Some(p) = item_of(d, &holders, &weapons, &cond) {
            if p.consume_burning
                && let Some(b) = cond.burn(v)
            {
                let r = b.left.max(0.0);
                d.amount = round_points(p.base_damage * (AXT_A + AXT_B * r) * MINI_CRIT / 100.0);
                commands.entity(v).remove::<Burning>().insert(BurnConsumed { by: a, at: clock.now });
                continue;
            }
            if p.damage_by_health
                && let Ok(h) = health.get(a)
                && h.max > 0.0
            {
                d.amount = round_points(p.base_damage * (EQUALIZER_A - EQUALIZER_B * h.current / h.max) / 100.0);
            }
            crit |= p.crit_airborne && cond.blast_jumping(a);
            crit |= p.crit_burning && cond.burn(v).is_some();
            mini |= p.mini_crit_airborne && cond.launched(v);
            mini |= p.back_attack && cond.behind(a, v);
            mini |= p.minicrit_burning && cond.burn(v).is_some();
            if p.minicrits_crit && mini {
                crit = true;
            }
        }
        // A crit's hit sound at the victim (weapons_stock.md 8.3), unless
        // a melee weapon's own crit swing plays it (`melee::after`).
        let melee_crit = holders.get(a).ok().and_then(|i| i.active).and_then(|w| melees.get(w).ok()).is_some_and(|m| m.crit);
        if crit && !melee_crit {
            sounds.write(GameSound(PlaySound { source: Some(v), ..PlaySound::at(CRIT_HIT, d.point) }));
        }
        // Whole points, halves up, after the factor (an item's hit, which
        // may come unrounded: a flare's 22.5).
        let f = if crit {
            CRIT_MULT
        } else if mini {
            MINI_CRIT
        } else {
            1.0
        };
        d.amount = round_points(d.amount * f);
    }
}

/// The speed factor of the boost, the Escape Plan and the heads on a
/// player whose class speed is `base` (units/s): `speed` his max speed
/// after his weapon's rules (units/s), `h` his health share.
pub fn speed_with(speed: f32, base: f32, boost: Option<&SpeedBoost>, escape: bool, h: f32, heads: u32) -> f32 {
    let mut s = speed + base * HEAD_SPEED * heads.min(HEAD_CAP) as f32;
    if escape {
        s *= escape_plan(h);
    }
    if let Some(b) = boost {
        s *= b.factor(base);
    }
    s
}

/// The Escape Plan's factor at health share `h` (§5.2).
pub fn escape_plan(h: f32) -> f32 {
    if h >= 0.8 {
        1.0
    } else if h >= 0.6 {
        1.1
    } else if h >= 0.4 {
        1.2
    } else if h >= 0.2 {
        1.4
    } else {
        1.6
    }
}

/// A speed boost of `secs` from `now`.
pub fn boost(now: f64, secs: f32) -> SpeedBoost {
    SpeedBoost { until: now + secs as f64, mult: BOOST_MULT, cap: BOOST_CAP }
}

fn give_boost(commands: &mut Commands, boosts: &Query<&SpeedBoost>, e: Entity, b: SpeedBoost) {
    let b = SpeedBoost::longer(boosts.get(e).ok(), b);
    commands.entity(e).insert(b);
}

/// What the batch-2 items do once a hit landed (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn after(
    mut taken: MessageReader<DamageTaken>,
    mut died: MessageReader<Died>,
    mut dealt: MessageReader<chimera_sim::weapon::grenade::BlastDealt>,
    mut events: MessageReader<WeaponEvent>,
    cond: Conditions,
    holders: Query<&Inventory>,
    weapons: Query<(&Weapon, &Tf2Item)>,
    mut health: Query<(&mut Health, Option<&Team>, Option<&mut Heads>, Option<&Transform>)>,
    bleeds: Query<&Bleeding>,
    boosts: Query<&SpeedBoost>,
    consumed: Query<&BurnConsumed>,
    pyros: Query<&crate::game::classes::Tf2Class>,
    (flares, rockets, classes, before, clock): (
        Option<Res<crate::game::weapons::flare::FlareRules>>,
        Option<Res<crate::game::weapons::rocket::RocketRules>>,
        Option<Res<crate::game::classes::Classes>>,
        Res<HitHealth>,
        Res<SimClock>,
    ),
    (mut out, mut launch): (MessageWriter<Damage>, MessageWriter<chimera_sim::weapon::projectile::LaunchRequest>),
    mut commands: Commands,
) {
    let now = clock.now;
    let mates = |a: Entity, b: Entity, health: &Query<(&mut Health, Option<&Team>, Option<&mut Heads>, Option<&Transform>)>| {
        let team = |e: Entity| health.get(e).ok().and_then(|h| h.1.copied()).filter(|t| t.0 != 0);
        a != b && team(a).is_some() && team(a) == team(b)
    };
    let alive = |e: Entity, health: &Query<(&mut Health, Option<&Team>, Option<&mut Heads>, Option<&Transform>)>| {
        health.get(e).is_ok_and(|h| h.0.current > 0.0)
    };
    for t in taken.read() {
        let d = &t.damage;
        if matches!(d.kind, DamageKind::Burn | DamageKind::Bleed) || d.amount <= 0.0 {
            continue;
        }
        // A flare's hit or explosion sets alight (its owner too): an
        // enemy Pyro only looks it (`pyro_burn`, weapons_more.md 7.3), a
        // Pyro in his own explosion takes one tick (batch2_mechanics.md
        // 4.3).
        if let Some(f) = flares.as_deref().and_then(|r| d.weapon.and_then(|w| r.get(w)))
            && alive(d.target, &health)
        {
            let pyro = classes.as_deref().zip(pyros.get(d.target).ok()).is_some_and(|(t, c)| c.class(t).name == crate::game::weapons::flame::PYRO);
            let own = d.attacker == Some(d.target);
            let (left, next, damage, harmless) = match (pyro, own) {
                (true, false) => (0.0, now + f.pyro_burn as f64, 0.0, true),
                (true, true) => (f.burn_interval, now + f.burn_interval as f64, f.burn_damage, false),
                _ => (f.burn, now + f.burn_interval as f64, f.burn_damage, false),
            };
            commands.entity(d.target).insert(Burning {
                left,
                next,
                attacker: d.attacker,
                weapon: d.weapon.expect("a flare's weapon"),
                damage,
                interval: f.burn_interval,
                harmless,
                from_tick: true,
            });
        }
        let Some(a) = d.attacker.filter(|a| *a != d.target) else { continue };
        let Some(p) = item_of(d, &holders, &weapons, &cond) else { continue };
        if p.bleed > 0.0 && alive(d.target, &health) {
            let new = Bleeding::start(now, p.bleed, BLEED_TICK / 100.0, DOT_PERIOD, Some(a), d.weapon.or_else(|| weapon_id(a, &holders, &weapons)));
            commands.entity(d.target).insert(Bleeding::refresh(bleeds.get(d.target).ok(), new, now));
        }
        if p.boost_on_hit > 0.0 && !mates(a, d.target, &health) {
            give_boost(&mut commands, &boosts, a, boost(now, p.boost_on_hit));
        }
    }
    for e in events.read() {
        if e.replay {
            continue;
        }
        let WeaponEventKind::Swing { hit, at, .. } = e.kind else { continue };
        let Ok((weapon, item)) = weapons.get(e.weapon) else { continue };
        let p = &item.0;
        match at {
            Some((_, _, target)) if hit && p.buff_ally && mates(e.owner, target, &health) && alive(target, &health) => {
                give_boost(&mut commands, &boosts, target, boost(now, ALLY_BOOST));
                give_boost(&mut commands, &boosts, e.owner, boost(now, SELF_BOOST));
            }
            _ if !hit && p.hit_self_on_miss && alive(e.owner, &health) => {
                // Half its damage, halves up, and a wound (§3).
                out.write(Damage {
                    target: e.owner,
                    attacker: Some(e.owner),
                    amount: round_points(p.base_damage / 2.0 / 100.0),
                    point: Vec3::ZERO,
                    dir: Vec3::NEG_Y,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Melee,
                    weapon: Some(weapon.id),
                    force: Vec3::ZERO,
                });
                if p.bleed > 0.0 {
                    let new = Bleeding::start(now, p.bleed, BLEED_TICK / 100.0, DOT_PERIOD, Some(e.owner), Some(weapon.id));
                    commands.entity(e.owner).insert(Bleeding::refresh(bleeds.get(e.owner).ok(), new, now));
                }
            }
            _ => {}
        }
    }
    // Black Box: once per rocket, from the enemy damage it dealt.
    for b in dealt.read() {
        let (Some(a), Some(w)) = (b.attacker, b.weapon) else { continue };
        let Some(p) = cond.item(w).filter(|p| p.heal_on_radius > 0.0) else { continue };
        let base = rockets.as_deref().and_then(|r| r.get(w)).map_or(90.0, |(r, _)| r.damage);
        let d: f32 = b.victims.iter().filter(|v| v.victim != a).map(|v| v.damage * 100.0).sum();
        if d <= 0.0 {
            continue;
        }
        let gain = (p.heal_on_radius * (d / base).min(1.0) + 1e-4).floor();
        if let Ok((mut h, ..)) = health.get_mut(a)
            && h.current > 0.0
            && h.current < h.max
        {
            h.current = (h.current + gain / 100.0).min(h.max);
        }
    }
    for k in died.read() {
        // Heads are lost at death.
        if let Ok((_, _, Some(mut heads), _)) = health.get_mut(k.entity) {
            heads.0 = 0;
            commands.entity(k.entity).remove::<Heads>();
        }
        let d = &k.damage;
        let Some(a) = d.attacker.filter(|a| *a != k.entity) else { continue };
        // The Kunai's backstab (credited to the backstab, not the knife).
        let p = if d.weapon == Some(BACKSTAB) {
            holders.get(a).ok().and_then(|i| i.active).and_then(|w| weapons.get(w).ok()).map(|(_, i)| i.0.clone())
        } else {
            item_of(d, &holders, &weapons, &cond)
        };
        let Some(p) = p else { continue };
        if p.sanguisuge
            && d.weapon == Some(BACKSTAB)
            && let Some(gain) = before.0.get(&k.entity).copied()
            && let Ok((mut h, ..)) = health.get_mut(a)
            && h.current > 0.0
        {
            let cap = h.max * KUNAI_CAP;
            h.current = (h.current + gain).min(cap).max(h.current);
            if h.current > h.max {
                commands.entity(a).insert(Overhealed { rate: None });
            }
        }
        if p.decapitate
            && let Ok((mut h, _, heads, _)) = health.get_mut(a)
            && h.current > 0.0
        {
            let n = heads.as_ref().map_or(0, |h| h.0) + 1;
            match heads {
                Some(mut x) => x.0 = n,
                None => {
                    commands.entity(a).insert(Heads(n));
                }
            }
            if n <= HEAD_CAP {
                h.max += HEAD_HEALTH / 100.0;
            }
            h.current += HEAD_HEALTH / 100.0;
            if h.current > h.max {
                commands.entity(a).insert(Overhealed { rate: Some(HEAD_DRAIN) });
            }
        }
        if p.consume_burning && consumed.get(k.entity).is_ok_and(|c| c.by == a && (now - c.at).abs() < 1e-6) {
            give_boost(&mut commands, &boosts, a, boost(now, AXT_BOOST));
        }
        if p.drop_kit
            && let Ok((.., Some(at))) = health.get(k.entity)
        {
            let mut f = chimera_sim::weapon::projectile::Flyer::new(KIT, Some(a), Vec3::ZERO, &clock);
            f.gravity = 800.0 * crate::game::weapons::damage::U;
            f.half = Vec3::splat(4.0 * crate::game::weapons::damage::U);
            f.expires = Some(now + KIT_LIFE as f64);
            f.pass_teammates = false;
            launch.write(chimera_sim::weapon::projectile::LaunchRequest { flyer: f, start: at.translation, shown: Some(KIT_SHOWN.into()), tag: None });
        }
    }
}

fn weapon_id(a: Entity, holders: &Query<&Inventory>, weapons: &Query<(&Weapon, &Tf2Item)>) -> Option<&'static str> {
    holders.get(a).ok()?.active.and_then(|w| weapons.get(w).ok()).map(|(w, _)| w.id)
}

/// The Candy Cane's kits: taken by the first living body that reaches
/// one (module doc), gone at their life's end (the flyer's).
#[allow(clippy::type_complexity)]
pub fn kits(
    kits: Query<(Entity, &chimera_sim::weapon::projectile::Flyer, &Transform)>,
    mut bodies: Query<(Entity, &Transform, Option<&avian3d::prelude::ColliderAabb>, Option<&MovementState>, &mut Health, Has<Burning>, Has<Bleeding>), With<Intent>>,
    mut commands: Commands,
) {
    let reach = KIT_REACH * crate::game::weapons::damage::U;
    for (k, f, at) in &kits {
        if f.weapon != KIT {
            continue;
        }
        for (e, t, aabb, state, mut h, burning, bleeding) in &mut bodies {
            if h.current <= 0.0 {
                continue;
            }
            let (min, max) = crate::game::weapons::blast::boxed(t, aabb, state);
            if at.translation.clamp(min, max).distance(at.translation) > reach {
                continue;
            }
            let gain = ((h.max * 100.0 * KIT_SHARE) - 1e-4).ceil() / 100.0;
            let heals = h.current < h.max;
            if !heals && !burning && !bleeding {
                continue;
            }
            if heals {
                h.current = (h.current + gain).min(h.max);
            }
            commands.entity(e).remove::<(Burning, Bleeding)>();
            commands.entity(k).despawn();
            break;
        }
    }
}

/// A player's regeneration state (module doc).
#[derive(Clone, Copy, Debug)]
pub struct Regen {
    pub next: f64,
    pub carry: f32,
    pub hurt: f64,
}

/// Each regenerating player's state: a resource, not a part (no
/// archetype per Medic).
#[derive(Resource, Clone, Debug, Default)]
pub struct Regens(pub HashMap<Entity, Regen>);

/// The regeneration's rate (points/s) `since` s after the last hurt: the
/// Medic's `medic` (none: 0) and the items' `item`.
pub fn regen_rate(medic: bool, item: f32, since: f32) -> f32 {
    let ramp = ((since - 5.0) / 5.0).clamp(0.0, 1.0);
    let m = if medic { MEDIC_REGEN * (1.0 + ramp) } else { 0.0 };
    let s = if since < 5.0 { 0.25 } else { 0.5 + 0.5 * ramp };
    m + item * s
}

/// Once a second, each regenerating player's gain (module doc).
#[allow(clippy::type_complexity)]
pub fn regen(
    mut q: Query<(Entity, &crate::game::classes::Tf2Class, &mut Health, Option<&Inventory>)>,
    mut taken: MessageReader<DamageTaken>,
    items: Query<&Tf2Item>,
    table: Res<crate::game::classes::Classes>,
    clock: Res<SimClock>,
    mut regens: ResMut<Regens>,
) {
    let now = clock.now;
    let hurt: Vec<Entity> = taken.read().filter(|t| t.damage.amount > 0.0).map(|t| t.damage.target).collect();
    regens.0.retain(|e, _| q.contains(*e));
    for (e, class, mut h, inv) in &mut q {
        let medic = class.class(&table).name == MEDIC;
        let item: f32 = inv.map_or(0.0, |inv| {
            inv.weapons
                .iter()
                .filter_map(|w| Some((*w, items.get(*w).ok()?)))
                .filter(|(w, i)| !i.0.provide_on_active || inv.active == Some(*w))
                .map(|(_, i)| i.0.regen)
                .sum()
        });
        if !medic && item == 0.0 {
            regens.0.remove(&e);
            continue;
        }
        let Some(r) = regens.0.get_mut(&e) else {
            regens.0.insert(e, Regen { next: now + 1.0, carry: 0.0, hurt: f64::MIN });
            continue;
        };
        if hurt.contains(&e) {
            r.hurt = now;
        }
        if h.current <= 0.0 || now + 1e-6 < r.next {
            continue;
        }
        r.next += 1.0;
        let since = (now - r.hurt).min(1e6) as f32;
        let total = r.carry + regen_rate(medic, item, since).max(0.0);
        let whole = total.floor();
        r.carry = total - whole;
        if h.current < h.max && whole > 0.0 {
            h.current = (h.current + whole / 100.0).min(h.max);
        }
    }
}

/// The normal overheal's fall (points/s) for max health `max` (points).
pub fn overheal_rate(max: f32) -> f32 {
    (((1.5 * max / 5.0) + 1e-4).floor() * 5.0 - max) / 15.0
}

/// Health an item put above max falls (module doc).
pub fn overheal(mut q: Query<(Entity, &mut Health, &Overhealed)>, time: Res<Time<Fixed>>, mut commands: Commands) {
    let dt = time.delta_secs();
    for (e, mut h, o) in &mut q {
        if h.current <= h.max || h.current <= 0.0 {
            commands.entity(e).remove::<Overhealed>();
            continue;
        }
        let rate = o.rate.unwrap_or_else(|| overheal_rate(h.max * 100.0));
        h.current = (h.current - rate * dt / 100.0).max(h.max);
    }
}

/// The rules' schedule (`mods::add` calls it).
pub fn add(app: &mut App) {
    chimera_sim::status::add(app);
    crate::core::PredictedAppExt::predicted_net::<Heads>(app);
    app.init_resource::<HitHealth>().init_resource::<Regens>().init_resource::<Donks>().add_systems(
        FixedUpdate,
        (
            crits.after(crate::core::SimSet::Weapons).before(super::mods::taken).before(crate::core::apply_damage),
            (after, kits).after(crate::core::apply_damage),
            (regen, overheal).after(crate::core::apply_damage).after(crate::game::movement::class_health),
        )
            .run_if(crate::core::authoritative),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_escape_plans_tiers_and_the_regeneration_follow_the_spec() {
        assert_eq!(240.0 * escape_plan(159.0 / 200.0), 264.0);
        assert_eq!(240.0 * escape_plan(39.0 / 200.0), 384.0);
        assert!((regen_rate(true, -2.0, 1.0) - 2.5).abs() < 1e-6);
        assert!((regen_rate(true, -2.0, 12.0) - 4.0).abs() < 1e-6);
        assert!((overheal_rate(70.0) - 35.0 / 15.0).abs() < 1e-5);
        // Heads: 280 + 4 x 22.4.
        assert!((speed_with(280.0, 280.0, None, false, 1.0, 5) - 369.6).abs() < 1e-3);
    }
}
