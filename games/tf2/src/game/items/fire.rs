//! The Pyro's fire family (specs/tf2/batch3b_mechanics.md 2-7), what the
//! other modules don't hold:
//!
//! - **The crit boost** (`weapons::flame::CRIT_BOOST`, a condition in
//!   Chimera's `status::Conditions`; the Phlogistinator's taunt gives it,
//!   `meters`' Mmmph): its holder's flame hits crit (`weapons::flame`),
//!   every other hit too (`effects::crits`), its afterburn ticks never;
//!   while he has it he wears `weapons::crit_boost::CritBoosted` for its
//!   looks (`boost_shown`, only the ones this module put on).
//! - **The Manmelter** (`tf2:revenge`, `RevengeParams`, the part
//!   `Tf2Revenge`, the server's): attack2 with it out puts out each burning
//!   teammate in the airblast's sphere (`reach` units centred `reach`
//!   ahead of the eye, seen from it), each storing a crit (at most `cap`)
//!   and healing the Pyro `heal`; each shot while one is stored spends it
//!   (`revenge_shots`): its flare's hit is a full crit (`CritShots`, taken
//!   by `effects::crits`). Its flare is the flare gun's (3000 units/s,
//!   30, no crit on burning targets: not the flare gun class), its shots
//!   free (an energy cell: data/weapons/flaregun_revenge.toml).
//! - **The Gas Passer** (the jar of `consumables` with its cloud): its
//!   throw breaks into a cloud `cloud_up` (30) over the break that lasts
//!   `cloud` (5 s); enemies within `radius` (220) of it at the break, and
//!   any entering it while it lasts (`clouds`), are coated (`GAS`, 10 s).
//!   The next damage a coated enemy takes (any) takes the gas off and sets
//!   him burning `ignite` (6 s, 4 every 0.5 s) credited to the thrower
//!   (`gas_ignite`). Its meter starts empty and survives deaths; it fills
//!   over 60 s and by the damage its holder deals, 750 to full
//!   (`gas_fill`).
//!
//! The Backburner's crits from behind, the Degreaser's and the airblast's
//! costs are `weapons::flame`'s, `items::docs`' and `weapons::airblast`'s;
//! the Dragon's Fury's fireball is `weapons::shot`'s (its base
//! data/weapons/rocketlauncher_fireball.toml).
//!
//! Known shortcuts (the spec's open questions and what it left
//! unmeasured, provisional): the Phlogistinator's rage comes from the
//! fire damage as dealt (rounded), its taunt has no sequence, sound or
//! look and it isn't healed (measured; the mega-heal condition's use is
//! open); the Manmelter's stored crits are the server's (no HUD count),
//! capped at `REVENGE_CAP` (unmeasured), a crit is spent on the next hit
//! of its flare within `CRIT_WINDOW` of the shot (a miss loses it then),
//! one press puts out every teammate in reach (no cooldown measured), it
//! doesn't reload while holstered (not measured); the Gas Passer's cloud
//! has no look or sound, its coating is to the box centre with no line of
//! sight, its thrower switching back after 0.7 s is the jars' switch to
//! the last weapon at once; the Dragon's Fury's damage by distance is the
//! measured table (linear between), its pressure is the gun's next-shot
//! time (no HUD gauge).

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use serde::{Deserialize, Serialize};

use avian3d::prelude::ColliderAabb;
use chimera_sim::{
    status::{Condition, Conditions},
    weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind, aim_of},
};

use crate::core::{DamageTaken, Health, Intent, MovementState, SimClock, Team};
use crate::game::weapons::{
    airblast::{box_meets_sphere, heal},
    blast::boxed,
    crit_boost::CritBoosted,
    damage::U,
    flame::{Burning, CRIT_BOOST, FlameWorld},
};

/// The Gas Passer's condition (`status::Condition::Game`).
pub const GAS: &str = "tf2:gas";
/// The Manmelter's hook.
pub const REVENGE_HOOK: &str = "tf2:revenge";
/// The Manmelter's stored crits at most (unmeasured: the Diamondback's).
pub const REVENGE_CAP: u32 = 35;
/// A spent crit rides on its flare's hit this long after the shot (s).
pub const CRIT_WINDOW: f64 = 2.0;

/// Shots whose hit crits (attacker, weapon, until when).
#[derive(Clone, Debug, Default)]
pub struct CritShots(pub Vec<(Entity, &'static str, f64)>);

impl CritShots {
    /// Those past their window gone.
    pub fn expire(&mut self, now: f64) {
        self.0.retain(|s| s.2 > now);
    }

    /// Whether `a`'s hit with `weapon` spends one.
    pub fn take(&mut self, a: Entity, weapon: &str) -> bool {
        match self.0.iter().position(|s| s.0 == a && s.1 == weapon) {
            Some(i) => {
                self.0.remove(i);
                true
            }
            None => false,
        }
    }
}

/// `tf2:revenge`'s parameters (units, points).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RevengeParams {
    pub reach: f32,
    pub heal: f32,
}

impl Default for RevengeParams {
    fn default() -> Self {
        Self { reach: 128.0, heal: 20.0 }
    }
}

/// The Manmelter's part: its stored crits (the server's).
#[derive(Component, Clone, Debug)]
pub struct Tf2Revenge {
    pub params: RevengeParams,
    pub stored: u32,
    pub prev_alt: bool,
}

/// The Manmelter's attack2: burning teammates in reach put out, a crit
/// stored and the Pyro healed for each (module doc; the server's).
#[allow(clippy::type_complexity)]
pub fn revenge(
    owners: Query<(Entity, &Intent, &Inventory, &Transform, Option<&MovementState>, Option<&Health>, Option<&Team>)>,
    mut guns: Query<&mut Tf2Revenge>,
    chars: Query<(Entity, &Transform, Option<&ColliderAabb>, Option<&MovementState>, &Health, Option<&Team>), (With<Intent>, With<Burning>)>,
    world: FlameWorld,
    mut commands: Commands,
) {
    for (pyro, intent, inv, at, state, health, team) in &owners {
        let Some(w) = inv.active else { continue };
        let Ok(mut gun) = guns.get_mut(w) else { continue };
        let press = intent.secondary && !gun.prev_alt;
        gun.prev_alt = intent.secondary;
        if !press || health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(team) = team.copied().filter(|t| t.0 != 0) else { continue };
        let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        let aim = aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        let (r, centre) = (gun.params.reach * U, eye + aim * gun.params.reach * U);
        for (e, t, aabb, s, h, their) in &chars {
            if e == pyro || h.current <= 0.0 || their.copied() != Some(team) {
                continue;
            }
            let (min, max) = boxed(t, aabb, s);
            if !box_meets_sphere(centre, r, min, max) || world.line_blocked(eye, (min + max) / 2.0) {
                continue;
            }
            commands.entity(e).remove::<Burning>();
            gun.stored = (gun.stored + 1).min(REVENGE_CAP);
            heal(&mut commands, pyro, gun.params.heal);
        }
    }
}

/// Each Manmelter shot with a crit stored spends it on its flare's hit
/// (the server's, before the hits' crits).
pub fn revenge_shots(mut events: MessageReader<WeaponEvent>, mut guns: Query<(&mut Tf2Revenge, &Weapon)>, mut state: ResMut<super::ItemState>, clock: Res<SimClock>) {
    let shots = &mut state.crit_shots;
    for e in events.read() {
        if e.replay || !matches!(e.kind, WeaponEventKind::Fired { .. }) {
            continue;
        }
        let Ok((mut gun, weapon)) = guns.get_mut(e.weapon) else { continue };
        if gun.stored > 0 {
            gun.stored -= 1;
            shots.0.push((e.owner, weapon.id, clock.now + CRIT_WINDOW));
        }
    }
}

/// A gas cloud standing (module doc).
#[derive(Clone, Debug)]
pub struct GasCloud {
    pub centre: Vec3,
    pub until: f64,
    pub owner: Option<Entity>,
    pub weapon: &'static str,
    /// Those it coated already.
    pub coated: Vec<Entity>,
}

/// The clouds standing, and which weapon coated each coated enemy.
#[derive(Clone, Debug, Default)]
pub struct GasClouds {
    pub live: Vec<GasCloud>,
    pub coated_by: HashMap<Entity, &'static str>,
}

/// Enemies entering a standing cloud are coated (the server's).
#[allow(clippy::type_complexity)]
pub fn clouds(
    mut state: ResMut<super::ItemState>,
    rules: Option<Res<super::consumables::ConsumableRules>>,
    chars: Query<(Entity, &Transform, Option<&ColliderAabb>, Option<&MovementState>, &Health, Option<&Team>), With<Intent>>,
    friendly: Option<Res<crate::core::FriendlyFire>>,
    mut conditions: ResMut<Conditions>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    let clouds = &mut state.gas;
    if clouds.live.is_empty() {
        return;
    }
    let ff = friendly.is_some_and(|f| f.0 != 0);
    let team = |e: Entity| chars.get(e).ok().and_then(|c| c.5.copied()).filter(|t| t.0 != 0);
    clouds.live.retain(|c| c.until > now);
    for cloud in &mut clouds.live {
        let Some(p) = rules.as_deref().and_then(|r| r.get(cloud.weapon)) else { continue };
        for (e, t, aabb, s, h, _) in &chars {
            if h.current <= 0.0 || cloud.coated.contains(&e) || Some(e) == cloud.owner {
                continue;
            }
            if !ff && team(e).is_some() && cloud.owner.and_then(team) == team(e) {
                continue;
            }
            let (min, max) = boxed(t, aabb, s);
            if ((min + max) / 2.0).distance(cloud.centre) > p.radius * U {
                continue;
            }
            conditions.give(e, Condition::Game(GAS), now + p.duration as f64, cloud.owner);
            cloud.coated.push(e);
            clouds.coated_by.insert(e, cloud.weapon);
        }
    }
}

/// A coated enemy's next damage takes the gas off and sets him burning
/// (the server's, after the damage).
pub fn gas_ignite(
    mut taken: MessageReader<DamageTaken>,
    mut conditions: ResMut<Conditions>,
    mut state: ResMut<super::ItemState>,
    rules: Option<Res<super::consumables::ConsumableRules>>,
    health: Query<&Health>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    for t in taken.read() {
        let d = &t.damage;
        if d.amount <= 0.0 {
            continue;
        }
        let Some(held) = conditions.get(d.target, Condition::Game(GAS), now).copied() else { continue };
        conditions.remove(d.target, Condition::Game(GAS));
        let Some(weapon) = state.gas.coated_by.remove(&d.target) else { continue };
        if health.get(d.target).is_ok_and(|h| h.current <= 0.0) {
            continue;
        }
        let secs = rules.as_deref().and_then(|r| r.get(weapon)).map_or(0.0, |p| p.ignite);
        if secs <= 0.0 {
            continue;
        }
        let fire = crate::game::weapons::flame::stock_params();
        commands.entity(d.target).insert(Burning {
            left: secs,
            next: now + fire.burn_interval as f64,
            attacker: held.by,
            weapon,
            damage: fire.burn_damage,
            interval: fire.burn_interval,
            harmless: false,
            from_tick: false,
        });
    }
}

/// The Gas Passer's meter from the damage its holder deals enemies (the
/// server's).
pub fn gas_fill(
    mut taken: MessageReader<DamageTaken>,
    holders: Query<(&Inventory, Option<&Team>)>,
    mut items: Query<&mut super::consumables::Tf2Consumable>,
) {
    for t in taken.read() {
        let d = &t.damage;
        let points = d.amount * 100.0;
        let Some(a) = d.attacker.filter(|a| *a != d.target && points > 0.0) else { continue };
        let Ok((inv, team)) = holders.get(a) else { continue };
        let mates = team.copied().filter(|t| t.0 != 0).is_some_and(|t| holders.get(d.target).ok().and_then(|h| h.1.copied()) == Some(t));
        if mates {
            continue;
        }
        for &w in &inv.weapons {
            if let Ok(mut c) = items.get_mut(w)
                && c.params.damage_fill > 0.0
            {
                let m = (c.state.meter + points / c.params.damage_fill).min(1.0);
                if m != c.state.meter {
                    c.state.meter = m;
                }
            }
        }
    }
}

/// The ones this module put `CritBoosted` on.
#[derive(Clone, Debug, Default)]
pub struct BoostShown(pub HashSet<Entity>);

/// A crit-boosted player wears `CritBoosted` (its looks) while he has the
/// condition (the server's; replicated).
pub fn boost_shown(conditions: Res<Conditions>, mut state: ResMut<super::ItemState>, marked: Query<Has<CritBoosted>>, clock: Res<SimClock>, mut commands: Commands) {
    let now = clock.now;
    let shown = &mut state.boost_shown;
    let boost = Condition::Game(CRIT_BOOST);
    let gone: Vec<Entity> = shown.0.iter().copied().filter(|e| !conditions.has(*e, boost, now)).collect();
    for e in gone {
        shown.0.remove(&e);
        if marked.get(e).unwrap_or(false) {
            commands.entity(e).remove::<CritBoosted>();
        }
    }
    for (e, _) in conditions.0.iter() {
        if conditions.has(*e, boost, now) && !shown.0.contains(e) && marked.get(*e).is_ok_and(|m| !m) {
            shown.0.insert(*e);
            commands.entity(*e).insert(CritBoosted);
        }
    }
}

/// The hook and the systems (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_sim::weapon::data::WeaponDataAppExt;
    app.register_weapon_hook::<RevengeParams>(REVENGE_HOOK, |e, _, p| {
            e.insert(Tf2Revenge { params: p.clone(), stored: 0, prev_alt: false });
        })
        .add_systems(
            FixedUpdate,
            (
                (revenge, clouds).after(crate::core::SimSet::Weapons).before(crate::core::apply_damage),
                revenge_shots.after(crate::core::SimSet::Weapons).before(super::effects::crits),
                (gas_ignite, gas_fill).after(crate::core::apply_damage),
                boost_shown.after(crate::core::SimSet::Weapons),
            )
                .run_if(crate::core::authoritative),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crit_shot_is_taken_once_and_expires() {
        let mut s = CritShots::default();
        let a = Entity::from_raw_u32(7).unwrap();
        s.0.push((a, "tf2:manmelter", 2.0));
        assert!(!s.take(a, "tf2:other"));
        assert!(s.take(a, "tf2:manmelter"));
        assert!(!s.take(a, "tf2:manmelter"));
        s.0.push((a, "tf2:manmelter", 2.0));
        s.expire(2.5);
        assert!(s.0.is_empty());
    }
}
