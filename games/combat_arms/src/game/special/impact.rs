//! Impact projectiles (special_weapons.md 1.1-1.4, 2, 3, 4): rockets,
//! launcher shells, flame puffs, the air strike's shells. They fly straight
//! (or under the grenades' object gravity with its quarter term), are a
//! point against the world, and detonate on the first thing they touch
//! (the world: 1 u back along the flight from the surface; a character:
//! where the flight line meets his hit spheres, from 100 u behind the
//! projectile; a prop) or in the air at the end of their lifetime. The
//! blast is `grenades`' (linear falloff, cover traces, players twice, the
//! struck one without a cover test); a lasting sphere is a
//! `CaDamageSphere`.

use bevy::prelude::*;

use super::{CaObject, CaProjectileData, Cx, Kind, ProjectileParams, age, cone_dir, spawn_object};
use super::super::{
    grenades::{self, Area, CaDamageSphere, DESTROYED_AT, OBJECT_GRAVITY, character_touches, flight_step, sphere_entry},
    weapons::{EXPLOSION, box_entry},
};
use crate::{
    weapon::{
        Weapon,
        grenade::{Detonated, GrenadeKind},
    },
};

/// The hit point on a character's spheres is searched from this far
/// behind the projectile, u (constants).
pub const CHAR_LOOKBACK: f32 = 100.0;
/// A projectile hitting world geometry detonates this far back along its
/// flight, u.
pub const BLAST_BACK_OFF: f32 = 1.0;

/// The damage types that reach both the player object and its hit box
/// (the blast applies twice): bullet, burn, explode, claymore, infected
/// explode, EMP.
pub fn applied_twice(damage_type: u32) -> bool {
    matches!(damage_type, 2 | 3 | 7 | 38 | 40 | 46)
}

/// A round of an impact weapon: the projectile from the eye along the view
/// (1.1).
pub(super) fn fire(world: &mut World, weapon: Entity, owner: Entity, eye: Vec3, yaw: f32, pitch: f32, seed: u32) {
    let Some(d) = world.get::<CaProjectileData>(weapon).cloned() else { return };
    let Some(id) = world.get::<Weapon>(weapon).map(|w| w.id) else { return };
    let p = &d.params;
    let dir = cone_dir(p.perturb, p.dynamic_perturb, yaw, pitch, seed, super::PROJECTILE);
    let o = CaObject::new(id, Some(owner), d.unit, d.hp, eye / d.unit, dir * p.speed, Kind::Impact(d.params.clone()));
    spawn_object(world, o);
    crate::game::effects::sounds::write_create_sound(world, &p.create_sound, p.create_radius, false, eye);
}

/// A projectile of `p` for `weapon` at `at` (u) with `velocity` (u/s),
/// made by `owner` (an air strike's shell).
pub fn projectile(weapon: &'static str, owner: Option<Entity>, unit: f32, hp: f32, p: &ProjectileParams, at: Vec3, velocity: Vec3) -> CaObject {
    CaObject::new(weapon, owner, unit, hp, at, velocity, Kind::Impact(std::sync::Arc::new(p.clone())))
}

/// One frame of a projectile: the lifetime test (the air), else its
/// physics.
pub(super) fn update(cx: &mut Cx, e: Entity, o: &mut CaObject, p: &ProjectileParams) {
    let elapsed = age(o, cx.f);
    if o.destroyed || elapsed + 1e-9 >= p.lifetime as f64 {
        let at = o.at();
        detonate(cx, e, o, p, at, None);
        return;
    }
    let unit = o.unit;
    let dt = cx.f.dt;
    let gravity = if p.gravity { -OBJECT_GRAVITY } else { 0.0 };
    let (delta, after) = flight_step(o.velocity, gravity, dt);
    o.velocity = after;
    o.steps += 1;
    let len = delta.length();
    if len <= 0.0 {
        return;
    }
    let (from, to, dir) = (o.pos, o.pos + delta, delta / len);
    let (from_m, to_m) = (from * unit, to * unit);
    let wall = cx.world.cast(from_m, to_m, o.owner);
    let wall_t = wall.map_or(f32::MAX, |(d, ..)| d / (len * unit));
    o.s.last_dir = after.normalize_or_zero();
    // A character the step crosses while the line passes a hit sphere.
    let mut best: Option<(f32, Vec3, Entity)> = None;
    for b in cx.world.bodies() {
        if Some(b.entity) == o.owner {
            continue;
        }
        let Some(tb) = box_entry(b.min, b.max, from_m, dir, len * unit) else { continue };
        let t = tb / (len * unit);
        if t >= wall_t || best.is_some_and(|(bt, ..)| bt <= t) {
            continue;
        }
        if character_touches(from_m, to_m, after, b.min, b.max, &b.spheres) == 0 {
            continue;
        }
        // Where the flight line enters the spheres, searched from 100 u
        // behind the projectile.
        let start = from_m - dir * CHAR_LOOKBACK * unit;
        let end = to_m + dir * CHAR_LOOKBACK * unit;
        let line = end - start;
        let entry = b
            .spheres
            .iter()
            .filter_map(|(c, r)| sphere_entry(start, end, *c, *r))
            .fold(f32::MAX, f32::min);
        let point = if entry < f32::MAX { start + line * entry } else { from_m + dir * tb };
        best = Some((t, point, b.entity));
    }
    // A prop in the way.
    let prop = cx.world.prop_touch(from_m, to_m).filter(|(t, _)| *t < wall_t && best.is_none_or(|(bt, ..)| *t < bt));
    if let Some((t, point, who)) = best {
        o.pos = to;
        detonate(cx, e, o, p, point, Some(who));
        let _ = t;
        return;
    }
    if let Some((t, _)) = prop {
        let point = from_m + (to_m - from_m) * t - o.s.last_dir * BLAST_BACK_OFF * unit;
        o.pos = from + delta * t;
        detonate(cx, e, o, p, point, None);
        return;
    }
    let Some((distance, _n, collider)) = wall else {
        o.pos = to;
        return;
    };
    // The surface point; sky removes it silently.
    if cx.world.surface(collider).as_deref() == Some("sky") {
        cx.remove(e);
        return;
    }
    let q = from_m + dir * distance;
    o.pos = q / unit;
    let at = q - o.s.last_dir * BLAST_BACK_OFF * unit;
    detonate(cx, e, o, p, at, None);
}

/// Detonate at `at` (m): the effect, the blast, the sphere. The projectile
/// is gone. A shooter who left makes an explosion that does nothing (1.5).
pub(super) fn detonate(cx: &mut Cx, e: Entity, o: &CaObject, p: &ProjectileParams, at: Vec3, direct: Option<Entity>) {
    cx.remove(e);
    cx.out.detonated.write(Detonated {
        kind: GrenadeKind::Blast,
        weapon: o.weapon,
        thrower: o.owner,
        at,
        ground: None,
        projectile: e,
        shake: None,
    });
    if o.owner.is_some_and(|w| !cx.world.entities.contains(w)) {
        return;
    }
    if p.area_damage > 0.0 && p.area_radius > 0.0 {
        blast(cx, o, at, p.area_damage, p.area_radius, p.area_type, direct);
    }
    if let Some(ot) = p.over_time
        && ot.lifetime > 0.0
    {
        cx.out.commands.spawn((
            Name::new("Damage sphere"),
            Transform::from_translation(at),
            CaDamageSphere {
                exempt_thrower: p.shooter_exempt,
                centre: at,
                params: ot,
                unit: o.unit,
                hp: o.hp,
                thrower: o.owner,
                weapon: o.weapon,
                created: cx.f.frame,
                until: cx.f.time + ot.lifetime as f64,
            },
        ));
    }
}

/// A blast of `damage` in `radius` (u) of damage type `ty` at `at` (m)
/// from object `o`: players (twice for the types that reach both of their
/// objects), live grenades, breakables and other objects.
pub(super) fn blast(cx: &mut Cx, o: &CaObject, at: Vec3, damage: f32, radius: f32, ty: u32, direct: Option<Entity>) {
    let area = Area {
        damage,
        radius,
        damage_type: if ty == 7 { EXPLOSION } else { ty as i32 },
        twice: applied_twice(ty),
        thrower: o.owner,
        weapon: o.weapon,
        unit: o.unit,
        hp: o.hp,
    };
    grenades::blast_bodies(cx.f, at, &area, direct, &|_, d| area.at(d), cx.world, cx.out);
    blast_grenades(cx, &area, at);
    grenades::blast_things(at, &area, cx.world, cx.out);
}

/// Live grenades in the blast (grenades.md 13): their hit points go down
/// and a spent one goes off on the next frame.
pub(super) fn blast_grenades(cx: &mut Cx, area: &Area, at: Vec3) {
    let r = area.radius;
    let mut hit = Vec::new();
    for g in cx.x.grenades.iter() {
        let pos = g.pos * g.unit;
        let d = pos.distance(at) / area.unit;
        if d <= r && cx.world.clear(at, pos) {
            hit.push((g.seq, area.at(d)));
        }
    }
    for mut g in cx.x.grenades.iter_mut() {
        if let Some((_, amount)) = hit.iter().find(|(s, _)| *s == g.seq) {
            g.health -= amount;
            if g.health <= DESTROYED_AT {
                g.destroyed = true;
            }
        }
    }
}
