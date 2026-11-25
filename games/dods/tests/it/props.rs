//! DoD:S's players meeting physics props under turbo physics (its movement's
//! `turbo_physics`, specs/dods/physics_props.md's test cases) on the
//! headless harness (`harness::DodsSim`, the greybox): no physics shadow; a
//! `prop_physics` of any mass and a settled multiplayer prop are walls; a
//! multiplayer prop asleep since it spawned (the push-away group) pushes
//! the player back (0.88 v − 33 a tick) and is shoved the less the heavier
//! it is; a non-solid one and debris are walked through; and a room the
//! spawn spots' walker reaches past a prop (`prop_physics::PlayerProps::
//! Turbo`) is one a player walks into, a room it doesn't reach one a
//! player can't. Speeds in units/s, places in metres.

use avian3d::prelude::{Collider, Mass, Position, RigidBody, Sleeping};
use bevy::prelude::*;
use chimera_core::core::{MapGame, Velocity};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        MapPropCollider, PhysicsProp, PushAway,
        prop_physics::{DebrisProp, NoShadow, Shadowed, StartAsleep},
    },
};
use dods::{
    game::{GAME, TICK_INTERVAL, movement},
    harness::DodsSim,
};

const U: f32 = 0.0254;

fn sim() -> Sim {
    let mut sim = Sim::new(DodsSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// A settled player at the greybox's first spawn, looking down -Z.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    p
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    sim.velocity(p).xz().length() / U
}

fn feet(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + Vec3::Y * sim.state(p).hull_min.y
}

/// How a drum is placed.
#[derive(Clone, Copy)]
struct Drum {
    push: PushAway,
    mass: f32,
    /// Spawned asleep (spawnflag 1: the push-away group until woken).
    asleep: bool,
    debris: bool,
}

/// A drum, a box 24 units across and 36 high, standing on the floor
/// `ahead` units in front of `p` (along -Z) as a map's physics prop.
fn drum(sim: &mut Sim, p: Entity, d: Drum, ahead: f32) -> Entity {
    let f = feet(sim, p);
    let half = Vec3::new(12.0, 18.0, 12.0) * U;
    let at = Vec3::new(f.x, f.y + half.y + 0.01 * U, f.z - ahead * U);
    let w = sim.app.world_mut();
    let e = w
        .spawn((
            Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z),
            Transform::from_translation(at),
            Mass(d.mass),
            PhysicsProp { push: d.push, mass: d.mass, bounds: (-half, half) },
            MapPropCollider,
            // A simulated body, as the map makes every physics prop first
            // (`prop_physics::dynamic_body`), held still if asleep.
            RigidBody::Dynamic,
        ))
        .id();
    if d.asleep {
        w.entity_mut(e).insert((StartAsleep, RigidBody::Static));
    }
    if d.debris {
        w.entity_mut(e).insert(DebrisProp);
    }
    sim.ticks(2);
    e
}

fn drum_at(sim: &Sim, e: Entity) -> Vec3 {
    sim.app.world().get::<Position>(e).map_or_else(|| sim.app.world().get::<Transform>(e).unwrap().translation, |p| p.0)
}

/// Running forward at 220 for `ticks`, each tick's speed.
fn run(sim: &mut Sim, p: Entity, ticks: usize) -> Vec<f32> {
    sim.intent(p).move_axis = Vec2::Y;
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::NEG_Z * 220.0 * U;
    (0..ticks)
        .map(|_| {
            sim.ticks(1);
            speed(sim, p)
        })
        .collect()
}

/// Section 1: a `prop_physics` of 1, 50 or 400 kg stops a player running at
/// 220 dead on contact and never moves; the player has no shadow.
#[test]
fn a_prop_physics_is_a_wall_at_any_mass() {
    for mass in [1.0, 50.0, 400.0] {
        let mut sim = sim();
        let p = player(&mut sim);
        assert!(sim.app.world().get::<NoShadow>(p).is_some_and(|n| n.0), "turbo physics: no shadow");
        assert_eq!(sim.app.world().get::<Shadowed>(p).unwrap().0, None);
        let e = drum(&mut sim, p, Drum { push: PushAway::Collide, mass, asleep: false, debris: false }, 120.0);
        sim.seconds(0.5);
        let before = drum_at(&sim, e);
        let v = run(&mut sim, p, 150);
        let moved = (drum_at(&sim, e) - before).length() / U;
        assert!(v[149] < 0.5, "{mass} kg: still moving at {}", v[149]);
        assert!(moved < 0.5, "{mass} kg: the drum moved {moved} units");
        let gap = (feet(&sim, p).z - drum_at(&sim, e).z) / U;
        assert!((27.0..30.0).contains(&gap), "{mass} kg: stopped {gap} from its axis (16 + 12)");
        // Stopped at once: from 220 to 0 in a tick at the contact.
        let hit = v.iter().position(|s| *s < 200.0).unwrap();
        assert!(v[hit] < 1.0, "{mass} kg: {} on the contact tick", v[hit]);
    }
}

/// Section 1: a multiplayer prop in "solid" mode spawned awake settles into
/// the normal group: once asleep it is a wall like any other.
#[test]
fn a_settled_multiplayer_prop_is_a_wall() {
    let mut sim = sim();
    let p = player(&mut sim);
    let e = drum(&mut sim, p, Drum { push: PushAway::Solid, mass: 50.0, asleep: false, debris: false }, 120.0);
    for _ in 0..300 {
        if sim.app.world().get::<Sleeping>(e).is_some() {
            break;
        }
        sim.ticks(1);
    }
    assert!(sim.app.world().get::<Sleeping>(e).is_some(), "the drum settled");
    let before = drum_at(&sim, e);
    let v = run(&mut sim, p, 120);
    assert!(v[119] < 0.5, "still moving at {}", v[119]);
    assert!((drum_at(&sim, e) - before).length() / U < 0.5, "the drum moved");
    let gap = (feet(&sim, p).z - drum_at(&sim, e).z) / U;
    assert!((27.0..30.0).contains(&gap), "stopped {gap} from its axis");
}

/// Section 3: a 1000 kg multiplayer drum asleep since it spawned is no wall:
/// the player runs into it and is pushed back, the speed 160.6, 108.3,
/// 62.3, 21.8 on the first four ticks of the contact (v ← 0.88 v − 33).
#[test]
fn an_asleep_multiplayer_prop_pushes_the_player_back() {
    let mut sim = sim();
    let p = player(&mut sim);
    drum(&mut sim, p, Drum { push: PushAway::Solid, mass: 1000.0, asleep: true, debris: false }, 120.0);
    let v = run(&mut sim, p, 60);
    let hit = v.iter().position(|s| *s < 219.0).expect("the push-back");
    for (k, want) in [160.6, 108.3, 62.3, 21.8].into_iter().enumerate() {
        assert!((v[hit + k] - want).abs() < 0.2, "tick {} of the contact: {} against {want} ({:?})", k + 1, v[hit + k], &v[hit..hit + 4]);
    }
}

/// Section 3: shoved the less the heavier: over the 120 ticks after the
/// contact a 10 kg drum is kicked well ahead (320 units measured), a
/// 1000 kg one hardly moves (0.3).
#[test]
fn an_asleep_multiplayer_prop_is_shoved_the_less_the_heavier_it_is() {
    let moved = |mass: f32| {
        let mut sim = sim();
        let p = player(&mut sim);
        let e = drum(&mut sim, p, Drum { push: PushAway::Solid, mass, asleep: true, debris: false }, 120.0);
        let before = drum_at(&sim, e);
        let v = run(&mut sim, p, 400);
        assert!(v.iter().any(|s| *s < 219.0), "{mass} kg: met");
        let moved = (drum_at(&sim, e) - before).xz().length() / U;
        eprintln!("{mass} kg: shoved {moved} units");
        moved
    };
    let (light, heavy) = (moved(10.0), moved(1000.0));
    assert!(light > 60.0, "10 kg: kicked {light} units");
    assert!(heavy < 2.0, "1000 kg: moved {heavy} units");
}

/// Section 4: a non-solid (mode 2) multiplayer drum is walked straight
/// through at 220; so is a `prop_physics` placed as debris.
#[test]
fn a_non_solid_prop_and_debris_are_walked_through() {
    for (what, d) in [
        ("non-solid", Drum { push: PushAway::NonSolid, mass: 1000.0, asleep: true, debris: false }),
        ("debris", Drum { push: PushAway::Collide, mass: 50.0, asleep: false, debris: true }),
    ] {
        let mut sim = sim();
        let p = player(&mut sim);
        let e = drum(&mut sim, p, d, 120.0);
        sim.seconds(0.5);
        let v = run(&mut sim, p, 60);
        assert!(v.iter().all(|s| (s - 220.0).abs() < 0.5), "{what}: {v:?}");
        assert!(feet(&sim, p).z < drum_at(&sim, e).z - 30.0 * U, "{what}: past it");
    }
}

/// The spawn spots' walker and the movement agree (physics_props.md 5):
/// a 10 m room whose one 2 m doorway a 10 kg crate 1.6 m across blocks
/// (the room as chimera_sim's `spawns` tests build it). Asleep since it
/// spawned, the walker reaches the room past it, and a DoD:S player
/// walking at the doorway shoves it in and follows; settled, neither gets
/// in. (A push-away crate that can't be shoved, wedged or heavy, holds a
/// player back as it does in the game, while the walker passes it: the
/// spec's rule for walkers.)
#[test]
fn a_room_the_spots_reach_past_a_prop_is_one_a_player_walks_into() {
    for asleep in [true, false] {
        let mut sim = sim();
        sim.app.insert_resource(MapGame(GAME.into()));
        // x 26 to 36, z 10 to 20; the west wall's doorway z 14 to 16.
        for (size, at) in [
            (Vec3::new(10.4, 3.0, 0.4), Vec3::new(31.0, 1.5, 9.8)),
            (Vec3::new(10.4, 3.0, 0.4), Vec3::new(31.0, 1.5, 20.2)),
            (Vec3::new(0.4, 3.0, 10.4), Vec3::new(36.2, 1.5, 15.0)),
            (Vec3::new(0.4, 3.0, 4.2), Vec3::new(25.8, 1.5, 11.9)),
            (Vec3::new(0.4, 3.0, 4.2), Vec3::new(25.8, 1.5, 18.1)),
        ] {
            sim.app.world_mut().spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(at)));
        }
        let w = sim.app.world_mut();
        let e = w
            .spawn((
                RigidBody::Dynamic,
                Mass(10.0),
                Collider::cuboid(0.8, 1.0, 1.6),
                Transform::from_translation(Vec3::new(25.8, 0.5, 15.0)),
                PhysicsProp { push: PushAway::Solid, mass: 10.0, bounds: (Vec3::new(-0.4, -0.5, -0.8), Vec3::new(0.4, 0.5, 0.8)) },
                MapPropCollider,
            ))
            .id();
        // Asleep since it spawned (held still), or settled (asleep in the
        // normal group).
        if asleep {
            w.entity_mut(e).insert((StartAsleep, RigidBody::Static));
        }
        sim.ticks(2);
        if !asleep {
            for _ in 0..300 {
                if sim.app.world().get::<Sleeping>(e).is_some() {
                    break;
                }
                sim.ticks(1);
            }
            assert!(sim.app.world().get::<Sleeping>(e).is_some(), "the crate settled");
        }
        let inside = |p: Vec3| (26.5..35.5).contains(&p.x) && (10.5..19.5).contains(&p.z);
        let spots = chimera_sim::minigame::spawns::candidates(sim.app.world_mut());
        assert!(spots.len() > 500, "{} spots", spots.len());
        let reached = spots.iter().filter(|p| inside(**p)).count() > 10;
        assert_eq!(reached, asleep, "the walker, asleep {asleep}");
        // A player west of the doorway walks east (+X: a quarter turn
        // right of -Z) at it for 6 s.
        let p = sim.spawn_character(Vec3::new(22.0, 1.0, 15.0), movement::ID);
        sim.seconds(0.5);
        sim.intent(p).yaw = -std::f32::consts::FRAC_PI_2;
        sim.intent(p).move_axis = Vec2::Y;
        let mut got_in = false;
        for _ in 0..(6.0 / TICK_INTERVAL) as usize {
            sim.ticks(1);
            got_in |= inside(sim.position(p));
        }
        assert_eq!(got_in, asleep, "the player, asleep {asleep}: at {}", sim.position(p));
    }
}
