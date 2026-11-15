//! TF2's explosions (specs/tf2/weapons.md 5.2-5.4, weapons_more.md 4.3,
//! 5.4, 6), shared by the rocket, the grenade and the stickybomb
//! (`BlastWorld::detonate`):
//!
//! - every living character within the radius of the centre, with a
//!   clear line from it to its centre (the world blocks), takes `damage` x
//!   the splash f(r) = 1 - (1 - `edge`) r / radius (linear to 50 % at the
//!   edge) x the distance modifier between the owner's and its centre at
//!   the explosion with the blast's ramp (`Blast::ramp`; None: the
//!   grenade's and an old sticky's flat 1); the player a projectile
//!   touched takes f = 1;
//! - the owner his own blast with no distance modifier, x `self_air_scale`
//!   when airborne (the Soldier's 0.6, the Demoman's 0.75);
//! - each is pushed from the centre by `points x scale` units/s in our
//!   reading (`PUSH_PER_POINT`, open question 9), the scale `push_other`
//!   for others, `push_self_air` for the owner airborne (a rocket or
//!   sticky jump), `push_self_ground` on the ground, over the TF2 standing
//!   box's volume against the victim's (a crouched, smaller hull goes
//!   farther; 5.4); a push upward of more than `LIFT` takes him off the
//!   ground;
//! - its sound there, heard by every client (`map::GameSound`), and an
//!   explosion's look (`Tf2Explosion`, drawn by the client).
//!
//! Each damage rounds to whole points, of kind blast, naming the gun.
//!
//! Known shortcuts: the push per damage point is ours (open question 9);
//! the Heavy's half knockback isn't modelled; the splash measures to the
//! victim's centre (open question 8); crits are off.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};
use serde::Deserialize;

use chimera_sim::weapon::Inflicted;

use super::damage::{HP, U, modifier, round_points};
use crate::{
    core::{Damage, DamageKind, FriendlyFire, Health, Hitgroup, Intent, MovementState, SOLID_LAYERS, Team, Velocity},
    map::{GameSound, PlaySound},
};

/// Units/s of push per damage point and scale (ours: open question 9).
pub const PUSH_PER_POINT: f32 = 1.0;
/// TF2's standing box (units: 48 x 48 x 82, specs/tf2/classes.md).
pub const STANDING: Vec3 = Vec3::new(48.0, 48.0, 82.0);
/// An upward push above this (units/s) lifts the victim off the ground
/// (Source's non-jump velocity, as HL2:DM's shove).
pub const LIFT: f32 = 140.0;

/// A blast's numbers (data units: units, points), as the hooks give them
/// (`tf2:rocket`, `tf2:grenade`; `tf2:sticky` builds one).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BlastParams {
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
    /// The distance modifier applies (the gun's `tf2:gun` ramp); false:
    /// the grenade's flat damage.
    #[serde(default = "yes")]
    pub by_distance: bool,
    /// The explosion's sound entry.
    #[serde(default)]
    pub sound: Option<String>,
    /// A grenade's bounce off the world.
    #[serde(default)]
    pub bounce_sound: Option<String>,
}

fn yes() -> bool {
    true
}

/// One explosion: its numbers, the radius it reaches, its ramp (None: no
/// distance modifier).
#[derive(Clone, Debug)]
pub struct Blast<'a> {
    pub params: &'a BlastParams,
    pub radius: f32,
    pub ramp: Option<f32>,
}

/// An explosion for the clients to draw (its centre), with the weapon it
/// came from.
#[derive(Message, Clone, Copy, Debug)]
pub struct Tf2Explosion {
    pub at: Vec3,
    pub weapon: &'static str,
}

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
pub fn bounds(at: &Transform, aabb: Option<&ColliderAabb>, state: Option<&MovementState>) -> (Vec3, Vec3) {
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

type Chars<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static ColliderAabb>,
        &'static Health,
        Option<&'static Team>,
        Option<&'static MovementState>,
        &'static mut Velocity,
    ),
    With<Intent>,
>;

/// What an explosion reads and writes.
#[derive(SystemParam)]
pub struct BlastWorld<'w, 's> {
    pub chars: Chars<'w, 's>,
    spatial: SpatialQuery<'w, 's>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    friendly_fire: Option<Res<'w, FriendlyFire>>,
    damage: MessageWriter<'w, Damage>,
    inflicted: MessageWriter<'w, Inflicted>,
    sounds: MessageWriter<'w, GameSound>,
    looks: MessageWriter<'w, Tf2Explosion>,
    commands: Commands<'w, 's>,
}

impl BlastWorld<'_, '_> {
    /// A character's centre (m), if it is one.
    pub fn centre(&self, e: Entity) -> Option<Vec3> {
        self.chars.get(e).ok().map(|(_, t, b, ..)| bounds(t, b, None).1)
    }

    /// Whether `a` and `b` are teammates (non-zero teams, friendly fire off).
    pub fn mates(&self, a: Entity, b: Entity) -> bool {
        let ff = self.friendly_fire.as_ref().is_some_and(|f| f.0 != 0);
        let team = |e: Entity| self.chars.get(e).ok().and_then(|c| c.4.copied()).filter(|t| t.0 != 0);
        !ff && a != b && team(a).is_some() && team(a) == team(b)
    }

    /// Explode `blast` of `weapon` by `owner` at `centre` (m), `touched`
    /// the player hit directly (full splash).
    pub fn detonate(&mut self, blast: &Blast, centre: Vec3, owner: Option<Entity>, weapon: &'static str, touched: Option<Entity>) {
        let p = blast.params;
        let owner_at = owner.and_then(|o| self.centre(o));
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let mut dealt: Vec<(Entity, f32, Vec3, f32)> = Vec::new();
        for (e, at, aabb, health, _, state, _) in &self.chars {
            if health.current <= 0.0 {
                continue;
            }
            let own = Some(e) == owner;
            // Teammates aren't hurt with friendly fire off (the owner is).
            if !own && owner.is_some_and(|o| self.mates(o, e)) {
                continue;
            }
            let (_, c) = bounds(at, aabb, state);
            let hit = touched == Some(e);
            let r = c.distance(centre) / U;
            let Some(f) = (if hit { Some(1.0) } else { splash(r, blast.radius, p.edge) }) else { continue };
            // A clear line from the centre to it: only characters between.
            if !hit && let Ok(dir) = Dir3::new(c - centre) {
                let blocked = self
                    .spatial
                    .cast_ray_predicate(centre, dir, c.distance(centre), true, &filter, &|hit_e| {
                        let body = self.colliders.get(hit_e).map_or(hit_e, |o| o.body);
                        !self.chars.contains(body)
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
                let m = match blast.ramp {
                    Some(k) => modifier(owner_at.map_or(0.0, |o| o.distance(c) / U), k),
                    None => 1.0,
                };
                p.damage * m * f
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
            self.damage.write(Damage {
                target: e,
                attacker: owner,
                amount,
                point: c,
                dir,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Blast,
                weapon: Some(weapon),
                force: Vec3::ZERO,
            });
            self.inflicted.write(Inflicted { target: e, amount, from: centre });
            if let Ok((_, at, aabb, _, _, state, mut velocity)) = self.chars.get_mut(e) {
                // Only the box's volume counts: its axes' order doesn't.
                let (size, _) = bounds(at, aabb, state);
                let dv = push(amount / HP, scale, size, dir) * U;
                velocity.0 += dv;
                if dv.y > LIFT * U {
                    self.commands.entity(e).queue(|mut w: EntityWorldMut| {
                        if let Some(mut s) = w.get_mut::<MovementState>() {
                            s.on_ground = false;
                        }
                    });
                }
            }
        }
        if let Some(s) = &p.sound {
            self.sounds.write(GameSound(PlaySound::at(s.clone(), centre)));
        }
        self.looks.write(Tf2Explosion { at: centre, weapon });
    }

    /// A direct hit's damage (a needle's), from `from`.
    pub fn hurt(&mut self, d: Damage, from: Vec3) {
        self.inflicted.write(Inflicted { target: d.target, amount: d.amount, from });
        self.damage.write(d);
    }

    /// A sound at `at`, heard by every client.
    pub fn sound(&mut self, entry: &str, at: Vec3) {
        self.sounds.write(GameSound(PlaySound::at(entry.to_string(), at)));
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
