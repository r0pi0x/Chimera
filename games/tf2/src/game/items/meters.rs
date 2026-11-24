//! The item meters (specs/tf2/batch2b_mechanics.md 14): the hook
//! `tf2:meter` (`MeterParams`, made by `items::docs` from the item's
//! attributes, `MeterParams::of`) and the weapon part `Tf2Meter`
//! (predicted: the server's fill is a correction to the holder's client).
//!
//! - **Hype** (the Soda Popper, `hype_on_damage`): `HYPE_PER_DAMAGE` % a
//!   point of damage its own hits deal (fitted: 60 → 15.4); full, attack2
//!   with it out starts the hype, which lasts while the meter drains at
//!   `HYPE_DRAIN` %/s (about 10.7 s): `HYPE_AIR_JUMPS` air jumps.
//! - **Boost** (Baby Face's Blaster, `boost_on_damage`): 1 % a point of
//!   its own damage, at most `BOOST_MAX` (99); its holder's max speed
//!   gains `BOOST_SPEED` × the boost (Scout: 360 + 1.62 × 60 = 457.2, the
//!   0.9 its `mult_player_movespeed`); each point of damage he takes
//!   takes `lose_hype_on_take_damage` (4) off; a jump takes
//!   `hype_resets_on_jump` % (75) of it.
//! - **Crikey** (the Cleaner's Carbine, `minicrit_boost_charge_rate`): 1 %
//!   a point of its own damage, full at 100; attack2 when full empties it
//!   and gives the mini-crit buff for `minicrit_boost_when_charged` s (8;
//!   Chimera's `status::Condition::MiniCritBuff`).
//! - **Heads** (the Bazaar Bargain, its class `tf_weapon_sniperrifle_decap`):
//!   a headshot kill with it takes a head; its charge rate is × (0.5 +
//!   0.25 h), h at most `HEADS_CAP` (`decap_factor`; the rifle's
//!   `weapons::sniper::scope`).
//!
//! - **Rage** (the Soldier's banners, `tf_weapon_buff_item`, batch3_
//!   mechanics.md 2): every point of damage its holder deals an enemy,
//!   with any weapon, fills `per_damage` (1/6: 600 fills it; the
//!   Concheror's 1/4.8: 480); none while it runs. Full, attack with the
//!   banner out blows the horn: the buff starts `delay` s later (2.7) and
//!   runs while the rage drains at `drain` (10 a second: 10 s). While it
//!   runs, every `PULSE` s (`banner_pulses`, the server's) its holder and
//!   each teammate whose origin is within `BANNER_RADIUS` (450) of his get
//!   the banner's condition for `PULSE_LIFE` s (so it lingers about a
//!   second after the rage is gone or a teammate leaves): by
//!   `set_buff_type`, 1 the Buff Banner's mini-crit buff (Chimera's
//!   `Condition::MiniCritBuff`), 2 the Battalion's Backup's defense buff
//!   (`Condition::DefenseBuff`: damage taken × `BACKUP_CUT`, a crit
//!   taken a normal hit: `banner_taken`, `effects::crits`), 3 the
//!   Concheror's (`CONCH`: speed × `CONCH_SPEED` as Chimera's
//!   `SpeedBoost`, heal on hit `CONCH_HEAL` of the damage, rounded down:
//!   `fill`). The banners' own player numbers (the Backup's + 20 max
//!   health, the Concheror's regeneration) are `super::mods`'.
//!
//! Each shows on TF2's HUD as a simple meter (`meter`: its label and fill;
//! the ui crate's item meter at `HudItemEffectMeter`). A death empties
//! them.
//!
//! Hype's 5 air jumps are measured (batch3_mechanics.md 6: each the normal
//! air jump, landing resets them, no drain of their own); heads come only
//! from headshot kills with the Bazaar, a body-shot kill or hit changing
//! nothing, and count past 6 (the rate's cap; measured).
//!
//! Known shortcuts (provisional): only the item's own hits fill hype,
//! boost and crikey, from the damage as dealt (rounded: the Carbine's
//! 10.2 counts 10; rage too: 103 fills 17.17, the game's 17.196 is from
//! the unrounded hit); a jump's loss is a share of the boost; the boost
//! cap 99. The banners: the horn has no sequence, sound or look and the
//! banner stays out after it; the pulse reaches through walls (line of
//! sight was clear in every measured run); the Concheror's speed has no
//! cap (only the Soldier's 336 measured); the Backup's sentry cut waits
//! on buildings and its cut on blasts is the plain × 0.65 (the measured
//! bot rocket's 0.69 is open).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::{
    status::{Condition, Conditions},
    weapon::{Inventory, NetPart, Weapon},
};

use super::convert::ItemRecord;
use crate::core::{Damage, DamageTaken, Died, Health, Hitgroup, Intent, MovementState, SimClock, Team};

/// The hook.
pub const HOOK: &str = "tf2:meter";
/// Hype: % a point of damage (fitted), its drain (%/s), its air jumps.
pub const HYPE_PER_DAMAGE: f32 = 15.4 / 60.0;
pub const HYPE_DRAIN: f32 = 9.4;
pub const HYPE_AIR_JUMPS: u8 = 5;
/// Boost: its cap and the speed a point of it adds (units/s).
pub const BOOST_MAX: f32 = 99.0;
pub const BOOST_SPEED: f32 = 1.62;
/// Heads counted for the charge rate.
pub const HEADS_CAP: u32 = 6;
/// A full meter.
pub const FULL: f32 = 100.0;
/// The banners (batch3_mechanics.md 2): rage a point of damage (the
/// Concheror's), the horn, the drain, the pulse's reach (units), period
/// and life (s, fitted), the Backup's cut, the Concheror's speed and heal.
pub const RAGE_PER_DAMAGE: f32 = 1.0 / 6.0;
pub const RAGE_PER_DAMAGE_CONCH: f32 = 1.0 / 4.8;
pub const BANNER_DELAY: f32 = 2.7;
pub const RAGE_DRAIN: f32 = 10.0;
pub const BANNER_RADIUS: f32 = 450.0;
pub const PULSE: f64 = 0.5;
pub const PULSE_LIFE: f64 = 1.2;
pub const BACKUP_CUT: f32 = 0.65;
pub const CONCH_SPEED: f32 = 1.4;
pub const CONCH_HEAL: f32 = 0.35;
/// The Concheror's condition (heal on hit).
pub const CONCH: &str = "tf2:conch";
/// The Phlogistinator (batch3b_mechanics.md 2): rage a point of fire
/// damage (300 fills it), the taunt's lock and invulnerability from its
/// start, the crit boost's linger after the rage (s).
pub const MMMPH_PER_DAMAGE: f32 = 1.0 / 3.0;
pub const MMMPH_TAUNT: f32 = 2.76;
pub const MMMPH_INVULNERABLE: f32 = 2.61;
pub const MMMPH_LINGER: f32 = 1.2;
/// The Heatmaker's focus lingers this long after its meter is empty (s).
pub const FOCUS_LINGER: f32 = 0.33;

/// Which meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeterKind {
    Hype,
    Boost,
    Crikey,
    Heads,
    Rage,
    /// The Phlogistinator's (batch3b_mechanics.md 2).
    Mmmph,
    /// The Hitman's Heatmaker's (batch3b_mechanics.md 12).
    Focus,
}

/// `tf2:meter`'s parameters (%, s, units; module doc).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeterParams {
    pub kind: MeterKind,
    pub label: String,
    #[serde(default)]
    pub per_damage: f32,
    #[serde(default = "full")]
    pub max: f32,
    #[serde(default)]
    pub drain: f32,
    #[serde(default)]
    pub lose_on_hurt: f32,
    #[serde(default)]
    pub jump_loss: f32,
    #[serde(default)]
    pub buff: f32,
    #[serde(default)]
    pub speed_per: f32,
    #[serde(default)]
    pub air_jumps: u8,
    /// A banner's `set_buff_type` (1 Buff Banner, 2 Battalion's Backup,
    /// 3 Concheror) and its horn's delay (s).
    #[serde(default)]
    pub banner: u8,
    #[serde(default)]
    pub delay: f32,
    /// Batch 3b: only fire damage fills it (the Phlogistinator's `burn
    /// damage earns rage`); a kill with it fills this (the Heatmaker's
    /// `rage_on_kill`); its condition lingers this long after the meter
    /// is empty (s); the taunt's lock and its invulnerability (s).
    #[serde(default)]
    pub fire_only: bool,
    #[serde(default)]
    pub per_kill: f32,
    #[serde(default)]
    pub linger: f32,
    #[serde(default)]
    pub taunt: f32,
    #[serde(default)]
    pub invulnerable: f32,
}

fn full() -> f32 {
    FULL
}

impl MeterParams {
    /// The meter record `r`'s attributes and class give (None: none).
    pub fn of(r: &ItemRecord) -> Option<Self> {
        let add = |c: &str| r.attr(c, 0.0) as f32;
        let base = |kind, label: &str| MeterParams {
            kind,
            label: label.into(),
            per_damage: 0.0,
            max: FULL,
            drain: 0.0,
            lose_on_hurt: 0.0,
            jump_loss: 0.0,
            buff: 0.0,
            speed_per: 0.0,
            air_jumps: 0,
            banner: 0,
            delay: 0.0,
            fire_only: false,
            per_kill: 0.0,
            linger: 0.0,
            taunt: 0.0,
            invulnerable: 0.0,
        };
        if r.flag("hype_on_damage") {
            return Some(MeterParams { per_damage: HYPE_PER_DAMAGE, drain: HYPE_DRAIN, air_jumps: HYPE_AIR_JUMPS, ..base(MeterKind::Hype, "HYPE") });
        }
        if r.flag("boost_on_damage") {
            return Some(MeterParams {
                per_damage: 1.0,
                max: BOOST_MAX,
                lose_on_hurt: add("lose_hype_on_take_damage"),
                jump_loss: add("hype_resets_on_jump"),
                speed_per: BOOST_SPEED,
                ..base(MeterKind::Boost, "BOOST")
            });
        }
        if r.flag("minicrit_boost_charge_rate") {
            return Some(MeterParams {
                per_damage: add("minicrit_boost_charge_rate"),
                buff: add("minicrit_boost_when_charged"),
                ..base(MeterKind::Crikey, "CRIKEY")
            });
        }
        if r.class == "tf_weapon_buff_item" {
            let banner = add("set_buff_type") as u8;
            let per = if banner == 3 { RAGE_PER_DAMAGE_CONCH } else { RAGE_PER_DAMAGE };
            return Some(MeterParams { per_damage: per, drain: RAGE_DRAIN, banner, delay: BANNER_DELAY, ..base(MeterKind::Rage, "RAGE") });
        }
        if r.flag("burn_damage_earns_rage") {
            return Some(MeterParams {
                per_damage: MMMPH_PER_DAMAGE,
                drain: RAGE_DRAIN,
                fire_only: true,
                linger: MMMPH_LINGER,
                taunt: MMMPH_TAUNT,
                invulnerable: MMMPH_INVULNERABLE,
                ..base(MeterKind::Mmmph, "MMMPH")
            });
        }
        if add("rage_on_kill") > 0.0 {
            return Some(MeterParams { per_kill: add("rage_on_kill"), drain: RAGE_DRAIN, linger: FOCUS_LINGER, ..base(MeterKind::Focus, "FOCUS") });
        }
        if r.class == "tf_weapon_sniperrifle_decap" {
            return Some(MeterParams { max: HEADS_CAP as f32, ..base(MeterKind::Heads, "HEADS") });
        }
        None
    }
}

/// The meter's state (predicted).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeterState {
    /// % (heads: their count).
    pub value: f32,
    /// The hype's or the buff's end (sim time).
    pub until: f64,
    pub prev_alt: bool,
    pub prev_jump: bool,
    /// The banner's buff's start (sim time; the horn before it).
    #[serde(default)]
    pub from: f64,
    #[serde(default)]
    pub prev_fire: bool,
    #[serde(default)]
    pub prev_reload: bool,
}

/// The weapon part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Meter {
    pub params: MeterParams,
    pub state: MeterState,
}

impl NetPart for Tf2Meter {
    type Net = MeterState;
    fn net(&self) -> MeterState {
        self.state
    }
    fn set_net(&mut self, net: MeterState) {
        self.state = net;
    }
}

impl Tf2Meter {
    /// Hype or the buff in force at `now`.
    pub fn active(&self, now: f64) -> bool {
        self.state.until > now
    }

    /// The HUD meter: its label and fill.
    pub fn meter(&self) -> (&str, f32) {
        (&self.params.label, (self.state.value / self.params.max.max(1e-6)).clamp(0.0, 1.0))
    }

    /// A banner's buff in force at `now` (after the horn).
    pub fn buffing(&self, now: f64) -> bool {
        self.params.kind == MeterKind::Rage && now + 1e-6 >= self.state.from && now <= self.state.until + 1e-6 && self.state.until > 0.0
    }

    /// The Phlogistinator's taunt or the Heatmaker's focus in force at
    /// `now` (the condition's linger included).
    pub fn in_force(&self, now: f64) -> bool {
        matches!(self.params.kind, MeterKind::Mmmph | MeterKind::Focus)
            && self.state.until > 0.0
            && now + 1e-6 >= self.state.from
            && now <= self.state.until + self.params.linger as f64 + 1e-6
    }

    /// The Phlogistinator's taunt holds its holder at `now`.
    pub fn taunting(&self, now: f64) -> bool {
        self.params.kind == MeterKind::Mmmph && self.state.until > 0.0 && now + 1e-6 >= self.state.from && now < self.state.from + self.params.taunt as f64
    }

    /// The heads counted.
    pub fn heads(&self) -> u32 {
        if self.params.kind == MeterKind::Heads { self.state.value.max(0.0) as u32 } else { 0 }
    }
}

/// The Bazaar Bargain's charge-rate factor with `heads` (module doc).
pub fn decap_factor(heads: u32) -> f32 {
    0.5 + 0.25 * heads.min(HEADS_CAP) as f32
}

/// The movement the meters of `inv` give at `now`: speed added (units/s)
/// and the air jumps during hype (None: the class's).
pub fn movement(inv: &Inventory, meters: &Query<&Tf2Meter>, now: f64) -> (f32, Option<u8>) {
    let mut speed = 0.0;
    let mut jumps = None;
    for m in inv.weapons.iter().filter_map(|w| meters.get(*w).ok()) {
        speed += m.params.speed_per * m.state.value.max(0.0);
        if m.params.kind == MeterKind::Hype && m.active(now) {
            jumps = Some(m.params.air_jumps);
        }
    }
    (speed, jumps)
}

/// `tf2:meter` on a weapon: empty.
pub fn attach(e: &mut EntityWorldMut, p: &MeterParams) {
    e.insert(Tf2Meter { params: p.clone(), state: MeterState::default() });
}

/// Each holder's meters: the hype's drain, attack2's hype and buff, the
/// jump's loss (predicted, before the weapon switch).
#[allow(clippy::type_complexity)]
pub fn think(
    mut owners: Query<(&mut Intent, &Inventory, Option<&MovementState>), Without<crate::map::interp::NetDrawn>>,
    mut meters: Query<&mut Tf2Meter>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for (mut intent, inv, state) in &mut owners {
        for &w in &inv.weapons {
            let Ok(mut m) = meters.get_mut(w) else { continue };
            let p = m.params.clone();
            let mut s = m.state;
            let press = intent.secondary && !s.prev_alt && inv.active == Some(w) && inv.ready(now);
            let fire = intent.fire && !s.prev_fire && inv.active == Some(w) && inv.ready(now);
            let reload = intent.reload && !s.prev_reload && inv.active == Some(w) && inv.ready(now);
            match p.kind {
                MeterKind::Hype => {
                    if s.until > now {
                        s.value = ((s.until - now) as f32 * p.drain).max(0.0);
                    } else if s.until > 0.0 {
                        s.until = 0.0;
                        s.value = 0.0;
                    } else if press && s.value >= p.max - 1e-4 && p.drain > 0.0 {
                        s.until = now + (s.value / p.drain) as f64;
                    }
                }
                MeterKind::Crikey => {
                    if press && s.value >= p.max - 1e-4 {
                        s.value = 0.0;
                        s.until = now + p.buff as f64;
                    }
                }
                MeterKind::Boost => {
                    let jump = intent.jump && !s.prev_jump && state.is_some_and(|m| m.on_ground);
                    if jump && p.jump_loss > 0.0 {
                        s.value *= 1.0 - p.jump_loss / 100.0;
                    }
                }
                MeterKind::Heads => {}
                MeterKind::Rage => {
                    if s.until > now {
                        if now >= s.from {
                            s.value = ((s.until - now) as f32 * p.drain).max(0.0);
                        }
                    } else if s.until > 0.0 {
                        s.until = 0.0;
                        s.from = 0.0;
                        s.value = 0.0;
                    } else if fire && s.value >= p.max - 1e-4 && p.drain > 0.0 {
                        s.from = now + p.delay as f64;
                        s.until = s.from + (s.value / p.drain) as f64;
                    }
                }
                MeterKind::Mmmph | MeterKind::Focus => {
                    // Attack2 (the taunt) or reload (focus) with it full;
                    // the rage drains from the start, the condition
                    // lingers `linger` after.
                    let start = if p.kind == MeterKind::Mmmph { press } else { reload };
                    if s.until > 0.0 && now <= s.until + p.linger as f64 + 1e-6 {
                        s.value = ((s.until - now) as f32 * p.drain).max(0.0);
                    } else if s.until > 0.0 {
                        s.until = 0.0;
                        s.from = 0.0;
                        s.value = 0.0;
                    } else if start && s.value >= p.max - 1e-4 && p.drain > 0.0 {
                        s.from = now;
                        s.until = now + (s.value / p.drain) as f64;
                    }
                }
            }
            s.prev_fire = intent.fire;
            s.prev_alt = intent.secondary;
            s.prev_jump = intent.jump;
            s.prev_reload = intent.reload;
            if m.state != s {
                m.state = s;
            }
            // The taunt: no moving, jumping or attacking.
            if m.taunting(now) {
                intent.fire = false;
                intent.secondary = false;
                intent.jump = false;
                intent.move_axis = Vec2::ZERO;
            }
        }
    }
}

/// The meters' fill from the hits that landed, the boost's loss, heads,
/// the Carbine's buff, a death's reset (the server's).
#[allow(clippy::too_many_arguments)]
pub fn fill(
    mut taken: MessageReader<DamageTaken>,
    mut died: MessageReader<Died>,
    holders: Query<(&Inventory, Option<&Team>, &Health)>,
    weapons: Query<&Weapon>,
    mut meters: Query<&mut Tf2Meter>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    let mates = |a: Entity, b: Entity| {
        let team = |e: Entity| holders.get(e).ok().and_then(|h| h.1.copied()).filter(|t| t.0 != 0);
        a != b && team(a).is_some() && team(a) == team(b)
    };
    // The meter weapons of `a` that dealt a hit naming `weapon`.
    let own = |a: Entity, weapon: Option<&'static str>| -> Vec<Entity> {
        let Ok((inv, ..)) = holders.get(a) else { return Vec::new() };
        inv.weapons
            .iter()
            .copied()
            .filter(|w| match weapon {
                Some(n) => weapons.get(*w).is_ok_and(|x| x.id == n),
                None => inv.active == Some(*w),
            })
            .collect()
    };
    for t in taken.read() {
        let d = &t.damage;
        let points = d.amount * 100.0;
        if points <= 0.0 {
            continue;
        }
        // The boost's loss on its holder.
        if let Ok((inv, ..)) = holders.get(d.target) {
            for &w in &inv.weapons {
                if let Ok(mut m) = meters.get_mut(w)
                    && m.params.lose_on_hurt > 0.0
                {
                    m.state.value = (m.state.value - m.params.lose_on_hurt * points).max(0.0);
                }
            }
        }
        let Some(a) = d.attacker.filter(|a| *a != d.target && !mates(*a, d.target)) else { continue };
        for w in own(a, d.weapon) {
            let Ok(mut m) = meters.get_mut(w) else { continue };
            if m.params.per_damage <= 0.0 || m.active(now) || m.params.kind == MeterKind::Rage {
                continue;
            }
            if m.params.fire_only && d.kind != crate::core::DamageKind::Burn {
                continue;
            }
            m.state.value = (m.state.value + m.params.per_damage * points).min(m.params.max);
        }
        // Rage: from any of his weapons' damage.
        if let Ok((inv, ..)) = holders.get(a) {
            for &w in &inv.weapons {
                if let Ok(mut m) = meters.get_mut(w)
                    && m.params.kind == MeterKind::Rage
                    && !m.active(now)
                {
                    m.state.value = (m.state.value + m.params.per_damage * points).min(m.params.max);
                }
            }
        }
    }
    for k in died.read() {
        // A death empties his meters.
        if let Ok((inv, ..)) = holders.get(k.entity) {
            for &w in &inv.weapons {
                if let Ok(mut m) = meters.get_mut(w) {
                    m.state = MeterState::default();
                }
            }
        }
        let d = &k.damage;
        let Some(a) = d.attacker.filter(|a| *a != k.entity) else { continue };
        // A kill with it (the Heatmaker's focus).
        for w in own(a, d.weapon) {
            if let Ok(mut m) = meters.get_mut(w)
                && m.params.per_kill > 0.0
                && !m.in_force(now)
            {
                m.state.value = (m.state.value + m.params.per_kill).min(m.params.max);
            }
        }
        if d.hitgroup != Hitgroup::Head {
            continue;
        }
        for w in own(a, d.weapon) {
            if let Ok(mut m) = meters.get_mut(w)
                && m.params.kind == MeterKind::Heads
            {
                m.state.value += 1.0;
            }
        }
    }
}

/// The Carbine's buff as the condition (the server's).
pub fn buffs(holders: Query<(Entity, &Inventory, &Health)>, meters: Query<&Tf2Meter>, mut conditions: ResMut<Conditions>, clock: Res<SimClock>) {
    let now = clock.now;
    for (e, inv, h) in &holders {
        if h.current <= 0.0 {
            continue;
        }
        for m in inv.weapons.iter().filter_map(|w| meters.get(*w).ok()) {
            if m.params.kind == MeterKind::Crikey && m.active(now) {
                conditions.give(e, Condition::MiniCritBuff, m.state.until, Some(e));
            }
            // The Phlogistinator's taunt: invulnerable from its start, then
            // crits until the rage is gone and its linger.
            if m.params.kind == MeterKind::Mmmph && m.in_force(now) {
                let from = m.state.from;
                if now < from + m.params.invulnerable as f64 {
                    conditions.give(e, Condition::Phased, from + m.params.invulnerable as f64, Some(e));
                }
                let crit = Condition::Game(crate::game::weapons::flame::CRIT_BOOST);
                conditions.give(e, crit, m.state.until + m.params.linger as f64, Some(e));
            }
        }
    }
}

/// Each banner's pulses (its holder's entity → the next pulse and the
/// buff's end), the server's.
#[derive(Resource, Clone, Debug, Default)]
pub struct Pulses(pub bevy::platform::collections::HashMap<Entity, (f64, f64)>);

/// The banners' pulses (module doc, "Rage"; the server's).
#[allow(clippy::type_complexity)]
pub fn banner_pulses(
    holders: Query<(Entity, &Inventory, &Health, &Transform, Option<&Team>)>,
    meters: Query<&Tf2Meter>,
    mut pulses: ResMut<Pulses>,
    mut conditions: ResMut<Conditions>,
    boosts: Query<&chimera_sim::status::SpeedBoost>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    pulses.0.retain(|e, (_, until)| holders.contains(*e) && now <= *until + 1e-6);
    for (e, inv, h, ..) in &holders {
        if h.current <= 0.0 {
            pulses.0.remove(&e);
            continue;
        }
        for m in inv.weapons.iter().filter_map(|w| meters.get(*w).ok()) {
            if m.buffing(now) {
                let p = pulses.0.entry(e).or_insert((m.state.from, m.state.until));
                p.1 = m.state.until;
            }
        }
    }
    let due: Vec<(Entity, u8)> = pulses
        .0
        .iter()
        .filter(|(_, (next, until))| now + 1e-6 >= *next && now <= *until + 1e-6)
        .filter_map(|(e, _)| {
            let (_, inv, ..) = holders.get(*e).ok()?;
            let b = inv.weapons.iter().filter_map(|w| meters.get(*w).ok()).find(|m| m.params.kind == MeterKind::Rage)?.params.banner;
            Some((*e, b))
        })
        .collect();
    for (soldier, banner) in due {
        if let Some(p) = pulses.0.get_mut(&soldier) {
            p.0 += PULSE;
        }
        let Ok((_, _, _, at, team)) = holders.get(soldier) else { continue };
        let team = team.copied().filter(|t| t.0 != 0);
        let reach = BANNER_RADIUS * crate::game::weapons::damage::U;
        let until = now + PULSE_LIFE;
        for (e, _, h, t, their) in &holders {
            let mate = e == soldier || (team.is_some() && their.copied() == team);
            if !mate || h.current <= 0.0 || t.translation.distance(at.translation) > reach + 1e-4 {
                continue;
            }
            match banner {
                1 => conditions.give(e, Condition::MiniCritBuff, until, Some(soldier)),
                2 => conditions.give(e, Condition::DefenseBuff, until, Some(soldier)),
                3 => {
                    conditions.give(e, Condition::Game(CONCH), until, Some(soldier));
                    let b = chimera_sim::status::SpeedBoost { until, mult: CONCH_SPEED, cap: 1.0e4 };
                    let b = chimera_sim::status::SpeedBoost::longer(boosts.get(e).ok(), b);
                    if boosts.get(e).ok() != Some(&b) {
                        commands.entity(e).insert(b);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Damage on a player with the defense buff × `BACKUP_CUT` (the Battalion's
/// Backup's; his own too), before it lands.
pub fn banner_taken(mut damage: MessageMutator<Damage>, conditions: Res<Conditions>, clock: Res<SimClock>) {
    if conditions.0.is_empty() {
        return;
    }
    for d in damage.read() {
        if conditions.has(d.target, Condition::DefenseBuff, clock.now) {
            d.amount = crate::game::weapons::damage::round_points(d.amount * BACKUP_CUT);
        }
    }
}

/// The Concheror's heal on hit (`CONCH_HEAL` of an enemy hit's damage,
/// rounded down, up to max), after the damage.
pub fn conch_heals(mut taken: MessageReader<DamageTaken>, conditions: Res<Conditions>, mut health: Query<(&mut Health, Option<&Team>)>, clock: Res<SimClock>) {
    for t in taken.read() {
        let d = &t.damage;
        let Some(a) = d.attacker.filter(|a| *a != d.target) else { continue };
        if d.amount <= 0.0 || !conditions.has(a, Condition::Game(CONCH), clock.now) {
            continue;
        }
        let team = |e: Entity| health.get(e).ok().and_then(|h| h.1.copied()).filter(|t| t.0 != 0);
        if team(a).is_some() && team(a) == team(d.target) {
            continue;
        }
        if let Ok((mut h, _)) = health.get_mut(a)
            && h.current > 0.0
            && h.current < h.max
        {
            let gain = (d.amount * 100.0 * CONCH_HEAL + 1e-4).floor();
            h.current = (h.current + gain / 100.0).min(h.max);
        }
    }
}

/// The hook, the part and the systems (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_sim::weapon::{CarriedAppExt, SelectWeapons, data::WeaponDataAppExt};
    app.init_resource::<Pulses>()
        .register_weapon_hook::<MeterParams>(HOOK, |e, _, p| attach(e, p))
        .predicted_part::<Tf2Meter>()
        .add_systems(crate::core::Predict::Select, think.before(SelectWeapons))
        .add_systems(
            FixedUpdate,
            (
                fill.after(crate::core::apply_damage),
                conch_heals.after(crate::core::apply_damage),
                (buffs, banner_pulses).after(crate::core::SimSet::Weapons).before(crate::core::apply_damage),
                banner_taken.after(super::mods::taken).before(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bazaar_rates_and_the_hype_follow_14() {
        assert_eq!(decap_factor(0), 0.5);
        assert_eq!(decap_factor(3), 1.25);
        assert_eq!(decap_factor(8), 2.0);
        assert!((60.0 * HYPE_PER_DAMAGE - 15.4).abs() < 1e-4);
    }
}
