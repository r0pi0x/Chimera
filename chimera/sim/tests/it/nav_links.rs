//! A generated navigation mesh held to where a map is played and linked
//! through its teleporters (`map::nav_gen`: `NavGenParams::within`,
//! `NavWithin`, `link_map`; `map::nav`: `Via::Teleport`): on a toy map
//! whose ledges climb out of a floor onto a high shelf (Halo's cliffs past
//! the drawn level), a mesh held to a box keeps nothing outside it; on two
//! rooms joined only by a one-way teleporter, a path runs from the first to
//! the second through the pad, its length the walk to the pad plus the walk
//! from its exit, and none runs back.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_sim::SimBasePlugins;
use chimera_world::{
    harness::Sim,
    map::{
        nav::{NavMesh, Via},
        nav_gen::{self, NavGenParams, NavWithin},
        teleport::{MapTeleporters, Teleporter},
    },
};

fn block(sim: &mut Sim, min: Vec3, max: Vec3) {
    let (c, s) = ((min + max) * 0.5, max - min);
    sim.app.world_mut().spawn((
        Transform::from_translation(c),
        RigidBody::Static,
        Collider::cuboid(s.x, s.y, s.z),
    ));
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

fn size(m: &NavMesh) -> f32 {
    m.areas.iter().map(|a| (a.max - a.min).x * (a.max - a.min).y).sum()
}

/// A floor (top at 0) from x -10 to 10, z -10 to 10, and from its +x edge
/// ledges 0.6 m apart up to a shelf 3 m high from x 10 to 20.
fn shelf() -> Sim {
    let mut sim = Sim::new(SimBasePlugins, ());
    block(&mut sim, Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
    for i in 0..4 {
        let x = 6.0 + i as f32;
        block(
            &mut sim,
            Vec3::new(x, 0.0, -10.0),
            Vec3::new(x + 1.0, 0.6 * (i + 1) as f32, 10.0),
        );
    }
    block(&mut sim, Vec3::new(10.0, 0.0, -10.0), Vec3::new(20.0, 3.0, 10.0));
    sim.ticks(2);
    sim
}

#[test]
fn a_mesh_held_to_a_box_has_no_cell_outside_it() {
    let mut sim = shelf();
    let w = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let seeds = [Vec3::new(-5.0, 0.0, 0.0)];
    let free = nav_gen::generate(&q, &|_| false, &seeds, &climber());
    assert!(
        free.areas.iter().any(|a| a.center.y > 2.9 && a.min.x >= 10.0),
        "unheld, the climber is on the shelf"
    );
    // Held to the floor's room: x up to 8, 2 m high.
    let (lo, hi) = (Vec3::new(-10.0, -1.0, -10.0), Vec3::new(8.0, 2.0, 10.0));
    let held = nav_gen::generate(
        &q,
        &|_| false,
        &seeds,
        &NavGenParams {
            within: Some((lo, hi)),
            ..climber()
        },
    );
    let cell = climber().cell;
    for a in &held.areas {
        // Each cell's floor (its centre) is in the box: its rectangle at
        // most half a cell past it.
        assert!(
            a.max.x <= hi.x + cell * 0.5 && a.heights.iter().all(|y| *y <= hi.y),
            "{:?} {:?} {:?}",
            a.min,
            a.max,
            a.heights
        );
    }
    assert!(
        held.areas.iter().any(|a| (a.center.y - 1.2).abs() < 0.05),
        "the ledges inside stay"
    );
    let floor = |m: &NavMesh| {
        size(&NavMesh {
            areas: m.areas.iter().filter(|a| a.center.y < 0.3).cloned().collect(),
            ..default()
        })
    };
    assert_eq!(floor(&held), floor(&free), "every floor cell inside kept");
    assert!(
        size(&held) < size(&free) - 150.0,
        "{} against {}",
        size(&held),
        size(&free)
    );
    // A held flood grows to its box's footprint (`cell_limit`): 100 cells
    // unheld, the whole room held.
    let few = NavGenParams {
        max_cells: 100,
        ..climber()
    };
    assert!(size(&nav_gen::generate(&q, &|_| false, &seeds, &few)) <= 100.0 * cell * cell + 0.01);
    let held_few = nav_gen::generate(
        &q,
        &|_| false,
        &seeds,
        &NavGenParams {
            within: Some((lo, hi)),
            ..few
        },
    );
    assert_eq!(floor(&held_few), floor(&free), "held, the whole floor in the box");
    // The loaded map's box (`NavWithin`, a map extra's resource).
    assert_eq!(NavWithin::of(sim.app.world()), None);
    sim.app.world_mut().insert_resource(NavWithin { min: lo, max: hi });
    assert_eq!(NavWithin::of(sim.app.world()), Some((lo, hi)));
}

/// Two rooms 40 m apart (x -20 to -10 and 20 to 30) with nothing between,
/// a teleporter from the first's (-12, 0) to the second's (22, 0).
fn rooms() -> (Sim, Teleporter) {
    let mut sim = Sim::new(SimBasePlugins, ());
    block(&mut sim, Vec3::new(-20.0, -1.0, -5.0), Vec3::new(-10.0, 0.0, 5.0));
    block(&mut sim, Vec3::new(20.0, -1.0, -5.0), Vec3::new(30.0, 0.0, 5.0));
    sim.ticks(2);
    let pad = Teleporter {
        source: Vec3::new(-12.0, 0.0, 0.0),
        source_yaw: 0.0,
        dest: Vec3::new(22.0, 0.0, 0.0),
        dest_yaw: 0.0,
    };
    sim.app.world_mut().insert_resource(MapTeleporters {
        pads: vec![pad],
        radius: 0.5,
        rearm: 1.0,
        keep_velocity: false,
        telefrag: None,
        ..Default::default()
    });
    (sim, pad)
}

#[test]
fn a_path_runs_through_a_teleporter_between_two_rooms() {
    let (mut sim, pad) = rooms();
    let w = sim.app.world_mut();
    let mut mesh = {
        let mut state = SystemState::<SpatialQuery>::new(w);
        let q = state.get(w).unwrap();
        // Seeded in each room (a map's spawns, or the pad's exit).
        nav_gen::generate(&q, &|_| false, &[Vec3::new(-15.0, 0.0, 0.0), pad.dest], &climber())
    };
    let (a, b) = (Vec3::new(-18.0, 0.0, 3.0), Vec3::new(28.0, 0.0, -3.0));
    let (from, to) = (
        mesh.area_at(a + Vec3::Y * 0.1).unwrap(),
        mesh.area_at(b + Vec3::Y * 0.1).unwrap(),
    );
    assert!(mesh.find_path(from, to).is_none(), "the rooms are apart");
    assert_eq!(nav_gen::link_map(&mut mesh, w), 1, "the map's one teleporter");
    assert_eq!(mesh.teleports.len(), 1);
    let (path, cost) = mesh.find_path(from, to).expect("a path through the teleporter");
    let cost = cost as f32;
    assert_eq!(
        path.iter().filter(|(_, v)| matches!(v, Some(Via::Teleport(0)))).count(),
        1,
        "{path:?}"
    );
    // The path's length: the walk to the pad plus the walk from its exit
    // (area centre to centre, within the areas' zigzag), not the 40 m gap.
    let walk = a.distance(pad.source) + pad.dest.distance(b);
    assert!(
        cost > walk * 0.8 && cost < walk * 1.4,
        "cost {cost} against the walks' {walk}"
    );
    // Its points: to the pad's source, on from its destination.
    let points = mesh.route(a, b).unwrap();
    let k = points
        .iter()
        .position(|p| *p == pad.source)
        .expect("the pad's source on the route");
    assert_eq!(points[k + 1], pad.dest);
    let walked: f32 = points
        .windows(2)
        .enumerate()
        .filter(|(i, _)| *i != k)
        .map(|(_, w)| w[0].distance(w[1]))
        .sum::<f32>()
        + a.distance(points[0]);
    assert!((walked - walk).abs() < walk * 0.25, "walked {walked} against {walk}");
    // One way: nothing back.
    assert!(mesh.find_path(to, from).is_none(), "the teleporter is one-way");
    // Areas within reach count the teleporter by its walks.
    assert!(mesh.within(from, walk * 1.5).iter().any(|(i, _)| *i == to));
}
