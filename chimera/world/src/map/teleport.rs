//! A map's teleporters in a form every game's characters use: pads that
//! move a character standing on one to its destination, whatever movement
//! it has (the party apps play one game's maps with every game's
//! characters). A loader publishes them as `MapTeleporters` in its map's
//! typed extras (`MapData::extras`); the map's spawn makes them the
//! world's resource, and the map's tick system after the movement
//! (`map::fall_out_of_map`, the server's) has them act (`busy`, `apply`). Halo's netgame
//! teleporters are the first (games/halo_ce `maps::teleporters`); Source's
//! `trigger_teleport`s stay its map logic's (`source_logic`).
//!
//! The rule (`step`, pure, on `Walker`s):
//! - A character's feet (its movement's `slots::Footing`, else its
//!   capsule's bottom) within `radius` (3D) of a pad's source is moved, the
//!   first such pad in order, unless that pad is the one it last arrived
//!   at (`TeleportArrival`): a pad stays disarmed for a character that
//!   arrived beside it until its feet are farther than `rearm` from it.
//! - Moved: its feet to the destination (the body by the same offset), its
//!   look turned by the destination's yaw minus the source's
//!   (`core::set_view`), its velocity kept in world axes (or zeroed,
//!   `keep_velocity`), and it then counts as arrived at the first pad whose
//!   source is within `rearm` of its new feet (arriving beside a source
//!   doesn't bounce it back). The server's change outside the commands is
//!   counted (`core::rebase`), so a predicting client takes it as a restart.
//! - Blocked: another living character's feet within `radius` of the
//!   destination refuse the move. A blocked character's count
//!   (`TeleportBlocked`) rises by the tick's length per refused try and
//!   falls by it otherwise; at `telefrag` seconds the blockers die (a death
//!   with no killer); none: never.
//!
//! Known shortcuts: no message to the blocked player and no sound or
//! effect here (a game's own code may add them from `TeleportArrival`'s
//! change); props, projectiles and vehicles are not teleported (the only
//! loader's spec moves players alone); bots' navigation has no teleport
//! link (`nav::Via` has walks and ladders only).

use bevy::prelude::*;

use chimera_core::{
    character::CAPSULE_HEIGHT,
    slots::{MovementRegistry, MovementSlot},
};

use crate::core::{Damage, DamageKind, Dead, Hitgroup, Intent, SimClock, Velocity};

/// One pad: where it takes a character and where to (engine space, m;
/// yaws as `Intent::yaw`, radians).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Teleporter {
    pub source: Vec3,
    pub source_yaw: f32,
    pub dest: Vec3,
    pub dest_yaw: f32,
}

/// The map's teleporters and their rules (module doc), a map's typed extra
/// and, while the map plays, a resource.
#[derive(Resource, Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapTeleporters {
    pub pads: Vec<Teleporter>,
    /// A character's feet this close (m, 3D) to a source are taken.
    pub radius: f32,
    /// A pad arrived beside stays disarmed until the feet are farther (m).
    pub rearm: f32,
    /// The velocity is kept in world axes (else zeroed).
    pub keep_velocity: bool,
    /// Seconds of refused tries that kill the blockers (None: never).
    pub telefrag: Option<f32>,
}

/// The pad a character last arrived beside (index into `pads`): disarmed
/// for it until it leaves (`MapTeleporters::rearm`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct TeleportArrival(pub Option<usize>);

/// A character's refused tries (s), toward a telefrag.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct TeleportBlocked(pub f32);

/// One character as the rule sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Walker {
    pub feet: Vec3,
    pub alive: bool,
    pub arrival: Option<usize>,
    pub blocked: f32,
}

/// What `step` decided for a walker.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Moved by pad `pad`: feet to `to`, the look turned by `turn`.
    Moved { pad: usize, to: Vec3, turn: f32 },
    /// Refused at pad `pad` by the walkers `by`.
    Blocked { pad: usize, by: Vec<usize> },
}

/// One tick of the rule (module doc) over `walkers` (updated in place:
/// feet, arrival, blocked counts): each moved or refused walker's outcome,
/// and the walkers to kill (telefrags).
pub fn step(t: &MapTeleporters, walkers: &mut [Walker], dt: f32) -> (Vec<(usize, Outcome)>, Vec<usize>) {
    let mut out = Vec::new();
    let mut kill = Vec::new();
    for i in 0..walkers.len() {
        let w = walkers[i];
        if !w.alive {
            continue;
        }
        // Re-armed once far enough from the pad it arrived at.
        let mut arrival = w.arrival.filter(|&p| t.pads.get(p).is_some_and(|pad| w.feet.distance(pad.source) <= t.rearm));
        let pad = t
            .pads
            .iter()
            .position(|p| w.feet.distance(p.source) <= t.radius)
            .filter(|&p| Some(p) != arrival);
        let mut blocked = (w.blocked - dt).max(0.0);
        if let Some(p) = pad {
            let pad = t.pads[p];
            let by: Vec<usize> = (0..walkers.len())
                .filter(|&j| j != i && walkers[j].alive && walkers[j].feet.distance(pad.dest) <= t.radius)
                .collect();
            if by.is_empty() {
                walkers[i].feet = pad.dest;
                arrival = t.pads.iter().position(|q| pad.dest.distance(q.source) <= t.rearm);
                out.push((i, Outcome::Moved { pad: p, to: pad.dest, turn: pad.dest_yaw - pad.source_yaw }));
            } else {
                blocked = w.blocked + dt;
                if t.telefrag.is_some_and(|limit| blocked >= limit) {
                    for &j in &by {
                        if !kill.contains(&j) {
                            kill.push(j);
                        }
                    }
                    blocked = 0.0;
                }
                out.push((i, Outcome::Blocked { pad: p, by }));
            }
        }
        walkers[i].arrival = arrival;
        walkers[i].blocked = blocked;
    }
    (out, kill)
}

/// A character's feet: its movement's footing, else its capsule's bottom.
fn feet(world: &World, e: Entity) -> Option<Vec3> {
    let slot = world.get::<MovementSlot>(e).map(|s| s.0);
    let footing = slot
        .and_then(|id| world.get_resource::<MovementRegistry>()?.get(id)?.footing)
        .and_then(|f| f(world, e));
    match footing {
        Some(f) => Some(f.feet),
        None => Some(world.get::<Transform>(e)?.translation - Vec3::Y * (CAPSULE_HEIGHT / 2.0)),
    }
}

/// Whether the teleporters have work this tick (characters as their
/// body's place, arrival and refusals): a character near a pad (its body
/// within the radius and a body's height of a source), or one with an
/// arrival or refusals to count down. Else the rule needs no run.
pub fn busy<'a>(t: &MapTeleporters, chars: impl Iterator<Item = (Vec3, Option<&'a TeleportArrival>, Option<&'a TeleportBlocked>)>) -> bool {
    if t.pads.is_empty() {
        return false;
    }
    let reach = t.radius + 2.0 * CAPSULE_HEIGHT;
    let mut chars = chars;
    chars.any(|(at, a, b)| {
        a.is_some_and(|a| a.0.is_some()) || b.is_some_and(|b| b.0 > 0.0) || t.pads.iter().any(|p| at.distance(p.source) <= reach)
    })
}

/// The map's teleporters acting on every character (module doc), on the
/// world: queued by the map's tick system after the movement
/// (`map::fall_out_of_map`, when `busy`) on the server; feet come from each
/// movement's `Footing`, which reads the world.
pub fn apply(world: &mut World) {
    let Some(t) = world.get_resource::<MapTeleporters>().filter(|t| !t.pads.is_empty()).cloned() else {
        return;
    };
    let dt = world.get_resource::<SimClock>().map_or(0.0, |c| c.delta.as_secs_f32());
    let mut q = world.query_filtered::<(Entity, Has<Dead>, Option<&TeleportArrival>, Option<&TeleportBlocked>), With<Intent>>();
    let mut chars: Vec<(Entity, Walker)> = q
        .iter(world)
        .map(|(e, dead, a, b)| {
            (
                e,
                Walker {
                    feet: Vec3::ZERO,
                    alive: !dead,
                    arrival: a.and_then(|a| a.0),
                    blocked: b.map_or(0.0, |b| b.0),
                },
            )
        })
        .collect();
    // A stable order (spawn order), whatever the archetypes.
    chars.sort_by_key(|(e, _)| *e);
    for (e, w) in &mut chars {
        match feet(world, *e) {
            Some(f) => w.feet = f,
            None => w.alive = false,
        }
    }
    let mut walkers: Vec<Walker> = chars.iter().map(|(_, w)| *w).collect();
    let before = walkers.clone();
    let (outcomes, kill) = step(&t, &mut walkers, dt);
    for (i, outcome) in &outcomes {
        let e = chars[*i].0;
        let Outcome::Moved { to, turn, .. } = outcome else { continue };
        let offset = *to - before[*i].feet;
        if let Some(mut tr) = world.get_mut::<Transform>(e) {
            tr.translation += offset;
        }
        if !t.keep_velocity
            && let Some(mut v) = world.get_mut::<Velocity>(e)
        {
            v.0 = Vec3::ZERO;
        }
        if let Some((yaw, pitch)) = world.get::<Intent>(e).map(|i| (i.yaw, i.pitch)) {
            crate::core::set_view(world, e, (yaw + turn).rem_euclid(std::f32::consts::TAU), pitch);
        }
        crate::core::rebase(world, e);
    }
    for (i, (e, w)) in chars.iter().enumerate() {
        let now = walkers[i];
        if now.arrival != w.arrival
            && let Ok(mut ent) = world.get_entity_mut(*e)
        {
            ent.insert(TeleportArrival(now.arrival));
        }
        if now.blocked != w.blocked
            && let Ok(mut ent) = world.get_entity_mut(*e)
        {
            ent.insert(TeleportBlocked(now.blocked));
        }
    }
    for i in kill {
        let (e, w) = chars[i];
        let amount = world.get::<crate::core::Health>(e).map_or(1.0, |h| h.current.max(1.0)) * 1000.0;
        world.write_message(Damage {
            force: Vec3::ZERO,
            target: e,
            attacker: None,
            amount,
            point: w.feet,
            dir: Vec3::NEG_Y,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Generic,
            weapon: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spec's example (specs/halo_ce/scenario.md, "Test cases"), in
    /// its own units on the X axis: pads A (to T0, turning a quarter) and
    /// B (to T1).
    fn example() -> MapTeleporters {
        MapTeleporters {
            pads: vec![
                Teleporter {
                    source: Vec3::ZERO,
                    source_yaw: 0.0,
                    dest: Vec3::new(10.0, 0.0, 0.0),
                    dest_yaw: 1.5707964,
                },
                Teleporter {
                    source: Vec3::new(10.8, 0.0, 0.0),
                    source_yaw: 0.0,
                    dest: Vec3::new(50.0, 0.0, 0.0),
                    dest_yaw: 0.0,
                },
            ],
            radius: 0.5,
            rearm: 1.0,
            keep_velocity: true,
            telefrag: Some(3.0),
        }
    }

    fn at(x: f32) -> Walker {
        Walker {
            feet: Vec3::new(x, 0.0, 0.0),
            alive: true,
            ..Default::default()
        }
    }

    #[test]
    fn the_specs_example_moves_turns_and_does_not_bounce_back() {
        let t = example();
        let mut w = [at(0.6)];
        assert!(step(&t, &mut w, 0.1).0.is_empty(), "0.6 from A: outside the radius");
        let mut w = [at(0.3)];
        let (out, _) = step(&t, &mut w, 0.1);
        assert_eq!(
            out,
            vec![(
                0,
                Outcome::Moved {
                    pad: 0,
                    to: Vec3::new(10.0, 0.0, 0.0),
                    turn: 1.5707964
                }
            )]
        );
        // Arrived 0.8 from B: B is disarmed for it.
        assert_eq!(w[0].arrival, Some(1));
        w[0].feet.x = 10.5;
        assert!(step(&t, &mut w, 0.1).0.is_empty(), "0.3 from B, disarmed");
        w[0].feet.x = 9.5;
        assert!(step(&t, &mut w, 0.1).0.is_empty());
        assert_eq!(w[0].arrival, None, "1.3 from B re-arms it");
        w[0].feet.x = 10.6;
        let (out, _) = step(&t, &mut w, 0.1);
        assert_eq!(
            out,
            vec![(
                0,
                Outcome::Moved {
                    pad: 1,
                    to: Vec3::new(50.0, 0.0, 0.0),
                    turn: 0.0
                }
            )]
        );
    }

    #[test]
    fn a_character_on_the_exit_blocks_and_is_telefragged_in_time() {
        let t = example();
        let mut w = [at(0.2), at(10.1)];
        let mut killed = Vec::new();
        let mut ticks = 0;
        while killed.is_empty() && ticks < 100 {
            let (out, kill) = step(&t, &mut w, 0.25);
            assert_eq!(out, vec![(0, Outcome::Blocked { pad: 0, by: vec![1] })]);
            killed = kill;
            ticks += 1;
        }
        assert_eq!(killed, vec![1]);
        assert_eq!(ticks, 12, "3 s of 0.25 s tries");
        // The dead don't block.
        w[1].alive = false;
        assert!(matches!(step(&t, &mut w, 0.1).0[0].1, Outcome::Moved { pad: 0, .. }));
    }

    #[test]
    fn a_refused_count_falls_when_not_trying() {
        let t = example();
        let mut w = [at(0.2), at(10.1)];
        step(&t, &mut w, 0.5);
        assert_eq!(w[0].blocked, 0.5);
        w[0].feet.x = -5.0;
        step(&t, &mut w, 0.2);
        assert!((w[0].blocked - 0.3).abs() < 1e-6);
    }

    /// The system on a world: a character entering a source arrives with
    /// its body offset kept, its velocity kept and its look turned.
    #[test]
    fn the_system_moves_a_character_with_its_velocity_and_turns_its_look() {
        let mut world = World::new();
        world.insert_resource(example());
        world.init_resource::<Messages<Damage>>();
        let half = Vec3::Y * (CAPSULE_HEIGHT / 2.0);
        let e = world
            .spawn((
                Transform::from_translation(Vec3::new(0.3, 0.0, 0.0) + half),
                Velocity(Vec3::new(1.0, 0.0, 2.0)),
                Intent {
                    yaw: 0.5,
                    ..Default::default()
                },
            ))
            .id();
        apply(&mut world);
        let p = world.get::<Transform>(e).unwrap().translation;
        assert!((p - (Vec3::new(10.0, 0.0, 0.0) + half)).length() < 1e-5, "{p}");
        assert_eq!(world.get::<Velocity>(e).unwrap().0, Vec3::new(1.0, 0.0, 2.0));
        assert!((world.get::<Intent>(e).unwrap().yaw - 2.0707964).abs() < 1e-5);
        assert_eq!(world.get::<TeleportArrival>(e), Some(&TeleportArrival(Some(1))));
        // Standing still beside B: not sent on.
        apply(&mut world);
        let q = world.get::<Transform>(e).unwrap().translation;
        assert_eq!(p, q);
    }
}
