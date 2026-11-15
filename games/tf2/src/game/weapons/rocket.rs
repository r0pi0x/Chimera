//! The Rocket Launcher's rocket (specs/tf2/weapons.md 5), on Chimera's
//! flyers: `chimera:projectile` flies it (1100 units/s, no gravity, from
//! the eye 23.5 forward, 12 right, 3 down (crouched 8 up), through its
//! owner and his teammates, reporting its touch); `tf2:rocket`
//! (`RocketParams`) explodes it where it touched (`explode`, the server's,
//! in `FlyerSet::Touch`):
//!
//! - every living character within `radius` (146) of the explosion
//!   centre, with a clear line from it to its centre (the world blocks),
//!   takes `damage` (90) x the splash f(r) = 1 - (1 - `edge`) r / radius
//!   (linear to 50 % at the edge) x the distance modifier between the
//!   owner's and its centre at the explosion (`damage::modifier`, the
//!   gun's ramp 0.5); the player the rocket touched takes f = 1;
//! - the owner his own blast with no distance modifier, x `self_air_scale`
//!   (0.6) when airborne (5.3);
//! - each is pushed from the centre by `points x scale` units/s in our
//!   reading (`PUSH_PER_POINT`, open question 9), the scale 6 for
//!   others, 10 for the owner airborne (a rocket jump), 5 on the ground,
//!   over the TF2 standing box's volume against the victim's (a crouched,
//!   smaller hull goes farther; 5.4); a push upward of more than `LIFT`
//!   takes him off the ground;
//! - `BaseExplosionEffect.Sound` there, heard by every client
//!   (`map::GameSound`).
//!
//! Each damage rounds to whole points, of kind blast, naming the gun.
//!
//! Known shortcuts: the push per damage point is ours (1 unit/s per
//! point and scale: open question 9 and probe P3 settle it); the Heavy's
//! half knockback waits for the classes (M3); the explosion's look
//! (particles, a scorch) isn't drawn yet; the splash measures to the
//! victim's centre (open question 8); crits are off.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Inflicted,
    data::WeaponSet,
    projectile::{Flyer, FlyerGone, FlyerHit},
};

use super::damage::{HP, U, modifier, round_points};
use crate::{
    core::{Damage, DamageKind, FriendlyFire, Health, Hitgroup, Intent, MovementState, SOLID_LAYERS, Team, Velocity},
    map::{GameSound, PlaySound},
};

/// `tf2:rocket`'s parameters (data units: units, points).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RocketParams {
    pub damage: f32,
    pub radius: f32,
    /// The splash's factor at the radius.
    pub edge: f32,
    /// The owner's own blast while airborne.
    pub self_air_scale: f32,
    /// Push scales: others', the owner's airborne and on the ground.
    pub push_other: f32,
    pub push_self_air: f32,
    pub push_self_ground: f32,
    /// The explosion's sound entry.
    #[serde(default)]
    pub sound: Option<String>,
}

/// The weapon part (the rocket's rules are read from `RocketRules`, as
/// the gun may be gone when its rocket lands).
#[derive(Component, Clone, Debug)]
pub struct Tf2Rocket(pub RocketParams);

/// Each rocket weapon's rules and ramp, by weapon id.
#[derive(Resource, Clone, Debug, Default)]
pub struct RocketRules(pub Vec<(String, Arc<RocketParams>, f32)>);

impl RocketRules {
    /// The rules of every weapon in `set` with `tf2:rocket`.
    pub fn from_set(set: &WeaponSet) -> Self {
        let mut out = Vec::new();
        for w in &set.weapons {
            let Some(p) = w.hooks.get(super::ROCKET_HOOK) else { continue };
            let Ok(p) = p.clone().try_into::<RocketParams>() else { continue };
            let ramp = w
                .hooks
                .get(super::GUN_HOOK)
                .and_then(|g| chimera_sim::weapon::harm::num(g, "ramp"))
                .unwrap_or(1.0);
            out.push((w.id.clone(), Arc::new(p), ramp));
        }
        Self(out)
    }

    pub fn get(&self, weapon: &str) -> Option<(&RocketParams, f32)> {
        self.0.iter().find(|(id, ..)| id == weapon).map(|(_, p, k)| (&**p, *k))
    }
}

/// Units/s of push per damage point and scale (ours: open question 9).
pub const PUSH_PER_POINT: f32 = 1.0;
/// TF2's standing box (units: 48 x 48 x 82, specs/tf2/classes.md).
pub const STANDING: Vec3 = Vec3::new(48.0, 48.0, 82.0);
/// An upward push above this (units/s) lifts the victim off the ground
/// (Source's non-jump velocity, as HL2:DM's shove).
pub const LIFT: f32 = 140.0;

/// The splash factor at `r` units of a blast of `radius` with `edge` at
/// its edge; None outside.
pub fn splash(r: f32, radius: f32, edge: f32) -> Option<f32> {
    (r <= radius).then(|| 1.0 - (1.0 - edge) * r / radius)
}

/// The push (units/s) of `points` of damage at `scale` on a victim whose
/// box is `size` (units): along `away` (unit).
pub fn push(points: f32, scale: f32, size: Vec3, away: Vec3) -> Vec3 {
    let volume = (size.x * size.y * size.z).max(1.0);
    away * points * scale * PUSH_PER_POINT * (STANDING.x * STANDING.y * STANDING.z) / volume
}

/// A character's box size (units, any axes' order) and centre (m).
fn bounds(at: &Transform, aabb: Option<&ColliderAabb>, state: Option<&MovementState>) -> (Vec3, Vec3) {
    let size = match state {
        Some(s) if s.hull_max != s.hull_min => (s.hull_max - s.hull_min).abs() / U,
        _ => match aabb {
            Some(b) => (b.max - b.min) / U,
            None => STANDING,
        },
    };
    let centre = aabb.map_or(at.translation, |b| (b.min + b.max) / 2.0);
    (size, centre)
}

#[allow(clippy::type_complexity)]
pub fn explode(
    mut hits: MessageReader<FlyerHit>,
    rules: Res<RocketRules>,
    flyers: Query<&Flyer>,
    mut chars: Query<
        (Entity, &Transform, Option<&ColliderAabb>, &Health, Option<&Team>, Option<&MovementState>, &mut Velocity),
        With<Intent>,
    >,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    friendly_fire: Option<Res<FriendlyFire>>,
    mut damage: MessageWriter<Damage>,
    mut inflicted: MessageWriter<Inflicted>,
    mut gone: MessageWriter<FlyerGone>,
    mut sounds: MessageWriter<GameSound>,
    mut commands: Commands,
) {
    let ff = friendly_fire.is_some_and(|f| f.0 != 0);
    for hit in hits.read() {
        let Some((p, ramp)) = rules.get(hit.weapon) else { continue };
        if !flyers.contains(hit.flyer) {
            continue;
        }
        // Just off the surface it touched.
        let centre = hit.point + hit.normal * U;
        let owner = hit.owner;
        let owner_at = owner.and_then(|o| chars.get(o).ok()).map(|(_, t, b, ..)| bounds(t, b, None).1);
        let owner_team = owner.and_then(|o| chars.get(o).ok()).and_then(|c| c.4.copied()).filter(|t| t.0 != 0);
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let mut dealt: Vec<(Entity, f32, Vec3, f32)> = Vec::new();
        for (e, at, aabb, health, team, state, _) in &chars {
            if health.current <= 0.0 {
                continue;
            }
            let own = Some(e) == owner;
            // Teammates aren't hurt with friendly fire off (the owner is).
            if !own && !ff && owner_team.is_some() && team.copied() == owner_team {
                continue;
            }
            let (_, c) = bounds(at, aabb, state);
            let touched = hit.other == Some(e);
            let r = c.distance(centre) / U;
            let Some(f) = (if touched { Some(1.0) } else { splash(r, p.radius, p.edge) }) else { continue };
            // A clear line from the centre to it: only characters between.
            if !touched && let Ok(dir) = Dir3::new(c - centre) {
                let blocked = spatial
                    .cast_ray_predicate(centre, dir, c.distance(centre), true, &filter, &|hit_e| {
                        let body = colliders.get(hit_e).map_or(hit_e, |o| o.body);
                        !chars.contains(body)
                    })
                    .is_some();
                if blocked {
                    continue;
                }
            }
            let airborne = state.is_some_and(|s| !s.on_ground);
            let points = if own {
                p.damage * f * if airborne { p.self_air_scale } else { 1.0 }
            } else {
                let d = owner_at.map_or(0.0, |o| o.distance(c) / U);
                p.damage * modifier(d, ramp) * f
            };
            let scale = match (own, airborne) {
                (false, _) => p.push_other,
                (true, true) => p.push_self_air,
                (true, false) => p.push_self_ground,
            };
            dealt.push((e, points, c, scale));
        }
        for (e, points, c, scale) in dealt {
            let amount = round_points(points * HP);
            if amount <= 0.0 {
                continue;
            }
            let dir = (c - centre).normalize_or(Vec3::Y);
            damage.write(Damage {
                target: e,
                attacker: owner,
                amount,
                point: c,
                dir,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Blast,
                weapon: Some(hit.weapon),
                force: Vec3::ZERO,
            });
            inflicted.write(Inflicted { target: e, amount, from: centre });
            if let Ok((_, at, aabb, _, _, state, mut velocity)) = chars.get_mut(e) {
                // Only the box's volume counts: its axes' order doesn't.
                let (size, _) = bounds(at, aabb, state);
                let dv = push(amount / HP, scale, size, dir) * U;
                velocity.0 += dv;
                if dv.y > LIFT * U {
                    commands.entity(e).queue(|mut w: EntityWorldMut| {
                        if let Some(mut s) = w.get_mut::<MovementState>() {
                            s.on_ground = false;
                        }
                    });
                }
            }
        }
        if let Some(s) = &p.sound {
            sounds.write(GameSound(PlaySound::at(s.clone(), centre)));
        }
        gone.write(FlyerGone {
            flyer: hit.flyer,
            weapon: hit.weapon,
            owner,
            at: centre,
            expired: false,
        });
        commands.entity(hit.flyer).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 5.2: linear to 50 % at 146 units, nothing beyond.
    #[test]
    fn the_splash_falls_to_half_at_the_edge() {
        assert_eq!(splash(0.0, 146.0, 0.5), Some(1.0));
        assert_eq!(splash(73.0, 146.0, 0.5), Some(0.75));
        assert_eq!(splash(146.0, 146.0, 0.5), Some(0.5));
        assert_eq!(splash(147.0, 146.0, 0.5), None);
        // Test 21: a standing Soldier 43 units from his rocket: 76.75.
        assert!((90.0 * splash(43.0, 146.0, 0.5).unwrap() - 76.75).abs() < 0.01);
    }

    #[test]
    fn a_smaller_box_is_pushed_farther() {
        let up = push(50.0, 10.0, STANDING, Vec3::Z);
        assert!((up.z - 500.0).abs() < 1e-3);
        let crouched = push(50.0, 10.0, STANDING * Vec3::new(1.0, 1.0, 62.0 / 82.0), Vec3::Z);
        assert!(crouched.z > up.z);
    }
}
