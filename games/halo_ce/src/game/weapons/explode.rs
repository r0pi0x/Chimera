//! A detonation's area damage and push (specs/halo_ce/projectiles.md
//! behaviours 10 and 11; health_shields.md "Applying a damage event";
//! movement.md 8 "Explosions and other impulses"), for every effect of a
//! projectile's detonation (rockets, grenades, the needles' supercombine,
//! the flame).
//!
//! `area` works out one damage effect's events from an epicenter, in world
//! units: the candidates whose bounding sphere meets the cutoff radius,
//! each seen from the epicenter (a unit with a core radius by four probes
//! around the line, any one clear; anything else by the one line), the
//! scale clamp(1 - (d - r0) / (r1 - r0), 0, 1) by the distance to its
//! centre (1 for an effect not scaled by distance), the amount ((1 - s)
//! lower + s U), the direction from the epicenter. `push` is the push a
//! damage event with an amount above 0 gives: the effect's instantaneous
//! acceleration times the object's acceleration scale over 30, along the
//! direction lifted by 0.45 and normalised (straight up when the
//! direction is zero), half of it for a living biped. The data's numbers
//! (the biped's acceleration scale and bounding radius, the grenades',
//! the bias and the biped's share) are `Explosions` (data/grenades.toml).
//!
//! The owner is not left out (no disc explosion has "does not hurt
//! owner"; the flag is honoured) and an explosion never sets off a
//! grenade: a grenade in flight has no collision model (it is pushed, not
//! damaged), so there are no chain reactions in multiplayer.
//!
//! Known shortcuts: a character's centre is its movement's pill centre
//! (the spec's bounding-sphere centre is the pelvis); the area damage's
//! children (a vehicle's riders) are not walked (no vehicles); a dead
//! body (a ragdoll) takes no push.

use bevy::math::Vec3;
use serde::Deserialize;

use super::super::vitality::data::{DamageEffect, flags};

/// The explosion constants (data/grenades.toml `[explosions]`).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Explosions {
    /// Added to the up part of the push's direction (PUSH_UPWARD_BIAS).
    pub upward_bias: f32,
    /// A living biped's share of the push (BIPED_PUSH_SCALE).
    pub biped_push_scale: f32,
    /// The multiplayer Spartan's acceleration scale and bounding radius
    /// (objects.md: 0.7, 0.42 WU).
    pub biped_acceleration_scale: f32,
    pub biped_bounding_radius: f32,
    /// A thrown grenade's acceleration scale (projectiles.md 10: 2.0).
    pub grenade_acceleration_scale: f32,
}

/// One candidate of an area damage event (WU).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate<T> {
    pub object: T,
    pub centre: Vec3,
    pub radius: f32,
    /// A unit (the core radius's probes apply).
    pub unit: bool,
    /// The owner of the explosion.
    pub owner: bool,
}

/// One damage event of an area damage: the object, the scale, the
/// amount, the direction (unit, or zero at the centre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaHit<T> {
    pub object: T,
    pub scale: f32,
    pub amount: f32,
    pub dir: Vec3,
}

/// perp(a) of behaviour 10: a vector perpendicular to `a` (Halo's
/// component rule, applied to our axes alike).
fn perp(a: Vec3) -> Vec3 {
    let (x, y, z) = (a.x.abs(), a.y.abs(), a.z.abs());
    if x <= y && x <= z {
        Vec3::new(0.0, a.z, -a.y)
    } else if y <= z {
        Vec3::new(-a.z, 0.0, a.x)
    } else {
        Vec3::new(a.y, -a.x, 0.0)
    }
}

/// The events of one damage effect `fx` detonating at `e` (WU):
/// `clear(from, to)` says a line is clear of the level, `draw()` gives
/// each event's uniform draw.
pub fn area<T: Copy>(
    fx: &DamageEffect,
    e: Vec3,
    candidates: &[Candidate<T>],
    clear: &mut dyn FnMut(Vec3, Vec3) -> bool,
    draw: &mut dyn FnMut() -> f32,
) -> Vec<AreaHit<T>> {
    let [r0, r1] = fx.radius;
    let mut out = Vec::new();
    for c in candidates {
        if (c.centre - e).length() - c.radius >= r1 {
            continue;
        }
        if c.owner && fx.has(flags::DOES_NOT_HURT_OWNER) {
            continue;
        }
        let w = c.centre - e;
        let visible = if c.unit && fx.core > 0.0001 && w.length_squared() > 0.0 {
            let p = perp(w).normalize_or_zero();
            let q = w.cross(p).normalize_or_zero();
            [p, -p, q, -q].into_iter().any(|o| {
                let end = e + o * fx.core;
                // From where the probe stops (its end when clear) to the centre.
                let from = if clear(e, end) { end } else { e };
                clear(from, c.centre)
            })
        } else {
            clear(e, c.centre)
        };
        if !visible {
            continue;
        }
        let d = w.length();
        let s = if fx.no_distance_scaling || r1 <= r0 { 1.0 } else { (1.0 - (d - r0) / (r1 - r0)).clamp(0.0, 1.0) };
        if s <= 0.0 {
            continue;
        }
        let dir = if d > 0.0 { w / d } else { Vec3::ZERO };
        out.push(AreaHit { object: c.object, scale: s, amount: fx.amount(s, draw()), dir });
    }
    out
}

/// The push of a damage event (WU per tick, engine axes, +Y up) on an
/// object of acceleration scale `scale`; `biped`: a living biped's share.
pub fn push(x: &Explosions, fx: &DamageEffect, dir: Vec3, scale: f32, biped: bool) -> Option<Vec3> {
    if scale <= 0.0001 || fx.accel <= 0.0 {
        return None;
    }
    let m = fx.accel * scale / 30.0;
    let d = (dir + Vec3::Y * x.upward_bias).normalize_or(Vec3::Y);
    Some(d * m * if biped { x.biped_push_scale } else { 1.0 })
}
