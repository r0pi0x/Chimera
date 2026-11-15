//! Crossbows (special_weapons.md 7): `Assassin_Bowgun_Nor` and the
//! explosive `Assassin_Bowgun_Expl`.
//!
//! A bolt flies straight at 5000 u/s (250 u a server frame); the
//! alternate fire's (right button: a one-round `combat_arms:alt_fire`
//! volley, the weapon busy for `alt_fire_0`, 550 ms) at 1300 u/s with
//! `InstDamage` 41 (`BoltParams::alt`), known by the weapon's
//! `CaAltVolley` on the tick it fires. A character
//! whose hit spheres the step passes, before the first world surface, takes
//! it: it sticks to him on the struck sphere's node (`follow_stuck`, every
//! tick, so it never trails the body) and deals the direct damage with
//! the roll, the range band, the zone and the armour step; the band
//! measures the shooter's distance from the map's origin, because a
//! projectile shot never reports an impact point (Quirk 4, kept as the
//! reference has it). In the world it stops at the surface and goes 5 s
//! later. The explosive bolt waits stuck for its controller (`super::press`:
//! the blast follows the delay later), or goes off at once when it flies
//! beyond the weapon's range, enters the sky, its owner dies, the
//! character it sticks in dies, or it is shot to 0 HP. It is drawn as its
//! projectile record's model (`BoltParams::shown_model`).
//!
//! Known shortcuts: the spec sticks a bolt to the character's socket
//! nearest the hit point; ours sits on the struck hit sphere's node (the
//! sphere's centre, which follows the pose, plus the hit's offset turned
//! with the body's look, not with the node's own turn).

use bevy::prelude::*;

use super::{BoltParams, CaBoltData, CaObject, Cx, Kind, StuckOn, age, cone_dir, devices, facing, impact, spawn_object};
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
            // Where it is on him is `follow_stuck`'s, every tick.
            Some((who, _)) => match bodies.iter().find(|b| b.entity == who) {
                Some(_) => true,
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
        // Stuck on the struck sphere's node, in the body's turn
        // (`follow_stuck` keeps it there every tick).
        let turn = body_turn(b.facing).inverse();
        o.s.stuck = Some((b.entity, turn * (point - b.spheres[si].0)));
        o.s.stuck_on = Some(StuckOn { sphere: si, dir: turn * dir });
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

/// The turn of a body facing `facing` (horizontal, unit: `Intent::
/// yaw_rotation` of its look).
fn body_turn(facing: Vec3) -> Quat {
    Quat::from_rotation_y(f32::atan2(-facing.x, -facing.z))
}

/// Bolts stuck in a character, every tick (not on the 20 Hz frames, and
/// not drawn a frame behind as flying objects are): on the node of the
/// sphere they struck as the character is posed now (`characters::
/// CaPose`, the drawn pose: `weapons::posed_geometry`; else the spec's
/// skeleton, `weapons::body_geometry`; another game's body, its hitbox),
/// at the hit's offset and in the direction it went in, both turned with
/// the body. Its place (the blast's point) and transform are set here
/// together, before the network's `NetItem` and the tick blend sample
/// them, so every client draws it on the body at the same server tick
/// as the body (the 2026-11-14 playtest: a stuck bolt dragged behind a
/// running player). On a machine that draws, it is eased between ticks
/// with the characters (`map::interp::Interpolated`).
#[allow(clippy::type_complexity)]
pub(super) fn follow_stuck(
    mut objects: Query<(Entity, &mut CaObject, &mut Transform, Has<crate::map::interp::Interpolated>)>,
    bodies: Query<
        (
            &Transform,
            &crate::core::Intent,
            &crate::core::MovementState,
            Option<&avian3d::prelude::ColliderAabb>,
            Option<&super::super::weapons::HitSpheres>,
            Option<&super::super::characters::CaPose>,
            (Option<&crate::slots::GameIdentity>, Option<&crate::core::Hitboxes>),
        ),
        Without<CaObject>,
    >,
    mut commands: Commands,
) {
    use super::super::weapons::{MALE, body_box, body_geometry, posed_geometry, their_shapes};
    for (e, mut o, mut t, eased) in &mut objects {
        let (Some((who, offset)), Some(on)) = (o.s.stuck, o.s.stuck_on) else { continue };
        let Ok((bt, intent, state, aabb, spheres, pose, (identity, boxes))) = bodies.get(who) else { continue };
        let data = spheres.map_or(*MALE, |s| s.0);
        let turn = intent.yaw_rotation();
        let anchor = if let Some(shapes) = their_shapes(bt, intent, aabb, identity, boxes) {
            shapes.0.get(on.sphere).map(|h| h.center)
        } else if let (Some(pose), Some((min, _))) = (pose, body_box(bt, state, aabb)) {
            posed_geometry(bt.translation, min, pose.legs_yaw(), pose, data)
                .get(on.sphere)
                .filter(|s| s.1 > 0.0)
                .map(|s| s.0)
        } else {
            None
        };
        let anchor = anchor
            .or_else(|| {
                body_geometry(bt, intent, state, aabb, data)
                    .and_then(|(_, _, s)| s.get(on.sphere).filter(|s| s.1 > 0.0).map(|s| s.0))
            })
            .unwrap_or(bt.translation);
        let at = anchor + turn * offset;
        let pos = at / o.unit;
        if o.pos != pos || o.drawn_from != pos {
            o.pos = pos;
            o.drawn_from = pos;
        }
        let pose_now = Transform::from_translation(at).with_rotation(facing(turn * on.dir));
        if t.translation != pose_now.translation || t.rotation != pose_now.rotation {
            t.translation = pose_now.translation;
            t.rotation = pose_now.rotation;
        }
        if !eased {
            commands.entity(e).insert(crate::map::interp::Interpolated::default());
        }
    }
}
