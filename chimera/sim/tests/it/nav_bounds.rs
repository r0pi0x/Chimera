//! A navigation mesh generated for a walker that jumps up ledges
//! (`map::nav_gen`, the Combat Arms bots' 0.8 m rises), bounded to the
//! map's playable space (`nav_gen::generate_within`): on a toy map whose
//! wall a staircase of 0.6 m ledges climbs, onto an outside floor in a
//! cluster of its own (de_aztec's jungle), the bounded mesh keeps the
//! ledges inside and drops the wall's far half and everything beyond.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_sim::SimBasePlugins;
use chimera_world::{
    harness::Sim,
    map::{
        nav::NavMesh,
        nav_gen::{self, Bounds, NavGenParams},
        vis::MapVisibility,
    },
};

/// The wall's middle: inside is x < WALL, outside x > WALL.
const WALL: f32 = 4.5;

fn block(sim: &mut Sim, min: Vec3, max: Vec3) {
    let (c, s) = ((min + max) * 0.5, max - min);
    sim.app
        .world_mut()
        .spawn((Transform::from_translation(c), RigidBody::Static, Collider::cuboid(s.x, s.y, s.z)));
}

/// A floor (top at 0) from x -20 to 20, a 2.5 m wall across it at x 4-5,
/// and ledges against the wall rising 0.6 m each (more than a walking
/// player's step) up to 2.4 m, a rise of 0.1 m to the wall's top; outside,
/// terraces 0.5 m apart down to the floor.
fn toy() -> Sim {
    let mut sim = Sim::new(SimBasePlugins, ());
    block(&mut sim, Vec3::new(-20.0, -1.0, -10.0), Vec3::new(20.0, 0.0, 10.0));
    block(&mut sim, Vec3::new(4.0, 0.0, -10.0), Vec3::new(5.0, 2.5, 10.0));
    for i in 0..4 {
        let z = -8.0 + 2.0 * i as f32;
        block(&mut sim, Vec3::new(2.0, 0.0, z), Vec3::new(4.0, 0.6 * (i + 1) as f32, z + 2.0));
    }
    for i in 0..4 {
        let x = 5.0 + i as f32;
        block(&mut sim, Vec3::new(x, 0.0, -10.0), Vec3::new(x + 1.0, 2.0 - 0.5 * i as f32, 10.0));
    }
    sim.ticks(2);
    sim
}

/// Two clusters, split at the wall's middle, that see each other.
fn vis() -> MapVisibility {
    MapVisibility {
        planes: vec![(Vec3::X, WALL)],
        nodes: vec![(0, [-2, -1])],
        leaf_clusters: vec![0, 1],
        cluster_count: 2,
        visible: vec![vec![3], vec![3]],
        ..default()
    }
}

/// The bots' kind of walker: 0.8 m rises, a 0.38 m radius.
fn climber() -> NavGenParams {
    NavGenParams {
        step: 0.8,
        radius: 0.38,
        max_drop: 3.0,
        ..default()
    }
}

fn size(m: &NavMesh, keep: impl Fn(&chimera_world::map::nav::NavArea) -> bool) -> f32 {
    m.areas.iter().filter(|a| keep(a)).map(|a| (a.max - a.min).x * (a.max - a.min).y).sum()
}

#[test]
fn a_generated_mesh_is_bounded_to_the_rooms_a_player_walks() {
    let mut sim = toy();
    let vis = vis();
    let w = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let seeds = [Vec3::new(-5.0, 0.0, 0.0)];
    let free = nav_gen::generate(&q, &|_| false, &seeds, &climber());
    let bounds = Bounds {
        vis: &vis,
        walker: nav_gen::walking_player(),
    };
    let bound = nav_gen::generate_within(&q, &|_| false, &seeds, &climber(), Some(bounds));
    let outside = |a: &chimera_world::map::nav::NavArea| a.max.x > WALL + 0.01;
    // Unbounded, the climber goes up the ledges, over the wall and down.
    assert!(size(&free, |a| a.min.x >= 9.0 && a.center.y < 0.3) > 100.0, "the outside floor");
    // Bounded: nothing beyond the wall's middle...
    assert!(bound.areas.iter().all(|a| !outside(a)), "{:?}", bound.areas.iter().filter(|a| outside(a)).map(|a| (a.min, a.max)).collect::<Vec<_>>());
    // ...but the jump-up ledges inside stay (a player walks below them,
    // in their cluster), and every cell inside is kept.
    assert!(bound.areas.iter().any(|a| (a.center.y - 2.4).abs() < 0.15), "the top ledge");
    assert!(bound.areas.iter().any(|a| (a.center.y - 1.2).abs() < 0.05), "a lower ledge");
    let floor = |a: &chimera_world::map::nav::NavArea| a.center.y < 0.3 && !outside(a);
    assert!(size(&bound, floor) > 200.0);
    assert_eq!(size(&bound, floor), size(&free, floor));
    // Without visibility data there is nothing to bound by.
    let none = nav_gen::generate_within(&q, &|_| false, &seeds, &climber(), None);
    assert_eq!(none.areas.len(), free.areas.len());
}

/// A flood taken a slice at a time (`NavGenJob`, a front end's) makes the
/// mesh a whole one makes, bounded or not.
#[test]
fn a_sliced_flood_makes_the_same_mesh() {
    let mut sim = toy();
    let vis = vis();
    let w = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let seeds = [Vec3::new(-5.0, 0.0, 0.0)];
    let whole = nav_gen::generate_within(
        &q,
        &|_| false,
        &seeds,
        &climber(),
        Some(Bounds { vis: &vis, walker: nav_gen::walking_player() }),
    );
    let mut job = nav_gen::NavGenJob::new(&seeds, &climber(), Some(nav_gen::walking_player()));
    let mut slices = 0;
    let sliced = loop {
        slices += 1;
        if let Some(m) = job.step(&q, &|_| false, Some(&vis), 7) {
            break m;
        }
    };
    assert!(slices > 10, "{slices} slices");
    assert_eq!(format!("{:?}", whole.areas), format!("{:?}", sliced.areas));
    assert!(!whole.areas.is_empty());
}
