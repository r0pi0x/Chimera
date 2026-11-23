//! Pushes some unlocks give (specs/tf2/batch2b_mechanics.md 8-10), each
//! through the shared push (`blast::BlastWorld`, Chimera's `ExternalPush`):
//!
//! - **Force-a-Nature** (9; the item's `set_scattergun_has_knockback`,
//!   `items::mods::ItemParams::knockback`; `knockback`): a shot's hit on
//!   an enemy within `KNOCK_RANGE` of the shooter sets his velocity away
//!   from the shooter at `KNOCK_PER_DAMAGE` × the shot's damage
//!   horizontally and `knock_up` (d) up, in place of the bullets' push;
//!   farther, the bullets' push alone. A shot fired in the air pushes the
//!   shooter opposite his aim by `self_push` (p) (`self_push`).
//! - **The Shortstop's shove** (10; `tf2:shove`, `ShoveParams`, the part
//!   `Tf2Shove`; `shove`): attack2 with it out, every `cooldown` (1.5 s),
//!   shoves `delay` (0.25 s) later: the first enemy whose box a line of
//!   `reach` from the eye along the aim meets takes `damage` (1) and his
//!   velocity becomes `speed` (400) along the aim raised to at least
//!   `min_up` degrees; a grounded one `speed` horizontally and
//!   `ground_up` up.
//! - **The Ullapool Caber** (8; `tf2:caber`, a blast's numbers, the part
//!   `Tf2Caber`; `caber`): its first swing that hits explodes at the hit
//!   (the shared blast, its owner in it as in his own); later hits are
//!   the swing alone until its holder dies.
//!
//! Known shortcuts (the spec's open questions, provisional): the
//! knockback's lift law (`knock_up`, fitted to the table) and range (390,
//! between the measured 375 and 400); the self push is the fitted law,
//! not predicted (the server's push corrects the shooter's client); the
//! shove's reach (75, between 60 and 90) and its lift; the Caber's blast
//! (75 at the centre, radius 146, the Demoman's own × 0.75, fitted to the
//! measured 69 and 52) and its refill only on death (no resupply in a
//! party match).

use std::sync::Arc;

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{BulletHit, Inventory, Weapon, WeaponEvent, WeaponEventKind, aim_of, projectile::ray_box};

use super::{
    blast::{Blast, BlastParams, BlastWorld, boxed},
    damage::{HP, U},
};
use crate::core::{Damage, DamageKind, Died, Health, Hitgroup, Intent, MovementState, SimClock};
use crate::game::items::mods::Tf2Item;

/// The Force-a-Nature's knockback (9): its reach (units), horizontal
/// speed per point of damage.
pub const KNOCK_RANGE: f32 = 390.0;
pub const KNOCK_PER_DAMAGE: f32 = 7.9;

/// The knockback's lift at `d` units (fitted: 401 at 50, 311 at 150, about
/// 275 beyond 200).
pub fn knock_up(d: f32) -> f32 {
    (450.0 - 0.9 * d).max(275.0)
}

/// The shooter's own push (units/s) for an aim `p` degrees below the
/// horizon (fitted: 300 + 183 sin p, opposite the aim).
pub fn self_push(p: f32) -> f32 {
    300.0 + 183.0 * p.to_radians().sin()
}

/// The hook of the Shortstop's shove.
pub const SHOVE_HOOK: &str = "tf2:shove";
/// The hook of the Caber's blast.
pub const CABER_HOOK: &str = "tf2:caber";

/// `tf2:shove`'s parameters (units, seconds, points).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShoveParams {
    pub delay: f32,
    pub cooldown: f32,
    pub reach: f32,
    pub damage: f32,
    pub speed: f32,
    pub min_up: f32,
    pub ground_up: f32,
}

impl ShoveParams {
    /// The victim's velocity (units/s) for an aim `dir` (unit), grounded
    /// or not.
    pub fn velocity(&self, dir: Vec3, grounded: bool) -> Vec3 {
        let flat = dir.with_y(0.0).normalize_or(Vec3::NEG_Z);
        if grounded {
            return flat * self.speed + Vec3::Y * self.ground_up;
        }
        let up = dir.y.asin().max(self.min_up.to_radians());
        (flat * up.cos() + Vec3::Y * up.sin()) * self.speed
    }
}

/// The Shortstop's shove state (the server's).
#[derive(Component, Clone, Debug)]
pub struct Tf2Shove {
    pub params: Arc<ShoveParams>,
    pub next: f64,
    pub due: Option<f64>,
}

/// `tf2:shove` on a weapon.
pub fn attach_shove(e: &mut EntityWorldMut, p: &ShoveParams) {
    e.insert(Tf2Shove { params: Arc::new(p.clone()), next: 0.0, due: None });
}

/// The Caber's blast and whether it went off (the server's).
#[derive(Component, Clone, Debug)]
pub struct Tf2Caber {
    pub params: Arc<BlastParams>,
    pub spent: bool,
}

/// `tf2:caber` on a weapon.
pub fn attach_caber(e: &mut EntityWorldMut, p: &BlastParams) {
    e.insert(Tf2Caber { params: Arc::new(p.clone()), spent: false });
}

/// The Force-a-Nature's hits and shots (module doc).
pub fn knockback(
    mut hits: MessageReader<BulletHit>,
    mut events: MessageReader<WeaponEvent>,
    items: Query<&Tf2Item>,
    mut world: BlastWorld,
) {
    for h in hits.read() {
        let d = &h.damage;
        if d.amount <= 0.0 || !items.get(h.weapon).is_ok_and(|i| i.0.knockback) {
            continue;
        }
        let Some(a) = d.attacker.filter(|a| *a != d.target) else { continue };
        let (Some(from), Some(to)) = (world.centre(a), world.centre(d.target)) else { continue };
        let dist = from.distance(to) / U;
        if dist > KNOCK_RANGE {
            continue;
        }
        let away = (to - from).with_y(0.0).normalize_or(Vec3::NEG_Z);
        world.set_push(d.target, away * KNOCK_PER_DAMAGE * d.amount / HP + Vec3::Y * knock_up(dist));
    }
    for e in events.read() {
        let WeaponEventKind::Fired { yaw, pitch, .. } = e.kind else { continue };
        if e.replay || !items.get(e.weapon).is_ok_and(|i| i.0.knockback) || world.grounded(e.owner) {
            continue;
        }
        let dir = aim_of(yaw, pitch) * Vec3::NEG_Z;
        let below = (-dir.y).clamp(-1.0, 1.0).asin().to_degrees();
        world.add_push(e.owner, -dir * self_push(below));
    }
}

/// The Shortstop's shoves (module doc).
#[allow(clippy::type_complexity)]
pub fn shove(
    owners: Query<(Entity, &Intent, &Inventory, &Transform, Option<&MovementState>, Option<&Health>)>,
    mut guns: Query<(&Weapon, &mut Tf2Shove)>,
    clock: Res<SimClock>,
    mut world: BlastWorld,
) {
    let now = clock.now;
    for (owner, intent, inv, at, state, health) in &owners {
        let Some(w) = inv.active else { continue };
        let Ok((weapon, mut gun)) = guns.get_mut(w) else { continue };
        if health.is_some_and(|h| h.current <= 0.0) {
            gun.due = None;
            continue;
        }
        if gun.due.is_none() && intent.secondary && now + 1e-6 >= gun.next {
            gun.due = Some(now + gun.params.delay as f64);
            gun.next = now + gun.params.cooldown as f64;
        }
        if !gun.due.is_some_and(|d| now + 1e-6 >= d) {
            continue;
        }
        gun.due = None;
        let p = gun.params.clone();
        let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        let dir = aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        let target = world
            .chars
            .iter()
            .filter(|c| c.0 != owner && c.3.current > 0.0)
            .filter_map(|(e, t, aabb, _, _, s, ..)| {
                let (lo, hi) = boxed(t, aabb, s);
                Some((ray_box(lo, hi, eye, dir, p.reach * U)?, e))
            })
            .filter(|(_, e)| !world.mates(owner, *e))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, e)| e);
        let Some(v) = target else { continue };
        world.hurt(
            Damage {
                target: v,
                attacker: Some(owner),
                amount: p.damage * HP,
                point: eye,
                dir,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Melee,
                weapon: Some(weapon.id),
                force: Vec3::ZERO,
            },
            eye,
        );
        let grounded = world.grounded(v);
        world.set_push(v, p.velocity(dir, grounded));
    }
}

/// The Caber's first hit explodes (module doc).
pub fn caber(
    mut events: MessageReader<WeaponEvent>,
    mut died: MessageReader<Died>,
    mut cabers: Query<(&Weapon, &mut Tf2Caber)>,
    holders: Query<&Inventory>,
    mut world: BlastWorld,
) {
    for d in died.read() {
        let Ok(inv) = holders.get(d.entity) else { continue };
        for w in &inv.weapons {
            if let Ok((_, mut c)) = cabers.get_mut(*w) {
                c.spent = false;
            }
        }
    }
    for e in events.read() {
        let WeaponEventKind::Swing { hit: true, at: Some((point, normal, target)), .. } = e.kind else { continue };
        if e.replay {
            continue;
        }
        let Ok((weapon, mut c)) = cabers.get_mut(e.weapon) else { continue };
        if c.spent {
            continue;
        }
        c.spent = true;
        let p = c.params.clone();
        let blast = Blast { params: &p, radius: p.radius, ramp: None, scale: 1.0, mini_crit: false, self_law: None };
        let at = world.touched(point, normal, Some(target));
        world.detonate(&blast, &at, Some(e.owner), weapon.id, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fitted_laws_meet_the_tables() {
        // 9: 93 at 300 -> about (735, 275); the self push at 89 degrees.
        assert!((KNOCK_PER_DAMAGE * 93.0 - 754.0).abs() < 25.0);
        assert!((knock_up(50.0) - 401.0).abs() < 10.0 && (knock_up(300.0) - 273.0).abs() < 10.0);
        assert!((self_push(89.0) - 483.0).abs() < 1.0);
        // 10: grounded (400, 262); level in the air, 25 degrees up.
        let p = ShoveParams { delay: 0.25, cooldown: 1.5, reach: 75.0, damage: 1.0, speed: 400.0, min_up: 25.0, ground_up: 262.0 };
        let g = p.velocity(Vec3::NEG_Z, true);
        assert!((g.z + 400.0).abs() < 1e-3 && (g.y - 262.0).abs() < 1e-3);
        let a = p.velocity(Vec3::NEG_Z, false);
        assert!((a.length() - 400.0).abs() < 1e-3 && (a.y - 169.0).abs() < 1.0);
    }
}
