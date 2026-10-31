//! Mines and devices (special_weapons.md 6): land mines, the hornet,
//! claymores, remote grenades, caltrops, the nerve confuse mine, the auto
//! turret, first-aid kits, the medkit and air strike designators.
//!
//! The weapon is Chimera's `Throwable` with the device's pre-fire, key and
//! post-fire (6.12; `device_weapon`), whose launch (`launch`, from this
//! module's observer of Chimera's `Launch` on a weapon with
//! `CaDeviceData`) makes the object: thrown at the record's speed from the eye along the view bent by
//! the cone (the meters at 0), or placed (a claymore: the owner's position
//! plus the aim's horizontal part by his depth, dropped to the floor, facing
//! his yaw), or, for the medkit, a heal of its user. A thrown device flies
//! under the grenades' object gravity (`flight_step`), is a point against
//! the world, passes characters, and lands (6.0) on the first surface whose
//! normal's up part is at least 0.75; a wall does nothing of its own (the
//! inward speed goes, it keeps falling). Once landed it re-snaps to the
//! floor below every 0.1 s.
//!
//! Each device's rules are its function below, in the spec's order: the
//! removal tests (owner gone, round over, lifetime, replaced), marks and
//! triggers, then its physics.

use bevy::prelude::*;

use super::{
    CaCameraReverse, CaDeviceData, CaObject, Cx, DeviceKind, DeviceParams, Heal, Kind, StrikeParams, TurretParams, age,
    cone_dir, impact, spawn_object,
};
use super::super::{
    grenades::{self, Area, OBJECT_GRAVITY, OverTime, OverTimeKind, REST_FLOOR_NORMAL, add_over_time, falloff, flight_step},
    movement::StunPenalty,
    weapons::{ArmourHit, CaDamage, Location, armour_and_zone, base_damage, box_entry, pick_sphere},
};
use crate::{
    core::{DamageKind, Intent, MovementState},
    weapon::{
        Weapon,
        data::WeaponData,
        grenade::{
            Blast, Detonated, Flight, GrenadeEffect, GrenadeKind, Launch, ThrowOptions, ThrowRule, Throwable,
        },
    },
};

/// A device sticks only to a surface whose normal's up part is at least this.
pub const FLOOR_NORMAL_Y: f32 = REST_FLOOR_NORMAL;
/// A landed device re-snaps to the floor below this often, s.
pub const RESNAP: f64 = 0.1;
/// Kept off a surface a device touched, u.
const SKIN: f32 = 1e-3;
/// How far below a device the floor is looked for, u.
const FLOOR_PROBE: f32 = 100_000.0;
/// The air strike marker's floor probe: from this height down this far, u.
const STRIKE_PROBE_TOP: f32 = 5000.0;
const STRIKE_PROBE: f32 = 10_000.0;

// ---------------------------------------------------------------------------
// The weapon

/// The `combat_arms:device` hook: the weapon as Chimera's `Throwable` with
/// the device's timings and its own launch (`launch`).
pub(super) fn device_weapon(e: &mut EntityWorldMut, d: &'static WeaponData, p: &DeviceParams) {
    let unit = d.units.length;
    e.insert((
        Throwable {
            count: p.spawned.min(p.max.max(p.spawned)),
            max: p.max,
            pin_time: p.prefire,
            throw_delay: p.key,
            throw_time: p.postfire,
            redraw_time: p.redraw,
            fuse: p.lifetime,
            // Unused: `launch` makes the object.
            throw: ThrowRule {
                pitch_lift: 0.0,
                pitch_scale: 1.0,
                pitch_scale_up: 1.0,
                speed_per_deg: 0.0,
                speed_max: p.speed * unit,
                forward: 0.0,
            },
            flight: Flight {
                half: 0.0,
                gravity: OBJECT_GRAVITY * unit,
                elasticity: 1.0,
                player_elasticity: 1.0,
                elasticity_cap: 1.0,
                stop_speed: 0.0,
                floor_normal: FLOOR_NORMAL_Y,
                breakable_damage: 0.0,
                breakable_slowdown: 1.0,
                water_slowdown: 1.0,
                water_entry: 1.0,
                max_velocity: f32::MAX,
                check_interval: grenades::SERVER_FRAME as f32,
                spin_roll: 0.0,
                spin_pitch: 0.0,
            },
            effect: GrenadeEffect::Blast(Blast {
                damage: 0.0,
                radius: 0.0,
                bell_sigma: None,
                legacy: false,
                armor_ratio: None,
                quantum: 0.0,
                probe_up: 0.0,
                probe_down: 0.0,
                pull_out: 0.0,
                src_lift: 0.0,
                body_target: (0.5, 0.5),
                force: 0.0,
                force_jitter: (1.0, 1.0),
                max_push_speed: 0.0,
                ragdolls: None,
                scorch: None,
                sound: None,
                hearing: None,
                shake: None,
                shield: None,
                through_walls: false,
            }),
            pin_sounds: Vec::new(),
            bounce_sound: None,
            water_sounds: None,
            pin: false,
            throw_at: None,
            redraw_at: None,
        },
        ThrowOptions {
            release_at_pin_end: true,
            hold_loop: (p.throw_loop > 0.0).then_some(p.throw_loop),
            drop_primed: false,
            game_launch: true,
        },
        CaDeviceData {
            params: std::sync::Arc::new(p.clone()),
            unit,
            hp: d.units.damage,
        },
    ));
}

/// A throw leaving the hand of one of our devices (Chimera's `Launch`,
/// the server's): the device (`launch`), in the throw's place among its
/// commands.
pub(super) fn on_launch(l: On<Launch>, ours: Query<(), With<CaDeviceData>>, mut commands: Commands) {
    if ours.contains(l.weapon) {
        let l = *l.event();
        commands.queue(move |w: &mut World| launch(w, l));
    }
}

/// The device leaves the hand (6): made by its kind.
pub fn launch(world: &mut World, l: Launch) {
    let Some(d) = world.get::<CaDeviceData>(l.weapon).cloned() else { return };
    let Some(id) = world.get::<Weapon>(l.weapon).map(|w| w.id) else { return };
    let p = d.params.clone();
    if p.kind == DeviceKind::Medkit {
        if let Some(m) = p.medkit {
            world.resource_mut::<super::CaHeals>().entries.push(Heal {
                target: l.thrower,
                rate: m.rate,
                remaining: m.duration,
                left: m.total,
                hp: d.hp,
            });
        }
        return;
    }
    // One at a time (6.11): the owner's previous one goes on its next update.
    if p.once_only {
        let mut q = world.query::<&mut CaObject>();
        for mut o in q.iter_mut(world) {
            if o.owner == Some(l.thrower) && o.weapon == id {
                o.s.expire = true;
            }
        }
    }
    let dir = cone_dir(p.perturb, p.dynamic_perturb, l.yaw, l.pitch, l.command, super::DEVICE);
    let unit = d.unit;
    // The record's create sound, unless the weapon's call-out stands in.
    crate::game::effects::sounds::write_create_sound(world, &p.create_sound, p.create_radius, p.auto_throw, l.eye);
    let mut o = if p.speed > 0.0 {
        CaObject::new(id, Some(l.thrower), unit, d.hp, l.eye / unit, dir * p.speed, Kind::Device(d.params.clone()))
    } else {
        // Placed: in front of the owner at his depth, facing his yaw (6.3).
        let (centre, depth) = {
            let t = world.get::<Transform>(l.thrower).map_or(l.eye, |t| t.translation);
            match world.get::<MovementState>(l.thrower) {
                Some(s) => (t + (s.hull_min + s.hull_max) / 2.0, (s.hull_max.z - s.hull_min.z) / 2.0),
                None => (t, 0.0),
            }
        };
        let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::NEG_Z);
        let facing = world
            .get::<Intent>(l.thrower)
            .map_or(flat, |i| i.yaw_rotation() * Vec3::NEG_Z);
        let mut o = CaObject::new(
            id,
            Some(l.thrower),
            unit,
            d.hp,
            (centre + flat * depth) / unit,
            Vec3::ZERO,
            Kind::Device(d.params.clone()),
        );
        o.s.facing = Vec3::new(facing.x, 0.0, facing.z).normalize_or(flat);
        o
    };
    o.s.origin = l.eye / unit;
    spawn_object(world, o);
}

// ---------------------------------------------------------------------------
// A frame

/// One frame of a device. Returns nothing: it removes itself through `cx`.
pub(super) fn update(cx: &mut Cx, e: Entity, o: &mut CaObject, p: &DeviceParams) {
    let time = cx.f.time;
    let owner_gone = o.owner.is_some_and(|w| !cx.world.entities.contains(w));
    let aged = age(o, cx.f) + 1e-9 >= p.lifetime as f64;
    if p.kind == DeviceKind::AirStrike {
        if owner_gone || cx.round_over {
            cx.remove(e);
            return;
        }
        if aged && o.s.strike_at.is_none() {
            o.s.strike_at = Some(time);
        }
    } else if owner_gone || cx.round_over || aged || o.s.expire {
        cx.remove(e);
        return;
    }
    // A claymore is put on the floor the frame after it is made.
    if p.speed == 0.0 && !o.s.landed {
        place(cx, o, p);
    }
    if o.s.landed && time + 1e-9 >= o.s.next_resnap && p.kind != DeviceKind::AirStrike {
        resnap(cx, o);
    }
    let removed = match p.kind {
        DeviceKind::LandMine => land_mine(cx, o, p),
        DeviceKind::Hornet => hornet(cx, o, p),
        DeviceKind::Claymore | DeviceKind::RemoteGrenade => detonator_device(cx, o, p),
        DeviceKind::Caltrops => caltrops(cx, e, o, p),
        DeviceKind::NerveMine => nerve_mine(cx, o, p),
        DeviceKind::Turret => match &p.turret {
            Some(t) => turret(cx, o, t),
            None => false,
        },
        DeviceKind::FirstAid => first_aid(cx, o, p),
        DeviceKind::AirStrike => match &p.strike {
            Some(s) => air_strike(cx, o, s),
            None => false,
        },
        DeviceKind::Medkit => false,
    };
    if removed {
        cx.remove(e);
        return;
    }
    if !o.s.landed {
        fly(cx, e, o, p);
    }
}

/// Put a placed device on the floor below its position (6.3).
fn place(cx: &mut Cx, o: &mut CaObject, p: &DeviceParams) {
    let unit = o.unit;
    let from = o.pos * unit;
    let Some((d, _, _)) = cx.world.cast(from, from - Vec3::Y * FLOOR_PROBE * unit, o.owner) else {
        // No floor below: it stays in the air (it falls with the next
        // frame's physics from rest).
        land(cx, o, p, o.pos, Vec3::Y);
        return;
    };
    let at = o.pos - Vec3::Y * (d / unit);
    land(cx, o, p, at, Vec3::Y);
}

/// Land at `at` (u) on a floor with normal `n` (6.0): at rest, its hit
/// points the ammo's, its zone effect on; the kind's own state starts.
fn land(cx: &mut Cx, o: &mut CaObject, p: &DeviceParams, at: Vec3, _n: Vec3) {
    let time = cx.f.time;
    o.pos = at + Vec3::Y * o.half.y;
    o.velocity = Vec3::ZERO;
    o.s.landed = true;
    o.s.landed_at = time;
    o.s.next_resnap = time + RESNAP;
    o.health = p.health;
    let flat = Vec3::new(o.s.last_dir.x, 0.0, o.s.last_dir.z).normalize_or_zero();
    if p.speed > 0.0 && flat != Vec3::ZERO {
        o.s.facing = flat;
    }
    if let Some(t) = &p.turret {
        o.s.ammo = t.ammo;
        o.s.ready_at = time + t.setup as f64;
    }
    if let Some(k) = &p.kit {
        o.s.pool = k.pool;
        o.s.next_tick = time + k.tick as f64;
    }
}

/// A landed device follows the floor below it (6.0).
fn resnap(cx: &mut Cx, o: &mut CaObject) {
    let unit = o.unit;
    o.s.next_resnap = cx.f.time + RESNAP;
    let from = o.pos * unit;
    if let Some((d, n, _)) = cx.world.cast(from, from - Vec3::Y * FLOOR_PROBE * unit, o.owner)
        && n.normalize_or(Vec3::Y).y >= FLOOR_NORMAL_Y
    {
        o.pos.y = from.y / unit - d / unit + o.half.y;
    }
}

/// Thrown flight (6.0): the object gravity with its quarter term, a point
/// against the world, characters passed.
fn fly(cx: &mut Cx, e: Entity, o: &mut CaObject, p: &DeviceParams) {
    let unit = o.unit;
    let straight = p.kind == DeviceKind::AirStrike;
    let gravity = if straight { 0.0 } else { -OBJECT_GRAVITY };
    let (delta, after) = flight_step(o.velocity, gravity, cx.f.dt);
    o.velocity = after;
    o.steps += 1;
    let len = delta.length();
    if len <= 0.0 {
        return;
    }
    let (from, to, dir) = (o.pos, o.pos + delta, delta / len);
    o.s.last_dir = dir;
    let wall = cx.world.cast(from * unit, to * unit, o.owner);
    // The marker's first touch may be a character (6.10).
    if straight && o.s.strike_point.is_none() {
        let wall_t = wall.map_or(f32::MAX, |(d, ..)| d / (len * unit));
        let mut hit: Option<f32> = None;
        for b in cx.world.bodies() {
            if Some(b.entity) == o.owner {
                continue;
            }
            if let Some(t) = box_entry(b.min, b.max, from * unit, dir, len * unit) {
                let t = t / (len * unit);
                if t < wall_t && hit.is_none_or(|h| t < h) {
                    hit = Some(t);
                }
            }
        }
        if let Some(t) = hit {
            let at = from + delta * t;
            mark_target(cx, o, at);
            return;
        }
    }
    let Some((distance, n, collider)) = wall else {
        o.pos = to;
        return;
    };
    if cx.world.surface(collider).as_deref() == Some("sky") {
        cx.remove(e);
        return;
    }
    let n = n.normalize_or(Vec3::Y);
    let at = from + dir * (distance / unit);
    if straight {
        if o.s.strike_point.is_none() {
            mark_target(cx, o, at);
        }
        return;
    }
    if p.kind == DeviceKind::NerveMine {
        o.s.active = true;
    }
    if n.y >= FLOOR_NORMAL_Y {
        land(cx, o, p, at, n);
    } else {
        // A wall or a ceiling does nothing of its own: the speed into the
        // surface goes (the engine's contact response), it falls on.
        o.pos = at + n * SKIN;
        let inward = o.velocity.dot(n).min(0.0);
        o.velocity -= n * inward;
    }
}

// ---------------------------------------------------------------------------
// Blasts

fn detonated(cx: &mut Cx, e: Option<Entity>, o: &CaObject, at: Vec3) {
    cx.out.detonated.write(Detonated {
        kind: GrenadeKind::Blast,
        weapon: o.weapon,
        thrower: o.owner,
        at,
        ground: None,
        projectile: e.unwrap_or(Entity::PLACEHOLDER),
        shake: None,
    });
}

/// The plain blast of a device (mines, hornets, remote grenades).
fn plain_blast(cx: &mut Cx, o: &CaObject, p: &DeviceParams) {
    let at = o.at();
    detonated(cx, None, o, at);
    impact::blast(cx, o, at, p.area_damage, p.area_radius, p.area_type, None);
}

// ---------------------------------------------------------------------------
// Land mines and the hornet (6.1, 6.2)

/// Whether a character of the other side is touching `o`'s box.
fn enemy_on(cx: &Cx, o: &CaObject) -> bool {
    let c = o.at();
    let half = o.half * o.unit;
    cx.world.bodies().iter().any(|b| {
        cx.x.enemy(o.owner, b.entity) && b.min.cmple(c + half).all() && b.max.cmpge(c - half).all()
    })
}

fn land_mine(cx: &mut Cx, o: &mut CaObject, p: &DeviceParams) -> bool {
    if !o.s.landed {
        return false;
    }
    let time = cx.f.time;
    if o.s.triggered_at.is_none() && (o.destroyed || enemy_on(cx, o)) {
        o.s.triggered_at = Some(time);
    }
    if let Some(t) = o.s.triggered_at
        && time + 1e-9 >= t + p.active as f64
    {
        plain_blast(cx, o, p);
        return true;
    }
    false
}

fn hornet(cx: &mut Cx, o: &mut CaObject, p: &DeviceParams) -> bool {
    if !o.s.landed {
        return false;
    }
    let time = cx.f.time;
    let me = o.at();
    if o.s.triggered_at.is_none() {
        if o.destroyed {
            o.s.triggered_at = Some(time);
        } else if time + 1e-9 >= o.s.landed_at + p.ready as f64 {
            let r = p.sensor_radius * o.unit;
            let hit = cx.world.bodies().iter().any(|b| {
                cx.x.enemy(o.owner, b.entity)
                    && b.centre.distance(me) <= r
                    && b.centre.y >= me.y
                    && cx.world.clear(me, b.centre)
            });
            if hit {
                o.s.triggered_at = Some(time);
            }
        }
    }
    if let Some(t) = o.s.triggered_at
        && time + 1e-9 >= t + p.active as f64
    {
        plain_blast(cx, o, p);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Claymores and remote grenades (6.3, 6.4)

fn detonator_device(cx: &mut Cx, o: &mut CaObject, p: &DeviceParams) -> bool {
    if !o.s.landed {
        return false;
    }
    if o.destroyed {
        o.s.marked = true;
    }
    if !o.s.marked {
        return false;
    }
    if p.kind == DeviceKind::RemoteGrenade || p.arc.is_none() {
        plain_blast(cx, o, p);
        return true;
    }
    let arc = p.arc.unwrap();
    let at = o.at();
    detonated(cx, None, o, at);
    let unit = o.unit;
    let cos = arc.angle.to_radians().cos();
    let facing = o.s.facing;
    // The cone's part where a victim is inside it, else the plain part
    // (a cone is three-dimensional: a victim above has a smaller dot).
    let outer = Area {
        damage: arc.damage,
        radius: arc.radius.max(p.area_radius),
        damage_type: p.area_type as i32,
        twice: impact::applied_twice(p.area_type),
        thrower: o.owner,
        weapon: o.weapon,
        unit,
        hp: o.hp,
    };
    let (plain_damage, plain_radius) = (p.area_damage, p.area_radius);
    grenades::blast_bodies(
        cx.f,
        at,
        &outer,
        None,
        &|b, d| {
            let u = (b.centre - at).normalize_or_zero();
            if d <= arc.radius && facing.dot(u) > cos {
                arc.damage * falloff(arc.radius, d)
            } else if d <= plain_radius {
                plain_damage * falloff(plain_radius, d)
            } else {
                0.0
            }
        },
        cx.world,
        cx.out,
    );
    let plain = Area {
        damage: plain_damage,
        radius: plain_radius,
        ..outer
    };
    impact::blast_grenades(cx, &plain, at);
    grenades::blast_things(at, &plain, cx.world, cx.out);
    true
}

// ---------------------------------------------------------------------------
// Caltrops and the nerve mine (6.5, 6.6)

fn caltrops(cx: &mut Cx, e: Entity, o: &mut CaObject, p: &DeviceParams) -> bool {
    let (Some(c), true) = (p.caltrops, o.s.landed) else { return false };
    let time = cx.f.time;
    let unit = o.unit;
    let me = o.at();
    let mut victims = Vec::new();
    for b in cx.world.bodies() {
        if !cx.x.enemy(o.owner, b.entity) {
            continue;
        }
        let rel = b.centre - me;
        let height = rel.y / unit;
        if rel.xz().length() / unit <= c.radius && (0.0..=c.height).contains(&height) {
            victims.push(b.entity);
        }
    }
    for v in victims {
        add_over_time(
            &mut cx.out.effects.over_time,
            OverTime {
                victim: v,
                source: Some(e),
                kind: OverTimeKind::Bleed,
                rate: c.rate * o.hp,
                remaining: c.duration,
                attacker: o.owner,
                weapon: o.weapon,
            },
        );
        cx.out.commands.entity(v).insert(StunPenalty {
            penalty: c.stun_penalty,
            until: cx.f.now + c.stun_time as f64,
        });
        // Counted when first damaged, and again after `recount` s.
        let again = match o.s.counted.iter_mut().find(|(who, _)| *who == v) {
            Some((_, last)) => {
                if ((time - *last) * 1000.0).round() > c.recount as f64 * 1000.0 {
                    *last = time;
                    true
                } else {
                    false
                }
            }
            None => {
                o.s.counted.push((v, time));
                true
            }
        };
        if again {
            o.s.counts += 1;
            if o.s.counts >= c.counts {
                return true;
            }
        }
    }
    false
}

fn nerve_mine(cx: &mut Cx, o: &mut CaObject, p: &DeviceParams) -> bool {
    let (Some(n), true) = (p.nerve, o.s.active) else { return false };
    let me = o.at();
    let r = n.radius * o.unit;
    let until = cx.f.now + n.duration as f64;
    for b in cx.world.bodies() {
        if cx.x.enemy(o.owner, b.entity) && b.centre.distance(me) <= r {
            cx.out.commands.entity(b.entity).insert(CaCameraReverse {
                amount: n.amount,
                until,
            });
        }
    }
    false
}

// ---------------------------------------------------------------------------
// The auto turret (6.7)

/// Whether the world leaves the line from `a` to `b` clear (a hit nearer
/// than `b` blocks it).
fn line_clear(cx: &Cx, a: Vec3, b: Vec3, skip: Option<Entity>) -> bool {
    cx.world
        .cast(a, b, skip)
        .is_none_or(|(d, ..)| d >= a.distance(b) - 1e-4)
}

fn turret(cx: &mut Cx, o: &mut CaObject, t: &TurretParams) -> bool {
    if o.destroyed {
        return true;
    }
    if !o.s.landed {
        return false;
    }
    let time = cx.f.time;
    if time + 1e-9 < o.s.ready_at {
        return false;
    }
    let unit = o.unit;
    let me = o.at();
    let sensor = me + Vec3::from(t.sensor_offset) * unit;
    let bodies = cx.world.bodies();
    // The target (6.7): the nearest candidate with a clear line to one of
    // three points and the centre within the shooting angles; none if the
    // sensor point is itself blocked from the turret.
    if line_clear(cx, me, sensor, o.owner) {
        let mut cands: Vec<&grenades::Body> = bodies
            .iter()
            .filter(|b| cx.x.enemy(o.owner, b.entity) && b.centre.distance(me) <= t.search_radius * unit)
            .collect();
        cands.sort_by(|a, b| a.centre.distance(me).total_cmp(&b.centre.distance(me)));
        o.s.target = None;
        for b in cands {
            let rel = b.centre - sensor;
            let elevation = rel.y.atan2(rel.xz().length()).to_degrees();
            if elevation > t.angles[1] || elevation < t.angles[0] {
                continue;
            }
            let half = (b.max.y - b.min.y) / 2.0;
            let seen = [b.centre, b.centre + Vec3::Y * half, b.centre - Vec3::Y * half]
                .iter()
                .any(|pt| line_clear(cx, sensor, *pt, o.owner));
            if seen {
                o.s.target = Some(b.entity);
                break;
            }
        }
    }
    let Some(target) = o.s.target else { return false };
    let Some(tb) = bodies.iter().find(|b| b.entity == target) else {
        o.s.target = None;
        return false;
    };
    if o.s.ammo == 0 {
        return true;
    }
    if time + 1e-9 < o.s.next_shot {
        return false;
    }
    o.s.next_shot = time + t.fire_delay as f64;
    o.s.ammo -= 1;
    // One instant ray from the gun toward the target's centre, the aim
    // point off by a uniform +-Perturb on each axis (at any range).
    let mut rng = crate::weapon::random::shared_random(o.seq as u32, "combat_arms:turret", o.steps as i32 + o.s.ammo as i32);
    let mut jit = || (rng.next() * 2.0 - 1.0) * t.perturb * unit;
    let aim = tb.centre + Vec3::new(jit(), jit(), jit());
    let dir = (aim - sensor).normalize_or(Vec3::Z);
    let end = sensor + dir * t.range * unit;
    let wall = cx.world.cast(sensor, end, o.owner).map_or(f32::MAX, |(d, ..)| d);
    let mut best: Option<(f32, usize, usize)> = None;
    for (bi, b) in bodies.iter().enumerate() {
        if !cx.x.enemy(o.owner, b.entity) {
            continue;
        }
        let Some(tbx) = box_entry(b.min, b.max, sensor, dir, t.range * unit) else { continue };
        if tbx >= wall || best.is_some_and(|(bt, ..)| bt <= tbx) {
            continue;
        }
        if let Some(si) = pick_sphere(&b.spheres, sensor, end) {
            best = Some((tbx, bi, si));
        }
    }
    if let Some((tbx, bi, si)) = best {
        let b = &bodies[bi];
        let dmg = CaDamage::new(&t.gun, unit, o.hp);
        let point = sensor + dir * tbx;
        let u = rng.next();
        let base = base_damage(&dmg, u, false, tbx);
        shoot(cx, o, b, si, base, t.gun.damage_type, point);
    }
    o.s.ammo == 0
}

/// A shot's damage on `b`'s sphere `si`: the zone and the armour step, the
/// team rules, a kill at 1 HP or less (credited to `o`'s owner).
pub(super) fn shoot(cx: &mut Cx, o: &CaObject, b: &grenades::Body, si: usize, base: f32, damage_type: i32, point: Vec3) {
    let (location, zone) = b.zones.get(si).copied().unwrap_or((Location::Torso, 1.0));
    let lands = !cx.out.spared(b.entity, o.owner);
    let hit = ArmourHit {
        damage_type,
        location,
        zone,
        infected: false,
    };
    let hp = o.hp;
    let points = match cx.out.armour.get_mut(b.entity) {
        Ok((a, pieces)) => armour_and_zone(base / hp, hit, Some(a.into_inner()), pieces, lands),
        Err(_) => armour_and_zone(base / hp, hit, None, None, lands),
    } * hp;
    cx.out
        .deal(cx.f, b.entity, b.health, points, DamageKind::Bullet, o.owner, o.weapon, point, hp);
}

// ---------------------------------------------------------------------------
// The first-aid kit (6.8)

fn first_aid(cx: &mut Cx, o: &mut CaObject, p: &DeviceParams) -> bool {
    let (Some(k), true) = (p.kit, o.s.landed) else { return false };
    if o.destroyed {
        return true;
    }
    let time = cx.f.time;
    if time + 1e-9 < o.s.next_tick {
        return false;
    }
    o.s.next_tick += k.tick as f64;
    let unit = o.unit;
    let me = o.at();
    for b in cx.world.bodies() {
        let mine = Some(b.entity) == o.owner || o.owner.is_some_and(|w| cx.x.same_team(w, b.entity));
        if !mine || b.centre.distance(me) > k.radius * unit || b.centre.y < me.y || b.health >= 1.0 - 1e-6 {
            continue;
        }
        let heal = k.heal.min(o.s.pool);
        if heal <= 0.0 {
            break;
        }
        cx.x.heals.now.push((b.entity, heal * o.hp));
        o.s.pool -= heal;
    }
    o.s.pool <= 1e-6
}

// ---------------------------------------------------------------------------
// Air strikes (6.10)

/// The marker's first touch at `at` (u): the target point is the floor
/// straight below it (from y = 5000 down 10 000 u) plus the marker box's
/// half-height; later touches are ignored. The marker stops.
fn mark_target(cx: &mut Cx, o: &mut CaObject, at: Vec3) {
    let unit = o.unit;
    let top = Vec3::new(at.x, STRIKE_PROBE_TOP, at.z) * unit;
    let floor = cx
        .world
        .cast(top, top - Vec3::Y * STRIKE_PROBE * unit, o.owner)
        .map(|(d, ..)| (top.y - d) / unit);
    let target = match floor {
        Some(y) => Vec3::new(at.x, y + o.half.y, at.z),
        None => at,
    };
    o.s.strike_point = Some(target);
    o.pos = at;
    o.velocity = Vec3::ZERO;
    o.s.landed = true;
}

fn air_strike(cx: &mut Cx, o: &mut CaObject, s: &StrikeParams) -> bool {
    let (Some(start), Some(point)) = (o.s.strike_at, o.s.strike_point) else {
        // The marker's life ran out without a touch: nothing to strike.
        return o.s.strike_at.is_some();
    };
    let time = cx.f.time;
    let course = {
        let d = point - o.s.origin;
        Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::NEG_Z)
    };
    let right = course.cross(Vec3::Y);
    while o.s.dropped < s.points.len() && time + 1e-6 >= start + o.s.dropped as f64 * s.delay as f64 {
        let [x, y] = s.points[o.s.dropped];
        let at = point + right * x + course * y + Vec3::Y * s.height;
        let shell = impact::projectile(
            o.weapon,
            o.owner,
            o.unit,
            o.hp,
            &s.shell,
            at,
            Vec3::new(0.0, -s.shell.speed, 0.0),
        );
        cx.spawn(shell);
        o.s.dropped += 1;
    }
    o.s.dropped >= s.points.len()
}
