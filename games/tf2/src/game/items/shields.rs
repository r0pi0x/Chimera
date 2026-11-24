//! The Demoman's shields and their charge (specs/tf2/batch3_mechanics.md
//! 1, the condition; batch3b_mechanics.md 1, the shield itself): the hook
//! `tf2:shield` (`ShieldParams`, on the abstract base
//! data/weapons/demoshield.toml) and the weapon part `Tf2Shield`
//! (predicted: the charge moves its holder, so a client runs it and the
//! server's meter is a correction), and the hook `tf2:worn` (`Unselectable`:
//! a worn item, a shield or the boots, is carried, never drawn).
//!
//! - **The charge** (`think`, predicted, before the weapon switch): attack2
//!   with any weapon out starts it, only from a full meter (99: nothing).
//!   It drives him forward along his view with no key held (`Intent`'s
//!   forward set). While it lasts the meter drains from 100 over `time` s
//!   plus the items' `mod_charge_time` (the Claidheamh Mòr's + 0.5 while
//!   out: 2.0 s), one point a tick at 1.5 s; the max speed is `speed`
//!   (750) × the melee weapon's move-speed attributes (the Skullcutter's
//!   × 0.85: 637.5; heads and boosts don't count), past TF2's 520
//!   (`movement::select_class`, `SourceTraits::ceiling`). It ends when the
//!   meter is empty, when he is stopped (from `STOP_AFTER` s on, his speed
//!   under `STOP_SPEED`: a wall), on a melee swing (that tick) or on an
//!   impact; the meter is then 0. It refills from 0 to 100 in `recharge` s
//!   (12) × the items' `charge_recharge_rate` (the Splendid Screen's 1.5:
//!   8 s).
//! - **Turning** while charging is capped at `turn_cap` degrees/s (48) ×
//!   the items' `charge_turn_control` (the boots' 3: 144); from
//!   `FREE_TURN` up (the Tide Turner's 50) it is free. The cap holds the
//!   charge's heading, which the tick's view yaw is set to (`Intent::yaw`:
//!   his movement and his swing follow it).
//! - **The swing's crit by meter** (`Boost::at`): the meter when he swings
//!   (or meets an enemy) decides: above `mini_below` (75) a normal hit, from
//!   75 down to `crit_below` + 1 a mini-crit, `crit_below` (40) and below a
//!   crit, never past the shield's `boost_max` (the Tide Turner's
//!   mini-crit: data/items.toml); the boost lasts `linger` s (0.3) after
//!   the charge ends, covering the smack, through
//!   `weapons::melee::Tf2Melee::boost`.
//! - **Impact** (`impacts`, the server's): a charge reaching an enemy (his
//!   origin within `IMPACT_REACH` across, ahead of the heading) ends and
//!   hits him, never a crit, for min(`impact_cap`, `impact_base` +
//!   `impact_slope` × u) × the items' `charge_impact_damage` (the
//!   Splendid Screen's 1.7) × (1 + `head_bonus` × his heads, at most
//!   `head_cap`), u the meter used (`impact_damage`), credited to the
//!   shield.
//! - **On the server** (`effects`): starting a charge removes the debuffs
//!   (burning, bleeding, Jarate, Mad Milk; marked for death stays);
//!   damage taken while charging with `lose_demo_charge_on_damage_when_
//!   charging` (the Tide Turner) takes that many points off the meter
//!   (55 → − 55); a melee or impact kill refills the items'
//!   `kill_refills_meter` × 100 at once (the Tide Turner's 75, the boots'
//!   and the Claidheamh Mòr's 25). A death fills the meter.
//! - **The rest** are the items' player numbers (`super::mods`): the
//!   shields' fire and blast resistances (others' damage only), the boots'
//!   + 25 max health and × 1.1 walking speed with a shield carried.
//!
//! The meter shows as TF2's item meter (`meter`: CHARGE). Shields and
//! boots are utility items: no ladder draws them as a gun (data/items.toml);
//! the boots may ride with a rung as its passive (`super::super::passives`,
//! off by default; the shields wait for P3's key rule).
//!
//! Known shortcuts: the drive reaches 750 with the ground's acceleration
//! (five ticks; the shield's 0 → 218 → 732 → 750 is quicker) and, ended,
//! his speed is clamped to walking at once (the shield's decays by
//! friction); the impact's law is fitted (batch3b's open question), its
//! reach `IMPACT_REACH` covers the measured 48 to 60, and the Splendid
//! Screen's knock on the victim isn't built; the stop rule (`STOP_AFTER`,
//! `STOP_SPEED`) is fitted on the one no-input run of the condition; the
//! turn cap holds the heading, not the camera (a client's view turns
//! freely, his movement and swing don't), and is the base rate at the
//! measuring client's frame rate (no frame-time law); the "full" turn
//! control's curve is `FREE_TURN`'s step; the impact is the server's (a
//! client's prediction is corrected); the Persian Persuader's meter from
//! hits and ammo isn't built; the charge has no look or sound of its own
//! (no crit glow on the boost) and the shield isn't drawn on his arm (its
//! model has no asset set).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::{
    status::{Bleeding, Condition, Conditions},
    weapon::{Inventory, NetPart, Unselectable, Weapon},
};

use super::mods::Tf2Item;
use crate::core::{Damage, DamageKind, DamageTaken, Died, Health, Hitgroup, Intent, SimClock, Team, Velocity};
use crate::game::weapons::{damage::U, flame::Burning, melee::Tf2Melee};

/// The hooks.
pub const HOOK: &str = "tf2:shield";
pub const WORN_HOOK: &str = "tf2:worn";
/// A full meter.
pub const FULL: f32 = 100.0;
/// From this turn control up the turn is free (the Tide Turner's 50; the
/// "full" attribute's curve is unmeasured).
pub const FREE_TURN: f32 = 10.0;
/// The stop rule (fitted: with no input the charge ended 12 ticks in).
pub const STOP_AFTER: f64 = 0.17;
pub const STOP_SPEED: f32 = 100.0;
/// The impact's reach (origin to origin across, units; measured 48 to 60).
pub const IMPACT_REACH: f32 = 60.0;
/// How far ahead of the heading an impact is (cosine of the angle).
pub const IMPACT_AHEAD: f32 = 0.5;

/// A boost on the next swing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Boost {
    #[default]
    None,
    Mini,
    Crit,
}

impl Boost {
    /// The swing's boost at `meter` (module doc), at most `max`.
    pub fn at(meter: f32, p: &ShieldParams) -> Boost {
        let b = if meter <= p.crit_below + 1e-3 {
            Boost::Crit
        } else if meter <= p.mini_below + 1e-3 {
            Boost::Mini
        } else {
            Boost::None
        };
        if b.code() > p.boost_max.code() { p.boost_max } else { b }
    }

    /// `weapons::melee::Tf2Melee::boost`'s number.
    pub fn code(self) -> u8 {
        match self {
            Boost::None => 0,
            Boost::Mini => 1,
            Boost::Crit => 2,
        }
    }
}

/// `tf2:shield`'s parameters (units, s, meter points; module doc).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ShieldParams {
    pub speed: f32,
    pub time: f32,
    pub recharge: f32,
    pub mini_below: f32,
    pub crit_below: f32,
    pub boost_max: Boost,
    pub linger: f32,
    pub turn_cap: f32,
    pub impact_base: f32,
    pub impact_slope: f32,
    pub impact_cap: f32,
    pub head_bonus: f32,
    pub head_cap: u32,
}

impl Default for ShieldParams {
    fn default() -> Self {
        Self {
            speed: 750.0,
            time: 1.5,
            recharge: 12.0,
            mini_below: 75.0,
            crit_below: 40.0,
            boost_max: Boost::Crit,
            linger: 0.3,
            turn_cap: 48.0,
            impact_base: 8.0,
            impact_slope: 0.7,
            impact_cap: 50.0,
            head_bonus: 0.1,
            head_cap: 5,
        }
    }
}

/// The charge's state (predicted).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChargeState {
    /// 0 to 100.
    pub meter: f32,
    pub charging: bool,
    /// The charge's start (sim time).
    pub since: f64,
    /// The charge's heading (yaw, radians).
    pub heading: f32,
    /// The boost on the next swing and until when.
    pub boost: Boost,
    pub boost_until: f64,
    pub prev_alt: bool,
    pub prev_fire: bool,
}

impl Default for ChargeState {
    fn default() -> Self {
        Self { meter: FULL, charging: false, since: f64::MIN, heading: 0.0, boost: Boost::None, boost_until: f64::MIN, prev_alt: false, prev_fire: false }
    }
}

/// The weapon part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Shield {
    pub params: ShieldParams,
    pub state: ChargeState,
}

impl NetPart for Tf2Shield {
    type Net = ChargeState;
    fn net(&self) -> ChargeState {
        self.state
    }
    fn set_net(&mut self, net: ChargeState) {
        self.state = net;
    }
}

impl Tf2Shield {
    /// The HUD meter: its label and fill.
    pub fn meter(&self) -> (&str, f32) {
        ("CHARGE", (self.state.meter / FULL).clamp(0.0, 1.0))
    }
}

/// What a holder's items fold into for the charge (module doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Charge {
    /// Seconds added to the charge.
    pub time: f32,
    pub recharge: f32,
    pub turn: f32,
    pub impact: f32,
    /// Meter refilled by a melee kill (points).
    pub refill: f32,
    pub lose_on_hurt: bool,
    /// The melee weapon's move-speed attributes.
    pub melee_speed: f32,
}

impl Charge {
    /// The items of `inv` (`items` reads a weapon's numbers and slot).
    pub fn of(inv: &Inventory, items: &impl Fn(Entity) -> Option<(super::mods::ItemParams, u8)>) -> Charge {
        let mut c = Charge { time: 0.0, recharge: 1.0, turn: 1.0, impact: 1.0, refill: 0.0, lose_on_hurt: false, melee_speed: 1.0 };
        for &w in &inv.weapons {
            let Some((p, slot)) = items(w) else { continue };
            let active = inv.active == Some(w);
            if slot == super::consumables::MELEE_SLOT {
                c.melee_speed *= p.move_speed * if active { p.move_speed_active } else { 1.0 };
            }
            if !(active || !p.provide_on_active) {
                continue;
            }
            c.time += p.charge_time;
            c.recharge *= p.charge_recharge;
            c.turn *= p.charge_turn;
            c.impact *= p.charge_impact;
            c.refill += p.kill_refill * FULL;
            c.lose_on_hurt |= p.lose_charge_on_hurt;
        }
        c
    }

    /// The turn cap (degrees/s) with the shield's base `cap`; None: free.
    pub fn turn_cap(&self, cap: f32) -> Option<f32> {
        (self.turn < FREE_TURN).then_some(cap * self.turn)
    }
}

/// `items`' reader for `Charge::of`.
fn reader<'a>(items: &'a Query<(&Tf2Item, &Weapon)>) -> impl Fn(Entity) -> Option<(super::mods::ItemParams, u8)> + 'a {
    move |w| items.get(w).ok().map(|(i, x)| (i.0.clone(), x.slot))
}

/// The charging shield of `inv`, if any, with its holder's charge.
pub fn charging<'a>(inv: &Inventory, shields: &'a Query<&Tf2Shield>) -> Option<&'a Tf2Shield> {
    inv.weapons.iter().filter_map(|w| shields.get(*w).ok()).find(|s| s.state.charging)
}

/// The max speed of a holder charging (module doc), None when not.
pub fn charge_speed(inv: &Inventory, shields: &Query<&Tf2Shield>, items: &Query<(&Tf2Item, &Weapon)>) -> Option<f32> {
    let s = charging(inv, shields)?;
    Some(s.params.speed * Charge::of(inv, &reader(items)).melee_speed)
}

/// The impact's damage (points, before rounding) with `used` meter points
/// used, the items' factor `mult` and `heads` (module doc).
pub fn impact_damage(p: &ShieldParams, used: f32, mult: f32, heads: u32) -> f32 {
    (p.impact_base + p.impact_slope * used.max(0.0)).min(p.impact_cap) * mult * (1.0 + p.head_bonus * heads.min(p.head_cap) as f32)
}

/// `tf2:shield` on a weapon: full.
pub fn attach(e: &mut EntityWorldMut, p: &ShieldParams) {
    e.insert(Tf2Shield { params: p.clone(), state: ChargeState::default() });
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(t) - std::f32::consts::PI
}

/// Each shield's charge (module doc), predicted, before the weapon
/// switch: by the shields, not the characters (a query over every
/// character's archetypes costs each tick).
#[allow(clippy::type_complexity)]
pub fn think(
    mut shields: Query<(&mut Tf2Shield, &Weapon)>,
    mut owners: Query<(&mut Intent, &Inventory, Option<&Health>, Option<&Velocity>), Without<crate::map::interp::NetDrawn>>,
    items: Query<(&Tf2Item, &Weapon)>,
    mut melees: Query<&mut Tf2Melee>,
    clock: Res<SimClock>,
) {
    let (now, dt) = (clock.now, clock.dt());
    for (mut shield, weapon) in &mut shields {
        let Some(owner) = weapon.owner else { continue };
        let Ok((mut intent, inv, health, velocity)) = owners.get_mut(owner) else { continue };
        let p = shield.params.clone();
        let mut s = shield.state;
        if health.is_some_and(|h| h.current <= 0.0) {
            s = ChargeState { prev_alt: s.prev_alt, prev_fire: s.prev_fire, ..default() };
        } else {
            let c = Charge::of(inv, &reader(&items));
            let melee_out = inv.active.is_some_and(|w| melees.contains(w));
            let press_alt = intent.secondary && !s.prev_alt;
            let press_fire = intent.fire && !s.prev_fire;
            if !s.charging {
                s.meter = (s.meter + FULL / p.recharge.max(1e-3) * c.recharge * dt).min(FULL);
                if press_alt && s.meter >= FULL - 1e-3 {
                    s.charging = true;
                    s.since = now;
                    s.heading = intent.yaw;
                }
            }
            if s.charging {
                // The turn cap holds the heading.
                if let Some(cap) = c.turn_cap(p.turn_cap) {
                    let most = cap.to_radians() * dt;
                    s.heading = wrap(s.heading + wrap(intent.yaw - s.heading).clamp(-most, most));
                } else {
                    s.heading = intent.yaw;
                }
                if intent.yaw != s.heading {
                    intent.yaw = s.heading;
                }
                // The drive: forward, no key needed.
                intent.move_axis = Vec2::new(0.0, 1.0);
                s.meter -= FULL / (p.time + c.time).max(1e-3) * dt;
                // The boost a swing now would get (and for `linger` after).
                s.boost = Boost::at(s.meter, &p);
                s.boost_until = now + p.linger as f64;
                let speed = velocity.map_or(0.0, |v| v.0.with_y(0.0).length() / U);
                let stopped = now - s.since >= STOP_AFTER - 1e-6 && speed < STOP_SPEED;
                let swing = press_fire && melee_out && now > s.since + 1e-6;
                if s.meter <= 1e-3 || stopped || swing {
                    s.charging = false;
                    s.meter = 0.0;
                }
            }
        }
        s.prev_alt = intent.secondary;
        s.prev_fire = intent.fire;
        // The boost on the melee weapon in hand.
        let boost = if s.boost_until > now { s.boost.code() } else { 0 };
        if let Some(w) = inv.active
            && let Ok(mut m) = melees.get_mut(w)
            && m.boost != boost
        {
            m.boost = boost;
        }
        if shield.state != s {
            shield.state = s;
        }
    }
}

/// A charge's impact on an enemy (module doc; the server's).
#[allow(clippy::type_complexity)]
pub fn impacts(
    mut shields: Query<(&mut Tf2Shield, &Weapon)>,
    chars: Query<(Entity, &Transform, &Health, Option<&Team>), With<Intent>>,
    heads: Query<&super::effects::Heads>,
    holders: Query<&Inventory>,
    items: Query<(&Tf2Item, &Weapon)>,
    friendly: Option<Res<crate::core::FriendlyFire>>,
    clock: Res<SimClock>,
    mut out: MessageWriter<Damage>,
) {
    let ff = friendly.is_some_and(|f| f.0 != 0);
    for (mut shield, weapon) in &mut shields {
        if !shield.state.charging {
            continue;
        }
        let Some(owner) = weapon.owner else { continue };
        let Ok((_, at, _, team)) = chars.get(owner) else { continue };
        let team = team.copied().filter(|t| t.0 != 0);
        let ahead = Quat::from_rotation_y(shield.state.heading) * Vec3::NEG_Z;
        let hit = chars.iter().find(|(e, t, h, their)| {
            let to = (t.translation - at.translation) / U;
            *e != owner
                && h.current > 0.0
                && (ff || team.is_none() || their.copied() != team)
                && to.with_y(0.0).length() <= IMPACT_REACH
                && to.y.abs() <= 72.0
                && to.with_y(0.0).normalize_or_zero().dot(ahead) >= IMPACT_AHEAD
        });
        let Some((victim, ..)) = hit else { continue };
        let c = holders.get(owner).map_or(1.0, |inv| Charge::of(inv, &reader(&items)).impact);
        let n = heads.get(owner).map_or(0, |h| h.0);
        let points = impact_damage(&shield.params, FULL - shield.state.meter, c, n);
        out.write(Damage {
            target: victim,
            attacker: Some(owner),
            amount: crate::game::weapons::damage::round_points(points / 100.0),
            point: at.translation,
            dir: ahead,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Generic,
            weapon: Some(weapon.id),
            force: Vec3::ZERO,
        });
        let p = shield.params.clone();
        let s = &mut shield.state;
        // The swing after it: the meter's boost then (module doc).
        s.boost = Boost::at(s.meter, &p);
        s.boost_until = clock.now + p.linger as f64;
        s.charging = false;
        s.meter = 0.0;
    }
}

/// The charge's server effects (module doc): debuffs off at the start, the
/// Tide Turner's meter loss, the kill refills.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn effects(
    mut taken: MessageReader<DamageTaken>,
    mut died: MessageReader<Died>,
    mut shields: Query<(&mut Tf2Shield, &Weapon)>,
    holders: Query<&Inventory>,
    items: Query<(&Tf2Item, &Weapon)>,
    debuffed: Query<(Has<Burning>, Has<Bleeding>)>,
    mut conditions: ResMut<Conditions>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    let charge_of = |owner: Entity| holders.get(owner).ok().map(|inv| Charge::of(inv, &reader(&items)));
    let shield_of = |owner: Entity, shields: &Query<(&mut Tf2Shield, &Weapon)>| {
        holders.get(owner).ok().and_then(|inv| inv.weapons.iter().copied().find(|w| shields.contains(*w)))
    };
    // Debuffs off as a charge starts.
    for (shield, weapon) in &shields {
        if !(shield.state.charging && (shield.state.since - now).abs() < 1e-6) {
            continue;
        }
        let Some(owner) = weapon.owner else { continue };
        if let Ok((burning, bleeding)) = debuffed.get(owner) {
            if burning {
                commands.entity(owner).remove::<Burning>();
            }
            if bleeding {
                commands.entity(owner).remove::<Bleeding>();
            }
        }
        // Marked for death stays (batch3b 1).
        for c in [Condition::Game(super::consumables::JARATE), Condition::Game(super::consumables::MILK)] {
            conditions.remove(owner, c);
        }
    }
    for t in taken.read() {
        let d = &t.damage;
        let points = d.amount * 100.0;
        if points <= 0.0 {
            continue;
        }
        let Some(w) = shield_of(d.target, &shields) else { continue };
        if !charge_of(d.target).is_some_and(|c| c.lose_on_hurt) {
            continue;
        }
        if let Ok((mut s, _)) = shields.get_mut(w)
            && s.state.charging
        {
            s.state.meter = (s.state.meter - points).max(0.0);
        }
    }
    for k in died.read() {
        let d = &k.damage;
        let Some(a) = d.attacker.filter(|a| *a != k.entity) else { continue };
        let Some(w) = shield_of(a, &shields) else { continue };
        let impact = d.weapon.is_some_and(|n| shields.get(w).is_ok_and(|(_, x)| x.id == n));
        if d.kind != DamageKind::Melee && !impact {
            continue;
        }
        let refill = charge_of(a).map_or(0.0, |c| c.refill);
        if refill > 0.0
            && let Ok((mut s, _)) = shields.get_mut(w)
        {
            s.state.meter = (s.state.meter + refill).min(FULL);
        }
    }
}

/// The hooks, the part and the systems (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_sim::weapon::{CarriedAppExt, SelectWeapons, data::WeaponDataAppExt};
    app.register_weapon_hook::<ShieldParams>(HOOK, |e, _, p| attach(e, p))
        .register_weapon_hook::<toml::Table>(WORN_HOOK, |e, _, _| {
            e.insert(Unselectable);
        })
        .predicted_part::<Tf2Shield>()
        .add_systems(crate::core::Predict::Select, think.before(SelectWeapons))
        .add_systems(
            FixedUpdate,
            (
                impacts.after(crate::core::SimSet::Movement).before(crate::core::SimSet::Weapons),
                effects.after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_turn_cap_follows_the_control() {
        let c = |turn: f32| Charge { time: 0.0, recharge: 1.0, turn, impact: 1.0, refill: 0.0, lose_on_hurt: false, melee_speed: 1.0 };
        assert_eq!(c(1.0).turn_cap(48.0), Some(48.0));
        assert_eq!(c(3.0).turn_cap(48.0), Some(144.0));
        assert_eq!(c(50.0).turn_cap(48.0), None);
        assert!((wrap(3.5 * std::f32::consts::PI) + 0.5 * std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn the_impact_and_the_swing_follow_batch3b_1() {
        let p = ShieldParams::default();
        let d = |u: f32, m: f32, h: u32| impact_damage(&p, u, m, h).round();
        assert_eq!(d(53.0, 1.0, 0), 45.0, "meter 47");
        assert_eq!(d(71.0, 1.0, 0), 50.0, "capped");
        assert_eq!(d(71.0, 1.7, 0), 85.0, "the Splendid Screen");
        assert_eq!(d(71.0, 1.0, 4), 70.0, "4 heads");
        assert_eq!(d(26.0, 1.0, 6), 39.0, "heads at most 5");
        assert_eq!(Boost::at(76.0, &p), Boost::None);
        assert_eq!(Boost::at(75.0, &p), Boost::Mini);
        assert_eq!(Boost::at(41.0, &p), Boost::Mini);
        assert_eq!(Boost::at(40.0, &p), Boost::Crit);
        let tide = ShieldParams { boost_max: Boost::Mini, ..p };
        assert_eq!(Boost::at(10.0, &tide), Boost::Mini);
    }
}
