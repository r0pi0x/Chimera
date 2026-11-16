//! The range (`chimera_world::range`, the sandbox's map) on the engine's
//! simulation alone (`SimBasePlugins`): it loads with its layout
//! (`RangeLayout`), players spawn at the firing line, the floor is solid
//! down the range's axis out to 200 m (ray casts), a walking character
//! stands and walks down range on it, a generated navigation mesh takes a
//! bot from the firing line to 40 m (and up the platform), spawning on
//! spots still spawns at the firing line, and unloading takes all of it
//! out.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{character::CAPSULE_HEIGHT, core::SpawnPoint};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::{
    greybox,
    harness::Sim,
    map::nav_gen,
    range::{RangeLayout, RangeMapPlugin, RangePart},
};

fn range() -> Sim {
    let mut sim = Sim::new(SimBasePlugins, RangeMapPlugin);
    sim.ticks(2);
    sim
}

fn layout(sim: &Sim) -> RangeLayout {
    sim.app.world().get_resource::<RangeLayout>().cloned().expect("the range's layout is in")
}

#[test]
fn the_range_loads_with_its_layout() {
    let sim = range();
    let l = layout(&sim);
    assert_eq!(l.distances, [5.0, 10.0, 20.0, 40.0, 100.0, 200.0]);
    assert!(l.length > 200.0, "the backstop is past 200 m");
    assert_eq!(l.dir, Vec3::NEG_Z);
    assert_eq!(*sim.app.world().resource::<greybox::CodeMap>(), greybox::CodeMap("range"));
}

#[test]
fn players_spawn_at_the_firing_line() {
    let mut sim = range();
    let l = layout(&sim);
    let w = sim.app.world_mut();
    let spawns: Vec<Vec3> = w
        .query_filtered::<&Transform, With<SpawnPoint>>()
        .iter(w)
        .map(|t| t.translation)
        .collect();
    assert!(spawns.len() >= 2, "{spawns:?}");
    for s in &spawns {
        let behind = (*s - l.firing_line).dot(-l.dir);
        assert!((0.0..3.0).contains(&behind), "a spawn {behind} m behind the firing line: {s}");
    }
}

#[test]
fn the_floor_is_solid_from_5_to_200_m() {
    let mut sim = range();
    let l = layout(&sim);
    let w = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let filter = SpatialQueryFilter::default();
    let mut d = 5.0;
    while d <= 200.0 {
        for side in [-8.0, 0.0, 8.0] {
            let p = l.point(d, side) + Vec3::Y * 2.0;
            let hit = q
                .cast_ray(p, Dir3::NEG_Y, 10.0, true, &filter)
                .unwrap_or_else(|| panic!("no floor at {d} m, side {side}"));
            assert!((hit.distance - 2.0).abs() < 0.01, "the floor at {d} m, side {side} is at {}", 2.0 - hit.distance);
            assert!(hit.normal.y > 0.99, "flat at {d} m");
        }
        d += 2.5;
    }
}

#[test]
fn a_character_stands_and_walks_down_range() {
    let mut sim = range();
    let l = layout(&sim);
    let p = sim.spawn_character(l.spawns[0], placeholder::ID);
    sim.seconds(0.5);
    assert!(sim.state(p).on_ground);
    let y = sim.position(p).y;
    assert!((y - CAPSULE_HEIGHT / 2.0).abs() < 0.05, "standing at {y}");
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(3.0);
    let along = (sim.position(p) - l.firing_line).dot(l.dir);
    assert!(along > 10.0, "walked {along} m down range");
    assert!(sim.state(p).on_ground);
}

#[test]
fn bots_find_a_path_to_40_m_and_up_the_platform() {
    let mut sim = range();
    let l = layout(&sim);
    let w = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let mesh = nav_gen::generate(&q, &|_| false, &l.spawns, &nav_gen::walking_player());
    let goal = l.at_distance(40.0);
    let area = mesh.nearest_area(goal).expect("an area near 40 m");
    let a = &mesh.areas[area];
    assert!(
        a.min.x <= goal.x && goal.x <= a.max.x && a.min.y <= goal.z && goal.z <= a.max.y,
        "the area nearest 40 m covers it: {a:?}"
    );
    let route = mesh.route(l.spawns[0], goal).expect("a route from the firing line to 40 m");
    assert!(route.last().unwrap().distance(goal) < 0.5, "{route:?}");
    let top = l.platform_top;
    let up = mesh.nearest_area(top).map(|i| &mesh.areas[i]).expect("an area on the platform");
    assert!((up.center.y - top.y).abs() < 0.3, "the platform is walked onto: {up:?}");
    assert!(mesh.route(l.spawns[0], top).is_some(), "a route up the ramp");
}

#[test]
fn unloading_takes_the_range_out() {
    let mut sim = range();
    let w = sim.app.world_mut();
    assert!(w.query_filtered::<(), With<RangePart>>().iter(w).count() > 10);
    greybox::unload(w);
    assert_eq!(w.query_filtered::<(), With<RangePart>>().iter(w).count(), 0);
    assert!(!w.contains_resource::<RangeLayout>());
    // And back, as a map change to it does.
    greybox::select(w, "chimera:range");
    greybox::respawn(w);
    assert!(w.contains_resource::<RangeLayout>());
}

#[test]
fn spawning_anywhere_still_spawns_at_the_firing_line() {
    // A minigame spawning on the floor's spots (`points = "scored"`,
    // `"random_hidden"`) gets the range's spawn points as its spots
    // (`map::code_maps::SpawnOnPoints`), facing down range.
    let mut sim = range();
    let l = layout(&sim);
    let p = sim.spawn_character(l.spawns[0], placeholder::ID);
    for _ in 0..8 {
        let at = chimera_sim::minigame::spawns::pick(sim.app.world_mut(), p, 5.0, Default::default())
            .expect("a spot");
        let behind = (at.translation - l.firing_line).dot(-l.dir);
        assert!((-0.5..3.5).contains(&behind), "a spawn {behind} m behind the firing line: {}", at.translation);
        let look = at.rotation * Vec3::NEG_Z;
        assert!(look.dot(l.dir) > 0.7, "facing down range: {look}");
    }
}
