//! Spawning (specs/hl2dm/rules.md "Spawn-point choice", "The spawn
//! itself"; maps_entities.md 6): the spot of the player's class
//! (`info_player_deathmatch`; in TDM `info_player_combine` or
//! `info_player_rebel` when the map has any, else deathmatch's;
//! `info_player_start` when the class has none), chosen by the ring walk
//! (`first_candidate`, `choose`), the telefrag when every spot is taken,
//! and the spawn: 1 unit above the spot, its angles, still, the suit and
//! the loadout fresh. Every other player takes a spot, dead or a spectator
//! (measured, rules.md P10); the telefrag kills the living whose box
//! reaches within the fallback radius of the first candidate (P9).

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;

use super::{COMBINE, Hl2Match, Hl2Player, Hl2Rules, REBELS, SPAWN_CHECK_RADIUS, SPAWN_RAISE, SPECTATOR, TELEFRAG_DAMAGE, UNASSIGNED};
use crate::{
    core::{Damage, DamageKind, Dead, Hitgroup, MovementState, SimClock, Spectating},
    map::{MapEntities, entities::entity_to_engine},
};

const UNIT: f32 = 0.0254;
/// A standing player's half height (units): its origin (the feet) is this
/// far below its centre when its movement hasn't said.
const HALF_HEIGHT: f32 = 36.0;

/// A spawn spot: its place in the map's entity order among its class,
/// origin and angles (Source units, degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub origin: Vec3,
    pub angles: Vec3,
}

/// The spot classes.
pub const CLASSES: [&str; 3] = ["info_player_deathmatch", "info_player_combine", "info_player_rebel"];

/// The map's spots of `class`, in entity order.
pub fn spots(map: &MapEntities, class: &str) -> Vec<Spot> {
    map.entities
        .iter()
        .filter(|e| e.classname().eq_ignore_ascii_case(class))
        .map(|e| Spot {
            origin: e.origin(),
            angles: e.angles(),
        })
        .collect()
}

/// The first candidate (rules.md step 2): from the last spot (or the empty
/// slot) `n` slots on through a ring of `count` spots and one empty slot,
/// which takes the first spot.
pub fn first_candidate(count: usize, last: Option<usize>, n: usize) -> usize {
    let start = last.filter(|l| *l < count).unwrap_or(count);
    let slot = (start + n) % (count + 1);
    if slot == count { 0 } else { slot }
}

/// From the first candidate on round the ring (the empty slot skipped),
/// the first free spot that isn't at the world origin; None after a full
/// turn.
pub fn choose(spots: &[Spot], first: usize, free: impl Fn(usize) -> bool) -> Option<usize> {
    (0..spots.len())
        .map(|k| (first + k) % spots.len())
        .find(|i| spots[*i].origin != Vec3::ZERO && free(*i))
}

/// A player's box in Source units around its feet: (lo, hi).
fn bounds(t: &Transform, s: Option<&MovementState>) -> (Vec3, Vec3) {
    let (lo, hi) = match s {
        Some(s) if s.hull_max != s.hull_min => (s.hull_min, s.hull_max),
        _ => (Vec3::new(-16.0, -36.0, -16.0) * UNIT, Vec3::new(16.0, 36.0, 16.0) * UNIT),
    };
    let to_source = |v: Vec3| Vec3::new(v.x, -v.z, v.y) / UNIT;
    let (a, b) = (to_source(t.translation + lo), to_source(t.translation + hi));
    (a.min(b), a.max(b))
}

/// Distance (units) from `p` to the box.
fn to_box(p: Vec3, (lo, hi): (Vec3, Vec3)) -> f32 {
    (lo - p).max(p - hi).max(Vec3::ZERO).length()
}

/// The spot class `e` spawns at: index into `CLASSES`, the class's spots;
/// `info_player_start`'s first when the class has none.
fn class_spots(world: &World, e: Entity) -> (usize, Vec<Spot>) {
    let Some(map) = world.get_resource::<MapEntities>() else { return (0, Vec::new()) };
    let teamplay = world.resource::<Hl2Match>().teamplay;
    let team = world.get::<Hl2Player>(e).map_or(UNASSIGNED, |p| p.team);
    let class = match (teamplay, team) {
        (true, COMBINE) => 1,
        (true, REBELS) => 2,
        _ => 0,
    };
    let own = spots(map, CLASSES[class]);
    if !own.is_empty() {
        return (class, own);
    }
    let dm = spots(map, CLASSES[0]);
    if !dm.is_empty() {
        return (0, dm);
    }
    (0, spots(map, "info_player_start").into_iter().take(1).collect())
}

/// Bring `e` into play (the runtime's `Flow::respawn`, the respawn, a
/// join): a spectator stays out; a TDM player on no team joins the
/// smaller; the spot chosen (telefragging if it must), revived there, the
/// suit and the loadout fresh, the timers free.
pub fn spawn(world: &mut World, e: Entity) {
    let Some(p) = world.get::<Hl2Player>(e).cloned() else { return };
    if p.team == SPECTATOR || world.get::<Spectating>(e).is_some() {
        return;
    }
    // The model and team timers free first.
    if let Some(mut p) = world.get_mut::<Hl2Player>(e) {
        p.model_free_at = f64::NEG_INFINITY;
        p.last_jointeam = f64::NEG_INFINITY;
        p.last_spectate = f64::NEG_INFINITY;
    }
    if world.resource::<Hl2Match>().teamplay && p.team == UNASSIGNED {
        let team = super::players::smaller_team(world, e);
        super::players::change_team(world, e, team, false);
    }
    let (class, spots) = class_spots(world, e);
    let at = if spots.is_empty() {
        None
    } else {
        let n = 1 + world.resource_mut::<Hl2Match>().roll(5) as usize;
        let last = world.resource::<Hl2Match>().last_spot[class];
        let first = first_candidate(spots.len(), last, n);
        // Every other player occupies a spot, dead or a spectator
        // (measured, rules.md P10: a bot's body and a spectating bot on a
        // spot each made it taken); the telefrag hurts only the living.
        let others: Vec<(Entity, (Vec3, Vec3))> = world
            .query_filtered::<(Entity, &Transform, Option<&MovementState>), With<Hl2Player>>()
            .iter(world)
            .filter(|(o, ..)| *o != e)
            .map(|(o, t, s)| (o, bounds(t, s)))
            .collect();
        let free = |i: usize| !others.iter().any(|(_, b)| to_box(spots[i].origin, *b) < SPAWN_CHECK_RADIUS);
        let chosen = match choose(&spots, first, free) {
            Some(i) => i,
            None => {
                telefrag(world, e, spots[first].origin, &others);
                first
            }
        };
        let mut m = world.resource_mut::<Hl2Match>();
        m.last_spot[class] = Some(chosen);
        m.last_spot[0] = Some(chosen).filter(|_| class == 0).or(m.last_spot[0]);
        Some(spots[chosen])
    };
    place(world, e, at);
}

/// Every other player whose box reaches within the fallback radius of
/// `origin` takes 300 generic damage from the world (measured, rules.md
/// P9: a bot 62 units from the spot, its box 46 away, died; one 66 away,
/// 50, lived).
fn telefrag(world: &mut World, e: Entity, origin: Vec3, others: &[(Entity, (Vec3, Vec3))]) {
    let radius = world.resource::<Hl2Rules>().frag_fallback_radius;
    for (o, b) in others {
        if to_box(origin, *b) > radius || *o == e || world.get::<Dead>(*o).is_some() || world.get::<Spectating>(*o).is_some() {
            continue;
        }
        world.write_message(Damage {
            target: *o,
            attacker: None,
            amount: TELEFRAG_DAMAGE / 100.0,
            point: entity_to_engine(origin, UNIT),
            dir: Vec3::NEG_Y,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Generic,
            weapon: Some("worldspawn"),
            force: Vec3::ZERO,
        });
    }
}

/// `e` revived at `spot` (1 unit up, its angles), fresh.
fn place(world: &mut World, e: Entity, spot: Option<Spot>) {
    let now = world.resource::<SimClock>().now;
    let half = world
        .get::<MovementState>(e)
        .map(|s| -s.hull_min.y)
        .filter(|h| *h > 0.0)
        .unwrap_or(HALF_HEIGHT * UNIT);
    let at = spot.map(|s| {
        let feet = entity_to_engine(s.origin + Vec3::Z * SPAWN_RAISE, UNIT);
        let yaw = (s.angles.y - 90.0).to_radians();
        let pitch = -s.angles.x.to_radians();
        Transform::from_translation(feet + Vec3::Y * half).with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0))
    });
    // Dead first, so the suit's spawn reset runs as at any respawn.
    world.entity_mut(e).insert(Dead { since: now });
    chimera_sim::spawning::revive(world, e, at);
    world.entity_mut(e).remove::<ColliderDisabled>();
    // The loadout again: no weapons, the starting ones given back.
    let held = world.get::<chimera_sim::weapon::Inventory>(e).map(|i| i.weapons.clone()).unwrap_or_default();
    for w in held {
        if world.get_entity(w).is_ok() {
            world.despawn(w);
        }
    }
    world
        .entity_mut(e)
        .remove::<(chimera_sim::weapon::Inventory, super::super::weapons::inventory::Hl2Armed, super::super::weapons::inventory::Hl2Ammo)>();
    if let Some(mut p) = world.get_mut::<Hl2Player>(e) {
        p.death_tick = None;
        p.respawnable = false;
        p.ready = false;
        p.spawned_at = now;
    }
    super::deaths::spawn_event(world, e);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> Spot {
        Spot {
            origin: Vec3::new(x, 1.0, 0.0),
            angles: Vec3::ZERO,
        }
    }

    /// rules.md tests 24-26 on the walk itself.
    #[test]
    fn the_ring_walk() {
        // 24: A, B, C; last A, N 2: C.
        assert_eq!(first_candidate(3, Some(0), 2), 2);
        // 25: last C, N 1: the empty slot, so A.
        assert_eq!(first_candidate(3, Some(2), 1), 0);
        // No last spot: from the empty slot, N 1 is A, N 3 is C.
        assert_eq!(first_candidate(3, None, 1), 0);
        assert_eq!(first_candidate(3, None, 3), 2);
        // The first spot twice as likely: over every last spot and N.
        let mut hits = [0; 4];
        for last in 0..4 {
            for n in 1..=5 {
                hits[first_candidate(4, Some(last), n)] += 1;
            }
        }
        assert_eq!(hits, [8, 4, 4, 4]);
        // 26: A taken, B free.
        let s = [at(0.0), at(500.0), at(1000.0)];
        assert_eq!(choose(&s, 0, |i| i != 0), Some(1));
        assert_eq!(choose(&s, 2, |i| i != 2), Some(0), "round the ring");
        assert_eq!(choose(&s, 0, |_| false), None);
        let origin = [Spot { origin: Vec3::ZERO, angles: Vec3::ZERO }, at(5.0)];
        assert_eq!(choose(&origin, 0, |_| true), Some(1), "never the world origin");
    }
}
