//! Physics props across the games' movements (the user's call,
//! 2026-11-24; `chimera_movement::props`): a character follows the prop rule
//! of the map it is on whatever movement moves it. Headless on the party's
//! greybox, made a CS:S map or a DoD:S one (`core::MapGame`), each body on
//! its own game's movement (Combat Arms', Halo's):
//! - on a CS:S map (physics shadows, specs/cs_source/physics_props.md 4.1)
//!   a 20 kg `prop_physics` crate is shoved ahead and a 400 kg one stops
//!   the body;
//! - on a DoD:S map (turbo physics, specs/dods/physics_props.md) the body
//!   has no shadow, the 20 kg crate is a wall and a non-solid multiplayer
//!   prop is walked through;
//! - a room the spawn spots' walker reaches past a crate in its doorway
//!   (`prop_physics::walked_through`, the map's rule) is one the body walks
//!   into, a room it doesn't reach one the body can't.

use avian3d::prelude::{Collider, Mass, Position, RigidBody, Sleeping};
use bevy::prelude::*;
use chimera_core::core::MapGame;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        MapPropCollider, PhysicsProp, PushAway,
        prop_physics::{NoShadow, Shadowed, StartAsleep},
    },
};
use combat_arms::harness::CaSim;
use party_roster::PartyPartsPlugin;

const CA: &str = combat_arms::game::movement::ID;
const HALO: &str = halo_ce::game::movement::ID;
const U: f32 = 0.0254;

/// The party's simulation on the greybox as a map of `game`.
fn sim(game: &str) -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.app.insert_resource(MapGame(game.into()));
    sim.ticks(2);
    sim
}

/// A body of `movement` settled at the greybox's first spawn, looking down
/// -Z.
fn body(sim: &mut Sim, movement: &'static str) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement);
    sim.seconds(1.5);
    p
}

/// The bottom of `p`'s collider (engine).
fn feet(sim: &Sim, p: Entity) -> Vec3 {
    use avian3d::prelude::SimpleCollider;
    let w = sim.app.world();
    let at = sim.position(p);
    let aabb = w.get::<Collider>(p).unwrap().aabb(at, Quat::IDENTITY, 0.0);
    Vec3::new(at.x, aabb.min.y, at.z)
}

/// A crate 24 units across and 36 high, of `push` and `mass`, on the floor
/// `ahead` units in front of `p` (along -Z), as a map's physics prop.
fn prop(sim: &mut Sim, p: Entity, push: PushAway, mass: f32, ahead: f32) -> Entity {
    let f = feet(sim, p);
    let half = Vec3::new(12.0, 18.0, 12.0) * U;
    let at = Vec3::new(f.x, f.y + half.y + 0.01 * U, f.z - ahead * U);
    let e = sim
        .app
        .world_mut()
        .spawn((
            Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z),
            Transform::from_translation(at),
            Mass(mass),
            PhysicsProp {
                push,
                mass,
                bounds: (-half, half),
            },
            MapPropCollider,
            RigidBody::Dynamic,
        ))
        .id();
    sim.seconds(0.5);
    e
}

fn at(sim: &Sim, e: Entity) -> Vec3 {
    let w = sim.app.world();
    w.get::<Position>(e)
        .map_or_else(|| w.get::<Transform>(e).unwrap().translation, |p| p.0)
}

/// `p` walking forward (-Z) for `secs`.
fn walk(sim: &mut Sim, p: Entity, secs: f64) {
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(secs);
    sim.intent(p).move_axis = Vec2::ZERO;
}

/// On a CS:S map both bodies have a physics shadow: a 20 kg crate is
/// shoved ahead of them (they follow it), a 400 kg one stops them.
#[test]
fn on_a_css_map_they_shove_a_light_crate_and_stop_at_a_heavy_one() {
    for movement in [CA, HALO] {
        for mass in [20.0, 400.0] {
            let mut sim = sim("cs_source");
            let p = body(&mut sim, movement);
            let e = prop(&mut sim, p, PushAway::Collide, mass, 80.0);
            // A shadow once there is a prop to push.
            assert!(
                sim.app.world().get::<Shadowed>(p).unwrap().0.is_some(),
                "{movement}: a shadow"
            );
            assert!(sim.app.world().get::<NoShadow>(p).is_none(), "{movement}");
            let (crate_before, start) = (at(&sim, e), sim.position(p));
            walk(&mut sim, p, 3.0);
            let moved = (at(&sim, e) - crate_before).length() / U;
            let went = (start.z - sim.position(p).z) / U;
            println!("{movement} {mass} kg: crate moved {moved} units, body went {went}");
            if mass < 350.0 {
                assert!(moved > 40.0, "{movement}: a {mass} kg crate shoved, moved {moved}");
                assert!(went > 80.0, "{movement}: followed it, went {went}");
            } else {
                assert!(moved < 1.0, "{movement}: a {mass} kg crate stays, moved {moved}");
                assert!(went < 70.0, "{movement}: stopped at it, went {went}");
            }
        }
    }
}

/// On a DoD:S map (turbo physics) neither body has a shadow: the 20 kg
/// crate is a wall that never moves, and a non-solid multiplayer prop is
/// walked through.
#[test]
fn on_a_dods_map_they_follow_turbo_physics() {
    for movement in [CA, HALO] {
        for push in [PushAway::Collide, PushAway::NonSolid] {
            let mut sim = sim(dods::game::GAME);
            let p = body(&mut sim, movement);
            let e = prop(&mut sim, p, push, 20.0, 80.0);
            assert!(
                sim.app.world().get::<NoShadow>(p).is_some(),
                "{movement}: turbo physics, no shadow"
            );
            assert_eq!(sim.app.world().get::<Shadowed>(p).unwrap().0, None, "{movement}");
            let (crate_before, start) = (at(&sim, e), sim.position(p));
            walk(&mut sim, p, 2.0);
            let moved = (at(&sim, e) - crate_before).length() / U;
            let went = (start.z - sim.position(p).z) / U;
            println!("{movement} {push:?}: crate moved {moved} units, body went {went}");
            if push == PushAway::Collide {
                assert!(moved < 1.0, "{movement}: a wall at 20 kg, moved {moved}");
                assert!(went < 70.0, "{movement}: stopped at it, went {went}");
            } else {
                assert!(went > 120.0, "{movement}: walked through it, went {went}");
            }
        }
    }
}

/// The walker and the movements agree (physics_props.md 5): a 10 m room
/// whose one 2 m doorway a 10 kg crate 1.6 m across blocks. On a CS:S map
/// the crate is a light `prop_physics` (shoved by the shadow): the walker
/// reaches the room and each body walks in. On a DoD:S map it is a wall:
/// neither gets in; a multiplayer crate asleep since it spawned (the
/// push-away group) lets both in.
#[test]
fn a_room_the_spots_reach_past_a_crate_is_one_each_body_walks_into() {
    for (game, push, asleep, open) in [
        ("cs_source", PushAway::Collide, false, true),
        (dods::game::GAME, PushAway::Collide, false, false),
        (dods::game::GAME, PushAway::Solid, true, true),
    ] {
        for movement in [CA, HALO] {
            let mut sim = sim(game);
            // x 26 to 36, z 10 to 20; the west wall's doorway z 14 to 16.
            for (size, at) in [
                (Vec3::new(10.4, 3.0, 0.4), Vec3::new(31.0, 1.5, 9.8)),
                (Vec3::new(10.4, 3.0, 0.4), Vec3::new(31.0, 1.5, 20.2)),
                (Vec3::new(0.4, 3.0, 10.4), Vec3::new(36.2, 1.5, 15.0)),
                (Vec3::new(0.4, 3.0, 4.2), Vec3::new(25.8, 1.5, 11.9)),
                (Vec3::new(0.4, 3.0, 4.2), Vec3::new(25.8, 1.5, 18.1)),
            ] {
                sim.app.world_mut().spawn((
                    RigidBody::Static,
                    Collider::cuboid(size.x, size.y, size.z),
                    Transform::from_translation(at),
                ));
            }
            let e = sim
                .app
                .world_mut()
                .spawn((
                    RigidBody::Dynamic,
                    Mass(10.0),
                    Collider::cuboid(0.8, 1.0, 1.6),
                    Transform::from_translation(Vec3::new(25.8, 0.5, 15.0)),
                    PhysicsProp {
                        push,
                        mass: 10.0,
                        bounds: (Vec3::new(-0.4, -0.5, -0.8), Vec3::new(0.4, 0.5, 0.8)),
                    },
                    MapPropCollider,
                ))
                .id();
            if asleep {
                sim.app
                    .world_mut()
                    .entity_mut(e)
                    .insert((StartAsleep, RigidBody::Static));
            }
            sim.ticks(2);
            if !asleep {
                for _ in 0..300 {
                    if sim.app.world().get::<Sleeping>(e).is_some() {
                        break;
                    }
                    sim.ticks(1);
                }
            }
            let inside = |p: Vec3| (26.5..35.5).contains(&p.x) && (10.5..19.5).contains(&p.z);
            let spots = chimera_sim::minigame::spawns::candidates(sim.app.world_mut());
            assert!(spots.len() > 500, "{} spots", spots.len());
            let reached = spots.iter().filter(|p| inside(**p)).count() > 10;
            assert_eq!(reached, open, "{game} {push:?}: the walker");
            // A body west of the doorway walks east (+X: a quarter turn
            // right of -Z) at it for 6 s.
            let p = sim.spawn_character(Vec3::new(22.0, 1.0, 15.0), movement);
            sim.seconds(0.5);
            sim.intent(p).yaw = -std::f32::consts::FRAC_PI_2;
            sim.intent(p).move_axis = Vec2::Y;
            let mut got_in = false;
            for _ in 0..(6.0 * sim.tick_hz()) as usize {
                sim.ticks(1);
                got_in |= inside(sim.position(p));
            }
            assert_eq!(
                got_in,
                open,
                "{game} {push:?} {movement}: the body, at {}",
                sim.position(p)
            );
        }
    }
}
