//! Crossbows (special_weapons.md 7): `Assassin_Bowgun_Nor` and the
//! explosive `Assassin_Bowgun_Expl`.
//!
//! A bolt flies straight at 5000 u/s (250 u a server frame); the
//! alternate fire's (right button: a one-round `combat_arms:alt_fire`
//! volley, the weapon busy for `alt_fire_0`, 550 ms) at 1300 u/s with
//! `InstDamage` 41 (`BoltParams::alt`), known by the weapon's
//! `CaAltVolley` on the tick it fires. A character
//! whose hit spheres the step passes, before the first world surface, takes
//! it: it sticks to him (following him) and deals the direct damage with
//! the roll, the range band, the zone and the armour step; the band
//! measures the shooter's distance from the map's origin, because a
//! projectile shot never reports an impact point (Quirk 4, kept as the
//! reference has it). In the world it stops at the surface and goes 5 s
//! later. The explosive bolt waits stuck for its controller (`super::press`:
//! the blast follows the delay later), or goes off at once when it flies
//! beyond the weapon's range, enters the sky, its owner dies, the
//! character it sticks in dies, or it is shot to 0 HP.

use bevy::prelude::*;

use super::{BoltParams, CaBoltData, CaObject, Cx, Kind, age, cone_dir, devices, impact, spawn_object};
use super::super::{
    grenades::{DESTROYED_AT, character_touches},
    weapons::{CaDamage, base_damage, box_entry, pick_sphere},
};
use crate::{
    weapon::{
        Weapon,
        grenade::{Detonated, GrenadeKind},
        random::shared_random,
    },
};

/// A bolt stuck in the world or a body goes after this, s.
pub const STUCK_TIME: f64 = 5.0;

/// A round of a crossbow: the bolt from the eye along the view (7); the
/// alternate fire's (`alt`) at its own speed and damage.
#[allow(clippy::too_many_arguments)]
pub(super) fn fire(world: &mut World, weapon: Entity, owner: Entity, eye: Vec3, yaw: f32, pitch: f32, seed: u32, alt: bool) {
    let Some(d) = world.get::<CaBoltData>(weapon).cloned() else { return };
    let Some(id) = world.get::<Weapon>(weapon).map(|w| w.id) else { return };
    let params = if alt { std::sync::Arc::new(d.params.alternate()) } else { d.params.clone() };
    let p = &params;
    let dir = cone_dir(p.perturb, p.dynamic_perturb, yaw, pitch, seed, super::BOLT);
    let mut o = CaObject::new(id, Some(owner), d.unit, d.hp, eye / d.unit, dir * p.speed, Kind::Bolt(params.clone()));
    o.s.origin = eye / d.unit;
    spawn_object(world, o);
}

/// One frame of a bolt.
pub(super) fn update(cx: &mut Cx, e: Entity, o: &mut CaObject, p: &BoltParams) {
    let time = cx.f.time;
    let owner_gone = o.owner.is_some_and(|w| !cx.world.entities.contains(w));
    if owner_gone || age(o, cx.f) + 1e-9 >= p.lifetime as f64 {
        cx.remove(e);
        return;
    }
    // Stuck: the 5 s, the character it follows, the explosive's reasons.
    if o.s.landed {
        let bodies = cx.world.bodies();
        let carrier = match o.s.stuck {
            Some((who, offset)) => match bodies.iter().find(|b| b.entity == who) {
                Some(b) => {
                    o.pos = (b.centre + offset) / o.unit;
                    true
                }
                None => {
                    // The character died (or left): the explosive bolt goes off.
                    if p.explosive.is_some() {
                        explode(cx, e, o, p);
                    } else {
                        cx.remove(e);
                    }
                    return;
                }
            },
            None => true,
        };
        let _ = carrier;
        if let Some(x) = &p.explosive {
            if o.destroyed || o.s.boom_at.is_some_and(|t| time + 1e-9 >= t) {
                let _ = x;
                explode(cx, e, o, p);
            }
        } else if time + 1e-9 >= o.s.stuck_at + STUCK_TIME {
            cx.remove(e);
        }
        return;
    }
    if o.destroyed {
        if p.explosive.is_some() {
            explode(cx, e, o, p);
        } else {
            cx.remove(e);
        }
        return;
    }
    fly(cx, e, o, p);
}

fn explode(cx: &mut Cx, e: Entity, o: &mut CaObject, p: &BoltParams) {
    let Some(x) = p.explosive else { return };
    let at = o.at();
    cx.remove(e);
    cx.out.detonated.write(Detonated {
        kind: GrenadeKind::Blast,
        weapon: o.weapon,
        thrower: o.owner,
        at,
        ground: None,
        projectile: e,
        shake: None,
        flags: Default::default(),
    });
    impact::blast(cx, o, at, x.area_damage, x.area_radius, x.area_type, None);
}

fn fly(cx: &mut Cx, e: Entity, o: &mut CaObject, p: &BoltParams) {
    let unit = o.unit;
    let delta = o.velocity * cx.f.dt;
    o.steps += 1;
    let len = delta.length();
    if len <= 0.0 {
        return;
    }
    let (from, to, dir) = (o.pos, o.pos + delta, delta / len);
    let (from_m, to_m) = (from * unit, to * unit);
    o.s.last_dir = dir;
    // Beyond the weapon's range the explosive bolt goes off at once.
    if let Some(x) = &p.explosive
        && to.distance(o.s.origin) > x.range
    {
        o.pos = to;
        explode(cx, e, o, p);
        return;
    }
    let wall = cx.world.cast(from_m, to_m, o.owner);
    let wall_t = wall.map_or(f32::MAX, |(d, ..)| d / (len * unit));
    // The first character whose spheres the step passes before the wall.
    let mut best: Option<(f32, usize)> = None;
    let bodies = cx.world.bodies();
    for (bi, b) in bodies.iter().enumerate() {
        if Some(b.entity) == o.owner {
            continue;
        }
        let Some(tb) = box_entry(b.min, b.max, from_m, dir, len * unit) else { continue };
        let t = tb / (len * unit);
        if t >= wall_t || best.is_some_and(|(bt, _)| bt <= t) {
            continue;
        }
        if character_touches(from_m, to_m, o.velocity, b.min, b.max, &b.spheres) == 0 {
            continue;
        }
        best = Some((t, bi));
    }
    if let Some((t, bi)) = best {
        let b = &bodies[bi];
        let Some(si) = pick_sphere(&b.spheres, from_m, to_m).or_else(|| pick_sphere(&b.spheres, from_m - dir * 100.0 * unit, to_m)) else {
            o.pos = to;
            return;
        };
        let point = from_m + (to_m - from_m) * t;
        // The direct damage: the roll, the band of the shooter's distance
        // from the map's origin (Quirk 4), the zone and the armour.
        let dmg = CaDamage::new(&p.damage, unit, o.hp);
        let mut rng = shared_random(o.seq as u32, "combat_arms:bolt", 0);
        let from_origin = o.s.origin * unit;
        let base = base_damage(&dmg, rng.next(), false, from_origin.length());
        devices::shoot(cx, o, b, si, base, p.damage.damage_type, point);
        o.s.stuck = Some((b.entity, point - b.centre));
        o.s.landed = true;
        o.s.stuck_at = cx.f.time;
        o.pos = point / unit;
        o.velocity = Vec3::ZERO;
        return;
    }
    let Some((distance, _n, collider)) = wall else {
        o.pos = to;
        return;
    };
    if cx.world.surface(collider).as_deref() == Some("sky") {
        if p.explosive.is_some() {
            o.pos = to;
            explode(cx, e, o, p);
        } else {
            cx.remove(e);
        }
        return;
    }
    // Stopped 1 u off the surface (a blast there is not inside the wall).
    o.pos = (from_m + dir * distance) / unit - dir;
    o.velocity = Vec3::ZERO;
    o.s.landed = true;
    o.s.stuck_at = cx.f.time;
    let _ = DESTROYED_AT;
}
