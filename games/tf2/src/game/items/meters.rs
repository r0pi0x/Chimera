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
//! Each shows on TF2's HUD as a simple meter (`meter`: its label and fill;
//! the ui crate's item meter at `HudItemEffectMeter`). A death empties
//! them.
//!
//! Known shortcuts (provisional): hype's air jumps (5 by the item's
//! description) are unmeasured; only the item's own hits fill its meter,
//! from the damage as dealt (rounded: the Carbine's 10.2 counts 10); a
//! jump's loss is a share of the boost; the boost cap 99; heads come only
//! from headshot kills with the Bazaar (none lost on a body-shot kill) and
//! have no cap of their own.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::{
    status::{Condition, Conditions},
    weapon::{Inventory, NetPart, Weapon},
};

use super::convert::ItemRecord;
use crate::core::{DamageTaken, Died, Health, Hitgroup, Intent, MovementState, SimClock, Team};

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

/// Which meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeterKind {
    Hype,
    Boost,
    Crikey,
    Heads,
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
    owners: Query<(&Intent, &Inventory, Option<&MovementState>), Without<crate::map::interp::NetDrawn>>,
    mut meters: Query<&mut Tf2Meter>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for (intent, inv, state) in &owners {
        for &w in &inv.weapons {
            let Ok(mut m) = meters.get_mut(w) else { continue };
            let p = m.params.clone();
            let mut s = m.state;
            let press = intent.secondary && !s.prev_alt && inv.active == Some(w) && inv.ready(now);
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
            }
            s.prev_alt = intent.secondary;
            s.prev_jump = intent.jump;
            if m.state != s {
                m.state = s;
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
            if m.params.per_damage <= 0.0 || m.active(now) {
                continue;
            }
            m.state.value = (m.state.value + m.params.per_damage * points).min(m.params.max);
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
        }
    }
}

/// The hook, the part and the systems (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_sim::weapon::{CarriedAppExt, SelectWeapons, data::WeaponDataAppExt};
    app.register_weapon_hook::<MeterParams>(HOOK, |e, _, p| attach(e, p))
        .predicted_part::<Tf2Meter>()
        .add_systems(crate::core::Predict::Select, think.before(SelectWeapons))
        .add_systems(
            FixedUpdate,
            (fill.after(crate::core::apply_damage), buffs.after(crate::core::SimSet::Weapons).before(crate::core::apply_damage))
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
