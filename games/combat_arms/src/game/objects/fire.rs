//! A destructible world object fires its weapon (specs/combat_arms/
//! world_objects.md Follow-up (2026-10-31b) 5, case 51: `CreateExplosion`
//! at its death, `FIRE [target]` while alive): `CaObjectFire` turned into
//! a real shot on the server. The object's `WeaponId` is the install's
//! weapon record index (attribute_data.md: a record's position is its
//! runtime ID), our document through `CaData::weapon_for_index`; the
//! weapon is built from the registry as a player's would be and shot the
//! way it defines, with no character as the shooter (the object is):
//!
//! - a thrown weapon (`grenades::CaGrenadeData`): a live grenade from the
//!   object's position along the direction at the weapon's speed, or,
//!   going off in place, still with a fuse of 0 (the spec's lifetime 0);
//! - an impact projectile weapon (`special::CaProjectileData`): its
//!   projectile the same way (lifetime 0 in place: it detonates on the
//!   next special frame where it is);
//! - a hitscan gun (`weapons::CaDamage` with `Hitscan`): each pellet
//!   traced from the position along the direction to the weapon's range,
//!   the first character's box or `Damageable` node before the world
//!   taking the ray's damage (the roll's middle, the range band at that
//!   distance), the object's own node skipped.
//!
//! Damage is x `DamageFactor` (the blast's and lasting sphere's for
//! grenades and projectiles, each ray's for guns). Credited to no
//! character (`attacker` None), the weapon named.
//!
//! Known shortcuts: a projectile or grenade fired along the object's
//! forward is a point that can strike the object's own node at once (no
//! owner to skip: none of ours is a character); a gun's rays take
//! neither armour nor hit zones (no shooter's test, the server walk's
//! surfaces not passed); devices, bolts and knives fire nothing (logged).

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

use super::{
    super::{
        data::CaData,
        grenades::{self, CaGrenadeData},
        special::{self, CaProjectileData, impact},
        weapons::{CaDamage, base_damage, box_entry},
    },
    CaObjectFire, CaObjects,
};
use crate::{
    core::{Damage, DamageKind, Damageable, Health, Hitgroup, Intent, MovementState},
    weapon::{self, Hitscan},
};

/// What a fire became (tests, logs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shot {
    /// No weapon of that id in this data (no install, or a record we have
    /// no document for).
    NoWeapon,
    Grenade(Entity),
    Projectile(Entity),
    /// Hitscan rays: how many hit something that took damage.
    Rays(u32),
    /// A weapon of a kind an object can't fire.
    Unsupported,
}

/// Fire `f` (the server, from `objects::apply_effects`).
pub fn fire(world: &mut World, f: &CaObjectFire) -> Shot {
    let Some(id) = world
        .get_resource::<CaData>()
        .and_then(|d| d.weapon_for_index(f.weapon))
    else {
        debug!("objects: no weapon {} to fire", f.weapon);
        return Shot::NoWeapon;
    };
    // The weapon is built for a stand-in holder, then dropped with it.
    let holder = world.spawn(Name::new("Object weapon holder")).id();
    let Some(template) = weapon::spawn_weapon(world, id, holder) else {
        world.despawn(holder);
        return Shot::NoWeapon;
    };
    let dir = f.dir.normalize_or(Vec3::NEG_Y);
    let shot = if let Some(d) = world.get::<CaGrenadeData>(template).cloned() {
        let speed = if f.in_place { 0.0 } else { d.params.speed * d.unit };
        match grenades::spawn_grenade(world, template, None, f.at, dir * speed) {
            Some(g) => {
                if let Some(mut c) = world.get_mut::<grenades::CaGrenade>(g) {
                    c.params.area_damage *= f.factor;
                    if let Some(o) = c.params.over_time.as_mut() {
                        o.rate *= f.factor;
                    }
                    if f.in_place {
                        c.params.fuse = 0.0;
                    }
                }
                Shot::Grenade(g)
            }
            None => Shot::Unsupported,
        }
    } else if let Some(d) = world.get::<CaProjectileData>(template).cloned() {
        let mut p = (*d.params).clone();
        p.area_damage *= f.factor;
        if let Some(o) = p.over_time.as_mut() {
            o.rate *= f.factor;
        }
        if f.in_place {
            p.lifetime = 0.0;
        }
        let o = impact::projectile(id, None, d.unit, d.hp, &p, f.at / d.unit, dir * p.speed);
        Shot::Projectile(special::spawn_object(world, o))
    } else if let (Some(d), Some(h)) = (
        world.get::<CaDamage>(template).cloned(),
        world.get::<Hitscan>(template).cloned(),
    ) {
        let skip = world.resource::<CaObjects>().nodes.get(&f.shooter).copied();
        Shot::Rays(rays(world, id, &d, &h, f, dir, skip))
    } else {
        debug!("objects: {id} isn't a weapon an object fires");
        Shot::Unsupported
    };
    world.despawn(template);
    world.despawn(holder);
    shot
}

/// A hitscan weapon's rays from the object: each pellet straight along
/// `dir` (no spread: an object has no meters), the nearest character box
/// or damageable node before the world hit, within the range.
fn rays(
    world: &mut World,
    id: &'static str,
    d: &CaDamage,
    h: &Hitscan,
    f: &CaObjectFire,
    dir: Vec3,
    skip: Option<Entity>,
) -> u32 {
    type Bodies<'w, 's> =
        Query<'w, 's, (Entity, &'static Transform, &'static MovementState, &'static Health), With<Intent>>;
    let mut st: SystemState<(SpatialQuery, Bodies, Query<(), With<Damageable>>)> = SystemState::new(world);
    let Ok((spatial, bodies, damageable)) = st.get(world) else {
        return 0;
    };
    let mut excluded: Vec<Entity> = bodies.iter().map(|(e, ..)| e).collect();
    excluded.extend(skip);
    let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(crate::core::SOLID_LAYERS);
    let Ok(ray) = Dir3::new(dir) else { return 0 };
    let wall = spatial.cast_ray(f.at, ray, h.range, true, &filter);
    let limit = wall.map_or(h.range, |w| w.distance);
    let mut target = wall
        .filter(|w| damageable.contains(w.entity))
        .map(|w| (w.distance, w.entity));
    for (e, t, s, health) in &bodies {
        if health.current <= 0.0 {
            continue;
        }
        let Some(at) = box_entry(t.translation + s.hull_min, t.translation + s.hull_max, f.at, dir, limit) else {
            continue;
        };
        if target.is_none_or(|(best, _)| at < best) {
            target = Some((at, e));
        }
    }
    let Some((distance, who)) = target else { return 0 };
    let amount = base_damage(d, 0.5, false, distance) * f.factor;
    let hit = Damage {
        target: who,
        attacker: None,
        amount,
        point: f.at + dir * distance,
        dir,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: Some(id),
        force: Vec3::ZERO,
    };
    let pellets = h.pellets.max(1);
    for _ in 0..pellets {
        world.write_message(hit.clone());
    }
    pellets
}
